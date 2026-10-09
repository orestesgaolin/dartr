// Dart source: pkg/analyzer/lib/src/error/annotation_verifier.dart

//! `AnnotationVerifier`: the validity of annotations at their declarations
//! (`@awaitNotRequired`, `@Deprecated.*`, `@factory`, `@internal`,
//! `@literal`, `@nonVirtual`, `@reopen`, `@redeclare`, `@UseResult`, the
//! visibility annotations, and the `@Target` kinds of annotation classes).
//!
//! Differences: the element of an annotation is the one the resolver
//! recorded, else the syntactic resolution of [`crate::element_metadata`]
//! (the annotations of the unit are not resolved before unit C9 lands).
//! The values that Dart reads with `computeConstantValue()` (the
//! `TargetKind`s of `@Target`, `parameterDefined` of `@UseResult.unless`)
//! come from the syntax of the annotation arguments.

use std::sync::Arc;

use dartr_ast::{
    Annotation, Ast, BlockClassBody, BlockFunctionBody, ClassDeclaration, ClassTypeAlias,
    CompilationUnit, ConstructorDeclaration, Declaration, Directive, EmptyFunctionBody,
    EnumConstantDeclaration, EnumDeclaration, ExportDirective, Expression, ExpressionFunctionBody,
    ExtensionTypeDeclaration, FieldDeclaration, FieldFormalParameter, FormalParameter,
    FormalParameterList, FunctionDeclaration, FunctionExpression, GenericFunctionType,
    GenericTypeAlias, Id, ImportDirective, InstanceCreationExpression, MethodDeclaration,
    MixinDeclaration, NamedArgument, NodeId, NullLiteral, PartOfDirective, PrefixedIdentifier,
    PrimaryConstructorBody, RegularFormalParameter, ReturnStatement, SimpleStringLiteral,
    SuperFormalParameter, TopLevelVariableDeclaration,
};
use dartr_diagnostics::{LocatableDiagnostic, diag};
use dartr_element::{
    Ctx, ElementFlags, ElementId, FormalParameterElement, FragmentFlags, InterfaceElement, Tag,
    TypeId, TypeKind,
};
use dartr_typesystem::{TypeExt, member};

use super::support::{
    Range, corresponding_parameter, declared_element, enclosing_of, node_range, token_range,
};
use super::{UnitVerifier, VerifierHost};
use crate::ast_ext::formal_parameter_parts;
use crate::element_metadata::{
    AnnotationRef, TargetKind, UnitAst, WorkspacePackage, element_annotations, flags, target_kinds,
};

/// Dart `AnnotationVerifier`.
pub struct AnnotationVerifier {
    /// Dart `_workspacePackage`.
    pub workspace_package: Option<Arc<WorkspacePackage>>,
    /// Dart `_inPackagePublicApi`.
    pub in_package_public_api: bool,
}

impl AnnotationVerifier {
    pub fn new(v: &UnitVerifier<'_>, workspace_package: Option<Arc<WorkspacePackage>>) -> Self {
        let in_package_public_api = workspace_package.as_ref().is_some_and(|p| {
            let library = v.ctx.get(v.library);
            let path = &v.ctx.fragment(library.first_fragment()).source.path;
            p.source_is_in_public_api(path)
        });
        AnnotationVerifier {
            workspace_package,
            in_package_public_api,
        }
    }

    /// Dart `checkAnnotation(node)`.
    pub fn check_annotation(&self, v: &mut UnitVerifier<'_>, node: Id<Annotation>) {
        let ast = v.ast;
        let unit = UnitAst {
            ast: v.ast,
            tables: v.tables,
        };
        let element = AnnotationRef::of_node(&v.ctx, unit, node, v.fragment);
        let Some(parent) = ast.parent(node) else {
            return;
        };
        let ctx = v.ctx;
        let kind = element.kind(&ctx);
        if kind & flags::AWAIT_NOT_REQUIRED != 0 {
            check_await_not_required(v, node);
        } else if kind & flags::DEPRECATED != 0 {
            check_deprecated(v, node, &element);
        } else if kind & flags::FACTORY != 0 {
            check_factory(v, node);
        } else if kind & flags::INTERNAL != 0 {
            self.check_internal(v, node);
        } else if kind & flags::LITERAL != 0 {
            check_literal(v, node);
        } else if kind & flags::NON_VIRTUAL != 0 {
            check_non_virtual(v, node);
        } else if kind & flags::REOPEN != 0 {
            check_reopen(v, node);
        } else if kind & flags::REDECLARE != 0 {
            check_redeclare(v, node);
        } else if kind & flags::USE_RESULT != 0 {
            check_use_result(v, node, &element);
        } else if kind
            & (flags::VISIBLE_FOR_TEMPLATE
                | flags::VISIBLE_FOR_TESTING
                | flags::VISIBLE_FOR_OVERRIDING)
            != 0
        {
            check_visibility(v, node, &element);
        } else if kind & flags::VISIBLE_OUTSIDE_TEMPLATE != 0 {
            check_visibility(v, node, &element);
            check_visible_outside_template(v, node);
        }

        check_kinds(v, node, parent, &element);
    }

