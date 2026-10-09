// Dart source: pkg/linter/lib/src/rules/overridden_fields.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{FieldElement, InterfaceElement};
use dartr_typesystem::{
    TypeExt,
    inheritance_manager3::{InheritanceManager3, Name},
    member,
};
pub fn register(r: &mut RuleVisitorRegistry) {
    r.add_field_declaration("overridden_fields", check);
    r.add_primary_constructor_declaration("overridden_fields", check);
}
fn check_field(
    ctx: &LinterContext<'_>,
    field: dartr_element::EId<FieldElement>,
    token: dartr_syntax::TokenId,
    out: &mut Vec<Diagnostic>,
) {
    let r = ctx.resolved.unwrap();
    let Some(parent) = r
        .ctx
        .element_data(field.raw())
        .and_then(|d| d.enclosing)
        .and_then(|e| e.cast::<InterfaceElement>())
    else {
        return;
    };
    let text = ctx.ast.tokens.lexeme(token);
    let name = Name::for_library(&r.ctx, Some(r.library), text);
    let manager = InheritanceManager3::new(r.ctx);
    let Some(inherited) = manager
        .get_inherited_concrete_map(parent)
        .get(&name)
        .copied()
    else {
        return;
    };
    if member::base_element(&r.ctx, inherited).kind() != dartr_element::ElementKind::Getter
        || member::variable(&r.ctx, inherited).is_none()
    {
        return;
    }
    let defining = member::enclosing_interface(&r.ctx, inherited)
        .and_then(|e| r.ctx.element_name(e.raw()))
        .unwrap_or("");
    ctx.report_token(out, &diag::OVERRIDDEN_FIELDS, token, &[defining]);
}
fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(_) = ctx.resolved else {
        return;
    };
    match ctx.ast.kind(node) {
        NodeKind::FieldDeclaration => {
            let n = &ctx.ast[Id::<FieldDeclaration>::from_raw(node)];
            if n.augment_keyword.is_some() || n.static_keyword.is_some() {
                return;
            }
            for v in ctx.ast.list(ctx.ast[n.fields].variables) {
                if let Some(f) = ctx
                    .declared_element(v.raw())
                    .and_then(|e| e.cast::<FieldElement>())
                {
                    check_field(ctx, f, ctx.ast[*v].name, out);
                }
            }
        }
        NodeKind::PrimaryConstructorDeclaration => {
            for p in ctx.ast.list(
                ctx.ast[ctx.ast[Id::<PrimaryConstructorDeclaration>::from_raw(node)]
                    .formal_parameters]
                    .parameters,
            ) {
                let Some(e) = ctx
                    .declared_element(p.raw())
                    .and_then(|e| e.cast::<dartr_element::FormalParameterElement>())
                else {
                    continue;
                };
                if let Some(f) = ctx.resolved.unwrap().ctx.get(e).field.get() {
                    let token = ctx.ast.begin_token(*p);
                    check_field(ctx, f, token, out);
                }
            }
        }
        _ => {}
    }
}
