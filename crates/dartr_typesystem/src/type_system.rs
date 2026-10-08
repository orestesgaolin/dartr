// Dart source: pkg/analyzer/lib/src/dart/element/type_system.dart
// (TypeSystemImpl, ExtensionTypeErasure, RelatedTypeParameters2,
// _RemoveBoundsOfGenericFunctionTypeVisitor, _TypeVariableEliminator)

//! [`TypeSystem`]: `TypeSystemImpl`. One value per library analysis (it is
//! `Copy`: the [`Ctx`] and the options).
//!
//! Ported here: the predicates and operations of `TypeSystemImpl` (unit
//! A3). The algorithms of other Dart files are in their own modules and
//! `TypeSystem` delegates to them, as the Dart class does.
//!
//! Not ported yet (open points):
//! - `canBeSubtypeOf` (needs `ClassElementImpl.allSubtypes` and enum
//!   constants), `demoteType` (type_demotion.dart), `isWellBounded`
//!   (well_bounded.dart), `setupGenericTypeInference`,
//!   `inferFunctionTypeInstantiation`, `matchSupertypeConstraints` (unit A6).
//! - `getCallMethodType` uses a direct member search (the declared `call`
//!   method of the class, else of the first supertype in `allSupertypes`
//!   order that declares one) until the inheritance manager (unit A7) is
//!   ported.

use dartr_element::{
    ClassElement, Ctx, EId, EnumElement, ExtensionElement, ExtensionTypeElement, FragmentFlags,
    InterfaceElement, MethodElement, Nullability, TypeId, TypeKind, TypeParameterElement,
};
use indexmap::IndexMap;

use crate::replacement_visitor::{ReplacementVisitor, super_visit_interface_type};
use crate::subtype::SubtypeHelper;
use crate::type_algebra::{FnSubstitution, MapSubstitution, substitute_type, unite_nullabilities};
use crate::type_ext::TypeExt;

/// `RelatedTypeParameters2`: fresh type parameters that unify two lists of
/// type parameters.
#[derive(Clone, Debug, Default)]
pub struct RelatedTypeParameters {
    pub type_parameters: Vec<EId<TypeParameterElement>>,
    pub type_parameter_types: Vec<TypeId>,
}

/// `TypeSystemImpl`.
#[derive(Clone, Copy)]
pub struct TypeSystem<'a> {
    pub ctx: Ctx<'a>,
}

