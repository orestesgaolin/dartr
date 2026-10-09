// Dart source: pkg/analyzer/lib/src/error/type_arguments_verifier.dart

//! STUB (wd-errors): the `TypeArgumentsVerifier` API that `ErrorVerifier` calls
//! (`generated/error_verifier.dart`). The functions report no diagnostics;
//! the error/* port (branch `wd-errors`) replaces this file.

use dartr_ast::*;

use crate::error_verifier::ErrorVerifier;

macro_rules! checks {
    ($($name:ident: $kind:ident),* $(,)?) => {
        $(
            /// Dart `TypeArgumentsVerifier.checkX(node)`.
            pub fn $name(ev: &mut ErrorVerifier<'_>, node: Id<$kind>) {
                let _ = (ev, node);
            }
        )*
    };
}

checks! {
    check_constructor_reference: ConstructorReference,
    check_enum_constant_declaration: EnumConstantDeclaration,
    check_function_expression_invocation: FunctionExpressionInvocation,
    check_function_reference: FunctionReference,
    check_list_literal: ListLiteral,
    check_map_literal: SetOrMapLiteral,
    check_method_invocation: MethodInvocation,
    check_named_type: NamedType,
    check_set_literal: SetOrMapLiteral,
}
