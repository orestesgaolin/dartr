// Dart source: pkg/linter/lib/src/rules/analyzer_public_api.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{
    AnyElement, DirectiveUri, EId, ElementId, ExecutableElement, ExtensionElement, FragmentFlags,
    InterfaceElement, LibraryElement, LibraryFragment, NamespaceCombinator,
    PropertyInducingElement, TypeId, TypeKind, TypeParameterElement,
};
use dartr_typesystem::{TypeExt, member};
use indexmap::IndexSet;

use super::helpers::{KnownAnnotation, annotation_status, descendants};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(NodeKind::CompilationUnit, "analyzer_public_api", check);
}

fn is_analyzer_public_uri(uri: &str) -> bool {
    uri.starts_with("package:analyzer/") && !uri.starts_with("package:analyzer/src/")
}

fn is_public_package_uri(uri: &str) -> bool {
    let Some(rest) = uri.strip_prefix("package:") else {
        return false;
    };
    !rest
        .split('/')
        .nth(1)
        .is_some_and(|segment| segment == "src")
}

fn directive_library(uri: &DirectiveUri) -> Option<EId<LibraryElement>> {
    match uri {
        DirectiveUri::Library { library, .. } => Some(*library),
        _ => None,
    }
}

fn directive_source_uri(uri: &DirectiveUri) -> Option<&str> {
    match uri {
        DirectiveUri::Source { source, .. } | DirectiveUri::Library { source, .. } => {
            Some(&source.uri)
        }
        _ => None,
    }
}

fn current_fragment<'a>(c: &LinterContext<'a>) -> Option<&'a LibraryFragment> {
    let r = c.resolved?;
    dartr_link::dump::fragments(&r.ctx, r.library.raw())
        .into_iter()
        .filter_map(|fragment| fragment.cast::<LibraryFragment>())
        .map(|fragment| r.ctx.fragment(fragment))
        .find(|fragment| fragment.source.path.as_ref() == c.path)
}

fn blocks(c: &LinterContext<'_>, combinators: &[NamespaceCombinator], name: &str) -> bool {
    combinators.iter().any(|combinator| match combinator {
        NamespaceCombinator::Hide { hidden_names, .. } => hidden_names
            .iter()
            .any(|hidden| c.resolved.is_some_and(|r| r.ctx.name_str(*hidden) == name)),
        NamespaceCombinator::Show { shown_names, .. } => !shown_names
            .iter()
            .any(|shown| c.resolved.is_some_and(|r| r.ctx.name_str(*shown) == name)),
    })
}

fn publicly_imported(c: &LinterContext<'_>, element: ElementId) -> bool {
    let Some(r) = c.resolved else { return false };
    let Some(name) = r.ctx.element_name(element) else {
        return false;
    };
    let lookup_name = name.strip_suffix('=').unwrap_or(name);
    let Some(fragment) = current_fragment(c) else {
        return false;
    };
    fragment.library_imports.iter().any(|import| {
        let Some(library) = directive_library(&import.directive.uri) else {
            return false;
        };
        if !is_public_package_uri(r.ctx.library_uri(library))
            || blocks(c, &import.combinators, lookup_name)
        {
            return false;
        }
        r.ctx
            .get(library)
            .export_namespace
            .try_get()
            .and_then(|namespace| namespace.defined_names.get(&r.ctx.name(name)))
            .is_some_and(|imported| {
                member::base_element(&r.ctx, dartr_element::ElemRef::Base(*imported))
                    == member::base_element(&r.ctx, dartr_element::ElemRef::Base(element))
            })
    })
}

