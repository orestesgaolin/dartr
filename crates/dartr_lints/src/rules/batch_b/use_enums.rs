// Dart source: pkg/linter/lib/src/rules/use_enums.dart
use super::util::*;
use crate::{ExperimentalFlag, LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElementId, FragmentFlags, Tag};
use dartr_typesystem::TypeExt;

pub fn register(r: &mut RuleVisitorRegistry, c: &LinterContext<'_>) {
    if !c.is_feature_enabled(ExperimentalFlag::EnhancedEnums) {
        return;
    }
    r.add(NodeKind::ClassDeclaration, "use_enums", check);
}

fn is_factory(c: &LinterContext<'_>, constructor: ElementId) -> bool {
    flags(c, constructor).contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY)
}

/// Dart `_BaseVisitor.invokesGenerativeConstructor`.
fn invokes_generative_constructor(c: &LinterContext<'_>, node: NodeId, class: ElementId) -> bool {
    let constructor_name = c.ast[Id::<InstanceCreationExpression>::from_raw(node)].constructor_name;
    c.element(constructor_name).is_some_and(|e| {
        let e = base(c, e);
        !is_factory(c, e) && enclosing(c, e) == Some(class)
    })
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let n = &c.ast[Id::<ClassDeclaration>::from_raw(node)];
    if n.augment_keyword.is_some() || n.abstract_keyword.is_some() {
        return;
    }
    let Some(class) = c.declared_element(node) else {
        return;
    };
    let interface = dartr_element::EId::<dartr_element::InterfaceElement>::from_raw(class);
    if ctx
        .element_supertype(interface)
        .is_some_and(|t| !ctx.is_dart_core_object(t))
    {
        return;
    }
    if let Some(primary) = c
        .ast
        .cast::<PrimaryConstructorDeclaration>(n.name_part.raw())
    {
        let Some(constructor) = c.declared_element(primary) else {
            return;
        };
        if !flags(c, constructor).contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST) {
            return;
        }
        let name = c.ast[primary]
            .constructor_name
            .map(|cn| lexeme(c, c.ast[cn].name));
        if is_public(c, class) && name.is_none_or(|n| !n.starts_with('_')) {
            return;
        }
        for parameter in parameters(c, Some(c.ast[primary].formal_parameters)) {
            if matches!(
                super::prefer_iterable_wheretype::parameter_name(c, parameter),
                Some("hashCode" | "index" | "values")
            ) {
                return;
            }
        }
    }
    if kind(c, n.body) != NodeKind::BlockClassBody {
        return;
    }
    let mut candidates: Vec<NodeId> = Vec::new();
    for member in super::sort_unnamed_constructors_first::body_members(c, node) {
        if has_field_or_method(c, member, "hashCode")
            || has_field_or_method(c, member, "index")
            || c.ast
                .cast::<MethodDeclaration>(member)
                .is_some_and(|m| lexeme(c, c.ast[m].name) == "==")
            || has_field_or_method(c, member, "values")
        {
            return;
        }
        if let Some(field) = c.ast.cast::<FieldDeclaration>(member) {
            let f = &c.ast[field];
            if f.static_keyword.is_none() {
                continue;
            }
            let is_const = c.ast[f.fields]
                .keyword
                .is_some_and(|k| lexeme(c, k) == "const");
            for &variable in c.ast.list(c.ast[f.fields].variables) {
                let Some(field_element) = c.declared_element(variable) else {
                    continue;
                };
                if field_element.tag() != Tag::Field {
                    continue;
                }
                if c.ast.tokens.get(c.ast[variable].name).is_synthetic() || !is_const {
                    continue;
                }
                let Some(initializer) = c.ast[variable].initializer else {
                    continue;
                };
                let Some(creation) = c.ast.cast::<InstanceCreationExpression>(initializer.raw())
                else {
                    continue;
                };
                let Some(constructor) = c
                    .element(c.ast[creation].constructor_name)
                    .map(|e| base(c, e))
                else {
                    continue;
                };
                if is_factory(c, constructor) || enclosing(c, constructor) != Some(class) {
                    continue;
                }
                if element_constant_value(c, field_element).is_none() {
                    continue;
                }
                candidates.push(variable.raw());
            }
        }
        if let Some(constructor) = c.ast.cast::<ConstructorDeclaration>(member) {
            let Some(element) = c.declared_element(member) else {
                return;
            };
            if !is_factory(c, element)
                && !flags(c, element).contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST)
            {
                return;
            }
            let name = c.ast[constructor].name.map(|t| lexeme(c, t));
            if is_public(c, class) && name.is_none_or(|n| !n.starts_with('_')) {
                return;
            }
        }
    }
    if candidates.len() < 2 {
        return;
    }
    // Dart `_EnumVisitor` over the class.
    {
        let mut in_constant_declaration = false;
        let mut stack: Vec<(NodeId, bool)> = vec![(node, false)];
        while let Some((n, exit)) = stack.pop() {
            if exit {
                in_constant_declaration = false;
                continue;
            }
            match kind(c, n) {
                NodeKind::InstanceCreationExpression => {
                    if !in_constant_declaration && invokes_generative_constructor(c, n, class) {
                        return;
                    }
                    in_constant_declaration = false;
                }
                NodeKind::VariableDeclaration => {
                    if candidates.contains(&n) {
                        in_constant_declaration = true;
                    }
                    stack.push((n, true));
                }
                _ => {}
            }
            for child in c.ast.children(n).into_iter().rev() {
                stack.push((child, false));
            }
        }
    }
    // Dart `_NonEnumVisitor` over the unit.
    let root = ancestors(c, node).last().unwrap_or(node);
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        match kind(c, n) {
            NodeKind::ClassDeclaration => {
                let Some(element) = c.declared_element(n) else {
                    return;
                };
                if element == class {
                    continue;
                }
                let e = dartr_element::EId::<dartr_element::InterfaceElement>::from_raw(element);
                let is_class = |t: dartr_element::TypeId| {
                    ctx.interface_element(t).map(|i| i.raw()) == Some(class)
                };
                if ctx.element_supertype(e).is_some_and(is_class)
                    || ctx.element_interfaces(e).iter().any(|&t| is_class(t))
                    || ctx.element_mixins(e).iter().any(|&t| is_class(t))
                {
                    return;
                }
            }
            NodeKind::InstanceCreationExpression if invokes_generative_constructor(c, n, class) => {
                return;
            }
            _ => {}
        }
        stack.extend(c.ast.children(n).into_iter().rev());
    }
    let (offset, length) = node_to_annotate(c, node);
    c.report_offset(out, &diag::USE_ENUMS, offset, length, &[]);
}
