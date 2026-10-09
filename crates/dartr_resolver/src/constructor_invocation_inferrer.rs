// Dart source: pkg/analyzer/lib/src/dart/resolver/invocation_inferrer.dart (InvocationInferrer, FullInvocationInferrer: the parts that constructor invocations use) and pkg/analyzer/lib/src/generated/resolver.dart (ResolverVisitor.resolveArgumentsToParameters)

//! The invocation inference of constructor invocations (unit C8):
//! instance creations, dot shorthand constructor invocations, `super(...)`
//! and `this(...)` constructor invocations, and enum constant arguments.
//!
//! TEMPORARY: `invocation_inferrer.rs` (unit C3) is the full port of
//! `invocation_inferrer.dart`. Until it lands, this module has the subset
//! that the constructor resolvers of C8 need. Differences to Dart:
//!
//! - The deferred function literals (`inference-update-1`) are resolved in
//!   one stage, after the other arguments (Dart plans the stages with
//!   `FunctionLiteralDependencies`).
//! - `whyNotPromotedArguments` are not collected
//!   (`checkForArgumentTypesNotAssignableInList` is wave D).
//!
//! When C3 lands, the constructor resolvers call its inferrer and this
//! module goes away.

use dartr_ast::{
    Argument, ArgumentList, Expression, FunctionExpression, Id, NamedArgument, NodeId,
    TypeArgumentList,
};
use dartr_diagnostics::{LocatableDiagnostic, diag};
use dartr_element::diagnostics::type_arg;
use dartr_element::{ElemRef, FnParam, FormalParameterElement, TypeId, TypeKind};
use dartr_typesystem::generic_inferrer::{InferenceErrorEntity, InferenceFlags};
use dartr_typesystem::type_algebra::{MapSubstitution, get_fresh_type_parameters};
use dartr_typesystem::{TypeExt, member};
use indexmap::IndexMap;

use crate::ast_ext::un_parenthesized;
use crate::resolver::ResolverVisitor;

/// The key of a parameter in Dart `_computeParameterMap`: the name of a
/// named parameter, or the index of a positional parameter.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum ParamKey {
    Index(usize),
    Name(String),
}

/// Dart `_computeParameterMap(parameters)`.
fn compute_parameter_map(
    rv: &ResolverVisitor<'_>,
    parameters: &[FnParam],
) -> IndexMap<ParamKey, FnParam> {
    let mut unnamed_parameter_index = 0;
    let mut map = IndexMap::new();
    for &p in parameters {
        let key = if p.kind.is_named() {
            ParamKey::Name(
                p.name
                    .map(|n| rv.ctx.name_str(n).to_string())
                    .unwrap_or_default(),
            )
        } else {
            let k = ParamKey::Index(unnamed_parameter_index);
            unnamed_parameter_index += 1;
            k
        };
        map.insert(key, p);
    }
    map
}

/// Dart `_DeferredParamInfo`.
struct DeferredParamInfo {
    parameter: Option<FnParam>,
    index: usize,
}

/// The parameters of `FullInvocationInferrer` that its subclasses define
/// (`_errorEntity`, `_isConst`, `_isGenericInferenceDisabled`,
/// `_needsTypeArgumentBoundsCheck`, `_typeArguments`,
/// `_reportWrongNumberOfTypeArguments`).
pub struct FullInvocation {
    /// Dart `node`.
    pub node: NodeId,
    pub argument_list: Id<ArgumentList>,
    pub context_type: TypeId,
    /// Dart `target?.rawType`.
    pub raw_type: Option<TypeId>,
    /// Dart `_typeArguments`.
    pub type_arguments: Option<Id<TypeArgumentList>>,
    /// Dart `_errorEntity`.
    pub error_entity: InferenceErrorEntity,
    /// Dart `_isConst`.
    pub is_const: bool,
    /// Dart `_isGenericInferenceDisabled`.
    pub is_generic_inference_disabled: bool,
    /// Dart `_needsTypeArgumentBoundsCheck`.
    pub needs_type_argument_bounds_check: bool,
    /// Dart `_reportWrongNumberOfTypeArguments`: the diagnostic for
    /// (type parameter count, type argument count), or `None` if the
    /// subclass does not report it.
    pub wrong_number_of_type_arguments: Option<Box<dyn Fn(usize, usize) -> LocatableDiagnostic>>,
}

