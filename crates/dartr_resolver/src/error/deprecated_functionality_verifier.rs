// Dart source: pkg/analyzer/lib/src/error/deprecated_functionality_verifier.dart

//! `DeprecatedFunctionalityVerifier`: uses of deprecated functionality of
//! other libraries (`@Deprecated.extend`, `.implement`, `.subclass`,
//! `.mixin`, `.instantiate`, `.optional`). The methods of the Dart class
//! are free functions over the unit.
//!
//! `verifySuperFormalParameters` of `super_formal_parameters_verifier.dart`
//! is replaced by [`count_super_formal_parameters`]: the counting part only
//! (the best practices verifier passes no positional arguments, so Dart
//! reports nothing there).

use dartr_ast::{
    ArgumentList, Ast, BlockClassBody, BlockEnumBody, ClassDeclaration, ClassTypeAlias,
    ConstructorDeclaration, ConstructorName, DotShorthandConstructorInvocation,
    DotShorthandInvocation, EnumDeclaration, FormalParameterList, Id, ImplementsClause,
    InstanceCreationExpression, MethodInvocation, MixinDeclaration, NamedArgument, NamedType,
    NodeId, PrimaryConstructorBody, PrimaryConstructorDeclaration,
    RedirectingConstructorInvocation, SuperConstructorInvocation, SuperFormalParameter, WithClause,
};
use dartr_diagnostics::{LocatableDiagnostic, diag};
use dartr_element::{
    ConstructorElement, ElemRef, ElementId, FormalParameterElement, Tag, TypeKind,
};
use dartr_typesystem::{member, type_ext};

use super::support::{
    Range, corresponding_parameter, element_of, library_of, node_range, token_range,
};
use super::{UnitVerifier, VerifierHost};
use crate::ast_ext::formal_parameter_parts;
use crate::element_metadata::{UnitAst, is_deprecated_with_kind};

fn report(v: &mut UnitVerifier<'_>, d: LocatableDiagnostic, range: Range) {
    v.report(d.at_offset(range.0 as usize, range.1 as usize));
}

fn deprecated_with_kind(v: &UnitVerifier<'_>, element: ElementId, kind: &str) -> bool {
    let unit = Some(UnitAst {
        ast: v.ast,
        tables: v.tables,
    });
    is_deprecated_with_kind(&v.ctx, element, kind, unit)
}

/// Dart `element.name!`.
fn name_of(v: &UnitVerifier<'_>, element: ElementId) -> String {
    dartr_typesystem::TypeExt::element_name(&v.ctx, element)
        .unwrap_or("")
        .to_string()
}

/// Dart `node.type?.element is InterfaceElement`.
fn type_is_interface(v: &UnitVerifier<'_>, node: Id<NamedType>) -> bool {
    v.tables
        .annotation_type
        .get(node.raw())
        .is_some_and(|&t| matches!(v.ctx.ty(t), TypeKind::Interface { .. }))
}

fn in_current_library(v: &UnitVerifier<'_>, element: ElementId) -> bool {
    library_of(&v.ctx, element) == Some(v.library)
}

/// Dart `classDeclaration(node)`.
pub fn class_declaration(v: &mut UnitVerifier<'_>, node: Id<ClassDeclaration>) {
    let ast = v.ast;
    check_for_deprecated_extend(v, ast[node].extends_clause.map(|e| ast[e].superclass));
    check_for_deprecated_implement(v, interfaces(ast, ast[node].implements_clause));
    check_for_deprecated_mixin(v, ast[node].with_clause);
    check_for_deprecated_subclass(v, mixin_types(ast, ast[node].with_clause));
}

/// Dart `classTypeAlias(node)`.
pub fn class_type_alias(v: &mut UnitVerifier<'_>, node: Id<ClassTypeAlias>) {
    let ast = v.ast;
    check_for_deprecated_extend(v, Some(ast[node].superclass));
    check_for_deprecated_implement(v, interfaces(ast, ast[node].implements_clause));
    check_for_deprecated_mixin(v, Some(ast[node].with_clause));
}