    /// Dart `_checkInternal(node)`.
    fn check_internal(&self, v: &mut UnitVerifier<'_>, node: Id<Annotation>) {
        let ast = v.ast;
        let ctx = v.ctx;
        let Some(parent) = ast.parent(node) else {
            return;
        };
        let parent_element = if ast.is::<Declaration>(parent) {
            declared_element(&ctx, v.tables, parent)
        } else {
            None
        };
        let parent_element_is_private = parent_element.is_some_and(|e| is_private(&ctx, e));
        if let Some(p) = ast.cast::<TopLevelVariableDeclaration>(parent) {
            for &variable in ast.list(ast[ast[p].variables].variables) {
                if declared_element(&ctx, v.tables, variable).is_some_and(|e| is_private(&ctx, e)) {
                    report_node(v, diag::invalid_internal_annotation(), variable.raw());
                }
            }
        } else if let Some(p) = ast.cast::<FieldDeclaration>(parent) {
            for &variable in ast.list(ast[ast[p].fields].variables) {
                if declared_element(&ctx, v.tables, variable).is_some_and(|e| is_private(&ctx, e)) {
                    report_node(v, diag::invalid_internal_annotation(), variable.raw());
                }
            }
        } else if ast.is::<ConstructorDeclaration>(parent) {
            if let Some(element) = declared_element(&ctx, v.tables, parent)
                && (is_private(&ctx, element)
                    || enclosing_of(&ctx, element).is_some_and(|c| is_private(&ctx, c)))
            {
                report_node(v, diag::invalid_internal_annotation(), ast[node].name.raw());
            }
        } else if let Some(body) = ast.cast::<PrimaryConstructorBody>(parent) {
            let element = super::element_usage_detector::primary_constructor_declaration(ast, body)
                .and_then(|d| declared_element(&ctx, v.tables, d));
            if let Some(element) = element
                && (is_private(&ctx, element)
                    || enclosing_of(&ctx, element).is_some_and(|c| is_private(&ctx, c)))
            {
                report_node(v, diag::invalid_internal_annotation(), ast[node].name.raw());
            }
        } else if parent_element_is_private || self.in_package_public_api {
            report_node(v, diag::invalid_internal_annotation(), ast[node].name.raw());
        }
    }
}

fn report(v: &mut UnitVerifier<'_>, d: LocatableDiagnostic, range: Range) {
    v.report(d.at_offset(range.0 as usize, range.1 as usize));
}

fn report_node(v: &mut UnitVerifier<'_>, d: LocatableDiagnostic, node: NodeId) {
    let range = node_range(v.ast, node);
    report(v, d, range);
}

/// Dart `Element.isPrivate` (`name == null` or a private name).
fn is_private(ctx: &Ctx<'_>, element: ElementId) -> bool {
    match ctx.element_name(element) {
        None => true,
        Some(name) => name.starts_with('_'),
    }
}

fn element_flag(ctx: &Ctx<'_>, element: ElementId, flag: ElementFlags) -> bool {
    ctx.element_data(element).is_some_and(|d| d.flags.has(flag))
}

fn first_fragment_flag(ctx: &Ctx<'_>, element: ElementId, flag: FragmentFlags) -> bool {
    ctx.element_data(element)
        .and_then(|d| ctx.fragment_data(d.first_fragment))
        .is_some_and(|f| f.flags.has(flag))
}

fn class_is_abstract(ctx: &Ctx<'_>, e: ElementId) -> bool {
    element_flag(ctx, e, ElementFlags::CLASS_ELEMENT_IS_ABSTRACT)
}

fn class_is_base(ctx: &Ctx<'_>, e: ElementId) -> bool {
    if e.tag() == Tag::Mixin {
        return first_fragment_flag(ctx, e, FragmentFlags::MIXIN_FRAGMENT_IS_BASE);
    }
    element_flag(ctx, e, ElementFlags::CLASS_ELEMENT_IS_BASE)
}

fn class_is_final(ctx: &Ctx<'_>, e: ElementId) -> bool {
    element_flag(ctx, e, ElementFlags::CLASS_ELEMENT_IS_FINAL)
}

fn class_is_interface(ctx: &Ctx<'_>, e: ElementId) -> bool {
    element_flag(ctx, e, ElementFlags::CLASS_ELEMENT_IS_INTERFACE)
}

fn class_is_sealed(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_flag(ctx, e, FragmentFlags::CLASS_FRAGMENT_IS_SEALED)
}

fn class_is_mixin_class(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_flag(ctx, e, FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_CLASS)
}

/// Dart `ClassElement.isExtendableOutside`.
fn is_extendable_outside(ctx: &Ctx<'_>, e: ElementId) -> bool {
    !class_is_interface(ctx, e) && !class_is_final(ctx, e) && !class_is_sealed(ctx, e)
}

/// Dart `ClassElement.isImplementableOutside` /
/// `MixinElement.isImplementableOutside`.
fn is_implementable_outside(ctx: &Ctx<'_>, e: ElementId) -> bool {
    if e.tag() == Tag::Mixin {
        return !class_is_base(ctx, e);
    }
    !class_is_base(ctx, e) && !class_is_final(ctx, e) && !class_is_sealed(ctx, e)
}

