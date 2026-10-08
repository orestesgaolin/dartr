// Dart source: pkg/analyzer/lib/src/dart/element/subtype.dart

//! `SubtypeHelper`: the subtype relation
//! (`resources/type-system/subtyping.md`).

use dartr_element::{EId, InterfaceElement, Nullability, TypeId, TypeKind, Variance};

use crate::type_algebra::MapSubstitution;
use crate::type_ext::{
    TypeExt, is_named, is_optional_positional, is_positional, is_required, is_required_named,
    is_required_positional,
};
use crate::type_system::TypeSystem;

/// `SubtypeHelper`.
pub struct SubtypeHelper<'a> {
    type_system: TypeSystem<'a>,
    null_none: TypeId,
    object_none: TypeId,
    object_question: TypeId,
}

impl<'a> SubtypeHelper<'a> {
    pub fn new(type_system: TypeSystem<'a>) -> Self {
        SubtypeHelper {
            null_none: type_system.null_none(),
            object_none: type_system.object_none(),
            object_question: type_system.object_question(),
            type_system,
        }
    }

    /// `isSubtypeOf(T0, T1)`.
    pub fn is_subtype_of(&self, t0: TypeId, t1: TypeId) -> bool {
        let ctx = self.type_system.ctx;
        // Reflexivity: if `T0` and `T1` are the same type then `T0 <: T1`.
        // Dart: identical(T0, T1)
        if t0 == t1 {
            return true;
        }

        // `_` is treated as a top and a bottom type during inference.
        if t0 == TypeId::UNKNOWN || t1 == TypeId::UNKNOWN {
            return true;
        }

        let k0 = *ctx.ty(t0);
        let k1 = *ctx.ty(t1);

        if matches!(k0, TypeKind::Invalid) || matches!(k1, TypeKind::Invalid) {
            return true;
        }

        let t1_nullability = ctx.nullability_suffix(t1);
        let t0_nullability = ctx.nullability_suffix(t0);

        // Right Top: if `T1` is a top type (i.e. `dynamic`, or `void`, or
        // `Object?`) then `T0 <: T1`.
        if matches!(k1, TypeKind::Dynamic | TypeKind::Invalid | TypeKind::Void)
            || t1_nullability == Nullability::Question && ctx.is_dart_core_object(t1)
        {
            return true;
        }

        // Left Top: if `T0` is `dynamic` or `void`,
        //   then `T0 <: T1` if `Object? <: T1`.
        if matches!(k0, TypeKind::Dynamic | TypeKind::Invalid | TypeKind::Void)
            && self.is_subtype_of(self.object_question, t1)
        {
            return true;
        }

        // Left Bottom: if `T0` is `Never`, then `T0 <: T1`.
        if k0 == TypeKind::Never(Nullability::None) {
            return true;
        }

        // Right Object: if `T1` is `Object` then:
        if t1_nullability == Nullability::None && ctx.is_dart_core_object(t1) {
            // * if `T0` is an unpromoted type variable with bound `B`,
            //   then `T0 <: T1` iff `B <: Object`.
            // * if `T0` is a promoted type variable `X & S`,
            //   then `T0 <: T1` iff `S <: Object`.
            if t0_nullability == Nullability::None
                && let TypeKind::TypeParameter {
                    param,
                    promoted_bound,
                    ..
                } = k0
            {
                return match promoted_bound {
                    None => {
                        let b = ctx
                            .type_parameter_bound(param)
                            .unwrap_or(self.object_question);
                        self.is_subtype_of(b, self.object_none)
                    }
                    Some(s) => self.is_subtype_of(s, self.object_none),
                };
            }
            // * if `T0` is `FutureOr<S>` for some `S`,
            //   then `T0 <: T1` iff `S <: Object`
            if t0_nullability == Nullability::None && ctx.is_dart_async_future_or(t0) {
                return self.is_subtype_of(ctx.type_arguments(t0)[0], t1);
            }
            // * if `T0` is `Null`, `dynamic`, `void`, or `S?` for any `S`,
            //   then the subtyping does not hold, the result is false.
            if t0_nullability == Nullability::None && ctx.is_dart_core_null(t0)
                || matches!(k0, TypeKind::Dynamic | TypeKind::Invalid | TypeKind::Void)
                || t0_nullability == Nullability::Question
            {
                return false;
            }
            // Extension types require explicit `Object` implementation.
            if ctx.is_extension_type(t0) {
                for interface in ctx.interfaces(t0) {
                    if self.is_subtype_of(interface, t1) {
                        return true;
                    }
                }
                return false;
            }
            // Otherwise `T0 <: T1` is true.
            return true;
        }

        // Left Null: if `T0` is `Null` then:
        if t0_nullability == Nullability::None && ctx.is_dart_core_null(t0) {
            // * If `T1` is `FutureOr<S>` for some `S`, then the query is true
            // iff `Null <: S`.
            if t1_nullability == Nullability::None && ctx.is_dart_async_future_or(t1) {
                let s = ctx.type_arguments(t1)[0];
                return self.is_subtype_of(self.null_none, s);
            }
            // If `T1` is `Null` or `S?` for some `S`, then the query is true.
            if t1_nullability == Nullability::None && ctx.is_dart_core_null(t1)
                || t1_nullability == Nullability::Question
            {
                return true;
            }
            // * if `T1` is a type variable (promoted or not) the query is false
            // Otherwise, the query is false.
            return false;
        }

        // Left FutureOr: if `T0` is `FutureOr<S0>` then:
        if t0_nullability == Nullability::None && ctx.is_dart_async_future_or(t0) {
            let s0 = ctx.type_arguments(t0)[0];
            // * `T0 <: T1` iff `Future<S0> <: T1` and `S0 <: T1`
            if self.is_subtype_of(s0, t1) {
                let future_s0 = ctx.instantiate_interface(
                    ctx.tp.future_element().upcast(),
                    &[s0],
                    Nullability::None,
                );
                return self.is_subtype_of(future_s0, t1);
            }
            return false;
        }

        // Left Nullable: if `T0` is `S0?` then:
        //   * `T0 <: T1` iff `S0 <: T1` and `Null <: T1`.
        if t0_nullability == Nullability::Question {
            let s0 = ctx.with_nullability(t0, Nullability::None);
            return self.is_subtype_of(s0, t1) && self.is_subtype_of(self.null_none, t1);
        }

        // Type Variable Reflexivity 1: if T0 is a type variable X0 or a
        // promoted type variables X0 & S0 and T1 is X0 then:
        //   * T0 <: T1
        if let (
            TypeKind::TypeParameter { param: p0, .. },
            TypeKind::TypeParameter {
                param: p1,
                promoted_bound: None,
                ..
            },
        ) = (k0, k1)
            && p0 == p1
        {
            return true;
        }

        // Right Promoted Variable: if `T1` is a promoted type variable `X1 & S1`:
        //   * `T0 <: T1` iff `T0 <: X1` and `T0 <: S1`
        if let TypeKind::TypeParameter {
            param,
            nullability,
            promoted_bound: Some(t1_promoted_bound),
            ..
        } = k1
        {
            let x1 = ctx.type_parameter_type(param, nullability);
            return self.is_subtype_of(t0, x1) && self.is_subtype_of(t0, t1_promoted_bound);
        }

        // Right FutureOr: if `T1` is `FutureOr<S1>` then:
        if t1_nullability == Nullability::None && ctx.is_dart_async_future_or(t1) {
            let s1 = ctx.type_arguments(t1)[0];
            // `T0 <: T1` iff any of the following hold:
            // * either `T0 <: Future<S1>`
            let future_s1 = ctx.instantiate_interface(
                ctx.tp.future_element().upcast(),
                &[s1],
                Nullability::None,
            );
            if self.is_subtype_of(t0, future_s1) {
                return true;
            }
            // * or `T0 <: S1`
            if self.is_subtype_of(t0, s1) {
                return true;
            }
            // * or `T0` is `X0` and `X0` has bound `S0` and `S0 <: T1`
            // * or `T0` is `X0 & S0` and `S0 <: T1`
            if self.type_variable_bound_is_subtype(k0, t1) {
                return true;
            }
            // iff
            return false;
        }

        // Right Nullable: if `T1` is `S1?` then:
        if t1_nullability == Nullability::Question {
            let s1 = ctx.with_nullability(t1, Nullability::None);
            // `T0 <: T1` iff any of the following hold:
            // * either `T0 <: S1`
            if self.is_subtype_of(t0, s1) {
                return true;
            }
            // * or `T0 <: Null`
            if self.is_subtype_of(t0, self.null_none) {
                return true;
            }
            // or `T0` is `X0` and `X0` has bound `S0` and `S0 <: T1`
            // or `T0` is `X0 & S0` and `S0 <: T1`
            if self.type_variable_bound_is_subtype(k0, t1) {
                return true;
            }
            // iff
            return false;
        }

        // Super-Interface: `T0` is an interface type with super-interfaces
        // `S0,...Sn`:
        //   * and `Si <: T1` for some `i`.
        if matches!(k0, TypeKind::Interface { .. }) && matches!(k1, TypeKind::Interface { .. }) {
            return self.is_interface_subtype_of(t0, t1);
        }

        // Left Promoted Variable: `T0` is a promoted type variable `X0 & S0`
        //   * and `S0 <: T1`
        // Left Type Variable Bound: `T0` is a type variable `X0` with bound `B0`
        //   * and `B0 <: T1`
        if self.type_variable_bound_is_subtype(k0, t1) {
            return true;
        }

        if let TypeKind::Function(_) = k0 {
            // Function Type/Function: `T0` is a function type and `T1` is
            // `Function`.
            if ctx.is_dart_core_function(t1) {
                return true;
            }
            if let TypeKind::Function(_) = k1 {
                return self.is_function_subtype_of(t0, t1);
            }
        }

        if let TypeKind::Record { .. } = k0 {
            // Record Type/Record: `T0` is a record type, and `T1` is `Record`.
            if ctx.is_dart_core_record(t1) {
                return true;
            }
            if let TypeKind::Record { .. } = k1 {
                return self.is_record_subtype_of(t0, t1);
            }
        }

        false
    }