impl<'a> TypeSystem<'a> {
    pub fn new(ctx: Ctx<'a>) -> TypeSystem<'a> {
        TypeSystem { ctx }
    }

    /// `nullNone`.
    pub fn null_none(&self) -> TypeId {
        self.ctx
            .with_nullability(self.ctx.tp.null_type(), Nullability::None)
    }

    /// `objectNone`.
    pub fn object_none(&self) -> TypeId {
        self.ctx
            .with_nullability(self.ctx.tp.object_type(), Nullability::None)
    }

    /// `objectQuestion`.
    pub fn object_question(&self) -> TypeId {
        self.ctx
            .with_nullability(self.ctx.tp.object_type(), Nullability::Question)
    }

    /// Dart `==` on types (design §1.4).
    pub fn dart_eq(&self, a: TypeId, b: TypeId) -> bool {
        self.ctx.dart_eq(a, b)
    }

    /// `acceptsFunctionType(t)`.
    pub fn accepts_function_type(&self, t: TypeId) -> bool {
        let ctx = self.ctx;
        if ctx.is_dart_async_future_or(t) {
            return self.accepts_function_type(ctx.type_arguments(t)[0]);
        }
        matches!(ctx.ty(t), TypeKind::Function(_)) || ctx.is_dart_core_function(t)
    }

    /// `eliminateTypeVariables(type)`.
    pub fn eliminate_type_variables(&self, t: TypeId) -> TypeId {
        let top = self.object_question();
        let bottom = TypeId::NEVER;
        let eliminator =
            FnSubstitution(move |_, upper_bound: bool| Some(if upper_bound { bottom } else { top }));
        substitute_type(&self.ctx, &eliminator, t, false)
    }

    /// `extensionTypeErasure` (`ExtensionTypeErasure().perform(type)`).
    pub fn extension_type_erasure(&self, t: TypeId) -> TypeId {
        let mut v = ExtensionTypeErasure { ctx: self.ctx };
        v.visit(t).unwrap_or(t)
    }

    /// `factor(T, S)`.
    pub fn factor(&self, t: TypeId, s: TypeId) -> TypeId {
        let ctx = self.ctx;
        // * If T <: S then Never
        if self.is_subtype_of(t, s) {
            return TypeId::NEVER;
        }

        // * Else if T is R? and Null <: S then factor(R, S)
        // * Else if T is R? then factor(R, S)?
        if ctx.nullability_suffix(t) == Nullability::Question {
            let r = ctx.with_nullability(t, Nullability::None);
            let factor_rs = self.factor(r, s);
            if self.is_subtype_of(self.null_none(), s) {
                return factor_rs;
            } else {
                return ctx.with_nullability(factor_rs, Nullability::Question);
            }
        }

        // * Else if T is FutureOr<R> and Future<R> <: S then factor(R, S)
        // * Else if T is FutureOr<R> and R <: S then factor(Future<R>, S)
        if ctx.is_dart_async_future_or(t) {
            let r = ctx.type_arguments(t)[0];
            let future_r = ctx.tp.future_type(&ctx, r);
            if self.is_subtype_of(future_r, s) {
                return self.factor(r, s);
            }
            if self.is_subtype_of(r, s) {
                return self.factor(future_r, s);
            }
        }

        t
    }

    /// `flatten(T)`.
    pub fn flatten(&self, t: TypeId) -> TypeId {
        let ctx = self.ctx;
        // Dart: identical(T, UnknownInferredType.instance)
        if t == TypeId::UNKNOWN {
            return t;
        }

        // if T is S? then flatten(T) = flatten(S)?
        let nullability = ctx.nullability_suffix(t);
        if nullability != Nullability::None {
            let s = ctx.with_nullability(t, Nullability::None);
            return ctx.with_nullability(self.flatten(s), nullability);
        }

        // If T is X & S for some type variable X and type S then:
        if let TypeKind::TypeParameter {
            param,
            promoted_bound: Some(s),
            ..
        } = *ctx.ty(t)
        {
            // * if S has future type U then flatten(T) = flatten(U)
            if let Some(future_type) = self.future_type(s) {
                return self.flatten(future_type);
            }
            // * otherwise, flatten(T) = flatten(X)
            return self.flatten(ctx.type_parameter_type(param, nullability));
        }

        // If T has future type Future<S> or FutureOr<S> then flatten(T) = S
        // If T has future type Future<S>? or FutureOr<S>? then flatten(T) = S?
        if let Some(future_type) = self.future_type(t)
            && (ctx.is_dart_async_future(future_type) || ctx.is_dart_async_future_or(future_type))
        {
            let s = ctx.type_arguments(future_type)[0];
            if ctx.nullability_suffix(future_type) == Nullability::Question {
                return ctx.with_nullability(s, Nullability::Question);
            }
            return s;
        }

        // otherwise flatten(T) = T
        t
    }

    /// `futureOrBase(type)`.
    pub fn future_or_base(&self, t: TypeId) -> TypeId {
        let ctx = self.ctx;
        if ctx.is_dart_async_future_or(t) {
            return self.future_or_base(ctx.type_arguments(t)[0]);
        }
        t
    }

    /// `futureType(T)`: the future type of [t], if any.
    pub fn future_type(&self, t: TypeId) -> Option<TypeId> {
        let ctx = self.ctx;
        // T implements S, and there is a U such that S is Future<U>
        if ctx.nullability_suffix(t) != Nullability::Question {
            let future_element = ctx.tp.future_element().upcast::<InterfaceElement>();
            if let Some(result) = ctx.as_instance_of(t, future_element) {
                return Some(result);
            }
        }
        // T is S bounded, and there is a U such that S is FutureOr<U>,
        // Future<U>?, or FutureOr<U>?.
        self.future_type_of_bounded(t)
    }

    /// `_futureTypeOfBounded(T)`.
    fn future_type_of_bounded(&self, t: TypeId) -> Option<TypeId> {
        let ctx = self.ctx;
        if let TypeKind::Interface { .. } = ctx.ty(t) {
            if ctx.nullability_suffix(t) != Nullability::Question {
                if ctx.is_dart_async_future_or(t) {
                    return Some(t);
                }
            } else if ctx.is_dart_async_future_or(t) || ctx.is_dart_async_future(t) {
                return Some(t);
            }
        }
        if let TypeKind::TypeParameter {
            param,
            promoted_bound,
            ..
        } = *ctx.ty(t)
        {
            if let Some(bound) = ctx.type_parameter_bound(param)
                && let Some(result) = self.future_type_of_bounded(bound)
            {
                return Some(result);
            }
            if let Some(promoted_bound) = promoted_bound
                && let Some(result) = self.future_type_of_bounded(promoted_bound)
            {
                return Some(result);
            }
        }
        None
    }

    /// `futureValueType(T)`.
    pub fn future_value_type(&self, t: TypeId) -> TypeId {
        let ctx = self.ctx;
        // futureValueType(`S?`) = futureValueType(`S`), for all `S`.
        if ctx.nullability_suffix(t) != Nullability::None {
            let s = ctx.with_nullability(t, Nullability::None);
            return self.future_value_type(s);
        }
        // futureValueType(Future<`S`>) = `S`, for all `S`.
        // futureValueType(FutureOr<`S`>) = `S`, for all `S`.
        if ctx.is_dart_async_future(t) || ctx.is_dart_async_future_or(t) {
            return ctx.type_arguments(t)[0];
        }
        // futureValueType(`dynamic`) = `dynamic`.
        // futureValueType(`void`) = `void`.
        if matches!(ctx.ty(t), TypeKind::Dynamic | TypeKind::Void) {
            return t;
        }
        // Otherwise, for all `S`, futureValueType(`S`) = `Object?`.
        self.object_question()
    }

    /// `gatherMixinSupertypeConstraintsForInference(mixinElement)`.
    pub fn gather_mixin_supertype_constraints_for_inference(
        &self,
        mixin_element: EId<InterfaceElement>,
    ) -> Vec<TypeId> {
        let ctx = self.ctx;
        let candidates: Vec<TypeId> = if mixin_element.raw().is::<dartr_element::MixinElement>() {
            ctx.element_superclass_constraints(mixin_element).to_vec()
        } else {
            let Some(supertype) = ctx.element_supertype(mixin_element) else {
                return Vec::new();
            };
            let mut candidates = vec![supertype];
            candidates.extend_from_slice(ctx.element_mixins(mixin_element));
            if let Some(class) = mixin_element.cast::<ClassElement>() {
                let fragment = ctx.fragment(ctx.get(class).first_fragment());
                if fragment
                    .flags
                    .has(FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_APPLICATION)
                {
                    candidates.pop();
                }
            }
            candidates
        };
        candidates
            .into_iter()
            .filter(|&t| {
                let e = ctx.interface_element(t).unwrap();
                !ctx.interface_type_parameters(e).is_empty()
            })
            .collect()
    }

    /// `getCallMethodType(t)` (interim lookup, see the module
    /// documentation).
    pub fn get_call_method_type(&self, t: TypeId) -> Option<TypeId> {
        let ctx = self.ctx;
        let element = ctx.interface_element(t)?;
        let find = |e: EId<InterfaceElement>| -> Option<EId<MethodElement>> {
            ctx.interface(e)
                .methods
                .iter()
                .copied()
                .find(|&m| ctx.element_name(m.raw()) == Some("call"))
        };
        let (owner_type, method) = if let Some(m) = find(element) {
            (t, m)
        } else {
            let mut found = None;
            for s in ctx.all_supertypes(t) {
                if let Some(m) = find(ctx.interface_element(s).unwrap()) {
                    found = Some((s, m));
                    break;
                }
            }
            found?
        };
        let method_type = crate::element_type::executable_type(&ctx, method.upcast());
        Some(MapSubstitution::from_interface_type(&ctx, owner_type).substitute_type(&ctx, method_type))
    }

    /// `getFreeParameters(rootType, candidates:)`.
    pub fn get_free_parameters(
        &self,
        root_type: TypeId,
        candidates: Option<&[EId<TypeParameterElement>]>,
    ) -> Option<Vec<EId<TypeParameterElement>>> {
        struct State<'c> {
            parameters: Option<Vec<EId<TypeParameterElement>>>,
            visited: Vec<TypeId>,
            bound: Vec<EId<TypeParameterElement>>,
            candidates: Option<&'c [EId<TypeParameterElement>]>,
        }
        fn append(ctx: &Ctx<'_>, s: &mut State<'_>, t: TypeId) {
            // Dart: a HashSet<DartType> of visited types (Dart `==`).
            if s.visited.iter().any(|&v| ctx.dart_eq(v, t)) {
                return;
            }
            s.visited.push(t);
            match *ctx.ty(t) {
                TypeKind::TypeParameter { param, .. } => {
                    if s.candidates.is_none_or(|c| c.contains(&param)) && !s.bound.contains(&param) {
                        s.parameters.get_or_insert_with(Vec::new).push(param);
                    }
                }
                TypeKind::Function(f) => {
                    let formals = ctx.list(f.type_params);
                    s.bound.extend_from_slice(formals);
                    append(ctx, s, f.ret);
                    for p in ctx.list(f.params) {
                        append(ctx, s, p.ty);
                    }
                    if let Some(alias) = f.alias {
                        for &a in ctx.list(ctx.alias(alias).args) {
                            append(ctx, s, a);
                        }
                    }
                    s.bound.retain(|b| !formals.contains(b));
                }
                TypeKind::Interface { args, .. } => {
                    for &a in ctx.list(args) {
                        append(ctx, s, a);
                    }
                }
                TypeKind::Record {
                    positional, named, ..
                } => {
                    for &p in ctx.list(positional) {
                        append(ctx, s, p);
                    }
                    for n in ctx.list(named) {
                        append(ctx, s, n.ty);
                    }
                }
                _ => {}
            }
        }
        let mut state = State {
            parameters: None,
            visited: Vec::new(),
            bound: Vec::new(),
            candidates,
        };
        append(&self.ctx, &mut state, root_type);
        state.parameters
    }

