// Dart source: pkg/linter/lib/src/rules/strict_top_level_inference.dart
use super::util::*;
use crate::{ExperimentalFlag, LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{EId, ElemRef, FormalParameterElement, MethodElement, Tag};
use dartr_syntax::TokenId;
use dartr_typesystem::TypeExt;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    const NAME: &str = "strict_top_level_inference";
    r.add(NodeKind::ConstructorDeclaration, NAME, visit_constructor_declaration);
    r.add(NodeKind::FunctionDeclaration, NAME, visit_function_declaration);
    r.add(NodeKind::MethodDeclaration, NAME, visit_method_declaration);
    r.add(NodeKind::PrimaryConstructorDeclaration, NAME, visit_primary_constructor_declaration);
    r.add(NodeKind::VariableDeclarationList, NAME, visit_variable_declaration_list);
}

fn is_wildcard_identifier(c: &LinterContext<'_>, lexeme: &str) -> bool {
    c.is_feature_enabled(ExperimentalFlag::WildcardVariables) && lexeme == "_"
}

fn visit_constructor_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let list = c.ast[Id::<ConstructorDeclaration>::from_raw(node)].parameters;
    check_formal_parameters(c, parameters(c, Some(list)), None, out);
}

fn visit_primary_constructor_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let list = c.ast[Id::<PrimaryConstructorDeclaration>::from_raw(node)].formal_parameters;
    check_formal_parameters(c, parameters(c, Some(list)), None, out);
}

fn visit_function_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if !c.ast.parent(node).is_some_and(|p| kind(c, p) == NodeKind::CompilationUnit) {
        return;
    }
    let n = &c.ast[Id::<FunctionDeclaration>::from_raw(node)];
    let is_setter = n.property_keyword.is_some_and(|k| lexeme(c, k) == "set");
    if n.return_type.is_none() && !is_setter && lexeme(c, n.name) != "[]=" {
        report(c, n.name, None, out);
    }
    let list = c.ast[n.function_expression].parameters;
    if list.is_some() {
        check_formal_parameters(c, parameters(c, list), None, out);
    }
}

fn is_static(c: &LinterContext<'_>, n: &MethodDeclaration) -> bool {
    n.modifier_keyword.is_some_and(|k| lexeme(c, k) == "static")
}

fn visit_method_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(element) = c.declared_element(node) else { return };
    let n = &c.ast[Id::<MethodDeclaration>::from_raw(node)];
    match element.tag() {
        Tag::Getter | Tag::Setter => {
            if n.property_keyword.is_some_and(|k| lexeme(c, k) == "get") {
                check_getter(c, node, n, element, out);
            } else {
                check_setter(c, node, n, element, out);
            }
        }
        Tag::Method => check_method(c, node, n, element, out),
        _ => {}
    }
}

fn check_getter(c: &LinterContext<'_>, node: NodeId, n: &MethodDeclaration, element: dartr_element::ElementId, out: &mut Vec<Diagnostic>) {
    if n.return_type.is_some() {
        return;
    }
    if !is_override(c, node, n, element) {
        c.report_token(out, &diag::STRICT_TOP_LEVEL_INFERENCE_ADD_TYPE, n.name, &[]);
    }
}

fn check_setter(c: &LinterContext<'_>, node: NodeId, n: &MethodDeclaration, element: dartr_element::ElementId, out: &mut Vec<Diagnostic>) {
    let Some(&parameter) = parameters(c, n.parameters).first() else { return };
    let Some(parameter) = c.ast.cast::<RegularFormalParameter>(parameter) else { return };
    let p = &c.ast[parameter];
    if p.function_typed_suffix.is_some() || p.type_.is_some() {
        return;
    }
    if !is_override(c, node, n, element) {
        c.report_token(out, &diag::STRICT_TOP_LEVEL_INFERENCE_ADD_TYPE, n.name, &[]);
    }
}

fn container_is_extension(c: &LinterContext<'_>, element: dartr_element::ElementId) -> bool {
    enclosing(c, element).is_some_and(|e| matches!(e.tag(), Tag::Extension | Tag::ExtensionType))
}

fn is_override(c: &LinterContext<'_>, node: NodeId, n: &MethodDeclaration, element: dartr_element::ElementId) -> bool {
    if is_static(c, n) || container_is_extension(c, element) {
        return false;
    }
    c.declared_element(node).and_then(|e| overridden_member(c, e)).is_some()
}

/// Dart `InterfaceElementExtension.isReflectiveTest`.
fn is_reflective_test(c: &LinterContext<'_>, container: dartr_element::ElementId) -> bool {
    container.tag() == Tag::Class
        && c.has_annotation_where(container, |ctx, a| {
            a.tag() == Tag::Getter
                && ctx.element_name(a) == Some("reflectiveTest")
                && dartr_typesystem::member::library(ctx, ElemRef::Base(a)).is_some_and(|l| {
                    ctx.library_uri(l) == "package:test_reflective_loader/test_reflective_loader.dart"
                })
        })
}

