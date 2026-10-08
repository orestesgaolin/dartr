// Dart source: pkg/analyzer/lib/src/dart/element/greatest_lower_bound.dart

//! `GreatestLowerBoundHelper`: `DOWN(T1, T2)` of the spec.

use std::cmp::Ordering;

use dartr_element::{Ctx, FnParam, NamedType, Nullability, ParameterKind, TypeId, TypeKind};

use crate::least_upper_bound::{function_data, record_fields};
use crate::type_ext::{TypeExt, is_named, is_optional, is_positional, is_required_named};
use crate::type_system::TypeSystem;

/// `GreatestLowerBoundHelper`.
pub struct GreatestLowerBoundHelper<'a> {
    pub type_system: TypeSystem<'a>,
}

impl<'a> GreatestLowerBoundHelper<'a> {
    pub fn new(type_system: TypeSystem<'a>) -> Self {
        GreatestLowerBoundHelper { type_system }
    }

    fn ctx(&self) -> Ctx<'a> {
        self.type_system.ctx
    }

    /// `_nullNone`.
    fn null_none(&self) -> TypeId {
        self.type_system.null_none()
    }

    /// `getGreatestLowerBound(T1, T2)`: the greatest lower bound of [T1]
    /// and [T2].
    ///
    /// https://github.com/dart-lang/language
    /// See `resources/type-system/upper-lower-bounds.md`
    #[allow(non_snake_case)]
    pub fn get_greatest_lower_bound(&self, T1: TypeId, T2: TypeId) -> TypeId {
        let ts = self.type_system;
        let ctx = self.ctx();

        // DOWN(T, T) = T
        // Dart: identical
        if T1 == T2 {
            return T1;
        }

        // For any type T, DOWN(?, T) == T.
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

        // DOWN(T1, T2) where TOP(T1) and TOP(T2)
        if T1_isTop && T2_isTop {
            // * T1 if MORETOP(T2, T1)
            // * T2 otherwise
            if ts.is_more_top(T2, T1) {
                return T1;
            } else {
                return T2;
            }
        }

        // DOWN(T1, T2) = T2 if TOP(T1)
        if T1_isTop {
            return T2;
        }

        // DOWN(T1, T2) = T1 if TOP(T2)
        if T2_isTop {
            return T1;
        }

        let T1_isBottom = ctx.is_bottom(T1);
        let T2_isBottom = ctx.is_bottom(T2);

        // DOWN(T1, T2) where BOTTOM(T1) and BOTTOM(T2)
        if T1_isBottom && T2_isBottom {
            // * T1 if MOREBOTTOM(T1, T2)
            // * T2 otherwise
            if ts.is_more_bottom(T1, T2) {
                return T1;
            } else {
                return T2;
            }
        }

        // DOWN(T1, T2) = T1 if BOTTOM(T1)
        if T1_isBottom {
            return T1;
        }

        // DOWN(T1, T2) = T2 if BOTTOM(T2)
        if T2_isBottom {
            return T2;
        }

        let T1_isNull = ts.is_null(T1);
        let T2_isNull = ts.is_null(T2);

        // DOWN(T1, T2) where NULL(T1) and NULL(T2)
        if T1_isNull && T2_isNull {
            // * T1 if MOREBOTTOM(T1, T2)
            // * T2 otherwise
            if ts.is_more_bottom(T1, T2) {
                return T1;
            } else {
                return T2;
            }
        }

        let T1_nullability = ctx.nullability_suffix(T1);
        let T2_nullability = ctx.nullability_suffix(T2);

        // DOWN(Null, T2)
        if T1_nullability == Nullability::None && ctx.is_dart_core_null(T1) {
            // * Null if Null <: T2
            // * Never otherwise
            if ts.is_subtype_of(self.null_none(), T2) {
                return self.null_none();
            } else {
                return TypeId::NEVER;
            }
        }

        // DOWN(T1, Null)
        if T2_nullability == Nullability::None && ctx.is_dart_core_null(T2) {
            // * Null if Null <: T1
            // * Never otherwise
            if ts.is_subtype_of(self.null_none(), T1) {
                return self.null_none();
            } else {
                return TypeId::NEVER;
            }
        }

        let T1_isObject = ts.is_object(T1);
        let T2_isObject = ts.is_object(T2);

        // DOWN(T1, T2) where OBJECT(T1) and OBJECT(T2)
        if T1_isObject && T2_isObject {
            // * T1 if MORETOP(T2, T1)
            // * T2 otherwise
            if ts.is_more_top(T2, T1) {
                return T1;
            } else {
                return T2;
            }
        }

        // DOWN(T1, T2) where OBJECT(T1)
        if T1_isObject {
            // * T2 if T2 is non-nullable
            if ts.is_non_nullable(T2) {
                return T2;
            }

            // * NonNull(T2) if NonNull(T2) is non-nullable
            let T2_nonNull = ts.promote_to_non_null(T2);
            if ts.is_non_nullable(T2_nonNull) {
                return T2_nonNull;
            }

            // * Never otherwise
            return TypeId::NEVER;
        }

        // DOWN(T1, T2) where OBJECT(T2)
        if T2_isObject {
            // * T1 if T1 is non-nullable
            if ts.is_non_nullable(T1) {
                return T1;
            }

            // * NonNull(T1) if NonNull(T1) is non-nullable
            let T1_nonNull = ts.promote_to_non_null(T1);
            if ts.is_non_nullable(T1_nonNull) {
                return T1_nonNull;
            }

            // * Never otherwise
            return TypeId::NEVER;
        }

        // DOWN(T1?, T2?) = S? where S is DOWN(T1, T2)
        // DOWN(T1?, T2) = S where S is DOWN(T1, T2)
        // DOWN(T1, T2?) = S where S is DOWN(T1, T2)
        if T1_nullability != Nullability::None || T2_nullability != Nullability::None {
            let T1_none = ctx.with_nullability(T1, Nullability::None);
            let T2_none = ctx.with_nullability(T2, Nullability::None);
            let S = self.get_greatest_lower_bound(T1_none, T2_none);
            if T1_nullability == Nullability::Question && T2_nullability == Nullability::Question {
                return ctx.with_nullability(S, Nullability::Question);
            }
            return S;
        }

        debug_assert_eq!(T1_nullability, Nullability::None);
        debug_assert_eq!(T2_nullability, Nullability::None);

        let T1_isFunction = matches!(ctx.ty(T1), TypeKind::Function(_));
        let T2_isFunction = matches!(ctx.ty(T2), TypeKind::Function(_));
        if T1_isFunction && T2_isFunction {
            return self.function_type(T1, T2);
        }

        let T1_isRecord = matches!(ctx.ty(T1), TypeKind::Record { .. });
        let T2_isRecord = matches!(ctx.ty(T2), TypeKind::Record { .. });
        if T1_isRecord && T2_isRecord {
            return self.record_type(T1, T2);
        }

        // DOWN(T1, T2) = T1 if T1 <: T2
        if ts.is_subtype_of(T1, T2) {
            return T1;
        }

        // DOWN(T1, T2) = T2 if T2 <: T1
        if ts.is_subtype_of(T2, T1) {
            return T2;
        }

        // FutureOr<S1>
        if ctx.is_dart_async_future_or(T1) {
            let S1 = ctx.type_arguments(T1)[0];
            // DOWN(FutureOr<S1>, FutureOr<S2>) = FutureOr(S)
            //   S = DOWN(S1, S2)
            if ctx.is_dart_async_future_or(T2) {
                let S2 = ctx.type_arguments(T2)[0];
                let S = self.get_greatest_lower_bound(S1, S2);
                return ctx.tp.future_or_type(&ctx, S);
            }
            // DOWN(FutureOr<S1>, Future<S2>) = Future(S)
            //   S = DOWN(S1, S2)
            if ctx.is_dart_async_future(T2) {
                let S2 = ctx.type_arguments(T2)[0];
                let S = self.get_greatest_lower_bound(S1, S2);
                return ctx.tp.future_type(&ctx, S);
            }
            // DOWN(FutureOr<S1>, T2) = DOWN(S1, T2)
            return self.get_greatest_lower_bound(S1, T2);
        }

        // FutureOr<S2>
        if ctx.is_dart_async_future_or(T2) {
            let S2 = ctx.type_arguments(T2)[0];
            // DOWN(Future<S1>, FutureOr<S2>) = Future<S>
            //   S = DOWN(S1, S2)
            if ctx.is_dart_async_future(T1) {
                let S1 = ctx.type_arguments(T1)[0];
                let S = self.get_greatest_lower_bound(S1, S2);
                return ctx.tp.future_type(&ctx, S);
            }
            // DOWN(T1, FutureOr<S2>) = DOWN(T1, S2)
            return self.get_greatest_lower_bound(T1, S2);
        }

        // DOWN(T1, T2) = Never otherwise
        TypeId::NEVER
    }

    /// `_functionType(f, g)`: the greatest lower bound of function types
    /// [f] and [g].
    ///
    /// https://github.com/dart-lang/language
    /// See `resources/type-system/upper-lower-bounds.md`
    fn function_type(&self, f: TypeId, g: TypeId) -> TypeId {
        let ts = self.type_system;
        let ctx = self.ctx();
        let f_type_formals = ctx.list(function_data(&ctx, f).type_params);
        let g_type_formals = ctx.list(function_data(&ctx, g).type_params);

        // The number of type parameters must be the same.
        // Otherwise the result is `Never`.
        if f_type_formals.len() != g_type_formals.len() {
            return TypeId::NEVER;
        }

        // The bounds of type parameters must be equal.
        // Otherwise the result is `Never`.
        let Some(fresh) = ts.relate_type_parameters(f_type_formals, g_type_formals) else {
            return TypeId::NEVER;
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
            if is_positional(f_parameter.kind) {
                if is_positional(g_parameter.kind) {
                    f_index += 1;
                    g_index += 1;
                    // `fParameter.copyWith(type: ..., kind: ...)`
                    parameters.push(FnParam {
                        ty: ts.least_upper_bound(f_parameter.ty, g_parameter.ty),
                        kind: if is_optional(f_parameter.kind) || is_optional(g_parameter.kind) {
                            ParameterKind::Positional
                        } else {
                            ParameterKind::Required
                        },
                        ..*f_parameter
                    });
                } else {
                    return TypeId::NEVER;
                }
            } else if is_named(f_parameter.kind) {
                if is_named(g_parameter.kind) {
                    let (Some(f_name), Some(g_name)) = (f_parameter.name, g_parameter.name) else {
                        return TypeId::NEVER;
                    };

                    // Dart `String.compareTo` compares UTF-16 code units; this
                    // is the byte order of the names for the names that occur.
                    let compare_names = ctx.name_str(f_name).cmp(ctx.name_str(g_name));
                    match compare_names {
                        Ordering::Equal => {
                            f_index += 1;
                            g_index += 1;
                            parameters.push(FnParam {
                                ty: ts.least_upper_bound(f_parameter.ty, g_parameter.ty),
                                kind: if is_required_named(f_parameter.kind)
                                    && is_required_named(g_parameter.kind)
                                {
                                    ParameterKind::NamedRequired
                                } else {
                                    ParameterKind::Named
                                },
                                ..*f_parameter
                            });
                        }
                        Ordering::Less => {
                            f_index += 1;
                            parameters.push(FnParam {
                                kind: ParameterKind::Named,
                                ..*f_parameter
                            });
                        }
                        Ordering::Greater => {
                            g_index += 1;
                            parameters.push(FnParam {
                                kind: ParameterKind::Named,
                                ..*g_parameter
                            });
                        }
                    }
                } else {
                    return TypeId::NEVER;
                }
            }
        }

        while f_index < f_parameters.len() {
            let f_parameter = &f_parameters[f_index];
            f_index += 1;
            if is_positional(f_parameter.kind) {
                parameters.push(FnParam {
                    kind: ParameterKind::Positional,
                    ..*f_parameter
                });
            } else {
                debug_assert!(is_named(f_parameter.kind));
                parameters.push(FnParam {
                    kind: ParameterKind::Named,
                    ..*f_parameter
                });
            }
        }

        while g_index < g_parameters.len() {
            let g_parameter = &g_parameters[g_index];
            g_index += 1;
            if is_positional(g_parameter.kind) {
                parameters.push(FnParam {
                    kind: ParameterKind::Positional,
                    ..*g_parameter
                });
            } else {
                debug_assert!(is_named(g_parameter.kind));
                parameters.push(FnParam {
                    kind: ParameterKind::Named,
                    ..*g_parameter
                });
            }
        }

        let return_type = self.get_greatest_lower_bound(f.ret, g.ret);

        ctx.function_type(
            &fresh.type_parameters,
            &parameters,
            return_type,
            Nullability::None,
            None,
        )
    }

    /// `_recordType(T1, T2)`.
    #[allow(non_snake_case)]
    fn record_type(&self, T1: TypeId, T2: TypeId) -> TypeId {
        let ctx = self.ctx();
        let (positional1, named1) = record_fields(&ctx, T1);
        let (positional2, named2) = record_fields(&ctx, T2);
        if positional1.len() != positional2.len() {
            return ctx.tp.never_type();
        }

        if named1.len() != named2.len() {
            return ctx.tp.never_type();
        }

        let mut positional_fields = Vec::with_capacity(positional1.len());
        for i in 0..positional1.len() {
            let field1 = positional1[i];
            let field2 = positional2[i];
            let t = self.get_greatest_lower_bound(field1, field2);
            positional_fields.push(t);
        }

        let mut named_fields = Vec::with_capacity(named1.len());
        for i in 0..named1.len() {
            let field1 = named1[i];
            let field2 = named2[i];
            if field1.name != field2.name {
                return ctx.tp.never_type();
            }
            let t = self.get_greatest_lower_bound(field1.ty, field2.ty);
            named_fields.push(NamedType {
                name: field1.name,
                ty: t,
            });
        }

        ctx.record_type(&positional_fields, &named_fields, Nullability::None, None)
    }
}