    /// `greatestClosure(type, typeParameters)`.
    pub fn greatest_closure(&self, t: TypeId, type_parameters: &[EId<TypeParameterElement>]) -> TypeId {
        crate::least_greatest_closure::LeastGreatestClosureHelper::new(
            *self,
            self.object_question(),
            self.ctx.tp.function_type(),
            TypeId::NEVER,
            type_parameters.to_vec(),
        )
        .eliminate_to_greatest(t)
    }

    /// `greatestClosureOfSchema(schema)`.
    pub fn greatest_closure_of_schema(&self, schema: TypeId) -> TypeId {
        crate::type_schema_elimination::run(
            &self.ctx,
            self.object_question(),
            TypeId::NEVER,
            false,
            schema,
        )
    }

    /// `greatestLowerBound(T1, T2)`.
    pub fn greatest_lower_bound(&self, t1: TypeId, t2: TypeId) -> TypeId {
        crate::greatest_lower_bound::GreatestLowerBoundHelper::new(*self)
            .get_greatest_lower_bound(t1, t2)
    }

    /// `instantiateInterfaceToBounds(element, nullabilitySuffix)`.
    pub fn instantiate_interface_to_bounds(
        &self,
        element: EId<InterfaceElement>,
        nullability: Nullability,
    ) -> TypeId {
        let ctx = self.ctx;
        let args = self.default_type_arguments(ctx.interface_type_parameters(element));
        ctx.instantiate_interface(element, &args, nullability)
    }

    /// `instantiateType(type, typeArguments)`.
    pub fn instantiate_type(&self, t: TypeId, type_arguments: &[TypeId]) -> TypeId {
        let ctx = self.ctx;
        match *ctx.ty(t) {
            TypeKind::Function(_) => ctx.instantiate_function_type(t, type_arguments),
            TypeKind::Interface {
                element,
                nullability,
                ..
            } => ctx.instantiate_interface(element, type_arguments, nullability),
            _ => t,
        }
    }

    /// `instantiateTypeAliasToBounds(element, nullabilitySuffix)`.
    pub fn instantiate_type_alias_to_bounds(
        &self,
        element: EId<dartr_element::TypeAliasElement>,
        nullability: Nullability,
    ) -> TypeId {
        let ctx = self.ctx;
        let args = self.default_type_arguments(&ctx.get(element).type_params);
        ctx.instantiate_type_alias(element, &args, nullability)
    }

    /// `instantiateTypeFormalsToBounds(typeParameters, hasError:, knownTypes:)`.
    /// Returns the arguments and whether the bounds were malformed
    /// (`hasError[0]`).
    pub fn instantiate_type_formals_to_bounds(
        &self,
        type_parameters: &[EId<TypeParameterElement>],
        known_types: Option<IndexMap<EId<TypeParameterElement>, TypeId>>,
    ) -> (Vec<TypeId>, bool) {
        let ctx = self.ctx;
        if type_parameters.is_empty() {
            return (Vec::new(), false);
        }
        let all: Vec<EId<TypeParameterElement>> = type_parameters.to_vec();
        // all ground
        let mut defaults = known_types.unwrap_or_default();
        // not ground
        let mut partials: IndexMap<EId<TypeParameterElement>, TypeId> = IndexMap::new();
        for &p in type_parameters {
            if !defaults.contains_key(&p) {
                partials.insert(p, ctx.type_parameter_bound(p).unwrap_or(TypeId::DYNAMIC));
            }
        }

        let mut has_progress = true;
        while has_progress {
            has_progress = false;
            for (&parameter, &value) in &partials {
                let free = self.get_free_parameters(value, Some(&all));
                match free {
                    None => {
                        defaults.insert(parameter, value);
                        partials.shift_remove(&parameter);
                        has_progress = true;
                        break;
                    }
                    Some(free) if free.iter().all(|f| defaults.contains_key(f)) => {
                        let s = MapSubstitution::from_map(defaults.clone()).substitute_type(&ctx, value);
                        defaults.insert(parameter, s);
                        partials.shift_remove(&parameter);
                        has_progress = true;
                        break;
                    }
                    Some(_) => {}
                }
            }
        }

        let mut has_error = false;
        if !partials.is_empty() {
            has_error = true;
            let mut domain: Vec<EId<TypeParameterElement>> = defaults.keys().copied().collect();
            let mut range: Vec<TypeId> = defaults.values().copied().collect();
            for &parameter in partials.keys() {
                domain.push(parameter);
                range.push(TypeId::DYNAMIC);
            }
            for (&parameter, &value) in &partials {
                let s = MapSubstitution::from_pairs(&domain, &range).substitute_type(&ctx, value);
                defaults.insert(parameter, s);
            }
        }

        (
            type_parameters.iter().map(|p| defaults[p]).collect(),
            has_error,
        )
    }

