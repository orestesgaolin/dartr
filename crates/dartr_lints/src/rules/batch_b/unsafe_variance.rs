// Dart source: pkg/linter/lib/src/rules/unsafe_variance.dart
use super::util::*;
use super::variance_checker::VarianceChecker;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElementId, ExecutableElement, Tag, TypeId, TypeKind, Variance};
use dartr_typesystem::TypeExt;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::MethodDeclaration, "unsafe_variance", visit_method_declaration);
    r.add(NodeKind::VariableDeclarationList, "unsafe_variance", visit_variable_declaration_list);
}

/// Dart `_UnsafeVarianceChecker.owningDeclarationSupportsVariance`.
fn owning_declaration_supports_variance(c: &LinterContext<'_>, element: ElementId) -> bool {
    let mut parent = enclosing(c, element);
    while let Some(p) = parent {
        match p.tag() {
            Tag::Class | Tag::Mixin | Tag::Enum => return true,
            Tag::ExtensionType | Tag::Extension => return false,
            _ if p.cast::<ExecutableElement>().is_some() => return false,
            _ => {}
        }
        parent = enclosing(c, p);
    }
    false
}

/// Dart `_UnsafeVarianceChecker.checkNamedType`.
fn check_named_type(c: &LinterContext<'_>, variance: Variance, static_type: TypeId, type_annotation: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    if let TypeKind::TypeParameter { param, .. } = *ctx.ty(static_type) {
        if !owning_declaration_supports_variance(c, param.raw()) {
            return;
        }
        if ctx.type_parameter_is_legacy_covariant(param) && variance != Variance::Covariant {
            c.report_node(out, &diag::UNSAFE_VARIANCE, type_annotation, &[]);
        }
    }
}

fn checker<'c, 'a>(
    c: &'c LinterContext<'a>,
    reports: &'c mut Vec<(Variance, TypeId, NodeId)>,
) -> VarianceChecker<'c, 'a, impl FnMut(Variance, TypeId, NodeId) + 'c> {
    VarianceChecker { c, check_named_type: move |v, t, n| reports.push((v, t, n)) }
}

fn visit_method_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<MethodDeclaration>::from_raw(node)];
    if n.modifier_keyword.is_some_and(|k| lexeme(c, k) == "static") {
        return;
    }
    let mut reports = Vec::new();
    {
        let mut checker = checker(c, &mut reports);
        checker.check_out(n.return_type.map(|t| t.raw()));
        if let Some(list) = n.type_parameters {
            for &parameter in c.ast.list(c.ast[list].type_parameters) {
                checker.check_bound(parameter);
            }
        }
    }
    for (variance, ty, annotation) in reports {
        check_named_type(c, variance, ty, annotation, out);
    }
}

fn visit_variable_declaration_list(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let mut reports = Vec::new();
    checker(c, &mut reports).check_out(c.ast[Id::<VariableDeclarationList>::from_raw(node)].type_.map(|t| t.raw()));
    for (variance, ty, annotation) in reports {
        check_named_type(c, variance, ty, annotation, out);
    }
}
