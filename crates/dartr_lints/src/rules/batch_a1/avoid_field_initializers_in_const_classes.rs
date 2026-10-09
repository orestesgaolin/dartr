// Dart source: pkg/linter/lib/src/rules/avoid_field_initializers_in_const_classes.dart

use super::helpers::{ancestor, base_element, descendants};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{ConstructorFieldInitializer, FieldDeclaration, Id, NodeId, NodeKind};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::ClassElement;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::FieldDeclaration,
        "avoid_field_initializers_in_const_classes",
        check,
    );
    registry.add(
        NodeKind::ConstructorFieldInitializer,
        "avoid_field_initializers_in_const_classes",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if c.ast.kind(node) == NodeKind::ConstructorFieldInitializer {
        check_initializer(c, Id::<ConstructorFieldInitializer>::from_raw(node), out);
        return;
    }
    let field = &c.ast[Id::<FieldDeclaration>::from_raw(node)];
    if field.augment_keyword.is_some() || field.static_keyword.is_some() {
        return;
    }
    let fields = &c.ast[field.fields];
    if fields
        .keyword
        .is_none_or(|k| c.ast.tokens.lexeme(k) != "final")
    {
        return;
    }
    let Some(class_node) = ancestor(c.ast, node, NodeKind::ClassDeclaration) else {
        return;
    };
    let (Some(r), Some(class)) = (
        c.resolved,
        c.declared_element(class_node)
            .and_then(|element| element.cast::<ClassElement>()),
    ) else {
        return;
    };
    let has_const = r
        .ctx
        .get(class)
        .constructors
        .iter()
        .any(|constructor| dartr_link::dump::is_const(&r.ctx, constructor.raw()));
    if !has_const {
        return;
    }
    for variable in c.ast.list(fields.variables) {
        if c.ast[*variable].initializer.is_some() {
            c.report_node(
                out,
                &diag::AVOID_FIELD_INITIALIZERS_IN_CONST_CLASSES,
                *variable,
                &[],
            );
        }
    }
}

fn check_initializer(
    c: &LinterContext<'_>,
    node: Id<ConstructorFieldInitializer>,
    out: &mut Vec<Diagnostic>,
) {
    let Some(r) = c.resolved else { return };
    let constructor_node = ancestor(c.ast, node, NodeKind::ConstructorDeclaration).or_else(|| {
        let body = ancestor(c.ast, node, NodeKind::PrimaryConstructorBody)?;
        let declaration = c.ast.parent(c.ast.parent(body)?)?;
        let name_part = match c.ast.kind(declaration) {
            NodeKind::ClassDeclaration => c.ast
                [Id::<dartr_ast::ClassDeclaration>::from_raw(declaration)]
            .name_part
            .raw(),
            NodeKind::EnumDeclaration => c.ast
                [Id::<dartr_ast::EnumDeclaration>::from_raw(declaration)]
            .name_part
            .raw(),
            NodeKind::ExtensionTypeDeclaration => c.ast
                [Id::<dartr_ast::ExtensionTypeDeclaration>::from_raw(declaration)]
            .name_part
            .raw(),
            _ => return None,
        };
        (c.ast.kind(name_part) == NodeKind::PrimaryConstructorDeclaration).then_some(name_part)
    });
    let Some(constructor_node) = constructor_node else {
        return;
    };
    let Some(constructor) = c
        .declared_element(constructor_node)
        .and_then(|e| e.cast::<dartr_element::ConstructorElement>())
    else {
        return;
    };
    let fragment = r.ctx.fragment(r.ctx.get(constructor).first_fragment());
    if !fragment
        .flags
        .has(dartr_element::FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST)
    {
        return;
    }
    let Some(enclosing) = r
        .ctx
        .get(constructor)
        .enclosing
        .and_then(|e| e.cast::<ClassElement>())
    else {
        return;
    };
    if r.ctx.get(enclosing).constructors.len() > 1 {
        return;
    }
    let parameters = &r.ctx.get(constructor).formal_params;
    let expression = c.ast[node].expression;
    let uses_parameter = std::iter::once(expression.raw())
        .chain(descendants(c.ast, expression))
        .any(|child| {
            c.element(child)
                .and_then(|e| base_element(c, e))
                .is_some_and(|e| parameters.iter().any(|p| p.raw() == e))
        });
    if !uses_parameter {
        c.report_node(
            out,
            &diag::AVOID_FIELD_INITIALIZERS_IN_CONST_CLASSES,
            node,
            &[],
        );
    }
}