    /// `isAlwaysExhaustive(type)`.
    pub fn is_always_exhaustive(&self, t: TypeId) -> bool {
        let ctx = self.ctx;
        match *ctx.ty(t) {
            TypeKind::Interface { element, .. } => {
                if ctx.is_dart_core_bool(t) || ctx.is_dart_core_null(t) {
                    return true;
                }
                if element.raw().is::<EnumElement>() {
                    return true;
                }
                if let Some(class) = element.cast::<ClassElement>() {
                    let fragment = ctx.fragment(ctx.get(class).first_fragment());
                    if fragment.flags.has(FragmentFlags::CLASS_FRAGMENT_IS_SEALED) {
                        return true;
                    }
                }
                if element.raw().is::<ExtensionTypeElement>() {
                    return self.is_always_exhaustive(self.extension_type_erasure(t));
                }
                if ctx.is_dart_async_future_or(t) {
                    return self.is_always_exhaustive(ctx.type_arguments(t)[0]);
                }
                false
            }
            TypeKind::TypeParameter {
                param,
                promoted_bound,
                ..
            } => {
                if let Some(promoted_bound) = promoted_bound
                    && self.is_always_exhaustive(promoted_bound)
                {
                    return true;
                }
                if let Some(bound) = ctx.type_parameter_bound(param)
                    && self.is_always_exhaustive(bound)
                {
                    return true;
                }
                false
            }
            TypeKind::Record {
                positional, named, ..
            } => {
                ctx.list(positional).iter().all(|&f| self.is_always_exhaustive(f))
                    && ctx.list(named).iter().all(|f| self.is_always_exhaustive(f.ty))
            }
            _ => false,
        }
    }

    /// `isAssignableTo(fromType, toType, strictCasts:)`.
    pub fn is_assignable_to(&self, from_type: TypeId, to_type: TypeId, strict_casts: bool) -> bool {
        let ctx = self.ctx;
        // An actual subtype
        if self.is_subtype_of(from_type, to_type) {
            return true;
        }
        // Accept the invalid type, we have already reported an error for it.
        if matches!(ctx.ty(from_type), TypeKind::Invalid) {
            return true;
        }
        // A 'call' method tearoff.
        if matches!(ctx.ty(from_type), TypeKind::Interface { .. })
            && !self.is_nullable(from_type)
            && self.accepts_function_type(to_type)
            && let Some(call_method_type) = self.get_call_method_type(from_type)
            && self.is_assignable_to(call_method_type, to_type, strict_casts)
        {
            return true;
        }
        // First make sure that the static analysis option, `strict-casts: true`
        // disables all downcasts, including casts from `dynamic`.
        if strict_casts {
            return false;
        }
        // Now handle NNBD default behavior, where we disable non-dynamic downcasts.
        matches!(ctx.ty(from_type), TypeKind::Dynamic)
    }

    /// `isDynamicBounded(type)`.
    pub fn is_dynamic_bounded(&self, t: TypeId) -> bool {
        let ctx = self.ctx;
        match *ctx.ty(t) {
            TypeKind::Dynamic => true,
            TypeKind::TypeParameter {
                param,
                promoted_bound,
                ..
            } => {
                ctx.type_parameter_bound(param)
                    .is_some_and(|b| self.is_dynamic_bounded(b))
                    || promoted_bound.is_some_and(|b| self.is_dynamic_bounded(b))
            }
            _ => false,
        }
    }

    /// `isEqualTo(left, right)`: mutual subtypes.
    pub fn is_equal_to(&self, left: TypeId, right: TypeId) -> bool {
        self.is_subtype_of(left, right) && self.is_subtype_of(right, left)
    }

    /// `isFunctionBounded(type)`.
    pub fn is_function_bounded(&self, t: TypeId) -> bool {
        let ctx = self.ctx;
        match *ctx.ty(t) {
            TypeKind::Function(f) => f.nullability != Nullability::Question,
            TypeKind::Interface { nullability, .. } if ctx.is_dart_core_function(t) => {
                nullability != Nullability::Question
            }
            TypeKind::TypeParameter {
                param,
                promoted_bound,
                ..
            } => {
                ctx.type_parameter_bound(param)
                    .is_some_and(|b| self.is_function_bounded(b))
                    || promoted_bound.is_some_and(|b| self.is_function_bounded(b))
            }
            _ => false,
        }
    }

    /// `isIncompatibleWithAwait(T)`.
    pub fn is_incompatible_with_await(&self, t: TypeId) -> bool {
        let ctx = self.ctx;
        // `T` is `S?`, and `S` is incompatible with await.
        if ctx.nullability_suffix(t) == Nullability::Question {
            let t_none = ctx.with_nullability(t, Nullability::None);
            return self.is_incompatible_with_await(t_none);
        }
        // `T` is an extension type that does not implement `Future`.
        if ctx.is_extension_type(t) {
            let any_future = ctx.tp.future_type(&ctx, self.object_question());
            if !self.is_subtype_of(t, any_future) {
                return true;
            }
        }
        if let TypeKind::TypeParameter {
            param,
            promoted_bound,
            ..
        } = *ctx.ty(t)
        {
            // `T` is `X & B`, and `B` is incompatible with await.
            if let Some(b) = promoted_bound {
                return self.is_incompatible_with_await(b);
            }
            // `T` is a type variable with bound `S`, and `S` is incompatible
            // with await.
            if let Some(s) = ctx.type_parameter_bound(param) {
                return self.is_incompatible_with_await(s);
            }
        }
        false
    }

    /// `isInvalidBounded(type)`.
    pub fn is_invalid_bounded(&self, t: TypeId) -> bool {
        let ctx = self.ctx;
        match *ctx.ty(t) {
            TypeKind::Invalid => true,
            TypeKind::TypeParameter {
                param,
                promoted_bound,
                ..
            } => {
                ctx.type_parameter_bound(param)
                    .is_some_and(|b| self.is_invalid_bounded(b))
                    || promoted_bound.is_some_and(|b| self.is_invalid_bounded(b))
            }
            _ => false,
        }
    }

