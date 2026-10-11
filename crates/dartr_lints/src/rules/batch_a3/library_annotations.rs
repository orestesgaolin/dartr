// Dart source: pkg/linter/lib/src/rules/library_annotations.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::{TypeExt, member};
pub fn register(r: &mut RuleVisitorRegistry) {
    r.add(NodeKind::Annotation, "library_annotations", check);
}

fn checked_top_level_annotation_is_first_directive(
    ctx: &LinterContext<'_>,
    node: NodeId,
) -> Option<bool> {
    let site = ctx.ast.parent(node)?;
    let unit = ctx
        .ast
        .parent(site)
        .and_then(|parent| ctx.ast.cast::<CompilationUnit>(parent))?;
    if ctx.ast.kind(site) == NodeKind::LibraryDirective {
        return None;
    }
    let directives = ctx.ast.list(ctx.ast[unit].directives);
    if directives
        .iter()
        .any(|directive| ctx.ast.kind(*directive) == NodeKind::PartOfDirective)
    {
        return None;
    }
    Some(directives.first().is_some_and(|d| d.raw() == site))
}

fn is_library_only_target(ctx: &LinterContext<'_>, element: dartr_element::ElemRef) -> bool {
    let Some(r) = ctx.resolved else {
        return false;
    };
    let Some(metadata) = r.metadata else {
        return false;
    };
    let base = member::base_element(&r.ctx, element);
    let interface = match base.tag() {
        dartr_element::Tag::Getter => {
            let ty = member::return_type(&r.ctx, base.into());
            match *r.ctx.ty(ty) {
                dartr_element::TypeKind::Interface { element, .. } => element.raw(),
                _ => return false,
            }
        }
        dartr_element::Tag::Constructor => {
            let Some(enclosing) = r.ctx.element_data(base).and_then(|d| d.enclosing) else {
                return false;
            };
            enclosing
        }
        _ => return false,
    };
    if !ctx.has_constructor_annotation(interface, "meta_meta", "Target") {
        return false;
    }
    const TARGET_KINDS: &[&str] = &[
        "classType",
        "constructor",
        "directive",
        "enumType",
        "enumValue",
        "exportDirective",
        "extension",
        "extensionType",
        "field",
        "function",
        "importDirective",
        "getter",
        "library",
        "method",
        "mixinType",
        "optionalParameter",
        "overridableMember",
        "parameter",
        "partOfDirective",
        "setter",
        "topLevelVariable",
        "type",
        "typedefType",
        "typeParameter",
    ];
    for ann in metadata.annotations(interface) {
        let Some(ann_elem) = metadata.annotation_element(ann) else {
            continue;
        };
        if ann_elem.tag() != dartr_element::Tag::Constructor
            || r.ctx
                .element_data(ann_elem)
                .and_then(|d| d.enclosing)
                .and_then(|c| r.ctx.element_name(c))
                != Some("Target")
            || !member::library(&r.ctx, dartr_element::ElemRef::Base(ann_elem))
                .is_some_and(|lib| r.ctx.element_name(lib.raw()) == Some("meta_meta"))
        {
            continue;
        }
        let Some(value) = metadata.annotation_value(ann) else {
            return false;
        };
        let Some(kinds) = value.get_field("kinds").and_then(|v| v.to_set_value()) else {
            return false;
        };
        let mut valid_names: Vec<&str> = Vec::new();
        for elem in kinds.iter() {
            let field = elem.get_field("name").or_else(|| elem.get_field("_name"));
            if let Some(name) = field.and_then(|f| f.to_string_value())
                && TARGET_KINDS.contains(&name)
                && !valid_names.contains(&name)
            {
                valid_names.push(name);
            }
        }
        return valid_names == ["library"];
    }
    false
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    // The Dart visitor checks metadata on non-library directives and top-level
    // declarations only, and it skips part units entirely.
    let Some(is_first_directive) = checked_top_level_annotation_is_first_directive(ctx, node)
    else {
        return;
    };
    let n = &ctx.ast[Id::<Annotation>::from_raw(node)];
    let Some(r) = ctx.resolved else {
        return;
    };
    let Some(e) = ctx.element(node) else {
        return;
    };
    if is_first_directive && is_library_only_target(ctx, e) {
        ctx.report_node(out, &diag::LIBRARY_ANNOTATIONS, node, &[]);
        return;
    }
    let constructor = member::base_element(&r.ctx, e);
    let Some(class) = r.ctx.element_data(constructor).and_then(|d| d.enclosing) else {
        return;
    };
    if !r.ctx.is_element(class, "dart.core", "pragma") {
        return;
    };
    let Some(args) = n.arguments else {
        return;
    };
    let Some(first) = ctx.ast.list(ctx.ast[args].arguments).first() else {
        return;
    };
    let expression = ctx
        .ast
        .cast::<NamedArgument>(*first)
        .map_or(first.raw(), |a| ctx.ast[a].argument_expression.raw());
    if ctx
        .constant_value(expression)
        .and_then(|value| value.to_string_value().map(str::to_owned))
        .as_deref()
        == Some("dart2js:late:trust")
    {
        ctx.report_node(out, &diag::LIBRARY_ANNOTATIONS, node, &[]);
    }
}