/// Dart `ClassElement.hasGenerativeConstructor` (the extension of this
/// file): a public generative constructor.
fn has_generative_constructor(ctx: &Ctx<'_>, e: ElementId) -> bool {
    let Some(interface) = e.cast::<InterfaceElement>() else {
        return false;
    };
    ctx.interface(interface).constructors.iter().any(|&c| {
        !is_private(ctx, c.raw())
            && !first_fragment_flag(ctx, c.raw(), FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY)
    })
}

/// Dart `typeAnnotation.type?.element` for an interface type.
fn type_annotation_element(v: &UnitVerifier<'_>, node: NodeId) -> Option<ElementId> {
    let ty = *v.tables.annotation_type.get(node)?;
    match *v.ctx.ty(ty) {
        TypeKind::Interface { element, .. } => Some(element.raw()),
        _ => None,
    }
}

/// Dart `_checkAwaitNotRequired(node)`.
fn check_await_not_required(v: &mut UnitVerifier<'_>, node: Id<Annotation>) {
    let ast = v.ast;
    let ctx = v.ctx;
    let check_type = |v: &mut UnitVerifier<'_>, ty: Option<TypeId>, error_node: Option<NodeId>| {
        let Some(ty) = ty else {
            return;
        };
        if ctx.is_dart_async_future(ty) || ctx.is_dart_async_future_or(ty) {
            return;
        }
        if let Some(element) = ctx.interface_element(ty)
            && ctx
                .element_all_supertypes(element)
                .iter()
                .any(|&t| ctx.is_dart_async_future(t))
        {
            return;
        }
        let at = error_node.unwrap_or(ast[node].name.raw());
        report_node(v, diag::invalid_await_not_required_annotation(), at);
    };
    let Some(parent) = ast.parent(node) else {
        return;
    };
    if ast.is::<MethodDeclaration>(parent) || ast.is::<FunctionDeclaration>(parent) {
        if let Some(element) = declared_element(&ctx, v.tables, parent) {
            check_type(v, Some(member::return_type(&ctx, element.into())), None);
        }
    } else if let Some(p) = ast.cast::<FieldDeclaration>(parent) {
        for &field in ast.list(ast[ast[p].fields].variables) {
            let ty = declared_element(&ctx, v.tables, field).map(|e| member::type_(&ctx, e.into()));
            check_type(v, ty, Some(field.raw()));
        }
    } else if let Some(p) = ast.cast::<TopLevelVariableDeclaration>(parent) {
        for &variable in ast.list(ast[ast[p].variables].variables) {
            let ty =
                declared_element(&ctx, v.tables, variable).map(|e| member::type_(&ctx, e.into()));
            check_type(v, ty, Some(variable.raw()));
        }
    } else if let Some(p) = ast.cast::<GenericTypeAlias>(parent)
        && let Some(function_type) = ast.cast::<GenericFunctionType>(ast[p].type_)
    {
        let ty = ast[function_type]
            .return_type
            .and_then(|r| v.tables.annotation_type.get(r.raw()).copied());
        check_type(v, ty, None);
    } else {
        // Warning reported by `_checkKinds`.
    }
}

/// Dart `_checkDeprecated(node)`.
fn check_deprecated(v: &mut UnitVerifier<'_>, node: Id<Annotation>, element: &AnnotationRef<'_>) {
    match element.deprecation_kind(&v.ctx) {
        Some("extend") => check_deprecated_extend(v, node),
        Some("implement") => check_deprecated_implement(v, node),
        Some("instantiate") => check_deprecated_instantiate(v, node),
        Some("mixin") => check_deprecated_mixin(v, node),
        Some("optional") => check_deprecated_optional(v, node),
        Some("subclass") => check_deprecated_subclass(v, node),
        _ => {}
    }
}

/// The element declared by the parent of a deprecation annotation for the
/// `_checkDeprecated*` checks: a class declaration or class type alias, a
/// mixin if [mixins], or the element of the type of a generic type alias.
fn deprecated_declared_element(
    v: &UnitVerifier<'_>,
    node: Id<Annotation>,
    mixins: bool,
) -> Option<ElementId> {
    let ast = v.ast;
    let parent = ast.parent(node)?;
    if ast.is::<ClassDeclaration>(parent)
        || ast.is::<ClassTypeAlias>(parent)
        || (mixins && ast.is::<MixinDeclaration>(parent))
    {
        return declared_element(&v.ctx, v.tables, parent);
    }
    if let Some(p) = ast.cast::<GenericTypeAlias>(parent) {
        return type_annotation_element(v, ast[p].type_.raw());
    }
    None
}

/// Dart `_checkDeprecatedExtend(node)`.
fn check_deprecated_extend(v: &mut UnitVerifier<'_>, node: Id<Annotation>) {
    let ctx = v.ctx;
    if let Some(e) = deprecated_declared_element(v, node, false)
        && e.tag() == Tag::Class
        && !is_private(&ctx, e)
        && is_extendable_outside(&ctx, e)
        && has_generative_constructor(&ctx, e)
    {
        return;
    }
    let name = v.ast[node].name.raw();
    report_node(v, diag::invalid_deprecated_extend_annotation(), name);
}

