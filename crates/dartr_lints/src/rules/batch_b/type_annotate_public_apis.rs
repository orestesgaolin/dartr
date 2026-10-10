// Dart source: pkg/linter/lib/src/rules/type_annotate_public_apis.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElementId, Tag, TypeKind};
use dartr_typesystem::TypeExt;

const RULE: &str = "type_annotate_public_apis";

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::ConstructorDeclaration, RULE, check);
    r.add(NodeKind::FieldDeclaration, RULE, check);
    r.add(NodeKind::FunctionDeclaration, RULE, check);
    r.add(NodeKind::FunctionTypeAlias, RULE, check);
    r.add(NodeKind::MethodDeclaration, RULE, check);
    r.add(NodeKind::PrimaryConstructorDeclaration, RULE, check);
    r.add(NodeKind::TopLevelVariableDeclaration, RULE, check);
}

fn is_private(c: &LinterContext<'_>, token: dartr_syntax::TokenId) -> bool {
    lexeme(c, token).starts_with('_')
}

/// Dart `_isPublicConstructor`.
fn is_public_constructor(c: &LinterContext<'_>, constructor: ElementId) -> bool {
    let Some(enclosing) = enclosing(c, constructor) else {
        return false;
    };
    if enclosing.tag() == Tag::Enum || !is_public(c, enclosing) {
        return false;
    }
    is_public(c, constructor)
}

/// Dart `_VisitorHelper` (a `RecursiveAstVisitor`) over [root].
fn helper(c: &LinterContext<'_>, root: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        match kind(c, n) {
            NodeKind::RegularFormalParameter => {
                let p = &c.ast[Id::<RegularFormalParameter>::from_raw(n)];
                if p.type_.is_none()
                    && let Some(name) = p.name
                    && !is_just_underscores(lexeme(c, name))
                {
                    c.report_node(out, &diag::TYPE_ANNOTATE_PUBLIC_APIS, n, &[]);
                }
                // The override does not visit the children.
                continue;
            }
            NodeKind::VariableDeclaration => {
                let v = &c.ast[Id::<VariableDeclaration>::from_raw(n)];
                let keyword = c
                    .ast
                    .parent(n)
                    .and_then(|l| c.ast.cast::<VariableDeclarationList>(l))
                    .and_then(|l| c.ast[l].keyword)
                    .map(|k| lexeme(c, k));
                let has_inferred_type = v.initializer.and_then(|i| c.static_type(i)).is_some_and(|t| {
                    !matches!(*ctx.ty(t), TypeKind::Dynamic) && !ctx.is_dart_core_null(t)
                });
                if !is_private(c, v.name) && keyword != Some("const") && !(keyword == Some("final") && has_inferred_type) {
                    c.report_token(out, &diag::TYPE_ANNOTATE_PUBLIC_APIS, v.name, &[]);
                }
                continue;
            }
            _ => {}
        }
        stack.extend(c.ast.children(n).into_iter().rev());
    }
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if is_augmentation(c, node) {
        return;
    }
    match kind(c, node) {
        NodeKind::ConstructorDeclaration => {
            if let Some(element) = c.declared_element(node)
                && is_public_constructor(c, element)
            {
                helper(c, c.ast[Id::<ConstructorDeclaration>::from_raw(node)].parameters.raw(), out);
            }
        }
        NodeKind::PrimaryConstructorDeclaration => {
            if let Some(element) = c.declared_element(node)
                && is_public_constructor(c, element)
            {
                helper(c, c.ast[Id::<PrimaryConstructorDeclaration>::from_raw(node)].formal_parameters.raw(), out);
            }
        }
        NodeKind::FieldDeclaration => {
            let fields = c.ast[Id::<FieldDeclaration>::from_raw(node)].fields;
            if c.ast[fields].type_.is_none() {
                helper(c, fields.raw(), out);
            }
        }
        NodeKind::TopLevelVariableDeclaration => {
            let variables = c.ast[Id::<TopLevelVariableDeclaration>::from_raw(node)].variables;
            if c.ast[variables].type_.is_none() {
                helper(c, variables.raw(), out);
            }
        }
        NodeKind::FunctionDeclaration => {
            let n = &c.ast[Id::<FunctionDeclaration>::from_raw(node)];
            if !is_private(c, n.name) && c.ast.parent(node).is_some_and(|p| kind(c, p) == NodeKind::CompilationUnit) {
                let is_setter = n.property_keyword.is_some_and(|k| lexeme(c, k) == "set");
                if n.return_type.is_none() && !is_setter {
                    c.report_token(out, &diag::TYPE_ANNOTATE_PUBLIC_APIS, n.name, &[]);
                } else if let Some(parameters) = c.ast[n.function_expression].parameters {
                    helper(c, parameters.raw(), out);
                }
            }
        }
        NodeKind::FunctionTypeAlias => {
            let n = &c.ast[Id::<FunctionTypeAlias>::from_raw(node)];
            if !is_private(c, n.name) {
                if n.return_type.is_none() {
                    c.report_token(out, &diag::TYPE_ANNOTATE_PUBLIC_APIS, n.name, &[]);
                } else {
                    helper(c, n.parameters.raw(), out);
                }
            }
        }
        _ => {
            let n = &c.ast[Id::<MethodDeclaration>::from_raw(node)];
            if is_private(c, n.name) {
                return;
            }
            let is_setter = n.property_keyword.is_some_and(|k| lexeme(c, k) == "set");
            if n.return_type.is_none() && !is_setter && lexeme(c, n.name) != "[]=" {
                c.report_token(out, &diag::TYPE_ANNOTATE_PUBLIC_APIS, n.name, &[]);
            }
            if let Some(parameters) = n.parameters {
                helper(c, parameters.raw(), out);
            }
        }
    }
}
