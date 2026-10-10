// Dart source: pkg/linter/lib/src/rules/prefer_final_fields.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{EId, ElementId, FormalParameterElement, FragmentFlags, InterfaceElement, Tag};
use dartr_typesystem::inheritance_manager3::{InheritanceManager3, Name};
use indexmap::IndexMap;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::CompilationUnit, "prefer_final_fields", check);
}

/// Dart `_DeclarationsCollector.overridesField`.
fn overrides_field(c: &LinterContext<'_>, field: ElementId) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    let Some(enclosing) = enclosing(c, field).and_then(|e| e.cast::<InterfaceElement>()) else {
        return false;
    };
    let library = ctx.element_data(field).and_then(|d| d.library);
    let name = format!("{}=", name(c, field).unwrap_or(""));
    InheritanceManager3::new(ctx)
        .get_overridden(enclosing, Name::for_library(&ctx, library, &name))
        .is_some()
}

fn is_final_or_const(c: &LinterContext<'_>, list: Id<VariableDeclarationList>) -> bool {
    c.ast[list]
        .keyword
        .is_some_and(|k| matches!(lexeme(c, k), "final" | "const"))
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    // Dart `_DeclarationsCollector`.
    let mut fields: IndexMap<ElementId, Id<VariableDeclaration>> = IndexMap::new();
    let mut fields_from_parameters: IndexMap<ElementId, NodeId> = IndexMap::new();
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        match kind(c, n) {
            NodeKind::FieldDeclaration => {
                let f = &c.ast[Id::<FieldDeclaration>::from_raw(n)];
                let declaration = c.ast.parent(n).and_then(|p| c.ast.parent(p));
                let invalid_extension_type_field = f.static_keyword.is_none()
                    && declaration
                        .is_some_and(|d| kind(c, d) == NodeKind::ExtensionTypeDeclaration);
                let in_enum = declaration.is_some_and(|d| kind(c, d) == NodeKind::EnumDeclaration);
                if !invalid_extension_type_field && !in_enum && !is_final_or_const(c, f.fields) {
                    for &variable in c.ast.list(c.ast[f.fields].variables) {
                        if let Some(element) = c.declared_element(variable)
                            && element.tag() == Tag::Field
                            && name(c, element).is_some_and(|n| n.starts_with('_'))
                            && !overrides_field(c, element)
                        {
                            fields.insert(element, variable);
                        }
                    }
                }
            }
            NodeKind::PrimaryConstructorDeclaration => {
                let declaration = c.ast.parent(n);
                if !declaration.is_some_and(|d| {
                    matches!(
                        kind(c, d),
                        NodeKind::EnumDeclaration | NodeKind::ExtensionTypeDeclaration
                    )
                }) {
                    let list =
                        c.ast[Id::<PrimaryConstructorDeclaration>::from_raw(n)].formal_parameters;
                    for parameter in parameters(c, Some(list)) {
                        let Some(element) = c.declared_element(parameter) else {
                            continue;
                        };
                        if element.tag() == Tag::FieldFormalParameter
                            && flags(c, element).contains(
                                FragmentFlags::FIELD_FORMAL_PARAMETER_FRAGMENT_IS_DECLARING,
                            )
                            && name(c, element).is_some_and(|n| n.starts_with('_'))
                            && let Some(field) = ctx
                                .get(EId::<FormalParameterElement>::from_raw(element))
                                .field
                                .get()
                            && !flags(c, field.raw())
                                .contains(FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL)
                            && !overrides_field(c, field.raw())
                        {
                            fields_from_parameters.insert(field.raw(), parameter);
                        }
                    }
                }
            }
            _ => {}
        }
        stack.extend(c.ast.children(n).into_iter().rev());
    }
    // Dart `_FieldMutationFinder` over all units of the library.
    for index in 0..c.resolved_units.len() {
        let Some(unit) = c.resolved_unit(index) else {
            continue;
        };
        let Some(resolved) = unit.resolved else {
            continue;
        };
        for n in (0..unit.ast.node_count()).map(NodeId::from_index) {
            let mutating = match unit.ast.kind(n) {
                NodeKind::AssignmentExpression | NodeKind::PostfixExpression => true,
                NodeKind::PrefixExpression => matches!(
                    unit.ast
                        .tokens
                        .lexeme(unit.ast[Id::<PrefixExpression>::from_raw(n)].operator),
                    "--" | "++"
                ),
                _ => false,
            };
            if !mutating {
                continue;
            }
            if let Some(element) = resolved
                .tables
                .write_element
                .get(n)
                .and_then(|&e| unit.canonical_element2(e))
                && element.tag() == Tag::Field
            {
                fields.shift_remove(&element);
                fields_from_parameters.shift_remove(&element);
            }
        }
    }
    for (field, variable) in fields {
        let class = c
            .ast
            .parent(variable.raw())
            .and_then(|l| c.ast.parent(l))
            .and_then(|f| c.ast.parent(f))
            .and_then(|b| c.ast.parent(b));
        let constructors: Vec<Id<ConstructorDeclaration>> = match class {
            Some(class) if kind(c, class) == NodeKind::ClassDeclaration => {
                super::sort_unnamed_constructors_first::body_members(c, class)
                    .into_iter()
                    .filter_map(|m| c.ast.cast::<ConstructorDeclaration>(m))
                    .collect()
            }
            _ => Vec::new(),
        };
        let is_set_in = |constructor: Id<ConstructorDeclaration>| {
            let k = &c.ast[constructor];
            c.ast.list_raw(k.initializers).iter().any(|&i| {
                c.ast
                    .cast::<ConstructorFieldInitializer>(i)
                    .is_some_and(|fi| {
                        canonical_element(c, c.ast[fi].field_name.raw()).map(|e| base(c, e))
                            == Some(field)
                    })
            }) || parameters(c, Some(k.parameters)).iter().any(|&p| {
                c.declared_element(p).is_some_and(|e| {
                    e.tag() == Tag::FieldFormalParameter
                        && ctx
                            .get(EId::<FormalParameterElement>::from_raw(e))
                            .field
                            .get()
                            .map(|f| f.raw())
                            == Some(field)
                })
            })
        };
        let name = lexeme(c, c.ast[variable].name);
        if constructors.iter().any(|&k| is_set_in(k)) {
            if constructors.iter().all(|&k| is_set_in(k)) {
                c.report_node(out, &diag::PREFER_FINAL_FIELDS, variable, &[name]);
            }
        } else if flags(c, field)
            .contains(FragmentFlags::NON_PARAMETER_VARIABLE_FRAGMENT_HAS_INITIALIZER)
        {
            c.report_node(out, &diag::PREFER_FINAL_FIELDS, variable, &[name]);
        }
    }
    for (_, parameter) in fields_from_parameters {
        let name = super::prefer_iterable_wheretype::parameter_name(c, parameter).unwrap_or("");
        c.report_node(out, &diag::PREFER_FINAL_FIELDS, parameter, &[name]);
    }
}
