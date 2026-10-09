// Dart source: pkg/analyzer/lib/src/dart/resolver/invocation_inferrer.dart,
// pkg/analyzer/lib/src/dart/type_instantiation_target.dart (InvocationTarget),
// pkg/_fe_analyzer_shared/lib/src/deferred_function_literal_heuristic.dart,
// pkg/_fe_analyzer_shared/lib/src/util/dependency_walker.dart,
// pkg/analyzer/lib/src/generated/resolver.dart (resolveArgumentsToParameters,
// _reportNotEnoughPositionalArguments)

//! `InvocationInferrer` and its specializations: the downward inference of
//! the type arguments of an invocation, the resolution of the arguments
//! (with the deferred function literals of `inference-update-1`), the
//! upward inference, and the mapping of the arguments to the parameters.
//!
//! The Dart class hierarchy (`FullInvocationInferrer`,
//! `InvocationExpressionInferrer`, `MethodInvocationInferrer`, ...) is one
//! struct [`InvocationInferrer`] with the specialization in
//! [`InferrerKind`]; the base `InvocationInferrer` (extension overrides,
//! redirecting and super constructor invocations) is
//! [`resolve_invocation_base`].
//!
//! Not ported: the `whyNotPromotedArguments` list (it only feeds
//! `checkForArgumentTypesNotAssignableInList`, a check of wave D), the
//! inference log, and `dataForTesting`.

use std::cell::RefCell;

use dartr_ast::{
    Annotation, ArgumentList, AsExpression, ConstructorName, DotShorthandConstructorInvocation,
    DotShorthandInvocation, EnumConstantArguments, EnumConstantDeclaration, Expression,
    FunctionExpression, FunctionExpressionInvocation, Id, InstanceCreationExpression,
    MethodInvocation, NamedArgument, NodeId, PrefixedIdentifier, RedirectingConstructorInvocation,
    SimpleIdentifier, SuperConstructorInvocation, TypeArgumentList,
};
use dartr_diagnostics::{Diagnostic, DiagnosticReporter, LocatableDiagnostic, diag};
use dartr_element::diagnostics::type_arg;
use dartr_element::{
    EId, ElemRef, ElementId, ExtensionElement, FnParam, Tag, TypeId, TypeKind, TypeParameterElement,
};
use dartr_flow::flow_analysis::FlowAnalysis;
use dartr_flow::shared_type::SharedTypeView;
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_typesystem::generic_inferrer::{
    GenericInferrer, InferenceErrorEntity, InferenceErrorEntityKind, InferenceFlags,
    SimpleIdentifierElementFacts,
};
use dartr_typesystem::type_algebra::{MapSubstitution, get_fresh_type_parameters};
use dartr_typesystem::{TypeExt, member};
use indexmap::{IndexMap, IndexSet};

use crate::ast_ext;
use crate::flow_analysis_visitor::ResolverExpressionInfo;
use crate::resolver::ResolverVisitor;

// ------------------------------------------------------------------ targets

/// Dart `InvocationTarget` (type_instantiation_target.dart): the entity that
/// arguments (and type arguments) are applied to.
#[derive(Clone, Copy, Debug)]
pub enum InvocationTarget {
    /// Dart `InvocationTargetExecutableElement` (a method, a function, a
    /// getter): the raw type is the type of the element.
    ExecutableElement(ElemRef),
    /// Dart `InvocationTargetConstructorElement`.
    ConstructorElement { element: ElemRef, raw_type: TypeId },
    /// Dart `InvocationTargetExtensionOverride`.
    ExtensionOverride {
        element: EId<ExtensionElement>,
        ty: TypeId,
    },
    /// Dart `InvocationTargetFunctionTypedExpression`.
    FunctionTypedExpression(TypeId),
}

impl InvocationTarget {
    /// Dart `InvocationTargetFunctionTypedExpression.orNull(type)`.
    pub fn function_typed_expression_or_null(
        rv: &ResolverVisitor<'_>,
        ty: Option<TypeId>,
    ) -> Option<InvocationTarget> {
        let ty = ty?;
        matches!(rv.ctx.ty(ty), TypeKind::Function(_))
            .then_some(InvocationTarget::FunctionTypedExpression(ty))
    }

    /// Dart `rawType`: the function type of the invoked entity before type
    /// argument instantiation.
    pub fn raw_type(&self, rv: &ResolverVisitor<'_>) -> TypeId {
        match *self {
            InvocationTarget::ExecutableElement(e) => member::type_(&rv.ctx, e),
            InvocationTarget::ConstructorElement { raw_type, .. } => raw_type,
            InvocationTarget::ExtensionOverride { ty, .. } => ty,
            InvocationTarget::FunctionTypedExpression(ty) => ty,
        }
    }

    /// Dart `wrongNumberOfTypeArgumentsError(typeParameterCount:,
    /// typeArgumentCount:)`.
    pub fn wrong_number_of_type_arguments_error(
        &self,
        rv: &ResolverVisitor<'_>,
        type_parameter_count: usize,
        type_argument_count: usize,
    ) -> LocatableDiagnostic {
        let ctx = rv.ctx;
        match *self {
            InvocationTarget::ExecutableElement(e) => {
                let base = member::base_element(&ctx, e);
                diag::wrong_number_of_type_arguments_element(
                    base.kind().display_name(),
                    ctx.element_name(base).unwrap_or(""),
                    type_parameter_count as i64,
                    type_argument_count as i64,
                )
            }
            InvocationTarget::ConstructorElement { element, .. } => {
                // The type parameters are declared by the enclosing class.
                let base = member::base_element(&ctx, element);
                let enclosing = ctx
                    .element_data(base)
                    .and_then(|d| d.enclosing)
                    .unwrap_or(base);
                diag::wrong_number_of_type_arguments_element(
                    enclosing.kind().display_name(),
                    ctx.element_name(enclosing).unwrap_or(""),
                    type_parameter_count as i64,
                    type_argument_count as i64,
                )
            }
            InvocationTarget::ExtensionOverride { element, .. } => {
                diag::wrong_number_of_type_arguments_extension(
                    ctx.element_name(element.raw()).unwrap_or(""),
                    type_parameter_count as i64,
                    type_argument_count as i64,
                )
            }
            InvocationTarget::FunctionTypedExpression(ty) => {
                diag::wrong_number_of_type_arguments_function(
                    type_arg(&ctx, ty),
                    type_parameter_count as i64,
                    type_argument_count as i64,
                )
            }
        }
    }
}

// ------------------------------------------------------------------ inferrer

/// The specialization of [`InvocationInferrer`] (the Dart subclasses of
/// `FullInvocationInferrer`).
#[derive(Clone, Copy, Debug)]
pub enum InferrerKind {
    /// Dart `MethodInvocationInferrer`.
    MethodInvocation(Id<MethodInvocation>),
    /// Dart `FunctionExpressionInvocationInferrer`.
    FunctionExpressionInvocation(Id<FunctionExpressionInvocation>),
    /// Dart `DotShorthandInvocationInferrer`.
    DotShorthandInvocation(Id<DotShorthandInvocation>),
    /// Dart `InstanceCreationInferrer`.
    InstanceCreation(Id<InstanceCreationExpression>),
    /// Dart `DotShorthandConstructorInvocationInferrer`.
    DotShorthandConstructorInvocation(Id<DotShorthandConstructorInvocation>),
    /// Dart `AnnotationInferrer`.
    Annotation {
        node: Id<Annotation>,
        constructor_name: Option<Id<SimpleIdentifier>>,
    },
    /// The base Dart `InvocationInferrer` (no type arguments, no inference):
    /// `ExtensionOverride`, `RedirectingConstructorInvocation`,
    /// `SuperConstructorInvocation`. See [`resolve_invocation_base`].
    Base(NodeId),
}

