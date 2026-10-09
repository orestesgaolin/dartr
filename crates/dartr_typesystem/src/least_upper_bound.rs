// Dart source: pkg/analyzer/lib/src/dart/element/least_upper_bound.dart

//! `LeastUpperBoundHelper` (`UP(T1, T2)` of the spec) and
//! `InterfaceLeastUpperBoundHelper` (the classic LUB algorithm for two
//! interface types).
//!
//! Dart `Set<InterfaceType>` (a `LinkedHashSet` with Dart `==`) is
//! [`InterfaceTypeSet`]: the iteration order of the superinterface sets
//! decides which type `_computeTypeAtMaxUniqueDepth` returns, so the set
//! keeps the insertion order, and it compares types with Dart `==`
//! ([`TypeExt::dart_eq`]), not with `TypeId` identity.

use std::cmp::Ordering;

use dartr_element::{
    ClassElement, Ctx, EId, ExtensionTypeElement, FnParam, FragmentFlags, InterfaceElement,
    MixinElement, NamedType, Nullability, ParameterKind, TypeId, TypeKind, TypeParameterElement,
    Variance,
};
use indexmap::IndexMap;

use crate::type_ext::{
    TypeExt, is_named, is_optional_positional, is_required, is_required_named,
    is_required_positional,
};
use crate::type_system::TypeSystem;

/// A Dart `Set<InterfaceType>` literal (`<InterfaceTypeImpl>{}`): a
/// `LinkedHashSet` that keeps the insertion order and compares the types
/// with Dart `==` (`InterfaceTypeImpl.==`: element, type arguments and
/// nullability; the alias is ignored).
///
/// The types are grouped by element (the Dart `hashCode` of an interface
/// type depends on the element), so `add` and `contains` compare only the
/// types of the same element.
#[derive(Clone, Debug, Default)]
pub struct InterfaceTypeSet {
    items: Vec<TypeId>,
    by_element: IndexMap<EId<InterfaceElement>, Vec<usize>>,
}

impl InterfaceTypeSet {
    pub fn new() -> InterfaceTypeSet {
        InterfaceTypeSet::default()
    }

    /// `set.contains(type)`.
    pub fn contains(&self, ctx: &Ctx<'_>, t: TypeId) -> bool {
        let element = ctx.interface_element(t).expect("interface type");
        match self.by_element.get(&element) {
            // Dart: ==
            Some(indexes) => indexes.iter().any(|&i| ctx.dart_eq(self.items[i], t)),
            None => false,
        }
    }

    /// `set.add(type)`: false when an equal type is already in the set (the
    /// set then keeps the first one).
    pub fn add(&mut self, ctx: &Ctx<'_>, t: TypeId) -> bool {
        if self.contains(ctx, t) {
            return false;
        }
        let element = ctx.interface_element(t).expect("interface type");
        self.by_element
            .entry(element)
            .or_default()
            .push(self.items.len());
        self.items.push(t);
        true
    }

    /// The types in insertion order.
    pub fn items(&self) -> &[TypeId] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// `set.intersection(other).toList()`: the types of this set (in the
    /// order of this set) that [other] contains.
    pub fn intersection(&self, ctx: &Ctx<'_>, other: &InterfaceTypeSet) -> Vec<TypeId> {
        self.items
            .iter()
            .copied()
            .filter(|&t| other.contains(ctx, t))
            .collect()
    }
}

/// `InterfaceLeastUpperBoundHelper`.
pub struct InterfaceLeastUpperBoundHelper<'a> {
    pub type_system: TypeSystem<'a>,
}

impl<'a> InterfaceLeastUpperBoundHelper<'a> {
    pub fn new(type_system: TypeSystem<'a>) -> Self {
        InterfaceLeastUpperBoundHelper { type_system }
    }