/// Dart `_checkDeprecatedImplement(node)`.
fn check_deprecated_implement(v: &mut UnitVerifier<'_>, node: Id<Annotation>) {
    let ctx = v.ctx;
    if let Some(e) = deprecated_declared_element(v, node, true)
        && matches!(e.tag(), Tag::Class | Tag::Mixin)
        && !is_private(&ctx, e)
        && is_implementable_outside(&ctx, e)
    {
        return;
    }
    let name = v.ast[node].name.raw();
    report_node(v, diag::invalid_deprecated_implement_annotation(), name);
}

/// Dart `_checkDeprecatedInstantiate(node)`.
fn check_deprecated_instantiate(v: &mut UnitVerifier<'_>, node: Id<Annotation>) {
    let ctx = v.ctx;
    if let Some(e) = deprecated_declared_element(v, node, false)
        && e.tag() == Tag::Class
        && !is_private(&ctx, e)
        && !class_is_abstract(&ctx, e)
        && has_generative_constructor(&ctx, e)
    {
        return;
    }
    let name = v.ast[node].name.raw();
    report_node(v, diag::invalid_deprecated_instantiate_annotation(), name);
}

/// Dart `_checkDeprecatedMixin(node)`.
fn check_deprecated_mixin(v: &mut UnitVerifier<'_>, node: Id<Annotation>) {
    let ast = v.ast;
    if let Some(parent) = ast
        .parent(node)
        .and_then(|p| ast.cast::<ClassDeclaration>(p))
        && declared_element(&v.ctx, v.tables, parent).is_some_and(|e| !is_private(&v.ctx, e))
        && ast[parent].mixin_keyword.is_some()
    {
        return;
    }
    report_node(
        v,
        diag::invalid_deprecated_mixin_annotation(),
        ast[node].name.raw(),
    );
}

/// Dart `_checkDeprecatedOptional(node)`.
fn check_deprecated_optional(v: &mut UnitVerifier<'_>, node: Id<Annotation>) {
    let ast = v.ast;
    if let Some(parent) = ast.parent(node)
        && ast.is::<FormalParameter>(parent)
    {
        let Some(parameter_list) = ast
            .parent(parent)
            .and_then(|p| ast.cast::<FormalParameterList>(p))
        else {
            // We shouldn't get here; if we do, don't report the annotation.
            return;
        };
        // This annotation is only valid on method declarations, constructor
        // declarations, and top-level function declarations.
        let is_valid_function = ast.parent(parameter_list).is_some_and(|p| {
            ast.is::<MethodDeclaration>(p)
                || ast.is::<ConstructorDeclaration>(p)
                || (ast.is::<FunctionExpression>(p)
                    && ast.parent(p).is_some_and(|f| {
                        ast.is::<FunctionDeclaration>(f)
                            && ast.parent(f).is_some_and(|u| ast.is::<CompilationUnit>(u))
                    }))
        });
        let kind = formal_parameter_parts(ast, parent).kind;
        let is_optional = matches!(
            kind,
            dartr_ast::ParameterKind::Positional | dartr_ast::ParameterKind::Named
        );
        if is_optional && is_valid_function {
            return;
        }
    }
    report_node(
        v,
        diag::invalid_deprecated_optional_annotation(),
        ast[node].name.raw(),
    );
}

/// Dart `_checkDeprecatedSubclass(node)`.
fn check_deprecated_subclass(v: &mut UnitVerifier<'_>, node: Id<Annotation>) {
    let ctx = v.ctx;
    if let Some(e) = deprecated_declared_element(v, node, true) {
        if e.tag() == Tag::Class
            && !is_private(&ctx, e)
            && (is_implementable_outside(&ctx, e) || is_extendable_outside(&ctx, e))
        {
            return;
        }
        if e.tag() == Tag::Mixin && !is_private(&ctx, e) && is_implementable_outside(&ctx, e) {
            return;
        }
    }
    let name = v.ast[node].name.raw();
    report_node(v, diag::invalid_deprecated_subclass_annotation(), name);
}

/// Dart `_checkFactory(node)`: reports a warning at [node] if its parent is
/// not a valid target for a `@factory` annotation.
fn check_factory(v: &mut UnitVerifier<'_>, node: Id<Annotation>) {
    let ast = v.ast;
    let Some(parent) = ast
        .parent(node)
        .and_then(|p| ast.cast::<MethodDeclaration>(p))
    else {
        // Warning reported by `_checkKinds`.
        return;
    };
    let name_token = ast[parent].name;
    let name = ast.tokens.lexeme(name_token).to_string();
    let return_type = ast[parent]
        .return_type
        .and_then(|r| v.tables.annotation_type.get(r.raw()).copied());
    if return_type.is_some_and(|t| matches!(v.ctx.ty(t), TypeKind::Void)) {
        let range = token_range(ast, name_token);
        report(v, diag::invalid_factory_method_decl(&name), range);
        return;
    }

    let body = ast[parent].body.raw();
    if ast.is::<EmptyFunctionBody>(body) {
        // Abstract methods are OK.
        return;
    }

    // Returns `true` for expressions like `new Foo()` or `null`.
    let factory_expression = |e: Option<NodeId>| {
        e.is_some_and(|e| ast.is::<InstanceCreationExpression>(e) || ast.is::<NullLiteral>(e))
    };

    if let Some(b) = ast.cast::<ExpressionFunctionBody>(body) {
        if factory_expression(Some(ast[b].expression.raw())) {
            return;
        }
    } else if let Some(b) = ast.cast::<BlockFunctionBody>(body) {
        let statements = ast.list(ast[ast[b].block].statements);
        if let Some(&last) = statements.last()
            && let Some(r) = ast.cast::<ReturnStatement>(last)
            && factory_expression(ast[r].expression.map(|e| e.raw()))
        {
            return;
        }
    }

    let range = token_range(ast, name_token);
    report(v, diag::invalid_factory_method_impl(&name), range);
}

