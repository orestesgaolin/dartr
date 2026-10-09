// Dart source: pkg/analyzer/lib/src/error/required_parameters_verifier.dart

//! `RequiredParametersVerifier`: the arguments of invocations supply every
//! required named parameter (`missingRequiredArgument`) and every
//! parameter annotated with `@required` (`missingRequiredParam`). The
//! error verifier calls the `visit_*` functions (Dart
//! `_requiredParametersVerifier.visitX(node)`).
//!
//! The `@required` annotation of a parameter is read from the element
//! metadata (Dart `_requiredAnnotation`). The metadata of elements is not
//! resolved in the element model yet, so `missingRequiredParam` is not
//! reported.

use dartr_ast::{
    Annotation, Argument, ArgumentList, DotShorthandConstructorInvocation, DotShorthandInvocation,
    EnumConstantDeclaration, FunctionExpressionInvocation, Id, Identifier,
    InstanceCreationExpression, MethodInvocation, NamedArgument, NodeId, PrefixedIdentifier,
    RedirectingConstructorInvocation, SimpleIdentifier, SuperConstructorInvocation,
};
use dartr_diagnostics::{LocatableDiagnostic, LocatedDiagnostic, diag};
use dartr_element::{
    ElemRef, FnParam, FormalParameterElement, ParameterKind, Tag, TypeId, TypeKind,
};
use dartr_syntax::TokenId;
use dartr_typesystem::{TypeExt, member};

use super::VerifierHost;
use crate::ast_ext;

/// Dart `SyntacticEntity errorEntity`: a node or a token.
#[derive(Clone, Copy, Debug)]
pub enum ErrorEntity {
    Node(NodeId),
    Token(TokenId),
}

impl ErrorEntity {
    fn at<'a, H: VerifierHost<'a>>(self, host: &H, d: LocatableDiagnostic) -> LocatedDiagnostic {
        match self {
            ErrorEntity::Node(n) => host.at(d, n),
            ErrorEntity::Token(t) => host.at_token(d, t),
        }
    }
}

/// One formal parameter (Dart `FormalParameterElement`: `name`,
/// `isRequiredNamed`, `isOptionalNamed`, `metadata`).
#[derive(Clone, Copy, Debug)]
struct Parameter<'a> {
    name: Option<&'a str>,
    kind: ParameterKind,
}

/// Dart `RequiredParametersVerifier.visitAnnotation(node)`.
pub fn visit_annotation<'a, H: VerifierHost<'a>>(host: &mut H, node: Id<Annotation>) {
    let ctx = host.ctx();
    let Some(element) = host.element(node) else {
        return;
    };
    let ast = host.ast();
    let Some(argument_list) = ast[node].arguments else {
        return;
    };
    if member::base_element(&ctx, element).tag() != Tag::Constructor {
        return;
    }
    let error_node = constructor_identifier(host, node).or_else(|| class_identifier(host, node));
    if let Some(error_node) = error_node {
        let parameters = executable_parameters(host, Some(element));
        check(
            host,
            parameters,
            None,
            argument_list,
            ErrorEntity::Node(error_node.raw()),
        );
    }
}

/// Dart `RequiredParametersVerifier.visitDotShorthandConstructorInvocation(node)`.
pub fn visit_dot_shorthand_constructor_invocation<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<DotShorthandConstructorInvocation>,
) {
    let ctx = host.ctx();
    let ast = host.ast();
    let constructor_name = ast[node].constructor_name;
    let argument_list = ast[node].argument_list;
    let Some(constructor_element) = host.element(constructor_name) else {
        return;
    };
    if member::base_element(&ctx, constructor_element).tag() == Tag::Constructor {
        let parameters = executable_parameters(host, Some(constructor_element));
        check(
            host,
            parameters,
            None,
            argument_list,
            ErrorEntity::Node(constructor_name.raw()),
        );
    }
}

