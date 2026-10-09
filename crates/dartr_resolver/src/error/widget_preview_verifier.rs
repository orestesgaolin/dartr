// Dart source: pkg/analyzer/lib/src/error/widget_preview_verifier.dart

//! `WidgetPreviewVerifier`: the applications of the Flutter `@Preview(...)`
//! annotation (`package:flutter/src/widget_previews/widget_previews.dart`)
//! and the private symbols in its arguments. The Flutter helpers it uses
//! (`isWidget`, `isWidgetType`, `isWidgetBuilder`, `isBuildContext`,
//! `isWidgetPreview` of `utilities/extensions/flutter.dart`) are ported
//! here.
//!
//! Difference: the element of the annotation is the one the resolver
//! recorded, else the syntactic resolution of [`crate::element_metadata`].

use dartr_ast::{
    Annotation, ArgumentList, Ast, AstVisitor, BlockClassBody, ClassDeclaration, CompilationUnit,
    ConstructorDeclaration, EnumDeclaration, ExtensionDeclaration, ExtensionTypeDeclaration,
    FormalParameterList, FunctionDeclaration, FunctionDeclarationStatement, Id, MethodDeclaration,
    MixinDeclaration, NamedArgument, NamedType, NodeId, PrimaryConstructorBody,
    PrimaryConstructorDeclaration, SimpleIdentifier,
};
use dartr_diagnostics::diag;
use dartr_element::{
    Ctx, ElementFlags, ElementId, InterfaceElement, Nullability, Tag, TypeId, TypeKind,
};
use dartr_typesystem::TypeExt;

use super::support::{class_name_token, declared_element, node_range};
use super::{UnitVerifier, VerifierHost};
use crate::ast_ext::formal_parameter_parts;
use crate::element_metadata::{AnnotationRef, UnitAst};

const URI_FRAMEWORK: &str = "package:flutter/src/widgets/framework.dart";
const URI_WIDGET_PREVIEWS: &str = "package:flutter/src/widget_previews/widget_previews.dart";

/// Dart `WidgetPreviewVerifier.checkAnnotation(node)`: checks [node] if it
/// is a widget preview application.
pub fn check_annotation(v: &mut UnitVerifier<'_>, node: Id<Annotation>) {
    let unit = UnitAst {
        ast: v.ast,
        tables: v.tables,
    };
    let annotation = AnnotationRef::of_node(&v.ctx, unit, node, v.fragment);
    if is_widget_preview(&v.ctx, annotation.element) {
        check_widget_preview(v, node);
    }
}

/// Dart `ElementAnnotation.isWidgetPreview` (utilities/extensions/flutter.dart).
fn is_widget_preview(ctx: &Ctx<'_>, element: Option<ElementId>) -> bool {
    let Some(element) = element else {
        return false;
    };
    if element.tag() != Tag::Constructor {
        return false;
    }
    let Some(data) = ctx.element_data(element) else {
        return false;
    };
    data.enclosing.and_then(|c| ctx.element_name(c)) == Some("Preview")
        && data
            .library
            .is_some_and(|l| ctx.library_uri(l) == URI_WIDGET_PREVIEWS)
}

/// Dart `_checkWidgetPreview(node)`.
fn check_widget_preview(v: &mut UnitVerifier<'_>, node: Id<Annotation>) {
    let ast = v.ast;
    let Some(arguments) = ast[node].arguments else {
        // This is an invalid annotation application since there's no
        // constructor invocation.
        return;
    };
    let Some(parent) = ast.parent(node) else {
        return;
    };

    let is_valid_application = if !is_supported_parent(ast, parent) {
        // First, check that the preview application is happening in a
        // supported context.
        false
    } else if let Some(c) = ast.cast::<ConstructorDeclaration>(parent) {
        is_valid_constructor_preview_application(
            v,
            parent,
            ast[c].name.map(|n| ast.tokens.lexeme(n)),
            ast.parent(parent).and_then(|p| ast.parent(p)),
            ast[c].external_keyword.is_some(),
            ast[c].factory_keyword.is_some(),
            ast[c].parameters,
        )
    } else if let Some(f) = ast.cast::<FunctionDeclaration>(parent) {
        is_valid_function_preview_application(v, f)
    } else if let Some(m) = ast.cast::<MethodDeclaration>(parent) {
        is_valid_method_preview_application(v, m)
    } else if let Some(body) = ast.cast::<PrimaryConstructorBody>(parent) {
        match super::element_usage_detector::primary_constructor_declaration(ast, body)
            .and_then(|d| ast.cast::<PrimaryConstructorDeclaration>(d))
        {
            Some(declaration) => is_valid_constructor_preview_application(
                v,
                parent,
                ast[declaration]
                    .constructor_name
                    .map(|n| ast.tokens.lexeme(ast[n].name)),
                ast.parent(parent).and_then(|p| ast.parent(p)),
                false,
                false,
                ast[declaration].formal_parameters,
            ),
            None => false,
        }
    } else {
        false
    };

    if !is_valid_application {
        let range = node_range(ast, ast[node].name);
        v.report(
            diag::invalid_widget_preview_application().at_offset(range.0 as usize, range.1 as usize),
        );
    }

    let mut visitor = InvalidWidgetPreviewArgumentDetectorVisitor {
        reports: Vec::new(),
        root_argument: None,
    };
    ast.accept(arguments, &mut visitor);
    for d in visitor.reports {
        v.report(d);
    }
}