    /// `compute(type1, type2)`.
    ///
    /// This currently does not implement a very complete least upper bound
    /// algorithm, but handles a couple of the very common cases that are
    /// causing pain in real code.  The current algorithm is:
    /// 1. If either of the types is a supertype of the other, return it.
    ///    This is in fact the best result in this case.
    /// 2. If the two types have the same class element and are implicitly or
    ///    explicitly covariant, then take the pointwise least upper bound of
    ///    the type arguments. This is again the best result, except that the
    ///    recursive calls may not return the true least upper bounds. The
    ///    result is guaranteed to be a well-formed type under the assumption
    ///    that the input types were well-formed (and assuming that the
    ///    recursive calls return well-formed types).
    ///    If the variance of the type parameter is contravariant, we take the
    ///    greatest lower bound of the type arguments. If the variance of the
    ///    type parameter is invariant, we verify if the type arguments satisfy
    ///    subtyping in both directions, then choose a bound.
    /// 3. Otherwise return the spec-defined least upper bound.  This will
    ///    be an upper bound, might (or might not) be least, and might
    ///    (or might not) be a well-formed type.
    pub fn compute(&self, type1: TypeId, type2: TypeId) -> TypeId {
        let ts = self.type_system;
        let ctx = ts.ctx;
        let nullability = Self::choose_nullability(&ctx, type1, type2);

        // Strip off nullability.
        let type1 = ctx.with_nullability(type1, Nullability::None);
        let type2 = ctx.with_nullability(type2, Nullability::None);

        if ts.is_subtype_of(type1, type2) {
            return ctx.with_nullability(type2, nullability);
        }
        if ts.is_subtype_of(type2, type1) {
            return ctx.with_nullability(type1, nullability);
        }

        let element1 = ctx.interface_element(type1).expect("interface type");
        let element2 = ctx.interface_element(type2).expect("interface type");
        if element1 == element2 {
            let args1 = ctx.type_arguments(type1);
            let args2 = ctx.type_arguments(type2);
            let params = ctx.interface_type_parameters(element1);
            debug_assert_eq!(args1.len(), args2.len());
            debug_assert_eq!(args1.len(), params.len());

            let mut args = Vec::with_capacity(args1.len());
            for i in 0..args1.len() {
                let parameter_variance = ctx.type_parameter_variance(params[i]);
                match parameter_variance {
                    Variance::Covariant => {
                        args.push(ts.least_upper_bound(args1[i], args2[i]));
                    }
                    Variance::Contravariant => {
                        args.push(ts.greatest_lower_bound(args1[i], args2[i]));
                    }
                    Variance::Invariant => {
                        if !ts.is_subtype_of(args1[i], args2[i])
                            || !ts.is_subtype_of(args2[i], args1[i])
                        {
                            // No bound will be valid, find bound at the interface level.
                            let result = self.compute_least_upper_bound(type1, type2);
                            return ctx.with_nullability(result, nullability);
                        }
                        // TODO(kallentu): : Fix asymmetric bounds behavior for invariant type
                        //  parameters.
                        args.push(args1[i]);
                    }
                    Variance::Unrelated => {
                        panic!(
                            "Type parameter {:?} has unknown variance {parameter_variance:?} \
                             for bounds calculation.",
                            ctx.element_name(params[i].raw())
                        );
                    }
                }
            }

            return ctx.interface_type(element1, &args, nullability);
        }

        let mut result = self.compute_least_upper_bound(type1, type2);
        if nullability != Nullability::None {
            result = ctx.with_nullability(result, nullability);
        }
        result
    }

    /// `computeSuperinterfaceSet(type)` (`@visibleForTesting`): all of the
    /// superinterfaces of the given [type].
    pub fn compute_superinterface_set(&self, t: TypeId) -> InterfaceTypeSet {
        let mut result = InterfaceTypeSet::new();
        self.add_superinterfaces(&mut result, t);
        result
    }

    /// `_addSuperinterfaces(set, type)`: add all of the superinterfaces of
    /// the given [type] to the given [set].
    fn add_superinterfaces(&self, set: &mut InterfaceTypeSet, t: TypeId) {
        let ts = self.type_system;
        let ctx = ts.ctx;

        // `type.isDartCoreObjectNone || type.isDartCoreNull`
        if (ctx.is_dart_core_object(t) && ctx.nullability_suffix(t) == Nullability::None)
            || ctx.is_dart_core_null(t)
        {
            set.add(&ctx, ts.object_question());
            return;
        }

        let element = ctx.interface_element(t).expect("interface type");
        if element.raw().is::<ExtensionTypeElement>() {
            set.add(&ctx, ts.object_question());
        }

        for interface in ctx.interfaces(t) {
            if set.add(&ctx, interface) {
                self.add_superinterfaces(set, interface);
            }
        }

        for mixin in ctx.mixins(t) {
            if set.add(&ctx, mixin) {
                self.add_superinterfaces(set, mixin);
            }
        }

        for constraint in ctx.superclass_constraints(t) {
            if set.add(&ctx, constraint) {
                self.add_superinterfaces(set, constraint);
            }
        }

        if let Some(supertype) = ctx.superclass(t)
            && set.add(&ctx, supertype)
        {
            self.add_superinterfaces(set, supertype);
        }
    }

    /// `_computeLeastUpperBound(i, j)`: the least upper bound of types [i]
    /// and [j], both of which are known to be interface types.
    fn compute_least_upper_bound(&self, i: TypeId, j: TypeId) -> TypeId {
        let ctx = self.type_system.ctx;

        // compute set of supertypes
        let mut si = self.compute_superinterface_set(i);
        let mut sj = self.compute_superinterface_set(j);

        // union si with i and sj with j
        si.add(&ctx, i);
        sj.add(&ctx, j);

        // compute intersection, reference as set 's'
        let s = si.intersection(&ctx, &sj);
        Self::compute_type_at_max_unique_depth(&ctx, &s)
    }

