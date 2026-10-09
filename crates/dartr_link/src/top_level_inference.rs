// Dart source: pkg/analyzer/lib/src/summary2/top_level_inference.dart

//! Top-level inference (Dart `TopLevelInference`, unit C10): override
//! inference ([`crate::instance_member_inferrer`]) and the types of fields
//! and top-level variables that come from their initializers (Dart
//! `_InitializerInference`, `_PropertyInducingElementTypeInference`).
//!
//! # On-demand inference
//!
//! Dart sets `element.typeInference` on every field and top-level
//! variable with an implicit type; the getter `element.type` runs it when
//! the type is not set yet. So the type of a variable is inferred when the
//! first reader asks for it: override inference (the type of an
//! overridden field), the initializer of another variable, a field formal
//! parameter. A variable that is read while its own initializer is
//! resolved is in a dependency cycle: every variable of the cycle gets
//! `dynamic` and the error `dependencyCycle`.
//!
//! This port installs [`TopLevelInference`] as the
//! `dartr_element::type_inference` hook of the linking thread while
//! override inference and [`TopLevelInference::perform`] run; the readers
//! of variable types call the hook when the type slot is empty.
//!
//! # Resolution
//!
//! The initializers are resolved by the [`LinkResolver`] that the driver
//! passes to [`crate::link::link_cycle`] (Dart `AstResolver`), see
//! [`crate::link::LinkResolver`].
//!
//! Not ported: `ConstantInitializersResolver` (the resolution of the
//! initializers of typed constants, for constant evaluation; the element
//! model does not keep resolved initializers yet).

use std::cell::RefCell;

use dartr_ast::{
    EnumConstantDeclaration, FieldFormalParameter, RegularFormalParameter, SuperFormalParameter,
    VariableDeclaration,
};
use dartr_element::type_inference::{
    PropertyTypeInference, ensure_property_type, with_property_type_inference,
};
use dartr_element::*;
use dartr_typesystem::{TypeExt, TypeSystem};
use indexmap::IndexMap;

use crate::dump::{fragments, has_implicit_type, is_origin_getter_setter, is_static};
use crate::link::{ExpressionRequest, ExpressionSource, LinkResolver, LinkResolverSession, Linker};
use crate::types::unit_ast;
use crate::types_builder::set_variable_type;

/// Dart `TopLevelInference.infer`.
pub fn infer(lk: &Linker<'_>, ctx: &Ctx<'_>, resolver: &dyn LinkResolver) {
    let session = resolver.session();
    let inference = TopLevelInference {
        lk,
        ctx,
        session: &*session,
        status: RefCell::new(IndexMap::new()),
        inferring: RefCell::new(Vec::new()),
    };
    // initializerInference.createNodes()
    let to_infer = inference.create_nodes();
    with_property_type_inference(&inference, || {
        // _performOverrideInference
        crate::instance_member_inferrer::perform(lk, ctx);
        // initializerInference.perform()
        for &element in &to_infer {
            // Will perform inference, if not done yet.
            ensure_property_type(ctx, element);
        }
    });
}

/// Dart `_InferenceStatus`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum InferenceStatus {
    NotInferred,
    BeingInferred,
    Inferred,
}

/// Dart `_InitializerInference` and the `_PropertyInducingElementTypeInference`
/// objects of its variables.
pub struct TopLevelInference<'l, 'c, 'a> {
    lk: &'l Linker<'l>,
    ctx: &'c Ctx<'a>,
    session: &'l dyn LinkResolverSession,
    /// Dart `_PropertyInducingElementTypeInference._status` of each variable
    /// with a type inference.
    status: RefCell<IndexMap<EId<PropertyInducingElement>, InferenceStatus>>,
    /// Dart `_inferring`: the stack of variables whose initializers are
    /// resolved now.
    inferring: RefCell<Vec<EId<PropertyInducingElement>>>,
}