fn interfaces(ast: &Ast, clause: Option<Id<ImplementsClause>>) -> Option<Vec<Id<NamedType>>> {
    clause.map(|c| ast.list(ast[c].interfaces).to_vec())
}

fn mixin_types(ast: &Ast, clause: Option<Id<WithClause>>) -> Option<Vec<Id<NamedType>>> {
    clause.map(|c| ast.list(ast[c].mixin_types).to_vec())
}

/// Dart `constructorDeclaration(node)`.
pub fn constructor_declaration(v: &mut UnitVerifier<'_>, node: Id<ConstructorDeclaration>) {
    let ast = v.ast;
    let initializers: Vec<NodeId> = ast
        .list(ast[node].initializers)
        .iter()
        .map(|i| i.raw())
        .collect();
    let super_constructor = super_constructor_of(v, node.raw());
    let error_range = constructor_error_range(ast, node);
    check_for_deprecated_optional_super_parameters(
        v,
        ast[node].parameters,
        &initializers,
        super_constructor,
        error_range,
    );
    check_for_deprecated_optional_redirected_parameters(v, node);

    // Check redirecting constructor invocations in the initializer list.
    for &initializer in &initializers {
        let Some(invocation) = ast.cast::<RedirectingConstructorInvocation>(initializer) else {
            continue;
        };
        let Some(element) = v.tables.element.get(invocation.raw()).copied() else {
            return;
        };
        if member::base_element(&v.ctx, element).tag() != Tag::Constructor {
            return;
        }
        let error_entity = match ast[invocation].constructor_name {
            Some(n) => node_range(ast, n),
            None => token_range(ast, ast[invocation].this_keyword),
        };
        check_for_deprecated_optional(v, element, ast[invocation].argument_list, error_entity);
    }

    // TODO(srawlins): Detect omitted parameters in a redirecting factory
    // constructor.
}

/// Dart `ConstructorDeclaration.errorRange`.
fn constructor_error_range(ast: &Ast, node: Id<ConstructorDeclaration>) -> Range {
    let c = &ast[node];
    let start = match c.type_name {
        Some(t) => ast.offset(t),
        None => match c.new_keyword.or(c.factory_keyword) {
            Some(k) => ast.tokens.offset(k),
            None => ast.offset(node),
        },
    };
    let end = match c.name {
        Some(n) => crate::ast_ext::token_end(ast, n),
        None => match c.type_name {
            Some(t) => ast.end(t),
            None => match c.new_keyword.or(c.factory_keyword) {
                Some(k) => crate::ast_ext::token_end(ast, k),
                None => ast.end(node),
            },
        },
    };
    (start, end - start)
}

/// Dart `node.declaredFragment?.element.superConstructor`.
fn super_constructor_of(v: &UnitVerifier<'_>, node: NodeId) -> Option<ElemRef> {
    let element = super::support::declared_element(&v.ctx, v.tables, node)?;
    let constructor = element.cast::<ConstructorElement>()?;
    v.ctx.get(constructor).super_constructor.get()
}

/// Dart `constructorName(node)`.
pub fn constructor_name(v: &mut UnitVerifier<'_>, node: Id<ConstructorName>) {
    let ast = v.ast;
    let Some(element) = element_of(&v.ctx, v.tables, ast[node].type_) else {
        return;
    };
    if !is_interface_element(element) {
        return;
    }
    let range = node_range(ast, node);
    check_for_deprecated_instantiate(v, element, range);
}

fn is_interface_element(element: ElementId) -> bool {
    matches!(
        element.tag(),
        Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType
    )
}

/// Dart `dotShorthandConstructorInvocation(node)`.
pub fn dot_shorthand_constructor_invocation(
    v: &mut UnitVerifier<'_>,
    node: Id<DotShorthandConstructorInvocation>,
) {
    let ast = v.ast;
    let Some(element) = v.tables.element.get(node.raw()).copied() else {
        return;
    };
    let base = member::base_element(&v.ctx, element);
    if base.tag() != Tag::Constructor {
        return;
    }
    let range = node_range(ast, ast[node].constructor_name);
    check_for_deprecated_optional(v, element, ast[node].argument_list, range);
    if let Some(enclosing) = super::support::enclosing_of(&v.ctx, base) {
        check_for_deprecated_instantiate(v, enclosing, range);
    }
}