/// Dart `RequiredParametersVerifier.visitDotShorthandInvocation(node)`.
pub fn visit_dot_shorthand_invocation<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<DotShorthandInvocation>,
) {
    let ast = host.ast();
    let member_name = ast[node].member_name;
    let argument_list = ast[node].argument_list;
    let element = executable_element(host, host.element(member_name));
    let parameters = executable_parameters(host, element);
    check(
        host,
        parameters,
        None,
        argument_list,
        ErrorEntity::Node(member_name.raw()),
    );
}

/// Dart `RequiredParametersVerifier.visitEnumConstantDeclaration(node)`.
pub fn visit_enum_constant_declaration<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<EnumConstantDeclaration>,
) {
    let ast = host.ast();
    let name = ast[node].name;
    let argument_list = ast[node].arguments.map(|a| ast[a].argument_list);
    // Dart `node.constructorElement?.formalParameters`.
    let constructor_element = host.element(node);
    let parameters = executable_parameters(host, constructor_element);
    check_arguments(
        host,
        parameters,
        None,
        &arguments_of(host, argument_list),
        ErrorEntity::Token(name),
    );
}

/// Dart `RequiredParametersVerifier.visitFunctionExpressionInvocation(node)`.
pub fn visit_function_expression_invocation<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<FunctionExpressionInvocation>,
) {
    let argument_list = host.ast()[node].argument_list;
    let Some(ty) = host.tables().invoke_type.get(node).copied() else {
        return;
    };
    if let Some(parameters) = function_type_parameters(host, ty) {
        check(
            host,
            Some(parameters),
            None,
            argument_list,
            ErrorEntity::Node(node.raw()),
        );
    }
}

/// Dart `RequiredParametersVerifier.visitInstanceCreationExpression(node)`.
pub fn visit_instance_creation_expression<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<InstanceCreationExpression>,
) {
    let ast = host.ast();
    let constructor_name = ast[node].constructor_name;
    let argument_list = ast[node].argument_list;
    let element = host.element(constructor_name);
    let parameters = executable_parameters(host, element);
    check(
        host,
        parameters,
        None,
        argument_list,
        ErrorEntity::Node(constructor_name.raw()),
    );
}

/// Dart `RequiredParametersVerifier.visitMethodInvocation(node)`.
pub fn visit_method_invocation<'a, H: VerifierHost<'a>>(host: &mut H, node: Id<MethodInvocation>) {
    let ast = host.ast();
    let method_name = ast[node].method_name;
    let argument_list = ast[node].argument_list;
    if ast_ext::identifier_name(ast, method_name) == "call" {
        let target_type =
            ast_ext::method_invocation_real_target(ast, node).and_then(|t| host.static_type(t));
        if let Some(parameters) = target_type.and_then(|t| function_type_parameters(host, t)) {
            check(
                host,
                Some(parameters),
                None,
                argument_list,
                ErrorEntity::Node(argument_list.raw()),
            );
            return;
        }
    }

    let element = executable_element(host, host.element(method_name));
    let parameters = executable_parameters(host, element);
    check(
        host,
        parameters,
        None,
        argument_list,
        ErrorEntity::Node(method_name.raw()),
    );
}

/// Dart `RequiredParametersVerifier.visitRedirectingConstructorInvocation(node)`.
pub fn visit_redirecting_constructor_invocation<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<RedirectingConstructorInvocation>,
) {
    let argument_list = host.ast()[node].argument_list;
    let element = executable_element(host, host.element(node));
    let parameters = executable_parameters(host, element);
    check(
        host,
        parameters,
        None,
        argument_list,
        ErrorEntity::Node(node.raw()),
    );
}

/// Dart `RequiredParametersVerifier.visitSuperConstructorInvocation(node,
/// enclosingConstructor:)`.
pub fn visit_super_constructor_invocation<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<SuperConstructorInvocation>,
    enclosing_constructor: Option<ElemRef>,
) {
    let argument_list = host.ast()[node].argument_list;
    let element = executable_element(host, host.element(node));
    let parameters = executable_parameters(host, element);
    check(
        host,
        parameters,
        enclosing_constructor,
        argument_list,
        ErrorEntity::Node(node.raw()),
    );
}

