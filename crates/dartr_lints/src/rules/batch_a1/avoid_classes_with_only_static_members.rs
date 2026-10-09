// Dart source: pkg/linter/lib/src/rules/avoid_classes_with_only_static_members.dart

use super::helpers::class_name_token;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{
    BlockClassBody, ClassDeclaration, FieldDeclaration, Id, MethodDeclaration, NodeId, NodeKind,
};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::ClassElement;
use dartr_typesystem::{TypeExt, inheritance_manager3::InheritanceManager3, member};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::ClassDeclaration,
        "avoid_classes_with_only_static_members",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let class = Id::<ClassDeclaration>::from_raw(node);
    let n = &c.ast[class];
    if n.augment_keyword.is_some() || n.sealed_keyword.is_some() {
        return;
    }
    if let (Some(r), Some(class_element)) = (
        c.resolved,
        c.declared_element(node)
            .and_then(|e| e.cast::<ClassElement>()),
    ) {
        let interface = InheritanceManager3::new(r.ctx).get_interface(class_element.upcast());
        for inherited in interface.map.values() {
            let Some(enclosing) = member::enclosing_element(&r.ctx, *inherited) else {
                continue;
            };
            if enclosing.is::<ClassElement>()
                && enclosing != class_element.raw()
                && !(r.ctx.element_name(enclosing) == Some("Object")
                    && member::library(&r.ctx, *inherited)
                        .is_some_and(|library| r.ctx.library_uri(library) == "dart:core"))
            {
                return;
            }
        }
    }
    let Some(body) = c.ast.cast::<BlockClassBody>(n.body) else {
        return;
    };
    let mut interesting = false;
    for member in c.ast.list_raw(c.ast[body].members) {
        match c.ast.kind(*member) {
            NodeKind::ConstructorDeclaration => {
                let ctor = &c.ast[Id::<dartr_ast::ConstructorDeclaration>::from_raw(*member)];
                if ctor.name.is_some() || ctor.factory_keyword.is_some() {
                    return;
                }
            }
            NodeKind::MethodDeclaration => {
                let method = &c.ast[Id::<MethodDeclaration>::from_raw(*member)];
                interesting = true;
                if method
                    .modifier_keyword
                    .is_none_or(|k| c.ast.tokens.lexeme(k) != "static")
                {
                    return;
                }
            }
            NodeKind::FieldDeclaration => {
                let field = &c.ast[Id::<FieldDeclaration>::from_raw(*member)];
                if field.static_keyword.is_none() {
                    return;
                }
                if c.ast[field.fields]
                    .keyword
                    .is_none_or(|k| c.ast.tokens.lexeme(k) != "const")
                {
                    interesting = true;
                }
            }
            _ => {}
        }
    }
    if interesting && let Some(name) = class_name_token(c.ast, class) {
        c.report_token(
            out,
            &diag::AVOID_CLASSES_WITH_ONLY_STATIC_MEMBERS,
            name,
            &[],
        );
    }
}
