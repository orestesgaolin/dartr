// Dart source: pkg/analyzer/lib/src/dart/element/type.dart (TypeImpl and
// subclasses: nullabilitySuffix, withNullability, withAlias, isDartCore*,
// isBottom, asInstanceOf, superclass, interfaces, mixins,
// superclassConstraints, allSupertypes, representationType, instantiate,
// referencesAny, the FunctionTypeImpl and RecordTypeImpl factories),
// pkg/analyzer/lib/src/dart/element/element.dart (InterfaceElementImpl
// .instantiateImpl, TypeAliasElementImpl.instantiateImpl,
// TypeParameterElementImpl.instantiate / .synthetic),
// pkg/analyzer/lib/src/utilities/extensions/element.dart
// (TypeParameterElementImplExtension.freshCopy)

//! The methods of Dart `TypeImpl` and of the elements that make types, as
//! an extension trait on [`Ctx`]: `ctx.with_nullability(t, n)`,
//! `ctx.is_dart_core_object(t)`, `ctx.as_instance_of(t, e)`, ...
//!
//! `TypeId ==` is Dart `identical`; Dart `==` on types is
//! [`TypeExt::dart_eq`] (see `equality.rs`).

use dartr_element::{
    AliasId, AliasRef, ClassElement, Ctx, EId, ElementData, ElementId, ElementStore,
    ExtensionTypeElement, FnParam, FunctionTypeData, InterfaceElement, LibraryElement,
    MixinElement, Name, NamedType, Nullability, ParameterKind, TypeAliasElement, TypeId, TypeKind,
    TypeList, TypeParameterElement, TypeParameterFragment, Variance,
};
use dartr_element::{ElemRef, FragmentData, MemberId, StoreId, TypeAliasFragment};

use crate::class_hierarchy;
use crate::type_algebra::MapSubstitution;

/// The store where fresh elements (fresh type parameters) go: the local
/// arena of a body analysis task, else the cycle being linked, else the
/// global synthetic store (design §1.4).
pub fn fresh_store<'a>(ctx: &Ctx<'a>) -> &'a ElementStore {
    if let Some(local) = ctx.local {
        &local.store
    } else if let Some(current) = ctx.current {
        current
    } else {
        &ctx.world.generation.synthetic
    }
}

/// Whether [t] mentions an element of the store [store] (an interface,
/// type alias or type parameter element, also in bounds of the type
/// parameters of function types and in the parameter elements).
pub fn type_mentions_store(ctx: &Ctx<'_>, t: TypeId, store: StoreId) -> bool {
    fn alias_mentions(ctx: &Ctx<'_>, alias: Option<AliasId>, store: StoreId) -> bool {
        alias.is_some_and(|a| {
            let a = ctx.alias(a);
            a.element.store() == store
                || ctx
                    .list(a.args)
                    .iter()
                    .any(|&t| type_mentions_store(ctx, t, store))
        })
    }
    match *ctx.ty(t) {
        TypeKind::Interface {
            element,
            args,
            alias,
            ..
        } => {
            element.store() == store
                || ctx
                    .list(args)
                    .iter()
                    .any(|&t| type_mentions_store(ctx, t, store))
                || alias_mentions(ctx, alias, store)
        }
        TypeKind::Function(f) => {
            ctx.list(f.type_params).iter().any(|&p| {
                p.store() == store
                    || ctx
                        .get(p)
                        .bound
                        .get()
                        .is_some_and(|b| type_mentions_store(ctx, b, store))
            }) || ctx.list(f.params).iter().any(|p| {
                type_mentions_store(ctx, p.ty, store)
                    || p.element.is_some_and(|e| match e {
                        ElemRef::Base(b) => b.store() == store,
                        ElemRef::Member(m) => member_mentions_store(ctx, m, store),
                    })
            }) || type_mentions_store(ctx, f.ret, store)
                || alias_mentions(ctx, f.alias, store)
        }
        TypeKind::Record {
            positional,
            named,
            alias,
            ..
        } => {
            ctx.list(positional)
                .iter()
                .any(|&t| type_mentions_store(ctx, t, store))
                || ctx
                    .list(named)
                    .iter()
                    .any(|n| type_mentions_store(ctx, n.ty, store))
                || alias_mentions(ctx, alias, store)
        }
        TypeKind::TypeParameter {
            param,
            promoted_bound,
            alias,
            ..
        } => {
            param.store() == store
                || promoted_bound.is_some_and(|b| type_mentions_store(ctx, b, store))
                || alias_mentions(ctx, alias, store)
        }
        TypeKind::Dynamic
        | TypeKind::Void
        | TypeKind::Invalid
        | TypeKind::Unknown
        | TypeKind::Never(_) => false,
    }
}

