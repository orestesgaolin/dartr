// Dart source: pkg/linter/lib/src/rules/use_full_hex_values_for_flutter_colors.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::InstanceCreationExpression,
        "use_full_hex_values_for_flutter_colors",
        check,
    );
}

/// Dart `ConstructorElementExtension.isSameAs(uri:, className:, constructorName:)`.
pub(crate) fn constructor_is_same_as(
    c: &LinterContext<'_>,
    constructor: dartr_element::ElementId,
    library: &str,
    class_name: &str,
    constructor_name: &str,
) -> bool {
    library_name(c, constructor) == Some(library)
        && enclosing(c, constructor).and_then(|e| name(c, e)) == Some(class_name)
        && name(c, constructor) == Some(constructor_name)
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<InstanceCreationExpression>::from_raw(node)];
    let Some(element) = c.element(n.constructor_name) else {
        return;
    };
    if !constructor_is_same_as(c, base(c, element), "dart.ui", "Color", "new") {
        return;
    }
    let Some(&argument) = c.ast.list_raw(c.ast[n.argument_list].arguments).first() else {
        return;
    };
    if let Some(literal) = c.ast.cast::<IntegerLiteral>(argument) {
        let value = lexeme(c, c.ast[literal].literal).to_lowercase().replace('_', "");
        if !value.starts_with("0x") || value.chars().count() != 10 {
            c.report_node(out, &diag::USE_FULL_HEX_VALUES_FOR_FLUTTER_COLORS, argument, &[]);
        }
    }
}
