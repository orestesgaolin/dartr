// Dart source: pkg/linter/lib/src/rules/use_late_for_private_fields_and_variables.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{EId, ElemRef, ElementId, FormalParameterElement, Tag};
use indexmap::IndexSet;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::CompilationUnit, "use_late_for_private_fields_and_variables", visit_compilation_unit);
}

/// Dart `_Visitor`: the lateable variables of each unit and the elements
/// with a nullable access in the library.
#[derive(Default)]
struct State {
    lateables: Vec<Vec<NodeId>>,
    nullable_access: IndexSet<ElementId>,
}

/// Dart reports in `afterLibrary` into the unit of each variable. This port
/// visits all units of the library from the processor of each unit and
/// reports the lateables of the current unit.
fn visit_compilation_unit(c: &LinterContext<'_>, _: NodeId, out: &mut Vec<Diagnostic>) {
    if c.resolved.is_none() {
        return;
    }
    let mut state = State::default();
    for index in 0..c.resolved_units.len() {
        let Some(unit) = c.resolved_unit(index) else {
            state.lateables.push(Vec::new());
            continue;
        };
        state.lateables.push(Vec::new());
        walk(&unit, c.resolved_units[index].unit, index, &mut state);
    }
    let Some(variables) = state.lateables.get(c.current_unit) else { return };
    for &variable in variables {
        let element = match kind(c, variable) {
            NodeKind::VariableDeclaration | NodeKind::FieldFormalParameter => c.declared_element(variable),
            _ => None,
        };
        let Some(element) = element else { continue };
        if !state.nullable_access.contains(&element) {
            c.report_node(out, &diag::USE_LATE_FOR_PRIVATE_FIELDS_AND_VARIABLES, variable, &[]);
        }
    }
}

fn is_non_nullable(c: &LinterContext<'_>, element: ElementId) -> bool {
    let (Some(ctx), Some(ts)) = (rctx(c), c.type_system()) else { return false };
    ts.is_non_nullable(dartr_typesystem::member::type_(&ctx, ElemRef::Base(element)))
}

/// Dart `RecursiveAstVisitor` traversal with the overrides of `_Visitor`.
fn walk(c: &LinterContext<'_>, node: NodeId, unit: usize, state: &mut State) {
    match kind(c, node) {
        NodeKind::AssignmentExpression => {
            let n = &c.ast[Id::<AssignmentExpression>::from_raw(node)];
            let element = c
                .resolved
                .as_ref()
                .and_then(|r| r.tables.write_element.get(node).copied())
                .and_then(|e| c.canonical_element2(e));
            if let Some(element) = element {
                let assignee = n.left_hand_side.raw();
                let rhs_type = c.static_type(n.right_hand_side);
                if kind(c, assignee) == NodeKind::SimpleIdentifier && in_declaration_context(c, assignee) {
                } else if lexeme(c, n.operator) == "="
                    && rhs_type.is_some_and(|t| c.type_system().is_some_and(|ts| ts.is_non_nullable(t)))
                {
                } else {
                    state.nullable_access.insert(element);
                }
            }
        }
        NodeKind::ClassDeclaration => {
            let n = &c.ast[Id::<ClassDeclaration>::from_raw(node)];
            if let Some(body) = c.ast.cast::<BlockClassBody>(n.body.raw()) {
                for &member in c.ast.list_raw(c.ast[body].members) {
                    if c.ast.cast::<ConstructorDeclaration>(member).is_some_and(|m| c.ast[m].const_keyword.is_some()) {
                        return;
                    }
                }
            }
            if let Some(primary) = c.ast.cast::<PrimaryConstructorDeclaration>(n.name_part.raw())
                && c.ast[primary].const_keyword.is_some()
            {
                return;
            }
        }
        NodeKind::EnumDeclaration => return,
        NodeKind::FieldDeclaration => {
            let n = &c.ast[Id::<FieldDeclaration>::from_raw(node)];
            let parent = c.ast.parent(node).and_then(|p| c.ast.parent(p));
            if parent.is_some_and(|p| kind(c, p) == NodeKind::ExtensionTypeDeclaration) && n.static_keyword.is_none() {
                return;
            }
            if let Some(parent) = parent {
                let variables = c.ast.list(c.ast[n.fields].variables).to_vec();
                if kind(c, parent) == NodeKind::ExtensionDeclaration
                    && c.declared_element(parent).is_some_and(|e| name(c, e).is_none_or(|n| n.is_empty() || n.starts_with('_')))
                {
                    for variable in variables {
                        visit_variable(c, variable, unit, state);
                    }
                } else {
                    for variable in variables {
                        if lexeme(c, c.ast[variable].name).starts_with('_') {
                            visit_variable(c, variable, unit, state);
                        }
                    }
                }
            }
        }
        NodeKind::FieldFormalParameter => {
            if let Some(ctx) = rctx(c)
                && let Some(element) = c.declared_element(node)
                && element.tag() == Tag::FieldFormalParameter
                && let Some(field) = ctx.get(EId::<FormalParameterElement>::from_raw(element)).field.get()
            {
                state.nullable_access.insert(field.raw());
            }
        }
        NodeKind::PrefixedIdentifier => {
            let element = c.element(node).and_then(|e| c.canonical_element2(e));
            visit_identifier_or_property_access(c, node, element, state);
        }
        NodeKind::PrimaryConstructorDeclaration => {
            // Only a declaring field formal parameter (Dart
            // `FieldFormalParameterElement.isDeclaring`) can be reported
            // in `afterLibrary`; the element model has no declaring
            // parameters, so no parameter is lateable here.
        }
        NodeKind::PropertyAccess => {
            let n = &c.ast[Id::<PropertyAccess>::from_raw(node)];
            let element = c.element(n.property_name).and_then(|e| c.canonical_element2(e));
            visit_identifier_or_property_access(c, node, element, state);
        }
        NodeKind::SimpleIdentifier => {
            let element = c.element(node).and_then(|e| c.canonical_element2(e));
            visit_identifier_or_property_access(c, node, element, state);
        }
        NodeKind::TopLevelVariableDeclaration => {
            let list = c.ast[Id::<TopLevelVariableDeclaration>::from_raw(node)].variables;
            for variable in c.ast.list(c.ast[list].variables).to_vec() {
                if lexeme(c, c.ast[variable].name).starts_with('_') {
                    visit_variable(c, variable, unit, state);
                }
            }
        }
        _ => {}
    }
    for child in c.ast.children(node) {
        walk(c, child, unit, state);
    }
}

