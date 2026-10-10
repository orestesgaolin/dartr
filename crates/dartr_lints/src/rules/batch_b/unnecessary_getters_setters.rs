// Dart source: pkg/linter/lib/src/rules/unnecessary_getters_setters.dart
// Dart source: pkg/linter/lib/src/ast.dart (isSimpleGetter, isSimpleSetter)
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{EId, FragmentFlags, Tag};
use indexmap::IndexMap;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::ClassDeclaration, "unnecessary_getters_setters", check);
    r.add(
        NodeKind::ExtensionTypeDeclaration,
        "unnecessary_getters_setters",
        check,
    );
}

/// Dart `isSimpleGetter`.
pub(crate) fn is_simple_getter(c: &LinterContext<'_>, declaration: NodeId) -> bool {
    if method_property(c, declaration) != Some("get") {
        return false;
    }
    let body = c.ast[Id::<MethodDeclaration>::from_raw(declaration)].body.raw();
    let expression = if let Some(e) = c.ast.cast::<ExpressionFunctionBody>(body) {
        Some(c.ast[e].expression.raw())
    } else if let Some(b) = c.ast.cast::<BlockFunctionBody>(body) {
        let statements = c.ast.list_raw(c.ast[c.ast[b].block].statements);
        if statements.len() != 1 {
            return false;
        }
        let Some(r) = c.ast.cast::<ReturnStatement>(statements[0]) else {
            return false;
        };
        c.ast[r].expression.map(|e| e.raw())
    } else {
        return false;
    };
    check_for_simple_getter(c, declaration, expression)
}

/// Dart `_checkForSimpleGetter`.
fn check_for_simple_getter(c: &LinterContext<'_>, getter: NodeId, expression: Option<NodeId>) -> bool {
    let Some(expression) = expression else { return false };
    if kind(c, expression) != NodeKind::SimpleIdentifier {
        return false;
    }
    let Some(element) = c.element(expression).map(|e| base(c, e)) else {
        return false;
    };
    if element.tag() != Tag::Getter {
        return false;
    }
    let enclosing_element = c.declared_element(getter).and_then(|e| enclosing(c, e));
    if enclosing(c, element) == enclosing_element {
        let ctx = rctx(c).unwrap();
        let is_origin_variable =
            flags(c, element).contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE);
        let variable = ctx
            .property_accessor(EId::from_raw(element))
            .variable
            .get()
            .map(|v| v.raw());
        return is_origin_variable
            && variable.is_some_and(|v| name(c, v).is_some_and(|n| n.starts_with('_')));
    }
    false
}

/// Dart `isSimpleSetter`.
pub(crate) fn is_simple_setter(c: &LinterContext<'_>, setter: NodeId) -> bool {
    let body = c.ast[Id::<MethodDeclaration>::from_raw(setter)].body.raw();
    if let Some(e) = c.ast.cast::<ExpressionFunctionBody>(body) {
        return check_for_simple_setter(c, setter, c.ast[e].expression.raw());
    } else if let Some(b) = c.ast.cast::<BlockFunctionBody>(body) {
        let statements = c.ast.list_raw(c.ast[c.ast[b].block].statements);
        if statements.len() == 1
            && let Some(s) = c.ast.cast::<ExpressionStatement>(statements[0])
        {
            return check_for_simple_setter(c, setter, c.ast[s].expression.raw());
        }
    }
    false
}

/// Dart `_checkForSimpleSetter`.
fn check_for_simple_setter(c: &LinterContext<'_>, setter: NodeId, expression: NodeId) -> bool {
    let Some(assignment) = c.ast.cast::<AssignmentExpression>(expression) else {
        return false;
    };
    let a = &c.ast[assignment];
    if lexeme(c, a.operator) != "=" {
        return false;
    }
    let (lhs, rhs) = (a.left_hand_side.raw(), a.right_hand_side.raw());
    if kind(c, lhs) == NodeKind::SimpleIdentifier && kind(c, rhs) == NodeKind::SimpleIdentifier {
        let resolved = c.resolved.unwrap();
        let Some(left) = resolved.tables.write_element.get(expression).map(|&e| base(c, e)) else {
            return false;
        };
        if left.tag() != Tag::Setter
            || flags(c, left).contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION)
        {
            return false;
        }
        let write_type = resolved.tables.write_type.get(expression).copied();
        let right_type = c.static_type(rhs);
        let same = match (write_type, right_type) {
            (Some(a), Some(b)) => types_equal(c, a, b),
            (None, None) => true,
            _ => false,
        };
        if !same {
            return false;
        }
        let Some(right) = c.element(rhs).map(|e| base(c, e)) else {
            return false;
        };
        if !matches!(
            right.tag(),
            Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter
        ) {
            return false;
        }
        let parameters = parameters(c, c.ast[Id::<MethodDeclaration>::from_raw(setter)].parameters);
        if parameters.len() == 1 {
            return Some(right) == c.declared_element(parameters[0]);
        }
    }
    false
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let augment = match kind(c, node) {
        NodeKind::ClassDeclaration => c.ast[Id::<ClassDeclaration>::from_raw(node)].augment_keyword,
        _ => c.ast[Id::<ExtensionTypeDeclaration>::from_raw(node)].augment_keyword,
    };
    if augment.is_some() {
        return;
    }
    let mut getters = IndexMap::new();
    let mut setters = IndexMap::new();
    for member in super::sort_unnamed_constructors_first::body_members(c, node) {
        if kind(c, member) != NodeKind::MethodDeclaration {
            continue;
        }
        let name = lexeme(c, c.ast[Id::<MethodDeclaration>::from_raw(member)].name);
        match method_property(c, member) {
            Some("get") => {
                getters.insert(name, member);
            }
            Some("set") => {
                setters.insert(name, member);
            }
            _ => {}
        }
    }
    for (id, &getter) in &getters {
        let Some(&setter) = setters.get(id) else {
            continue;
        };
        let (Some(getter_element), Some(setter_element)) =
            (c.declared_element(getter), c.declared_element(setter))
        else {
            continue;
        };
        let no_metadata = |e| c.resolved.and_then(|r| r.metadata).is_none_or(|m| m.annotations(e).is_empty());
        if is_simple_setter(c, setter)
            && is_simple_getter(c, getter)
            && no_metadata(getter_element)
            && no_metadata(setter_element)
        {
            let name = c.ast[Id::<MethodDeclaration>::from_raw(getter)].name;
            c.report_token(out, &diag::UNNECESSARY_GETTERS_SETTERS, name, &[]);
        }
    }
}