/// Whether the member [m] (its base element or its substitution) mentions
/// an element of the store [store].
pub fn member_mentions_store(ctx: &Ctx<'_>, m: MemberId, store: StoreId) -> bool {
    let member = ctx.member(m);
    member.base.store() == store
        || ctx
            .subst(member.subst)
            .iter()
            .any(|&(p, t)| p.store() == store || type_mentions_store(ctx, t, store))
}

/// The context for a lazy shared cache (design §2.3): `ctx.global()`, and
/// without the cycle being linked when [mentions_current] is `false`. A
/// cache on a frozen element (or a member of frozen elements) is seen by
/// every context that sees that element, so what it creates (fresh type
/// parameters, synthesized members) must not go to a cycle that is being
/// linked: other cycles that are linked at the same time cannot see it.
pub fn cache_ctx<'a>(ctx: &Ctx<'a>, mentions_current: impl FnOnce(StoreId) -> bool) -> Ctx<'a> {
    let mut global = ctx.global();
    if let Some(current) = global.current
        && !mentions_current(current.id)
    {
        global.current = None;
    }
    global
}

/// Whether the parameter kind is required positional (`isRequiredPositional`).
pub fn is_required_positional(kind: ParameterKind) -> bool {
    matches!(kind, ParameterKind::Required)
}

/// `isOptionalPositional`.
pub fn is_optional_positional(kind: ParameterKind) -> bool {
    matches!(kind, ParameterKind::Positional)
}

/// `isPositional`.
pub fn is_positional(kind: ParameterKind) -> bool {
    matches!(kind, ParameterKind::Required | ParameterKind::Positional)
}

/// `isNamed`.
pub fn is_named(kind: ParameterKind) -> bool {
    matches!(kind, ParameterKind::Named | ParameterKind::NamedRequired)
}

/// `isRequiredNamed`.
pub fn is_required_named(kind: ParameterKind) -> bool {
    matches!(kind, ParameterKind::NamedRequired)
}

/// `isRequired` (required positional or required named).
pub fn is_required(kind: ParameterKind) -> bool {
    matches!(kind, ParameterKind::Required | ParameterKind::NamedRequired)
}

/// `isOptional`.
pub fn is_optional(kind: ParameterKind) -> bool {
    matches!(kind, ParameterKind::Positional | ParameterKind::Named)
}

