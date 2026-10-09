// Dart source: pkg/linter/lib/src/rules/deprecated_member_use_from_same_package.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{
    ConstructorElement, ElementId, FieldElement, FormalParameterElement, TopLevelVariableElement,
};
use dartr_typesystem::TypeExt;
use indexmap::IndexMap;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::CompilationUnit,
        "deprecated_member_use_from_same_package",
        check,
    );
}
fn metadata(context: &LinterContext<'_>, node: NodeId) -> Option<NodeList<Annotation>> {
    match context.ast.kind(node) {
        NodeKind::ClassDeclaration => {
            Some(context.ast[context.ast.cast::<ClassDeclaration>(node)?].metadata)
        }
        NodeKind::MixinDeclaration => {
            Some(context.ast[context.ast.cast::<MixinDeclaration>(node)?].metadata)
        }
        NodeKind::EnumDeclaration => {
            Some(context.ast[context.ast.cast::<EnumDeclaration>(node)?].metadata)
        }
        NodeKind::ExtensionDeclaration => {
            Some(context.ast[context.ast.cast::<ExtensionDeclaration>(node)?].metadata)
        }
        NodeKind::ExtensionTypeDeclaration => {
            Some(context.ast[context.ast.cast::<ExtensionTypeDeclaration>(node)?].metadata)
        }
        NodeKind::ClassTypeAlias => {
            Some(context.ast[context.ast.cast::<ClassTypeAlias>(node)?].metadata)
        }
        NodeKind::FunctionTypeAlias => {
            Some(context.ast[context.ast.cast::<FunctionTypeAlias>(node)?].metadata)
        }
        NodeKind::GenericTypeAlias => {
            Some(context.ast[context.ast.cast::<GenericTypeAlias>(node)?].metadata)
        }
        NodeKind::LibraryDirective => {
            Some(context.ast[context.ast.cast::<LibraryDirective>(node)?].metadata)
        }
        NodeKind::ConstructorDeclaration => {
            Some(context.ast[context.ast.cast::<ConstructorDeclaration>(node)?].metadata)
        }
        NodeKind::PrimaryConstructorBody => {
            Some(context.ast[context.ast.cast::<PrimaryConstructorBody>(node)?].metadata)
        }
        NodeKind::MethodDeclaration => {
            Some(context.ast[context.ast.cast::<MethodDeclaration>(node)?].metadata)
        }
        NodeKind::FunctionDeclaration => {
            Some(context.ast[context.ast.cast::<FunctionDeclaration>(node)?].metadata)
        }
        NodeKind::FieldDeclaration => {
            Some(context.ast[context.ast.cast::<FieldDeclaration>(node)?].metadata)
        }
        NodeKind::VariableDeclaration => {
            Some(context.ast[context.ast.cast::<VariableDeclaration>(node)?].metadata)
        }
        NodeKind::RegularFormalParameter => {
            Some(context.ast[context.ast.cast::<RegularFormalParameter>(node)?].metadata)
        }
        NodeKind::FieldFormalParameter => {
            Some(context.ast[context.ast.cast::<FieldFormalParameter>(node)?].metadata)
        }
        NodeKind::SuperFormalParameter => {
            Some(context.ast[context.ast.cast::<SuperFormalParameter>(node)?].metadata)
        }
        _ => None,
    }
}
fn has_deprecated(context: &LinterContext<'_>, node: NodeId) -> bool {
    metadata(context, node)
        .is_some_and(|m| super::deprecated_consistency::has_deprecated(context, m))
}
fn deprecation_message(context: &LinterContext<'_>, node: NodeId) -> Option<Option<String>> {
    let metadata = metadata(context, node)?;
    let resolved = context.resolved?;
    for &annotation in context.ast.list(metadata) {
        let Some(element) = context.element(annotation) else {
            continue;
        };
        let base = dartr_typesystem::member::base_element(&resolved.ctx, element);
        let annotation_name = if base.is::<dartr_element::ConstructorElement>() {
            resolved
                .ctx
                .element_data(base)
                .and_then(|data| data.enclosing)
                .and_then(|enclosing| resolved.ctx.element_name(enclosing))
        } else {
            resolved.ctx.element_name(base)
        };
        let is_deprecated = matches!(
            (
                annotation_name,
                dartr_typesystem::member::library(&resolved.ctx, element)
                    .and_then(|library| resolved.ctx.element_name(library.raw())),
            ),
            (Some("deprecated" | "Deprecated"), Some("dart.core"))
        );
        if !is_deprecated {
            continue;
        }
        let annotation = &context.ast[annotation];
        let message = annotation.arguments.and_then(|arguments| {
            let arguments = context.ast.list(context.ast[arguments].arguments);
            let argument = (arguments.len() == 1).then_some(arguments[0].raw())?;
            let value = context.constant_value(argument)?;
            normalize_message(value.to_string_value()?)
        });
        return Some(message);
    }
    None
}
fn normalize_message(message: &str) -> Option<String> {
    let message = message.trim();
    if message.is_empty() || message == "." {
        None
    } else if message.ends_with(['.', '?', '!']) {
        Some(message.to_string())
    } else {
        Some(format!("{message}."))
    }
}
fn in_deprecated_declaration(context: &LinterContext<'_>, node: NodeId) -> bool {
    std::iter::successors(context.ast.parent(node), |n| context.ast.parent(*n))
        .any(|n| has_deprecated(context, n))
}