/// Dart `_checkKinds(node, parent, element)`.
fn check_kinds(
    v: &mut UnitVerifier<'_>,
    node: Id<Annotation>,
    parent: NodeId,
    element: &AnnotationRef<'_>,
) {
    let ctx = v.ctx;
    let Some(kinds) = target_kinds(&ctx, element) else {
        return;
    };
    if kinds.is_empty() || is_valid_target(v, parent, &kinds) {
        return;
    }
    let Some(invoked) = element.element else {
        return;
    };
    let mut name = ctx.element_name(invoked).unwrap_or("").to_string();
    if invoked.tag() == Tag::Constructor {
        let class_name = enclosing_of(&ctx, invoked)
            .and_then(|c| ctx.element_name(c))
            .unwrap_or("")
            .to_string();
        name = if name.is_empty() {
            class_name
        } else {
            format!("{class_name}.{name}")
        };
    }
    let mut kind_names: Vec<&str> = kinds.iter().map(|k| k.display_string()).collect();
    kind_names.sort();
    let valid_kinds = comma_separated_with_or(&kind_names);
    let at = v.ast[node].name.raw();
    report_node(v, diag::invalid_annotation_target(&name, &valid_kinds), at);
}

/// Dart `Iterable<String>.commaSeparatedWithOr`.
fn comma_separated_with_or(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [first] => first.to_string(),
        [first, second] => format!("{first} or {second}"),
        [init @ .., last] => format!("{}, or {last}", init.join(", ")),
    }
}

/// Dart `_checkLiteral(node)`.
fn check_literal(v: &mut UnitVerifier<'_>, node: Id<Annotation>) {
    let ast = v.ast;
    let Some(parent) = ast.parent(node) else {
        return;
    };
    let name = ast[node].name.raw();
    if let Some(p) = ast.cast::<ConstructorDeclaration>(parent) {
        if ast[p].const_keyword.is_none() {
            report_node(v, diag::invalid_literal_annotation(), name);
        }
    } else if let Some(body) = ast.cast::<PrimaryConstructorBody>(parent) {
        let is_const = super::element_usage_detector::primary_constructor_declaration(ast, body)
            .and_then(|d| ast.cast::<dartr_ast::PrimaryConstructorDeclaration>(d))
            .is_some_and(|d| ast[d].const_keyword.is_some());
        if !is_const {
            report_node(v, diag::invalid_literal_annotation(), name);
        }
    }
}

/// Dart `MethodDeclaration.isComplete`.
fn method_is_complete(ast: &Ast, node: Id<MethodDeclaration>) -> bool {
    ast[node].external_keyword.is_some() || !ast.is::<EmptyFunctionBody>(ast[node].body)
}

/// Dart `_checkNonVirtual(node)`.
fn check_non_virtual(v: &mut UnitVerifier<'_>, node: Id<Annotation>) {
    let ast = v.ast;
    if let Some(parent) = ast
        .parent(node)
        .and_then(|p| ast.cast::<MethodDeclaration>(p))
    {
        let in_extension_type = ast
            .parent(parent)
            .and_then(|p| ast.parent(p))
            .is_some_and(|p| ast.is::<ExtensionTypeDeclaration>(p));
        if in_extension_type || !method_is_complete(ast, parent) {
            report_node(
                v,
                diag::invalid_non_virtual_annotation(),
                ast[node].name.raw(),
            );
        }
    }
}

/// Dart `_checkRedeclare(node)`.
fn check_redeclare(v: &mut UnitVerifier<'_>, node: Id<Annotation>) {
    let ast = v.ast;
    let parent = ast.parent(node);
    let parent2 = parent.and_then(|p| ast.parent(p));
    let parent3 = parent2.and_then(|p| ast.parent(p));
    let is_static_method = parent
        .and_then(|p| ast.cast::<MethodDeclaration>(p))
        .is_some_and(|m| {
            ast[m]
                .modifier_keyword
                .is_some_and(|k| ast.tokens.lexeme(k) == "static")
        });
    if !parent2.is_some_and(|p| ast.is::<BlockClassBody>(p))
        || !parent3.is_some_and(|p| ast.is::<ExtensionTypeDeclaration>(p))
        || is_static_method
    {
        let name = dartr_ast::to_source::to_source(ast, ast[node].name.raw());
        report_node(
            v,
            diag::invalid_annotation_target(&name, "instance members of extension types"),
            ast[node].name.raw(),
        );
    }
}

