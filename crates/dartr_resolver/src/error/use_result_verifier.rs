// Dart source: pkg/analyzer/lib/src/error/use_result_verifier.dart

//! STUB (wd-errors): the `UseResultVerifier` API that `ErrorVerifier` calls
//! (`generated/error_verifier.dart`). The functions report no diagnostics;
//! the error/* port (branch `wd-errors`) replaces this file.

use dartr_ast::*;

use crate::error_verifier::ErrorVerifier;

macro_rules! checks {
    ($($name:ident: $kind:ident),* $(,)?) => {
        $(
            /// Dart `UseResultVerifier.checkX(node)`.
            pub fn $name(ev: &mut ErrorVerifier<'_>, node: Id<$kind>) {
                let _ = (ev, node);
            }
        )*
    };
}

checks! {
    check_function_expression_invocation: FunctionExpressionInvocation,
    check_instance_creation_expression: InstanceCreationExpression,
    check_method_invocation: MethodInvocation,
    check_property_access: PropertyAccess,
    check_simple_identifier: SimpleIdentifier,
}