fn library_children(c: &LinterContext<'_>, library: EId<LibraryElement>) -> Vec<ElementId> {
    let Some(r) = c.resolved else {
        return Vec::new();
    };
    let library = r.ctx.get(library);
    let mut result = Vec::new();
    result.extend(library.classes.iter().map(|element| element.raw()));
    result.extend(library.enums.iter().map(|element| element.raw()));
    result.extend(library.extensions.iter().map(|element| element.raw()));
    result.extend(library.extension_types.iter().map(|element| element.raw()));
    result.extend(library.getters.iter().map(|element| element.raw()));
    result.extend(library.setters.iter().map(|element| element.raw()));
    result.extend(library.mixins.iter().map(|element| element.raw()));
    result.extend(
        library
            .top_level_functions
            .iter()
            .map(|element| element.raw()),
    );
    result.extend(
        library
            .top_level_variables
            .iter()
            .map(|element| element.raw()),
    );
    result.extend(library.type_aliases.iter().map(|element| element.raw()));
    result
}

/// Dart `_isPublicApiAnnotation`: the constant value of the annotation has
/// the type `AnalyzerPublicApi`.
fn has_public_api_annotation(c: &LinterContext<'_>, element: ElementId) -> bool {
    let Some(r) = c.resolved else { return false };
    let Some(metadata) = r.metadata else {
        return false;
    };
    metadata.annotations(element).into_iter().any(|annotation| {
        metadata.annotation_value(annotation).is_some_and(|value| {
            r.ctx
                .interface_element(value.ty)
                .is_some_and(|e| r.ctx.element_name(e.raw()) == Some("AnalyzerPublicApi"))
        })
    })
}

fn first_fragment_flags(c: &LinterContext<'_>, element: ElementId) -> FragmentFlags {
    let Some(r) = c.resolved else {
        return FragmentFlags::EMPTY;
    };
    r.ctx
        .element_data(element)
        .and_then(|d| r.ctx.fragment_data(d.first_fragment))
        .map(|f| f.flags.get())
        .unwrap_or_default()
}

/// Dart `Element.isInAnalyzerPublicApi`.
fn is_in_analyzer_public_api(c: &LinterContext<'_>, element: ElementId) -> bool {
    let Some(r) = c.resolved else { return false };
    match r.ctx.any(element) {
        AnyElement::Getter(_) | AnyElement::Setter(_)
            if first_fragment_flags(c, element)
                .contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE) =>
        {
            let variable = r
                .ctx
                .property_accessor(EId::from_raw(element))
                .variable
                .get()
                .map(|v| v.raw());
            if variable.is_some_and(|v| is_in_analyzer_public_api(c, v)) {
                return true;
            }
        }
        AnyElement::Field(_) | AnyElement::TopLevelVariable(_) => {
            let flags = first_fragment_flags(c, element);
            let origin_getter_setter = !flags
                .contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION)
                && !flags
                    .contains(FragmentFlags::FIELD_FRAGMENT_IS_ORIGIN_DECLARING_FORMAL_PARAMETER);
            if origin_getter_setter {
                let data = r.ctx.property_inducing(EId::from_raw(element));
                for accessor in [data.getter.map(|g| g.raw()), data.setter.map(|s| s.raw())]
                    .into_iter()
                    .flatten()
                {
                    if is_in_analyzer_public_api(c, accessor) {
                        return true;
                    }
                }
            }
        }
        _ => {}
    }
    if has_public_api_annotation(c, element) {
        return true;
    }
    if r.ctx
        .element_name(element)
        .is_some_and(|n| n.starts_with('_'))
    {
        return false;
    }
    r.ctx
        .element_data(element)
        .and_then(|d| d.library)
        .is_some_and(|l| is_analyzer_public_uri(r.ctx.library_uri(l)))
}

/// Dart `Element.isOkForAnalyzerPublicApi`.
fn is_ok_for_analyzer_public_api(c: &LinterContext<'_>, element: ElementId) -> bool {
    let Some(r) = c.resolved else { return false };
    if matches!(r.ctx.any(element), AnyElement::Dynamic | AnyElement::Never)
        || element.tag() == dartr_element::Tag::TypeParameter
    {
        return true;
    }
    let Some(library) = r.ctx.element_data(element).and_then(|data| data.library) else {
        return false;
    };
    r.ctx.library_uri(library).starts_with("dart:") || is_in_analyzer_public_api(c, element)
}