/// Dart `_checkReopen(node)`.
fn check_reopen(v: &mut UnitVerifier<'_>, node: Id<Annotation>) {
    let ast = v.ast;
    let ctx = v.ctx;
    let Some(parent) = ast.parent(node) else {
        return;
    };
    if !ast.is::<ClassDeclaration>(parent) && !ast.is::<ClassTypeAlias>(parent) {
        // If `parent` is neither of the above types, then `_checkKinds` will
        // report a warning.
        return;
    }
    let Some(class_element) = declared_element(&ctx, v.tables, parent) else {
        return;
    };
    let Some(super_element) = class_element
        .cast::<InterfaceElement>()
        .and_then(|c| ctx.element_supertype(c))
        .and_then(|t| ctx.interface_element(t))
        .map(|e| e.raw())
    else {
        return;
    };
    if super_element.tag() != Tag::Class {
        return;
    }
    let name = ast[node].name.raw();
    if class_is_final(&ctx, class_element)
        || class_is_mixin_class(&ctx, class_element)
        || class_is_sealed(&ctx, class_element)
    {
        report_node(v, diag::invalid_reopen_annotation(), name);
        return;
    }
    let library = |e: ElementId| ctx.element_data(e).and_then(|d| d.library);
    if library(class_element) != library(super_element) {
        report_node(v, diag::invalid_reopen_annotation(), name);
        return;
    }
    if class_is_base(&ctx, class_element) {
        if !class_is_final(&ctx, super_element) && !class_is_interface(&ctx, super_element) {
            report_node(v, diag::invalid_reopen_annotation(), name);
        }
    } else if !class_is_base(&ctx, class_element)
        && !class_is_final(&ctx, class_element)
        && !class_is_interface(&ctx, class_element)
        && !class_is_sealed(&ctx, class_element)
        && !class_is_interface(&ctx, super_element)
    {
        report_node(v, diag::invalid_reopen_annotation(), name);
    }
}

/// Dart `_checkUseResult(node, element)`.
fn check_use_result(v: &mut UnitVerifier<'_>, node: Id<Annotation>, element: &AnnotationRef<'_>) {
    let ast = v.ast;
    let Some(parent) = ast.parent(node) else {
        return;
    };
    let Some(undefined_parameter) = find_undefined_use_result_parameter(v, element, node, parent)
    else {
        return;
    };
    let name = if let Some(f) = ast.cast::<FunctionDeclaration>(parent) {
        Some(ast.tokens.lexeme(ast[f].name).to_string())
    } else {
        ast.cast::<MethodDeclaration>(parent)
            .map(|m| ast.tokens.lexeme(ast[m].name).to_string())
    };
    let Some(name) = name else {
        return;
    };
    let parameter_name = if let Some(s) = ast.cast::<SimpleStringLiteral>(undefined_parameter) {
        Some(ast[s].value.to_string())
    } else {
        corresponding_parameter(&v.ctx, ast, v.tables, undefined_parameter.raw())
            .and_then(|p| v.ctx.element_name(p).map(str::to_string))
    };
    let parameter_name = parameter_name
        .unwrap_or_else(|| dartr_ast::to_source::to_source(ast, undefined_parameter.raw()));
    report_node(
        v,
        diag::undefined_referenced_parameter(&parameter_name, &name),
        undefined_parameter.raw(),
    );
}

/// Dart `_checkVisibility(node, element)`.
fn check_visibility(v: &mut UnitVerifier<'_>, node: Id<Annotation>, element: &AnnotationRef<'_>) {
    let ast = v.ast;
    let ctx = v.ctx;
    let Some(parent) = ast.parent(node) else {
        return;
    };
    if !ast.is::<Declaration>(parent) {
        // This is reported by `_checkKinds`.
        return;
    }
    let annotation_name = dartr_ast::to_source::to_source(ast, ast[node].name.raw());
    let report_invalid = |v: &mut UnitVerifier<'_>, name: &str| {
        report_node(
            v,
            diag::invalid_visibility_annotation(name, &annotation_name),
            ast[node].name.raw(),
        );
    };
    let is_visible_for_overriding = element.is(&ctx, flags::VISIBLE_FOR_OVERRIDING);

    if let Some(p) = ast.cast::<TopLevelVariableDeclaration>(parent) {
        for &variable in ast.list(ast[ast[p].variables].variables) {
            if let Some(name) =
                declared_element(&ctx, v.tables, variable).and_then(|e| ctx.element_name(e))
                && name.starts_with('_')
            {
                report_invalid(v, name);
            }
        }
    } else if let Some(p) = ast.cast::<FieldDeclaration>(parent) {
        for &variable in ast.list(ast[ast[p].fields].variables) {
            if ast[p].static_keyword.is_some() && is_visible_for_overriding {
                // This is reported by `_checkKinds`.
                return;
            }
            if let Some(name) =
                declared_element(&ctx, v.tables, variable).and_then(|e| ctx.element_name(e))
                && name.starts_with('_')
            {
                report_invalid(v, name);
            }
        }
    } else {
        let declared = if let Some(body) = ast.cast::<PrimaryConstructorBody>(parent) {
            super::element_usage_detector::primary_constructor_declaration(ast, body)
                .and_then(|d| declared_element(&ctx, v.tables, d))
        } else {
            declared_element(&ctx, v.tables, parent)
        };
        if let Some(declared) = declared {
            if is_visible_for_overriding
                && (!is_instance_member(&ctx, declared)
                    || enclosing_of(&ctx, declared).is_some_and(|e| e.tag() == Tag::ExtensionType))
            {
                // This is reported by `_checkKinds`.
                return;
            }
            if let Some(name) = ctx.element_name(declared)
                && name.starts_with('_')
            {
                report_invalid(v, name);
            }
        }
    }
}

