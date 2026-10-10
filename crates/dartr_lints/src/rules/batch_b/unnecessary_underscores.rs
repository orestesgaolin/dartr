// Dart source: pkg/linter/lib/src/rules/unnecessary_underscores.dart
use super::util::*;
use crate::{ExperimentalFlag, LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElemRef, ElementId, FragmentFlags, Tag};
use indexmap::IndexSet;

pub fn register(r: &mut RuleVisitorRegistry, c: &LinterContext<'_>) {
    if !c.is_feature_enabled(ExperimentalFlag::WildcardVariables) {
        return;
    }
    r.add(NodeKind::FormalParameterList, "unnecessary_underscores", parameters_check);
    r.add(NodeKind::VariableDeclaration, "unnecessary_underscores", variable);
}

/// Dart `isJustUnderscores` of the rule (`length > 1`).
fn is_just_underscores_name(name: Option<&str>) -> bool {
    name.is_some_and(|n| n.chars().count() > 1 && is_just_underscores(n))
}

/// Dart `collectReferences`: the elements of the simple identifiers of
/// [body] and [comment].
fn collect_references(c: &LinterContext<'_>, body: Option<NodeId>, comment: Option<NodeId>) -> IndexSet<ElementId> {
    let mut result = IndexSet::new();
    let Some(body) = body else { return result };
    let mut stack = vec![body];
    stack.extend(comment);
    while let Some(n) = stack.pop() {
        if kind(c, n) == NodeKind::SimpleIdentifier
            && let Some(ElemRef::Base(e)) = c.element(n)
        {
            result.insert(e);
        }
        if kind(c, n) == NodeKind::SimpleIdentifier
            && let Some(e @ ElemRef::Member(_)) = c.element(n)
        {
            result.insert(base(c, e));
        }
        stack.extend(c.ast.children(n));
    }
    result
}

fn parameters_check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let mut just_underscores = Vec::new();
    for parameter in c.ast.list_raw(c.ast[Id::<FormalParameterList>::from_raw(node)].parameters).to_vec() {
        if super::prefer_iterable_whereType::parameter_name(c, parameter).is_none() {
            continue;
        }
        let element = c.declared_element(parameter);
        if let Some(e) = element
            && e.tag() == Tag::FieldFormalParameter
            && flags(c, e).contains(FragmentFlags::FIELD_FORMAL_PARAMETER_FRAGMENT_IS_DECLARING)
        {
            continue;
        }
        if is_just_underscores_name(element.and_then(|e| name(c, e))) {
            just_underscores.push(parameter);
        }
    }
    if just_underscores.is_empty() {
        return;
    }
    let mut function_declaration = c.ast.parent(node);
    if let Some(f) = function_declaration
        && kind(c, f) == NodeKind::FunctionExpression
    {
        function_declaration = c.ast.parent(f);
    }
    let references = match function_declaration {
        Some(f) if kind(c, f) == NodeKind::ConstructorDeclaration => {
            let n = &c.ast[Id::<ConstructorDeclaration>::from_raw(f)];
            collect_references(c, Some(n.body.raw()), n.documentation_comment.map(|d| d.raw()))
        }
        Some(f) if kind(c, f) == NodeKind::FunctionDeclaration => {
            let n = &c.ast[Id::<FunctionDeclaration>::from_raw(f)];
            collect_references(
                c,
                Some(c.ast[n.function_expression].body.raw()),
                n.documentation_comment.map(|d| d.raw()),
            )
        }
        Some(f) if kind(c, f) == NodeKind::MethodDeclaration => {
            let n = &c.ast[Id::<MethodDeclaration>::from_raw(f)];
            collect_references(c, Some(n.body.raw()), n.documentation_comment.map(|d| d.raw()))
        }
        Some(f) if kind(c, f) == NodeKind::PrimaryConstructorDeclaration => {
            match super::tighten_type_of_initializing_formals::primary_constructor_body(c, f) {
                Some(body) => collect_references(
                    c,
                    Some(c.ast[body].body.raw()),
                    c.ast[body].documentation_comment.map(|d| d.raw()),
                ),
                None => IndexSet::new(),
            }
        }
        _ => IndexSet::new(),
    };
    for parameter in just_underscores {
        let element = c.declared_element(parameter);
        if element.is_none_or(|e| !references.contains(&e)) {
            let token = match kind(c, parameter) {
                NodeKind::RegularFormalParameter => c.ast[Id::<RegularFormalParameter>::from_raw(parameter)].name,
                NodeKind::FieldFormalParameter => Some(c.ast[Id::<FieldFormalParameter>::from_raw(parameter)].name),
                NodeKind::SuperFormalParameter => Some(c.ast[Id::<SuperFormalParameter>::from_raw(parameter)].name),
                _ => None,
            };
            if let Some(token) = token {
                c.report_token(out, &diag::UNNECESSARY_UNDERSCORES, token, &[]);
            }
        }
    }
}

fn variable(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let element = c.declared_element(node);
    if element.is_some_and(|e| matches!(e.tag(), Tag::Field | Tag::TopLevelVariable)) {
        return;
    }
    let name = c.ast[Id::<VariableDeclaration>::from_raw(node)].name;
    if is_just_underscores_name(Some(lexeme(c, name))) {
        let body = this_or_ancestor(c, node, |n| FunctionBody::test(kind(c, n)));
        let references = collect_references(c, body, None);
        if element.is_none_or(|e| !references.contains(&e)) {
            c.report_token(out, &diag::UNNECESSARY_UNDERSCORES, name, &[]);
        }
    }
}