fn type_problems(
    c: &LinterContext<'_>,
    ty: TypeId,
    visited: &mut IndexSet<TypeId>,
    problems: &mut IndexSet<String>,
) {
    let Some(r) = c.resolved else { return };
    if !visited.insert(ty) {
        return;
    }
    match *r.ctx.ty(ty) {
        TypeKind::Interface { element, args, .. } => {
            if !publicly_imported(c, element.raw())
                && !is_ok_for_analyzer_public_api(c, element.raw())
                && let Some(name) = r.ctx.element_name(element.raw())
            {
                problems.insert(name.to_owned());
            }
            for &argument in r.ctx.list(args) {
                type_problems(c, argument, visited, problems);
            }
        }
        TypeKind::Function(function) => {
            type_problems(c, function.ret, visited, problems);
            for parameter in r.ctx.list(function.params) {
                type_problems(c, parameter.ty, visited, problems);
            }
            for &parameter in r.ctx.list(function.type_params) {
                if let Some(bound) = r.ctx.get(parameter).bound.get() {
                    type_problems(c, bound, visited, problems);
                }
            }
        }
        TypeKind::Record {
            positional, named, ..
        } => {
            for &field in r.ctx.list(positional) {
                type_problems(c, field, visited, problems);
            }
            for field in r.ctx.list(named) {
                type_problems(c, field.ty, visited, problems);
            }
        }
        TypeKind::Dynamic
        | TypeKind::Void
        | TypeKind::Invalid
        | TypeKind::Unknown
        | TypeKind::Never(_)
        | TypeKind::TypeParameter { .. } => {}
    }
}

fn add_type_parameter_bounds(
    c: &LinterContext<'_>,
    parameters: &[EId<TypeParameterElement>],
    out: &mut Vec<TypeId>,
) {
    let Some(r) = c.resolved else { return };
    for parameter in parameters {
        if let Some(bound) = r.ctx.get(*parameter).bound.get() {
            out.push(bound);
        }
    }
}

fn exposed_types(c: &LinterContext<'_>, element: ElementId) -> Vec<TypeId> {
    let Some(r) = c.resolved else {
        return Vec::new();
    };
    let mut result = Vec::new();
    if let Some(interface) = element.cast::<InterfaceElement>() {
        let data = r.ctx.interface(interface);
        add_type_parameter_bounds(c, &data.type_params, &mut result);
        if let Some(supertype) = data.supertype.get() {
            result.push(supertype);
        }
        if let Some(mixins) = data.mixins.get() {
            result.extend_from_slice(r.ctx.list(mixins));
        }
        if let Some(interfaces) = data.interfaces.get() {
            result.extend_from_slice(r.ctx.list(interfaces));
        }
        result.extend_from_slice(r.ctx.element_superclass_constraints(interface));
    } else if let Some(extension) = element.cast::<ExtensionElement>() {
        let data = r.ctx.get(extension);
        add_type_parameter_bounds(c, &data.type_params, &mut result);
        if let Some(extended) = data.extended_type.get() {
            result.push(extended);
        }
    } else if let Some(executable) = element.cast::<ExecutableElement>() {
        let data = r.ctx.executable(executable);
        if let Some(ty) = data.type_.get() {
            result.push(ty);
        }
    } else if let Some(variable) = element.cast::<PropertyInducingElement>() {
        if let Some(ty) = r.ctx.property_inducing(variable).type_.get() {
            result.push(ty);
        }
    } else if let Some(alias) = element.cast::<dartr_element::TypeAliasElement>() {
        let data = r.ctx.get(alias);
        add_type_parameter_bounds(c, &data.type_params, &mut result);
        if let Some(ty) = data.aliased_type.get() {
            result.push(ty);
        }
    }
    result
}

