// Dart source: pkg/analyzer/lib/src/dart/element/normalize.dart

//! `NormalizeHelper`: the canonical presentation of types
//! (`resources/type-system/normalization.md` in dart-lang/language).
//!
//! Notes on the port:
//! - `_functionType` always makes fresh type parameters (as Dart does), so
//!   two normalizations of the same generic function type give different
//!   [`TypeId`]s that are Dart `==`.
//! - Dart `_typeParameters` is a `Set` used only for `add` / `remove`; it is
//!   a `Vec` here (the stack of type parameters whose bounds are being
//!   normalized).
//! - `UnknownInferredType` (`_`) is returned unchanged. In Dart it reaches
//!   `_nullabilityQuestion`, whose `withNullability(none)` returns the same
//!   object, so the recursion does not end (stack overflow). No caller
//!   normalizes `_`.

use dartr_element::{EId, FnParam, NamedType, Nullability, TypeId, TypeKind, TypeParameterElement};

use crate::type_algebra::get_fresh_type_parameters;
use crate::type_ext::TypeExt;
use crate::type_system::TypeSystem;

/// `NormalizeHelper`.
pub struct NormalizeHelper<'a> {
    pub type_system: TypeSystem<'a>,
    /// `_typeParameters`: the type parameters whose bounds are being
    /// normalized (stops the recursion on recursive bounds).
    type_parameters: Vec<EId<TypeParameterElement>>,
}

impl<'a> NormalizeHelper<'a> {
    pub fn new(type_system: TypeSystem<'a>) -> Self {
        NormalizeHelper {
            type_system,
            type_parameters: Vec::new(),
        }
    }

    /// `normalize(T)`.
    pub fn normalize(&mut self, t: TypeId) -> TypeId {
        self.normalize_(t)
    }

    /// `normalizeFunctionType(T)`.
    pub fn normalize_function_type(&mut self, t: TypeId) -> TypeId {
        let result = self.normalize_(t);
        // Dart: `as FunctionTypeImpl`.
        assert!(
            matches!(self.type_system.ctx.ty(result), TypeKind::Function(_)),
            "normalizeFunctionType: the result is not a function type"
        );
        result
    }

    /// `normalizeInterfaceType(T)`.
    pub fn normalize_interface_type(&mut self, t: TypeId) -> TypeId {
        let result = self.normalize_(t);
        // Dart: `as InterfaceTypeImpl`.
        assert!(
            matches!(self.type_system.ctx.ty(result), TypeKind::Interface { .. }),
            "normalizeInterfaceType: the result is not an interface type"
        );
        result
    }

    /// `_functionType(functionType)`.
    ///
    /// `NORM(R Function<X extends B>(S)) = R1 Function(X extends B1>(S1)`
    ///   * where R1 = NORM(R)
    ///   * and B1 = NORM(B)
    ///   * and S1 = NORM(S)
    fn function_type(&mut self, function_type: TypeId) -> TypeId {
        let ctx = self.type_system.ctx;
        let TypeKind::Function(f) = *ctx.ty(function_type) else {
            panic!("_functionType: not a function type");
        };
        let fresh = get_fresh_type_parameters(&ctx, ctx.list(f.type_params));
        for &type_parameter in &fresh.fresh_type_parameters {
            if let Some(bound) = ctx.type_parameter_bound(type_parameter) {
                let normalized = self.normalize_(bound);
                ctx.get(type_parameter).bound.set(Some(normalized));
            }
        }

        let function_type = fresh.apply_to_function_type(&ctx, function_type);
        let TypeKind::Function(f) = *ctx.ty(function_type) else {
            unreachable!();
        };

        // `e.copyWith(type: _normalize(e.type))`: name, kind and covariance
        // stay.
        let params: Vec<FnParam> = ctx
            .list(f.params)
            .iter()
            .map(|e| FnParam {
                ty: self.normalize_(e.ty),
                ..*e
            })
            .collect();
        let ret = self.normalize_(f.ret);
        // No alias: the Dart constructor call passes none.
        ctx.function_type(
            ctx.list(f.type_params),
            &params,
            ret,
            Nullability::None,
            None,
        )
    }