    /// The shared check "`T0` is `X0 & S0` and `S0 <: T1`, or `T0` is `X0`
    /// with bound `B0` and `B0 <: T1`" (promoted bound first).
    fn type_variable_bound_is_subtype(&self, k0: TypeKind, t1: TypeId) -> bool {
        let ctx = self.type_system.ctx;
        if let TypeKind::TypeParameter {
            param,
            promoted_bound,
            ..
        } = k0
        {
            if let Some(s0) = promoted_bound
                && self.is_subtype_of(s0, t1)
            {
                return true;
            }
            if let Some(b0) = ctx.type_parameter_bound(param)
                && self.is_subtype_of(b0, t1)
            {
                return true;
            }
        }
        false
    }

    /// `_interfaceArguments(element, subType, superType)`.
    fn interface_arguments(
        &self,
        element: EId<InterfaceElement>,
        sub_type: TypeId,
        super_type: TypeId,
    ) -> bool {
        let ctx = self.type_system.ctx;
        let parameters = ctx.interface_type_parameters(element);
        let sub_arguments = ctx.type_arguments(sub_type);
        let super_arguments = ctx.type_arguments(super_type);
        debug_assert_eq!(sub_arguments.len(), super_arguments.len());
        debug_assert_eq!(parameters.len(), sub_arguments.len());

        for i in 0..sub_arguments.len() {
            let sub_argument = sub_arguments[i];
            let super_argument = super_arguments[i];
            match ctx.type_parameter_variance(parameters[i]) {
                Variance::Covariant => {
                    if !self.is_subtype_of(sub_argument, super_argument) {
                        return false;
                    }
                }
                Variance::Contravariant => {
                    if !self.is_subtype_of(super_argument, sub_argument) {
                        return false;
                    }
                }
                Variance::Invariant => {
                    if !self.is_subtype_of(sub_argument, super_argument)
                        || !self.is_subtype_of(super_argument, sub_argument)
                    {
                        return false;
                    }
                }
                Variance::Unrelated => panic!(
                    "Type parameter has unknown variance unrelated for subtype checking."
                ),
            }
        }
        true
    }