/// Dart `FullInvocationInferrer` (with its subclasses, see
/// [`InferrerKind`]).
#[derive(Clone, Copy, Debug)]
pub struct InvocationInferrer {
    pub kind: InferrerKind,
    pub argument_list: Id<ArgumentList>,
    pub context_type: TypeId,
    pub target: Option<InvocationTarget>,
}

/// The key of a parameter in a parameter map: the name of a named
/// parameter, the index of a positional one.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum ParamKey {
    Named(String),
    Positional(usize),
}

/// Dart `_DeferredParamInfo`.
#[derive(Clone, Debug)]
struct DeferredParamInfo {
    parameter: Option<FnParam>,
    /// The function literal.
    value: Id<FunctionExpression>,
    /// The index of the argument in the argument list.
    index: usize,
    parameter_key: ParamKey,
}

/// Dart `_IdenticalArgumentInfo`.
#[derive(Clone)]
struct IdenticalArgumentInfo<'a> {
    expression_info: Option<ResolverExpressionInfo<'a>>,
    static_type: TypeId,
}

/// Dart `_computeParameterMap(parameters)`.
fn compute_parameter_map(
    rv: &ResolverVisitor<'_>,
    parameters: &[FnParam],
) -> IndexMap<ParamKey, FnParam> {
    let mut unnamed_parameter_index = 0;
    let mut map = IndexMap::new();
    for p in parameters {
        let key = if p.kind.is_named() {
            ParamKey::Named(p.name.map(|n| rv.ctx.name_str(n)).unwrap_or("").to_string())
        } else {
            let k = ParamKey::Positional(unnamed_parameter_index);
            unnamed_parameter_index += 1;
            k
        };
        map.insert(key, *p);
    }
    map
}

/// Dart `_computeExplicitlyTypedParameterSet(functionExpression)`.
fn compute_explicitly_typed_parameter_set(
    rv: &ResolverVisitor<'_>,
    function: Id<FunctionExpression>,
) -> IndexSet<ParamKey> {
    let mut result = IndexSet::new();
    let Some(list) = rv.ast[function].parameters else {
        return result;
    };
    let mut unnamed_parameter_index = 0;
    for &p in rv.ast.list(rv.ast[list].parameters) {
        let parts = ast_ext::formal_parameter_parts(rv.ast, p.raw());
        let key = if parts.kind.is_named() {
            ParamKey::Named(
                parts
                    .name
                    .map(|t| rv.lexeme(t).to_string())
                    .unwrap_or_default(),
            )
        } else {
            let k = ParamKey::Positional(unnamed_parameter_index);
            unnamed_parameter_index += 1;
            k
        };
        if parts.type_.is_some() || parts.function_typed_suffix.is_some() {
            result.insert(key);
        }
    }
    result
}

/// The parameters of the function type [ty] (Dart `formalParameters`), or
/// none if it is not a function type.
pub fn function_type_parameters(rv: &ResolverVisitor<'_>, ty: Option<TypeId>) -> Vec<FnParam> {
    match ty.map(|t| *rv.ctx.ty(t)) {
        Some(TypeKind::Function(f)) => rv.ctx.list(f.params).to_vec(),
        _ => Vec::new(),
    }
}

/// The type parameters of the function type [ty].
fn function_type_type_parameters(
    rv: &ResolverVisitor<'_>,
    ty: TypeId,
) -> Vec<EId<TypeParameterElement>> {
    match *rv.ctx.ty(ty) {
        TypeKind::Function(f) => rv.ctx.list(f.type_params).to_vec(),
        _ => Vec::new(),
    }
}

/// The name of a parameter (Dart `parameter.name ?? ''`).
fn param_name(rv: &ResolverVisitor<'_>, p: &FnParam) -> String {
    p.name.map(|n| rv.ctx.name_str(n)).unwrap_or("").to_string()
}

/// One argument of an argument list.
struct ArgumentInfo {
    /// The argument node (the expression, or the `NamedArgument`).
    node: NodeId,
    /// Dart `argument.argumentExpression`.
    expression: Id<Expression>,
    /// The name of a named argument.
    name: Option<String>,
}

/// The arguments of [argument_list].
fn arguments_of(rv: &ResolverVisitor<'_>, argument_list: Id<ArgumentList>) -> Vec<ArgumentInfo> {
    let ast = &*rv.ast;
    ast.list(ast[argument_list].arguments)
        .iter()
        .map(|&a| match ast.cast::<NamedArgument>(a) {
            Some(n) => ArgumentInfo {
                node: a.raw(),
                expression: ast[n].argument_expression,
                name: Some(rv.lexeme(ast[n].name).to_string()),
            },
            None => ArgumentInfo {
                node: a.raw(),
                expression: Id::from_raw(a.raw()),
                name: None,
            },
        })
        .collect()
}

/// Dart `typeArgument.typeOrThrow` of each type argument of [list].
pub fn type_argument_types(rv: &ResolverVisitor<'_>, list: Id<TypeArgumentList>) -> Vec<TypeId> {
    rv.ast
        .list(rv.ast[list].arguments)
        .iter()
        .map(|&t| {
            rv.tables
                .annotation_type
                .get(t)
                .copied()
                .unwrap_or(TypeId::DYNAMIC)
        })
        .collect()
}

/// A buffer of the diagnostics that a [`GenericInferrer`] reports, moved to
/// the resolver's diagnostics in report order by [`flush_inferrer_diagnostics`].
type DiagnosticBuffer = RefCell<Vec<Diagnostic>>;

/// Moves the diagnostics of [buffer] to the resolver.
fn flush_inferrer_diagnostics(rv: &mut ResolverVisitor<'_>, buffer: &DiagnosticBuffer) {
    let reported: Vec<Diagnostic> = std::mem::take(&mut *buffer.borrow_mut());
    if reported.is_empty() {
        return;
    }
    rv.flush_type_analyzer_errors();
    if rv.lock_level == 0 {
        rv.diagnostics.extend(reported);
    }
}

impl InvocationInferrer {
    /// The invocation node (Dart `node`).
    pub fn node(&self) -> NodeId {
        match self.kind {
            InferrerKind::MethodInvocation(n) => n.raw(),
            InferrerKind::FunctionExpressionInvocation(n) => n.raw(),
            InferrerKind::DotShorthandInvocation(n) => n.raw(),
            InferrerKind::InstanceCreation(n) => n.raw(),
            InferrerKind::DotShorthandConstructorInvocation(n) => n.raw(),
            InferrerKind::Annotation { node, .. } => node.raw(),
            InferrerKind::Base(n) => n,
        }
    }

    /// Dart `_errorEntity`.
    fn error_entity(&self, rv: &ResolverVisitor<'_>) -> NodeId {
        let ast = &*rv.ast;
        match self.kind {
            // Dart `InvocationExpression.function`.
            InferrerKind::MethodInvocation(n) => ast[n].method_name.raw(),
            InferrerKind::FunctionExpressionInvocation(n) => ast[n].function.raw(),
            InferrerKind::DotShorthandInvocation(n) => ast[n].member_name.raw(),
            InferrerKind::InstanceCreation(n) => ast[n].constructor_name.raw(),
            InferrerKind::DotShorthandConstructorInvocation(n) => ast[n].constructor_name.raw(),
            InferrerKind::Annotation { node, .. } => node.raw(),
            InferrerKind::Base(n) => n,
        }
    }

