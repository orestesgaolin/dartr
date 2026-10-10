// Dart source: pkg/linter/lib/src/rules/tighten_type_of_initializing_formals.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{FragmentFlags, Tag};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::ConstructorDeclaration,
        "tighten_type_of_initializing_formals",
        check,
    );
    r.add(
        NodeKind::PrimaryConstructorDeclaration,
        "tighten_type_of_initializing_formals",
        check,
    );
}

/// Dart `PrimaryConstructorDeclaration.body` (the `this : ...` body of the
/// enclosing type declaration).
pub(crate) fn primary_constructor_body(
    c: &LinterContext<'_>,
    node: NodeId,
) -> Option<Id<PrimaryConstructorBody>> {
    let declaration = c.ast.parent(node)?;
    super::sort_unnamed_constructors_first::body_members(c, declaration)
        .into_iter()
        .find_map(|m| c.ast.cast::<PrimaryConstructorBody>(m))
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let (parameters, initializers) = match kind(c, node) {
        NodeKind::ConstructorDeclaration => {
            let n = &c.ast[Id::<ConstructorDeclaration>::from_raw(node)];
            (
                parameters(c, Some(n.parameters)),
                c.ast.list_raw(n.initializers).to_vec(),
            )
        }
        _ => {
            let Some(body) = primary_constructor_body(c, node) else {
                return;
            };
            let n = &c.ast[Id::<PrimaryConstructorDeclaration>::from_raw(node)];
            (
                parameters(c, Some(n.formal_parameters)),
                c.ast.list_raw(c.ast[body].initializers).to_vec(),
            )
        }
    };
    let Some(ts) = c.type_system() else { return };
    for initializer in initializers {
        let Some(assert) = c.ast.cast::<AssertInitializer>(initializer) else {
            continue;
        };
        let Some(condition) = c
            .ast
            .cast::<BinaryExpression>(c.ast[assert].condition.raw())
        else {
            continue;
        };
        let b = &c.ast[condition];
        if lexeme(c, b.operator) != "!=" {
            continue;
        }
        let operand = if kind(c, b.right_operand) == NodeKind::NullLiteral {
            b.left_operand.raw()
        } else if kind(c, b.left_operand) == NodeKind::NullLiteral {
            b.right_operand.raw()
        } else {
            continue;
        };
        if !Identifier::test(kind(c, operand)) {
            continue;
        }
        if !c.static_type(operand).is_some_and(|t| ts.is_nullable(t)) {
            continue;
        }
        // Dart `_check`.
        let Some(element) = c.element(operand).map(|e| base(c, e)) else {
            continue;
        };
        let report = match element.tag() {
            Tag::FieldFormalParameter => !flags(c, element)
                .contains(FragmentFlags::FIELD_FORMAL_PARAMETER_FRAGMENT_IS_DECLARING),
            Tag::SuperFormalParameter => true,
            _ => false,
        };
        if report
            && let Some(&parameter) = parameters
                .iter()
                .find(|&&p| c.declared_element(p) == Some(element))
        {
            c.report_node(
                out,
                &diag::TIGHTEN_TYPE_OF_INITIALIZING_FORMALS,
                parameter,
                &[],
            );
        }
    }
}