    /// `_isFunctionSubtypeOf(f, g)`.
    fn is_function_subtype_of(&self, f: TypeId, g: TypeId) -> bool {
        let ctx = self.type_system.ctx;
        let (TypeKind::Function(fd), TypeKind::Function(gd)) = (*ctx.ty(f), *ctx.ty(g)) else {
            unreachable!()
        };
        let Some(fresh) = self
            .type_system
            .relate_type_parameters(ctx.list(fd.type_params), ctx.list(gd.type_params))
        else {
            return false;
        };

        let f = ctx.instantiate_function_type(f, &fresh.type_parameter_types);
        let g = ctx.instantiate_function_type(g, &fresh.type_parameter_types);
        let (TypeKind::Function(fd), TypeKind::Function(gd)) = (*ctx.ty(f), *ctx.ty(g)) else {
            unreachable!()
        };

        if !self.is_subtype_of(fd.ret, gd.ret) {
            return false;
        }

        let f_parameters = ctx.list(fd.params);
        let g_parameters = ctx.list(gd.params);

        let mut f_index = 0;
        let mut g_index = 0;
        while f_index < f_parameters.len() && g_index < g_parameters.len() {
            let f_parameter = &f_parameters[f_index];
            let g_parameter = &g_parameters[g_index];
            if is_required_positional(f_parameter.kind) {
                if is_required_positional(g_parameter.kind) {
                    if self.is_subtype_of(g_parameter.ty, f_parameter.ty) {
                        f_index += 1;
                        g_index += 1;
                    } else {
                        return false;
                    }
                } else {
                    return false;
                }
            } else if is_optional_positional(f_parameter.kind) {
                if is_positional(g_parameter.kind) {
                    if self.is_subtype_of(g_parameter.ty, f_parameter.ty) {
                        f_index += 1;
                        g_index += 1;
                    } else {
                        return false;
                    }
                } else {
                    return false;
                }
            } else if is_named(f_parameter.kind) {
                if is_named(g_parameter.kind) {
                    let (Some(f_name), Some(g_name)) = (f_parameter.name, g_parameter.name) else {
                        return false;
                    };
                    let f_name = ctx.name_str(f_name);
                    let g_name = ctx.name_str(g_name);
                    match f_name.cmp(g_name) {
                        std::cmp::Ordering::Equal => {
                            if is_required_named(f_parameter.kind)
                                && !is_required_named(g_parameter.kind)
                            {
                                return false;
                            } else if self.is_subtype_of(g_parameter.ty, f_parameter.ty) {
                                f_index += 1;
                                g_index += 1;
                            } else {
                                return false;
                            }
                        }
                        std::cmp::Ordering::Less => {
                            if is_required_named(f_parameter.kind) {
                                return false;
                            } else {
                                f_index += 1;
                            }
                        }
                        // The subtype must accept all parameters of the supertype.
                        std::cmp::Ordering::Greater => return false,
                    }
                } else {
                    break;
                }
            }
        }

        // The supertype must provide all required parameters to the subtype.
        while f_index < f_parameters.len() {
            let f_parameter = &f_parameters[f_index];
            f_index += 1;
            if is_required(f_parameter.kind) {
                return false;
            }
        }

        // The subtype must accept all parameters of the supertype.
        if g_index < g_parameters.len() {
            return false;
        }

        true
    }