    /// Dart `_isConst`.
    fn is_const(&self, rv: &ResolverVisitor<'_>) -> bool {
        match self.kind {
            InferrerKind::InstanceCreation(n) => ast_ext::instance_creation_is_const(rv.ast, n),
            InferrerKind::DotShorthandConstructorInvocation(n) => {
                rv.ast[n].const_keyword.is_some() || ast_ext::in_constant_context(rv.ast, n.raw())
            }
            InferrerKind::Annotation { .. } => true,
            _ => false,
        }
    }

    /// Dart `_isGenericInferenceDisabled`.
    fn is_generic_inference_disabled(&self, rv: &ResolverVisitor<'_>) -> bool {
        match self.kind {
            InferrerKind::Annotation { .. } => !rv.generic_metadata_is_enabled(),
            _ => false,
        }
    }

    /// Dart `_needsTypeArgumentBoundsCheck`.
    fn needs_type_argument_bounds_check(&self) -> bool {
        matches!(
            self.kind,
            InferrerKind::InstanceCreation(_)
                | InferrerKind::DotShorthandConstructorInvocation(_)
                | InferrerKind::Annotation { .. }
        )
    }

    /// Dart `_typeArguments`.
    fn type_arguments(&self, rv: &ResolverVisitor<'_>) -> Option<Id<TypeArgumentList>> {
        let ast = &*rv.ast;
        match self.kind {
            InferrerKind::MethodInvocation(n) => ast[n].type_arguments,
            InferrerKind::FunctionExpressionInvocation(n) => ast[n].type_arguments,
            InferrerKind::DotShorthandInvocation(n) => ast[n].type_arguments,
            // For an instance creation expression the type arguments are on
            // the constructor name.
            InferrerKind::InstanceCreation(n) => {
                let constructor_name = ast[n].constructor_name;
                ast[ast[constructor_name].type_].type_arguments
            }
            InferrerKind::DotShorthandConstructorInvocation(n) => ast[n].type_arguments,
            InferrerKind::Annotation { node, .. } => ast[node].type_arguments,
            InferrerKind::Base(_) => None,
        }
    }

    /// Dart `_isIdentical`.
    fn is_identical(&self, rv: &ResolverVisitor<'_>) -> bool {
        let InferrerKind::MethodInvocation(n) = self.kind else {
            return false;
        };
        let method_name = rv.ast[n].method_name;
        let Some(ElemRef::Base(e)) = rv.element(method_name) else {
            return false;
        };
        let argument_count = rv.ast.list(rv.ast[rv.ast[n].argument_list].arguments).len();
        e.tag() == Tag::TopLevelFunction && is_dart_core_identical(rv, e) && argument_count == 2
    }

    /// `node.methodName.element` of a method invocation, as a method
    /// element.
    fn invoked_method_element(
        &self,
        rv: &ResolverVisitor<'_>,
        n: Id<MethodInvocation>,
    ) -> Option<EId<dartr_element::MethodElement>> {
        let e = rv.element(rv.ast[n].method_name)?;
        let base = member::base_element(&rv.ctx, e);
        (base.tag() == Tag::Method).then(|| EId::from_raw(base))
    }

    /// Dart `_computeContextForArgument(parameterType)`.
    fn compute_context_for_argument(
        &self,
        rv: &ResolverVisitor<'_>,
        parameter_type: TypeId,
    ) -> TypeId {
        let InferrerKind::MethodInvocation(n) = self.kind else {
            return parameter_type;
        };
        let mut argument_context_type = parameter_type;
        let target_type =
            ast_ext::method_invocation_real_target(rv.ast, n).and_then(|t| rv.static_type(t));
        if let Some(target_type) = target_type {
            argument_context_type = rv.type_system.refine_numeric_invocation_context(
                Some(target_type),
                self.invoked_method_element(rv, n),
                self.context_type,
                parameter_type,
            );
        }
        argument_context_type
    }

    /// Dart `_refineReturnType(returnType)`.
    fn refine_return_type(&self, rv: &ResolverVisitor<'_>, return_type: TypeId) -> TypeId {
        let InferrerKind::MethodInvocation(n) = self.kind else {
            return return_type;
        };
        let target_type =
            ast_ext::method_invocation_real_target(rv.ast, n).and_then(|t| rv.static_type(t));
        let Some(target_type) = target_type else {
            return return_type;
        };
        let argument_types: Vec<TypeId> = arguments_of(rv, self.argument_list)
            .iter()
            .map(|a| rv.static_type(a.expression).unwrap_or(TypeId::DYNAMIC))
            .collect();
        rv.type_system.refine_numeric_invocation_type(
            target_type,
            self.invoked_method_element(rv, n),
            &argument_types,
            return_type,
        )
    }

    /// Dart `_reportWrongNumberOfTypeArguments(typeArgumentList,
    /// typeParameters)`.
    fn report_wrong_number_of_type_arguments(
        &self,
        rv: &mut ResolverVisitor<'_>,
        type_argument_list: Id<TypeArgumentList>,
        type_parameter_count: usize,
    ) {
        match self.kind {
            // Error reporting for instance creations and dot shorthand
            // constructor invocations is done elsewhere.
            InferrerKind::InstanceCreation(_)
            | InferrerKind::DotShorthandConstructorInvocation(_) => {}
            _ => {
                let Some(target) = self.target else {
                    return;
                };
                let count = rv.ast.list(rv.ast[type_argument_list].arguments).len();
                let d =
                    target.wrong_number_of_type_arguments_error(rv, type_parameter_count, count);
                let d = rv.at(d, type_argument_list);
                rv.report(d);
            }
        }
    }

    /// Dart `_storeResult(typeArgumentTypes, invokeType)`: records the
    /// results and returns the parameters to map the arguments to.
    fn store_result(
        &self,
        rv: &mut ResolverVisitor<'_>,
        type_argument_types: Option<&[TypeId]>,
        invoke_type: Option<TypeId>,
    ) -> Option<Vec<FnParam>> {
        let ctx = rv.ctx;
        match self.kind {
            InferrerKind::MethodInvocation(_)
            | InferrerKind::FunctionExpressionInvocation(_)
            | InferrerKind::DotShorthandInvocation(_) => {
                // Dart `InvocationExpressionInferrer._storeResult`.
                let node = self.node();
                match type_argument_types {
                    Some(types) => {
                        let list = ctx.intern_list(types);
                        rv.tables.type_arg_types.insert(node, list);
                    }
                    None => {
                        rv.tables.type_arg_types.remove(node);
                    }
                }
                rv.tables
                    .invoke_type
                    .insert(node, invoke_type.unwrap_or(TypeId::DYNAMIC));
                invoke_type.map(|t| function_type_parameters(rv, Some(t)))
            }
            InferrerKind::InstanceCreation(n) => {
                let invoke_type = invoke_type?;
                let constructed_type =
                    InvocationInferrer::compute_invoke_return_type(rv, Some(invoke_type));
                let constructor_name = rv.ast[n].constructor_name;
                let named_type = rv.ast[constructor_name].type_;
                rv.tables
                    .annotation_type
                    .insert(named_type, constructed_type);
                let base = member::base_element(&ctx, rv.element(constructor_name)?);
                let constructor_element = member::constructor_from2(&ctx, base, constructed_type);
                rv.set_element(constructor_name, Some(constructor_element));
                Some(function_type_parameters(
                    rv,
                    Some(member::type_(&ctx, constructor_element)),
                ))
            }
            InferrerKind::DotShorthandConstructorInvocation(n) => {
                let invoke_type = invoke_type?;
                let constructed_type =
                    InvocationInferrer::compute_invoke_return_type(rv, Some(invoke_type));
                let constructor_name = rv.ast[n].constructor_name;
                let element = rv.element(n).or_else(|| rv.element(constructor_name))?;
                let base = member::base_element(&ctx, element);
                let constructor_element = member::constructor_from2(&ctx, base, constructed_type);
                rv.set_element(constructor_name, Some(constructor_element));
                Some(function_type_parameters(
                    rv,
                    Some(member::type_(&ctx, constructor_element)),
                ))
            }
            InferrerKind::Annotation {
                node,
                constructor_name,
            } => {
                let invoke_type = invoke_type?;
                let constructed_type =
                    InvocationInferrer::compute_invoke_return_type(rv, Some(invoke_type));
                let base = member::base_element(&ctx, rv.element(node)?);
                let constructor_element = member::constructor_from2(&ctx, base, constructed_type);
                if let Some(c) = constructor_name {
                    rv.set_element(c, Some(constructor_element));
                }
                rv.set_element(node, Some(constructor_element));
                Some(function_type_parameters(
                    rv,
                    Some(member::type_(&ctx, constructor_element)),
                ))
            }
            InferrerKind::Base(_) => invoke_type.map(|t| function_type_parameters(rv, Some(t))),
        }
    }