fn class_name_part_token(
    c: &LinterContext<'_>,
    part: Id<ClassNamePart>,
) -> Option<dartr_syntax::TokenId> {
    c.ast
        .cast::<NameWithTypeParameters>(part)
        .map(|name| c.ast[name].type_name)
        .or_else(|| {
            c.ast
                .cast::<PrimaryConstructorDeclaration>(part)
                .map(|primary| c.ast[primary].type_name)
        })
}

fn name_token(c: &LinterContext<'_>, node: NodeId) -> Option<dartr_syntax::TokenId> {
    match c.ast.kind(node) {
        NodeKind::ClassDeclaration => {
            super::helpers::class_name_token(c.ast, Id::<ClassDeclaration>::from_raw(node))
        }
        NodeKind::EnumDeclaration => {
            class_name_part_token(c, c.ast[Id::<EnumDeclaration>::from_raw(node)].name_part)
        }
        NodeKind::MixinDeclaration => Some(c.ast[Id::<MixinDeclaration>::from_raw(node)].name),
        NodeKind::ExtensionDeclaration => c.ast[Id::<ExtensionDeclaration>::from_raw(node)].name,
        NodeKind::ExtensionTypeDeclaration => class_name_part_token(
            c,
            c.ast[Id::<ExtensionTypeDeclaration>::from_raw(node)].name_part,
        ),
        NodeKind::FunctionDeclaration => {
            Some(c.ast[Id::<FunctionDeclaration>::from_raw(node)].name)
        }
        NodeKind::GenericTypeAlias => Some(c.ast[Id::<GenericTypeAlias>::from_raw(node)].name),
        NodeKind::FunctionTypeAlias => Some(c.ast[Id::<FunctionTypeAlias>::from_raw(node)].name),
        NodeKind::MethodDeclaration => Some(c.ast[Id::<MethodDeclaration>::from_raw(node)].name),
        NodeKind::ConstructorDeclaration => {
            let constructor = &c.ast[Id::<ConstructorDeclaration>::from_raw(node)];
            constructor
                .name
                .or_else(|| constructor.type_name.map(|name| c.ast[name].token))
        }
        NodeKind::VariableDeclaration => {
            Some(c.ast[Id::<VariableDeclaration>::from_raw(node)].name)
        }
        _ => None,
    }
}

fn check_fragment(
    c: &LinterContext<'_>,
    node: NodeId,
    element: ElementId,
    out: &mut Vec<Diagnostic>,
) {
    let Some(token) = name_token(c, node) else {
        return;
    };
    let name = c.ast.tokens.lexeme(token);
    if name.starts_with('_')
        && annotation_status(c, node, KnownAnnotation::AnalyzerPublicApi) != Some(true)
    {
        return;
    }
    if name.ends_with("Impl") {
        c.report_token(
            out,
            &diag::ANALYZER_PUBLIC_API_IMPL_IN_PUBLIC_API,
            token,
            &[],
        );
    }
    let mut problems = IndexSet::new();
    let mut visited = IndexSet::new();
    for ty in exposed_types(c, element) {
        type_problems(c, ty, &mut visited, &mut problems);
    }
    if !problems.is_empty() {
        let joined = problems.into_iter().collect::<Vec<_>>().join(", ");
        c.report_token(out, &diag::ANALYZER_PUBLIC_API_BAD_TYPE, token, &[&joined]);
    }
}

fn check_export(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(r) = c.resolved else { return };
    let export = &c.ast[Id::<ExportDirective>::from_raw(node)];
    let offset = c.ast.tokens.offset(export.export_keyword) as i32;
    let Some(linked) = current_fragment(c).and_then(|fragment| {
        fragment
            .library_exports
            .iter()
            .find(|item| item.export_keyword_offset == offset)
    }) else {
        return;
    };
    let Some(library) = directive_library(&linked.directive.uri) else {
        return;
    };
    let mut bad = IndexSet::new();
    for element in library_children(c, library) {
        let Some(name) = r.ctx.element_name(element) else {
            continue;
        };
        if !name.starts_with('_')
            && !blocks(
                c,
                &linked.combinators,
                name.strip_suffix('=').unwrap_or(name),
            )
            && !is_ok_for_analyzer_public_api(c, element)
        {
            bad.insert(name.trim_end_matches('=').to_owned());
        }
    }
    if !bad.is_empty() {
        let joined = bad.into_iter().collect::<Vec<_>>().join(", ");
        c.report_node(
            out,
            &diag::ANALYZER_PUBLIC_API_EXPORTS_NON_PUBLIC_NAME,
            node,
            &[&joined],
        );
    }
}