    /// `computeLongestInheritancePathToObject(type)` (`@visibleForTesting`):
    /// the length of the longest inheritance path from the [type] to
    /// Object.
    pub fn compute_longest_inheritance_path_to_object(ctx: &Ctx<'_>, t: TypeId) -> i64 {
        Self::compute_longest_inheritance_path_to_object_visited(ctx, t, &mut Vec::new())
    }

    /// `_chooseNullability(type1, type2)`.
    fn choose_nullability(ctx: &Ctx<'_>, type1: TypeId, type2: TypeId) -> Nullability {
        let nullability1 = ctx.nullability_suffix(type1);
        let nullability2 = ctx.nullability_suffix(type2);
        if nullability1 == Nullability::Question || nullability2 == Nullability::Question {
            return Nullability::Question;
        }
        Nullability::None
    }

    /// `_computeLongestInheritancePathToObject(type, visitedElements)`: the
    /// length of the longest inheritance path from a subtype of the given
    /// [type] to `Object`.
    ///
    /// The set of [visited_elements] is used to prevent infinite recursion
    /// in the case of a cyclic type structure (Dart `Set<InterfaceElement>`;
    /// only `contains`, `add` and `remove` are used, so a `Vec` is enough).
    fn compute_longest_inheritance_path_to_object_visited(
        ctx: &Ctx<'_>,
        t: TypeId,
        visited_elements: &mut Vec<EId<InterfaceElement>>,
    ) -> i64 {
        let element = ctx.interface_element(t).expect("interface type");
        // recursion
        if visited_elements.contains(&element) {
            return 0;
        }
        // Null, direct subtype of Object?
        if ctx.is_dart_core_null(t) {
            return 1;
        }
        // Object case
        let class_element = element.raw().cast::<ClassElement>();
        if class_element.is_some() && ctx.is_element(element.raw(), "dart.core", "Object") {
            return if ctx.nullability_suffix(t) == Nullability::None {
                1
            } else {
                0
            };
        }

        // Extension type without interfaces, implicit `Object?`
        if element.raw().is::<ExtensionTypeElement>() && ctx.element_interfaces(element).is_empty()
        {
            return 1;
        }

        // Dart: `try { visitedElements.add(element); ... } finally {
        // visitedElements.remove(element); }`.
        visited_elements.push(element);
        let longest_path = Self::longest_path_body(ctx, element, class_element, visited_elements);
        if let Some(index) = visited_elements.iter().rposition(|&e| e == element) {
            visited_elements.remove(index);
        }
        longest_path
    }

    /// The `try` block of `_computeLongestInheritancePathToObject`.
    fn longest_path_body(
        ctx: &Ctx<'_>,
        element: EId<InterfaceElement>,
        class_element: Option<EId<ClassElement>>,
        visited_elements: &mut Vec<EId<InterfaceElement>>,
    ) -> i64 {
        let mut longest_path = 0;

        // loop through each of the superinterfaces recursively calling this
        // method and keeping track of the longest path to return
        if element.raw().is::<MixinElement>() {
            for &interface in ctx.element_superclass_constraints(element) {
                let path_length = Self::compute_longest_inheritance_path_to_object_visited(
                    ctx,
                    interface,
                    visited_elements,
                );
                longest_path = longest_path.max(1 + path_length);
            }
        }

        // loop through each of the superinterfaces recursively calling this
        // method and keeping track of the longest path to return
        for &interface in ctx.element_interfaces(element) {
            let path_length = Self::compute_longest_inheritance_path_to_object_visited(
                ctx,
                interface,
                visited_elements,
            );
            longest_path = longest_path.max(1 + path_length);
        }

        // `if (element is! ClassElement) return longestPath;`
        let Some(class_element) = class_element else {
            return longest_path;
        };

        let Some(supertype) = ctx.element_supertype(element) else {
            return longest_path;
        };

        let mut super_length = Self::compute_longest_inheritance_path_to_object_visited(
            ctx,
            supertype,
            visited_elements,
        );

        // Since mixin applications involve only one mixin, multiple mixins induce
        // multiple intermediate application classes. Consider the following
        // example:
        //
        //     class A extends B with M1, M2, M3 {}
        //
        // Applying M1, M2, and M3 to B results in the following chain of
        // declarations.
        //
        //     abstract class _A&B&M1 extends B implements M1 {}
        //     abstract class _A&B&M1&M2 extends _A&B&M1 implements M2 {}
        //     abstract class _A&B&M1&M2&M3 extends _AA&B&M1&M2 implements M3 {}
        //     class A extends _A&B&M1&M2&M3 {}
        //
        // Each of the intermediate applications increase the distance to the top
        // type by 1.
        //
        // Note that named mixin applications are different in that the last
        // application is the named application itself, so its distance from the
        // top type is shorter by 1. Consider the following example.
        //
        //     class A = B with M1, M2, M3;
        //
        // It will result in the following chain of declarations.
        //
        //     abstract class _A&B&M1 extends B implements M1 {}
        //     abstract class _A&B&M1&M2 extends _A&B&M1 implements M2 {}
        //     class A extends _A&B&M1&M2 implements M3 {}
        let mixins = ctx.element_mixins(element);
        for &mixin in mixins {
            // class _X&S&M extends S implements M {}
            // So, we choose the maximum length from S and M.
            let mixin_length = Self::compute_longest_inheritance_path_to_object_visited(
                ctx,
                mixin,
                visited_elements,
            );
            super_length = super_length.max(mixin_length);
            // For this synthetic class representing the mixin application.
            super_length += 1;
        }
        if !mixins.is_empty() {
            let fragment = ctx.fragment(ctx.get(class_element).first_fragment());
            if fragment
                .flags
                .has(FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_APPLICATION)
            {
                // In case of named mixin application, reduce the distance by 1.
                super_length -= 1;
            }
        }

        longest_path.max(1 + super_length)
    }

