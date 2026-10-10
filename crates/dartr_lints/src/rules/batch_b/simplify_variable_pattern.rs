// Dart source: pkg/linter/lib/src/rules/simplify_variable_pattern.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{EId, InstanceElement, Tag, TypeId, TypeKind};

pub fn register(r: &mut RuleVisitorRegistry, c: &LinterContext<'_>) {
    if !c.is_feature_enabled(crate::ExperimentalFlag::Patterns) {
        return;
    }
    r.add(NodeKind::PatternField, "simplify_variable_pattern", check);
}

fn unparenthesized_pattern(c: &LinterContext<'_>, mut node: NodeId) -> NodeId {
    while let Some(p) = c.ast.cast::<ParenthesizedPattern>(node) {
        node = c.ast[p].pattern.raw();
    }
    node
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let (Some(ctx), Some(resolved)) = (rctx(c), c.resolved) else {
        return;
    };
    let field = &c.ast[Id::<PatternField>::from_raw(node)];
    let pattern = unparenthesized_pattern(c, field.pattern.raw());
    let Some(declared) = c.ast.cast::<DeclaredVariablePattern>(pattern) else {
        return;
    };
    let Some(name) = field.name.and_then(|n| c.ast[n].name) else {
        return;
    };
    if c.ast.tokens.get(name).is_synthetic() || lexeme(c, c.ast[declared].name) != lexeme(c, name) {
        return;
    }
    let lexeme_text = lexeme(c, name);
    let Some(parent) = c.ast.parent(node) else {
        return;
    };
    let accessor = if kind(c, parent) == NodeKind::ObjectPattern {
        match c.element(node).map(|e| base(c, e).tag()) {
            Some(Tag::Method) => "method",
            Some(Tag::Getter) => "getter",
            _ => "field",
        }
    } else {
        "field"
    };
    if kind(c, parent) == NodeKind::RecordPattern {
        let matched = resolved
            .tables
            .pattern_info
            .get(parent)
            .and_then(|i| i.matched_value_type);
        report_if_needed(c, name, matched, lexeme_text, accessor, out);
    } else if let Some(object) = c.ast.cast::<ObjectPattern>(parent) {
        let named_type = c.ast[object].type_;
        let Some(mut element) = c.element(named_type).map(|e| base(c, e)) else {
            return;
        };
        let mut ty = annotation_type(c, named_type);
        loop {
            if element.tag() == Tag::TypeAlias {
                let aliased = ctx
                    .get(EId::<dartr_element::TypeAliasElement>::from_raw(element))
                    .aliased_type
                    .get();
                ty = aliased;
                let Some(aliased_element) = aliased.and_then(|t| type_element(c, t)) else {
                    break;
                };
                element = aliased_element;
            } else if element.tag() == Tag::TypeParameter {
                let bound = ctx
                    .get(EId::<dartr_element::TypeParameterElement>::from_raw(
                        element,
                    ))
                    .bound
                    .get()
                    .unwrap_or(ctx.tp.object_question_type());
                ty = Some(bound);
                let Some(bounded) = type_element(c, bound) else {
                    break;
                };
                element = bounded;
            } else {
                break;
            }
        }
        report_if_needed(c, name, ty, lexeme_text, accessor, out);
    }
}

/// Dart `DartType.element` (interface, type parameter elements).
fn type_element(c: &LinterContext<'_>, ty: TypeId) -> Option<dartr_element::ElementId> {
    let ctx = rctx(c)?;
    match *ctx.ty(ty) {
        TypeKind::Interface { element, .. } => Some(element.raw()),
        TypeKind::TypeParameter { param, .. } => Some(param.raw()),
        _ => None,
    }
}

/// Dart `_reportIfNeeded`.
fn report_if_needed(
    c: &LinterContext<'_>,
    name: dartr_syntax::TokenId,
    ty: Option<TypeId>,
    lexeme_text: &str,
    accessor: &str,
    out: &mut Vec<Diagnostic>,
) {
    let Some(ctx) = rctx(c) else { return };
    let is_dynamic = ty.is_some_and(|t| matches!(*ctx.ty(t), TypeKind::Dynamic));
    if !is_dynamic {
        let element = ty.and_then(|t| type_element(c, t));
        if let Some(element) = element.and_then(|e| e.cast::<InstanceElement>()) {
            let data = ctx.instance(element);
            let mut methods: Vec<&str> = data
                .methods
                .iter()
                .filter_map(|m| self::name(c, m.raw()))
                .collect();
            if self::name(c, element.raw()) == Some("Function")
                && library_uri(c, element.raw()) == Some("dart:core")
            {
                methods.push("call");
            }
            let getters: Vec<&str> = data
                .getters
                .iter()
                .filter_map(|g| self::name(c, g.raw()))
                .collect();
            if !getters.contains(&lexeme_text) && !methods.contains(&lexeme_text) {
                return;
            }
        } else if let Some(TypeKind::Record { named, .. }) = ty.map(|t| *ctx.ty(t)) {
            if !ctx
                .list(named)
                .iter()
                .any(|f| ctx.name_str(f.name) == lexeme_text)
            {
                return;
            }
        } else if let Some(TypeKind::Function(_)) = ty.map(|t| *ctx.ty(t)) {
            if lexeme(c, name) != "call" {
                return;
            }
        } else {
            return;
        }
    }
    c.report_token(
        out,
        &diag::SIMPLIFY_VARIABLE_PATTERN,
        name,
        &[lexeme_text, accessor],
    );
}