/// Dart `FullInvocationInferrer.resolveInvocation()`. [store_result] is
/// Dart `_storeResult(typeArgumentTypes, invokeType)`: it returns the formal
/// parameters to resolve the arguments to. Returns the return type.
pub fn resolve_full_invocation(
    rv: &mut ResolverVisitor<'_>,
    inv: FullInvocation,
    store_result: &mut dyn FnMut(
        &mut ResolverVisitor<'_>,
        Option<&[TypeId]>,
        Option<TypeId>,
    ) -> Option<Vec<ElemRef>>,
) -> TypeId {
    let ctx = rv.ctx;
    let mut raw_type = inv.raw_type;
    let original_type = raw_type;
    let raw_type_params = |t: Option<TypeId>| match t.map(|t| *ctx.ty(t)) {
        Some(TypeKind::Function(f)) => ctx.list(f.type_params).to_vec(),
        _ => Vec::new(),
    };

    let mut type_argument_types: Option<Vec<TypeId>> = None;
    let mut substitution: Option<MapSubstitution> = None;
    let mut inferrer_needed = false;
    if inv.is_generic_inference_disabled {
        let type_parameters = raw_type_params(raw_type);
        if raw_type.is_some() && !type_parameters.is_empty() {
            let types = vec![TypeId::DYNAMIC; type_parameters.len()];
            substitution = Some(MapSubstitution::from_pairs(&type_parameters, &types));
            type_argument_types = Some(types);
        } else {
            type_argument_types = Some(Vec::new());
        }
    } else if let Some(type_argument_list) = inv.type_arguments {
        let arguments = rv.ast.list(rv.ast[type_argument_list].arguments).to_vec();
        let type_parameters = raw_type_params(raw_type);
        if raw_type.is_some() && arguments.len() != type_parameters.len() {
            if let Some(report) = &inv.wrong_number_of_type_arguments {
                let d = report(type_parameters.len(), arguments.len());
                let d = rv.at(d, type_argument_list);
                rv.report(d);
            }
            type_argument_types = Some(vec![TypeId::DYNAMIC; type_parameters.len()]);
        } else {
            let types: Vec<TypeId> = arguments
                .iter()
                .map(|&a| {
                    rv.tables
                        .annotation_type
                        .get(a)
                        .copied()
                        .unwrap_or(TypeId::DYNAMIC)
                })
                .collect();
            if raw_type.is_some() && inv.needs_type_argument_bounds_check {
                let substitution = MapSubstitution::from_pairs(&type_parameters, &types);
                for (i, &type_parameter) in type_parameters.iter().enumerate() {
                    if let Some(bound) = ctx.type_parameter_bound(type_parameter) {
                        let bound = substitution.substitute_type(&ctx, bound);
                        let type_argument = types[i];
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
                            let d = rv.at(d, arguments[i]);
                            rv.report(d);
                        }
                    }
                }
            }
            type_argument_types = Some(types);
        }
        if raw_type.is_some() {
            let types = type_argument_types.as_deref().unwrap_or(&[]);
            if types.len() == type_parameters.len() {
                substitution = Some(MapSubstitution::from_pairs(&type_parameters, types));
            }
        }
    } else if raw_type.is_none() || raw_type_params(raw_type).is_empty() {
        type_argument_types = Some(Vec::new());
    } else {
        let type_parameters = raw_type_params(raw_type);
        let fresh = get_fresh_type_parameters(&ctx, &type_parameters);
        raw_type = Some(fresh.apply_to_function_type(&ctx, raw_type.unwrap()));
        inferrer_needed = true;
    }

    let fresh_type_parameters = raw_type_params(raw_type);
    let (fn_params, declared_return_type) = match raw_type.map(|t| *ctx.ty(t)) {
        Some(TypeKind::Function(f)) => (ctx.list(f.params).to_vec(), f.ret),
        _ => (Vec::new(), TypeId::DYNAMIC),
    };

    let mut reported = Vec::new();
    let flags = InferenceFlags {
        generic_metadata_is_enabled: rv.generic_metadata_is_enabled(),
        inference_using_bounds_is_enabled: rv.inference_using_bounds_is_enabled(),
        strict_inference: rv.unit.options.strict_inference,
    };
    let type_system = rv.type_system;
    let operations = rv.flow_analysis.type_operations;
    let mut listener = |d: dartr_diagnostics::Diagnostic| reported.push(d);
    let mut reporter = dartr_diagnostics::DiagnosticReporter::new(&mut listener);
    let mut inferrer = if inferrer_needed {
        let mut inferrer = type_system.setup_generic_type_inference(
            &fresh_type_parameters,
            declared_return_type,
            inv.context_type,
            Some(&mut reporter),
            Some(inv.error_entity.clone()),
            flags,
            inv.is_const,
            operations,
            None,
            Some(inv.node),
        );
        let preliminary = inferrer.choose_preliminary_types();
        substitution = Some(MapSubstitution::from_pairs(
            &fresh_type_parameters,
            &preliminary,
        ));
        Some(inferrer)
    } else {
        None
    };

    let parameter_map = compute_parameter_map(rv, &fn_params);
    let deferred = visit_arguments(
        rv,
        inv.argument_list,
        &parameter_map,
        substitution.as_ref(),
        &mut inferrer,
        inv.node,
    );
    if let Some(deferred) = deferred {
        // Dart resolves the deferred function literals in stages
        // (`_FunctionLiteralDependencies.planReconciliationStages`); this
        // shim resolves them in one stage.
        resolve_deferred_function_literals(
            rv,
            inv.argument_list,
            &deferred,
            substitution.as_ref(),
            &mut inferrer,
            inv.node,
        );
    }

    if let Some(inferrer) = inferrer.as_mut() {
        type_argument_types = Some(inferrer.choose_final_types());
    }
    drop(inferrer);
    rv.flush_type_analyzer_errors();
    if rv.lock_level == 0 {
        rv.diagnostics.extend(reported);
    }

    let invoke_type = match (&type_argument_types, original_type) {
        (Some(types), Some(t)) => match *ctx.ty(t) {
            TypeKind::Function(f) if ctx.list(f.type_params).len() == types.len() => {
                Some(ctx.instantiate_function_type(t, types))
            }
            _ => Some(t),
        },
        (None, t) => t,
        (Some(_), None) => None,
    };

    let parameters = store_result(rv, type_argument_types.as_deref(), invoke_type);
    if let Some(parameters) = parameters {
        resolve_arguments_to_parameters(rv, inv.argument_list, &parameters, true, None);
    }
    compute_invoke_return_type(rv, invoke_type)
}