    /// `isMoreBottom(T, S)`.
    pub fn is_more_bottom(&self, t: TypeId, s: TypeId) -> bool {
        let ctx = self.ctx;
        let t_nullability = ctx.nullability_suffix(t);
        let s_nullability = ctx.nullability_suffix(s);

        // MOREBOTTOM(Never, T) = true
        if t == TypeId::NEVER {
            return true;
        }
        // MOREBOTTOM(T, Never) = false
        if s == TypeId::NEVER {
            return false;
        }
        // MOREBOTTOM(Null, T) = true
        if t_nullability == Nullability::None && ctx.is_dart_core_null(t) {
            return true;
        }
        // MOREBOTTOM(T, Null) = false
        if s_nullability == Nullability::None && ctx.is_dart_core_null(s) {
            return false;
        }
        // MOREBOTTOM(T?, S?) = MOREBOTTOM(T, S)
        if t_nullability == Nullability::Question && s_nullability == Nullability::Question {
            let t2 = ctx.with_nullability(t, Nullability::None);
            let s2 = ctx.with_nullability(s, Nullability::None);
            return self.is_more_bottom(t2, s2);
        }
        // MOREBOTTOM(T, S?) = true
        if s_nullability == Nullability::Question {
            return true;
        }
        // MOREBOTTOM(T?, S) = false
        if t_nullability == Nullability::Question {
            return false;
        }
        // Type parameters.
        if let (
            TypeKind::TypeParameter {
                param: t_element,
                promoted_bound: t_promoted_bound,
                ..
            },
            TypeKind::TypeParameter {
                param: s_element,
                promoted_bound: s_promoted_bound,
                ..
            },
        ) = (*ctx.ty(t), *ctx.ty(s))
        {
            // MOREBOTTOM(X&T, Y&S) = MOREBOTTOM(T, S)
            if let (Some(tb), Some(sb)) = (t_promoted_bound, s_promoted_bound) {
                return self.is_more_bottom(tb, sb);
            }
            // MOREBOTTOM(X&T, S) = true
            if t_promoted_bound.is_some() {
                return true;
            }
            // MOREBOTTOM(T, Y&S) = false
            if s_promoted_bound.is_some() {
                return false;
            }
            // MOREBOTTOM(X extends T, Y extends S) = MOREBOTTOM(T, S)
            let t_bound = ctx.type_parameter_bound(t_element).unwrap();
            let s_bound = ctx.type_parameter_bound(s_element).unwrap();
            return self.is_more_bottom(t_bound, s_bound);
        }
        false
    }

    /// `isMoreTop(T, S)`.
    pub fn is_more_top(&self, t: TypeId, s: TypeId) -> bool {
        let ctx = self.ctx;
        let t_nullability = ctx.nullability_suffix(t);
        let s_nullability = ctx.nullability_suffix(s);

        // MORETOP(void, S) = true
        if matches!(ctx.ty(t), TypeKind::Void) {
            return true;
        }
        // MORETOP(T, void) = false
        if matches!(ctx.ty(s), TypeKind::Void) {
            return false;
        }
        // MORETOP(dynamic, S) = true
        if matches!(ctx.ty(t), TypeKind::Dynamic | TypeKind::Invalid) {
            return true;
        }
        // MORETOP(T, dynamic) = false
        if matches!(ctx.ty(s), TypeKind::Dynamic | TypeKind::Invalid) {
            return false;
        }
        // MORETOP(Object, S) = true
        if t_nullability == Nullability::None && ctx.is_dart_core_object(t) {
            return true;
        }
        // MORETOP(T, Object) = false
        if s_nullability == Nullability::None && ctx.is_dart_core_object(s) {
            return false;
        }
        // MORETOP(T?, S?) = MORETOP(T, S)
        if t_nullability == Nullability::Question && s_nullability == Nullability::Question {
            let t2 = ctx.with_nullability(t, Nullability::None);
            let s2 = ctx.with_nullability(s, Nullability::None);
            return self.is_more_top(t2, s2);
        }
        // MORETOP(T, S?) = true
        if s_nullability == Nullability::Question {
            return true;
        }
        // MORETOP(T?, S) = false
        if t_nullability == Nullability::Question {
            return false;
        }
        // MORETOP(FutureOr<T>, FutureOr<S>) = MORETOP(T, S)
        if ctx.is_dart_async_future_or(t) && ctx.is_dart_async_future_or(s) {
            let t2 = ctx.type_arguments(t)[0];
            let s2 = ctx.type_arguments(s)[0];
            return self.is_more_top(t2, s2);
        }
        false
    }

    /// `isNonNullable(type)`.
    pub fn is_non_nullable(&self, t: TypeId) -> bool {
        let ctx = self.ctx;
        match *ctx.ty(t) {
            TypeKind::Dynamic | TypeKind::Invalid | TypeKind::Unknown | TypeKind::Void => false,
            _ if ctx.is_dart_core_null(t) => false,
            TypeKind::TypeParameter {
                promoted_bound: Some(b),
                ..
            } => self.is_non_nullable(b),
            _ if ctx.nullability_suffix(t) == Nullability::Question => false,
            TypeKind::Interface { element, .. } => {
                if ctx.is_dart_async_future_or(t) {
                    return self.is_non_nullable(ctx.type_arguments(t)[0]);
                }
                if element.raw().is::<ExtensionTypeElement>() {
                    return !ctx.interfaces(t).is_empty();
                }
                true
            }
            TypeKind::TypeParameter { param, .. } => ctx
                .type_parameter_bound(param)
                .is_some_and(|b| self.is_non_nullable(b)),
            _ => true,
        }
    }

    /// `isNull(type)`: in the equivalence class of `Null`.
    pub fn is_null(&self, t: TypeId) -> bool {
        let ctx = self.ctx;
        // NULL(Null) is true
        if ctx.is_dart_core_null(t) {
            return true;
        }
        // NULL(T?) is true iff NULL(T) or BOTTOM(T)
        if ctx.nullability_suffix(t) == Nullability::Question {
            let t = ctx.with_nullability(t, Nullability::None);
            return ctx.is_bottom(t);
        }
        // NULL(T) is false otherwise
        false
    }

    /// `isNullable(type)`.
    pub fn is_nullable(&self, t: TypeId) -> bool {
        let ctx = self.ctx;
        match *ctx.ty(t) {
            TypeKind::Dynamic | TypeKind::Invalid | TypeKind::Unknown | TypeKind::Void => true,
            _ if ctx.is_dart_core_null(t) => true,
            TypeKind::TypeParameter {
                promoted_bound: Some(b),
                ..
            } => self.is_nullable(b),
            _ if ctx.nullability_suffix(t) == Nullability::Question => true,
            TypeKind::Interface { .. } if ctx.is_dart_async_future_or(t) => {
                self.is_nullable(ctx.type_arguments(t)[0])
            }
            _ => false,
        }
    }

    /// `isObject(type)`: in the equivalence class of `Object`.
    pub fn is_object(&self, t: TypeId) -> bool {
        let ctx = self.ctx;
        if ctx.nullability_suffix(t) != Nullability::None {
            return false;
        }
        // OBJECT(Object) is true
        if ctx.is_dart_core_object(t) {
            return true;
        }
        // OBJECT(FutureOr<T>) is OBJECT(T)
        if ctx.is_dart_async_future_or(t) {
            return self.is_object(ctx.type_arguments(t)[0]);
        }
        false
    }

    /// `isPotentiallyNonNullable(type)`.
    pub fn is_potentially_non_nullable(&self, t: TypeId) -> bool {
        !self.is_nullable(t)
    }

    /// `isPotentiallyNullable(type)`.
    pub fn is_potentially_nullable(&self, t: TypeId) -> bool {
        !self.is_non_nullable(t)
    }