/// Dart `dotShorthandInvocation(node)`.
pub fn dot_shorthand_invocation(v: &mut UnitVerifier<'_>, node: Id<DotShorthandInvocation>) {
    let ast = v.ast;
    let Some(element) = v.tables.element.get(ast[node].member_name.raw()).copied() else {
        return;
    };
    if !is_executable(member::base_element(&v.ctx, element)) {
        return;
    }
    let range = node_range(ast, ast[node].member_name);
    check_for_deprecated_optional(v, element, ast[node].argument_list, range);
}

fn is_executable(element: ElementId) -> bool {
    matches!(
        element.tag(),
        Tag::Constructor
            | Tag::Method
            | Tag::Getter
            | Tag::Setter
            | Tag::TopLevelFunction
            | Tag::LocalFunction
    )
}

/// Dart `enumDeclaration(node)`.
pub fn enum_declaration(v: &mut UnitVerifier<'_>, node: Id<EnumDeclaration>) {
    let ast = v.ast;
    check_for_deprecated_implement(v, interfaces(ast, ast[node].implements_clause));
    check_for_deprecated_mixin(v, ast[node].with_clause);
}

/// Dart `instanceCreationExpression(node)`.
pub fn instance_creation_expression(
    v: &mut UnitVerifier<'_>,
    node: Id<InstanceCreationExpression>,
) {
    let ast = v.ast;
    let constructor_name = ast[node].constructor_name;
    let Some(constructor) = v.tables.element.get(constructor_name.raw()).copied() else {
        return;
    };
    let range = node_range(ast, constructor_name);
    check_for_deprecated_optional(v, constructor, ast[node].argument_list, range);
    let Some(interface) = element_of(&v.ctx, v.tables, ast[constructor_name].type_) else {
        return;
    };
    if !is_interface_element(interface) {
        return;
    }
    check_for_deprecated_instantiate(v, interface, range);
}

/// Dart `methodInvocation(node)`.
pub fn method_invocation(v: &mut UnitVerifier<'_>, node: Id<MethodInvocation>) {
    let ast = v.ast;
    let method_name = ast[node].method_name;
    let Some(method) = v.tables.element.get(method_name.raw()).copied() else {
        return;
    };
    let base = member::base_element(&v.ctx, method);
    if !is_executable(base) || base.tag() == Tag::LocalFunction {
        return;
    }
    let range = node_range(ast, method_name);
    check_for_deprecated_optional(v, method, ast[node].argument_list, range);
}

/// Dart `mixinDeclaration(node)`.
pub fn mixin_declaration(v: &mut UnitVerifier<'_>, node: Id<MixinDeclaration>) {
    let ast = v.ast;
    check_for_deprecated_implement(v, interfaces(ast, ast[node].implements_clause));
    // Not technically "implementing," but is similar enough for
    // `@Deprecated.implement` and `@Deprecated.subclass`.
    let constraints = ast[node]
        .on_clause
        .map(|c| ast.list(ast[c].superclass_constraints).to_vec());
    check_for_deprecated_implement(v, constraints);
}

/// Dart `primaryConstructorDeclaration(node)`.
pub fn primary_constructor_declaration(
    v: &mut UnitVerifier<'_>,
    node: Id<PrimaryConstructorDeclaration>,
) {
    let ast = v.ast;
    let initializers: Vec<NodeId> = primary_constructor_body(ast, node)
        .map(|b| {
            ast.list(ast[b].initializers)
                .iter()
                .map(|i| i.raw())
                .collect()
        })
        .unwrap_or_default();
    let super_constructor = super_constructor_of(v, node.raw());
    // Dart `PrimaryConstructorDeclaration.errorRange`.
    let start = ast.offset(node);
    let end = match ast[node].constructor_name {
        Some(n) => ast.end(n),
        None => {
            crate::ast_ext::token_end(ast, ast[node].const_keyword.unwrap_or(ast[node].type_name))
        }
    };
    check_for_deprecated_optional_super_parameters(
        v,
        ast[node].formal_parameters,
        &initializers,
        super_constructor,
        (start, end - start),
    );
}