/// Dart `InvocationInferrer.resolveInvocation()` (the base class, used for
/// `super(...)` / `this(...)` constructor invocations): resolves the
/// arguments with the parameter types of [raw_type] as contexts.
pub fn resolve_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: NodeId,
    argument_list: Id<ArgumentList>,
    raw_type: Option<TypeId>,
) {
    let fn_params = match raw_type.map(|t| *rv.ctx.ty(t)) {
        Some(TypeKind::Function(f)) => rv.ctx.list(f.params).to_vec(),
        _ => Vec::new(),
    };
    let parameter_map = compute_parameter_map(rv, &fn_params);
    let mut inferrer = None;
    let deferred = visit_arguments(rv, argument_list, &parameter_map, None, &mut inferrer, node);
    if let Some(deferred) = deferred {
        resolve_deferred_function_literals(rv, argument_list, &deferred, None, &mut inferrer, node);
    }
}

/// Dart `InvocationInferrer.computeInvokeReturnType(type)`.
pub fn compute_invoke_return_type(rv: &ResolverVisitor<'_>, ty: Option<TypeId>) -> TypeId {
    match ty.map(|t| *rv.ctx.ty(t)) {
        Some(TypeKind::Function(f)) => f.ret,
        _ => TypeId::DYNAMIC,
    }
}