/// Dart `Element.isInstanceMember` (dart/element/extensions.dart).
fn is_instance_member(ctx: &Ctx<'_>, element: ElementId) -> bool {
    let enclosing_is_interface = enclosing_of(ctx, element).is_some_and(|e| {
        matches!(
            e.tag(),
            Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType
        )
    });
    enclosing_is_interface
        && matches!(element.tag(), Tag::Method | Tag::Getter | Tag::Setter)
        && !member::is_static(ctx, element.into())
}

/// Dart `_checkVisibleOutsideTemplate(node)`.
fn check_visible_outside_template(v: &mut UnitVerifier<'_>, node: Id<Annotation>) {
    let ast = v.ast;
    let ctx = v.ctx;
    let name = ast[node].name.raw();
    let contained = ast.parent(node).filter(|&p| {
        ast.is::<ConstructorDeclaration>(p)
            || ast.is::<EnumConstantDeclaration>(p)
            || ast.is::<FieldDeclaration>(p)
            || ast.is::<MethodDeclaration>(p)
    });
    let Some(contained) = contained else {
        report_node(v, diag::invalid_visible_outside_template_annotation(), name);
        return;
    };
    let container = ast
        .parent(contained)
        .and_then(|p| ast.parent(p))
        .filter(|&p| {
            ast.is::<ClassDeclaration>(p)
                || ast.is::<EnumDeclaration>(p)
                || ast.is::<MixinDeclaration>(p)
        });
    let Some(container) = container else {
        report_node(v, diag::invalid_visible_outside_template_annotation(), name);
        return;
    };
    let Some(declared) = declared_element(&ctx, v.tables, container) else {
        report_node(v, diag::invalid_visible_outside_template_annotation(), name);
        return;
    };
    let unit = Some(UnitAst {
        ast: v.ast,
        tables: v.tables,
    });
    if element_annotations(&ctx, declared, unit)
        .iter()
        .any(|a| a.is(&ctx, flags::VISIBLE_FOR_TEMPLATE))
    {
        return;
    }
    report_node(v, diag::invalid_visible_outside_template_annotation(), name);
}

/// Dart `_findUndefinedUseResultParameter(element, node, parent)`.
fn find_undefined_use_result_parameter(
    v: &UnitVerifier<'_>,
    element: &AnnotationRef<'_>,
    node: Id<Annotation>,
    parent: NodeId,
) -> Option<Id<Expression>> {
    let ast = v.ast;
    let constructor_name = ast.cast::<PrefixedIdentifier>(ast[node].name)?;
    if ast
        .tokens
        .lexeme(ast[ast[constructor_name].identifier].token)
        != "unless"
    {
        return None;
    }
    let unless_param = element.string_argument(None, Some("parameterDefined"))?;

    let check_params = |parameter_list: Option<Id<FormalParameterList>>| -> Option<Id<Expression>> {
        let parameter_list = parameter_list?;
        for &parameter in ast.list(ast[parameter_list].parameters) {
            let name = formal_parameter_parts(ast, parameter.raw())
                .name
                .map(|t| ast.tokens.lexeme(t));
            if name == Some(unless_param.as_str()) {
                return None;
            }
        }
        // Find and return the parameter value node.
        let arguments = ast[node].arguments?;
        for &argument in ast.list(ast[arguments].arguments) {
            if let Some(named) = ast.cast::<NamedArgument>(argument)
                && ast.tokens.lexeme(ast[named].name) == "parameterDefined"
            {
                return Some(ast[named].argument_expression);
            }
        }
        None
    };

    if let Some(f) = ast.cast::<FunctionDeclaration>(parent) {
        return check_params(ast[ast[f].function_expression].parameters);
    }
    if let Some(m) = ast.cast::<MethodDeclaration>(parent) {
        return check_params(ast[m].parameters);
    }
    None
}

