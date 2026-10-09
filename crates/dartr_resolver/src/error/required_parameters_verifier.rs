// Dart source: pkg/analyzer/lib/src/error/required_parameters_verifier.dart

//! STUB (wd-errors): the `RequiredParametersVerifier` API that `ErrorVerifier` calls
//! (`generated/error_verifier.dart`). The functions report no diagnostics;
//! the error/* port (branch `wd-errors`) replaces this file.

use dartr_ast::*;
use dartr_element::ElementId;

use crate::error_verifier::ErrorVerifier;

macro_rules! visits {
    ($($name:ident: $kind:ident),* $(,)?) => {
        $(
            /// Dart `RequiredParametersVerifier.visitX(node)`.
            pub fn $name(ev: &mut ErrorVerifier<'_>, node: Id<$kind>) {
                let _ = (ev, node);
            }
        )*
    };
}

visits! {
    visit_annotation: Annotation,
    visit_dot_shorthand_constructor_invocation: DotShorthandConstructorInvocation,
    visit_dot_shorthand_invocation: DotShorthandInvocation,
    visit_enum_constant_declaration: EnumConstantDeclaration,
    visit_function_expression_invocation: FunctionExpressionInvocation,
    visit_instance_creation_expression: InstanceCreationExpression,
    visit_method_invocation: MethodInvocation,
    visit_redirecting_constructor_invocation: RedirectingConstructorInvocation,
}

/// Dart `RequiredParametersVerifier.visitSuperConstructorInvocation(node,
/// enclosingConstructor:)`.
pub fn visit_super_constructor_invocation(
    ev: &mut ErrorVerifier<'_>,
    node: Id<SuperConstructorInvocation>,
    enclosing_constructor: Option<ElementId>,
) {
    let _ = (ev, node, enclosing_constructor);
}