/// Dart `_hasRequiredParameters(parameters)`.
fn has_required_parameters(ast: &Ast, parameters: Id<FormalParameterList>) -> bool {
    ast.list(ast[parameters].parameters).iter().any(|&p| {
        matches!(
            formal_parameter_parts(ast, p.raw()).kind,
            dartr_ast::ParameterKind::Required | dartr_ast::ParameterKind::NamedRequired
        )
    })
}

/// Dart `_isPrivateContext(name:, node:)`: whether [name] is private or
/// `node.parent.parent` is a declaration with a private name.
fn is_private_context(ast: &Ast, name: Option<&str>, node: NodeId) -> bool {
    if name.is_some_and(|n| n.starts_with('_')) {
        return true;
    }
    let Some(parent) = ast.parent(node).and_then(|p| ast.parent(p)) else {
        return false;
    };
    let name_token = if let Some(d) = ast.cast::<ClassDeclaration>(parent) {
        Some(class_name_token(ast, ast[d].name_part))
    } else if let Some(d) = ast.cast::<EnumDeclaration>(parent) {
        Some(class_name_token(ast, ast[d].name_part))
    } else if let Some(d) = ast.cast::<ExtensionDeclaration>(parent) {
        ast[d].name
    } else if let Some(d) = ast.cast::<ExtensionTypeDeclaration>(parent) {
        Some(class_name_token(ast, ast[d].name_part))
    } else if let Some(d) = ast.cast::<MixinDeclaration>(parent) {
        Some(ast[d].name)
    } else {
        None
    };
    name_token.is_some_and(|t| ast.tokens.lexeme(t).starts_with('_'))
}

/// Dart `_isSupportedParent(node:)`: previews are supported at the top
/// level of a compilation unit and within classes.
fn is_supported_parent(ast: &Ast, node: NodeId) -> bool {
    let Some(parent) = ast.parent(node) else {
        return false;
    };
    if ast.is::<CompilationUnit>(parent) {
        return true;
    }
    ast.is::<BlockClassBody>(parent)
        && ast
            .parent(parent)
            .is_some_and(|p| ast.is::<ClassDeclaration>(p))
}

/// Dart `_isValidConstructorPreviewApplication(...)`.
fn is_valid_constructor_preview_application(
    v: &UnitVerifier<'_>,
    declaration: NodeId,
    name: Option<&str>,
    parent_declaration: Option<NodeId>,
    is_external: bool,
    is_factory: bool,
    parameters: Id<FormalParameterList>,
) -> bool {
    if is_external {
        return false;
    }
    let ast = v.ast;
    let ctx = v.ctx;
    let Some(parent_declaration) = parent_declaration.filter(|&p| ast.is::<ClassDeclaration>(p))
    else {
        return false;
    };
    let Some(element) =
        declared_element(&ctx, v.tables, parent_declaration).filter(|e| e.tag() == Tag::Class)
    else {
        return false;
    };
    let is_abstract = ctx
        .element_data(element)
        .is_some_and(|d| d.flags.has(ElementFlags::CLASS_ELEMENT_IS_ABSTRACT));
    !is_private_context(ast, name, declaration)
        && is_widget(&ctx, Some(element))
        && !(is_abstract && !is_factory)
        && !has_required_parameters(ast, parameters)
}

/// Dart `_isValidFunctionPreviewApplication(declaration:)`.
fn is_valid_function_preview_application(
    v: &UnitVerifier<'_>,
    declaration: Id<FunctionDeclaration>,
) -> bool {
    let ast = v.ast;
    let Some(return_type) = ast[declaration]
        .return_type
        .and_then(|r| ast.cast::<NamedType>(r))
    else {
        return false;
    };
    let Some(parameters) = ast[ast[declaration].function_expression].parameters else {
        return false;
    };
    let name = ast.tokens.lexeme(ast[declaration].name);
    !is_private_context(ast, Some(name), declaration.raw())
        // Check for nested function.
        && !ast
            .parent(declaration)
            .is_some_and(|p| ast.is::<FunctionDeclarationStatement>(p))
        && ast[declaration].external_keyword.is_none()
        && is_valid_widget_preview_return_type(v, return_type)
        && !has_required_parameters(ast, parameters)
}

