//! Syntactic stand-ins for facts that the Dart server reads from the
//! resolved AST. They are used until `dartr_driver` gives resolved units to
//! the server; each one names the Dart code that it replaces.
//!
//! - Instance creation without `new`/`const`: the parser makes a
//!   `MethodInvocation` (`Foo()`, `Foo.named()`, `p.Foo()`), and the
//!   resolver rewrites it to an `InstanceCreationExpression` when `Foo` is a
//!   class. Here: when the type name starts with an upper case letter (Dart
//!   naming rules).
//! - Test functions (`isTest`/`isTestGroup` in `computer_outline.dart`,
//!   `hasIsTest`/`hasIsTestGroup` in `computer_closing_labels.dart`): a call
//!   of `test`, `group` or `testWidgets` without a target, in a unit that
//!   imports `package:test`, `package:test_api`, `package:test_core` or
//!   `package:flutter_test`.
//! - `@deprecated` and `@Deprecated(...)` annotations (`_hasDeprecated`):
//!   by name, without the element check.

use dartr_ast::*;

/// A constructor call seen in a `MethodInvocation`: the type name as Dart
/// `NamedType.qualifiedName` (`p.Foo`) and the constructor name.
pub struct ImplicitCreation {
    pub type_name: String,
    pub constructor_name: Option<String>,
}

fn is_type_name(name: &str) -> bool {
    name.trim_start_matches(['_', '$'])
        .chars()
        .next()
        .is_some_and(|c| c.is_uppercase())
}

/// The constructor call of [node], if the resolver would make it an
/// `InstanceCreationExpression` (see the module documentation).
pub fn implicit_creation(ast: &Ast, node: Id<MethodInvocation>) -> Option<ImplicitCreation> {
    let n = &ast[node];
    let method = ast.tokens.lexeme(ast[n.method_name].token).to_string();
    match n.target {
        None => is_type_name(&method).then_some(ImplicitCreation {
            type_name: method,
            constructor_name: None,
        }),
        Some(target) => {
            // Only `a.b(...)` with a `.` operator.
            if n.operator.map(|o| ast.tokens.lexeme(o)) != Some(".") {
                return None;
            }
            if let Some(id) = ast.cast::<SimpleIdentifier>(target) {
                let target_name = ast.tokens.lexeme(ast[id].token).to_string();
                if is_type_name(&target_name) {
                    // `Foo.named()`.
                    return Some(ImplicitCreation {
                        type_name: target_name,
                        constructor_name: Some(method),
                    });
                }
                if is_type_name(&method) {
                    // `prefix.Foo()`.
                    return Some(ImplicitCreation {
                        type_name: format!("{target_name}.{method}"),
                        constructor_name: None,
                    });
                }
                return None;
            }
            if let Some(p) = ast.cast::<PrefixedIdentifier>(target) {
                // `prefix.Foo.named()`.
                let prefix = ast.tokens.lexeme(ast[ast[p].prefix].token);
                let type_name = ast.tokens.lexeme(ast[ast[p].identifier].token);
                if is_type_name(type_name) {
                    return Some(ImplicitCreation {
                        type_name: format!("{prefix}.{type_name}"),
                        constructor_name: Some(method),
                    });
                }
            }
            None
        }
    }
}

/// The kind of a test function call.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TestCall {
    Group,
    Test,
}

/// Whether the unit imports a test framework (see the module documentation).
pub fn imports_test_framework(ast: &Ast, unit: Id<CompilationUnit>) -> bool {
    ast.list(ast[unit].directives).iter().any(|&d| {
        let Some(import) = ast.cast::<ImportDirective>(d) else {
            return false;
        };
        let uri = ast[import].uri;
        let Some(literal) = ast.cast::<SimpleStringLiteral>(uri) else {
            return false;
        };
        let lexeme = ast.tokens.lexeme(ast[literal].literal);
        let value = lexeme.trim_matches(|c| c == '\'' || c == '"');
        [
            "package:test/",
            "package:test_api/",
            "package:test_core/",
            "package:flutter_test/",
        ]
        .iter()
        .any(|p| value.starts_with(p))
    })
}

/// The test function kind of [node] when [has_test_import].
pub fn test_call(ast: &Ast, node: Id<MethodInvocation>, has_test_import: bool) -> Option<TestCall> {
    if !has_test_import || ast[node].target.is_some() {
        return None;
    }
    match ast.tokens.lexeme(ast[ast[node].method_name].token) {
        "group" => Some(TestCall::Group),
        "test" | "testWidgets" => Some(TestCall::Test),
        _ => None,
    }
}

/// Dart `_hasDeprecated` (`computer_outline.dart`): an annotation that
/// marks a declaration deprecated for use.
pub fn has_deprecated(ast: &Ast, annotations: NodeList<Annotation>) -> bool {
    ast.list(annotations).iter().any(|&a| {
        let n = &ast[a];
        let name = ast
            .cast::<SimpleIdentifier>(n.name)
            .map(|id| ast.tokens.lexeme(ast[id].token));
        match name {
            Some("deprecated") => n.arguments.is_none(),
            // `@Deprecated('...')`; the named constructors (`.extend()`,
            // `.implement()`, ...) are other deprecation kinds.
            Some("Deprecated") => n.constructor_name.is_none() && n.arguments.is_some(),
            _ => false,
        }
    })
}
