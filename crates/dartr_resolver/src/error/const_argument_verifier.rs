// Dart source: pkg/analyzer/lib/src/error/const_argument_verifier.dart

//! STUB (wd-errors): the `ConstArgumentsVerifier` API that `ErrorVerifier` calls
//! (`generated/error_verifier.dart`). The functions report no diagnostics;
//! the error/* port (branch `wd-errors`) replaces this file.

use dartr_ast::*;

use crate::error_verifier::ErrorVerifier;

macro_rules! visits {
    ($($name:ident: $kind:ident),* $(,)?) => {
        $(
            /// Dart `ConstArgumentsVerifier.visitX(node)`.
            pub fn $name(ev: &mut ErrorVerifier<'_>, node: Id<$kind>) {
                let _ = (ev, node);
            }
        )*
    };
}

visits! {
    visit_anonymous_method_invocation: AnonymousMethodInvocation,
    visit_assignment_expression: AssignmentExpression,
    visit_binary_expression: BinaryExpression,
    visit_constructor_reference: ConstructorReference,
    visit_function_expression_invocation: FunctionExpressionInvocation,
    visit_function_reference: FunctionReference,
    visit_instance_creation_expression: InstanceCreationExpression,
    visit_method_invocation: MethodInvocation,
    visit_prefixed_identifier: PrefixedIdentifier,
    visit_property_access: PropertyAccess,
    visit_redirecting_constructor_invocation: RedirectingConstructorInvocation,
    visit_simple_identifier: SimpleIdentifier,
    visit_super_constructor_invocation: SuperConstructorInvocation,
}