impl PropertyTypeInference for TopLevelInference<'_, '_, '_> {
    /// Dart `PropertyInducingElementImpl.type` when `_type` is `null`.
    fn infer(&self, element: EId<PropertyInducingElement>) {
        if !self.status.borrow().contains_key(&element) {
            return;
        }
        if self.ctx.property_inducing(element).type_.get().is_some() {
            return;
        }
        let Some((t, is_type_inferred_from_initializer)) = self.perform(element) else {
            return;
        };
        self.ctx.property_inducing(element).flags.set(
            ElementFlags::PROPERTY_INDUCING_ELEMENT_IS_TYPE_INFERRED_FROM_INITIALIZER,
            is_type_inferred_from_initializer,
        );
        set_variable_type(self.ctx, element, t);
    }
}

/// Where the initializer of a variable is (Dart `initializerLibraryFragment`,
/// `scope`, `getInitializer`).
struct Initializer {
    lib: usize,
    unit: usize,
    /// The node with the expression, or the synthetic initializer of an
    /// enum constant.
    owner: InitializerNode,
}

#[derive(Clone, Copy)]
enum InitializerNode {
    Node(dartr_ast::NodeId),
    Synthetic(ConstExprId),
}

impl TopLevelInference<'_, '_, '_> {
    fn status_of(&self, element: EId<PropertyInducingElement>) -> InferenceStatus {
        self.status
            .borrow()
            .get(&element)
            .copied()
            .unwrap_or(InferenceStatus::Inferred)
    }

    fn set_status(&self, element: EId<PropertyInducingElement>, status: InferenceStatus) {
        self.status.borrow_mut().insert(element, status);
    }

    /// Dart `_InitializerInference.createNodes`: the variables to infer,
    /// in the order of `perform`.
    fn create_nodes(&self) -> Vec<EId<PropertyInducingElement>> {
        let ctx = self.ctx;
        let mut result = Vec::new();
        for b in &self.lk.builders {
            let l = ctx.get(b.element);
            let instances: Vec<EId<InstanceElement>> = l
                .classes
                .iter()
                .map(|e| e.upcast())
                .chain(l.enums.iter().map(|e| e.upcast()))
                .chain(l.extensions.iter().map(|e| e.upcast()))
                .chain(l.extension_types.iter().map(|e| e.upcast()))
                .chain(l.mixins.iter().map(|e| e.upcast()))
                .collect();
            for instance in instances {
                for &field in &ctx.instance(instance).fields {
                    self.add_variable_node(field.upcast(), &mut result);
                }
            }
            for &variable in &l.top_level_variables {
                self.add_variable_node(variable.upcast(), &mut result);
            }
        }
        result
    }

    /// Dart `_addVariableNode`.
    fn add_variable_node(
        &self,
        element: EId<PropertyInducingElement>,
        to_infer: &mut Vec<EId<PropertyInducingElement>>,
    ) {
        let ctx = self.ctx;
        if is_origin_getter_setter(ctx, element.raw()) {
            return;
        }
        if !has_implicit_type(ctx, element.raw()) {
            return;
        }
        to_infer.push(element);
        self.set_status(element, InferenceStatus::NotInferred);
    }

    /// Dart `_PropertyInducingElementTypeInference.perform`: the type and
    /// `isTypeInferredFromInitializer`. `None` for a variable without a
    /// node (the enum constants and `values`, whose types the element
    /// builder sets).
    fn perform(&self, element: EId<PropertyInducingElement>) -> Option<(TypeId, bool)> {
        let ctx = self.ctx;
        let lk = self.lk;
        let mut initializer: Option<Initializer> = None;
        let mut in_scope_primary_constructor_parameters: Option<Vec<EId<FormalParameterElement>>> =
            None;
        let mut has_node = false;
        for fragment in fragments(ctx, element.raw()) {
            let Some(&(lib, unit, node)) = lk.core.fragment_nodes.get(&fragment) else {
                continue;
            };
            has_node = true;
            let ast = unit_ast(lk, lib as u32, unit as u32);
            if ast.is::<EnumConstantDeclaration>(node) {
                // Dart: the synthetic `VariableDeclaration` of the enum
                // constant, with an `InstanceCreationExpression`.
                let field = FId::<FieldFragment>::from_raw(fragment);
                if let Some(expression) = ctx.fragment(field).constant_initializer {
                    initializer = Some(Initializer {
                        lib,
                        unit,
                        owner: InitializerNode::Synthetic(expression),
                    });
                }
            } else if let Some(v) = ast.cast::<VariableDeclaration>(node) {
                if ast.get(v).initializer.is_some() {
                    initializer = Some(Initializer {
                        lib,
                        unit,
                        owner: InitializerNode::Node(node),
                    });
                    if element.raw().tag() == Tag::Field
                        && !is_static(ctx, element.raw())
                        && !is_late(ctx, element)
                    {
                        in_scope_primary_constructor_parameters = enclosing_element(ctx, element)
                            .and_then(|e| primary_constructor(ctx, e))
                            .map(|c| ctx.get(c).formal_params.clone());
                    }
                }
            } else if let Some(p) = ast.cast::<RegularFormalParameter>(node) {
                let p = ast.get(p);
                if p.default_clause.is_some() {
                    initializer = Some(Initializer {
                        lib,
                        unit,
                        owner: InitializerNode::Node(node),
                    });
                } else if p.function_typed_suffix.is_none() {
                    self.set_status(element, InferenceStatus::Inferred);
                    return Some((TypeSystem::new(*ctx).object_question(), false));
                }
            } else if let Some(p) = ast.cast::<FieldFormalParameter>(node) {
                if ast.get(p).default_clause.is_some() {
                    initializer = Some(Initializer {
                        lib,
                        unit,
                        owner: InitializerNode::Node(node),
                    });
                }
            } else if let Some(p) = ast.cast::<SuperFormalParameter>(node)
                && ast.get(p).default_clause.is_some()
            {
                initializer = Some(Initializer {
                    lib,
                    unit,
                    owner: InitializerNode::Node(node),
                });
            }
        }
        if !has_node {
            return None;
        }

        let Some(initializer) = initializer else {
            self.set_status(element, InferenceStatus::Inferred);
            return Some((TypeId::DYNAMIC, false));
        };

        // With this status the type must be already set.
        if self.status_of(element) == InferenceStatus::Inferred {
            return Some((TypeId::DYNAMIC, false));
        }

        // If we are already inferring this element, we found a cycle.
        if self.status_of(element) == InferenceStatus::BeingInferred {
            let cycle: Vec<EId<PropertyInducingElement>> = {
                let inferring = self.inferring.borrow();
                let start = inferring.iter().position(|&e| e == element).unwrap_or(0);
                inferring[start..].to_vec()
            };
            let mut names: Vec<Name> = cycle
                .iter()
                .map(|&e| {
                    ctx.element_data(e.raw())
                        .and_then(|d| d.name)
                        .unwrap_or_else(|| ctx.name(""))
                })
                .collect();
            names.sort_by(|a, b| ctx.name_str(*a).cmp(ctx.name_str(*b)));
            let error = TopLevelInferenceError::DependencyCycle { cycle: names };
            for &e in &cycle {
                if self.status_of(e) == InferenceStatus::BeingInferred {
                    let data = ctx.property_inducing(e);
                    if data.type_inference_error.try_get().is_none() {
                        data.type_inference_error.set_once(error.clone());
                    }
                    set_variable_type(ctx, e, TypeId::DYNAMIC);
                    self.set_status(e, InferenceStatus::Inferred);
                }
            }
            return Some((TypeId::DYNAMIC, false));
        }

        // Push self into the stack, and mark.
        self.inferring.borrow_mut().push(element);
        self.set_status(element, InferenceStatus::BeingInferred);

        let enclosing = enclosing_element(ctx, element);
        let enclosing_class = enclosing.and_then(|e| e.raw().cast::<InterfaceElement>());
        let builder = &lk.builders[initializer.lib];
        let unit = &builder.units[initializer.unit];
        let request = ExpressionRequest {
            library: builder.element,
            fragment: unit.fragment,
            source: match initializer.owner {
                InitializerNode::Node(owner) => ExpressionSource::Unit {
                    parsed: &unit.parsed,
                    declared_fragments: &unit.declared_fragments,
                    owner,
                },
                InitializerNode::Synthetic(expression) => ExpressionSource::Synthetic {
                    ast: &lk.core.const_exprs.ast,
                    expression: expression.0,
                    unit: &unit.parsed,
                },
            },
            enclosing_instance: enclosing,
            enclosing_class,
            context_type: TypeId::UNKNOWN,
            in_scope_primary_constructor_parameters: in_scope_primary_constructor_parameters
                .as_deref(),
        };
        let initializer_type = self.session.resolve_expression(ctx, &request);

        // Pop self from the stack.
        let popped = self.inferring.borrow_mut().pop();
        debug_assert_eq!(popped, Some(element));

        // We might have found a cycle, and already set the type.
        if self.status_of(element) == InferenceStatus::Inferred {
            let t = ctx
                .property_inducing(element)
                .type_
                .get()
                .unwrap_or(TypeId::DYNAMIC);
            return Some((t, false));
        }
        self.set_status(element, InferenceStatus::Inferred);

        let initializer_type = initializer_type.unwrap_or(TypeId::INVALID);
        Some((self.refine_type(element, initializer_type), true))
    }

    /// Dart `_refineType`.
    fn refine_type(&self, element: EId<PropertyInducingElement>, t: TypeId) -> TypeId {
        let ctx = self.ctx;
        if ctx.is_dart_core_null(t) {
            // When `T` is `Null`, `p` has declared type `Object?`.
            if element.raw().tag() == Tag::Field && self.has_declaring_formal_parameter(element) {
                return TypeSystem::new(*ctx).object_question();
            }
            // Logic for older language versions.
            return TypeId::DYNAMIC;
        }
        t
    }

    /// Dart `FieldElementImpl.declaringFormalParameter != null`.
    fn has_declaring_formal_parameter(&self, element: EId<PropertyInducingElement>) -> bool {
        let ctx = self.ctx;
        self.lk
            .core
            .declaring_formal_parameters
            .iter()
            .any(|&(field, _)| {
                ctx.fragment(field)
                    .element
                    .try_get()
                    .is_some_and(|&e| e == element.raw())
            })
    }
}

