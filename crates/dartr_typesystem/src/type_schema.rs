// Dart source: pkg/analyzer/lib/src/dart/element/type_schema.dart

//! `UnknownInferredType` helpers.
//!
//! The type `_` itself is `TypeKind::Unknown` / `TypeId::UNKNOWN` in
//! `dartr_element` (a type that is being inferred but is not currently
//! known; it appears only in type schemas). Here are its static methods
//! `isKnown` and `isUnknown`.

use dartr_element::{Ctx, TypeId, TypeKind};

use crate::type_ext::TypeExt;

/// `UnknownInferredType.isKnown(type)`: given a type T, return true if it
/// does not have an unknown type `_`.
pub fn is_known(ctx: &Ctx<'_>, t: TypeId) -> bool {
    !is_unknown(ctx, t)
}

/// `UnknownInferredType.isUnknown(type)`: given a type T, return true if it
/// has an unknown type `_`.
pub fn is_unknown(ctx: &Ctx<'_>, t: TypeId) -> bool {
    // Dart: identical(type, UnknownInferredType.instance)
    if t == TypeId::UNKNOWN {
        return true;
    }

    match *ctx.ty(t) {
        TypeKind::Interface { args, .. } => ctx.list(args).iter().any(|&a| is_unknown(ctx, a)),
        TypeKind::Function(f) => {
            if is_unknown(ctx, f.ret) {
                return true;
            }
            for &type_parameter in ctx.list(f.type_params) {
                if let Some(bound) = ctx.type_parameter_bound(type_parameter)
                    && is_unknown(ctx, bound)
                {
                    return true;
                }
            }
            ctx.list(f.params).iter().any(|p| is_unknown(ctx, p.ty))
        }
        TypeKind::Record {
            positional, named, ..
        } => {
            ctx.list(positional).iter().any(|&f| is_unknown(ctx, f))
                || ctx.list(named).iter().any(|f| is_unknown(ctx, f.ty))
        }
        _ => false,
    }
}
