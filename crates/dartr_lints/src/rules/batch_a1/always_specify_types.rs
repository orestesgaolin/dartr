// Dart source: pkg/linter/lib/src/rules/always_specify_types.dart

use super::helpers::{declared_type, display_type, lexeme};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{Id, NodeId, NodeKind, RegularFormalParameter, VariableDeclarationList};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{AnyElement, TypeId};
use indexmap::IndexSet;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    for kind in [
        NodeKind::ListLiteral,
        NodeKind::SetOrMapLiteral,
        NodeKind::DeclaredIdentifier,
        NodeKind::DeclaredVariablePattern,
        NodeKind::NamedType,
        NodeKind::RegularFormalParameter,
        NodeKind::VariableDeclarationList,
    ] {
        registry.add(kind, "always_specify_types", check);
    }
}

fn report_keyword(
    c: &LinterContext<'_>,
    _node: NodeId,
    keyword: dartr_syntax::TokenId,
    ty: Option<TypeId>,
    out: &mut Vec<Diagnostic>,
) {
    if lexeme(c, keyword) == "var"
        && let Some(name) = ty.and_then(|t| display_type(c, t))
    {
        c.report_token(
            out,
            &diag::ALWAYS_SPECIFY_TYPES_REPLACE_KEYWORD,
            keyword,
            &["var", &name],
        );
    } else if let Some(name) = ty
        .filter(|&t| t != TypeId::DYNAMIC)
        .and_then(|t| display_type(c, t))
    {
        c.report_token(
            out,
            &diag::ALWAYS_SPECIFY_TYPES_SPECIFY_TYPE,
            keyword,
            &[&name],
        );
    } else {
        c.report_token(out, &diag::ALWAYS_SPECIFY_TYPES_ADD_TYPE, keyword, &[]);
    }
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match c.ast.kind(node) {
        NodeKind::DeclaredIdentifier => {
            let n = &c.ast[Id::<dartr_ast::DeclaredIdentifier>::from_raw(node)];
            if n.type_.is_none()
                && let Some(keyword) = n.keyword
            {
                report_keyword(c, node, keyword, declared_type(c, node), out);
            }
        }
        NodeKind::DeclaredVariablePattern => {
            let n = &c.ast[Id::<dartr_ast::DeclaredVariablePattern>::from_raw(node)];
            if n.type_.is_none() {
                let token = n.keyword.unwrap_or(n.name);
                report_keyword(c, node, token, declared_type(c, node), out);
            }
        }
        NodeKind::NamedType => {
            let Some(r) = c.resolved else { return };
            let n = &c.ast[Id::<dartr_ast::NamedType>::from_raw(node)];
            if n.type_arguments.is_some()
                || c.ast
                    .parent(node)
                    .is_some_and(|p| c.ast.kind(p) == NodeKind::IsExpression)
            {
                return;
            }
            let Some(element) = c
                .element(node)
                .and_then(|e| super::helpers::base_element(c, e))
            else {
                return;
            };
            let parameterized = match r.ctx.any(element) {
                AnyElement::Class(e) => !e.type_params.is_empty(),
                AnyElement::Enum(e) => !e.type_params.is_empty(),
                AnyElement::Mixin(e) => !e.type_params.is_empty(),
                AnyElement::ExtensionType(e) => !e.type_params.is_empty(),
                AnyElement::TypeAlias(e) => !e.type_params.is_empty(),
                _ => false,
            };
            if parameterized {
                c.report_node(out, &diag::ALWAYS_SPECIFY_TYPES_ADD_TYPE, node, &[]);
            }
        }
        NodeKind::ListLiteral => {
            let n = &c.ast[Id::<dartr_ast::ListLiteral>::from_raw(node)];
            if n.type_arguments.is_none() {
                c.report_token(
                    out,
                    &diag::ALWAYS_SPECIFY_TYPES_ADD_TYPE,
                    n.left_bracket,
                    &[],
                );
            }
        }
        NodeKind::SetOrMapLiteral => {
            let n = &c.ast[Id::<dartr_ast::SetOrMapLiteral>::from_raw(node)];
            if n.type_arguments.is_none() {
                c.report_token(
                    out,
                    &diag::ALWAYS_SPECIFY_TYPES_ADD_TYPE,
                    n.left_bracket,
                    &[],
                );
            }
        }
        NodeKind::RegularFormalParameter => {
            let n = &c.ast[Id::<RegularFormalParameter>::from_raw(node)];
            if n.type_.is_none()
                && n.name
                    .is_some_and(|name| !lexeme(c, name).chars().all(|ch| ch == '_'))
            {
                if let Some(keyword) = n.const_final_or_var_keyword {
                    report_keyword(c, node, keyword, declared_type(c, node), out);
                } else if let Some(ty) = declared_type(c, node) {
                    if let Some(name) = display_type(c, ty).filter(|_| ty != TypeId::DYNAMIC) {
                        c.report_node(
                            out,
                            &diag::ALWAYS_SPECIFY_TYPES_SPECIFY_TYPE,
                            node,
                            &[&name],
                        );
                    } else {
                        c.report_node(out, &diag::ALWAYS_SPECIFY_TYPES_ADD_TYPE, node, &[]);
                    }
                }
            }
        }
        NodeKind::VariableDeclarationList => {
            let n = &c.ast[Id::<VariableDeclarationList>::from_raw(node)];
            let Some(keyword) = n.keyword else { return };
            if n.type_.is_some() {
                return;
            }
            let mut types = IndexSet::new();
            for variable in c.ast.list(n.variables) {
                if let Some(initializer) = c.ast[*variable].initializer
                    && let Some(ty) = c.static_type(initializer)
                    && let Some(name) = display_type(c, ty)
                {
                    types.insert(name);
                }
            }
            if types.len() > 1 && lexeme(c, keyword) == "var" {
                c.report_token(
                    out,
                    &diag::ALWAYS_SPECIFY_TYPES_SPLIT_TO_TYPES,
                    keyword,
                    &[],
                );
            } else {
                let ty = c
                    .ast
                    .list(n.variables)
                    .first()
                    .and_then(|v| declared_type(c, *v));
                report_keyword(c, node, keyword, ty, out);
            }
        }
        _ => {}
    }
}