/// Dart `VariableElementImpl.isLate`.
fn is_late(ctx: &Ctx<'_>, element: EId<PropertyInducingElement>) -> bool {
    let first = ctx.element_data(element.raw()).unwrap().first_fragment;
    ctx.fragment_data(first)
        .unwrap()
        .flags
        .has(FragmentFlags::VARIABLE_FRAGMENT_IS_LATE)
}

/// The instance element that encloses a field (Dart `enclosingElement`).
fn enclosing_element(
    ctx: &Ctx<'_>,
    element: EId<PropertyInducingElement>,
) -> Option<EId<InstanceElement>> {
    ctx.element_data(element.raw())
        .and_then(|d| d.enclosing)
        .and_then(|e| e.cast::<InstanceElement>())
}

/// Dart `InterfaceElementImpl.primaryConstructor`.
fn primary_constructor(
    ctx: &Ctx<'_>,
    element: EId<InstanceElement>,
) -> Option<EId<ConstructorElement>> {
    let interface = element.raw().cast::<InterfaceElement>()?;
    ctx.interface(interface)
        .constructors
        .iter()
        .copied()
        .find(|&c| {
            let first = ctx.get(c).first_fragment();
            ctx.fragment(first)
                .flags
                .has(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_PRIMARY)
        })
}
