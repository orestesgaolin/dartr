// Dart source: pkg/linter/lib/src/rules/use_declaring_parameters.dart
use super::util::*;
use crate::{ExperimentalFlag, LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{EId, ElemRef, ElementId, FormalParameterElement, Tag};

pub fn register(r: &mut RuleVisitorRegistry, c: &LinterContext<'_>) {
    if !c.is_feature_enabled(ExperimentalFlag::PrimaryConstructors) {
        return;
    }
    r.add(
        NodeKind::PrimaryConstructorDeclaration,
        "use_declaring_parameters",
        visit_primary_constructor_declaration,
    );
}

fn element_type(c: &LinterContext<'_>, element: ElementId) -> Option<dartr_element::TypeId> {
    Some(dartr_typesystem::member::type_(
        &rctx(c)?,
        ElemRef::Base(element),
    ))
}

fn visit_primary_constructor_declaration(
    c: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    let body = super::tighten_type_of_initializing_formals::primary_constructor_body(c, node);
    let list = c.ast[Id::<PrimaryConstructorDeclaration>::from_raw(node)].formal_parameters;
    for parameter in parameters(c, Some(list)) {
        if let Some(field_formal) = c.ast.cast::<FieldFormalParameter>(parameter) {
            check_field_formal_parameter(c, field_formal, out);
        } else if let Some(regular) = c.ast.cast::<RegularFormalParameter>(parameter)
            && c.ast[regular].const_final_or_var_keyword.is_none()
            && let Some(body) = body
        {
            check_non_declaring_parameter(c, regular, body, out);
        }
    }
}

/// Dart `_checkFieldFormalParameter`.
fn check_field_formal_parameter(
    c: &LinterContext<'_>,
    parameter: Id<FieldFormalParameter>,
    out: &mut Vec<Diagnostic>,
) {
    let Some(ctx) = rctx(c) else { return };
    let p = &c.ast[parameter];
    let Some(element) = c
        .declared_element(parameter.raw())
        .filter(|e| e.tag() == Tag::FieldFormalParameter)
    else {
        return;
    };
    let Some(field) = ctx
        .get(EId::<FormalParameterElement>::from_raw(element))
        .field
        .get()
    else {
        return;
    };
    let same_type = match (element_type(c, field.raw()), element_type(c, element)) {
        (Some(a), Some(b)) => types_equal(c, a, b),
        _ => false,
    };
    if p.type_.is_none() || same_type {
        c.report_token(out, &diag::USE_DECLARING_PARAMETERS, p.name, &[]);
    }
}

/// Dart `_checkNonDeclaringParameter`.
fn check_non_declaring_parameter(
    c: &LinterContext<'_>,
    parameter: Id<RegularFormalParameter>,
    body: Id<PrimaryConstructorBody>,
    out: &mut Vec<Diagnostic>,
) {
    let Some(assigned_field) = find_assigned_field(c, parameter.raw(), body) else {
        return;
    };
    let Some(name) = c.ast[parameter].name else {
        return;
    };
    let Some(element) = c.declared_element(parameter.raw()) else {
        return;
    };
    if let (Some(a), Some(b)) = (element_type(c, assigned_field), element_type(c, element))
        && types_equal(c, a, b)
    {
        c.report_token(out, &diag::USE_DECLARING_PARAMETERS, name, &[]);
    }
}

/// Dart `_findAssignedField`.
fn find_assigned_field(
    c: &LinterContext<'_>,
    parameter: NodeId,
    body: Id<PrimaryConstructorBody>,
) -> Option<ElementId> {
    let ctx = rctx(c)?;
    let parameter_element = c.declared_element(parameter)?;
    let is_parameter = |n: NodeId| {
        kind(c, n) == NodeKind::SimpleIdentifier
            && c.element(n)
                .is_some_and(|e| e == ElemRef::Base(parameter_element))
    };
    for &initializer in c.ast.list_raw(c.ast[body].initializers) {
        if let Some(i) = c.ast.cast::<ConstructorFieldInitializer>(initializer)
            && is_parameter(c.ast[i].expression.raw())
            && let Some(field) = c
                .element(c.ast[i].field_name)
                .map(|e| base(c, e))
                .filter(|e| e.tag() == Tag::Field)
            && names_match(name(c, parameter_element), name(c, field))
        {
            return Some(field);
        }
    }
    if let Some(block) = c.ast.cast::<BlockFunctionBody>(c.ast[body].body.raw()) {
        for &statement in c.ast.list_raw(c.ast[c.ast[block].block].statements) {
            let Some(statement) = c.ast.cast::<ExpressionStatement>(statement) else {
                continue;
            };
            let Some(assignment) = c
                .ast
                .cast::<AssignmentExpression>(c.ast[statement].expression.raw())
            else {
                continue;
            };
            if !is_parameter(c.ast[assignment].right_hand_side.raw()) {
                continue;
            }
            let write = c
                .resolved
                .as_ref()
                .and_then(|r| r.tables.write_element.get(assignment.raw()).copied());
            let Some(setter) = write.map(|e| base(c, e)).filter(|e| e.tag() == Tag::Setter) else {
                continue;
            };
            let variable = ctx
                .property_accessor(EId::from_raw(setter))
                .variable
                .get()
                .map(|v| v.raw());
            if let Some(field) = variable.filter(|v| v.tag() == Tag::Field)
                && names_match(name(c, parameter_element), name(c, field))
            {
                return Some(field);
            }
        }
    }
    None
}

/// Dart `_namesMatch`.
fn names_match(parameter_name: Option<&str>, field_name: Option<&str>) -> bool {
    let (Some(p), Some(f)) = (parameter_name, field_name) else {
        return false;
    };
    if p == f {
        true
    } else if let Some(rest) = p.strip_prefix('_') {
        rest == f
    } else if let Some(rest) = f.strip_prefix('_') {
        p == rest
    } else {
        false
    }
}