/// Dart `PrimaryConstructorDeclaration.body`: the primary constructor body
/// among the members of the declaration.
fn primary_constructor_body(
    ast: &Ast,
    node: Id<PrimaryConstructorDeclaration>,
) -> Option<Id<PrimaryConstructorBody>> {
    let declaration = ast.parent(node)?;
    let body = if let Some(c) = ast.cast::<ClassDeclaration>(declaration) {
        ast[c].body.raw()
    } else if let Some(e) = ast.cast::<EnumDeclaration>(declaration) {
        ast[e].body.raw()
    } else if let Some(e) = ast.cast::<dartr_ast::ExtensionTypeDeclaration>(declaration) {
        ast[e].body.raw()
    } else {
        return None;
    };
    let members = if let Some(b) = ast.cast::<BlockClassBody>(body) {
        ast[b].members
    } else if let Some(b) = ast.cast::<BlockEnumBody>(body) {
        ast[b].members
    } else {
        return None;
    };
    ast.list(members)
        .iter()
        .find_map(|&m| ast.cast::<PrimaryConstructorBody>(m))
}

/// Dart `_checkForDeprecatedExtend(node)`.
fn check_for_deprecated_extend(v: &mut UnitVerifier<'_>, node: Option<Id<NamedType>>) {
    let Some(node) = node else {
        return;
    };
    let Some(element) = element_of(&v.ctx, v.tables, node) else {
        return;
    };
    if type_is_interface(v, node) {
        if in_current_library(v, element) {
            return;
        }
        let range = node_range(v.ast, node);
        if deprecated_with_kind(v, element, "extend") {
            let name = name_of(v, element);
            report(v, diag::deprecated_extend(&name), range);
        } else if deprecated_with_kind(v, element, "subclass") {
            let name = name_of(v, element);
            report(v, diag::deprecated_subclass(&name), range);
        }
    }
}

/// Dart `_checkForDeprecatedImplement(namedTypes)`.
fn check_for_deprecated_implement(
    v: &mut UnitVerifier<'_>,
    named_types: Option<Vec<Id<NamedType>>>,
) {
    let Some(named_types) = named_types else {
        return;
    };
    for named_type in named_types {
        let Some(element) = element_of(&v.ctx, v.tables, named_type) else {
            continue;
        };
        if in_current_library(v, element) {
            continue;
        }
        if type_is_interface(v, named_type) {
            let range = node_range(v.ast, named_type);
            if deprecated_with_kind(v, element, "implement") {
                let name = name_of(v, element);
                report(v, diag::deprecated_implement(&name), range);
            } else if deprecated_with_kind(v, element, "subclass") {
                let name = name_of(v, element);
                report(v, diag::deprecated_subclass(&name), range);
            }
        }
    }
}

/// Dart `_checkForDeprecatedInstantiate(element:, errorNode:)`.
fn check_for_deprecated_instantiate(v: &mut UnitVerifier<'_>, element: ElementId, range: Range) {
    if deprecated_with_kind(v, element, "instantiate") {
        let name = name_of(v, element);
        report(v, diag::deprecated_instantiate(&name), range);
    }
}

/// Dart `_checkForDeprecatedMixin(node)`.
fn check_for_deprecated_mixin(v: &mut UnitVerifier<'_>, node: Option<Id<WithClause>>) {
    let Some(node) = node else {
        return;
    };
    let ast = v.ast;
    for &mixin in ast.list(ast[node].mixin_types) {
        let Some(&ty) = v.tables.annotation_type.get(mixin.raw()) else {
            continue;
        };
        let TypeKind::Interface { element, .. } = *v.ctx.ty(ty) else {
            continue;
        };
        let element = element.raw();
        if in_current_library(v, element) {
            continue;
        }
        if deprecated_with_kind(v, element, "mixin") {
            let name = name_of(v, element);
            let range = node_range(ast, mixin);
            report(v, diag::deprecated_mixin(&name), range);
        }
    }
}

