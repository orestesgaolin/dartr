// Dart source: pkg/analyzer/lib/src/dart/constant/has_type_parameter_reference.dart
// (and RecursiveTypeVisitor of pkg/analyzer/lib/src/dart/element/type_visitor.dart)

//! [`has_type_parameter_reference`].

use dartr_element::{Ctx, TypeId, TypeKind};

/// Return `true` if the [ty] has a type parameter reference.
pub fn has_type_parameter_reference(ctx: &Ctx<'_>, ty: TypeId) -> bool {
    let mut result = false;
    visit(ctx, ty, &mut result);
    result
}

/// `_ReferencesTypeParameterVisitor` (a `RecursiveTypeVisitor` with
/// `includeTypeAliasArguments: false`). Returns `false` to stop.
fn visit(ctx: &Ctx<'_>, ty: TypeId, result: &mut bool) -> bool {
    match *ctx.ty(ty) {
        TypeKind::TypeParameter { .. } => {
            *result = true;
            // Stop visiting at this point.
            false
        }
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
        TypeKind::Dynamic
        | TypeKind::Void
        | TypeKind::Invalid
        | TypeKind::Unknown
        | TypeKind::Never(_) => true,
    }
}