    /// `_computeTypeAtMaxUniqueDepth(types)`: the type from the [types] list
    /// that has the longest inheritance path to Object of unique length.
    fn compute_type_at_max_unique_depth(ctx: &Ctx<'_>, types: &[TypeId]) -> TypeId {
        // for each element in Set s, compute the largest inheritance path to Object
        let mut depths = vec![0i64; types.len()];
        let mut max_depth = 0;
        for i in 0..types.len() {
            depths[i] = Self::compute_longest_inheritance_path_to_object(ctx, types[i]);
            if depths[i] > max_depth {
                max_depth = depths[i];
            }
        }
        // ensure that the currently computed maxDepth is unique,
        // otherwise, decrement and test for uniqueness again
        while max_depth >= 0 {
            let mut index_of_least_upper_bound = 0;
            let mut number_of_types_at_max_depth = 0;
            for (m, &depth) in depths.iter().enumerate() {
                if depth == max_depth {
                    number_of_types_at_max_depth += 1;
                    index_of_least_upper_bound = m;
                }
            }
            if number_of_types_at_max_depth == 1 {
                return types[index_of_least_upper_bound];
            }
            max_depth -= 1;
        }
        // Should be impossible--there should always be exactly one type with the
        // maximum depth.
        panic!(
            "Empty path: {:?}",
            types
                .iter()
                .map(|&t| dartr_element::type_display_string_with(ctx, t, Default::default()))
                .collect::<Vec<_>>()
        );
    }
}

/// `LeastUpperBoundHelper`.
pub struct LeastUpperBoundHelper<'a> {
    pub type_system: TypeSystem<'a>,
}

impl<'a> LeastUpperBoundHelper<'a> {
    pub fn new(type_system: TypeSystem<'a>) -> Self {
        LeastUpperBoundHelper { type_system }
    }