/// Dart `_check(parameters:, enclosingConstructor:, arguments:,
/// errorEntity:)` with the arguments of [argument_list].
fn check<'a, H: VerifierHost<'a>>(
    host: &mut H,
    parameters: Option<Vec<Parameter<'a>>>,
    enclosing_constructor: Option<ElemRef>,
    argument_list: Id<ArgumentList>,
    error_entity: ErrorEntity,
) {
    let arguments = arguments_of(host, Some(argument_list));
    check_arguments(
        host,
        parameters,
        enclosing_constructor,
        &arguments,
        error_entity,
    );
}

/// Dart `_check(parameters:, enclosingConstructor:, arguments:,
/// errorEntity:)`.
fn check_arguments<'a, H: VerifierHost<'a>>(
    host: &mut H,
    parameters: Option<Vec<Parameter<'a>>>,
    enclosing_constructor: Option<ElemRef>,
    arguments: &[Id<Argument>],
    error_entity: ErrorEntity,
) {
    let Some(parameters) = parameters else {
        return;
    };

    for parameter in parameters {
        if parameter.kind.is_required_named() {
            let Some(parameter_name) = parameter.name else {
                continue;
            };

            if !contains_named_expression(host, enclosing_constructor, arguments, parameter_name) {
                let d = error_entity.at(host, diag::missing_required_argument(parameter_name));
                host.report(d);
            }
        }
        if parameter.kind.is_optional_named() {
            // Dart `_requiredAnnotation(parameter)`: the `@required`
            // annotation of the parameter (`getReason` reads its `reason`
            // field). The metadata of elements is not resolved yet, so no
            // parameter has the annotation here; with it, Dart reports
            // `missingRequiredParam` (or `missingRequiredParamWithDetails`
            // with the reason) at [error_entity] when no argument names the
            // parameter.
        }
    }
}

/// Dart `_containsNamedExpression(enclosingConstructor, arguments, name)`.
fn contains_named_expression<'a, H: VerifierHost<'a>>(
    host: &H,
    enclosing_constructor: Option<ElemRef>,
    arguments: &[Id<Argument>],
    name: &str,
) -> bool {
    let ast = host.ast();
    for &argument in arguments.iter().rev() {
        if let Some(argument) = ast.cast::<NamedArgument>(argument)
            && ast.tokens.lexeme(ast[argument].name) == name
        {
            return true;
        }
    }

    if let Some(enclosing_constructor) = enclosing_constructor {
        let ctx = host.ctx();
        return member::formal_parameters(&ctx, enclosing_constructor)
            .into_iter()
            .any(|e| {
                let base = member::base_element(&ctx, e);
                base.tag() == Tag::SuperFormalParameter
                    && base
                        .cast::<FormalParameterElement>()
                        .is_some_and(|p| ctx.get(p).kind.is_named())
                    && ctx.element_name(base) == Some(name)
            });
    }

    false
}

/// Dart `_executableElement(element)`: [element] if it is an executable
/// element.
fn executable_element<'a, H: VerifierHost<'a>>(
    host: &H,
    element: Option<ElemRef>,
) -> Option<ElemRef> {
    let ctx = host.ctx();
    element.filter(|&e| {
        matches!(
            member::base_element(&ctx, e).tag(),
            Tag::Constructor
                | Tag::Method
                | Tag::Getter
                | Tag::Setter
                | Tag::TopLevelFunction
                | Tag::LocalFunction
        )
    })
}