fn check_part(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let part = &c.ast[Id::<PartDirective>::from_raw(node)];
    let offset = c.ast.tokens.offset(part.part_keyword) as i32;
    let Some(linked) = current_fragment(c).and_then(|fragment| {
        fragment
            .parts
            .iter()
            .find(|item| item.part_keyword_offset == offset)
    }) else {
        return;
    };
    let target_uri = match &linked.directive.uri {
        DirectiveUri::Unit {
            library_fragment, ..
        } => c
            .resolved
            .map(|r| r.ctx.fragment(*library_fragment).source.uri.as_ref()),
        other => directive_source_uri(other),
    };
    if target_uri.is_some_and(|uri| !is_analyzer_public_uri(uri)) {
        c.report_node(
            out,
            &diag::ANALYZER_PUBLIC_API_BAD_PART_DIRECTIVE,
            node,
            &[],
        );
    }
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let unit = &c.ast[Id::<CompilationUnit>::from_raw(node)];
    // Dart `visitExportDirective` / `visitPartDirective` check only the
    // directives of the analyzer public `lib`; the declarations of any unit
    // are checked when their element is in the analyzer public API.
    let in_public_lib = is_analyzer_public_uri(&c.source_uri());
    for directive in c
        .ast
        .list_raw(unit.directives)
        .iter()
        .filter(|_| in_public_lib)
    {
        match c.ast.kind(*directive) {
            NodeKind::ExportDirective => check_export(c, *directive, out),
            NodeKind::PartDirective => check_part(c, *directive, out),
            _ => {}
        }
    }
    for declaration in c.ast.list_raw(unit.declarations) {
        let mut candidates = vec![*declaration];
        if c.ast.kind(*declaration) == NodeKind::TopLevelVariableDeclaration {
            let variables =
                c.ast[Id::<TopLevelVariableDeclaration>::from_raw(*declaration)].variables;
            candidates.extend(
                c.ast
                    .list(c.ast[variables].variables)
                    .iter()
                    .map(|node| node.raw()),
            );
        }
        let Some(top_element) = c.declared_element(*declaration).or_else(|| {
            candidates
                .iter()
                .find_map(|candidate| c.declared_element(*candidate))
        }) else {
            continue;
        };
        // Dart `_checkTopLevelFragment`: only elements in the analyzer public
        // API are checked (with their members).
        if c.declared_element(*declaration).is_some() {
            if !is_in_analyzer_public_api(c, top_element) {
                continue;
            }
            check_fragment(c, *declaration, top_element, out);
        } else {
            for candidate in candidates.iter().skip(1) {
                if let Some(element) = c.declared_element(*candidate)
                    && is_in_analyzer_public_api(c, element)
                {
                    check_fragment(c, *candidate, element, out);
                }
            }
        }
        let is_enum = c.ast.kind(*declaration) == NodeKind::EnumDeclaration;
        let mut seen = IndexSet::new();
        for child in descendants(c.ast, *declaration) {
            let Some(element) = c.declared_element(child) else {
                continue;
            };
            if element == top_element || !seen.insert(element) {
                continue;
            }
            if c.resolved
                .and_then(|r| r.ctx.element_data(element)?.enclosing)
                != Some(top_element)
            {
                continue;
            }
            if is_enum
                && matches!(
                    c.resolved.map(|r| r.ctx.any(element)),
                    Some(AnyElement::Constructor(_))
                )
            {
                continue;
            }
            check_fragment(c, child, element, out);
        }
    }
}