/// The expression of [argument] (Dart `argument.argumentExpression`).
fn argument_expression(rv: &ResolverVisitor<'_>, argument: Id<Argument>) -> Id<Expression> {
    match rv.ast.cast::<NamedArgument>(argument) {
        Some(named) => rv.ast[named].argument_expression,
        None => rv
            .ast
            .cast::<Expression>(argument)
            .expect("an expression argument"),
    }
}

type Inferrer<'a, 'r, 'l> = Option<dartr_typesystem::generic_inferrer::GenericInferrer<'a, 'r, 'l>>;

/// Dart `_visitArguments`.
fn visit_arguments(
    rv: &mut ResolverVisitor<'_>,
    argument_list: Id<ArgumentList>,
    parameter_map: &IndexMap<ParamKey, FnParam>,
    substitution: Option<&MapSubstitution>,
    inferrer: &mut Inferrer<'_, '_, '_>,
    node: NodeId,
) -> Option<Vec<DeferredParamInfo>> {
    let mut deferred: Option<Vec<DeferredParamInfo>> = None;
    rv.check_unreachable_node(argument_list);
    let ctx = rv.ctx;
    let inference_update_1 =
        rv.is_enabled(dartr_parser::experimental_flags::ExperimentalFlag::InferenceUpdate1);
    let mut unnamed_argument_index = 0;
    let count = rv.ast.list(rv.ast[argument_list].arguments).len();
    for i in 0..count {
        let argument = rv.ast.list(rv.ast[argument_list].arguments)[i];
        let key = match rv.ast.cast::<NamedArgument>(argument) {
            Some(named) => ParamKey::Name(rv.lexeme(rv.ast[named].name).to_string()),
            None => {
                let k = ParamKey::Index(unnamed_argument_index);
                unnamed_argument_index += 1;
                k
            }
        };
        let value = un_parenthesized(rv.ast, argument_expression(rv, argument));
        let parameter = parameter_map.get(&key).copied();
        if inference_update_1 && rv.ast.is::<FunctionExpression>(value) {
            deferred
                .get_or_insert_with(Vec::new)
                .push(DeferredParamInfo {
                    parameter,
                    index: i,
                });
        } else {
            let parameter_context_type = match parameter {
                Some(p) => substitution.map_or(p.ty, |s| s.substitute_type(&ctx, p.ty)),
                None => TypeId::UNKNOWN,
            };
            let expression = argument_expression(rv, argument);
            let rewritten = rv.resolve_expression(expression, parameter_context_type);
            if let (Some(p), Some(inferrer)) = (parameter, inferrer.as_mut()) {
                let name = p.name.map(|n| ctx.name_str(n)).unwrap_or("");
                inferrer.constrain_argument(rv.type_or_throw(rewritten), p.ty, name, Some(node));
            }
        }
    }
    deferred
}

