// Dart source: pkg/analyzer/lib/src/error/assignment_verifier.dart

//! `AssignmentVerifier`: verifies that the element that the left-hand side
//! of an assignment (an [`AssignmentExpression`], or a prefix or postfix
//! increment) resolves to is writable. The property element resolver calls
//! [`verify`] (through `assignment_expression_resolver::verify_assignment`).
//!
//! [`AssignmentExpression`]: dartr_ast::AssignmentExpression

use dartr_ast::{Id, SimpleIdentifier};
use dartr_diagnostics::diag;
use dartr_element::diagnostics::type_arg;
use dartr_element::{ElemRef, FragmentFlags, Tag, TypeId, VariableElement};
use dartr_typesystem::{TypeExt, member};

use super::VerifierHost;
use crate::ast_ext;
use crate::element_ext;

/// Dart `AssignmentVerifier.verify(node:, requested:, recovery:,
/// receiverType:)`: we resolved [node] and found that it references the
/// [requested] element. Verifies that this element is actually writable.
///
/// If the [requested] element is `None`, we might have the [recovery]
/// element, which is definitely not a valid write target. We want to
/// report a good error about this.
///
/// When the [receiver_type] is not `None`, we report `undefinedSetter`
/// instead of a more generic `undefinedIdentifier`.
pub fn verify<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<SimpleIdentifier>,
    requested: Option<ElemRef>,
    recovery: Option<ElemRef>,
    receiver_type: Option<TypeId>,
) {
    let ctx = host.ctx();
    if let Some(requested) = requested {
        let base = member::base_element(&ctx, requested);
        if base.is::<VariableElement>() && element_ext::is_const(&ctx, base) {
            let d = host.at(diag::assignment_to_const(), node);
            host.report(d);
        }
        return;
    }

    let recovery_base = recovery.map(|e| member::base_element(&ctx, e));
    match recovery_base.map(|e| e.tag()) {
        // Dart `DynamicElementImpl`, `InterfaceElement`, `TypeAliasElement`,
        // `TypeParameterElement`.
        Some(
            Tag::Dynamic
            | Tag::Class
            | Tag::Enum
            | Tag::Mixin
            | Tag::ExtensionType
            | Tag::TypeAlias
            | Tag::TypeParameter,
        ) => {
            let d = host.at(diag::assignment_to_type(), node);
            host.report(d);
        }
        Some(Tag::LocalFunction | Tag::TopLevelFunction) => {
            let d = host.at(diag::assignment_to_function(), node);
            host.report(d);
        }
        Some(Tag::Method) => {
            let d = host.at(diag::assignment_to_method(), node);
            host.report(d);
        }
        Some(Tag::Prefix) => {
            if let Some(prefix_name) = recovery_base.and_then(|e| ctx.element_name(e)) {
                let d = host.at(
                    diag::prefix_identifier_not_followed_by_dot(prefix_name),
                    node,
                );
                host.report(d);
            }
        }
        Some(Tag::Getter) => {
            let Some(variable) = recovery.and_then(|r| member::variable(&ctx, r)) else {
                return;
            };
            let variable = member::base_element(&ctx, variable);
            let Some(variable_name) = ctx.element_name(variable) else {
                return;
            };

            if element_ext::is_const(&ctx, variable) {
                let d = host.at(diag::assignment_to_const(), node);
                host.report(d);
            } else if variable.tag() == Tag::Field
                && element_ext::first_fragment_flags(&ctx, variable)
                    .contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER)
            {
                // Dart `variable.enclosingElement.displayName`.
                let class_name = ctx
                    .element_data(variable)
                    .and_then(|d| d.enclosing)
                    .and_then(|e| ctx.element_name(e))
                    .unwrap_or("");
                let d = host.at(
                    diag::assignment_to_final_no_setter(variable_name, class_name),
                    node,
                );
                host.report(d);
            } else {
                let d = host.at(diag::assignment_to_final(variable_name), node);
                host.report(d);
            }
        }
        Some(Tag::MultiplyDefined) => {
            // Will be reported in ErrorVerifier.
        }
        _ => {
            let ast = host.ast();
            if ast.tokens.get(ast[node].token).is_synthetic() {
                return;
            }
            let name = ast_ext::identifier_name(ast, node).to_string();
            let d = match receiver_type {
                Some(receiver_type) => diag::undefined_setter(&name, type_arg(&ctx, receiver_type)),
                None => diag::undefined_identifier(&name),
            };
            let d = host.at(d, node);
            host.report(d);
        }
    }
}