/// Dart `_checkForDeprecatedOptional(element:, argumentList:,
/// errorEntity:)`.
fn check_for_deprecated_optional(
    v: &mut UnitVerifier<'_>,
    element: ElemRef,
    argument_list: Id<ArgumentList>,
    error_entity: Range,
) {
    let ctx = v.ctx;
    let ast = v.ast;
    let mut omitted_parameters: Vec<ElementId> = member::formal_parameters(&ctx, element)
        .into_iter()
        .map(|p| member::base_element(&ctx, p))
        .collect();
    for &argument in ast.list(ast[argument_list].arguments) {
        let Some(parameter) = corresponding_parameter(&ctx, ast, v.tables, argument.raw()) else {
            continue;
        };
        if let Some(i) = omitted_parameters.iter().position(|&p| p == parameter) {
            omitted_parameters.remove(i);
        }
    }
    for parameter in omitted_parameters {
        if deprecated_with_kind(v, parameter, "optional") {
            let name = parameter_name(v, parameter);
            report(v, diag::deprecated_optional(&name), error_entity);
        }
    }
}

/// Dart `parameter.name ?? '<unknown>'`.
fn parameter_name(v: &UnitVerifier<'_>, parameter: ElementId) -> String {
    dartr_typesystem::TypeExt::element_name(&v.ctx, parameter)
        .unwrap_or("<unknown>")
        .to_string()
}

fn parameter_kind(v: &UnitVerifier<'_>, p: ElementId) -> Option<dartr_ast::ParameterKind> {
    p.cast::<FormalParameterElement>()
        .map(|p| v.ctx.get(p).kind)
}

/// Dart `_checkForDeprecatedOptionalRedirectedParameters(node)`.
fn check_for_deprecated_optional_redirected_parameters(
    v: &mut UnitVerifier<'_>,
    node: Id<ConstructorDeclaration>,
) {
    let ast = v.ast;
    let ctx = v.ctx;
    let Some(redirected) = ast[node].redirected_constructor else {
        return;
    };
    let Some(redirected_constructor) = v.tables.element.get(redirected.raw()).copied() else {
        return;
    };
    let error_range = constructor_error_range(ast, node);
    let parameters = ast.list(ast[ast[node].parameters].parameters);
    let positional_argument_count = parameters
        .iter()
        .filter(|&&p| type_ext::is_positional(formal_parameter_parts(ast, p.raw()).kind))
        .count();
    let named_argument_names: Vec<&str> = parameters
        .iter()
        .map(|&p| formal_parameter_parts(ast, p.raw()))
        .filter(|parts| type_ext::is_named(parts.kind))
        .filter_map(|parts| parts.name.map(|n| ast.tokens.lexeme(n)))
        .collect();
    let mut redirected_positional_count = 0;
    for parameter in member::formal_parameters(&ctx, redirected_constructor) {
        let parameter = member::base_element(&ctx, parameter);
        let Some(kind) = parameter_kind(v, parameter) else {
            continue;
        };
        if type_ext::is_positional(kind) {
            redirected_positional_count += 1;
        }
        if !type_ext::is_optional(kind) {
            continue;
        }
        if !deprecated_with_kind(v, parameter, "optional") {
            continue;
        }
        if type_ext::is_positional(kind) {
            if redirected_positional_count <= positional_argument_count {
                continue;
            }
        } else {
            let name = dartr_typesystem::TypeExt::element_name(&ctx, parameter);
            if name.is_some_and(|n| named_argument_names.contains(&n)) {
                continue;
            }
        }
        let name = parameter_name(v, parameter);
        report(v, diag::deprecated_optional(&name), error_range);
    }
}