    /// The error entity for the generic inferrer (what
    /// `_reportInferenceFailure` reads from the Dart node).
    fn inference_error_entity(&self, rv: &ResolverVisitor<'_>) -> InferenceErrorEntity {
        let entity = self.error_entity(rv);
        let ast = &*rv.ast;
        let offset = ast.offset(entity) as usize;
        let length = ast.length(entity) as usize;
        let is_invocation_in_as_expression = ast.parent(entity).is_some_and(|p| {
            (ast.is::<MethodInvocation>(p)
                || ast.is::<FunctionExpressionInvocation>(p)
                || ast.is::<DotShorthandInvocation>(p))
                && ast.parent(p).is_some_and(|pp| ast.is::<AsExpression>(pp))
        });
        // The facts that need `@optionalTypeArgs` metadata are only read with
        // the `strict-inference` option; the metadata is not checked yet.
        let kind = if let Some(c) = ast.cast::<ConstructorName>(entity) {
            let named_type = ast[c].type_;
            let type_name = ast.qualified_name(named_type);
            let constructor_name = match ast[c].name {
                None => type_name,
                Some(name) => format!("{}.{}", type_name, rv.lexeme(ast[name].token)),
            };
            InferenceErrorEntityKind::ConstructorName {
                type_element_has_optional_type_args: false,
                constructor_name,
            }
        } else if let Some(a) = ast.cast::<Annotation>(entity) {
            let name = ast.simple_name(ast[a].name);
            let name = rv.lexeme(ast[name].token).to_string();
            let constructor_name = match ast[a].constructor_name {
                None => name,
                Some(c) => format!("{}.{}", name, rv.lexeme(ast[c].token)),
            };
            InferenceErrorEntityKind::Annotation {
                element_has_optional_type_args: rv.element(ast[a].name).map(|_| false),
                constructor_name,
            }
        } else if let Some(s) = ast.cast::<SimpleIdentifier>(entity) {
            InferenceErrorEntityKind::SimpleIdentifier {
                name: rv.lexeme(ast[s].token).to_string(),
                element: rv.element(s).map(|_| SimpleIdentifierElementFacts {
                    variable_type_has_optional_type_args: false,
                    has_optional_type_args: false,
                }),
            }
        } else if ast.is::<Expression>(entity) {
            InferenceErrorEntityKind::Expression {
                static_type: rv.static_type(entity),
            }
        } else {
            InferenceErrorEntityKind::Other
        };
        InferenceErrorEntity {
            offset,
            length,
            is_invocation_in_as_expression,
            kind,
        }
    }

    /// Dart `FullInvocationInferrer.resolveInvocation()`: returns the static
    /// type of the invocation.
    pub fn resolve_invocation(&self, rv: &mut ResolverVisitor<'_>) -> TypeId {
        let ctx = rv.ctx;
        let mut raw_type = self.target.map(|t| t.raw_type(rv));
        // A target whose raw type is not a function type (recovery).
        if raw_type.is_some_and(|t| !matches!(ctx.ty(t), TypeKind::Function(_))) {
            raw_type = None;
        }
        let type_argument_list = self.type_arguments(rv);
        let original_type = raw_type;

        let buffer: DiagnosticBuffer = RefCell::new(Vec::new());
        let mut listener = |d: Diagnostic| buffer.borrow_mut().push(d);
        let mut reporter = DiagnosticReporter::new(&mut listener);

        let mut type_argument_types: Option<Vec<TypeId>> = None;
        let mut inferrer: Option<GenericInferrer<'_, '_, '_>> = None;
        let mut substitution: Option<MapSubstitution> = None;
        if self.is_generic_inference_disabled(rv) {
            let type_parameters = raw_type
                .map(|t| function_type_type_parameters(rv, t))
                .unwrap_or_default();
            if !type_parameters.is_empty() {
                let types = vec![TypeId::DYNAMIC; type_parameters.len()];
                substitution = Some(MapSubstitution::from_pairs(&type_parameters, &types));
                type_argument_types = Some(types);
            } else {
                type_argument_types = Some(Vec::new());
            }
        } else if let Some(type_argument_list) = type_argument_list {
            let arguments = self::type_argument_types(rv, type_argument_list);
            let type_parameters = raw_type.map(|t| function_type_type_parameters(rv, t));
            let types = match &type_parameters {
                Some(type_parameters) if arguments.len() != type_parameters.len() => {
                    self.report_wrong_number_of_type_arguments(
                        rv,
                        type_argument_list,
                        type_parameters.len(),
                    );
                    vec![TypeId::DYNAMIC; type_parameters.len()]
                }
                _ => {
                    if let Some(type_parameters) = &type_parameters
                        && self.needs_type_argument_bounds_check()
                    {
                        let substitution = MapSubstitution::from_pairs(type_parameters, &arguments);
                        let argument_nodes =
                            rv.ast.list(rv.ast[type_argument_list].arguments).to_vec();
                        for (i, &type_parameter) in type_parameters.iter().enumerate() {
                            let Some(bound) = ctx.type_parameter_bound(type_parameter) else {
                                continue;
                            };
                            let bound = substitution.substitute_type(&ctx, bound);
                            let type_argument = arguments[i];
                            if !rv.type_system.is_subtype_of(type_argument, bound) {
                                let name = ctx
                                    .element_name(type_parameter.raw())
                                    .unwrap_or("")
                                    .to_string();
                                let d = diag::type_argument_not_matching_bounds(
                                    type_arg(&ctx, type_argument),
                                    &name,
                                    type_arg(&ctx, bound),
                                );
                                let d = rv.at(d, argument_nodes[i]);
                                rv.report(d);
                            }
                        }
                    }
                    arguments
                }
            };
            if let Some(type_parameters) = &type_parameters {
                substitution = Some(MapSubstitution::from_pairs(type_parameters, &types));
            }
            type_argument_types = Some(types);
        } else if raw_type.is_none_or(|t| function_type_type_parameters(rv, t).is_empty()) {
            type_argument_types = Some(Vec::new());
        } else if let Some(raw) = raw_type {
            let type_parameters = function_type_type_parameters(rv, raw);
            let fresh = get_fresh_type_parameters(&ctx, &type_parameters);
            let raw = fresh.apply_to_function_type(&ctx, raw);
            raw_type = Some(raw);
            let TypeKind::Function(f) = *ctx.ty(raw) else {
                unreachable!("a function type");
            };
            let fresh_type_parameters = ctx.list(f.type_params).to_vec();
            let flags = InferenceFlags {
                generic_metadata_is_enabled: rv.generic_metadata_is_enabled(),
                inference_using_bounds_is_enabled: rv.inference_using_bounds_is_enabled(),
                strict_inference: rv.unit.options.strict_inference,
            };
            let error_entity = self.inference_error_entity(rv);
            let is_const = self.is_const(rv);
            let mut i = rv.type_system.setup_generic_type_inference(
                &fresh_type_parameters,
                f.ret,
                self.context_type,
                Some(&mut reporter),
                Some(error_entity),
                flags,
                is_const,
                rv.flow_analysis.type_operations,
                None,
                Some(self.node()),
            );
            let preliminary = i.choose_preliminary_types();
            substitution = Some(MapSubstitution::from_pairs(
                &fresh_type_parameters,
                &preliminary,
            ));
            inferrer = Some(i);
            flush_inferrer_diagnostics(rv, &buffer);
        }

        let mut identical_argument_info: Option<Vec<Option<IdenticalArgumentInfo<'_>>>> =
            self.is_identical(rv).then(Vec::new);
        let parameter_map = compute_parameter_map(rv, &function_type_parameters(rv, raw_type));
        let deferred_function_literals = visit_arguments(
            rv,
            self,
            &parameter_map,
            identical_argument_info.as_mut(),
            substitution.as_ref(),
            inferrer.as_mut(),
            &buffer,
        );
        if let Some(deferred_function_literals) = deferred_function_literals {
            let type_variables = raw_type
                .map(|t| function_type_type_parameters(rv, t))
                .unwrap_or_default();
            let undeferred = compute_undeferred_param_info(
                raw_type.is_some(),
                &parameter_map,
                &deferred_function_literals,
            );
            let stages = FunctionLiteralDependencies::new(
                rv,
                &deferred_function_literals,
                &type_variables,
                &undeferred,
            )
            .plan_reconciliation_stages();
            let mut is_first_stage = true;
            for stage in stages {
                if let Some(i) = inferrer.as_mut()
                    && !is_first_stage
                {
                    let preliminary = i.choose_preliminary_types();
                    flush_inferrer_diagnostics(rv, &buffer);
                    substitution = Some(MapSubstitution::from_pairs(&type_variables, &preliminary));
                }
                let stage: Vec<DeferredParamInfo> = stage
                    .into_iter()
                    .map(|i| deferred_function_literals[i].clone())
                    .collect();
                resolve_deferred_function_literals(
                    rv,
                    self,
                    &stage,
                    identical_argument_info.as_mut(),
                    substitution.as_ref(),
                    inferrer.as_mut(),
                    &buffer,
                );
                is_first_stage = false;
            }
        }

        if let Some(mut i) = inferrer {
            type_argument_types = Some(i.choose_final_types());
            drop(i);
            flush_inferrer_diagnostics(rv, &buffer);
        }
        let invoke_type = match (&type_argument_types, original_type) {
            (Some(types), Some(original)) => {
                if function_type_type_parameters(rv, original).len() == types.len() {
                    Some(ctx.instantiate_function_type(original, types))
                } else {
                    Some(original)
                }
            }
            (None, original) => original,
            (Some(_), None) => None,
        };

        let parameters = self.store_result(rv, type_argument_types.as_deref(), invoke_type);
        if let Some(parameters) = parameters {
            let corresponding =
                resolve_arguments_to_parameters(rv, self.argument_list, &parameters, true);
            record_corresponding_parameters(rv, self.argument_list, &corresponding);
        }
        let return_type = InvocationInferrer::compute_invoke_return_type(rv, invoke_type);
        let return_type = self.refine_return_type(rv, return_type);
        record_identical_argument_info(rv, self.argument_list, identical_argument_info);
        return_type
    }