/// Dart `_isValidTarget(target, kinds)`.
fn is_valid_target(v: &UnitVerifier<'_>, target: NodeId, kinds: &[TargetKind]) -> bool {
    let ast = v.ast;
    let ctx = v.ctx;
    // Handle the case of the deprecated `TargetKind.directive` before
    // handling the Directive subclasses below.
    if kinds.contains(&TargetKind::Directive) && ast.is::<Directive>(target) {
        return true;
    }

    // To support Dart language versions where unnamed libraries did not
    // exist, we allow annotating the first directive, if the annotation is
    // intended for a library directive.
    if kinds.contains(&TargetKind::Library)
        && ast.is::<Directive>(target)
        && let Some(unit) = ast
            .parent(target)
            .and_then(|p| ast.cast::<CompilationUnit>(p))
        && ast
            .list(ast[unit].directives)
            .first()
            .is_some_and(|d| d.raw() == target)
    {
        return true;
    }

    for element in target_elements(v, target) {
        if is_valid_annotation_target_element(&ctx, element, kinds) {
            return true;
        }
    }

    if ast.is::<ExportDirective>(target) {
        kinds.contains(&TargetKind::ExportDirective)
    } else if ast.is::<ImportDirective>(target) {
        kinds.contains(&TargetKind::ImportDirective)
    } else if ast.is::<PartOfDirective>(target) {
        kinds.contains(&TargetKind::PartOfDirective)
    } else {
        false
    }
}

/// Dart `_targetElements(target)`.
fn target_elements(v: &UnitVerifier<'_>, target: NodeId) -> Vec<ElementId> {
    let ast = v.ast;
    let ctx = v.ctx;
    let declared = |n: NodeId| declared_element(&ctx, v.tables, n);
    if let Some(f) = ast.cast::<FieldDeclaration>(target) {
        return ast
            .list(ast[ast[f].fields].variables)
            .iter()
            .filter_map(|&n| declared(n.raw()))
            .collect();
    }
    if ast.is::<RegularFormalParameter>(target)
        || ast.is::<FieldFormalParameter>(target)
        || ast.is::<SuperFormalParameter>(target)
    {
        let Some(element) = declared(target) else {
            return Vec::new();
        };
        if element.tag() == Tag::FieldFormalParameter
            && first_fragment_flag(
                &ctx,
                element,
                FragmentFlags::FIELD_FORMAL_PARAMETER_FRAGMENT_IS_DECLARING,
            )
        {
            let field = element
                .cast::<FormalParameterElement>()
                .and_then(|p| ctx.get(p).field.get());
            let mut result = vec![element];
            result.extend(field.map(|f| f.raw()));
            return result;
        }
        return vec![element];
    }
    if let Some(body) = ast.cast::<PrimaryConstructorBody>(target) {
        return super::element_usage_detector::primary_constructor_declaration(ast, body)
            .and_then(declared)
            .into_iter()
            .collect();
    }
    if let Some(t) = ast.cast::<TopLevelVariableDeclaration>(target) {
        return ast
            .list(ast[ast[t].variables].variables)
            .iter()
            .filter_map(|&n| declared(n.raw()))
            .collect();
    }
    if ast.is::<Declaration>(target) {
        return declared(target).into_iter().collect();
    }
    Vec::new()
}

/// Dart `isValidAnnotationTargetElement(element, kinds)`
/// (annotation_target.dart).
fn is_valid_annotation_target_element(
    ctx: &Ctx<'_>,
    element: ElementId,
    kinds: &[TargetKind],
) -> bool {
    let has = |k: TargetKind| kinds.contains(&k);
    if has(TargetKind::OverridableMember) && is_overridable_member(ctx, element) {
        return true;
    }
    match element.tag() {
        Tag::Class => has(TargetKind::ClassType) || has(TargetKind::Type),
        Tag::Constructor => has(TargetKind::Constructor),
        Tag::Enum => has(TargetKind::EnumType) || has(TargetKind::Type),
        Tag::Extension => has(TargetKind::Extension),
        Tag::ExtensionType => has(TargetKind::ExtensionType),
        Tag::Field => {
            if crate::element_ext::is_enum_constant(ctx, element) {
                has(TargetKind::EnumValue)
            } else {
                has(TargetKind::Field)
            }
        }
        Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => {
            let is_optional = element
                .cast::<FormalParameterElement>()
                .is_some_and(|p| dartr_typesystem::type_ext::is_optional(ctx.get(p).kind));
            has(TargetKind::Parameter) || (is_optional && has(TargetKind::OptionalParameter))
        }
        Tag::Getter => has(TargetKind::Getter),
        Tag::Library => has(TargetKind::Library),
        Tag::LocalFunction | Tag::TopLevelFunction => has(TargetKind::Function),
        Tag::Method => has(TargetKind::Method),
        Tag::Mixin => has(TargetKind::MixinType) || has(TargetKind::Type),
        Tag::Setter => has(TargetKind::Setter),
        Tag::TopLevelVariable => has(TargetKind::TopLevelVariable),
        Tag::TypeAlias => has(TargetKind::TypedefType) || has(TargetKind::Type),
        Tag::TypeParameter => has(TargetKind::TypeParameter),
        _ => false,
    }
}

/// Dart `_isOverridableMember(element)`.
fn is_overridable_member(ctx: &Ctx<'_>, element: ElementId) -> bool {
    matches!(
        element.tag(),
        Tag::Field | Tag::Getter | Tag::Method | Tag::Setter
    ) && !member::is_static(ctx, element.into())
        && enclosing_of(ctx, element)
            .is_some_and(|e| matches!(e.tag(), Tag::Class | Tag::ExtensionType | Tag::Mixin))
}
