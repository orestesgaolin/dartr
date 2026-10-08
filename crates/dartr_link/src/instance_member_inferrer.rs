// Dart source: pkg/analyzer/lib/src/summary2/instance_member_inferrer.dart,
// pkg/analyzer/lib/src/summary2/top_level_inference.dart
// (TopLevelInference._performOverrideInference, the parts of
// _InitializerInference that do not resolve an initializer)

//! Override inference (unit B5): the types of fields, getters, setters,
//! methods and their parameters that are not written in the source and
//! come from the members that they override, inherited covariance, the
//! induced modifiers of sealed classes and the types of initializing formal
//! parameters ([`perform`], Dart `InstanceMemberInferrer`).
//!
//! The overridden members and their combined signatures come from the
//! inheritance manager (unit A7, `dartr_typesystem`). The interfaces that it
//! computes during linking are cached on the elements of the cycle;
//! [`crate::link::link_cycle`] removes them before the store is frozen
//! (Dart: the linker has its own `InheritanceManager3`).
//!
//! The type of a field or top-level variable with an initializer comes from
//! the initializer (Dart `_PropertyInducingElementTypeInference`, unit C10:
//! needs the resolver). [`infer_variable_without_initializer`] is the part
//! of it that does not resolve an expression.

use dartr_ast::*;
use dartr_element::*;
use dartr_typesystem::inheritance_manager3::{Conflict, InheritanceManager3, Name as MemberName};
use dartr_typesystem::member;
use dartr_typesystem::type_algebra::{MapSubstitution, replace_type_parameters};
use dartr_typesystem::{TypeExt, TypeSystem};
use indexmap::IndexSet;

use crate::dump::{has_implicit_return_type, has_implicit_type, is_final, is_origin_getter_setter, is_origin_variable, is_static};
use crate::link::Linker;
use crate::types::unit_ast;
use crate::types_builder::{set_return_type, set_variable_type};

/// Dart `TopLevelInference._performOverrideInference`: the interface
/// elements of every library of the cycle (`children` order: classes,
/// enums, extension types, mixins), then `InstanceMemberInferrer.perform`.
pub fn perform(lk: &Linker<'_>, ctx: &Ctx<'_>) {
    let mut elements: Vec<EId<InterfaceElement>> = Vec::new();
    for b in &lk.builders {
        let l = ctx.get(b.element);
        elements.extend(l.classes.iter().map(|e| e.upcast()));
        elements.extend(l.enums.iter().map(|e| e.upcast()));
        elements.extend(l.extension_types.iter().map(|e| e.upcast()));
        elements.extend(l.mixins.iter().map(|e| e.upcast()));
    }
    let mut inferrer = InstanceMemberInferrer {
        lk,
        ctx,
        inheritance: InheritanceManager3::new(*ctx),
        interfaces_to_infer: IndexSet::new(),
        current: None,
    };
    inferrer.interfaces_to_infer.extend(elements.iter().copied());
    for &element in &elements {
        inferrer.infer_class(element);
    }

    // _InitializerInference.perform for the fields of extensions and the
    // top-level variables (the fields of interface elements are done at the
    // end of `_inferClass`, see there).
    for b in &lk.builders {
        let l = ctx.get(b.element);
        for &e in &l.extensions {
            for &f in &ctx.instance(e.upcast()).fields {
                infer_variable_without_initializer(lk, ctx, f.upcast());
            }
        }
        for &v in &l.top_level_variables {
            infer_variable_without_initializer(lk, ctx, v.upcast());
        }
    }
}

/// Dart `InstanceMemberInferrer`.
struct InstanceMemberInferrer<'l, 'c, 'a> {
    lk: &'l Linker<'l>,
    ctx: &'c Ctx<'a>,
    inheritance: InheritanceManager3<'a>,
    interfaces_to_infer: IndexSet<EId<InterfaceElement>>,
    current: Option<EId<InterfaceElement>>,
}

/// The parameter of an executable or of a function type that
/// `_getCorrespondingParameter` matches.
#[derive(Clone, Copy)]
struct ParamDesc {
    name: Option<Name>,
    kind: ParameterKind,
}

/// Dart `_getCorrespondingParameter`: the index in [parameters] of the
/// parameter that corresponds to [parameter] at [index].
fn corresponding_parameter(parameter: ParamDesc, index: usize, parameters: &[ParamDesc]) -> Option<usize> {
    if parameter.kind.is_named() {
        // lastWhereOrNull
        return parameters
            .iter()
            .rposition(|p| p.kind.is_named() && p.name == parameter.name);
    }
    if index < parameters.len() && !parameters[index].kind.is_named() {
        return Some(index);
    }
    None
}