/// Dart `SimpleIdentifier.inDeclarationContext()`.
fn in_declaration_context(c: &LinterContext<'_>, node: NodeId) -> bool {
    let Some(parent) = c.ast.parent(node) else { return false };
    match kind(c, parent) {
        NodeKind::ImportDirective => c.ast[Id::<ImportDirective>::from_raw(parent)].prefix.is_some_and(|p| p.raw() == node),
        NodeKind::Label => c.ast.parent(parent).is_some_and(|g| {
            Statement::test(kind(c, g)) || matches!(kind(c, g), NodeKind::SwitchCase | NodeKind::SwitchDefault | NodeKind::SwitchPatternCase)
        }),
        _ => false,
    }
}

/// Dart `_visit`.
fn visit_variable(c: &LinterContext<'_>, variable: Id<VariableDeclaration>, unit: usize, state: &mut State) {
    let list = c.ast.parent(variable.raw()).and_then(|p| c.ast.cast::<VariableDeclarationList>(p));
    if list.is_some_and(|l| c.ast[l].late_keyword.is_some()) {
        return;
    }
    if c.ast.tokens.get(c.ast[variable].name).is_synthetic() {
        return;
    }
    let Some(element) = c.declared_element(variable.raw()) else { return };
    if is_non_nullable(c, element) {
        return;
    }
    state.lateables[unit].push(variable.raw());
}

/// Dart `_visitIdentifierOrPropertyAccess`.
fn visit_identifier_or_property_access(
    c: &LinterContext<'_>,
    expression: NodeId,
    canonical_element: Option<ElementId>,
    state: &mut State,
) {
    let Some(canonical_element) = canonical_element else { return };
    let mut parent = c.ast.parent(expression);
    if let Some(p) = parent
        && Expression::test(kind(c, p))
    {
        parent = Some(unparenthesized(c, p));
    }
    if kind(c, expression) == NodeKind::SimpleIdentifier && in_declaration_context(c, expression) {
    } else if let Some(postfix) = parent.and_then(|p| c.ast.cast::<PostfixExpression>(p))
        && c.ast[postfix].operand.raw() == expression
        && lexeme(c, c.ast[postfix].operator) == "!"
    {
    } else {
        state.nullable_access.insert(canonical_element);
    }
}
