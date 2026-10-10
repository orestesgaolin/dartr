// Dart source: pkg/linter/lib/src/rules/unnecessary_parenthesis.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{EId, ElemRef, ExtensionElement, Tag, TypeKind};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::ParenthesizedExpression,
        "unnecessary_parenthesis",
        visit_parenthesized_expression,
    );
}

fn is_record_type(c: &LinterContext<'_>, element: Option<ElemRef>) -> bool {
    let (Some(element), Some(ctx)) = (element, rctx(c)) else {
        return false;
    };
    let ty = dartr_typesystem::member::type_(&ctx, element);
    matches!(*ctx.ty(ty), TypeKind::Record { .. })
}

fn report(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    c.report_node(out, &diag::UNNECESSARY_PARENTHESIS, node, &[]);
}

/// Whether [element] is a member of an extension on a nullable type.
fn is_nullable_extension_member(c: &LinterContext<'_>, element: Option<ElemRef>) -> bool {
    let (Some(element), Some(ctx), Some(ts)) = (element, rctx(c), c.type_system()) else {
        return false;
    };
    let Some(target) = enclosing(c, base(c, element)) else {
        return false;
    };
    if target.tag() != Tag::Extension {
        return false;
    }
    let extended = ctx
        .get(EId::<ExtensionElement>::from_raw(target))
        .extended_type
        .get();
    extended.is_some_and(|t| ts.is_nullable(t))
}