    /// `_futureOr(T)`: `NORM(FutureOr<T>)`.
    fn future_or(&mut self, t: TypeId) -> TypeId {
        let ts = self.type_system;
        let ctx = ts.ctx;
        // * let S be NORM(T)
        let s = self.normalize_(ctx.type_arguments(t)[0]);
        let s_nullability = ctx.nullability_suffix(s);

        // * if S is a top type then S
        if ts.is_top(s) {
            return s;
        }

        // * if S is Object then S
        if ctx.is_dart_core_object(s) && s_nullability == Nullability::None {
            return s;
        }

        // * if S is Never then Future<Never>
        if is_never_none(&ts, s) {
            return ctx.instantiate_interface(
                ctx.tp.future_element().upcast(),
                &[TypeId::NEVER],
                Nullability::None,
            );
        }

        // * if S is Null then Future<Null>?
        if s_nullability == Nullability::None && ctx.is_dart_core_null(s) {
            return ctx.instantiate_interface(
                ctx.tp.future_element().upcast(),
                &[ts.null_none()],
                Nullability::Question,
            );
        }

        // * else FutureOr<S>
        ctx.instantiate_interface(ctx.tp.future_or_element().upcast(), &[s], Nullability::None)
    }

    /// `_normalize(T)`.
    fn normalize_(&mut self, t: TypeId) -> TypeId {
        let ts = self.type_system;
        let ctx = ts.ctx;
        let t_nullability = ctx.nullability_suffix(t);
        let kind = *ctx.ty(t);

        // NORM(T) = T if T is primitive
        let is_primitive = match kind {
            TypeKind::Dynamic | TypeKind::Invalid | TypeKind::Void => true,
            TypeKind::Never(n) => n == Nullability::None,
            TypeKind::Interface { args, .. } => {
                t_nullability == Nullability::None && ctx.list(args).is_empty()
            }
            // Not in Dart, see the module documentation.
            TypeKind::Unknown => true,
            _ => false,
        };
        if is_primitive {
            return t;
        }

        // NORM(FutureOr<T>)
        if t_nullability == Nullability::None
            && matches!(kind, TypeKind::Interface { .. })
            && ctx.is_dart_async_future_or(t)
        {
            return self.future_or(t);
        }

        // NORM(T?)
        if t_nullability == Nullability::Question {
            return self.nullability_question(t);
        }

        debug_assert_eq!(t_nullability, Nullability::None);

        match kind {
            // NORM(X extends T)
            // NORM(X & T)
            TypeKind::TypeParameter { .. } => self.type_parameter_type(t),

            // NORM(C<T0, ..., Tn>) = C<R0, ..., Rn> where Ri is NORM(Ti)
            TypeKind::Interface { element, args, .. } => {
                let args: Vec<TypeId> =
                    ctx.list(args).iter().map(|&a| self.normalize_(a)).collect();
                ctx.instantiate_interface(element, &args, Nullability::None)
            }

            // NORM(Record(T0, ..., Tn)) = Record(R0, ..., Rn) where Ri is NORM(Ti)
            TypeKind::Record {
                positional, named, ..
            } => {
                let positional: Vec<TypeId> = ctx
                    .list(positional)
                    .iter()
                    .map(|&field| self.normalize_(field))
                    .collect();
                let named: Vec<NamedType> = ctx
                    .list(named)
                    .iter()
                    .map(|field| NamedType {
                        name: field.name,
                        ty: self.normalize_(field.ty),
                    })
                    .collect();
                ctx.record_type(&positional, &named, Nullability::None, None)
            }

            // NORM(R Function<X extends B>(S)) = R1 Function(X extends B1>(S1)
            TypeKind::Function(_) => self.function_type(t),

            // Dart: `T as FunctionTypeImpl` fails.
            _ => panic!("_normalize: unexpected type"),
        }
    }