/// The counting part of Dart `verifySuperFormalParameters(
/// formalParameterList:)`: the number of positional super formal
/// parameters and the names of the named ones.
fn count_super_formal_parameters(
    v: &UnitVerifier<'_>,
    parameters: Id<FormalParameterList>,
) -> (usize, Vec<String>) {
    let ast = v.ast;
    let mut positional_argument_count = 0;
    let mut named_argument_names = Vec::new();
    for &parameter in ast.list(ast[parameters].parameters) {
        let Some(p) = ast.cast::<SuperFormalParameter>(parameter) else {
            continue;
        };
        if type_ext::is_named(ast[p].kind) {
            named_argument_names.push(ast.tokens.lexeme(ast[p].name).to_string());
        } else {
            positional_argument_count += 1;
        }
    }
    (positional_argument_count, named_argument_names)
}

/// Dart `_checkForDeprecatedOptionalSuperParameters(parameters:,
/// initializers:, superConstructor:, errorRange:)`.
fn check_for_deprecated_optional_super_parameters(
    v: &mut UnitVerifier<'_>,
    parameters: Id<FormalParameterList>,
    initializers: &[NodeId],
    super_constructor: Option<ElemRef>,
    mut error_range: Range,
) {
    let Some(super_constructor) = super_constructor else {
        return;
    };
    let ast = v.ast;
    let ctx = v.ctx;
    let super_invocations: Vec<Id<SuperConstructorInvocation>> = initializers
        .iter()
        .filter_map(|&i| ast.cast::<SuperConstructorInvocation>(i))
        .collect();
    if super_invocations.len() > 1 {
        // Error reported elsewhere.
        return;
    }
    let (positional_argument_count, named_argument_names) =
        count_super_formal_parameters(v, parameters);

    let mut super_arguments: Vec<NodeId> = Vec::new();
    if let Some(&invocation) = super_invocations.first() {
        // Arguments may be passed to the super constructor _either_ in
        // `superConstructorInvocation` or via super-parameters.
        super_arguments = ast
            .list(ast[ast[invocation].argument_list].arguments)
            .iter()
            .map(|a| a.raw())
            .collect();
        error_range = match ast[invocation].constructor_name {
            Some(n) => node_range(ast, n),
            None => token_range(ast, ast[invocation].super_keyword),
        };
    }
    let named_super_argument_names: Vec<&str> = super_arguments
        .iter()
        .filter_map(|&a| ast.cast::<NamedArgument>(a))
        .map(|a| ast.tokens.lexeme(ast[a].name))
        .collect();
    let positional_super_argument_count = super_arguments
        .iter()
        .filter(|&&a| !ast.is::<NamedArgument>(a))
        .count();

    let mut super_positional_count = 0;
    for parameter in member::formal_parameters(&ctx, super_constructor) {
        let parameter = member::base_element(&ctx, parameter);
        let Some(kind) = parameter_kind(v, parameter) else {
            continue;
        };
        if type_ext::is_positional(kind) {
            super_positional_count += 1;
        }
        if !type_ext::is_optional(kind) {
            continue;
        }
        if !deprecated_with_kind(v, parameter, "optional") {
            continue;
        }
        if type_ext::is_positional(kind) {
            if super_positional_count <= positional_argument_count + positional_super_argument_count
            {
                continue;
            }
        } else {
            let name = dartr_typesystem::TypeExt::element_name(&ctx, parameter);
            if name.is_some_and(|n| named_argument_names.iter().any(|a| a == n)) {
                continue;
            }
            if name.is_some_and(|n| named_super_argument_names.contains(&n)) {
                continue;
            }
        }
        let name = parameter_name(v, parameter);
        report(v, diag::deprecated_optional(&name), error_range);
    }
}

/// Dart `_checkForDeprecatedSubclass(namedTypes)`.
fn check_for_deprecated_subclass(
    v: &mut UnitVerifier<'_>,
    named_types: Option<Vec<Id<NamedType>>>,
) {
    let Some(named_types) = named_types else {
        return;
    };
    for named_type in named_types {
        let Some(element) = element_of(&v.ctx, v.tables, named_type) else {
            continue;
        };
        if in_current_library(v, element) {
            continue;
        }
        if type_is_interface(v, named_type) && deprecated_with_kind(v, element, "subclass") {
            let name = name_of(v, element);
            let range = node_range(v.ast, named_type);
            report(v, diag::deprecated_subclass(&name), range);
        }
    }
}