fn visit_parenthesized_expression(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(parent) = c.ast.parent(node) else {
        return;
    };
    let parent_kind = kind(c, parent);
    if matches!(
        parent_kind,
        NodeKind::ConstantPattern | NodeKind::SpreadElement
    ) {
        return;
    }
    let expression = c.ast[Id::<ParenthesizedExpression>::from_raw(node)]
        .expression
        .raw();
    let expression_kind = kind(c, expression);
    let is_record_literal = expression_kind == NodeKind::RecordLiteral;
    if parent_kind == NodeKind::VariableDeclaration
        && is_record_type(c, c.declared_element(parent).map(ElemRef::Base))
        && !is_record_literal
    {
        return;
    }
    if parent_kind == NodeKind::ArgumentList
        && is_record_type(c, c.corresponding_parameter(node))
        && !is_record_literal
    {
        return;
    }
    if parent_kind == NodeKind::NamedArgument
        && is_record_type(
            c,
            c.corresponding_parameter(parent)
                .or_else(|| c.corresponding_parameter(node)),
        )
        && !is_record_literal
    {
        return;
    }

    let is = |field: NodeId| field == node;
    let directly_wrapped = match parent_kind {
        NodeKind::ParenthesizedExpression | NodeKind::InterpolationExpression => true,
        NodeKind::ArgumentList => {
            c.ast
                .list_raw(c.ast[Id::<ArgumentList>::from_raw(parent)].arguments)
                .len()
                == 1
        }
        NodeKind::IfStatement => is(c.ast[Id::<IfStatement>::from_raw(parent)].expression.raw()),
        NodeKind::IfElement => is(c.ast[Id::<IfElement>::from_raw(parent)].expression.raw()),
        NodeKind::WhileStatement => is(c.ast[Id::<WhileStatement>::from_raw(parent)]
            .condition
            .raw()),
        NodeKind::DoStatement => is(c.ast[Id::<DoStatement>::from_raw(parent)].condition.raw()),
        NodeKind::SwitchStatement => is(c.ast[Id::<SwitchStatement>::from_raw(parent)]
            .expression
            .raw()),
        NodeKind::SwitchExpression => is(c.ast[Id::<SwitchExpression>::from_raw(parent)]
            .expression
            .raw()),
        _ => false,
    };
    if directly_wrapped {
        report(c, node, out);
        return;
    }
    if matches!(
        expression_kind,
        NodeKind::ConditionalExpression | NodeKind::TypeLiteral
    ) {
        return;
    }

    if is_one_token(c, expression) || contains_null_aware_invocation_in_chain(c, Some(expression)) {
        if let Some(access) = c.ast.cast::<PropertyAccess>(parent) {
            let name = simple_name(c, c.ast[access].property_name);
            if name == "hashCode" || name == "runtimeType" {
                return;
            }
            if is_nullable_extension_member(c, c.element(c.ast[access].property_name)) {
                return;
            }
        } else if let Some(invocation) = c.ast.cast::<MethodInvocation>(parent) {
            let name = simple_name(c, c.ast[invocation].method_name);
            if name == "noSuchMethod" || name == "toString" {
                return;
            }
            if is_nullable_extension_member(c, c.element(c.ast[invocation].method_name)) {
                return;
            }
        } else if let Some(postfix) = c.ast.cast::<PostfixExpression>(parent)
            && lexeme(c, c.ast[postfix].operator) == "!"
        {
            return;
        } else if let Some(index) = c.ast.cast::<IndexExpression>(expression)
            && c.ast[index].question.is_some()
        {
            if let Some(conditional) = c.ast.cast::<ConditionalExpression>(parent)
                && c.ast[conditional].then_expression.raw() == node
            {
                return;
            } else if let Some(entry) = c.ast.cast::<MapLiteralEntry>(parent)
                && c.ast[entry].key.raw() == node
            {
                return;
            }
        }
        report(c, node, out);
        return;
    }

    if expression_kind == NodeKind::ConstructorReference {
        let keeps = c
            .ast
            .cast::<FunctionExpressionInvocation>(parent)
            .is_some_and(|p| c.ast[p].type_arguments.is_some());
        if !keeps {
            report(c, node, out);
            return;
        }
    }

    if expression_kind == NodeKind::CascadeExpression
        || this_or_ancestor(c, node, |n| {
            Statement::test(kind(c, n)) || kind(c, n) == NodeKind::CascadeExpression
        })
        .is_some_and(|n| kind(c, n) == NodeKind::CascadeExpression)
    {
        return;
    }

    if is_bare_in_constructor_field_initializer(c, node) && contains_function_expression(c, node) {
        return;
    }

    if let Some(binary) = c.ast.cast::<BinaryExpression>(expression)
        && matches!(lexeme(c, c.ast[binary].operator), "==" | "!=")
        && matches!(
            parent_kind,
            NodeKind::AssignmentExpression
                | NodeKind::VariableDeclaration
                | NodeKind::ReturnStatement
                | NodeKind::YieldStatement
                | NodeKind::ConstructorFieldInitializer
        )
    {
        return;
    }

    if parent_kind == NodeKind::ExpressionStatement && expression_kind == NodeKind::SwitchExpression
    {
        return;
    }

    if directly_contains_whitespace(c, Some(expression))
        && !matches!(
            parent_kind,
            NodeKind::AssignmentExpression
                | NodeKind::ConstructorFieldInitializer
                | NodeKind::ExpressionFunctionBody
                | NodeKind::RecordLiteral
                | NodeKind::ReturnStatement
                | NodeKind::VariableDeclaration
                | NodeKind::YieldStatement
        )
        && !is_argument(c, node)
    {
        return;
    }

    if Expression::test(parent_kind) {
        match parent_kind {
            NodeKind::BinaryExpression
            | NodeKind::ConditionalExpression
            | NodeKind::CascadeExpression
            | NodeKind::AsExpression
            | NodeKind::IsExpression => return,
            NodeKind::FunctionExpressionInvocation
                if expression_kind != NodeKind::PrefixedIdentifier =>
            {
                return;
            }
            _ => {}
        }
        let target = if let Some(m) = c.ast.cast::<MethodInvocation>(parent) {
            Some(c.ast[m].target.map(|t| t.raw()))
        } else {
            c.ast
                .cast::<PropertyAccess>(parent)
                .map(|p| c.ast[p].target.map(|t| t.raw()))
        };
        if let Some(target) = target {
            let target_is_node = target == Some(node);
            if c.ast
                .parent(parent)
                .is_some_and(|g| kind(c, g) == NodeKind::PrefixExpression)
                && target_is_node
                && directly_contains_whitespace(c, Some(expression))
            {
                return;
            }
            if matches!(
                expression_kind,
                NodeKind::PostfixExpression | NodeKind::PrefixExpression
            ) && target_is_node
            {
                return;
            }
        }
        if would_be_parsed_as_statement_block(c, node) {
            return;
        }
    }
    report(c, node, out);
}

/// Dart `Expression.isOneToken`.
fn is_one_token(c: &LinterContext<'_>, node: NodeId) -> bool {
    let k = kind(c, node);
    k == NodeKind::SimpleIdentifier
        || StringLiteral::test(k)
        || matches!(
            k,
            NodeKind::IntegerLiteral
                | NodeKind::DoubleLiteral
                | NodeKind::NullLiteral
                | NodeKind::BooleanLiteral
        )
}