/// Dart `executableElement?.formalParameters` (name and kind of each).
fn executable_parameters<'a, H: VerifierHost<'a>>(
    host: &H,
    element: Option<ElemRef>,
) -> Option<Vec<Parameter<'a>>> {
    let ctx = host.ctx();
    let element = element?;
    if !matches!(
        member::base_element(&ctx, element).tag(),
        Tag::Constructor
            | Tag::Method
            | Tag::Getter
            | Tag::Setter
            | Tag::TopLevelFunction
            | Tag::LocalFunction
    ) {
        return None;
    }
    Some(
        member::formal_parameters(&ctx, element)
            .into_iter()
            .filter_map(|p| {
                let base = member::base_element(&ctx, p).cast::<FormalParameterElement>()?;
                Some(Parameter {
                    name: ctx.element_name(base.raw()),
                    kind: ctx.get(base).kind,
                })
            })
            .collect(),
    )
}

/// Dart `type.formalParameters` of a function type (`None` for other
/// types).
fn function_type_parameters<'a, H: VerifierHost<'a>>(
    host: &H,
    ty: TypeId,
) -> Option<Vec<Parameter<'a>>> {
    let ctx = host.ctx();
    let TypeKind::Function(f) = ctx.ty(ty) else {
        return None;
    };
    Some(
        ctx.list(f.params)
            .iter()
            .map(|p: &FnParam| Parameter {
                name: p.name.map(|n| ctx.name_str(n)),
                kind: p.kind,
            })
            .collect(),
    )
}

/// The arguments of [argument_list] (empty without a list).
fn arguments_of<'a, H: VerifierHost<'a>>(
    host: &H,
    argument_list: Option<Id<ArgumentList>>,
) -> Vec<Id<Argument>> {
    let ast = host.ast();
    match argument_list {
        Some(list) => ast.list(ast[list].arguments).to_vec(),
        None => Vec::new(),
    }
}

/// Dart `_InstantiatedAnnotation.classIdentifier`: the name of the
/// annotation (or its last part) when it is an interface element.
fn class_identifier<'a, H: VerifierHost<'a>>(
    host: &H,
    node: Id<Annotation>,
) -> Option<Id<SimpleIdentifier>> {
    let name = annotation_name_identifier(host, node)?;
    if_element_tag(host, Some(name), |tag| {
        matches!(
            tag,
            Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType
        )
    })
}

/// Dart `_InstantiatedAnnotation.constructorIdentifier`: the constructor
/// name, or the name of the annotation (or its last part), when it is a
/// constructor element.
fn constructor_identifier<'a, H: VerifierHost<'a>>(
    host: &H,
    node: Id<Annotation>,
) -> Option<Id<SimpleIdentifier>> {
    let is_constructor = |tag: Tag| tag == Tag::Constructor;
    let constructor_name = host.ast()[node].constructor_name;
    if let Some(n) = if_element_tag(host, constructor_name, is_constructor) {
        return Some(n);
    }
    let name = annotation_name_identifier(host, node)?;
    if_element_tag(host, Some(name), is_constructor)
}

/// The `SimpleIdentifier` of the name of [node]: the name itself, or the
/// identifier of a prefixed name.
fn annotation_name_identifier<'a, H: VerifierHost<'a>>(
    host: &H,
    node: Id<Annotation>,
) -> Option<Id<SimpleIdentifier>> {
    let ast = host.ast();
    let name: Id<Identifier> = ast[node].name;
    if let Some(simple) = ast.cast::<SimpleIdentifier>(name) {
        Some(simple)
    } else {
        ast.cast::<PrefixedIdentifier>(name)
            .map(|p| ast[p].identifier)
    }
}

/// [node] if its element has a tag that satisfies [test].
fn if_element_tag<'a, H: VerifierHost<'a>>(
    host: &H,
    node: Option<Id<SimpleIdentifier>>,
    test: impl Fn(Tag) -> bool,
) -> Option<Id<SimpleIdentifier>> {
    let node = node?;
    let ctx = host.ctx();
    let element = host.element(node)?;
    test(member::base_element(&ctx, element).tag()).then_some(node)
}