    /// Dart `InvocationInferrer.computeInvokeReturnType(type)`.
    pub fn compute_invoke_return_type(rv: &ResolverVisitor<'_>, ty: Option<TypeId>) -> TypeId {
        match ty.map(|t| *rv.ctx.ty(t)) {
            Some(TypeKind::Function(f)) => f.ret,
            _ => TypeId::DYNAMIC,
        }
    }
}

/// Dart `TopLevelFunctionElement.isDartCoreIdentical`.
fn is_dart_core_identical(rv: &ResolverVisitor<'_>, e: ElementId) -> bool {
    let ctx = rv.ctx;
    ctx.element_name(e) == Some("identical")
        && ctx
            .element_data(e)
            .and_then(|d| d.library)
            .is_some_and(|l| ctx.tp.core_library.try_get() == Some(&l))
}

/// Dart `_computeUndeferredParamInfo(rawType, parameterMap,
/// deferredFunctionLiterals)`.
fn compute_undeferred_param_info(
    has_raw_type: bool,
    parameter_map: &IndexMap<ParamKey, FnParam>,
    deferred_function_literals: &[DeferredParamInfo],
) -> Vec<Option<FnParam>> {
    if !has_raw_type {
        return Vec::new();
    }
    let covered: IndexSet<&ParamKey> = deferred_function_literals
        .iter()
        .map(|d| &d.parameter_key)
        .collect();
    parameter_map
        .iter()
        .filter(|(k, _)| !covered.contains(k))
        .map(|(_, p)| Some(*p))
        .collect()
}

/// The context type of an argument for [parameter] (Dart: the parameter
/// type with [substitution], then `_computeContextForArgument`).
fn parameter_context_type(
    rv: &ResolverVisitor<'_>,
    inf: &InvocationInferrer,
    parameter: Option<&FnParam>,
    substitution: Option<&MapSubstitution>,
) -> TypeId {
    match parameter {
        Some(p) => {
            let parameter_type = match substitution {
                Some(s) => s.substitute_type(&rv.ctx, p.ty),
                None => p.ty,
            };
            inf.compute_context_for_argument(rv, parameter_type)
        }
        None => TypeId::UNKNOWN,
    }
}

/// Dart `_visitArguments(...)`: resolves each argument that is not a
/// deferred function literal; returns the deferred function literals.
fn visit_arguments<'a>(
    rv: &mut ResolverVisitor<'a>,
    inf: &InvocationInferrer,
    parameter_map: &IndexMap<ParamKey, FnParam>,
    mut identical_argument_info: Option<&mut Vec<Option<IdenticalArgumentInfo<'a>>>>,
    substitution: Option<&MapSubstitution>,
    mut inferrer: Option<&mut GenericInferrer<'_, '_, '_>>,
    buffer: &DiagnosticBuffer,
) -> Option<Vec<DeferredParamInfo>> {
    let mut deferred_function_literals: Option<Vec<DeferredParamInfo>> = None;
    rv.check_unreachable_node(inf.argument_list);
    let flow_active = rv.flow_analysis.flow.is_some();
    let inference_update_1 = rv.is_enabled(ExperimentalFlag::InferenceUpdate1);
    let mut unnamed_argument_index = 0;
    let arguments = arguments_of(rv, inf.argument_list);
    for (i, argument) in arguments.into_iter().enumerate() {
        let parameter_key = match argument.name {
            Some(name) => ParamKey::Named(name),
            None => {
                let k = ParamKey::Positional(unnamed_argument_index);
                unnamed_argument_index += 1;
                k
            }
        };
        let expression = argument.expression;
        let value = ast_ext::un_parenthesized(rv.ast, expression);
        let parameter = parameter_map.get(&parameter_key).copied();
        if inference_update_1 && let Some(function) = rv.ast.cast::<FunctionExpression>(value) {
            deferred_function_literals
                .get_or_insert_with(Vec::new)
                .push(DeferredParamInfo {
                    parameter,
                    value: function,
                    index: i,
                    parameter_key,
                });
            if let Some(info) = identical_argument_info.as_deref_mut() {
                info.push(None);
            }
        } else {
            let context_type = parameter_context_type(rv, inf, parameter.as_ref(), substitution);
            rv.analyze_expression_node(expression, context_type);
            let rewritten = rv.pop_rewrite().expect("rewritten argument");
            let rewritten_type = rv.static_type(rewritten).unwrap_or(TypeId::DYNAMIC);
            if flow_active && let Some(info) = identical_argument_info.as_deref_mut() {
                info.push(Some(IdenticalArgumentInfo {
                    expression_info: rv.flow_analysis.get_expression_info(Some(rewritten)),
                    static_type: rewritten_type,
                }));
            }
            if let (Some(p), Some(inferrer)) = (&parameter, inferrer.as_deref_mut()) {
                let name = param_name(rv, p);
                inferrer.constrain_argument(rewritten_type, p.ty, &name, Some(inf.node()));
                flush_inferrer_diagnostics(rv, buffer);
            }
        }
    }
    deferred_function_literals
}