/// Dart `_resolveDeferredFunctionLiterals`.
fn resolve_deferred_function_literals(
    rv: &mut ResolverVisitor<'_>,
    argument_list: Id<ArgumentList>,
    deferred: &[DeferredParamInfo],
    substitution: Option<&MapSubstitution>,
    inferrer: &mut Inferrer<'_, '_, '_>,
    node: NodeId,
) {
    let ctx = rv.ctx;
    for deferred_argument in deferred {
        let parameter = deferred_argument.parameter;
        let parameter_context_type = match parameter {
            Some(p) => substitution.map_or(p.ty, |s| s.substitute_type(&ctx, p.ty)),
            None => TypeId::UNKNOWN,
        };
        let argument = rv.ast.list(rv.ast[argument_list].arguments)[deferred_argument.index];
        let expression = argument_expression(rv, argument);
        let rewritten = rv.resolve_expression(expression, parameter_context_type);
        if let (Some(p), Some(inferrer)) = (parameter, inferrer.as_mut()) {
            let name = p.name.map(|n| ctx.name_str(n)).unwrap_or("");
            inferrer.constrain_argument(rv.type_or_throw(rewritten), p.ty, name, Some(node));
        }
    }
}

/// Dart `ResolverVisitor.resolveArgumentsToParameters(argumentList:,
/// formalParameters:, diagnosticReporter:,
/// enclosingConstructorFormalParameterList:)`: records the parameter of
/// each argument (Dart `correspondingStaticParameters`, here
/// `ResolutionTables::param_element` keyed by the argument expression) and
/// reports the argument count and name errors when [report] is set.
///
/// The super formal parameters of
/// [enclosing_constructor_formal_parameter_list] (Dart
/// `verifySuperFormalParameters`) count as arguments.
pub fn resolve_arguments_to_parameters(
    rv: &mut ResolverVisitor<'_>,
    argument_list: Id<ArgumentList>,
    formal_parameters: &[ElemRef],
    report: bool,
    enclosing_constructor_formal_parameter_list: Option<Id<dartr_ast::FormalParameterList>>,
) {
    let ctx = rv.ctx;
    let mut required_parameter_count = 0;
    let mut unnamed_parameter_count = 0;
    let mut unnamed_parameters: Vec<ElemRef> = Vec::new();
    let mut named_parameters: Option<IndexMap<String, ElemRef>> = None;
    for &parameter in formal_parameters {
        let base = member::base_element(&ctx, parameter);
        let Some(base) = base.cast::<FormalParameterElement>() else {
            continue;
        };
        let data = ctx.get(base);
        if data.kind.is_required_positional() {
            unnamed_parameters.push(parameter);
            unnamed_parameter_count += 1;
            required_parameter_count += 1;
        } else if data.kind.is_optional_positional() {
            unnamed_parameters.push(parameter);
            unnamed_parameter_count += 1;
        } else {
            let name = ctx.element_name(base.raw()).unwrap_or("").to_string();
            named_parameters
                .get_or_insert_with(IndexMap::new)
                .insert(name, parameter);
        }
    }
    let mut unnamed_index = 0;
    let arguments = rv.ast.list(rv.ast[argument_list].arguments).to_vec();
    let mut resolved_parameters: Vec<Option<ElemRef>> = vec![None; arguments.len()];
    let mut positional_argument_count = 0;
    let mut no_blank_arguments = true;
    let mut first_unresolved_argument: Option<Id<Expression>> = None;
    let mut last_positional_argument: Option<Id<Expression>> = None;
    for (i, &argument) in arguments.iter().enumerate() {
        if rv.ast.is::<NamedArgument>(argument) {
            continue;
        }
        let expression = argument_expression(rv, argument);
        if let Some(identifier) = rv.ast.cast::<dartr_ast::SimpleIdentifier>(expression)
            && rv.lexeme(rv.ast[identifier].token).is_empty()
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

    let mut used_names: Option<indexmap::IndexSet<String>> = None;
    if let Some(list) = enclosing_constructor_formal_parameter_list {
        let (positional, named) =
            verify_super_formal_parameters(rv, list, positional_argument_count != 0, report);
        positional_argument_count += positional;
        if !named.is_empty() {
            used_names = Some(named.into_iter().collect());
        }
    }

    for (i, &argument) in arguments.iter().enumerate() {
        let Some(named) = rv.ast.cast::<NamedArgument>(argument) else {
            continue;
        };
        let name_token = rv.ast[named].name;
        let name = rv.lexeme(name_token).to_string();
        let mut element = named_parameters
            .as_ref()
            .and_then(|m| m.get(&name).copied());
        if element.is_none() {
            element = if name.starts_with('_') && name.len() > 1 {
                named_parameters
                    .as_ref()
                    .and_then(|m| m.get(&name[1..]).copied())
            } else {
                None
            };
            if report {
                let d = match element {
                    None => diag::undefined_named_parameter(&name),
                    Some(_) => diag::use_of_private_parameter_name(&name[1..]),
                };
                let d = rv.at_token(d, name_token);
                rv.report(d);
            }
        } else {
            resolved_parameters[i] = element;
        }
        if !used_names
            .get_or_insert_with(Default::default)
            .insert(name.clone())
            && report
        {
            let d = rv.at_token(diag::duplicate_named_argument(&name), name_token);
            rv.report(d);
        }
    }

    if report {
        if positional_argument_count < required_parameter_count && no_blank_arguments {
            let token = match last_positional_argument {
                Some(e) => rv.ast.tokens.next(rv.ast.end_token(e)),
                None => rv.ast.tokens.next(rv.ast[argument_list].left_parenthesis),
            };
            let parent = rv.ast.parent(argument_list);
            report_not_enough_positional_arguments(
                rv,
                token,
                required_parameter_count,
                positional_argument_count,
                parent,
            );
        } else if positional_argument_count > unnamed_parameter_count && no_blank_arguments {
            let named_parameter_count = named_parameters.as_ref().map_or(0, |m| m.len());
            let named_argument_count = used_names.as_ref().map_or(0, |s| s.len());
            if let Some(first) = first_unresolved_argument {
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
    }

    for (i, &argument) in arguments.iter().enumerate() {
        // Keyed by the argument expression (the expression of a named
        // argument; Dart `Expression.correspondingParameter`).
        let expression = argument_expression(rv, argument);
        match resolved_parameters[i] {
            Some(p) => {
                rv.tables.param_element.insert(expression, p);
            }
            None => {
                rv.tables.param_element.remove(expression);
            }
        }
    }
}

/// Dart `ResolverVisitor._reportNotEnoughPositionalArguments`.
pub fn report_not_enough_positional_arguments(
    rv: &mut ResolverVisitor<'_>,
    token: dartr_syntax::TokenId,
    required_parameter_count: usize,
    actual_argument_count: usize,
    name_node: Option<NodeId>,
) {
    let name = name_node.and_then(|n| invoked_name(rv, n));
    let d = if required_parameter_count == 1 {
        match name {
            Some(name) => diag::not_enough_positional_arguments_name_singular(&name),
            None => diag::not_enough_positional_arguments_singular(),
        }
    } else {
        match name {
            Some(name) => diag::not_enough_positional_arguments_name_plural(
                required_parameter_count as i64,
                actual_argument_count as i64,
                &name,
            ),
            None => diag::not_enough_positional_arguments_plural(
                required_parameter_count as i64,
                actual_argument_count as i64,
            ),
        }
    };
    let d = rv.at_token(d, token);
    rv.report(d);
}

/// The name that `_reportNotEnoughPositionalArguments` puts into the
/// message (Dart: the switch over `nameNode`).
fn invoked_name(rv: &ResolverVisitor<'_>, node: NodeId) -> Option<String> {
    use dartr_ast::{
        DotShorthandConstructorInvocation, DotShorthandInvocation, EnumConstantArguments,
        EnumConstantDeclaration, InstanceCreationExpression, MethodInvocation,
        RedirectingConstructorInvocation, SuperConstructorInvocation,
    };
    let ast = &*rv.ast;
    let ctx = rv.ctx;
    // `'${element.returnType.getDisplayString()}.new'`.
    let constructor_new = |element: Option<ElemRef>| {
        element.map(|e| {
            let ty = member::return_type(&ctx, e);
            format!(
                "{}.new",
                dartr_element::diagnostics::type_display_string(&ctx, ty, true)
            )
        })
    };
    if let Some(n) = ast.cast::<InstanceCreationExpression>(node) {
        let constructor_name = ast[n].constructor_name;
        return Some(match ast[constructor_name].name {
            Some(id) => rv.lexeme(ast[id].token).to_string(),
            None => format!("{}.new", rv.lexeme(ast[ast[constructor_name].type_].name)),
        });
    }
    if let Some(n) = ast.cast::<RedirectingConstructorInvocation>(node) {
        return match ast[n].constructor_name {
            Some(id) => Some(rv.lexeme(ast[id].token).to_string()),
            None => constructor_new(rv.element(n)),
        };
    }
    if let Some(n) = ast.cast::<SuperConstructorInvocation>(node) {
        return match ast[n].constructor_name {
            Some(id) => Some(rv.lexeme(ast[id].token).to_string()),
            None => constructor_new(rv.element(n)),
        };
    }
    if let Some(n) = ast.cast::<MethodInvocation>(node) {
        return Some(rv.lexeme(ast[ast[n].method_name].token).to_string());
    }
    if let Some(n) = ast.cast::<EnumConstantArguments>(node) {
        return ast
            .parent(n)
            .filter(|&p| ast.is::<EnumConstantDeclaration>(p))
            .and_then(|p| enum_constant_type_name(rv, p));
    }
    if ast.is::<EnumConstantDeclaration>(node) {
        return enum_constant_type_name(rv, node);
    }
    if let Some(n) = ast.cast::<DotShorthandConstructorInvocation>(node) {
        return Some(rv.lexeme(ast[ast[n].constructor_name].token).to_string());
    }
    if let Some(n) = ast.cast::<DotShorthandInvocation>(node) {
        return Some(rv.lexeme(ast[ast[n].member_name].token).to_string());
    }
    None
}

/// Dart `declaredFragment!.element.type.getDisplayString()` of an enum
/// constant declaration.
fn enum_constant_type_name(rv: &ResolverVisitor<'_>, node: NodeId) -> Option<String> {
    let fragment = *rv.tables.declared_fragment.get(node)?;
    let element = *rv.ctx.fragment_data(fragment)?.element.try_get()?;
    let ty = crate::element_ext::variable_type(&rv.ctx, element);
    Some(dartr_element::diagnostics::type_display_string(
        &rv.ctx, ty, true,
    ))
}

/// Dart `verifySuperFormalParameters(formalParameterList:,
/// diagnosticReporter:, hasExplicitPositionalArguments:)`: the count of
/// positional super parameters and the names of the named super
/// parameters of [formal_parameter_list].
pub fn verify_super_formal_parameters(
    rv: &mut ResolverVisitor<'_>,
    formal_parameter_list: Id<dartr_ast::FormalParameterList>,
    has_explicit_positional_arguments: bool,
    report: bool,
) -> (usize, Vec<String>) {
    let mut positional_argument_count = 0;
    let mut named_argument_names = Vec::new();
    let parameters = rv
        .ast
        .list(rv.ast[formal_parameter_list].parameters)
        .to_vec();
    for parameter in parameters {
        let Some(parameter) = rv.ast.cast::<dartr_ast::SuperFormalParameter>(parameter) else {
            continue;
        };
        let name_token = rv.ast[parameter].name;
        if rv.ast[parameter].kind.is_named() {
            named_argument_names.push(rv.lexeme(name_token).to_string());
        } else {
            positional_argument_count += 1;
            if has_explicit_positional_arguments && report {
                let d = rv.at_token(
                    diag::positional_super_formal_parameter_with_positional_argument(),
                    name_token,
                );
                rv.report(d);
            }
        }
    }
    (positional_argument_count, named_argument_names)
}
