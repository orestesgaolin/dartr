// Dart source: pkg/linter/lib/src/rules/invalid_runtime_check_with_js_interop_types.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, DiagnosticCode, diag};
use dartr_element::{ExtensionTypeElement, TypeId, TypeKind};
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::AsExpression,
        "invalid_runtime_check_with_js_interop_types",
        check,
    );
    registry.add(
        NodeKind::CatchClause,
        "invalid_runtime_check_with_js_interop_types",
        check,
    );
    registry.add(
        NodeKind::IsExpression,
        "invalid_runtime_check_with_js_interop_types",
        check,
    );
}
fn is_direct_interop(context: &LinterContext<'_>, ty: TypeId) -> bool {
    let Some(resolved) = context.resolved else {
        return false;
    };
    match *resolved.ctx.ty(ty) {
        TypeKind::Interface { element, .. } if element.raw().is::<ExtensionTypeElement>() => {
            resolved.ctx.element_library_uri(element.raw()) == Some("dart:js_interop")
        }
        TypeKind::TypeParameter { param, .. } => resolved
            .ctx
            .get(param)
            .bound
            .get()
            .is_some_and(|b| is_direct_interop(context, b)),
        _ => false,
    }
}
fn invalid_test(
    context: &LinterContext<'_>,
    left: TypeId,
    right: TypeId,
    is_check: bool,
) -> Option<&'static DiagnosticCode> {
    let ts = context.type_system()?;
    let resolved = context.resolved?;
    let left_js = is_direct_interop(context, left);
    let right_js = is_direct_interop(context, right);
    if !left_js && !right_js {
        return None;
    }
    let left_erased = ts.promote_to_non_null(ts.extension_type_erasure(left));
    let right_erased = ts.promote_to_non_null(ts.extension_type_erasure(right));
    let left_sub = ts.is_subtype_of(left_erased, right_erased);
    let right_sub = ts.is_subtype_of(right_erased, left_erased);
    let left_dynamic = matches!(resolved.ctx.ty(left_erased), TypeKind::Dynamic);
    let right_dynamic = matches!(resolved.ctx.ty(right_erased), TypeKind::Dynamic);
    if is_check {
        if !left_sub && !right_dynamic {
            Some(if left_js && right_js {
                &diag::INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_JS_IS_INCONSISTENT_JS
            } else if left_js {
                &diag::INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_JS_IS_DART
            } else {
                &diag::INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_DART_IS_JS
            })
        } else {
            None
        }
    } else if !left_sub && !right_sub && !left_dynamic && !right_dynamic {
        Some(if left_js && right_js {
            &diag::INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_JS_AS_INCOMPATIBLE_JS
        } else if left_js {
            &diag::INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_JS_AS_DART
        } else {
            &diag::INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_DART_AS_JS
        })
    } else {
        None
    }
}
fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match context.ast.kind(node) {
        NodeKind::CatchClause => {
            let n = &context.ast[context.ast.cast::<CatchClause>(node).unwrap()];
            let Some(annotation) = n.exception_type else {
                return;
            };
            let Some(ty) = super::helpers::annotation_type(context, annotation.raw()) else {
                return;
            };
            if is_direct_interop(context, ty) {
                let text = super::helpers::type_text(context, ty);
                context.report_node(
                    out,
                    &diag::INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_CATCH_CLAUSE_JS_INTEROP_TYPE,
                    annotation,
                    &[&text],
                );
            }
        }
        NodeKind::AsExpression => {
            let n = &context.ast[context.ast.cast::<AsExpression>(node).unwrap()];
            let (Some(left), Some(right)) = (
                context.static_type(n.expression),
                super::helpers::annotation_type(context, n.type_.raw()),
            ) else {
                return;
            };
            if let Some(code) = invalid_test(context, left, right, false) {
                let l = super::helpers::type_text(context, left);
                let r = super::helpers::type_text(context, right);
                context.report_node(out, code, node, &[&l, &r]);
            }
        }
        NodeKind::IsExpression => {
            let n = &context.ast[context.ast.cast::<IsExpression>(node).unwrap()];
            let (Some(left), Some(right)) = (
                context.static_type(n.expression),
                super::helpers::annotation_type(context, n.type_.raw()),
            ) else {
                return;
            };
            if let Some(code) = invalid_test(context, left, right, true) {
                let l = super::helpers::type_text(context, left);
                let r = super::helpers::type_text(context, right);
                context.report_node(out, code, node, &[&l, &r]);
            }
        }
        _ => {}
    }
}