/// Dart `_resolveDeferredFunctionLiterals(...)`.
fn resolve_deferred_function_literals<'a>(
    rv: &mut ResolverVisitor<'a>,
    inf: &InvocationInferrer,
    deferred_function_literals: &[DeferredParamInfo],
    mut identical_argument_info: Option<&mut Vec<Option<IdenticalArgumentInfo<'a>>>>,
    substitution: Option<&MapSubstitution>,
    mut inferrer: Option<&mut GenericInferrer<'_, '_, '_>>,
    buffer: &DiagnosticBuffer,
) {
    let flow_active = rv.flow_analysis.flow.is_some();
    for deferred_argument in deferred_function_literals {
        let context_type =
            parameter_context_type(rv, inf, deferred_argument.parameter.as_ref(), substitution);
        let arguments = arguments_of(rv, inf.argument_list);
        let Some(argument) = arguments.get(deferred_argument.index) else {
            continue;
        };
        rv.analyze_expression_node(argument.expression, context_type);
        let expression = rv.pop_rewrite().expect("rewritten argument");
        let expression_type = rv.static_type(expression).unwrap_or(TypeId::DYNAMIC);
        if flow_active
            && let Some(info) = identical_argument_info.as_deref_mut()
            && let Some(slot) = info.get_mut(deferred_argument.index)
        {
            *slot = Some(IdenticalArgumentInfo {
                expression_info: rv.flow_analysis.get_expression_info(Some(expression)),
                static_type: expression_type,
            });
        }
        if let (Some(p), Some(inferrer)) = (&deferred_argument.parameter, inferrer.as_deref_mut()) {
            let name = param_name(rv, p);
            inferrer.constrain_argument(expression_type, p.ty, &name, Some(inf.node()));
            flush_inferrer_diagnostics(rv, buffer);
        }
    }
}

/// Dart `_recordIdenticalArgumentInfo(identicalArgumentInfo)`.
fn record_identical_argument_info<'a>(
    rv: &mut ResolverVisitor<'a>,
    argument_list: Id<ArgumentList>,
    identical_argument_info: Option<Vec<Option<IdenticalArgumentInfo<'a>>>>,
) {
    let Some(info) = identical_argument_info else {
        return;
    };
    let (Some(Some(left)), Some(Some(right))) = (info.first().cloned(), info.get(1).cloned())
    else {
        return;
    };
    let Some(parent) = rv.ast.parent(argument_list) else {
        return;
    };
    let result = rv.flow_analysis.flow.as_mut().and_then(|flow| {
        flow.equality_operation_end(
            left.expression_info,
            SharedTypeView::new(left.static_type),
            right.expression_info,
            SharedTypeView::new(right.static_type),
            false,
        )
    });
    rv.flow_analysis
        .store_expression_info(Id::from_raw(parent), result);
}

/// Dart `InvocationInferrer.resolveInvocation()` (the base class, used
/// directly for `ExtensionOverride`, `RedirectingConstructorInvocation` and
/// `SuperConstructorInvocation`): resolves the arguments of
/// [argument_list] with the parameters of [target] as context.
pub fn resolve_invocation_base(
    rv: &mut ResolverVisitor<'_>,
    node: NodeId,
    argument_list: Id<ArgumentList>,
    target: Option<InvocationTarget>,
) {
    let raw_type = target.map(|t| t.raw_type(rv));
    let parameter_map = compute_parameter_map(rv, &function_type_parameters(rv, raw_type));
    let inf = InvocationInferrer {
        kind: InferrerKind::Base(node),
        argument_list,
        context_type: TypeId::UNKNOWN,
        target,
    };
    let buffer: DiagnosticBuffer = RefCell::new(Vec::new());
    let deferred = visit_arguments(rv, &inf, &parameter_map, None, None, None, &buffer);
    if let Some(deferred) = deferred {
        resolve_deferred_function_literals(rv, &inf, &deferred, None, None, None, &buffer);
    }
}

// ------------------------------------------------------------------ arguments → parameters

/// Records `ArgumentList.correspondingStaticParameters` in
/// `ResolutionTables.param_element`, keyed by the argument expression (the
/// expression of a named argument; Dart `Expression.correspondingParameter`).
pub fn record_corresponding_parameters(
    rv: &mut ResolverVisitor<'_>,
    argument_list: Id<ArgumentList>,
    parameters: &[Option<FnParam>],
) {
    let arguments = arguments_of(rv, argument_list);
    for (argument, parameter) in arguments.into_iter().zip(parameters) {
        match parameter.and_then(|p| p.element) {
            Some(e) => {
                rv.tables.param_element.insert(argument.expression, e);
            }
            None => {
                rv.tables.param_element.remove(argument.expression);
            }
        }
        // Dart `correspondingParameter` is also set for the parameters of a
        // synthesized function type, which have no declaring element here.
        match parameter {
            Some(p) => {
                rv.tables.param_type.insert(argument.expression, p.ty);
            }
            None => {
                rv.tables.param_type.remove(argument.expression);
            }
        }
    }
}

