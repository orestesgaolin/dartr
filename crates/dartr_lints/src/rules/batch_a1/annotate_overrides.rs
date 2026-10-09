// Dart source: pkg/linter/lib/src/rules/annotate_overrides.dart

use super::helpers::{element_name, has_resolved_annotation};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{
    FieldDeclaration, Id, MethodDeclaration, NodeId, NodeKind, PrimaryConstructorDeclaration,
    RegularFormalParameter,
};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElemRef, FormalParameterElement, InterfaceElement, Tag};
use dartr_typesystem::inheritance_manager3::{InheritanceManager3, Name};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(NodeKind::FieldDeclaration, "annotate_overrides", check);
    registry.add(NodeKind::MethodDeclaration, "annotate_overrides", check);
    registry.add(
        NodeKind::PrimaryConstructorDeclaration,
        "annotate_overrides",
        check,
    );
}

fn check_member(
    c: &LinterContext<'_>,
    owner: NodeId,
    declaration: NodeId,
    token: dartr_syntax::TokenId,
    out: &mut Vec<Diagnostic>,
) {
    let Some(r) = c.resolved else { return };
    if has_resolved_annotation(c, owner, "override")
        || has_resolved_annotation(c, declaration, "override")
    {
        return;
    }
    // Metadata constants are not exposed by the resolver yet. If this
    // declaration has unresolved metadata, it may be `@override`.
    if super::helpers::metadata(c.ast, owner)
        .into_iter()
        .chain(super::helpers::metadata(c.ast, declaration))
        .any(|annotation| c.element(annotation).is_none())
    {
        return;
    }
    let Some(mut element) = c.declared_element(declaration) else {
        return;
    };
    if element.tag() == Tag::FieldFormalParameter {
        let parameter = r
            .ctx
            .get(dartr_element::EId::<FormalParameterElement>::from_raw(
                element,
            ));
        let Some(field) = parameter.field.get() else {
            return;
        };
        element = field.raw();
    }
    let Some(data) = r.ctx.element_data(element) else {
        return;
    };
    let Some(enclosing) = data.enclosing.and_then(|e| e.cast::<InterfaceElement>()) else {
        return;
    };
    let Some(name) = element_name(c, ElemRef::Base(element)) else {
        return;
    };
    let inherited = InheritanceManager3::new(r.ctx)
        .get_overridden(enclosing, Name::for_library(&r.ctx, data.library, name));
    if inherited.is_some_and(|members| !members.is_empty()) {
        c.report_token(out, &diag::ANNOTATE_OVERRIDES, token, &[name]);
    }
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match c.ast.kind(node) {
        NodeKind::MethodDeclaration => {
            let n = &c.ast[Id::<MethodDeclaration>::from_raw(node)];
            if n.augment_keyword.is_none()
                && n.modifier_keyword
                    .is_none_or(|k| c.ast.tokens.lexeme(k) != "static")
                && super::helpers::ancestor(c.ast, node, NodeKind::ExtensionTypeDeclaration)
                    .is_none()
            {
                check_member(c, node, node, n.name, out);
            }
        }
        NodeKind::FieldDeclaration => {
            let n = &c.ast[Id::<FieldDeclaration>::from_raw(node)];
            if n.augment_keyword.is_some()
                || n.static_keyword.is_some()
                || super::helpers::ancestor(c.ast, node, NodeKind::ExtensionTypeDeclaration)
                    .is_some()
            {
                return;
            }
            for variable in c.ast.list(c.ast[n.fields].variables) {
                check_member(c, node, variable.raw(), c.ast[*variable].name, out);
            }
        }
        NodeKind::PrimaryConstructorDeclaration => {
            let n = &c.ast[Id::<PrimaryConstructorDeclaration>::from_raw(node)];
            let class = super::helpers::ancestor(c.ast, node, NodeKind::ClassDeclaration);
            let enum_ = super::helpers::ancestor(c.ast, node, NodeKind::EnumDeclaration);
            if class.is_none() && enum_.is_none() {
                return;
            }
            if class.is_some_and(|class| {
                c.ast[Id::<dartr_ast::ClassDeclaration>::from_raw(class)]
                    .augment_keyword
                    .is_some()
            }) || enum_.is_some_and(|enum_| {
                c.ast[Id::<dartr_ast::EnumDeclaration>::from_raw(enum_)]
                    .augment_keyword
                    .is_some()
            }) {
                return;
            }
            for parameter in c.ast.list_raw(c.ast[n.formal_parameters].parameters) {
                let Some(parameter) = c.ast.cast::<RegularFormalParameter>(*parameter) else {
                    continue;
                };
                let data = &c.ast[parameter];
                let Some(keyword) = data.const_final_or_var_keyword else {
                    continue;
                };
                if matches!(c.ast.tokens.lexeme(keyword), "final" | "var") {
                    check_member(c, node, parameter.raw(), data.name.unwrap_or(keyword), out);
                }
            }
        }
        _ => {}
    }
}
