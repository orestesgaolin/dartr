// Dart source: pkg/analyzer/lib/src/error/immutable_verifier.dart

//! `ImmutableVerifier`: a class or mixin that is, or inherits from, a
//! declaration annotated with `@immutable` must not have non-final instance
//! fields (`must_be_immutable`).

use dartr_ast::NodeId;
use dartr_diagnostics::diag;
use dartr_element::{Ctx, EId, ElementId, FragmentFlags, InterfaceElement};
use dartr_syntax::TokenId;
use dartr_typesystem::{TypeExt, member};
use indexmap::IndexSet;

use super::support::{declared_element, token_range};
use super::{UnitVerifier, VerifierHost};
use crate::element_ext::{first_fragment_flags, is_final};
use crate::element_metadata::{UnitAst, element_has, flags};

/// Dart `ImmutableVerifier.checkDeclaration(node, nameToken:)` (a class,
/// class type alias or mixin declaration): reports `must_be_immutable` at
/// [name_token] if the declared element is or inherits `@immutable` and has
/// declared or inherited non-final instance fields.
pub fn check_declaration(v: &mut UnitVerifier<'_>, node: NodeId, name_token: TokenId) {
    let ctx = v.ctx;
    let Some(element) =
        declared_element(&ctx, v.tables, node).and_then(|e| e.cast::<InterfaceElement>())
    else {
        return;
    };
    let unit = Some(UnitAst {
        ast: v.ast,
        tables: v.tables,
    });
    if !is_or_inherits_immutable(&ctx, element, &mut IndexSet::new(), unit) {
        return;
    }
    let names =
        declared_and_inherited_non_final_instance_fields(&ctx, element, &mut IndexSet::new());
    if !names.is_empty() {
        let range = token_range(v.ast, name_token);
        v.report(diag::must_be_immutable(&names.join(", ")).at_offset(range.0 as usize, range.1 as usize));
    }
}

/// Dart `_declaredAndInheritedNonFinalInstanceFields(element, visited)`.
fn declared_and_inherited_non_final_instance_fields(
    ctx: &Ctx<'_>,
    element: EId<InterfaceElement>,
    visited: &mut IndexSet<EId<InterfaceElement>>,
) -> Vec<String> {
    if !visited.insert(element) {
        // Already checked `element`.
        return Vec::new();
    }
    let mut result = non_final_instance_field_names(ctx, element);
    for &mixin in ctx.element_mixins(element) {
        if let Some(m) = ctx.interface_element(mixin) {
            result.extend(non_final_instance_field_names(ctx, m));
        }
    }
    if let Some(supertype) = ctx.element_supertype(element)
        && let Some(s) = ctx.interface_element(supertype)
    {
        result.extend(declared_and_inherited_non_final_instance_fields(ctx, s, visited));
    }
    result
}

/// Dart `_isOrInheritsImmutable(element, visited)`.
fn is_or_inherits_immutable(
    ctx: &Ctx<'_>,
    element: EId<InterfaceElement>,
    visited: &mut IndexSet<EId<InterfaceElement>>,
    unit: Option<UnitAst<'_>>,
) -> bool {
    if visited.insert(element) {
        if element_has(ctx, element.raw(), flags::IMMUTABLE, unit) {
            return true;
        }
        for &t in ctx.element_mixins(element) {
            if let Some(m) = ctx.interface_element(t)
                && is_or_inherits_immutable(ctx, m, visited, unit)
            {
                return true;
            }
        }
        for &t in ctx.element_interfaces(element) {
            if let Some(i) = ctx.interface_element(t)
                && is_or_inherits_immutable(ctx, i, visited, unit)
            {
                return true;
            }
        }
        if let Some(supertype) = ctx.element_supertype(element)
            && let Some(s) = ctx.interface_element(supertype)
        {
            return is_or_inherits_immutable(ctx, s, visited, unit);
        }
    }
    false
}

/// Dart `InterfaceElement.nonFinalInstanceFieldNames` (`Class.field`).
fn non_final_instance_field_names(ctx: &Ctx<'_>, element: EId<InterfaceElement>) -> Vec<String> {
    let class_name = ctx.element_name(element.raw()).unwrap_or("");
    ctx.instance(element.upcast())
        .fields
        .iter()
        .map(|f| f.raw())
        .filter(|&f: &ElementId| {
            !member::is_static(ctx, f.into())
                && !is_final(ctx, f)
                && !first_fragment_flags(ctx, f)
                    .contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER)
        })
        .map(|f| format!("{class_name}.{}", ctx.element_name(f).unwrap_or("")))
        .collect()
}