fn element_param_desc(ctx: &Ctx<'_>, p: ElementId) -> ParamDesc {
    let p = ctx.get(EId::<FormalParameterElement>::from_raw(p));
    ParamDesc {
        name: p.name,
        kind: p.kind,
    }
}

/// Dart `FormalParameterElementImpl.isCovariant` of a base parameter.
fn is_covariant_parameter(ctx: &Ctx<'_>, p: EId<FormalParameterElement>) -> bool {
    ctx.get(p)
        .flags
        .has(ElementFlags::FORMAL_PARAMETER_ELEMENT_IS_COVARIANT)
}

fn set_covariant(ctx: &Ctx<'_>, p: EId<FormalParameterElement>) {
    ctx.get(p)
        .flags
        .set(ElementFlags::FORMAL_PARAMETER_ELEMENT_IS_COVARIANT, true);
}

/// Dart `ExecutableElementImpl._type = null`: the function type of an
/// executable is interned with the types and the covariance of its
/// parameters (Dart reads them from the parameter elements), so it is
/// reset when they change.
fn reset_executable_type(ctx: &Ctx<'_>, e: EId<ExecutableElement>) {
    ctx.executable(e).type_.set(None);
}

/// The function type parameters of [t] (`FunctionTypeImpl.formalParameters`).
fn function_params<'a>(ctx: &Ctx<'a>, t: TypeId) -> &'a [FnParam] {
    match *ctx.ty(t) {
        TypeKind::Function(f) => ctx.list(f.params),
        _ => &[],
    }
}

fn function_return_type(ctx: &Ctx<'_>, t: TypeId) -> TypeId {
    match *ctx.ty(t) {
        TypeKind::Function(f) => f.ret,
        _ => TypeId::DYNAMIC,
    }
}

fn function_type_params(ctx: &Ctx<'_>, t: TypeId) -> usize {
    match *ctx.ty(t) {
        TypeKind::Function(f) => ctx.list(f.type_params).len(),
        _ => 0,
    }
}

/// Dart `_isCovariantSetter` (of the base element).
fn is_covariant_setter(ctx: &Ctx<'_>, e: ElemRef) -> bool {
    let base = member::base_element(ctx, e);
    if !matches!(base.tag(), Tag::Getter | Tag::Setter) {
        return false;
    }
    ctx.executable(EId::from_raw(base))
        .formal_params
        .first()
        .is_some_and(|&p| is_covariant_parameter(ctx, p))
}

/// The element kinds of `_allSameElementKind` (Dart `ElementKind`).
fn element_kind(ctx: &Ctx<'_>, e: ElemRef) -> Tag {
    member::base_element(ctx, e).tag()
}

enum Accessor {
    Getter(EId<GetterElement>),
    Setter(EId<SetterElement>),
    Field(EId<FieldElement>),
}

impl<'a> InstanceMemberInferrer<'_, '_, 'a> {
    fn current(&self) -> EId<InterfaceElement> {
        self.current.expect("currentInterfaceElement")
    }

    fn library_of(&self, e: ElementId) -> Option<EId<LibraryElement>> {
        self.ctx.element_data(e).and_then(|d| d.library)
    }