    /// `isStrictlyNonNullable(type)`.
    pub fn is_strictly_non_nullable(&self, t: TypeId) -> bool {
        let ctx = self.ctx;
        match *ctx.ty(t) {
            TypeKind::Dynamic | TypeKind::Invalid | TypeKind::Unknown | TypeKind::Void => false,
            _ if ctx.is_dart_core_null(t) => false,
            _ if ctx.nullability_suffix(t) != Nullability::None => false,
            TypeKind::Interface { element, .. } => {
                if ctx.is_dart_async_future_or(t) {
                    return self.is_strictly_non_nullable(ctx.type_arguments(t)[0]);
                }
                if element.raw().is::<ExtensionTypeElement>() {
                    return !ctx.interfaces(t).is_empty();
                }
                true
            }
            TypeKind::TypeParameter { .. } => {
                self.is_strictly_non_nullable(ctx.type_parameter_type_bound(t))
            }
            _ => true,
        }
    }

    /// `isSubtypeOf(leftType, rightType)`.
    pub fn is_subtype_of(&self, left: TypeId, right: TypeId) -> bool {
        SubtypeHelper::new(*self).is_subtype_of(left, right)
    }

    /// `isTop(type)`: in the equivalence class of top types.
    pub fn is_top(&self, t: TypeId) -> bool {
        let ctx = self.ctx;
        match *ctx.ty(t) {
            // TOP(?) is true
            TypeKind::Unknown => true,
            // TOP(dynamic) is true
            TypeKind::Dynamic | TypeKind::Invalid => true,
            // TOP(void) is true
            TypeKind::Void => true,
            _ => {
                let nullability = ctx.nullability_suffix(t);
                // TOP(T?) is true iff TOP(T) or OBJECT(T)
                if nullability == Nullability::Question {
                    let t = ctx.with_nullability(t, Nullability::None);
                    return self.is_top(t) || self.is_object(t);
                }
                // TOP(FutureOr<T>) is TOP(T)
                if ctx.is_dart_async_future_or(t) {
                    return self.is_top(ctx.type_arguments(t)[0]);
                }
                false
            }
        }
    }

    /// `isValidExtensionTypeSuperinterface(type)`.
    pub fn is_valid_extension_type_superinterface(&self, t: TypeId) -> bool {
        let ctx = self.ctx;
        if !matches!(ctx.ty(t), TypeKind::Interface { .. }) {
            return false;
        }
        if ctx.nullability_suffix(t) == Nullability::Question {
            return false;
        }
        !(ctx.is_dart_async_future_or(t)
            || ctx.is_dart_core_function(t)
            || ctx.is_dart_core_null(t)
            || ctx.is_dart_core_record(t))
    }

    /// `leastClosure(type, typeParameters)`.
    pub fn least_closure(&self, t: TypeId, type_parameters: &[EId<TypeParameterElement>]) -> TypeId {
        crate::least_greatest_closure::LeastGreatestClosureHelper::new(
            *self,
            self.object_question(),
            self.ctx.tp.function_type(),
            TypeId::NEVER,
            type_parameters.to_vec(),
        )
        .eliminate_to_least(t)
    }

    /// `leastClosureOfSchema(schema)`.
    pub fn least_closure_of_schema(&self, schema: TypeId) -> TypeId {
        crate::type_schema_elimination::run(
            &self.ctx,
            self.object_question(),
            TypeId::NEVER,
            true,
            schema,
        )
    }

    /// `leastUpperBound(T1, T2)`.
    pub fn least_upper_bound(&self, t1: TypeId, t2: TypeId) -> TypeId {
        crate::least_upper_bound::LeastUpperBoundHelper::new(*self).get_least_upper_bound(t1, t2)
    }

    /// `makeNullable(type)`.
    pub fn make_nullable(&self, t: TypeId) -> TypeId {
        self.ctx.with_nullability(t, Nullability::Question)
    }

    /// `normalize(T)`.
    pub fn normalize(&self, t: TypeId) -> TypeId {
        crate::normalize::NormalizeHelper::new(*self).normalize(t)
    }

    /// `normalizeFunctionType(T)`.
    pub fn normalize_function_type(&self, t: TypeId) -> TypeId {
        crate::normalize::NormalizeHelper::new(*self).normalize_function_type(t)
    }

    /// `normalizeInterfaceType(T)`.
    pub fn normalize_interface_type(&self, t: TypeId) -> TypeId {
        crate::normalize::NormalizeHelper::new(*self).normalize_interface_type(t)
    }

    /// `promoteToNonNull(type)`: `NonNull(type)`.
    pub fn promote_to_non_null(&self, t: TypeId) -> TypeId {
        let ctx = self.ctx;
        if ctx.is_dart_core_null(t) {
            return TypeId::NEVER;
        }
        if let TypeKind::TypeParameter {
            param,
            promoted_bound,
            ..
        } = *ctx.ty(t)
        {
            // NonNull(X & T) = X & NonNull(T)
            if let Some(promoted_bound) = promoted_bound {
                let promoted_bound = self.promote_to_non_null(promoted_bound);
                return ctx.promoted_type_parameter_type(param, Nullability::None, Some(promoted_bound));
            }
            // NonNull(X) = X & NonNull(B), where B is the bound of X
            let bound = ctx.type_parameter_bound(param);
            let mut promoted_bound = Some(match bound {
                Some(b) => self.promote_to_non_null(b),
                None => ctx.tp.object_type(),
            });
            // Dart: identical(promotedBound, element.bound). For a type
            // parameter bound Dart always creates a new object, so it is
            // never identical; otherwise `withNullability(none)` returns
            // `this` exactly when the TypeId is unchanged.
            if let Some(b) = bound
                && promoted_bound == Some(b)
                && !matches!(ctx.ty(b), TypeKind::TypeParameter { .. })
            {
                promoted_bound = None;
            }
            return ctx.promoted_type_parameter_type(param, Nullability::None, promoted_bound);
        }
        ctx.with_nullability(t, Nullability::None)
    }

    /// `refineBinaryExpressionType(leftType, operator, rightType,
    /// currentType, operatorElement)`; [operator_element] is the resolved
    /// operator method.
    pub fn refine_binary_expression_type(
        &self,
        left_type: TypeId,
        right_type: TypeId,
        current_type: TypeId,
        operator_element: Option<EId<MethodElement>>,
    ) -> TypeId {
        match operator_element {
            None => current_type,
            Some(m) => {
                self.refine_numeric_invocation_type_null_safe(left_type, m, &[right_type], current_type)
            }
        }
    }