fn check_method(c: &LinterContext<'_>, node: NodeId, n: &MethodDeclaration, element: dartr_element::ElementId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    if ctx.get(EId::<MethodElement>::from_raw(element)).type_inference_error.try_get().is_some() {
        return;
    }
    let Some(container) = enclosing(c, element) else { return };
    let no_override = is_static(c, n) || matches!(container.tag(), Tag::Extension | Tag::ExtensionType);
    let name = lexeme(c, n.name);
    if no_override {
        if n.return_type.is_none() && name != "[]=" {
            c.report_token(out, &diag::STRICT_TOP_LEVEL_INFERENCE_ADD_TYPE, n.name, &[]);
        }
        if n.parameters.is_some() {
            check_formal_parameters(c, parameters(c, n.parameters), None, out);
        }
    } else {
        let overridden = c.declared_element(node).and_then(|e| overridden_member(c, e));
        if overridden.is_none()
            && n.return_type.is_none()
            && name != "[]="
            && (!is_reflective_test(c, container)
                || (!name.starts_with("test_") && !name.starts_with("solo_test_")))
        {
            report(c, n.name, None, out);
        }
        if n.parameters.is_some() {
            check_formal_parameters(c, parameters(c, n.parameters), Some(overridden), out);
        }
    }
}

fn visit_variable_declaration_list(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<VariableDeclarationList>::from_raw(node)];
    if n.type_.is_some() {
        return;
    }
    if !c
        .ast
        .parent(node)
        .is_some_and(|p| matches!(kind(c, p), NodeKind::TopLevelVariableDeclaration | NodeKind::FieldDeclaration))
    {
        return;
    }
    let variables = c.ast.list(n.variables);
    let missing: Vec<_> = variables.iter().copied().filter(|&v| c.ast[v].initializer.is_none()).collect();
    if missing.is_empty() {
        return;
    }
    let overridden = |v: Id<VariableDeclaration>| c.declared_element(v.raw()).and_then(|e| overridden_member(c, e));
    if variables.len() == 1 {
        let variable = variables[0];
        if overridden(variable).is_none() {
            report(c, c.ast[variable].name, n.keyword, out);
        }
    } else {
        for variable in missing {
            if overridden(variable).is_none() {
                c.report_token(out, &diag::STRICT_TOP_LEVEL_INFERENCE_SPLIT_TO_TYPES, c.ast[variable].name, &[]);
            }
        }
    }
}

fn is_named_parameter(c: &LinterContext<'_>, parameter: ElemRef) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    let base = dartr_typesystem::member::base_element(&ctx, parameter);
    ctx.get(EId::<FormalParameterElement>::from_raw(base)).kind.is_named()
}

/// Dart `_checkFormalParameters`; [overridden] is `Some(overriddenMember)`
/// for an instance method.
fn check_formal_parameters(
    c: &LinterContext<'_>,
    parameters: Vec<NodeId>,
    overridden: Option<Option<ElemRef>>,
    out: &mut Vec<Diagnostic>,
) {
    let overridden = overridden.flatten();
    for (i, &parameter) in parameters.iter().enumerate() {
        let name = match kind(c, parameter) {
            NodeKind::RegularFormalParameter => c.ast[Id::<RegularFormalParameter>::from_raw(parameter)].name,
            NodeKind::FieldFormalParameter => Some(c.ast[Id::<FieldFormalParameter>::from_raw(parameter)].name),
            NodeKind::SuperFormalParameter => Some(c.ast[Id::<SuperFormalParameter>::from_raw(parameter)].name),
            _ => None,
        };
        let Some(name) = name else { continue };
        if is_wildcard_identifier(c, lexeme(c, name)) {
            continue;
        }
        let Some(regular) = c.ast.cast::<RegularFormalParameter>(parameter) else { return };
        let p = &c.ast[regular];
        if p.function_typed_suffix.is_some() || p.type_.is_some() {
            return;
        }
        let keyword = p.const_final_or_var_keyword;
        match overridden {
            None => report(c, name, keyword, out),
            Some(member) => {
                let Some(ctx) = rctx(c) else { return };
                let formal_parameters = dartr_typesystem::member::formal_parameters(&ctx, member);
                if p.kind.is_positional() {
                    if formal_parameters.len() <= i || is_named_parameter(c, formal_parameters[i]) {
                        report(c, name, keyword, out);
                    }
                } else if !formal_parameters.iter().any(|&fp| is_named_parameter(c, fp)) {
                    report(c, name, keyword, out);
                }
            }
        }
    }
}

/// Dart `_report`.
fn report(c: &LinterContext<'_>, error_token: TokenId, keyword: Option<TokenId>, out: &mut Vec<Diagnostic>) {
    match keyword.map(|k| lexeme(c, k)) {
        None | Some("final") => c.report_token(out, &diag::STRICT_TOP_LEVEL_INFERENCE_ADD_TYPE, error_token, &[]),
        Some(keyword @ "var") => {
            c.report_token(out, &diag::STRICT_TOP_LEVEL_INFERENCE_REPLACE_KEYWORD, error_token, &[keyword])
        }
        _ => {}
    }
}