    fn name_text(&self, e: ElementId) -> &'a str {
        self.ctx
            .element_data(e)
            .and_then(|d| d.name)
            .map(|n| self.ctx.name_str(n))
            .unwrap_or("")
    }

    /// `inheritance.getOverridden(currentInterfaceElement, name)`.
    fn get_overridden(&self, name: MemberName) -> Option<Vec<ElemRef>> {
        self.inheritance.get_overridden(self.current(), name)
    }

    /// Dart `_inferAccessorOrField`.
    fn infer_accessor_or_field(&self, accessor: Accessor) {
        let ctx = self.ctx;
        let (library, element_name): (Option<EId<LibraryElement>>, &str) = match accessor {
            Accessor::Getter(getter) => {
                if is_origin_variable(ctx, getter.raw()) || is_static(ctx, getter.raw()) {
                    return;
                }
                (self.library_of(getter.raw()), self.name_text(getter.raw()))
            }
            Accessor::Setter(setter) => {
                if is_origin_variable(ctx, setter.raw()) || is_static(ctx, setter.raw()) {
                    return;
                }
                (self.library_of(setter.raw()), self.name_text(setter.raw()))
            }
            Accessor::Field(field) => {
                if is_static(ctx, field.raw()) {
                    return;
                }
                if is_origin_getter_setter(ctx, field.raw()) {
                    return;
                }
                (self.library_of(field.raw()), self.name_text(field.raw()))
            }
        };

        let getter_name = MemberName::new(ctx, library, element_name);
        let overridden_getters: Vec<ElemRef> = self
            .get_overridden(getter_name)
            .map(|list| list.into_iter().filter(|&e| member::is_getter(ctx, e)).collect())
            .unwrap_or_default();

        let setter_name = MemberName::new(ctx, library, &format!("{element_name}="));
        let overridden_setters: Vec<ElemRef> = self.get_overridden(setter_name).unwrap_or_default();

        let combined_getter_type = || -> TypeId {
            match self
                .inheritance
                .combine_signature_types(&overridden_getters, getter_name, None)
            {
                Some(t) => function_return_type(ctx, t),
                None => TypeId::DYNAMIC,
            }
        };
        let combined_setter_type = || -> TypeId {
            if let Some(t) = self
                .inheritance
                .combine_signature_types(&overridden_setters, setter_name, None)
                && let Some(p) = function_params(ctx, t).first()
            {
                return p.ty;
            }
            TypeId::DYNAMIC
        };

        match accessor {
            Accessor::Getter(getter) => {
                if !has_implicit_return_type(ctx, getter.upcast()) {
                    return;
                }
                let return_type = if !overridden_getters.is_empty() {
                    combined_getter_type()
                } else if !overridden_setters.is_empty() {
                    combined_setter_type()
                } else {
                    return;
                };
                set_return_type(ctx, getter.upcast(), return_type);
                if let Some(field) = ctx.get(getter).variable.get() {
                    set_variable_type(ctx, field, return_type);
                }
            }
            Accessor::Setter(setter) => {
                let Some(&value) = ctx.get(setter).formal_params.first() else {
                    return;
                };
                if overridden_setters.iter().any(|&s| is_covariant_setter(ctx, s)) {
                    set_covariant(ctx, value);
                    reset_executable_type(ctx, setter.upcast());
                }

                if !has_implicit_type(ctx, value.raw()) {
                    return;
                }

                let set_setter_value_type = |value_type: TypeId| {
                    ctx.get(value).type_.set(Some(value_type));
                    reset_executable_type(ctx, setter.upcast());
                    if let Some(field) = ctx.get(setter).variable.get()
                        && ctx.property_inducing(field).getter.is_none()
                    {
                        set_variable_type(ctx, field, value_type);
                    }
                };

                if !overridden_getters.is_empty() && overridden_setters.is_empty() {
                    set_setter_value_type(combined_getter_type());
                    return;
                }
                if !overridden_setters.is_empty() {
                    set_setter_value_type(combined_setter_type());
                }
            }
            Accessor::Field(field) => {
                if let Some(setter) = ctx.property_inducing(field.upcast()).setter
                    && overridden_setters.iter().any(|&s| is_covariant_setter(ctx, s))
                    && let Some(&value) = ctx.get(setter).formal_params.first()
                {
                    set_covariant(ctx, value);
                    reset_executable_type(ctx, setter.upcast());
                }

                if !has_implicit_type(ctx, field.raw()) {
                    return;
                }

                let field_type = if !overridden_getters.is_empty() && overridden_setters.is_empty() {
                    combined_getter_type()
                } else if overridden_getters.is_empty() && !overridden_setters.is_empty() {
                    combined_setter_type()
                } else if !overridden_getters.is_empty() && !overridden_setters.is_empty() {
                    if is_final(ctx, field.raw()) {
                        combined_getter_type()
                    } else {
                        let getter_type = combined_getter_type();
                        let setter_type = combined_setter_type();
                        // Dart: ==
                        if !TypeSystem::new(*ctx).dart_eq(getter_type, setter_type) {
                            return;
                        }
                        getter_type
                    }
                } else {
                    return;
                };
                set_variable_type(ctx, field.upcast(), field_type);
            }
        }
    }

    /// Dart `_inferClass`.
    fn infer_class(&mut self, element: EId<InterfaceElement>) {
        if !self.interfaces_to_infer.shift_remove(&element) {
            return;
        }
        let ctx = self.ctx;

        crate::outline::set_induced_modifier(ctx, element);

        // Ensure that all of instance members in the supertypes have had
        // types inferred for them.
        let i = ctx.interface(element);
        let mut supertypes: Vec<TypeId> = Vec::new();
        supertypes.extend(i.supertype.get());
        supertypes.extend(ctx.list(i.mixins.get().unwrap_or(TypeList::EMPTY)));
        supertypes.extend(ctx.list(i.interfaces.get().unwrap_or(TypeList::EMPTY)));
        for t in supertypes {
            if let Some(e) = ctx.interface_element(t) {
                self.infer_class(e);
            }
        }

        // Then infer the types for the members.
        self.current = Some(element);
        for &field in &i.fields {
            self.infer_accessor_or_field(Accessor::Field(field));
        }
        for &getter in &i.getters {
            self.infer_accessor_or_field(Accessor::Getter(getter));
        }
        for &setter in &i.setters {
            self.infer_accessor_or_field(Accessor::Setter(setter));
        }
        for &method in &i.methods {
            self.infer_executable(method);
        }

        // _InitializerInference: a field without a type and without an
        // initializer is `dynamic` (Dart infers it on the first read of its
        // type; the subtypes of this class read it after this point).
        for &field in &i.fields {
            infer_variable_without_initializer(self.lk, ctx, field.upcast());
        }

        // Infer initializing formal parameter types. This must happen after
        // field types are inferred.
        for &constructor in &i.constructors {
            self.infer_constructor(element, constructor);
        }
    }

    /// Dart `_inferConstructor`.
    fn infer_constructor(&self, class: EId<InterfaceElement>, constructor: EId<ConstructorElement>) {
        let ctx = self.ctx;
        let c = ctx.get(constructor);
        let mut changed = false;
        for &p in &c.formal_params {
            if !has_implicit_type(ctx, p.raw()) {
                continue;
            }
            let pe = ctx.get(p);
            match p.raw().tag() {
                Tag::FieldFormalParameter if pe.first_fragment().raw().tag() == Tag::FieldFormalParameter => {
                    if let Some(field) = pe.field.get() {
                        // Dart reads `field.type`, which infers it from the
                        // initializer when needed (unit C10).
                        if let Some(t) = ctx.get(field).type_.get() {
                            pe.type_.set(Some(t));
                            changed = true;
                        }
                    }
                }
                Tag::SuperFormalParameter if pe.first_fragment().raw().tag() == Tag::SuperFormalParameter => {
                    let t = crate::outline::super_constructor_parameter_type(ctx, class, constructor, p);
                    pe.type_.set(Some(t.unwrap_or(TypeId::DYNAMIC)));
                    changed = true;
                }
                _ => {}
            }
        }
        if changed {
            reset_executable_type(ctx, constructor.upcast());
        }

        if let Some(class_element) = class.raw().cast::<ClassElement>()
            && crate::dump::is_mixin_application(ctx, class_element.raw())
        {
            self.infer_mixin_application_constructor(class_element, constructor);
        }
    }

    /// Dart `_inferExecutable`.
    fn infer_executable(&self, element: EId<MethodElement>) {
        let ctx = self.ctx;
        if !crate::dump::first_fragment_has(ctx, element.raw(), FragmentFlags::METHOD_FRAGMENT_IS_ORIGIN_DECLARATION)
            || is_static(ctx, element.raw())
        {
            return;
        }

        let Some(name) = MemberName::for_element(ctx, ElemRef::Base(element.raw())) else {
            return;
        };

        let Some(overridden_elements) = self.get_overridden(name) else {
            return;
        };
        // _allSameElementKind
        if !overridden_elements
            .iter()
            .all(|&e| element_kind(ctx, e) == Tag::Method)
        {
            return;
        }

        let m = ctx.get(element);
        let parameters: Vec<EId<FormalParameterElement>> = m.formal_params.clone();
        let mut combined_signature_type: Option<TypeId> = None;
        let has_implicit_type = has_implicit_return_type(ctx, element.upcast())
            || parameters.iter().any(|p| crate::dump::has_implicit_type(ctx, p.raw()));
        if has_implicit_type {
            let mut conflicts: Vec<Conflict> = Vec::new();
            combined_signature_type =
                self.inheritance
                    .combine_signature_types(&overridden_elements, name, Some(&mut conflicts));
            if let Some(t) = combined_signature_type {
                combined_signature_type = self.to_overridden_function_type(element, t);
            } else {
                let mut candidate_signatures = String::from("<unknown>");
                if conflicts.len() == 1
                    && let Conflict::Candidates { candidates, .. } = &conflicts[0]
                {
                    candidate_signatures = candidates
                        .iter()
                        .map(|&candidate| {
                            let class_name = member::enclosing_element(ctx, candidate)
                                .and_then(|e| ctx.element_data(e))
                                .and_then(|d| d.name)
                                .map(|n| ctx.name_str(n))
                                .unwrap_or("");
                            let type_str = type_display_string_with(
                                ctx,
                                member::type_(ctx, candidate),
                                DisplayOptions::default(),
                            );
                            format!("{class_name}.{} ({type_str})", name.text(ctx))
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                }
                m.type_inference_error
                    .set_once(TopLevelInferenceError::OverrideNoCombinedSuperSignature {
                        candidate_signatures: candidate_signatures.into(),
                    });
            }
        }

        // Infer the return type.
        if has_implicit_return_type(ctx, element.upcast()) && self.name_text(element.raw()) != "[]=" {
            let return_type = match combined_signature_type {
                Some(t) => function_return_type(ctx, t),
                None => TypeId::DYNAMIC,
            };
            set_return_type(ctx, element.upcast(), return_type);
        }

        // Infer the parameter types.
        let overridden_params: Vec<Vec<(ParamDesc, bool)>> = overridden_elements
            .iter()
            .map(|&o| {
                member::formal_parameters(ctx, o)
                    .into_iter()
                    .map(|p| {
                        let base = member::base_element(ctx, p);
                        (element_param_desc(ctx, base), member::is_covariant(ctx, p))
                    })
                    .collect()
            })
            .collect();
        let combined_params: Option<(Vec<ParamDesc>, &[FnParam])> = combined_signature_type.map(|t| {
            let params = function_params(ctx, t);
            (
                params
                    .iter()
                    .map(|p| ParamDesc {
                        name: p.name,
                        kind: p.kind,
                    })
                    .collect(),
                params,
            )
        });
        for (index, &parameter) in parameters.iter().enumerate() {
            let desc = element_param_desc(ctx, parameter.raw());
            if !is_covariant_parameter(ctx, parameter) {
                // _inferParameterCovariance
                for o in &overridden_params {
                    let descs: Vec<ParamDesc> = o.iter().map(|(d, _)| *d).collect();
                    if let Some(i) = corresponding_parameter(desc, index, &descs)
                        && o[i].1
                    {
                        set_covariant(ctx, parameter);
                        break;
                    }
                }
            }
            if crate::dump::has_implicit_type(ctx, parameter.raw()) {
                // _inferParameterType
                let t = match &combined_params {
                    Some((descs, params)) => match corresponding_parameter(desc, index, descs) {
                        Some(i) => params[i].ty,
                        None => TypeId::DYNAMIC,
                    },
                    None => TypeId::DYNAMIC,
                };
                ctx.get(parameter).type_.set(Some(t));
            }
        }
        reset_executable_type(ctx, element.upcast());

        self.reset_operator_equal_parameter_type_to_dynamic(element, &overridden_elements);
    }

    /// Dart `_inferMixinApplicationConstructor`. The index of the
    /// constructor among the constructors of the class is used for the
    /// accessible constructors of the superclass (also factories, as in
    /// Dart).
    fn infer_mixin_application_constructor(&self, class: EId<ClassElement>, constructor: EId<ConstructorElement>) {
        let ctx = self.ctx;
        let Some(super_type) = ctx.get(class).supertype.get() else {
            return;
        };
        let Some(super_element) = ctx.interface_element(super_type) else {
            return;
        };
        let Some(index) = ctx.get(class).constructors.iter().position(|&c| c == constructor) else {
            return;
        };
        let library = self.library_of(class.raw());
        let super_constructors: Vec<EId<ConstructorElement>> = ctx
            .interface(super_element)
            .constructors
            .iter()
            .copied()
            .filter(|&c| {
                // isAccessibleIn(classElement.library)
                let name = self.name_text(c.raw());
                !name.starts_with('_') || self.library_of(c.raw()) == library
            })
            .collect();
        let Some(&base_constructor) = super_constructors.get(index) else {
            return;
        };
        let substitution = MapSubstitution::from_interface_type(ctx, super_type);
        let base_params = &ctx.get(base_constructor).formal_params;
        for (&parameter, &base_parameter) in ctx.get(constructor).formal_params.iter().zip(base_params.iter()) {
            let t = ctx.get(base_parameter).type_.get().unwrap_or(TypeId::INVALID);
            ctx.get(parameter)
                .type_
                .set(Some(substitution.substitute_type(ctx, t)));
        }
        reset_executable_type(ctx, constructor.upcast());
        // Dart also sets the static types of the arguments of the
        // `SuperConstructorInvocation` (pseudo expressions); the port does
        // not build them.
    }

    /// Dart `_resetOperatorEqualParameterTypeToDynamic`.
    fn reset_operator_equal_parameter_type_to_dynamic(&self, element: EId<MethodElement>, overridden_elements: &[ElemRef]) {
        let ctx = self.ctx;
        if self.name_text(element.raw()) != "==" {
            return;
        }
        let m = ctx.get(element);
        let flag = &m.is_operator_equal_with_parameter_type_from_object;
        if m.formal_params.len() != 1 {
            flag.set(false);
            return;
        }
        if !crate::dump::has_implicit_type(ctx, m.formal_params[0].raw()) {
            flag.set(false);
            return;
        }
        for &overridden in overridden_elements {
            let overridden = member::base_element(ctx, overridden);
            // Skip Object itself.
            let enclosing = ctx.element_data(overridden).and_then(|d| d.enclosing);
            if let Some(enclosing) = enclosing
                && enclosing.tag() == Tag::Class
                && member::is_dart_core_object_element(ctx, enclosing)
            {
                continue;
            }
            // Keep the type if it is not directly from Object.
            if let Some(method) = overridden.cast::<MethodElement>()
                && !ctx
                    .get(method)
                    .is_operator_equal_with_parameter_type_from_object
                    .get()
            {
                flag.set(false);
                return;
            }
        }
        flag.set(true);
    }

    /// Dart `_toOverriddenFunctionType`.
    fn to_overridden_function_type(&self, element: EId<MethodElement>, overridden_type: TypeId) -> Option<TypeId> {
        let ctx = self.ctx;
        let element_type_parameters = &ctx.get(element).type_params;
        if element_type_parameters.len() != function_type_params(ctx, overridden_type) {
            return None;
        }
        if element_type_parameters.is_empty() {
            return Some(overridden_type);
        }
        Some(replace_type_parameters(ctx, overridden_type, element_type_parameters))
    }
}

/// The part of Dart `_PropertyInducingElementTypeInference.perform` that
/// does not resolve an initializer: a field or top-level variable with an
/// implicit type and no type yet gets `dynamic` when no fragment has an
/// initializer, and `Object?` for a declaring formal parameter without a
/// default value (not function-typed). A variable with an initializer keeps
/// no type (unit C10).
pub fn infer_variable_without_initializer(lk: &Linker<'_>, ctx: &Ctx<'_>, element: EId<PropertyInducingElement>) {
    if is_origin_getter_setter(ctx, element.raw()) || !has_implicit_type(ctx, element.raw()) {
        return;
    }
    if ctx.property_inducing(element).type_.get().is_some() {
        return;
    }
    let mut has_initializer = false;
    let mut object_question = false;
    for fragment in crate::dump::fragments(ctx, element.raw()) {
        // Enum constants and `values` have a synthetic initializer in Dart
        // (a `VariableDeclaration` that the port does not build).
        let Some(&(lib, unit, node)) = lk.core.fragment_nodes.get(&fragment) else {
            has_initializer = true;
            continue;
        };
        let ast = unit_ast(lk, lib as u32, unit as u32);
        if let Some(v) = ast.cast::<VariableDeclaration>(node) {
            if ast.get(v).initializer.is_some() {
                has_initializer = true;
            }
        } else if let Some(p) = ast.cast::<RegularFormalParameter>(node) {
            let p = ast.get(p);
            if p.default_clause.is_some() {
                has_initializer = true;
            } else if p.function_typed_suffix.is_none() {
                object_question = true;
                break;
            }
        } else if let Some(p) = ast.cast::<FieldFormalParameter>(node) {
            if ast.get(p).default_clause.is_some() {
                has_initializer = true;
            }
        } else if let Some(p) = ast.cast::<SuperFormalParameter>(node) {
            if ast.get(p).default_clause.is_some() {
                has_initializer = true;
            }
        } else {
            has_initializer = true;
        }
    }
    if object_question {
        set_variable_type(ctx, element, TypeSystem::new(*ctx).object_question());
        return;
    }
    if !has_initializer {
        set_variable_type(ctx, element, TypeId::DYNAMIC);
    }
}