    /// `refineNumericInvocationContext(targetType, methodElement,
    /// invocationContext, currentType)`.
    pub fn refine_numeric_invocation_context(
        &self,
        target_type: Option<TypeId>,
        method_element: Option<EId<MethodElement>>,
        invocation_context: TypeId,
        current_type: TypeId,
    ) -> TypeId {
        match (target_type, method_element) {
            (Some(t), Some(m)) => {
                self.refine_numeric_invocation_context_null_safe(t, m, invocation_context, current_type)
            }
            _ => current_type,
        }
    }

    /// `refineNumericInvocationType(targetType, methodElement,
    /// argumentTypes, currentType)`.
    pub fn refine_numeric_invocation_type(
        &self,
        target_type: TypeId,
        method_element: Option<EId<MethodElement>>,
        argument_types: &[TypeId],
        current_type: TypeId,
    ) -> TypeId {
        match method_element {
            Some(m) => self.refine_numeric_invocation_type_null_safe(
                target_type,
                m,
                argument_types,
                current_type,
            ),
            None => current_type,
        }
    }

    /// `relateTypeParameters(typeParameters1, typeParameters2)`.
    pub fn relate_type_parameters(
        &self,
        type_parameters1: &[EId<TypeParameterElement>],
        type_parameters2: &[EId<TypeParameterElement>],
    ) -> Option<RelatedTypeParameters> {
        let ctx = self.ctx;
        if type_parameters1.len() != type_parameters2.len() {
            return None;
        }
        if type_parameters1.is_empty() {
            return Some(RelatedTypeParameters::default());
        }
        let fresh: Vec<EId<TypeParameterElement>> =
            type_parameters1.iter().map(|&p| ctx.fresh_copy(p)).collect();
        let fresh_types: Vec<TypeId> = fresh
            .iter()
            .map(|&p| ctx.type_parameter_type(p, Nullability::None))
            .collect();
        let substitution1 = MapSubstitution::from_pairs(type_parameters1, &fresh_types);
        let substitution2 = MapSubstitution::from_pairs(type_parameters2, &fresh_types);
        for i in 0..type_parameters1.len() {
            let bound1 = ctx.type_parameter_bound(type_parameters1[i]);
            let bound2 = ctx.type_parameter_bound(type_parameters2[i]);
            if bound1.is_none() && bound2.is_none() {
                continue;
            }
            let bound1 = substitution1.substitute_type(&ctx, bound1.unwrap_or(TypeId::DYNAMIC));
            let bound2 = substitution2.substitute_type(&ctx, bound2.unwrap_or(TypeId::DYNAMIC));
            if !self.is_equal_to(bound1, bound2) {
                return None;
            }
            if !matches!(ctx.ty(bound1), TypeKind::Dynamic) {
                ctx.get(fresh[i]).bound.set(Some(bound1));
            }
        }
        Some(RelatedTypeParameters {
            type_parameters: fresh,
            type_parameter_types: fresh_types,
        })
    }

    /// `replaceTopAndBottom(dartType)`.
    pub fn replace_top_and_bottom(&self, t: TypeId) -> TypeId {
        crate::replace_top_bottom_visitor::run(*self, self.object_question(), TypeId::NEVER, t)
    }

    /// `resolveToBound(type)`.
    pub fn resolve_to_bound(&self, t: TypeId) -> TypeId {
        let ctx = self.ctx;
        if let TypeKind::TypeParameter {
            param,
            nullability,
            promoted_bound,
            ..
        } = *ctx.ty(t)
        {
            if let Some(promoted_bound) = promoted_bound {
                return self.resolve_to_bound(promoted_bound);
            }
            let Some(bound) = ctx.type_parameter_bound(param) else {
                return self.object_question();
            };
            let resolved = self.resolve_to_bound(bound);
            let n = unite_nullabilities(
                unite_nullabilities(nullability, ctx.nullability_suffix(bound)),
                ctx.nullability_suffix(resolved),
            );
            return ctx.with_nullability(resolved, n);
        }
        t
    }

    /// `runtimeTypesEqual(T1, T2)`.
    pub fn runtime_types_equal(&self, t1: TypeId, t2: TypeId) -> bool {
        crate::runtime_type_equality::RuntimeTypeEqualityHelper::new(*self).equal(t1, t2)
    }

    /// `topMerge(T, S)`. Panics where Dart throws (the types cannot be
    /// merged); use [`TypeSystem::try_top_merge`] to get `None` instead.
    pub fn top_merge(&self, t: TypeId, s: TypeId) -> TypeId {
        self.try_top_merge(t, s)
            .unwrap_or_else(|| panic!("topMerge: the types cannot be merged"))
    }

    /// `topMerge(T, S)`; `None` where Dart throws.
    pub fn try_top_merge(&self, t: TypeId, s: TypeId) -> Option<TypeId> {
        crate::top_merge::TopMergeHelper::new(*self).top_merge(t, s)
    }

    /// `tryPromoteToType(to, from)`.
    pub fn try_promote_to_type(&self, to: TypeId, from: TypeId) -> Option<TypeId> {
        let ctx = self.ctx;
        // Allow promoting to a subtype.
        if self.is_subtype_of(to, from) {
            return Some(to);
        }
        // For a type parameter `T extends U`, allow promoting the upper bound
        // `U` to `S` where `S <: U`, yielding a type parameter `T extends S`.
        if let TypeKind::TypeParameter {
            param, nullability, ..
        } = *ctx.ty(from)
            && self.is_subtype_of(to, ctx.type_parameter_type_bound(from))
        {
            return Some(ctx.promoted_type_parameter_type(
                param,
                promoted_type_parameter_type_nullability(nullability, ctx.nullability_suffix(to)),
                Some(to),
            ));
        }
        None
    }

    /// `unionFreeType(type)`.
    pub fn union_free_type(&self, t: TypeId) -> TypeId {
        let ctx = self.ctx;
        if t == TypeId::UNKNOWN {
            return t;
        }
        if ctx.nullability_suffix(t) != Nullability::None {
            return self.union_free_type(ctx.with_nullability(t, Nullability::None));
        }
        if ctx.is_dart_async_future_or(t) {
            return self.union_free_type(ctx.type_arguments(t)[0]);
        }
        t
    }

    /// `_defaultTypeArguments(typeParameters)`.
    fn default_type_arguments(&self, type_parameters: &[EId<TypeParameterElement>]) -> Vec<TypeId> {
        type_parameters
            .iter()
            .map(|&p| {
                self.ctx
                    .get(p)
                    .default_type
                    .get()
                    .expect("defaultType is set")
            })
            .collect()
    }