/// Dart `Expression.isArgument`.
fn is_argument(c: &LinterContext<'_>, node: NodeId) -> bool {
    let Some(parent) = c.ast.parent(node) else {
        return false;
    };
    kind(c, parent) == NodeKind::ArgumentList
        || (kind(c, parent) == NodeKind::NamedArgument
            && c.ast
                .parent(parent)
                .is_some_and(|g| kind(c, g) == NodeKind::ArgumentList))
}

/// Dart `Expression.containsNullAwareInvocationInChain`.
fn contains_null_aware_invocation_in_chain(c: &LinterContext<'_>, node: Option<NodeId>) -> bool {
    let Some(node) = node else { return false };
    if let Some(n) = c.ast.cast::<PropertyAccess>(node) {
        lexeme(c, c.ast[n].operator).starts_with('?')
            || contains_null_aware_invocation_in_chain(c, c.ast[n].target.map(|t| t.raw()))
    } else if let Some(n) = c.ast.cast::<MethodInvocation>(node) {
        c.ast[n]
            .operator
            .is_some_and(|o| lexeme(c, o).starts_with('?'))
            || contains_null_aware_invocation_in_chain(c, c.ast[n].target.map(|t| t.raw()))
    } else if let Some(n) = c.ast.cast::<IndexExpression>(node) {
        c.ast[n].question.is_some()
            || contains_null_aware_invocation_in_chain(c, c.ast[n].target.map(|t| t.raw()))
    } else {
        false
    }
}

/// Dart `Expression?.directlyContainsWhitespace`.
fn directly_contains_whitespace(c: &LinterContext<'_>, node: Option<NodeId>) -> bool {
    let Some(node) = node else { return false };
    match kind(c, node) {
        NodeKind::AsExpression
        | NodeKind::AssignmentExpression
        | NodeKind::AwaitExpression
        | NodeKind::BinaryExpression
        | NodeKind::FunctionExpression
        | NodeKind::IsExpression
        | NodeKind::SwitchExpression => true,
        NodeKind::InstanceCreationExpression => c.ast
            [Id::<InstanceCreationExpression>::from_raw(node)]
        .keyword
        .is_some(),
        NodeKind::ListLiteral => c.ast[Id::<ListLiteral>::from_raw(node)]
            .const_keyword
            .is_some(),
        NodeKind::SetOrMapLiteral => c.ast[Id::<SetOrMapLiteral>::from_raw(node)]
            .const_keyword
            .is_some(),
        NodeKind::MethodInvocation => directly_contains_whitespace(
            c,
            c.ast[Id::<MethodInvocation>::from_raw(node)]
                .target
                .map(|t| t.raw()),
        ),
        NodeKind::PropertyAccess => directly_contains_whitespace(
            c,
            c.ast[Id::<PropertyAccess>::from_raw(node)]
                .target
                .map(|t| t.raw()),
        ),
        _ => false,
    }
}

/// Dart `ParenthesizedExpression.containsFunctionExpression`.
fn contains_function_expression(c: &LinterContext<'_>, node: NodeId) -> bool {
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        if kind(c, n) == NodeKind::FunctionExpression {
            return true;
        }
        stack.extend(c.ast.children(n));
    }
    false
}

/// Dart `ParenthesizedExpression.isBareInConstructorFieldInitializer`.
fn is_bare_in_constructor_field_initializer(c: &LinterContext<'_>, node: NodeId) -> bool {
    let mut ancestor = c.ast.parent(node);
    while let Some(a) = ancestor {
        let k = kind(c, a);
        if k == NodeKind::ConstructorFieldInitializer {
            return true;
        }
        if FunctionBody::test(k) || k == NodeKind::MethodInvocation {
            return false;
        }
        ancestor = c.ast.parent(a);
    }
    false
}

/// Dart `ParenthesizedExpression.wouldBeParsedAsStatementBlock`.
fn would_be_parsed_as_statement_block(c: &LinterContext<'_>, node: NodeId) -> bool {
    let n = &c.ast[Id::<ParenthesizedExpression>::from_raw(node)];
    if kind(c, n.expression.raw()) != NodeKind::SetOrMapLiteral {
        return false;
    }
    let Some(statement) =
        this_or_ancestor(c, node, |a| kind(c, a) == NodeKind::ExpressionStatement)
    else {
        return false;
    };
    c.ast.begin_token(statement) == n.left_parenthesis
}