    /// `_nullabilityQuestion(T)`: NORM(T?).
    fn nullability_question(&mut self, t: TypeId) -> TypeId {
        let ts = self.type_system;
        let ctx = ts.ctx;
        // * let S be NORM(T)
        let t_none = ctx.with_nullability(t, Nullability::None);
        let s = self.normalize_(t_none);
        let s_nullability = ctx.nullability_suffix(s);

        // * if S is a top type then S
        if ts.is_top(s) {
            return s;
        }

        // * if S is Never then Null
        if is_never_none(&ts, s) {
            return ts.null_none();
        }

        // * if S is Null then Null
        if s_nullability == Nullability::None && ctx.is_dart_core_null(s) {
            return ts.null_none();
        }

        // * if S is FutureOr<R> and R is nullable then S
        if s_nullability == Nullability::None
            && matches!(ctx.ty(s), TypeKind::Interface { .. })
            && ctx.is_dart_async_future_or(s)
        {
            let r = ctx.type_arguments(s)[0];
            if ts.is_nullable(r) {
                return s;
            }
        }

        // * if S is R? then R?
        // * else S?
        ctx.with_nullability(s, Nullability::Question)
    }

    /// `_typeParameterType(T)`:
    /// NORM(X & T)
    /// NORM(X extends T)
    fn type_parameter_type(&mut self, t: TypeId) -> TypeId {
        let ctx = self.type_system.ctx;
        let TypeKind::TypeParameter {
            param: element,
            promoted_bound,
            ..
        } = *ctx.ty(t)
        else {
            unreachable!();
        };

        // NORM(X & T)
        if let Some(promoted_bound) = promoted_bound {
            // let S be NORM(T)
            let s = self.normalize_(promoted_bound);
            return self.type_parameter_type_promoted(element, s);
        }

        let Some(bound) = ctx.type_parameter_bound(element) else {
            return t;
        };

        // * let S be NORM(T)
        // Dart: `_typeParameters.add(element)` is false when it is present.
        if self.type_parameters.contains(&element) {
            return t;
        }
        self.type_parameters.push(element);
        let s = self.normalize_(bound);
        self.type_parameters.retain(|&p| p != element);

        // * if S is Never then Never
        if is_never_none(&self.type_system, s) {
            return TypeId::NEVER;
        }

        // else X extends T
        t
    }

    /// `_typeParameterType_promoted(X, S)`:
    /// NORM(X & T)
    /// * let S be NORM(T)
    fn type_parameter_type_promoted(&mut self, x: EId<TypeParameterElement>, s: TypeId) -> TypeId {
        let ts = self.type_system;
        let ctx = ts.ctx;

        // * if S is Never then Never
        if is_never_none(&ts, s) {
            return TypeId::NEVER;
        }

        // * if S is a top type then X
        if ts.is_top(s) {
            return ctx.type_parameter_type(x, Nullability::None);
        }

        // * if S is X then X
        // Dart: `S.element == X` (elements compare by identity).
        if let TypeKind::TypeParameter {
            param,
            nullability: Nullability::None,
            ..
        } = *ctx.ty(s)
            && param == x
        {
            return ctx.type_parameter_type(x, Nullability::None);
        }

        // * if S is Object and NORM(B) is Object where B is the bound of X then X
        if ctx.nullability_suffix(s) == Nullability::None
            && ctx.is_dart_core_object(s)
            && let Some(b) = ctx.type_parameter_bound(x)
        {
            let b_norm = self.normalize_(b);
            if ctx.nullability_suffix(b_norm) == Nullability::None
                && ctx.is_dart_core_object(b_norm)
            {
                return ctx.type_parameter_type(x, Nullability::None);
            }
        }

        // * else X & S
        ctx.promoted_type_parameter_type(x, Nullability::None, Some(s))
    }
}

/// `S is NeverTypeImpl && S.nullabilitySuffix == NullabilitySuffix.none`.
fn is_never_none(ts: &TypeSystem<'_>, s: TypeId) -> bool {
    matches!(ts.ctx.ty(s), TypeKind::Never(Nullability::None))
}
