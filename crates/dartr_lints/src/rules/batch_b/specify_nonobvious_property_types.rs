// Dart source: pkg/linter/lib/src/rules/specify_nonobvious_property_types.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::TypeExt;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::FieldDeclaration, "specify_nonobvious_property_types", check);
    r.add(
        NodeKind::TopLevelVariableDeclaration,
        "specify_nonobvious_property_types",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let (list, is_instance_variable) = match kind(c, node) {
        NodeKind::FieldDeclaration => {
            let n = &c.ast[Id::<FieldDeclaration>::from_raw(node)];
            (n.fields, n.static_keyword.is_none())
        }
        _ => (c.ast[Id::<TopLevelVariableDeclaration>::from_raw(node)].variables, false),
    };
    if let Some(ty) = c.ast[list].type_.and_then(|t| annotation_type(c, t))
        && !ctx.is_dart_core_null(ty)
    {
        return;
    }
    let mut needed = Vec::new();
    for &child in c.ast.list(c.ast[list].variables) {
        if is_instance_variable {
            let variable_name = lexeme(c, c.ast[child].name);
            let mut ignore = false;
            let mut owning = Some(list.raw());
            while let Some(o) = owning {
                if matches!(
                    kind(c, o),
                    NodeKind::ClassDeclaration
                        | NodeKind::MixinDeclaration
                        | NodeKind::EnumDeclaration
                        | NodeKind::ExtensionTypeDeclaration
                ) && let Some(element) = c.declared_element(o).and_then(|e| e.cast::<dartr_element::InterfaceElement>())
                {
                    for &t in ctx.element_all_supertypes(element) {
                        if dartr_typesystem::lookup::type_get_getter(&ctx, t, variable_name).is_some()
                            || dartr_typesystem::lookup::type_get_setter(&ctx, t, variable_name).is_some()
                        {
                            ignore = true;
                        }
                    }
                }
                owning = c.ast.parent(o);
            }
            if ignore {
                continue;
            }
        }
        match c.ast[child].initializer {
            None => needed.push(child),
            Some(initializer) => {
                if !has_obvious_type(c, initializer.raw()) {
                    needed.push(child);
                }
            }
        }
    }
    if !needed.is_empty() {
        if c.ast.list(c.ast[list].variables).len() == 1 {
            c.report_node(out, &diag::SPECIFY_NONOBVIOUS_PROPERTY_TYPES, list, &[]);
        } else {
            for v in needed {
                c.report_node(out, &diag::SPECIFY_NONOBVIOUS_PROPERTY_TYPES, v, &[]);
            }
        }
    }
}
