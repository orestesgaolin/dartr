// Dart source: pkg/linter/lib/src/rules/use_setters_to_change_properties.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElemRef, Tag, TypeKind};
use dartr_typesystem::member;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::MethodDeclaration,
        "use_setters_to_change_properties",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let n = &c.ast[Id::<MethodDeclaration>::from_raw(node)];
    let returns_void = n
        .return_type
        .and_then(|t| annotation_type(c, t))
        .is_some_and(|t| matches!(*ctx.ty(t), TypeKind::Void));
    if method_property(c, node).is_some()
        || method_is_override(c, node)
        || n.parameters
            .map(|p| c.ast.list_raw(c.ast[p].parameters).len())
            != Some(1)
        || !returns_void
    {
        return;
    }
    let check_expression = |expression: NodeId, out: &mut Vec<Diagnostic>| {
        let Some(assignment) = c.ast.cast::<AssignmentExpression>(expression) else {
            return;
        };
        let a = &c.ast[assignment];
        if lexeme(c, a.operator) != "=" {
            return;
        }
        let left = c
            .resolved
            .unwrap()
            .tables
            .write_element
            .get(expression)
            .and_then(|&e| c.canonical_element2(e));
        let right = canonical_element(c, a.right_hand_side.raw()).map(|e| base(c, e));
        let parameter = c.declared_element(node).and_then(|e| {
            member::formal_parameters(&ctx, ElemRef::Base(e))
                .first()
                .map(|&p| base(c, p))
        });
        if right.is_some() && right == parameter && left.is_some_and(|l| l.tag() == Tag::Field) {
            c.report_token(out, &diag::USE_SETTERS_TO_CHANGE_PROPERTIES, n.name, &[]);
        }
    };
    let body = n.body.raw();
    if let Some(block) = c.ast.cast::<BlockFunctionBody>(body) {
        let statements = c.ast.list_raw(c.ast[c.ast[block].block].statements);
        if statements.len() == 1
            && let Some(statement) = c.ast.cast::<ExpressionStatement>(statements[0])
        {
            check_expression(c.ast[statement].expression.raw(), out);
        }
    } else if let Some(e) = c.ast.cast::<ExpressionFunctionBody>(body) {
        check_expression(c.ast[e].expression.raw(), out);
    }
}