    /// Whether the method is declared in an extension or extension type.
    fn is_extension_member(&self, method: EId<MethodElement>) -> bool {
        let enclosing = self.ctx.get(method).enclosing;
        enclosing.is_some_and(|e| e.is::<ExtensionElement>() || e.is::<ExtensionTypeElement>())
    }

    /// `_refineNumericInvocationContextNullSafe`.
    fn refine_numeric_invocation_context_null_safe(
        &self,
        t: TypeId,
        method_element: EId<MethodElement>,
        c: TypeId,
        current_type: TypeId,
    ) -> TypeId {
        let ctx = self.ctx;
        let tp = ctx.tp;
        if self.is_extension_member(method_element) {
            return current_type;
        }
        let name = ctx.element_name(method_element.raw()).unwrap_or("");
        if matches!(name, "+" | "-" | "*" | "%" | "remainder") {
            let num_type = tp.num_type();
            if self.is_subtype_of(t, tp.num_type_question()) {
                let int_type = tp.int_type();
                if self.is_subtype_of(int_type, c)
                    && !self.is_subtype_of(num_type, c)
                    && self.is_subtype_of(t, tp.int_type_question())
                {
                    return int_type;
                }
                let double_type = tp.double_type();
                if self.is_subtype_of(double_type, c)
                    && !self.is_subtype_of(num_type, c)
                    && !self.is_subtype_of(t, tp.double_type_question())
                {
                    return double_type;
                }
                return num_type;
            }
        }
        if name == "clamp" {
            let num_type = tp.num_type();
            if self.is_subtype_of(t, tp.num_type_question()) {
                let int_type = tp.int_type();
                if self.is_subtype_of(int_type, c)
                    && !self.is_subtype_of(num_type, c)
                    && self.is_subtype_of(t, tp.int_type_question())
                {
                    return int_type;
                }
                let double_type = tp.double_type();
                if self.is_subtype_of(double_type, c)
                    && !self.is_subtype_of(num_type, c)
                    && self.is_subtype_of(t, tp.double_type_question())
                {
                    return double_type;
                }
                return num_type;
            }
        }
        current_type
    }

    /// `_refineNumericInvocationTypeNullSafe`.
    fn refine_numeric_invocation_type_null_safe(
        &self,
        t: TypeId,
        method_element: EId<MethodElement>,
        argument_types: &[TypeId],
        current_type: TypeId,
    ) -> TypeId {
        let ctx = self.ctx;
        let tp = ctx.tp;
        if self.is_extension_member(method_element) {
            return current_type;
        }
        let name = ctx.element_name(method_element.raw()).unwrap_or("");
        if matches!(name, "+" | "-" | "*" | "%" | "remainder")
            && self.is_subtype_of(t, tp.num_type_question())
            && argument_types.len() == 1
        {
            let s = argument_types[0];
            let double_type = tp.double_type();
            let double_type_question = tp.double_type_question();
            if self.is_subtype_of(t, double_type_question) {
                return double_type;
            }
            if !ctx.is_bottom(s) && self.is_subtype_of(s, double_type_question) {
                return double_type;
            }
            let int_type_question = tp.int_type_question();
            if !ctx.is_bottom(s)
                && self.is_subtype_of(t, int_type_question)
                && self.is_subtype_of(s, int_type_question)
            {
                return tp.int_type();
            }
            return tp.num_type();
        }
        if name == "clamp" && argument_types.len() == 2 {
            let t2 = argument_types[0];
            let t3 = argument_types[1];
            let num_type_question = tp.num_type_question();
            if self.is_subtype_of(t, num_type_question) && !ctx.is_bottom(t2) && !ctx.is_bottom(t3) {
                let int_type_question = tp.int_type_question();
                if self.is_subtype_of(t, int_type_question)
                    && self.is_subtype_of(t2, int_type_question)
                    && self.is_subtype_of(t3, int_type_question)
                {
                    return tp.int_type();
                }
                let double_type_question = tp.double_type_question();
                if self.is_subtype_of(t, double_type_question)
                    && self.is_subtype_of(t2, double_type_question)
                    && self.is_subtype_of(t3, double_type_question)
                {
                    return tp.double_type();
                }
                return tp.num_type();
            }
        }
        current_type
    }

    /// `_removeBoundsOfGenericFunctionTypes(type)`.
    pub fn remove_bounds_of_generic_function_types(&self, t: TypeId) -> TypeId {
        let mut v = RemoveBoundsOfGenericFunctionTypeVisitor {
            ctx: self.ctx,
            bottom_type: TypeId::NEVER,
        };
        v.visit(t).unwrap_or(t)
    }
}

/// `_promotedTypeParameterTypeNullability`.
fn promoted_type_parameter_type_nullability(
    nullability_of_type: Nullability,
    nullability_of_bound: Nullability,
) -> Nullability {
    if nullability_of_type == Nullability::Question && nullability_of_bound == Nullability::None {
        return Nullability::None;
    }
    if nullability_of_type == Nullability::Question && nullability_of_bound == Nullability::Question {
        return Nullability::Question;
    }
    // Intersection with a non-nullable type always yields a non-nullable type.
    Nullability::None
}

/// `ExtensionTypeErasure`.
pub struct ExtensionTypeErasure<'a> {
    pub ctx: Ctx<'a>,
}

impl<'a> ReplacementVisitor<'a> for ExtensionTypeErasure<'a> {
    fn ctx(&self) -> Ctx<'a> {
        self.ctx
    }

    fn visit_interface_type(&mut self, t: TypeId) -> Option<TypeId> {
        let ctx = self.ctx;
        if let Some(representation_type) = ctx.representation_type(t) {
            let erased = self.visit(representation_type).unwrap_or(representation_type);
            // If the extension type is nullable, apply it to the erased.
            if ctx.nullability_suffix(t) == Nullability::Question {
                return Some(ctx.with_nullability(erased, Nullability::Question));
            }
            // Use the erased as is, still might be nullable.
            return Some(erased);
        }
        super_visit_interface_type(self, t)
    }
}

/// `_RemoveBoundsOfGenericFunctionTypeVisitor`.
struct RemoveBoundsOfGenericFunctionTypeVisitor<'a> {
    ctx: Ctx<'a>,
    bottom_type: TypeId,
}

impl<'a> ReplacementVisitor<'a> for RemoveBoundsOfGenericFunctionTypeVisitor<'a> {
    fn ctx(&self) -> Ctx<'a> {
        self.ctx
    }

    fn visit_type_parameter_bound(&mut self, _t: TypeId) -> Option<TypeId> {
        Some(self.bottom_type)
    }
}