    fn ctx(&self) -> Ctx<'a> {
        self.type_system.ctx
    }

    /// `_interfaceTypeFunctionNone`.
    fn interface_type_function_none(&self) -> TypeId {
        let ctx = self.ctx();
        let element = ctx.tp.function_element().upcast::<InterfaceElement>();
        ctx.instantiate_interface(element, &[], Nullability::None)
    }

    /// `getLeastUpperBound(T1, T2)`: the least upper bound of two types.
    ///
    /// https://github.com/dart-lang/language
    /// See `resources/type-system/upper-lower-bounds.md`
    #[allow(non_snake_case)]
    pub fn get_least_upper_bound(&self, T1: TypeId, T2: TypeId) -> TypeId {
        let _guard = crate::recursion_guard::enter("UP");
        let ts = self.type_system;
        let ctx = self.ctx();

        // UP(T, T) = T
        // Dart: identical
        if T1 == T2 {
            return T1;
        }

        // For any type T, UP(?, T) == T.
        // Dart: identical
        if T1 == TypeId::UNKNOWN {
            return T2;
        }
        // Dart: identical
        if T2 == TypeId::UNKNOWN {
            return T1;
        }

        let T1_isTop = ts.is_top(T1);
        let T2_isTop = ts.is_top(T2);

        // UP(T1, T2) where TOP(T1) and TOP(T2)
        if T1_isTop && T2_isTop {
            // * T1 if MORETOP(T1, T2)
            // * T2 otherwise
            if ts.is_more_top(T1, T2) {
                return T1;
            } else {
                return T2;
            }
        }

        // UP(T1, T2) = T1 if TOP(T1)
        if T1_isTop {
            return T1;
        }

        // UP(T1, T2) = T2 if TOP(T2)
        if T2_isTop {
            return T2;
        }

        let T1_isBottom = ctx.is_bottom(T1);
        let T2_isBottom = ctx.is_bottom(T2);

        // UP(T1, T2) where BOTTOM(T1) and BOTTOM(T2)
        if T1_isBottom && T2_isBottom {
            // * T2 if MOREBOTTOM(T1, T2)
            // * T1 otherwise
            if ts.is_more_bottom(T1, T2) {
                return T2;
            } else {
                return T1;
            }
        }

        // UP(T1, T2) = T2 if BOTTOM(T1)
        if T1_isBottom {
            return T2;
        }

        // UP(T1, T2) = T1 if BOTTOM(T2)
        if T2_isBottom {
            return T1;
        }

        // UP(X1 & B1, T2)
        if let TypeKind::TypeParameter {
            promoted_bound: Some(B1),
            ..
        } = *ctx.ty(T1)
        {
            let X1 = ctx.without_promoted_bound(T1);
            // T2 if X1 <: T2
            if ts.is_subtype_of(X1, T2) {
                return T2;
            }
            // otherwise X1 if T2 <: X1
            if ts.is_subtype_of(T2, X1) {
                return X1;
            }
            // otherwise UP(B1a, T2)
            //   where B1a is the greatest closure of B1 with respect to X1
            let B1a = ts.greatest_closure(B1, &[type_parameter_element(&ctx, X1)]);
            return self.get_least_upper_bound(B1a, T2);
        }

        // UP(T1, X2 & B2)
        if let TypeKind::TypeParameter {
            promoted_bound: Some(B2),
            ..
        } = *ctx.ty(T2)
        {
            let X2 = ctx.without_promoted_bound(T2);
            // X2 if T1 <: X2
            if ts.is_subtype_of(T1, X2) {
                return X2;
            }
            // otherwise T1 if X2 <: T1
            if ts.is_subtype_of(X2, T1) {
                return T1;
            }
            // otherwise UP(T1, B2a)
            //   where B2a is the greatest closure of B2 with respect to X2
            let B2a = ts.greatest_closure(B2, &[type_parameter_element(&ctx, X2)]);
            return self.get_least_upper_bound(T1, B2a);
        }

        let T1_isNull = ts.is_null(T1);
        let T2_isNull = ts.is_null(T2);

        // UP(T1, T2) where NULL(T1) and NULL(T2)
        if T1_isNull && T2_isNull {
            // * T2 if MOREBOTTOM(T1, T2)
            // * T1 otherwise
            if ts.is_more_bottom(T1, T2) {
                return T2;
            } else {
                return T1;
            }
        }

        let T1_nullability = ctx.nullability_suffix(T1);
        let T2_nullability = ctx.nullability_suffix(T2);

        // UP(T1, T2) where NULL(T1)
        if T1_isNull {
            // * T2 if T2 is nullable
            // * T2? otherwise
            if ts.is_nullable(T2) {
                return T2;
            } else {
                return ts.make_nullable(T2);
            }
        }

        // UP(T1, T2) where NULL(T2)
        if T2_isNull {
            // * T1 if T1 is nullable
            // * T1? otherwise
            if ts.is_nullable(T1) {
                return T1;
            } else {
                return ts.make_nullable(T1);
            }
        }

        let T1_isObject = ts.is_object(T1);
        let T2_isObject = ts.is_object(T2);

        // UP(T1, T2) where OBJECT(T1) and OBJECT(T2)
        if T1_isObject && T2_isObject {
            // * T1 if MORETOP(T1, T2)
            // * T2 otherwise
            if ts.is_more_top(T1, T2) {
                return T1;
            } else {
                return T2;
            }
        }

        // UP(T1, T2) where OBJECT(T1)
        if T1_isObject {
            // * T1 if T2 is non-nullable
            // * T1? otherwise
            if ts.is_non_nullable(T2) {
                return T1;
            } else {
                return ts.make_nullable(T1);
            }
        }

        // UP(T1, T2) where OBJECT(T2)
        if T2_isObject {
            // * T2 if T1 is non-nullable
            // * T2? otherwise
            if ts.is_non_nullable(T1) {
                return T2;
            } else {
                return ts.make_nullable(T2);
            }
        }

        // UP(T1?, T2?) = S? where S is UP(T1, T2)
        // UP(T1?, T2) = S? where S is UP(T1, T2)
        // UP(T1, T2?) = S? where S is UP(T1, T2)
        if T1_nullability != Nullability::None || T2_nullability != Nullability::None {
            let T1_none = ctx.with_nullability(T1, Nullability::None);
            let T2_none = ctx.with_nullability(T2, Nullability::None);
            let S = self.get_least_upper_bound(T1_none, T2_none);
            return ctx.with_nullability(S, Nullability::Question);
        }

        debug_assert_eq!(T1_nullability, Nullability::None);
        debug_assert_eq!(T2_nullability, Nullability::None);

        // UP(X1 extends B1, T2)
        if let TypeKind::TypeParameter { param, .. } = *ctx.ty(T1) {
            // T2 if X1 <: T2
            if ts.is_subtype_of(T1, T2) {
                return T2;
            }
            // otherwise X1 if T2 <: X1
            if ts.is_subtype_of(T2, T1) {
                return T1;
            }
            // otherwise UP(B1a, T2)
            //   where B1a is the greatest closure of B1 with respect to X1
            let bound = self.type_parameter_bound(T1);
            let closure = ts.greatest_closure(bound, &[param]);
            return self.get_least_upper_bound(closure, T2);
        }

        // UP(T1, X2 extends B2)
        if let TypeKind::TypeParameter { param, .. } = *ctx.ty(T2) {
            // X2 if T1 <: X2
            if ts.is_subtype_of(T1, T2) {
                // TODO(scheglov): How to get here?
                return T2;
            }
            // otherwise T1 if X2 <: T1
            if ts.is_subtype_of(T2, T1) {
                return T1;
            }
            // otherwise UP(T1, B2a)
            //   where B2a is the greatest closure of B2 with respect to X2
            let bound = self.type_parameter_bound(T2);
            let closure = ts.greatest_closure(bound, &[param]);
            return self.get_least_upper_bound(T1, closure);
        }

        let T1_isFunction = matches!(ctx.ty(T1), TypeKind::Function(_));
        let T2_isFunction = matches!(ctx.ty(T2), TypeKind::Function(_));

        // UP(T Function<...>(...), Function) = Function
        if T1_isFunction && ctx.is_dart_core_function(T2) {
            return T2;
        }

        // UP(Function, T Function<...>(...)) = Function
        if ctx.is_dart_core_function(T1) && T2_isFunction {
            return T1;
        }

        // UP(T Function<...>(...), S Function<...>(...)) = Function
        // And other, more interesting variants.
        if T1_isFunction && T2_isFunction {
            return self.function_type(T1, T2);
        }

        // UP(T Function<...>(...), T2) = UP(Object, T2)
        if T1_isFunction {
            return self.get_least_upper_bound(ts.object_none(), T2);
        }

        // UP(T1, T Function<...>(...)) = UP(T1, Object)
        if T2_isFunction {
            return self.get_least_upper_bound(T1, ts.object_none());
        }

        let T1_isRecord = matches!(ctx.ty(T1), TypeKind::Record { .. });
        let T2_isRecord = matches!(ctx.ty(T2), TypeKind::Record { .. });

        // UP((...), Record) = Record
        if T1_isRecord && ctx.is_dart_core_record(T2) {
            return T2;
        }

        // UP(Record, (...)) = Record
        if ctx.is_dart_core_record(T1) && T2_isRecord {
            return T1;
        }

        // Record types.
        if T1_isRecord && T2_isRecord {
            return self.record_type(T1, T2);
        }

        // UP(RecordType, T2) = UP(Object, T2)
        if T1_isRecord {
            return self.get_least_upper_bound(ts.object_none(), T2);
        }

        // UP(T1, RecordType) = UP(T1, Object)
        if T2_isRecord {
            return self.get_least_upper_bound(T1, ts.object_none());
        }

        if let Some(future_or_result) = self.future_or(T1, T2) {
            return future_or_result;
        }

        // UP(T1, T2) = T2 if T1 <: T2
        // UP(T1, T2) = T1 if T2 <: T1
        // And other, more complex variants of interface types.
        // Dart: `T1 as InterfaceTypeImpl`, `T2 as InterfaceTypeImpl`.
        assert!(
            ctx.interface_element(T1).is_some() && ctx.interface_element(T2).is_some(),
            "{:?} or {:?} is not an InterfaceTypeImpl",
            ctx.ty(T1),
            ctx.ty(T2)
        );
        let helper = InterfaceLeastUpperBoundHelper::new(ts);
        helper.compute(T1, T2)
    }

    /// `_functionType(f, g)`: the least upper bound of function types [f]
    /// and [g].
    ///
    /// https://github.com/dart-lang/language
    /// See `resources/type-system/upper-lower-bounds.md`
    fn function_type(&self, f: TypeId, g: TypeId) -> TypeId {
        let ts = self.type_system;
        let ctx = self.ctx();
        let f_type_formals = ctx.list(function_data(&ctx, f).type_params);
        let g_type_formals = ctx.list(function_data(&ctx, g).type_params);

        // The number of type parameters must be the same.
        // Otherwise the result is `Function`.
        if f_type_formals.len() != g_type_formals.len() {
            return self.interface_type_function_none();
        }

        // The bounds of type parameters must be equal.
        // Otherwise the result is `Function`.
        let Some(fresh) = ts.relate_type_parameters(f_type_formals, g_type_formals) else {
            return self.interface_type_function_none();
        };

        let f = function_data(
            &ctx,
            ctx.instantiate_function_type(f, &fresh.type_parameter_types),
        );
        let g = function_data(
            &ctx,
            ctx.instantiate_function_type(g, &fresh.type_parameter_types),
        );

        let f_parameters = ctx.list(f.params);
        let g_parameters = ctx.list(g.params);

        let mut parameters: Vec<FnParam> = Vec::new();
        let mut f_index = 0;
        let mut g_index = 0;
        while f_index < f_parameters.len() && g_index < g_parameters.len() {
            let f_parameter = &f_parameters[f_index];
            let g_parameter = &g_parameters[g_index];
            if is_required_positional(f_parameter.kind) {
                if is_required_positional(g_parameter.kind) {
                    f_index += 1;
                    g_index += 1;
                    // `fParameter.copyWith(type: ...)`
                    parameters.push(FnParam {
                        ty: self.parameter_type(f_parameter, g_parameter),
                        ..*f_parameter
                    });
                } else {
                    break;
                }
            } else if is_optional_positional(f_parameter.kind) {
                if is_optional_positional(g_parameter.kind) {
                    f_index += 1;
                    g_index += 1;
                    parameters.push(FnParam {
                        ty: self.parameter_type(f_parameter, g_parameter),
                        ..*f_parameter
                    });
                } else {
                    break;
                }
            } else if is_named(f_parameter.kind) {
                if is_named(g_parameter.kind) {
                    let (Some(f_name), Some(g_name)) = (f_parameter.name, g_parameter.name) else {
                        return self.interface_type_function_none();
                    };

                    // Dart `String.compareTo` compares UTF-16 code units; this
                    // is the byte order of the names for the names that occur.
                    let compare_names = ctx.name_str(f_name).cmp(ctx.name_str(g_name));
                    match compare_names {
                        Ordering::Equal => {
                            f_index += 1;
                            g_index += 1;
                            parameters.push(FnParam {
                                ty: self.parameter_type(f_parameter, g_parameter),
                                kind: if is_required_named(f_parameter.kind)
                                    || is_required_named(g_parameter.kind)
                                {
                                    ParameterKind::NamedRequired
                                } else {
                                    ParameterKind::Named
                                },
                                ..*f_parameter
                            });
                        }
                        Ordering::Less => {
                            if is_required_named(f_parameter.kind) {
                                // We cannot skip required named.
                                return self.interface_type_function_none();
                            } else {
                                f_index += 1;
                            }
                        }
                        Ordering::Greater => {
                            if is_required_named(g_parameter.kind) {
                                // We cannot skip required named.
                                return self.interface_type_function_none();
                            } else {
                                g_index += 1;
                            }
                        }
                    }
                } else {
                    break;
                }
            }
        }

        while f_index < f_parameters.len() {
            let f_parameter = &f_parameters[f_index];
            f_index += 1;
            if is_required(f_parameter.kind) {
                return self.interface_type_function_none();
            }
        }

        while g_index < g_parameters.len() {
            let g_parameter = &g_parameters[g_index];
            g_index += 1;
            if is_required(g_parameter.kind) {
                return self.interface_type_function_none();
            }
        }

        let return_type = self.get_least_upper_bound(f.ret, g.ret);

        ctx.function_type(
            &fresh.type_parameters,
            &parameters,
            return_type,
            Nullability::None,
            None,
        )
    }

    /// `_futureOr(T1, T2)`.
    #[allow(non_snake_case)]
    fn future_or(&self, T1: TypeId, T2: TypeId) -> Option<TypeId> {
        let ctx = self.ctx();
        let first_argument = |t: TypeId| ctx.type_arguments(t)[0];

        let T1_futureOr = ctx.is_dart_async_future_or(T1).then(|| first_argument(T1));

        let T1_future = ctx.is_dart_async_future(T1).then(|| first_argument(T1));

        let T2_futureOr = ctx.is_dart_async_future_or(T2).then(|| first_argument(T2));

        let T2_future = ctx.is_dart_async_future(T2).then(|| first_argument(T2));

        // UP(FutureOr<T1>, FutureOr<T2>) = FutureOr<T3> where T3 = UP(T1, T2)
        if let (Some(T1_futureOr), Some(T2_futureOr)) = (T1_futureOr, T2_futureOr) {
            let T3 = self.get_least_upper_bound(T1_futureOr, T2_futureOr);
            return Some(ctx.tp.future_or_type(&ctx, T3));
        }

        // UP(Future<T1>, FutureOr<T2>) = FutureOr<T3> where T3 = UP(T1, T2)
        if let (Some(T1_future), Some(T2_futureOr)) = (T1_future, T2_futureOr) {
            let T3 = self.get_least_upper_bound(T1_future, T2_futureOr);
            return Some(ctx.tp.future_or_type(&ctx, T3));
        }

        // UP(FutureOr<T1>, Future<T2>) = FutureOr<T3> where T3 = UP(T1, T2)
        if let (Some(T1_futureOr), Some(T2_future)) = (T1_futureOr, T2_future) {
            let T3 = self.get_least_upper_bound(T1_futureOr, T2_future);
            return Some(ctx.tp.future_or_type(&ctx, T3));
        }

        // UP(T1, FutureOr<T2>) = FutureOr<T3> where T3 = UP(T1, T2)
        if let Some(T2_futureOr) = T2_futureOr {
            let T3 = self.get_least_upper_bound(T1, T2_futureOr);
            return Some(ctx.tp.future_or_type(&ctx, T3));
        }

        // UP(FutureOr<T1>, T2) = FutureOr<T3> where T3 = UP(T1, T2)
        if let Some(T1_futureOr) = T1_futureOr {
            let T3 = self.get_least_upper_bound(T1_futureOr, T2);
            return Some(ctx.tp.future_or_type(&ctx, T3));
        }

        None
    }

    /// `_parameterType(a, b)`.
    fn parameter_type(&self, a: &FnParam, b: &FnParam) -> TypeId {
        self.type_system.greatest_lower_bound(a.ty, b.ty)
    }

    /// `_recordType(T1, T2)`.
    #[allow(non_snake_case)]
    fn record_type(&self, T1: TypeId, T2: TypeId) -> TypeId {
        let ctx = self.ctx();
        let (positional1, named1) = record_fields(&ctx, T1);
        let (positional2, named2) = record_fields(&ctx, T2);
        if positional1.len() != positional2.len() {
            return ctx.tp.record_type();
        }

        if named1.len() != named2.len() {
            return ctx.tp.record_type();
        }

        let mut positional_fields = Vec::with_capacity(positional1.len());
        for i in 0..positional1.len() {
            let field1 = positional1[i];
            let field2 = positional2[i];
            let t = self.get_least_upper_bound(field1, field2);
            positional_fields.push(t);
        }

        let mut named_fields = Vec::with_capacity(named1.len());
        for i in 0..named1.len() {
            let field1 = named1[i];
            let field2 = named2[i];
            if field1.name != field2.name {
                return ctx.tp.record_type();
            }
            let t = self.get_least_upper_bound(field1.ty, field2.ty);
            named_fields.push(NamedType {
                name: field1.name,
                ty: t,
            });
        }

        ctx.record_type(&positional_fields, &named_fields, Nullability::None, None)
    }

    /// `_typeParameterBound(type)`: the promoted or declared bound of the
    /// type parameter.
    fn type_parameter_bound(&self, t: TypeId) -> TypeId {
        let ctx = self.ctx();
        let TypeKind::TypeParameter {
            param,
            promoted_bound,
            ..
        } = *ctx.ty(t)
        else {
            unreachable!("type parameter type");
        };
        if let Some(bound) = promoted_bound.or_else(|| ctx.type_parameter_bound(param)) {
            return bound;
        }
        self.type_system.object_question()
    }
}

/// The data of a function type (Dart `f as FunctionTypeImpl`).
pub(crate) fn function_data(ctx: &Ctx<'_>, t: TypeId) -> dartr_element::FunctionTypeData {
    match *ctx.ty(t) {
        TypeKind::Function(f) => f,
        _ => unreachable!("function type"),
    }
}

/// The positional and the named fields of a record type.
pub(crate) fn record_fields<'a>(ctx: &Ctx<'a>, t: TypeId) -> (&'a [TypeId], &'a [NamedType]) {
    match *ctx.ty(t) {
        TypeKind::Record {
            positional, named, ..
        } => (ctx.list(positional), ctx.list(named)),
        _ => unreachable!("record type"),
    }
}

/// `X.element` of a type parameter type.
fn type_parameter_element(ctx: &Ctx<'_>, t: TypeId) -> EId<TypeParameterElement> {
    match *ctx.ty(t) {
        TypeKind::TypeParameter { param, .. } => param,
        _ => unreachable!("type parameter type"),
    }
}