/// Dart `_isValidMethodPreviewApplication(declaration:)`.
fn is_valid_method_preview_application(
    v: &UnitVerifier<'_>,
    declaration: Id<MethodDeclaration>,
) -> bool {
    let ast = v.ast;
    let Some(return_type) = ast[declaration]
        .return_type
        .and_then(|r| ast.cast::<NamedType>(r))
    else {
        return false;
    };
    let Some(parameters) = ast[declaration].parameters else {
        return false;
    };
    let is_static = ast[declaration]
        .modifier_keyword
        .is_some_and(|k| ast.tokens.lexeme(k) == "static");
    let name = ast.tokens.lexeme(ast[declaration].name);
    !is_private_context(ast, Some(name), declaration.raw())
        && is_static
        // Check for nested function.
        && !ast
            .parent(declaration)
            .is_some_and(|p| ast.is::<FunctionDeclarationStatement>(p))
        && ast[declaration].external_keyword.is_none()
        && is_valid_widget_preview_return_type(v, return_type)
        && !has_required_parameters(ast, parameters)
}

/// Dart `NamedType.isValidWidgetPreviewReturnType`: `Widget` (or a
/// subtype) or `Widget Function(BuildContext)`.
fn is_valid_widget_preview_return_type(v: &UnitVerifier<'_>, node: Id<NamedType>) -> bool {
    let Some(&ty) = v.tables.annotation_type.get(node.raw()) else {
        return false;
    };
    is_widget_type(&v.ctx, ty) || is_widget_builder(&v.ctx, ty)
}

/// Dart `DartType.isWidgetType`.
fn is_widget_type(ctx: &Ctx<'_>, ty: TypeId) -> bool {
    match *ctx.ty(ty) {
        TypeKind::Interface { element, .. } => is_widget(ctx, Some(element.raw())),
        _ => false,
    }
}

/// Dart `DartType.isWidgetBuilder`.
fn is_widget_builder(ctx: &Ctx<'_>, ty: TypeId) -> bool {
    let TypeKind::Function(f) = *ctx.ty(ty) else {
        return false;
    };
    let params = ctx.list(f.params);
    is_widget_type(ctx, f.ret) && params.len() == 1 && is_build_context(ctx, params[0].ty)
}

/// Dart `DartType.isBuildContext`.
fn is_build_context(ctx: &Ctx<'_>, ty: TypeId) -> bool {
    match *ctx.ty(ty) {
        TypeKind::Interface {
            element,
            nullability: Nullability::None,
            ..
        } => is_exactly(ctx, element.raw(), "BuildContext", URI_FRAMEWORK),
        _ => false,
    }
}

/// Dart `InterfaceElement?.isWidget`: the Flutter class `Widget` or a
/// subtype.
fn is_widget(ctx: &Ctx<'_>, element: Option<ElementId>) -> bool {
    let Some(element) = element.filter(|e| e.tag() == Tag::Class) else {
        return false;
    };
    if is_exactly(ctx, element, "Widget", URI_FRAMEWORK) {
        return true;
    }
    let Some(interface) = element.cast::<InterfaceElement>() else {
        return false;
    };
    ctx.element_all_supertypes(interface).iter().any(|&t| {
        ctx.interface_element(t)
            .is_some_and(|e| is_exactly(ctx, e.raw(), "Widget", URI_FRAMEWORK))
    })
}

/// Dart `_isExactly(type, uri)`.
fn is_exactly(ctx: &Ctx<'_>, element: ElementId, name: &str, uri: &str) -> bool {
    element.tag() == Tag::Class
        && ctx.element_name(element) == Some(name)
        && ctx.element_library_uri(element) == Some(uri)
}

/// Dart `_InvalidWidgetPreviewArgumentDetectorVisitor`: the private
/// identifiers in the named arguments of a preview annotation.
struct InvalidWidgetPreviewArgumentDetectorVisitor {
    reports: Vec<dartr_diagnostics::LocatedDiagnostic>,
    /// Dart `rootArgument`.
    root_argument: Option<Id<NamedArgument>>,
}

impl AstVisitor for InvalidWidgetPreviewArgumentDetectorVisitor {
    fn visit_argument_list(&mut self, ast: &Ast, node: Id<ArgumentList>) {
        for &argument in ast.list(ast[node].arguments) {
            if let Some(named) = ast.cast::<NamedArgument>(argument) {
                self.root_argument = Some(named);
                ast.accept(ast[named].argument_expression, self);
                self.root_argument = None;
            }
        }
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        let name = ast.tokens.lexeme(ast[node].token);
        if name.starts_with('_')
            && let Some(root) = self.root_argument
        {
            let suggested = name.trim_start_matches('_');
            let range = node_range(ast, root);
            self.reports.push(
                diag::invalid_widget_preview_private_argument(name, suggested)
                    .at_offset(range.0 as usize, range.1 as usize),
            );
        }
        ast.visit_children(node, self);
    }
}
