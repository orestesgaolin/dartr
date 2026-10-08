// Dart source: pkg/analyzer/lib/src/dart/constant/has_invalid_type.dart
// (and RecursiveTypeVisitor of pkg/analyzer/lib/src/dart/element/type_visitor.dart)

//! [`has_invalid_type`].

use dartr_element::{Ctx, TypeId, TypeKind};

/// Return `true` if the [ty] has an `InvalidType`.
pub fn has_invalid_type(ctx: &Ctx<'_>, ty: TypeId) -> bool {
    let mut result = false;
    visit(ctx, ty, &mut result);
    result
}

/// `_InvalidTypeVisitor` (a `RecursiveTypeVisitor` with
/// `includeTypeAliasArguments: false` that overrides `visitDartType`, which
/// the recursive visitor calls only for leaf types). Returns `false` to stop.
fn visit(ctx: &Ctx<'_>, ty: TypeId, result: &mut bool) -> bool {
    match *ctx.ty(ty) {
        TypeKind::Invalid => {
            *result = true;
            false
        }
        TypeKind::Dynamic | TypeKind::Void | TypeKind::Unknown | TypeKind::Never(_) => true,
        // `visitTypeParameterType` does not visit the bound.
        TypeKind::TypeParameter { .. } => true,
        TypeKind::Interface { args, .. } => ctx.list(args).iter().all(|&a| visit(ctx, a, result)),
        TypeKind::Function(f) => {
            if !visit(ctx, f.ret, result) {
                return false;
            }
            for &tp in ctx.list(f.type_params) {
                if let Some(bound) = ctx.get(tp).bound.get()
                    && !visit(ctx, bound, result)
                {
                    return false;
                }
            }
            ctx.list(f.params).iter().all(|p| visit(ctx, p.ty, result))
        }
        TypeKind::Record {
            positional, named, ..
        } => {
            ctx.list(positional).iter().all(|&t| visit(ctx, t, result))
                && ctx.list(named).iter().all(|n| visit(ctx, n.ty, result))
        }
    }
}