/// Dart `ResolverVisitor.resolveArgumentsToParameters(argumentList:,
/// formalParameters:, diagnosticReporter:)`: the parameter of each
/// argument (`None` where no parameter matches). Reports the argument count
/// and name diagnostics when [report] is set (Dart: a non-null
/// `diagnosticReporter`).
///
/// The `enclosingConstructorFormalParameterList` of super constructor
/// invocations (`verifySuperFormalParameters`) is not supported here.
pub fn resolve_arguments_to_parameters(
    rv: &mut ResolverVisitor<'_>,
    argument_list: Id<ArgumentList>,
    formal_parameters: &[FnParam],
    report: bool,
) -> Vec<Option<FnParam>> {
    let ctx = rv.ctx;
    let mut required_parameter_count = 0;
    let mut unnamed_parameter_count = 0;
    let mut unnamed_parameters: Vec<FnParam> = Vec::new();
    let mut named_parameters: Option<IndexMap<String, FnParam>> = None;
    for &parameter in formal_parameters {
        if parameter.kind.is_required_positional() {
            unnamed_parameters.push(parameter);
            unnamed_parameter_count += 1;
            required_parameter_count += 1;
        } else if parameter.kind.is_optional_positional() {
            unnamed_parameters.push(parameter);
            unnamed_parameter_count += 1;
        } else {
            let name = parameter
                .name
                .map(|n| ctx.name_str(n))
                .unwrap_or("")
                .to_string();
            named_parameters
                .get_or_insert_with(IndexMap::new)
                .insert(name, parameter);
        }
    }
    let mut unnamed_index = 0;
    let arguments = arguments_of(rv, argument_list);
    let argument_count = arguments.len();
    let mut resolved_parameters: Vec<Option<FnParam>> = vec![None; argument_count];
    let mut positional_argument_count = 0;
    let mut no_blank_arguments = true;
    let mut first_unresolved_argument: Option<Id<Expression>> = None;
    let mut last_positional_argument: Option<Id<Expression>> = None;
    for (i, argument) in arguments.iter().enumerate() {
        if argument.name.is_some() {
            continue;
        }
        let expression = argument.expression;
        if let Some(s) = rv.ast.cast::<SimpleIdentifier>(expression)
            && rv.lexeme(rv.ast[s].token).is_empty()
        {
            no_blank_arguments = false;
        }
        positional_argument_count += 1;
        if unnamed_index < unnamed_parameter_count {
            resolved_parameters[i] = Some(unnamed_parameters[unnamed_index]);
            unnamed_index += 1;
        } else if first_unresolved_argument.is_none() {
            first_unresolved_argument = Some(expression);
        }
        last_positional_argument = Some(expression);
    }

    let mut used_names: Option<IndexSet<String>> = None;
    for (i, argument) in arguments.iter().enumerate() {
        let Some(name) = &argument.name else {
            continue;
        };
        let Some(named_argument) = rv.ast.cast::<NamedArgument>(argument.node) else {
            continue;
        };
        let name_token = rv.ast[named_argument].name;
        let mut element = named_parameters.as_ref().and_then(|m| m.get(name).copied());
        if element.is_none() {
            element = if name.starts_with('_') && name.len() > 1 {
                named_parameters
                    .as_ref()
                    .and_then(|m| m.get(&name[1..]).copied())
            } else {
                None
            };
            if report {
                let d = if element.is_none() {
                    diag::undefined_named_parameter(name)
                } else {
                    diag::use_of_private_parameter_name(&name[1..])
                };
                let d = rv.at_token(d, name_token);
                rv.report(d);
            }
        } else {
            resolved_parameters[i] = element;
        }
        if !used_names
            .get_or_insert_with(IndexSet::new)
            .insert(name.clone())
            && report
        {
            let d = rv.at_token(diag::duplicate_named_argument(name), name_token);
            rv.report(d);
        }
    }

    if positional_argument_count < required_parameter_count && no_blank_arguments {
        if report && let Some(parent) = rv.ast.parent(argument_list) {
            // Dart `lastPositionalArgument?.endToken.next ??
            // argumentList.leftParenthesis.next ?? rightParenthesis`.
            let token = match last_positional_argument {
                Some(e) => rv.ast.tokens.next(rv.ast.end_token(e.raw())),
                None => rv.ast.tokens.next(rv.ast[argument_list].left_parenthesis),
            };
            report_not_enough_positional_arguments(
                rv,
                token,
                required_parameter_count,
                positional_argument_count,
                parent,
            );
        }
    } else if positional_argument_count > unnamed_parameter_count && no_blank_arguments {
        let named_parameter_count = named_parameters.as_ref().map_or(0, |m| m.len());
        let named_argument_count = used_names.as_ref().map_or(0, |m| m.len());
        if let Some(first) = first_unresolved_argument
            && report
        {
            let d = if named_parameter_count > named_argument_count {
                diag::extra_positional_arguments_could_be_named(
                    unnamed_parameter_count as i64,
                    positional_argument_count as i64,
                )
            } else {
                diag::extra_positional_arguments(
                    unnamed_parameter_count as i64,
                    positional_argument_count as i64,
                )
            };
            let d = rv.at(d, first);
            rv.report(d);
        }
    }
    resolved_parameters
}

/// Dart `ResolverVisitor._reportNotEnoughPositionalArguments(...)`.
fn report_not_enough_positional_arguments(
    rv: &mut ResolverVisitor<'_>,
    token: dartr_syntax::TokenId,
    required_parameter_count: usize,
    actual_argument_count: usize,
    name_node: NodeId,
) {
    let ctx = rv.ctx;
    let ast = &*rv.ast;
    let display = |t: TypeId| dartr_element::diagnostics::type_display_string(&ctx, t, true);
    let lexeme = |s: Id<SimpleIdentifier>| rv.lexeme(ast[s].token).to_string();
    let name: Option<String> = if let Some(n) = ast.cast::<InstanceCreationExpression>(name_node) {
        let constructor_name = ast[n].constructor_name;
        match ast[constructor_name].name {
            Some(name) => Some(lexeme(name)),
            None => {
                let named_type = ast[constructor_name].type_;
                Some(format!("{}.new", rv.lexeme(ast[named_type].name)))
            }
        }
    } else if let Some(n) = ast.cast::<RedirectingConstructorInvocation>(name_node) {
        match ast[n].constructor_name {
            Some(name) => Some(lexeme(name)),
            None => rv
                .element(n)
                .map(|e| format!("{}.new", display(member::return_type(&ctx, e)))),
        }
    } else if let Some(n) = ast.cast::<SuperConstructorInvocation>(name_node) {
        match ast[n].constructor_name {
            Some(name) => Some(lexeme(name)),
            None => rv
                .element(n)
                .map(|e| format!("{}.new", display(member::return_type(&ctx, e)))),
        }
    } else if let Some(n) = ast.cast::<MethodInvocation>(name_node) {
        Some(lexeme(ast[n].method_name))
    } else if let Some(n) = ast.cast::<FunctionExpressionInvocation>(name_node) {
        ast.cast::<SimpleIdentifier>(ast[n].function).map(lexeme)
    } else if ast.is::<EnumConstantArguments>(name_node) {
        ast.parent(name_node)
            .and_then(|p| ast.cast::<EnumConstantDeclaration>(p))
            .and_then(|p| enum_constant_type_display(rv, p))
    } else if let Some(n) = ast.cast::<EnumConstantDeclaration>(name_node) {
        enum_constant_type_display(rv, n)
    } else if let Some(n) = ast.cast::<Annotation>(name_node) {
        let name = ast[n].name;
        match ast.cast::<PrefixedIdentifier>(name) {
            Some(p) => Some(lexeme(ast[p].identifier)),
            None => Some(format!("{}.new", lexeme(ast.simple_name(name)))),
        }
    } else if let Some(n) = ast.cast::<DotShorthandConstructorInvocation>(name_node) {
        Some(lexeme(ast[n].constructor_name))
    } else {
        // Dart throws `UnimplementedError` for other nodes.
        ast.cast::<DotShorthandInvocation>(name_node)
            .map(|n| lexeme(ast[n].member_name))
    };
    let is_plural = required_parameter_count > 1;
    let d = match name {
        None if is_plural => diag::not_enough_positional_arguments_plural(
            required_parameter_count as i64,
            actual_argument_count as i64,
        ),
        None => diag::not_enough_positional_arguments_singular(),
        Some(name) if is_plural => diag::not_enough_positional_arguments_name_plural(
            required_parameter_count as i64,
            actual_argument_count as i64,
            &name,
        ),
        Some(name) => diag::not_enough_positional_arguments_name_singular(&name),
    };
    let d = rv.at_token(d, token);
    rv.report(d);
}

/// Dart `declaredFragment!.element.type.getDisplayString()` of an enum
/// constant.
fn enum_constant_type_display(
    rv: &ResolverVisitor<'_>,
    node: Id<EnumConstantDeclaration>,
) -> Option<String> {
    let ctx = rv.ctx;
    let fragment = *rv.tables.declared_fragment.get(node)?;
    let element = *ctx.fragment_data(fragment)?.element.try_get()?;
    let ty = crate::element_ext::variable_type(&ctx, element);
    Some(dartr_element::diagnostics::type_display_string(
        &ctx, ty, true,
    ))
}

// ------------------------------------------------------------------ function literal dependencies

/// Dart `_FunctionLiteralDependencies` (with the shared
/// `FunctionLiteralDependencies` and `DependencyWalker`): plans the stages
/// in which the deferred function literals are resolved.
struct FunctionLiteralDependencies {
    nodes: Vec<DepNode>,
}