fn is_local_parameter_use(context: &LinterContext<'_>, node: NodeId, element: ElementId) -> bool {
    let Some(parameter) = element.cast::<FormalParameterElement>() else {
        return false;
    };
    let Some(enclosing) = context
        .resolved
        .and_then(|resolved| resolved.ctx.element_data(parameter.raw()))
        .and_then(|data| data.enclosing)
    else {
        return false;
    };
    super::helpers::ancestors(context.ast, node).any(|ancestor| {
        let declaration = match context.ast.kind(ancestor) {
            NodeKind::ConstructorDeclaration
            | NodeKind::FunctionDeclaration
            | NodeKind::FunctionExpression
            | NodeKind::MethodDeclaration
            | NodeKind::PrimaryConstructorDeclaration => ancestor,
            NodeKind::PrimaryConstructorBody => {
                let Some(primary) = super::helpers::ancestors(context.ast, ancestor)
                    .find_map(|candidate| context.ast.cast::<ClassDeclaration>(candidate))
                    .and_then(|class| {
                        context
                            .ast
                            .cast::<PrimaryConstructorDeclaration>(context.ast[class].name_part)
                    })
                else {
                    return false;
                };
                primary.raw()
            }
            _ => return false,
        };
        context.declared_element(declaration) == Some(enclosing)
    })
}

fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    if context.package_root().is_none() {
        return;
    }
    let units = std::iter::once(*context)
        .chain(
            (0..context.resolved_units.len())
                .filter(|&index| index != context.current_unit)
                .filter_map(|index| context.resolved_unit(index)),
        )
        .collect::<Vec<_>>();
    // The upstream frontier starts with the library element. A deprecated
    // library therefore suppresses every use inside all of its units.
    if units.iter().any(|unit| {
        (0..unit.ast.node_count())
            .map(NodeId::from_index)
            .any(|current| {
                unit.ast.kind(current) == NodeKind::LibraryDirective
                    && has_deprecated(unit, current)
            })
    }) {
        return;
    }

    let mut deprecated = IndexMap::<ElementId, Option<String>>::new();
    for unit in units {
        for current in (0..unit.ast.node_count()).map(NodeId::from_index) {
            if let Some(message) = deprecation_message(&unit, current)
                && let Some(element) = unit.declared_element(current)
            {
                deprecated.insert(element, message.clone());
                if let Some(resolved) = unit.resolved {
                    let accessors = if let Some(field) = element.cast::<FieldElement>() {
                        let field = resolved.ctx.get(field);
                        Some((field.getter, field.setter))
                    } else if let Some(variable) = element.cast::<TopLevelVariableElement>() {
                        let variable = resolved.ctx.get(variable);
                        Some((variable.getter, variable.setter))
                    } else {
                        None
                    };
                    if let Some((getter, setter)) = accessors {
                        if let Some(getter) = getter {
                            deprecated.insert(getter.raw(), message.clone());
                        }
                        if let Some(setter) = setter {
                            deprecated.insert(setter.raw(), message.clone());
                        }
                    }
                }
            }
        }
    }
    let mut pending = vec![node];
    while let Some(current) = pending.pop() {
        if !in_deprecated_declaration(context, current)
            && let Some(element) = context
                .element(current)
                .and_then(|e| super::helpers::base_element(context, e))
            && !is_local_parameter_use(context, current, element)
            && let Some(message) = deprecated.get(&element)
        {
            let resolved = context.resolved.expect("resolved element requires context");
            // A constructor and its enclosing class can both be deprecated.
            // The analyzer reports this source site once, through the class
            // reference visited as the constructor's NamedType.
            if let Some(constructor) = element.cast::<ConstructorElement>()
                && resolved
                    .ctx
                    .element_data(constructor.raw())
                    .and_then(|data| data.enclosing)
                    .is_some_and(|enclosing| deprecated.contains_key(&enclosing))
            {
                pending.extend(context.ast.children(current));
                continue;
            }
            let name = if let Some(constructor) = element.cast::<ConstructorElement>() {
                let constructor_name = resolved.ctx.element_name(constructor.raw()).unwrap_or("");
                let class_name = resolved
                    .ctx
                    .element_data(constructor.raw())
                    .and_then(|data| data.enclosing)
                    .and_then(|enclosing| resolved.ctx.element_name(enclosing))
                    .unwrap_or("");
                if constructor_name.is_empty() || constructor_name == "new" {
                    class_name.to_owned()
                } else {
                    format!("{class_name}.{constructor_name}")
                }
            } else {
                resolved.ctx.element_name(element).unwrap_or("").to_owned()
            };
            if let Some(message) = message {
                report_usage(
                    context,
                    out,
                    &diag::DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE_WITH_MESSAGE,
                    current,
                    &[&name, message],
                );
            } else {
                report_usage(
                    context,
                    out,
                    &diag::DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE_WITHOUT_MESSAGE,
                    current,
                    &[&name],
                );
            }
        }
        pending.extend(context.ast.children(current));
    }
}

fn report_usage(
    context: &LinterContext<'_>,
    out: &mut Vec<Diagnostic>,
    code: &'static dartr_diagnostics::DiagnosticCode,
    node: NodeId,
    arguments: &[&str],
) {
    if let Some(named_type) = context.ast.cast::<NamedType>(node) {
        context.report_token(out, code, context.ast[named_type].name, arguments);
    } else {
        context.report_node(out, code, node, arguments);
    }
}
