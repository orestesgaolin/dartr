// Dart source: pkg/linter/lib/src/rules/invalid_runtime_check_with_js_interop_types.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, DiagnosticCode, diag};
use dartr_element::{
    ClassElement, ConstructorElement, ElementId, ExtensionTypeElement, TypeId, TypeKind,
};
use dartr_typesystem::{TypeExt, member};
use indexmap::IndexSet;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::AsExpression,
        "invalid_runtime_check_with_js_interop_types",
        check,
    );
    registry.add(
        NodeKind::CatchClause,
        "invalid_runtime_check_with_js_interop_types",
        check,
    );
    registry.add(
        NodeKind::IsExpression,
        "invalid_runtime_check_with_js_interop_types",
        check,
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InteropKind {
    Dart,
    UserExtension,
    StaticInterop(TypeId),
}
enum Knowledge<T> {
    Known(T),
    Unknown,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Presence {
    No,
    Yes,
    Unknown,
}
enum Search {
    Clean,
    Found(&'static DiagnosticCode),
    Unknown,
}

fn declaration_metadata<'a>(
    context: &LinterContext<'a>,
    element: ElementId,
) -> Option<(LinterContext<'a>, NodeList<Annotation>)> {
    for unit in std::iter::once(*context).chain(
        (0..context.resolved_units.len())
            .filter(|&index| index != context.current_unit)
            .filter_map(|index| context.resolved_unit(index)),
    ) {
        let declaration = (0..unit.ast.node_count())
            .map(NodeId::from_index)
            .find(|&node| unit.declared_element(node) == Some(element));
        let Some(class) = declaration.and_then(|node| unit.ast.cast::<ClassDeclaration>(node))
        else {
            continue;
        };
        return Some((unit, unit.ast[class].metadata));
    }
    None
}

fn annotation_is(
    context: &LinterContext<'_>,
    annotation: Id<Annotation>,
    expected_library: &str,
    name: &str,
) -> bool {
    let Some(resolved) = context.resolved else {
        return false;
    };
    context.element(annotation).is_some_and(|element| {
        let base = member::base_element(&resolved.ctx, element);
        let annotation_name = if base.is::<ConstructorElement>() {
            resolved
                .ctx
                .element_data(base)
                .and_then(|data| data.enclosing)
                .and_then(|enclosing| resolved.ctx.element_name(enclosing))
        } else {
            resolved.ctx.element_name(base)
        };
        annotation_name == Some(name)
            && member::library(&resolved.ctx, element)
                .is_some_and(|library| resolved.ctx.library_uri(library) == expected_library)
    })
}

/// Returns the JSObject representation, a known non-static result, or unknown
/// when linked annotations cannot be inspected from this unit.
fn static_interop_representation(
    context: &LinterContext<'_>,
    element: dartr_element::EId<ClassElement>,
) -> Knowledge<Option<TypeId>> {
    let Some(resolved) = context.resolved else {
        return Knowledge::Unknown;
    };
    let Some((declaration_context, metadata)) = declaration_metadata(context, element.raw()) else {
        let has_metadata = resolved
            .ctx
            .fragment_data(resolved.ctx.get(element).first_fragment)
            .is_some_and(|fragment| !fragment.metadata.annotations.is_empty());
        return if has_metadata {
            Knowledge::Unknown
        } else {
            Knowledge::Known(None)
        };
    };
    let mut js_library = None;
    let mut has_static_interop = false;
    for &annotation in declaration_context.ast.list(metadata) {
        if annotation_is(&declaration_context, annotation, "dart:js_interop", "JS") {
            js_library = declaration_context
                .element(annotation)
                .and_then(|element| member::library(&resolved.ctx, element));
        } else if annotation_is(
            &declaration_context,
            annotation,
            "dart:js_interop",
            "staticInterop",
        ) {
            has_static_interop = true;
        }
    }
    if !has_static_interop {
        return Knowledge::Known(None);
    }
    let Some(library) = js_library else {
        return Knowledge::Known(None);
    };
    let representation = resolved
        .ctx
        .get(library)
        .extension_types
        .iter()
        .find(|&&element| resolved.ctx.element_name(element.raw()) == Some("JSObject"))
        .map(|&element| resolved.ctx.interface_this_type(element.upcast()));
    Knowledge::Known(representation)
}

fn direct_interop_kind(
    context: &LinterContext<'_>,
    ty: TypeId,
    visited: &mut IndexSet<TypeId>,
) -> Knowledge<Option<InteropKind>> {
    if !visited.insert(ty) {
        return Knowledge::Known(None);
    }
    let Some(resolved) = context.resolved else {
        return Knowledge::Unknown;
    };
    match *resolved.ctx.ty(ty) {
        TypeKind::TypeParameter { param, .. } => resolved
            .ctx
            .get(param)
            .bound
            .get()
            .map_or(Knowledge::Known(None), |bound| {
                direct_interop_kind(context, bound, visited)
            }),
        TypeKind::Interface { element, .. } => {
            if let Some(extension) = element.raw().cast::<ExtensionTypeElement>() {
                if resolved.ctx.element_library_uri(extension.raw()) == Some("dart:js_interop") {
                    return Knowledge::Known(Some(InteropKind::Dart));
                }
                let Some(representation) = resolved.ctx.representation_type(ty) else {
                    return Knowledge::Known(None);
                };
                return match direct_interop_kind(context, representation, visited) {
                    Knowledge::Known(Some(_)) => Knowledge::Known(Some(InteropKind::UserExtension)),
                    Knowledge::Known(None) => Knowledge::Known(None),
                    Knowledge::Unknown => Knowledge::Unknown,
                };
            }
            if let Some(class) = element.raw().cast::<ClassElement>() {
                return match static_interop_representation(context, class) {
                    Knowledge::Known(Some(representation)) => {
                        Knowledge::Known(Some(InteropKind::StaticInterop(representation)))
                    }
                    Knowledge::Known(None) => Knowledge::Known(None),
                    Knowledge::Unknown => Knowledge::Unknown,
                };
            }
            Knowledge::Known(None)
        }
        _ => Knowledge::Known(None),
    }
}

fn presence(context: &LinterContext<'_>, ty: TypeId, visited: &mut IndexSet<TypeId>) -> Presence {
    if !visited.insert(ty) {
        return Presence::No;
    }
    match direct_interop_kind(context, ty, &mut IndexSet::new()) {
        Knowledge::Known(Some(_)) => return Presence::Yes,
        Knowledge::Unknown => return Presence::Unknown,
        Knowledge::Known(None) => {}
    }
    let Some(resolved) = context.resolved else {
        return Presence::Unknown;
    };
    let children: Vec<TypeId> = match *resolved.ctx.ty(ty) {
        TypeKind::Interface { args, .. } => resolved.ctx.list(args).to_vec(),
        TypeKind::Function(function) => {
            let mut children = vec![function.ret];
            for &parameter in resolved.ctx.list(function.type_params) {
                if let Some(bound) = resolved.ctx.get(parameter).bound.get() {
                    children.push(bound);
                }
            }
            children.extend(
                resolved
                    .ctx
                    .list(function.params)
                    .iter()
                    .map(|parameter| parameter.ty),
            );
            children
        }
        TypeKind::Record {
            positional, named, ..
        } => resolved
            .ctx
            .list(positional)
            .iter()
            .copied()
            .chain(resolved.ctx.list(named).iter().map(|field| field.ty))
            .collect(),
        TypeKind::TypeParameter { param, .. } => {
            resolved.ctx.get(param).bound.get().into_iter().collect()
        }
        _ => vec![],
    };
    let mut unknown = false;
    for child in children {
        match presence(context, child, visited) {
            Presence::Yes => return Presence::Yes,
            Presence::Unknown => unknown = true,
            Presence::No => {}
        }
    }
    if unknown {
        Presence::Unknown
    } else {
        Presence::No
    }
}

fn wasm_incompatible(context: &LinterContext<'_>, ty: TypeId) -> bool {
    let Some(resolved) = context.resolved else {
        return false;
    };
    if let TypeKind::TypeParameter { param, .. } = *resolved.ctx.ty(ty) {
        return resolved
            .ctx
            .get(param)
            .bound
            .get()
            .is_some_and(|bound| wasm_incompatible(context, bound));
    }
    let Some(element) = resolved.ctx.interface_element(ty) else {
        return false;
    };
    matches!(
        resolved.ctx.element_library_uri(element.raw()),
        Some(
            "dart:html"
                | "dart:indexed_db"
                | "dart:svg"
                | "dart:web_audio"
                | "dart:web_gl"
                | "dart:js"
        )
    )
}

fn erased_type(context: &LinterContext<'_>, ty: TypeId, kind: Option<InteropKind>) -> TypeId {
    let ts = context.type_system().expect("resolved lint context");
    if let Some(resolved) = context.resolved
        && let TypeKind::TypeParameter { param, .. } = *resolved.ctx.ty(ty)
        && let Some(bound) = resolved.ctx.get(param).bound.get()
        && bound != ty
    {
        return erased_type(context, bound, kind);
    }
    match kind {
        Some(InteropKind::Dart) => ts.promote_to_non_null(ty),
        Some(InteropKind::StaticInterop(js_object)) => ts.promote_to_non_null(js_object),
        Some(InteropKind::UserExtension) | None => {
            ts.promote_to_non_null(ts.extension_type_erasure(ty))
        }
    }
}
fn kept_user_type(context: &LinterContext<'_>, ty: TypeId, kind: Option<InteropKind>) -> TypeId {
    let ts = context.type_system().expect("resolved lint context");
    if let Some(resolved) = context.resolved
        && let TypeKind::TypeParameter { param, .. } = *resolved.ctx.ty(ty)
        && let Some(bound) = resolved.ctx.get(param).bound.get()
        && bound != ty
    {
        return kept_user_type(context, bound, kind);
    }
    if kind.is_some() {
        ts.promote_to_non_null(ty)
    } else {
        ts.promote_to_non_null(ts.extension_type_erasure(ty))
    }
}

fn invalid_leaf(context: &LinterContext<'_>, left: TypeId, right: TypeId, check: bool) -> Search {
    let left_kind = match direct_interop_kind(context, left, &mut IndexSet::new()) {
        Knowledge::Known(kind) => kind,
        Knowledge::Unknown => return Search::Unknown,
    };
    let right_kind = match direct_interop_kind(context, right, &mut IndexSet::new()) {
        Knowledge::Known(kind) => kind,
        Knowledge::Unknown => return Search::Unknown,
    };
    if left_kind.is_none() && right_kind.is_none() {
        return Search::Clean;
    }
    if wasm_incompatible(context, left) || wasm_incompatible(context, right) {
        return Search::Clean;
    }
    let Some(resolved) = context.resolved else {
        return Search::Unknown;
    };
    let ts = context.type_system().expect("resolved lint context");
    let erased_left = erased_type(context, left, left_kind);
    let erased_right = erased_type(context, right, right_kind);
    let left_subtype = ts.is_subtype_of(erased_left, erased_right);
    let right_subtype = ts.is_subtype_of(erased_right, erased_left);
    let left_dynamic = matches!(resolved.ctx.ty(erased_left), TypeKind::Dynamic);
    let right_dynamic = matches!(resolved.ctx.ty(erased_right), TypeKind::Dynamic);
    if check {
        if !left_subtype && !right_dynamic {
            return Search::Found(if left_kind.is_some() && right_kind.is_some() {
                &diag::INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_JS_IS_INCONSISTENT_JS
            } else if left_kind.is_some() {
                &diag::INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_JS_IS_DART
            } else {
                &diag::INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_DART_IS_JS
            });
        }
        if left_subtype
            && left_kind.is_some()
            && matches!(
                right_kind,
                Some(InteropKind::UserExtension | InteropKind::StaticInterop(_))
            )
            && !ts.is_subtype_of(
                kept_user_type(context, left, left_kind),
                kept_user_type(context, right, right_kind),
            )
        {
            return Search::Found(
                &diag::INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_JS_IS_UNRELATED_JS,
            );
        }
    } else if !left_subtype && !right_subtype && !left_dynamic && !right_dynamic {
        return Search::Found(if left_kind.is_some() && right_kind.is_some() {
            &diag::INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_JS_AS_INCOMPATIBLE_JS
        } else if left_kind.is_some() {
            &diag::INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_JS_AS_DART
        } else {
            &diag::INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_DART_AS_JS
        });
    }
    Search::Clean
}

fn merge(result: Search, next: Search) -> Search {
    match (result, next) {
        (Search::Found(code), _) | (_, Search::Found(code)) => Search::Found(code),
        (Search::Unknown, _) | (_, Search::Unknown) => Search::Unknown,
        _ => Search::Clean,
    }
}

fn invalid_test_inner(
    context: &LinterContext<'_>,
    left: TypeId,
    right: TypeId,
    check: bool,
    visited: &mut IndexSet<(TypeId, TypeId)>,
) -> Search {
    if !visited.insert((left, right)) {
        return Search::Clean;
    }
    let mut result = invalid_leaf(context, left, right, check);
    if matches!(result, Search::Found(_)) {
        return result;
    }
    let Some(resolved) = context.resolved else {
        return Search::Unknown;
    };
    let mut pairs = vec![];
    match (*resolved.ctx.ty(left), *resolved.ctx.ty(right)) {
        (TypeKind::Interface { element: le, .. }, TypeKind::Interface { element: re, .. }) => {
            if let Some(aligned_left) = (le == re)
                .then_some(left)
                .or_else(|| resolved.ctx.as_instance_of(left, re))
            {
                pairs.extend(
                    resolved
                        .ctx
                        .type_arguments(aligned_left)
                        .iter()
                        .copied()
                        .zip(resolved.ctx.type_arguments(right).iter().copied()),
                );
            } else if let Some(aligned_right) = resolved.ctx.as_instance_of(right, le) {
                pairs.extend(
                    resolved
                        .ctx
                        .type_arguments(left)
                        .iter()
                        .copied()
                        .zip(resolved.ctx.type_arguments(aligned_right).iter().copied()),
                );
            }
        }
        (TypeKind::Function(left), TypeKind::Function(right)) => {
            pairs.push((left.ret, right.ret));
            pairs.extend(
                resolved
                    .ctx
                    .list(left.params)
                    .iter()
                    .map(|p| p.ty)
                    .zip(resolved.ctx.list(right.params).iter().map(|p| p.ty)),
            );
        }
        (
            TypeKind::Record {
                positional: lp,
                named: ln,
                ..
            },
            TypeKind::Record {
                positional: rp,
                named: rn,
                ..
            },
        ) => {
            pairs.extend(
                resolved
                    .ctx
                    .list(lp)
                    .iter()
                    .copied()
                    .zip(resolved.ctx.list(rp).iter().copied()),
            );
            let right_named = resolved.ctx.list(rn);
            for field in resolved.ctx.list(ln) {
                if let Some(other) = right_named.iter().find(|other| other.name == field.name) {
                    pairs.push((field.ty, other.ty));
                }
            }
        }
        (TypeKind::TypeParameter { param, .. }, _) => {
            if let Some(bound) = resolved.ctx.get(param).bound.get() {
                pairs.push((bound, right));
            }
        }
        (_, TypeKind::TypeParameter { param, .. }) => {
            if let Some(bound) = resolved.ctx.get(param).bound.get() {
                pairs.push((left, bound));
            }
        }
        _ => {}
    }
    let had_pairs = !pairs.is_empty();
    for (left, right) in pairs {
        result = merge(
            result,
            invalid_test_inner(context, left, right, check, visited),
        );
        if matches!(result, Search::Found(_)) {
            return result;
        }
    }
    if matches!(result, Search::Clean) {
        let lp = presence(context, left, &mut IndexSet::new());
        let rp = presence(context, right, &mut IndexSet::new());
        if matches!(lp, Presence::Unknown)
            || matches!(rp, Presence::Unknown)
            || (!had_pairs && (matches!(lp, Presence::Yes) || matches!(rp, Presence::Yes)))
        {
            // The remaining positional comparison requires canBeSubtypeOf,
            // which is not ported by the type system yet.
            return Search::Unknown;
        }
    }
    result
}

fn invalid_test(
    context: &LinterContext<'_>,
    left: TypeId,
    right: TypeId,
    check: bool,
) -> Option<&'static DiagnosticCode> {
    match invalid_test_inner(context, left, right, check, &mut IndexSet::new()) {
        Search::Found(code) => Some(code),
        Search::Clean | Search::Unknown => None,
    }
}

fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match context.ast.kind(node) {
        NodeKind::CatchClause => {
            let clause = &context.ast[context.ast.cast::<CatchClause>(node).unwrap()];
            let Some(annotation) = clause.exception_type else {
                return;
            };
            let Some(ty) = super::helpers::annotation_type(context, annotation.raw()) else {
                return;
            };
            if matches!(presence(context, ty, &mut IndexSet::new()), Presence::Yes) {
                let text = super::helpers::type_text(context, ty);
                context.report_node(
                    out,
                    &diag::INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_CATCH_CLAUSE_JS_INTEROP_TYPE,
                    annotation,
                    &[&text],
                );
            }
        }
        NodeKind::AsExpression => {
            let expression = &context.ast[context.ast.cast::<AsExpression>(node).unwrap()];
            let (Some(left), Some(right)) = (
                context.static_type(expression.expression),
                super::helpers::annotation_type(context, expression.type_.raw()),
            ) else {
                return;
            };
            if let Some(code) = invalid_test(context, left, right, false) {
                let l = super::helpers::type_text(context, left);
                let r = super::helpers::type_text(context, right);
                context.report_node(out, code, node, &[&l, &r]);
            }
        }
        NodeKind::IsExpression => {
            let expression = &context.ast[context.ast.cast::<IsExpression>(node).unwrap()];
            let (Some(left), Some(right)) = (
                context.static_type(expression.expression),
                super::helpers::annotation_type(context, expression.type_.raw()),
            ) else {
                return;
            };
            if let Some(code) = invalid_test(context, left, right, true) {
                let l = super::helpers::type_text(context, left);
                let r = super::helpers::type_text(context, right);
                context.report_node(out, code, node, &[&l, &r]);
            }
        }
        _ => {}
    }
}