/// Dart `_Node`.
struct DepNode {
    /// The index of the parameter in the deferred list; `None` for an
    /// un-deferred parameter.
    deferred_param_index: Option<usize>,
    dependencies: Vec<usize>,
    stage_num: Option<usize>,
    /// Dart `Node._index`.
    index: usize,
    /// Dart `Node._lowLink`.
    low_link: usize,
}

impl DepNode {
    fn new(deferred_param_index: Option<usize>) -> DepNode {
        DepNode {
            deferred_param_index,
            dependencies: Vec::new(),
            stage_num: None,
            index: 0,
            low_link: 0,
        }
    }
}

impl FunctionLiteralDependencies {
    fn new(
        rv: &ResolverVisitor<'_>,
        deferred_params: &[DeferredParamInfo],
        type_variables: &[EId<TypeParameterElement>],
        undeferred_params: &[Option<FnParam>],
    ) -> FunctionLiteralDependencies {
        let mut nodes = Vec::new();
        let mut params_depending_on_type_var: IndexMap<EId<TypeParameterElement>, IndexSet<usize>> =
            IndexMap::new();
        let mut params_constraining_type_var: IndexMap<EId<TypeParameterElement>, IndexSet<usize>> =
            IndexMap::new();
        for (deferred_param_index, param) in deferred_params.iter().enumerate() {
            let node = nodes.len();
            nodes.push(DepNode::new(Some(deferred_param_index)));
            for v in type_vars_free_in_param_params(rv, param, type_variables) {
                params_depending_on_type_var
                    .entry(v)
                    .or_default()
                    .insert(node);
            }
            for v in type_vars_free_in_param_returns(rv, param.parameter.as_ref(), type_variables) {
                params_constraining_type_var
                    .entry(v)
                    .or_default()
                    .insert(node);
            }
        }
        for param in undeferred_params {
            let node = nodes.len();
            nodes.push(DepNode::new(None));
            // For un-deferred parameters only the free type variables of the
            // returns matter: they are already analyzed.
            for v in type_vars_free_in_param_returns(rv, param.as_ref(), type_variables) {
                params_constraining_type_var
                    .entry(v)
                    .or_default()
                    .insert(node);
            }
        }
        for type_variable in type_variables {
            let Some(depending) = params_depending_on_type_var.get(type_variable) else {
                continue;
            };
            let constraining: Vec<usize> = params_constraining_type_var
                .get(type_variable)
                .map(|s| s.iter().copied().collect())
                .unwrap_or_default();
            for &node in depending {
                nodes[node]
                    .dependencies
                    .extend(constraining.iter().copied());
            }
        }
        FunctionLiteralDependencies { nodes }
    }

    /// Dart `planReconciliationStages()`: the stages, each a list of indices
    /// into the deferred parameters.
    fn plan_reconciliation_stages(mut self) -> Vec<Vec<usize>> {
        let mut stages: Vec<Vec<usize>> = Vec::new();
        for start in 0..self.nodes.len() {
            self.walk(start, &mut stages);
        }
        stages
            .into_iter()
            .map(|mut stage| {
                stage.sort_by_key(|&n| self.nodes[n].deferred_param_index);
                stage
                    .into_iter()
                    .filter_map(|n| self.nodes[n].deferred_param_index)
                    .collect()
            })
            .collect()
    }

    /// Dart `DependencyWalker.walk(startingPoint)` (Tarjan's algorithm).
    fn walk(&mut self, start: usize, stages: &mut Vec<Vec<usize>>) {
        if self.nodes[start].stage_num.is_some() {
            return;
        }
        let mut index = 1;
        let mut stack: Vec<usize> = Vec::new();
        self.strong_connect(start, &mut index, &mut stack, stages);
    }

    /// Dart `strongConnect(node)`.
    fn strong_connect(
        &mut self,
        node: usize,
        index: &mut usize,
        stack: &mut Vec<usize>,
        stages: &mut Vec<Vec<usize>>,
    ) {
        self.nodes[node].index = *index;
        self.nodes[node].low_link = *index;
        *index += 1;
        stack.push(node);
        let dependencies = self.nodes[node].dependencies.clone();
        for dependency in dependencies {
            if self.nodes[dependency].stage_num.is_some() || dependency == node {
                // Evaluated, or a trivial cycle (`evaluate(v)` is
                // `evaluateScc([v])` here, so the cycle does not matter).
                continue;
            }
            if self.nodes[dependency].index == 0 {
                self.strong_connect(dependency, index, stack, stages);
                if self.nodes[dependency].low_link < self.nodes[node].low_link {
                    self.nodes[node].low_link = self.nodes[dependency].low_link;
                }
            } else if self.nodes[dependency].index < self.nodes[node].low_link {
                self.nodes[node].low_link = self.nodes[dependency].index;
            }
        }
        if self.nodes[node].low_link == self.nodes[node].index {
            let mut scc = Vec::new();
            loop {
                let other = stack.pop().expect("dependency walker stack");
                scc.push(other);
                if other == node {
                    break;
                }
            }
            self.evaluate_scc(&scc, stages);
        }
    }

    /// Dart `_DependencyWalker.evaluateScc(nodes)`.
    fn evaluate_scc(&mut self, scc: &[usize], stages: &mut Vec<Vec<usize>>) {
        let mut stage_num = 0;
        for &node in scc {
            for &dependency in &self.nodes[node].dependencies {
                if let Some(dependency_stage_num) = self.nodes[dependency].stage_num
                    && dependency_stage_num >= stage_num
                {
                    stage_num = dependency_stage_num + 1;
                }
            }
        }
        if stages.len() <= stage_num {
            stages.push(Vec::new());
        }
        for &node in scc {
            self.nodes[node].stage_num = Some(stage_num);
            if self.nodes[node].deferred_param_index.is_some() {
                stages[stage_num].push(node);
            }
        }
    }
}

/// Dart `_FunctionLiteralDependencies.typeVarsFreeInParamParams(paramInfo)`.
fn type_vars_free_in_param_params(
    rv: &ResolverVisitor<'_>,
    param_info: &DeferredParamInfo,
    type_variables: &[EId<TypeParameterElement>],
) -> Vec<EId<TypeParameterElement>> {
    let Some(parameter) = &param_info.parameter else {
        return Vec::new();
    };
    let TypeKind::Function(f) = *rv.ctx.ty(parameter.ty) else {
        return Vec::new();
    };
    let parameter_map = compute_parameter_map(rv, rv.ctx.list(f.params));
    let explicitly_typed_parameters = compute_explicitly_typed_parameter_set(rv, param_info.value);
    let mut result: IndexSet<EId<TypeParameterElement>> = IndexSet::new();
    for (key, p) in &parameter_map {
        if explicitly_typed_parameters.contains(key) {
            continue;
        }
        if let Some(free) = rv
            .type_system
            .get_free_parameters(p.ty, Some(type_variables))
        {
            result.extend(free);
        }
    }
    result.into_iter().collect()
}

/// Dart `_FunctionLiteralDependencies.typeVarsFreeInParamReturns(paramInfo)`.
fn type_vars_free_in_param_returns(
    rv: &ResolverVisitor<'_>,
    parameter: Option<&FnParam>,
    type_variables: &[EId<TypeParameterElement>],
) -> Vec<EId<TypeParameterElement>> {
    let Some(parameter) = parameter else {
        return Vec::new();
    };
    let ty = match *rv.ctx.ty(parameter.ty) {
        TypeKind::Function(f) => f.ret,
        _ => parameter.ty,
    };
    rv.type_system
        .get_free_parameters(ty, Some(type_variables))
        .unwrap_or_default()
}