/// The methods of Dart `TypeImpl` (and helpers of the elements that make
/// types). Implemented for [`Ctx`].
pub trait TypeExt<'a> {
    fn ctx(&self) -> Ctx<'a>;

    // ------------------------------------------------------------ elements

    /// The name of an element (`element.name`), `None` for unnamed elements.
    fn element_name(&self, element: ElementId) -> Option<&'a str> {
        let ctx = self.ctx();
        let name = ctx.element_data(element)?.name?;
        Some(ctx.name_str(name))
    }

    /// The URI of a library (`library.uri`), as text.
    fn library_uri(&self, library: EId<LibraryElement>) -> &'a str {
        let ctx = self.ctx();
        let unit = ctx.get(library).first_fragment();
        &ctx.fragment(unit).source.uri
    }

    /// The URI of the library of an element.
    fn element_library_uri(&self, element: ElementId) -> Option<&'a str> {
        let ctx = self.ctx();
        let library = ctx.element_data(element)?.library?;
        Some(self.library_uri(library))
    }

    /// The name of the library of an element (`element.library.name`).
    fn element_library_name(&self, element: ElementId) -> Option<&'a str> {
        let ctx = self.ctx();
        let library = ctx.element_data(element)?.library?;
        self.element_name(library.raw())
    }

    /// Whether [element] is named [name] and declared in the library named
    /// [library_name] (`element.name == name && element.library.isDartCore`;
    /// Dart `isDartCore` is `name == "dart.core"`, `isDartAsync` is
    /// `name == "dart.async"`).
    fn is_element(&self, element: ElementId, library_name: &str, name: &str) -> bool {
        self.element_name(element) == Some(name)
            && self.element_library_name(element) == Some(library_name)
    }

    /// The type parameters of an interface element.
    fn interface_type_parameters(
        &self,
        element: EId<InterfaceElement>,
    ) -> &'a [EId<TypeParameterElement>] {
        &self.ctx().interface(element).type_params
    }

    /// `InterfaceElementImpl.supertype`.
    fn element_supertype(&self, element: EId<InterfaceElement>) -> Option<TypeId> {
        self.ctx().interface(element).supertype.get()
    }

    /// `InterfaceElementImpl.interfaces`.
    fn element_interfaces(&self, element: EId<InterfaceElement>) -> &'a [TypeId] {
        let ctx = self.ctx();
        ctx.list(ctx.interface(element).interfaces.get().unwrap_or_default())
    }

    /// `InterfaceElementImpl.mixins`.
    fn element_mixins(&self, element: EId<InterfaceElement>) -> &'a [TypeId] {
        let ctx = self.ctx();
        ctx.list(ctx.interface(element).mixins.get().unwrap_or_default())
    }

    /// `MixinElementImpl.superclassConstraints` (empty for other elements).
    fn element_superclass_constraints(&self, element: EId<InterfaceElement>) -> &'a [TypeId] {
        let ctx = self.ctx();
        match element.cast::<MixinElement>() {
            Some(mixin) => ctx.list(
                ctx.get(mixin)
                    .superclass_constraints
                    .get()
                    .unwrap_or_default(),
            ),
            None => &[],
        }
    }

    /// `TypeParameterElementImpl.bound`.
    fn type_parameter_bound(&self, param: EId<TypeParameterElement>) -> Option<TypeId> {
        self.ctx().get(param).bound.get()
    }

    /// `TypeParameterElementImpl.variance` (covariant when not declared).
    fn type_parameter_variance(&self, param: EId<TypeParameterElement>) -> Variance {
        self.ctx()
            .get(param)
            .variance
            .unwrap_or(Variance::Covariant)
    }

    /// `TypeParameterElementImpl.isLegacyCovariant`.
    fn type_parameter_is_legacy_covariant(&self, param: EId<TypeParameterElement>) -> bool {
        self.ctx().get(param).variance.is_none()
    }

    /// Creates a type parameter element with [name], [variance] and
    /// [bound] in the [fresh_store] (`TypeParameterElementImpl.synthetic`).
    fn new_type_parameter(
        &self,
        name: Option<Name>,
        variance: Option<Variance>,
        bound: Option<TypeId>,
    ) -> EId<TypeParameterElement> {
        let ctx = self.ctx();
        let store = fresh_store(&ctx);
        let fragment = store.add_fragment::<TypeParameterFragment>(TypeParameterFragment {
            fragment: FragmentData::new(name, None),
        });
        let mut element = TypeParameterElement::new(ElementData::new(name, fragment.raw()));
        element.variance = variance;
        element.bound.set(bound);
        let id: EId<TypeParameterElement> = store.add(element);
        store.fragment(fragment).element.set_once(id.raw());
        id
    }

    /// `freshCopy()`: a new type parameter with the name and the bound of
    /// [param] (not the variance).
    fn fresh_copy(&self, param: EId<TypeParameterElement>) -> EId<TypeParameterElement> {
        let ctx = self.ctx();
        let data = ctx.get(param);
        self.new_type_parameter(data.name, None, data.bound.get())
    }

    /// [fresh_copy] that also copies the variance when it is declared (the
    /// pattern `if (!element.isLegacyCovariant) fresh.variance = ...`).
    fn fresh_copy_with_variance(
        &self,
        param: EId<TypeParameterElement>,
    ) -> EId<TypeParameterElement> {
        let ctx = self.ctx();
        let data = ctx.get(param);
        self.new_type_parameter(data.name, data.variance, data.bound.get())
    }

    // ------------------------------------------------------------ basics

    /// `nullabilitySuffix`.
    fn nullability_suffix(&self, t: TypeId) -> Nullability {
        match *self.ctx().ty(t) {
            TypeKind::Dynamic | TypeKind::Void | TypeKind::Invalid => Nullability::None,
            TypeKind::Unknown => Nullability::Question,
            TypeKind::Never(n) => n,
            TypeKind::Interface { nullability, .. } => nullability,
            TypeKind::Function(f) => f.nullability,
            TypeKind::Record { nullability, .. } => nullability,
            TypeKind::TypeParameter { nullability, .. } => nullability,
        }
    }

    /// `isQuestionType`.
    fn is_question_type(&self, t: TypeId) -> bool {
        self.nullability_suffix(t) != Nullability::None
    }

    /// `alias`.
    fn type_alias(&self, t: TypeId) -> Option<AliasId> {
        match *self.ctx().ty(t) {
            TypeKind::Interface { alias, .. }
            | TypeKind::Record { alias, .. }
            | TypeKind::TypeParameter { alias, .. } => alias,
            TypeKind::Function(f) => f.alias,
            _ => None,
        }
    }

    /// `InstantiatedTypeAliasElementImpl.withNullability`.
    fn alias_with_nullability(&self, alias: AliasId, n: Nullability) -> AliasId {
        let ctx = self.ctx();
        let a = *ctx.alias(alias);
        if a.nullability == n {
            return alias;
        }
        ctx.intern_alias(AliasRef {
            nullability: n,
            ..a
        })
    }

    /// `withNullability(n)`.
    fn with_nullability(&self, t: TypeId, n: Nullability) -> TypeId {
        let ctx = self.ctx();
        match *ctx.ty(t) {
            TypeKind::Dynamic | TypeKind::Void | TypeKind::Invalid | TypeKind::Unknown => t,
            TypeKind::Never(_) => match n {
                Nullability::None => TypeId::NEVER,
                Nullability::Question | Nullability::Star => TypeId::NEVER_QUESTION,
            },
            TypeKind::Interface {
                element,
                args,
                nullability,
                alias,
            } => {
                if nullability == n || self.is_dart_core_null_element(element) {
                    // NullTypeImpl.withNullability returns `this`.
                    return t;
                }
                ctx.intern(TypeKind::Interface {
                    element,
                    args,
                    nullability: n,
                    alias: alias.map(|a| self.alias_with_nullability(a, n)),
                })
            }
            TypeKind::Function(f) => {
                if f.nullability == n {
                    return t;
                }
                ctx.intern(TypeKind::Function(FunctionTypeData {
                    nullability: n,
                    alias: f.alias.map(|a| self.alias_with_nullability(a, n)),
                    ..f
                }))
            }
            TypeKind::Record {
                positional,
                named,
                nullability,
                alias,
            } => {
                if nullability == n {
                    return t;
                }
                ctx.intern(TypeKind::Record {
                    positional,
                    named,
                    nullability: n,
                    alias: alias.map(|a| self.alias_with_nullability(a, n)),
                })
            }
            TypeKind::TypeParameter {
                param,
                nullability,
                promoted_bound,
                alias,
            } => {
                if nullability == n {
                    return t;
                }
                ctx.intern(TypeKind::TypeParameter {
                    param,
                    nullability: n,
                    promoted_bound,
                    alias: alias.map(|a| self.alias_with_nullability(a, n)),
                })
            }
        }
    }

    /// `asQuestionType(isQuestionType)`.
    fn as_question_type(&self, t: TypeId, question: bool) -> TypeId {
        self.with_nullability(
            t,
            if question {
                Nullability::Question
            } else {
                Nullability::None
            },
        )
    }

    /// `withAlias(alias)`. Types without alias storage (`dynamic`, `void`,
    /// `Never`, `InvalidType`) are returned unchanged: the Rust type model
    /// has no alias for them (open point, see the crate documentation).
    fn with_alias(&self, t: TypeId, alias: AliasId) -> TypeId {
        let ctx = self.ctx();
        match *ctx.ty(t) {
            TypeKind::Interface {
                element,
                args,
                nullability,
                ..
            } => ctx.intern(TypeKind::Interface {
                element,
                args,
                nullability,
                alias: Some(alias),
            }),
            TypeKind::Function(f) => ctx.intern(TypeKind::Function(FunctionTypeData {
                alias: Some(alias),
                ..f
            })),
            TypeKind::Record {
                positional,
                named,
                nullability,
                ..
            } => ctx.intern(TypeKind::Record {
                positional,
                named,
                nullability,
                alias: Some(alias),
            }),
            TypeKind::TypeParameter {
                param,
                nullability,
                promoted_bound,
                ..
            } => ctx.intern(TypeKind::TypeParameter {
                param,
                nullability,
                promoted_bound,
                alias: Some(alias),
            }),
            _ => t,
        }
    }

    /// The same type without alias (for comparisons that ignore aliases).
    fn without_alias(&self, t: TypeId) -> TypeId {
        let ctx = self.ctx();
        match *ctx.ty(t) {
            TypeKind::Interface {
                element,
                args,
                nullability,
                alias: Some(_),
            } => ctx.intern(TypeKind::Interface {
                element,
                args,
                nullability,
                alias: None,
            }),
            TypeKind::Function(f) if f.alias.is_some() => {
                ctx.intern(TypeKind::Function(FunctionTypeData { alias: None, ..f }))
            }
            TypeKind::Record {
                positional,
                named,
                nullability,
                alias: Some(_),
            } => ctx.intern(TypeKind::Record {
                positional,
                named,
                nullability,
                alias: None,
            }),
            TypeKind::TypeParameter {
                param,
                nullability,
                promoted_bound,
                alias: Some(_),
            } => ctx.intern(TypeKind::TypeParameter {
                param,
                nullability,
                promoted_bound,
                alias: None,
            }),
            _ => t,
        }
    }

    // ------------------------------------------------------------ constructors

    /// The `InterfaceTypeImpl` factory: `Null` never has a suffix, and
    /// `FutureOr` without arguments gets `InvalidType`.
    fn interface_type(
        &self,
        element: EId<InterfaceElement>,
        args: &[TypeId],
        nullability: Nullability,
    ) -> TypeId {
        self.interface_type_with_alias(element, args, nullability, None)
    }

    /// The `InterfaceTypeImpl` factory with an alias.
    fn interface_type_with_alias(
        &self,
        element: EId<InterfaceElement>,
        args: &[TypeId],
        nullability: Nullability,
        alias: Option<AliasId>,
    ) -> TypeId {
        let ctx = self.ctx();
        if self.is_dart_core_null_element(element) {
            return ctx.intern(TypeKind::Interface {
                element,
                args: TypeList::EMPTY,
                nullability: Nullability::None,
                alias,
            });
        }
        let args = if args.is_empty() && self.is_dart_async_future_or_element(element) {
            ctx.intern_list(&[TypeId::INVALID])
        } else {
            ctx.intern_list(args)
        };
        ctx.intern(TypeKind::Interface {
            element,
            args,
            nullability,
            alias,
        })
    }

    /// `InterfaceElementImpl.instantiateImpl(typeArguments, nullabilitySuffix)`.
    fn instantiate_interface(
        &self,
        element: EId<InterfaceElement>,
        args: &[TypeId],
        nullability: Nullability,
    ) -> TypeId {
        debug_assert_eq!(args.len(), self.interface_type_parameters(element).len());
        self.interface_type(element, args, nullability)
    }

    /// `InterfaceElementImpl.thisType`: the element instantiated with its
    /// own type parameters.
    fn interface_this_type(&self, element: EId<InterfaceElement>) -> TypeId {
        let args: Vec<TypeId> = self
            .interface_type_parameters(element)
            .iter()
            .map(|&p| self.type_parameter_type(p, Nullability::None))
            .collect();
        self.interface_type(element, &args, Nullability::None)
    }

    /// `TypeParameterElementImpl.instantiate(nullabilitySuffix)`.
    fn type_parameter_type(&self, param: EId<TypeParameterElement>, n: Nullability) -> TypeId {
        self.ctx().intern(TypeKind::TypeParameter {
            param,
            nullability: n,
            promoted_bound: None,
            alias: None,
        })
    }

    /// `TypeParameterTypeImpl(element, nullabilitySuffix, promotedBound)`.
    fn promoted_type_parameter_type(
        &self,
        param: EId<TypeParameterElement>,
        n: Nullability,
        promoted_bound: Option<TypeId>,
    ) -> TypeId {
        self.ctx().intern(TypeKind::TypeParameter {
            param,
            nullability: n,
            promoted_bound,
            alias: None,
        })
    }

    /// `NeverTypeImpl.instance` / `instanceNullable`.
    fn never_type(&self, n: Nullability) -> TypeId {
        match n {
            Nullability::None => TypeId::NEVER,
            _ => TypeId::NEVER_QUESTION,
        }
    }

    /// The `FunctionTypeImpl` factory: positional parameters keep their
    /// order, named parameters are sorted by name.
    fn function_type(
        &self,
        type_params: &[EId<TypeParameterElement>],
        params: &[FnParam],
        ret: TypeId,
        nullability: Nullability,
        alias: Option<AliasId>,
    ) -> TypeId {
        let ctx = self.ctx();
        let mut sorted;
        let mut params = params;
        let first_named = params.iter().position(|p| is_named(p.kind));
        let mut required_positional = 0u16;
        for p in params {
            if is_required_positional(p.kind) {
                required_positional += 1;
            }
        }
        if let Some(first) = first_named {
            let name = |p: &FnParam| p.name.map(|n| ctx.name_str(n)).unwrap_or("");
            let already_sorted = params[first..]
                .windows(2)
                .all(|w| name(&w[0]) <= name(&w[1]));
            if !already_sorted {
                sorted = params.to_vec();
                // Dart `List.sort` is not stable, but parameter names are
                // unique in valid code; a stable sort keeps the input order
                // for equal names.
                sorted[first..].sort_by(|a, b| name(a).cmp(name(b)));
                params = &sorted;
            }
        }
        let type_params = ctx.intern_list(type_params);
        let params = ctx.intern_list(params);
        ctx.intern(TypeKind::Function(FunctionTypeData {
            type_params,
            params,
            required_positional,
            ret,
            nullability,
            alias,
        }))
    }

    /// The `RecordTypeImpl` constructor: named fields are sorted by name.
    fn record_type(
        &self,
        positional: &[TypeId],
        named: &[NamedType],
        nullability: Nullability,
        alias: Option<AliasId>,
    ) -> TypeId {
        let ctx = self.ctx();
        let mut named_sorted;
        let mut named = named;
        let name = |f: &NamedType| ctx.name_str(f.name);
        if !named.windows(2).all(|w| name(&w[0]) <= name(&w[1])) {
            named_sorted = named.to_vec();
            named_sorted.sort_by(|a, b| name(a).cmp(name(b)));
            named = &named_sorted;
        }
        let positional = ctx.intern_list(positional);
        let named = ctx.intern_list(named);
        ctx.intern(TypeKind::Record {
            positional,
            named,
            nullability,
            alias,
        })
    }

    /// `TypeAliasElementImpl.instantiateImpl(typeArguments, nullabilitySuffix)`.
    fn instantiate_type_alias(
        &self,
        element: EId<TypeAliasElement>,
        args: &[TypeId],
        nullability: Nullability,
    ) -> TypeId {
        let ctx = self.ctx();
        let data = ctx.get(element);
        let fragment: &TypeAliasFragment = ctx.fragment(data.first_fragment());
        if fragment.has_self_reference.get() {
            // `isNonFunctionTypeAliasesEnabled` is always true in 3.13.
            return TypeId::DYNAMIC;
        }
        let substitution = MapSubstitution::from_pairs(&data.type_params, args);
        let aliased = data.aliased_type.get().unwrap_or(TypeId::INVALID);
        let ty = substitution.substitute_type(&ctx, aliased);
        let result_nullability = if self.nullability_suffix(ty) == Nullability::Question {
            Nullability::Question
        } else {
            nullability
        };
        let args = ctx.intern_list(args);
        let alias = ctx.intern_alias(AliasRef {
            element,
            args,
            nullability,
        });
        let ty = self.with_nullability(ty, result_nullability);
        self.with_alias(ty, alias)
    }

    // ------------------------------------------------------------ predicates

    /// `element.name == 'Null' && element.library.isDartCore`.
    fn is_dart_core_null_element(&self, element: EId<InterfaceElement>) -> bool {
        self.is_element(element.raw(), "dart.core", "Null")
    }

    /// `element.name == 'FutureOr' && element.library.isDartAsync`.
    fn is_dart_async_future_or_element(&self, element: EId<InterfaceElement>) -> bool {
        self.is_element(element.raw(), "dart.async", "FutureOr")
    }

    /// The interface element of [t] when it is an interface type.
    fn interface_element(&self, t: TypeId) -> Option<EId<InterfaceElement>> {
        match *self.ctx().ty(t) {
            TypeKind::Interface { element, .. } => Some(element),
            _ => None,
        }
    }

    /// `DartType.element` for interface and type parameter types, and the
    /// `NeverElementImpl` for `Never` (`DynamicElementImpl` for `dynamic`).
    fn type_element(&self, t: TypeId) -> Option<ElementId> {
        match *self.ctx().ty(t) {
            TypeKind::Interface { element, .. } => Some(element.raw()),
            TypeKind::TypeParameter { param, .. } => Some(param.raw()),
            TypeKind::Never(_) => Some(ElementId::NEVER),
            TypeKind::Dynamic => Some(ElementId::DYNAMIC),
            _ => None,
        }
    }

    /// Whether [t] is an interface type of the class [name] in the library
    /// named [library_name].
    fn is_interface_of(&self, t: TypeId, library_name: &str, name: &str) -> bool {
        match self.interface_element(t) {
            Some(e) => self.is_element(e.raw(), library_name, name),
            None => false,
        }
    }

    fn is_dart_async_future(&self, t: TypeId) -> bool {
        self.is_interface_of(t, "dart.async", "Future")
    }
    fn is_dart_async_future_or(&self, t: TypeId) -> bool {
        self.is_interface_of(t, "dart.async", "FutureOr")
    }
    fn is_dart_async_stream(&self, t: TypeId) -> bool {
        self.is_interface_of(t, "dart.async", "Stream")
    }
    fn is_dart_core_bool(&self, t: TypeId) -> bool {
        self.is_interface_of(t, "dart.core", "bool")
    }
    fn is_dart_core_double(&self, t: TypeId) -> bool {
        self.is_interface_of(t, "dart.core", "double")
    }
    /// `isDartCoreEnum` (a class element named `Enum` in `dart:core`).
    fn is_dart_core_enum(&self, t: TypeId) -> bool {
        match self.interface_element(t) {
            Some(e) => {
                e.raw().is::<ClassElement>() && self.is_element(e.raw(), "dart.core", "Enum")
            }
            None => false,
        }
    }
    fn is_dart_core_function(&self, t: TypeId) -> bool {
        self.is_interface_of(t, "dart.core", "Function")
    }
    fn is_dart_core_int(&self, t: TypeId) -> bool {
        self.is_interface_of(t, "dart.core", "int")
    }
    fn is_dart_core_iterable(&self, t: TypeId) -> bool {
        self.is_interface_of(t, "dart.core", "Iterable")
    }
    fn is_dart_core_list(&self, t: TypeId) -> bool {
        self.is_interface_of(t, "dart.core", "List")
    }
    fn is_dart_core_map(&self, t: TypeId) -> bool {
        self.is_interface_of(t, "dart.core", "Map")
    }
    /// `isDartCoreNull`: the `Null` interface type, or `Never?`.
    fn is_dart_core_null(&self, t: TypeId) -> bool {
        match *self.ctx().ty(t) {
            TypeKind::Never(n) => n == Nullability::Question,
            TypeKind::Interface { element, .. } => self.is_dart_core_null_element(element),
            _ => false,
        }
    }
    fn is_dart_core_num(&self, t: TypeId) -> bool {
        self.is_interface_of(t, "dart.core", "num")
    }
    fn is_dart_core_object(&self, t: TypeId) -> bool {
        self.is_interface_of(t, "dart.core", "Object")
    }
    fn is_dart_core_record(&self, t: TypeId) -> bool {
        self.is_interface_of(t, "dart.core", "Record")
    }
    fn is_dart_core_set(&self, t: TypeId) -> bool {
        self.is_interface_of(t, "dart.core", "Set")
    }
    fn is_dart_core_string(&self, t: TypeId) -> bool {
        self.is_interface_of(t, "dart.core", "String")
    }
    fn is_dart_core_symbol(&self, t: TypeId) -> bool {
        self.is_interface_of(t, "dart.core", "Symbol")
    }
    fn is_dart_core_type(&self, t: TypeId) -> bool {
        self.is_interface_of(t, "dart.core", "Type")
    }

    /// Whether [t] is an interface type of an extension type element.
    fn is_extension_type(&self, t: TypeId) -> bool {
        self.interface_element(t)
            .is_some_and(|e| e.raw().is::<ExtensionTypeElement>())
    }

    /// `isBottom`.
    fn is_bottom(&self, t: TypeId) -> bool {
        let ctx = self.ctx();
        match *ctx.ty(t) {
            TypeKind::Never(n) => n != Nullability::Question,
            TypeKind::TypeParameter { .. } => {
                let mut seen: Vec<EId<TypeParameterElement>> = Vec::new();
                let mut current = t;
                loop {
                    let TypeKind::TypeParameter {
                        param, nullability, ..
                    } = *ctx.ty(current)
                    else {
                        unreachable!()
                    };
                    if seen.contains(&param) {
                        return false;
                    }
                    seen.push(param);
                    if nullability == Nullability::Question {
                        return false;
                    }
                    let bound = self.type_parameter_type_bound(current);
                    if matches!(ctx.ty(bound), TypeKind::TypeParameter { .. }) {
                        current = bound;
                    } else {
                        return self.is_bottom(bound);
                    }
                }
            }
            _ => false,
        }
    }

    // ------------------------------------------------------------ accessors

    /// `InterfaceType.typeArguments` (empty for other types).
    fn type_arguments(&self, t: TypeId) -> &'a [TypeId] {
        let ctx = self.ctx();
        match *ctx.ty(t) {
            TypeKind::Interface { args, .. } => ctx.list(args),
            _ => &[],
        }
    }

    /// `TypeParameterTypeImpl.bound`: the promoted bound, else the element
    /// bound, else `dynamic`.
    fn type_parameter_type_bound(&self, t: TypeId) -> TypeId {
        match *self.ctx().ty(t) {
            TypeKind::TypeParameter {
                param,
                promoted_bound,
                ..
            } => promoted_bound
                .or_else(|| self.type_parameter_bound(param))
                .unwrap_or(TypeId::DYNAMIC),
            _ => panic!("not a type parameter type"),
        }
    }

    /// `TypeParameterTypeImpl.withoutPromotedBound`.
    fn without_promoted_bound(&self, t: TypeId) -> TypeId {
        match *self.ctx().ty(t) {
            TypeKind::TypeParameter {
                param, nullability, ..
            } => self.type_parameter_type(param, nullability),
            _ => t,
        }
    }

    /// `InterfaceElementImpl.allSupertypes` (class_hierarchy.dart).
    fn element_all_supertypes(&self, element: EId<InterfaceElement>) -> &'a [TypeId] {
        class_hierarchy::implemented_interfaces(&self.ctx(), element)
    }

    /// `asInstanceOf(targetElement)`.
    fn as_instance_of(&self, t: TypeId, target: EId<InterfaceElement>) -> Option<TypeId> {
        let ctx = self.ctx();
        match *ctx.ty(t) {
            TypeKind::Interface { element, .. } => {
                if element == target {
                    return Some(t);
                }
                for &raw in self.element_all_supertypes(element) {
                    if self.interface_element(raw) == Some(target) {
                        let substitution = MapSubstitution::from_interface_type(&ctx, t);
                        return Some(substitution.substitute_type(&ctx, raw));
                    }
                }
                None
            }
            TypeKind::TypeParameter { .. } => {
                self.as_instance_of(self.type_parameter_type_bound(t), target)
            }
            _ => None,
        }
    }

    /// `InterfaceTypeImpl._instantiateSuperTypes`.
    fn instantiate_super_types(&self, t: TypeId, defined: &[TypeId]) -> Vec<TypeId> {
        let ctx = self.ctx();
        if defined.is_empty() {
            return Vec::new();
        }
        let element = self.interface_element(t).expect("interface type");
        let n = self.nullability_suffix(t);
        let substitution = if self.interface_type_parameters(element).is_empty() {
            None
        } else {
            Some(MapSubstitution::from_interface_type(&ctx, t))
        };
        defined
            .iter()
            .map(|&d| {
                let r = match &substitution {
                    Some(s) => s.substitute_type(&ctx, d),
                    None => d,
                };
                self.with_nullability(r, n)
            })
            .collect()
    }

    /// `InterfaceTypeImpl.interfaces`.
    fn interfaces(&self, t: TypeId) -> Vec<TypeId> {
        let element = self.interface_element(t).expect("interface type");
        self.instantiate_super_types(t, self.element_interfaces(element))
    }

    /// `InterfaceTypeImpl.mixins`.
    fn mixins(&self, t: TypeId) -> Vec<TypeId> {
        let element = self.interface_element(t).expect("interface type");
        self.instantiate_super_types(t, self.element_mixins(element))
    }

    /// `InterfaceTypeImpl.superclassConstraints`.
    fn superclass_constraints(&self, t: TypeId) -> Vec<TypeId> {
        let element = self.interface_element(t).expect("interface type");
        self.instantiate_super_types(t, self.element_superclass_constraints(element))
    }

    /// `InterfaceTypeImpl.superclass`.
    fn superclass(&self, t: TypeId) -> Option<TypeId> {
        let ctx = self.ctx();
        let element = self.interface_element(t).expect("interface type");
        let supertype = self.element_supertype(element)?;
        let s = MapSubstitution::from_interface_type(&ctx, t).substitute_type(&ctx, supertype);
        Some(self.with_nullability(s, self.nullability_suffix(t)))
    }

    /// `InterfaceTypeImpl.allSupertypes`.
    fn all_supertypes(&self, t: TypeId) -> Vec<TypeId> {
        let ctx = self.ctx();
        let element = self.interface_element(t).expect("interface type");
        let substitution = MapSubstitution::from_interface_type(&ctx, t);
        let n = self.nullability_suffix(t);
        self.element_all_supertypes(element)
            .iter()
            .map(|&i| self.with_nullability(substitution.substitute_type(&ctx, i), n))
            .collect()
    }

    /// `ExtensionTypeElementImpl.representation.type` (the type of the first
    /// field).
    fn extension_type_representation(&self, element: EId<ExtensionTypeElement>) -> TypeId {
        let ctx = self.ctx();
        let field = ctx.get(element).fields[0];
        dartr_element::type_inference::ensure_property_type(&ctx, field.upcast());
        ctx.get(field).type_.get().unwrap_or(TypeId::INVALID)
    }

    /// `InterfaceTypeImpl.representationType`.
    fn representation_type(&self, t: TypeId) -> Option<TypeId> {
        let ctx = self.ctx();
        let element = self.interface_element(t)?.cast::<ExtensionTypeElement>()?;
        let representation = self.extension_type_representation(element);
        Some(MapSubstitution::from_interface_type(&ctx, t).substitute_type(&ctx, representation))
    }

    /// `FunctionTypeImpl.instantiate(argumentTypes)`.
    fn instantiate_function_type(&self, t: TypeId, args: &[TypeId]) -> TypeId {
        let ctx = self.ctx();
        let TypeKind::Function(f) = *ctx.ty(t) else {
            panic!("not a function type");
        };
        let type_params = ctx.list(f.type_params);
        assert_eq!(
            args.len(),
            type_params.len(),
            "argumentTypes.length != typeFormals.length"
        );
        if args.is_empty() {
            return t;
        }
        let substitution = MapSubstitution::from_pairs(type_params, args);
        let params: Vec<FnParam> = ctx
            .list(f.params)
            .iter()
            .map(|p| FnParam {
                ty: substitution.substitute_type(&ctx, p.ty),
                ..*p
            })
            .collect();
        let ret = substitution.substitute_type(&ctx, f.ret);
        self.function_type(&[], &params, ret, f.nullability, None)
    }

    /// `referencesAny(parameters)`.
    fn references_any(&self, t: TypeId, parameters: &[EId<TypeParameterElement>]) -> bool {
        let ctx = self.ctx();
        match *ctx.ty(t) {
            TypeKind::Interface { args, .. } => ctx
                .list(args)
                .iter()
                .any(|&a| self.references_any(a, parameters)),
            TypeKind::TypeParameter { param, .. } => parameters.contains(&param),
            TypeKind::Function(f) => {
                for &tp in ctx.list(f.type_params) {
                    let data = ctx.get(tp);
                    if let Some(bound) = data.bound.get()
                        && self.references_any(bound, parameters)
                    {
                        return true;
                    }
                    if let Some(default) = data.default_type.get()
                        && self.references_any(default, parameters)
                    {
                        return true;
                    }
                }
                if ctx
                    .list(f.params)
                    .iter()
                    .any(|p| self.references_any(p.ty, parameters))
                {
                    return true;
                }
                self.references_any(f.ret, parameters)
            }
            _ => false,
        }
    }

    /// Dart `==` on types (see `equality.rs`).
    fn dart_eq(&self, a: TypeId, b: TypeId) -> bool {
        crate::equality::dart_eq(&self.ctx(), a, b)
    }
}

impl<'a> TypeExt<'a> for Ctx<'a> {
    #[inline(always)]
    fn ctx(&self) -> Ctx<'a> {
        *self
    }
}