    /// `_isInterfaceSubtypeOf(subType, superType)`.
    fn is_interface_subtype_of(&self, sub_type: TypeId, super_type: TypeId) -> bool {
        let ctx = self.type_system.ctx;
        // Dart: identical(subType, superType)
        if sub_type == super_type || ctx.is_dart_core_object(super_type) {
            return true;
        }

        // Object cannot subtype anything but itself (handled above).
        if ctx.is_dart_core_object(sub_type) {
            return false;
        }

        let sub_element = ctx.interface_element(sub_type).unwrap();
        let super_element = ctx.interface_element(super_type).unwrap();
        if sub_element == super_element {
            return self.interface_arguments(super_element, sub_type, super_type);
        }

        // Classes types cannot subtype `Function` or vice versa.
        if ctx.is_dart_core_function(sub_type) || ctx.is_dart_core_function(super_type) {
            return false;
        }

        for &interface in ctx.element_all_supertypes(sub_element) {
            if ctx.interface_element(interface) == Some(super_element) {
                let substitution = MapSubstitution::from_interface_type(&ctx, sub_type);
                let substituted_interface = substitution.substitute_type(&ctx, interface);
                return self.interface_arguments(super_element, substituted_interface, super_type);
            }
        }

        false
    }

    /// `_isRecordSubtypeOf(subType, superType)`.
    fn is_record_subtype_of(&self, sub_type: TypeId, super_type: TypeId) -> bool {
        let ctx = self.type_system.ctx;
        let (
            TypeKind::Record {
                positional: sub_positional,
                named: sub_named,
                ..
            },
            TypeKind::Record {
                positional: super_positional,
                named: super_named,
                ..
            },
        ) = (*ctx.ty(sub_type), *ctx.ty(super_type))
        else {
            unreachable!()
        };
        let (sub_positional, super_positional) = (ctx.list(sub_positional), ctx.list(super_positional));
        if sub_positional.len() != super_positional.len() {
            return false;
        }
        let (sub_named, super_named) = (ctx.list(sub_named), ctx.list(super_named));
        if sub_named.len() != super_named.len() {
            return false;
        }
        for (&sub, &sup) in sub_positional.iter().zip(super_positional) {
            if !self.is_subtype_of(sub, sup) {
                return false;
            }
        }
        for (sub, sup) in sub_named.iter().zip(super_named) {
            if sub.name != sup.name {
                return false;
            }
            if !self.is_subtype_of(sub.ty, sup.ty) {
                return false;
            }
        }
        true
    }
}
