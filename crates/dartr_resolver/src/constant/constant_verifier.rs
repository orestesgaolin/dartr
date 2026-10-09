// Dart source: pkg/analyzer/lib/src/dart/constant/constant_verifier.dart

//! [`ConstantVerifier`]: walks a resolved unit and reports the diagnostics
//! of constant expressions (Dart `LibraryAnalyzer._computeConstantErrors`).
//!
//! The verifier uses the engine that computed the constants of the library
//! (Dart creates a new engine, but the evaluation results are shared through
//! the elements). The switch exhaustiveness check
//! (`_validateSwitchExhaustiveness`) is in [`crate::constant::exhaustiveness`].

use std::cell::RefCell;

use dartr_ast::{
    Annotation, AnonymousMethodInvocation, ArgumentList, AsExpression, AssertInitializer, Ast,
    AstVisitor, BlockClassBody, BlockEnumBody, ClassDeclaration, ConstantPattern,
    ConstructorDeclaration, ConstructorFieldInitializer, ConstructorReference,
    DotShorthandConstructorInvocation, EnumConstantDeclaration, EnumDeclaration, Expression,
    ExtensionDeclaration, ExtensionTypeDeclaration, FieldDeclaration, FormalParameterDefaultClause,
    FormalParameterList, FunctionExpression, FunctionReference, GenericFunctionType, Id, IfElement,
    InstanceCreationExpression, IsExpression, ListLiteral, MapLiteralEntry, MapPattern,
    MapPatternEntry, MethodDeclaration, MixinDeclaration, NamedArgument, NamedType, NodeId,
    NodeKind, NodeType, NullAwareElement, PrimaryConstructorBody, PrimaryConstructorDeclaration,
    RecordLiteral, RecordLiteralNamedField, RedirectingConstructorInvocation,
    RegularFormalParameter, RelationalPattern, SetOrMapLiteral, SpreadElement,
    SuperConstructorInvocation, SwitchCase, SwitchExpression, SwitchPatternCase, SwitchStatement,
    VariableDeclaration, VariableDeclarationList,
};
use dartr_constant::{Constant, DartObjectImpl, InvalidConstant, has_type_parameter_reference};
use dartr_diagnostics::{Diagnostic, DiagnosticCode, DiagnosticMessage, LocatableDiagnostic, diag};
use dartr_element::diagnostics::type_arg;
use dartr_element::{
    Ctx, EId, ElemRef, ElementId, FeatureSet, LibraryElement, Tag, TypeId, TypeKind,
};
use dartr_syntax::{TokenId, TokenType};
use dartr_typesystem::least_greatest_closure::PatternGreatestClosureHelper;
use dartr_typesystem::{TypeExt, TypeSystem, member};
use indexmap::{IndexMap, IndexSet};

use crate::ast_ext;
use crate::constant::evaluation::{
    ConstantEvaluationEngine, ConstantVisitor, NodeRef,
    dot_shorthand_constructor_invocation_is_const, formal_parameter_default_value,
    has_primary_constructor, is_const_constructor, is_factory_constructor, list_literal_is_const,
    record_literal_is_const, runtime_type_match, set_or_map_kind, set_or_map_literal_is_const,
};
use crate::constant::exhaustiveness::{
    ExhaustivenessCache, SwitchExhaustivenessInput, validate_switch_exhaustiveness,
};
use crate::constant::potentially_constant::{ConstCheckInput, get_not_potentially_constants};
use crate::library_analyzer::ResolvedUnit;

/// Dart `LibraryAnalyzer._computeConstantErrors(fileAnalysis)`: runs the
/// constant verifier over the unit [unit] of the engine and returns its
/// diagnostics in report order.
pub fn verify_unit(
    engine: &ConstantEvaluationEngine<'_>,
    cache: &mut ExhaustivenessCache,
    unit: u32,
) -> Vec<Diagnostic> {
    let handle = engine.unit(unit);
    let resolved: &ResolvedUnit = &handle;
    let ctx = engine.ctx(resolved);
    let library = engine.unit_library(unit);
    let mut verifier = ConstantVerifier {
        engine,
        unit_index: unit,
        unit: resolved,
        ctx,
        library,
        features: engine.library_features(library),
        diagnostics: RefCell::new(Vec::new()),
        cache,
        constant_pattern_values: None,
        map_pattern_key_values: None,
    };
    let ast = &resolved.ast;
    ast.accept(resolved.unit.raw(), &mut verifier);
    verifier.diagnostics.into_inner()
}

/// Dart `ConstantVerifier`: traverses an AST structure looking for
/// additional errors and warnings not covered by the parser and resolver.
/// In particular, it looks for errors and warnings related to constant
/// expressions.
pub struct ConstantVerifier<'v, 'a> {
    /// Dart `_evaluationEngine`.
    engine: &'v ConstantEvaluationEngine<'a>,
    /// The index of the unit in the engine.
    unit_index: u32,
    unit: &'v ResolvedUnit,
    ctx: Ctx<'v>,
    /// Dart `_currentLibrary`.
    library: EId<LibraryElement>,
    /// Dart `_currentLibrary.featureSet`.
    features: &'a FeatureSet,
    /// Dart `_diagnosticReporter`.
    diagnostics: RefCell<Vec<Diagnostic>>,
    /// Dart `_exhaustivenessCache`.
    cache: &'v mut ExhaustivenessCache,
    /// Dart `_constantPatternValues`: the constant values of the
    /// `ConstantPattern`s of the switch being verified.
    constant_pattern_values: Option<IndexMap<NodeId, DartObjectImpl>>,
    /// Dart `_mapPatternKeyValues`.
    map_pattern_key_values: Option<IndexMap<NodeId, DartObjectImpl>>,
}

impl<'v, 'a> ConstantVerifier<'v, 'a> {
    fn ast(&self) -> &'v Ast {
        &self.unit.ast
    }

    /// Dart `_typeSystem`.
    fn ts(&self) -> TypeSystem<'v> {
        TypeSystem::new(self.ctx)
    }

    fn node_ref(&self, node: impl Into<NodeId>) -> NodeRef {
        NodeRef::new(self.unit_index, node)
    }

    fn static_type(&self, node: impl Into<NodeId>) -> Option<TypeId> {
        self.unit.tables.static_type.get(node.into()).copied()
    }

    /// Dart `expression.typeOrThrow is InvalidType`. An expression without
    /// static type (Dart: `typeOrThrow` throws) was not resolved, for example
    /// the default values of a primary constructor, which the resolver does
    /// not resolve yet (`visit_primary_constructor_declaration` is a stub):
    /// it is skipped like an expression with an invalid type.
    fn has_invalid_or_no_type(&self, expression: impl Into<NodeId>) -> bool {
        matches!(self.static_type(expression), None | Some(TypeId::INVALID))
    }

    /// Dart `node.declaredFragment!.element` (a base element).
    fn declared_element(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        let fragment = *self.unit.tables.declared_fragment.get(node.into())?;
        self.ctx.fragment_data(fragment)?.element.try_get().copied()
    }

    fn report(&self, diagnostic: Diagnostic) {
        self.diagnostics.borrow_mut().push(diagnostic);
    }

    /// Dart `_diagnosticReporter.report(d.at(node))`.
    fn report_at_node(&self, d: LocatableDiagnostic, node: impl Into<NodeId>) {
        let node = node.into();
        let ast = self.ast();
        self.report(d.to_diagnostic(ast.offset(node) as usize, ast.length(node) as usize));
    }

    /// Dart `_diagnosticReporter.report(d.at(token))`.
    fn report_at_token(&self, d: LocatableDiagnostic, token: TokenId) {
        let ast = self.ast();
        let offset = ast.tokens.offset(token);
        let end = ast_ext::token_end(ast, token);
        self.report(d.to_diagnostic(offset as usize, (end - offset) as usize));
    }

    // -----------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------

    /// Returns `false` if we can prove that `constant == value` always
    /// returns `false`, taking into account the fact that [constant_type]
    /// has primitive equality.
    fn can_be_equal(&self, constant_type: TypeId, value_type: TypeId) -> bool {
        let ctx = &self.ctx;
        let ts = self.ts();
        if matches!(ctx.ty(constant_type), TypeKind::Interface { .. }) {
            match *ctx.ty(value_type) {
                TypeKind::Interface { .. } => {
                    if ctx.is_dart_core_int(constant_type) && ctx.is_dart_core_double(value_type) {
                        return true;
                    }
                    let value_type_greatest = PatternGreatestClosureHelper::new(
                        *ctx,
                        ts.object_question(),
                        ctx.tp.never_type(),
                    )
                    .eliminate_to_greatest(value_type);
                    return ts.is_subtype_of(constant_type, value_type_greatest);
                }
                TypeKind::TypeParameter {
                    param,
                    nullability,
                    promoted_bound,
                    ..
                } => {
                    let bound = promoted_bound.or_else(|| ctx.type_parameter_bound(param));
                    if let Some(bound) = bound
                        && !has_type_parameter_reference(ctx, bound)
                    {
                        let lowest_bound = if nullability == dartr_element::Nullability::Question {
                            ts.make_nullable(bound)
                        } else {
                            bound
                        };
                        return self.can_be_equal(constant_type, lowest_bound);
                    }
                }
                TypeKind::Function(_) | TypeKind::Record { .. } => {
                    if ctx.is_dart_core_null(constant_type) {
                        return ts.is_nullable(value_type);
                    }
                    return false;
                }
                _ => {}
            }
        }
        // All other cases are not supported, so no warning.
        true
    }

    /// Verify that the given [type_] does not reference any type parameters
    /// which are declared outside [type_].
    ///
    /// A generic function type is allowed to reference its own type
    /// parameter(s).
    fn check_for_const_with_type_parameters(
        &self,
        type_: NodeId,
        diagnostic: &LocatableDiagnostic,
        allowed_type_parameters: &IndexSet<ElementId>,
    ) {
        let ast = self.ast();
        let mut allowed_type_parameters = allowed_type_parameters.clone();
        if let Some(named_type) = ast.cast::<NamedType>(type_) {
            // Should not be a type parameter.
            let element = self
                .unit
                .tables
                .element
                .get(named_type)
                .map(|&e| member::base_element(&self.ctx, e));
            if let Some(element) = element
                && element.tag() == Tag::TypeParameter
                && !allowed_type_parameters.contains(&element)
            {
                self.report_at_node(diagnostic.clone(), type_);
                return;
            }
            // Check type arguments.
            if let Some(type_arguments) = ast[named_type].type_arguments {
                for &argument in ast.list_raw(ast[type_arguments].arguments) {
                    self.check_for_const_with_type_parameters(
                        argument,
                        diagnostic,
                        &allowed_type_parameters,
                    );
                }
            }
        } else if let Some(function_type) = ast.cast::<GenericFunctionType>(type_) {
            if let Some(type_parameters) = ast[function_type].type_parameters {
                let type_parameters = ast.list(ast[type_parameters].type_parameters).to_vec();
                for &tp in &type_parameters {
                    if let Some(e) = self.declared_element(tp) {
                        allowed_type_parameters.insert(e);
                    }
                }
                for &tp in &type_parameters {
                    if let Some(bound) = ast[tp].bound {
                        self.check_for_const_with_type_parameters(
                            bound.raw(),
                            diagnostic,
                            &allowed_type_parameters,
                        );
                    }
                }
            }
            if let Some(return_type) = ast[function_type].return_type {
                self.check_for_const_with_type_parameters(
                    return_type.raw(),
                    diagnostic,
                    &allowed_type_parameters,
                );
            }
            let parameters = ast[function_type].parameters;
            for &parameter in ast.list_raw(ast[parameters].parameters) {
                // In a generic function type, [parameter] can only be a non
                // function-typed regular formal parameter.
                if let Some(parameter) = ast.cast::<RegularFormalParameter>(parameter)
                    && ast[parameter].function_typed_suffix.is_none()
                    && let Some(parameter_type) = ast[parameter].type_
                {
                    self.check_for_const_with_type_parameters(
                        parameter_type.raw(),
                        diagnostic,
                        &allowed_type_parameters,
                    );
                }
            }
        }
    }

    /// Evaluates [expression] and reports any evaluation error.
    ///
    /// Returns the compile time constant of [expression], or an
    /// `InvalidConstant` if an error was found during evaluation. If an
    /// `InvalidConstant` was found, the error will be reported and
    /// [diagnostic_code] will be the default error code to be reported.
    fn evaluate_and_report_error(
        &self,
        expression: NodeId,
        diagnostic_code: &'static DiagnosticCode,
    ) -> Constant {
        // Dart: a sub-reporter whose diagnostics are dropped.
        let constant_visitor = ConstantVisitor::new(self.engine, self.library, None);
        let result = constant_visitor.evaluate_constant(self.node_ref(expression));
        if let Constant::Invalid(invalid) = &result {
            self.report_error(invalid, Some(diagnostic_code));
        }
        result
    }

    /// Reports an error to the diagnostic reporter.
    ///
    /// If the [error] isn't found in the list, use the given
    /// [default_diagnostic_code] instead.
    fn report_error(
        &self,
        error: &InvalidConstant,
        default_diagnostic_code: Option<&'static DiagnosticCode>,
    ) {
        if error.avoid_reporting {
            return;
        }

        // These error codes are more specific than the [defaultErrorCode] so
        // they will overwrite and replace the default when we report the
        // error.
        let diagnostic_code = error.locatable_diagnostic.code;
        let offset = error.offset.max(0) as usize;
        let length = error.length.max(0) as usize;
        if SPECIFIC_CODES
            .iter()
            .any(|&code| std::ptr::eq(code, diagnostic_code))
        {
            self.report(error.locatable_diagnostic.to_diagnostic(offset, length));
        } else if let Some(default_diagnostic_code) = default_diagnostic_code {
            self.report(Diagnostic::with_arguments(
                default_diagnostic_code,
                offset,
                length,
                &[],
                Vec::new(),
            ));
        }
    }

    /// Dart `getNotPotentiallyConstants(node, featureSet:)`.
    fn not_potentially_constants(&self, node: NodeId) -> Vec<NodeId> {
        let input = ConstCheckInput {
            ctx: &self.ctx,
            ast: self.ast(),
            tables: &self.unit.tables,
            rt: &self.unit.rt,
            features: self.features,
        };
        get_not_potentially_constants(&input, node)
    }

    /// Dart `_reportNotPotentialConstants`.
    fn report_not_potential_constants(&self, node: NodeId) {
        for not_const in self.not_potentially_constants(node) {
            self.report_at_node(diag::invalid_constant(), not_const);
        }
    }

    /// Validates that all arguments in the [argument_list] are potentially
    /// constant expressions.
    fn report_not_potential_constants_arguments(&self, argument_list: Id<ArgumentList>) {
        let ast = self.ast();
        for &argument in ast.list_raw(ast[argument_list].arguments) {
            self.report_not_potential_constants(argument_expression(ast, argument));
        }
    }

    /// Check if the object [obj] matches the type [ty] according to runtime
    /// type checking rules.
    fn runtime_type_match(&self, obj: &DartObjectImpl, ty: TypeId) -> bool {
        runtime_type_match(&self.ts(), obj, ty)
    }

    /// Validates that the arguments in [argument_list] are constant
    /// expressions.
    fn validate_constant_arguments(&self, argument_list: Id<ArgumentList>) {
        let ast = self.ast();
        for &argument in ast.list_raw(ast[argument_list].arguments) {
            let real_argument = argument_expression(ast, argument);
            self.evaluate_and_report_error(real_argument, &diag::CONST_WITH_NON_CONSTANT_ARGUMENT);
        }
    }

    /// Validates that expressions in [initializers] are constant
    /// expressions.
    fn validate_constructor_initializers(&self, initializers: &[NodeId]) {
        let ast = self.ast();
        for &initializer in initializers {
            if let Some(initializer) = ast.cast::<AssertInitializer>(initializer) {
                self.report_not_potential_constants(ast[initializer].condition.raw());
                if let Some(message) = ast[initializer].message {
                    self.report_not_potential_constants(message.raw());
                }
            } else if let Some(initializer) = ast.cast::<ConstructorFieldInitializer>(initializer) {
                self.report_not_potential_constants(ast[initializer].expression.raw());
            } else if let Some(initializer) =
                ast.cast::<RedirectingConstructorInvocation>(initializer)
            {
                self.report_not_potential_constants_arguments(ast[initializer].argument_list);
            } else if let Some(initializer) = ast.cast::<SuperConstructorInvocation>(initializer) {
                self.report_not_potential_constants_arguments(ast[initializer].argument_list);
            }
        }
    }

    /// Validates that the [constructor] invocation, its type arguments, and
    /// its arguments are constant expressions.
    fn validate_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: NodeId,
        constructor: ElemRef,
        argument_list: Id<ArgumentList>,
    ) {
        let result = {
            let constant_visitor =
                ConstantVisitor::new(self.engine, self.library, Some(&self.diagnostics));
            let type_arguments = self
                .ctx
                .type_arguments(member::return_type(&self.ctx, constructor))
                .to_vec();
            let arguments: Vec<NodeRef> = ast
                .list_raw(ast[argument_list].arguments)
                .iter()
                .map(|&a| self.node_ref(a))
                .collect();
            self.engine.evaluate_and_format_errors_in_constructor_call(
                self.library,
                self.node_ref(node),
                Some(type_arguments),
                &arguments,
                constructor,
                &constant_visitor,
                None,
            )
        };
        match result {
            Constant::Invalid(result) => {
                if !result.avoid_reporting {
                    self.report(result.locatable_diagnostic.to_diagnostic(
                        result.offset.max(0) as usize,
                        result.length.max(0) as usize,
                    ));
                }
            }
            Constant::Value(_) => {
                // Check for further errors in individual arguments.
                ast.accept(argument_list.raw(), self);
            }
        }
    }

    /// Validates that the default value associated with each of the
    /// parameters in [parameters] is a constant expression.
    fn validate_default_values(&self, parameters: Option<Id<FormalParameterList>>) {
        let Some(parameters) = parameters else {
            return;
        };
        let ast = self.ast();
        for &parameter in ast.list_raw(ast[parameters].parameters) {
            let Some(default_value) = formal_parameter_default_value(ast, parameter) else {
                continue;
            };
            let result =
                if self.has_invalid_or_no_type(default_value) {
                    // We have already reported an error.
                    None
                } else {
                    Some(self.evaluate_and_report_error(
                        default_value,
                        &diag::NON_CONSTANT_DEFAULT_VALUE,
                    ))
                };
            if let Some(element) = self.declared_element(parameter) {
                self.engine.replace_evaluation_result(element, result);
            }
        }
    }

    /// Validates that the expressions of any field initializers in
    /// [members] are all compile-time constants. Since this is only
    /// required if the class has a constant constructor, the error is
    /// reported at [const_keyword], the const keyword on such a constant
    /// constructor.
    fn validate_field_initializers(
        &self,
        members: &[NodeId],
        const_keyword: TokenId,
        is_enum_declaration: bool,
    ) {
        let ast = self.ast();
        for &member in members {
            let Some(member) = ast.cast::<FieldDeclaration>(member) else {
                continue;
            };
            if ast[member].static_keyword.is_some() {
                continue;
            }
            let fields = ast[member].fields;
            for &variable_declaration in ast.list(ast[fields].variables) {
                let name = ast[variable_declaration].name;
                if is_enum_declaration && ast.tokens.lexeme(name) == "values" {
                    continue;
                }
                if let Some(initializer) = ast[variable_declaration].initializer {
                    // Ignore any diagnostics produced during validation--if
                    // the constant can't be evaluated we'll just report a
                    // single error.
                    let result = ConstantVisitor::new(self.engine, self.library, None)
                        .evaluate_constant(self.node_ref(initializer));
                    if !matches!(result, Constant::Value(_)) {
                        self.report_at_token(
                            diag::const_constructor_with_field_initialized_by_non_const(
                                ast.tokens.lexeme(name),
                            ),
                            const_keyword,
                        );
                    }
                }
            }
        }
    }

    /// Validates that field declaration initializers in a const primary
    /// constructor class are potentially constant. Unlike
    /// [Self::validate_field_initializers], this allows references to
    /// primary constructor parameters.
    fn validate_primary_field_initializers(
        &self,
        members: &[NodeId],
        error_token: TokenId,
        is_enum_declaration: bool,
    ) {
        let ast = self.ast();
        for &member in members {
            let Some(member) = ast.cast::<FieldDeclaration>(member) else {
                continue;
            };
            if ast[member].static_keyword.is_some() {
                continue;
            }
            let fields = ast[member].fields;
            for &variable_declaration in ast.list(ast[fields].variables) {
                let name = ast[variable_declaration].name;
                if is_enum_declaration && ast.tokens.lexeme(name) == "values" {
                    continue;
                }
                let Some(initializer) = ast[variable_declaration].initializer else {
                    continue;
                };
                if !self.not_potentially_constants(initializer.raw()).is_empty() {
                    self.report_at_token(
                        diag::const_constructor_with_field_initialized_by_non_const(
                            ast.tokens.lexeme(name),
                        ),
                        error_token,
                    );
                }
            }
        }
    }

    /// Dart `_validateSwitchStatement_nullSafety`.
    fn validate_switch_statement_null_safety(&mut self, ast: &Ast, node: Id<SwitchStatement>) {
        for &switch_member in ast.list_raw(ast[node].members) {
            if let Some(switch_member) = ast.cast::<SwitchCase>(switch_member) {
                self.validate_case_expression(ast[switch_member].expression.raw());
            } else if let Some(switch_member) = ast.cast::<SwitchPatternCase>(switch_member) {
                if self.features.is_enabled("patterns") {
                    ast.accept(switch_member.raw(), self);
                } else {
                    let guarded_pattern = ast[switch_member].guarded_pattern;
                    let pattern = ast[guarded_pattern].pattern;
                    if let Some(pattern) = ast.cast::<ConstantPattern>(pattern) {
                        let expression = ast_ext::un_parenthesized(ast, ast[pattern].expression);
                        self.validate_case_expression(expression.raw());
                    }
                }
            }
        }
    }

    /// Dart `validateExpression` of `_validateSwitchStatement_nullSafety`.
    fn validate_case_expression(&self, expression: NodeId) {
        let expression_value =
            self.evaluate_and_report_error(expression, &diag::NON_CONSTANT_CASE_EXPRESSION);
        let Constant::Value(expression_value) = expression_value else {
            return;
        };
        if !self.features.is_enabled("patterns") {
            let expression_type = expression_value.ty;
            if !expression_value.has_primitive_equality(&self.ts(), self.features) {
                self.report_at_node(
                    diag::case_expression_type_implements_equals(type_arg(
                        &self.ctx,
                        expression_type,
                    )),
                    expression,
                );
            }
        }
    }

    /// Dart `_withConstantPatternValues(f)` around the visit of the
    /// children of [node] and `_validateSwitchExhaustiveness`.
    fn with_constant_pattern_values(
        &mut self,
        ast: &Ast,
        node: NodeId,
        validate: impl FnOnce(
            &mut Self,
            &IndexMap<NodeId, DartObjectImpl>,
            &IndexMap<NodeId, DartObjectImpl>,
        ),
    ) {
        let previous_map_key_values = self.map_pattern_key_values.replace(IndexMap::new());
        let previous_constant_pattern_values =
            self.constant_pattern_values.replace(IndexMap::new());
        ast.visit_children(node, self);
        let map_key_values = self.map_pattern_key_values.take().unwrap_or_default();
        let constant_values = self.constant_pattern_values.take().unwrap_or_default();
        // Dart restores the previous maps after `f`; the inner maps are only
        // read by `f`.
        validate(self, &map_key_values, &constant_values);
        self.map_pattern_key_values = previous_map_key_values;
        self.constant_pattern_values = previous_constant_pattern_values;
    }

    /// Calls [validate_switch_exhaustiveness] (Dart
    /// `_validateSwitchExhaustiveness`).
    fn validate_switch_exhaustiveness(
        &mut self,
        node: NodeId,
        map_pattern_key_values: &IndexMap<NodeId, DartObjectImpl>,
        constant_pattern_values: &IndexMap<NodeId, DartObjectImpl>,
        must_be_exhaustive: bool,
        is_switch_expression: bool,
    ) {
        let input = SwitchExhaustivenessInput {
            node,
            map_pattern_key_values,
            constant_pattern_values,
            must_be_exhaustive,
            is_switch_expression,
        };
        let mut diagnostics = self.diagnostics.borrow_mut();
        validate_switch_exhaustiveness(
            self.engine,
            self.cache,
            self.unit_index,
            &input,
            &mut diagnostics,
        );
    }

    /// Dart `ExpressionExtension.inConstantExpression` (the extension at the
    /// end of constant_verifier.dart): whether [node] is found in a constant
    /// expression. This does not check whether [node] is found in a
    /// constant context.
    fn in_constant_expression(&self, node: NodeId) -> bool {
        let ast = self.ast();
        let mut child = node;
        let mut parent = ast.parent(child);
        while let Some(p) = parent {
            if let Some(clause) = ast.cast::<FormalParameterDefaultClause>(p)
                && ast[clause].value.raw() == child
            {
                // A parameter default value does not constitute a constant
                // context, but must be a constant expression.
                return true;
            } else if let Some(declaration) = ast.cast::<VariableDeclaration>(p)
                && ast[declaration].initializer.map(|i| i.raw()) == Some(child)
            {
                let declaration_list = ast.parent(p);
                if let Some(declaration_list) = declaration_list
                    && ast.kind(declaration_list) == NodeKind::VariableDeclarationList
                    && let Some(declaration_list_parent) = ast.parent(declaration_list)
                    && let Some(field) = ast.cast::<FieldDeclaration>(declaration_list_parent)
                    && ast[field].static_keyword.is_none()
                    && let Some(body) = ast.parent(field)
                    && ast.kind(body) == NodeKind::BlockClassBody
                    && let Some(container) = ast.parent(body)
                    && ast.kind(container) == NodeKind::ClassDeclaration
                    && let Some(enclosing_class) = self.declared_element(container)
                    && enclosing_class.tag() == Tag::Class
                {
                    // A field initializer of a class with at least one
                    // generative const constructor does not constitute a
                    // constant context, but must be a constant expression.
                    return has_generative_const_constructor(&self.ctx, enclosing_class);
                }
                return false;
            } else {
                child = p;
                parent = ast.parent(child);
            }
        }
        false
    }

    /// Dart `ConstructorDeclaration.errorRange`.
    fn constructor_error_range(&self, node: Id<ConstructorDeclaration>) -> Option<(u32, u32)> {
        let ast = self.ast();
        let n = &ast[node];
        let (start, start_end) = match n.type_name {
            Some(type_name) => (ast.offset(type_name), ast.end(type_name)),
            None => {
                let token = n.new_keyword.or(n.factory_keyword)?;
                (ast.tokens.offset(token), ast_ext::token_end(ast, token))
            }
        };
        let end = match n.name {
            Some(name) => ast_ext::token_end(ast, name),
            None => start_end,
        };
        Some((start, end.saturating_sub(start)))
    }
}

/// The codes that `_reportError` reports as they are (more specific than
/// the default code).
static SPECIFIC_CODES: &[&DiagnosticCode] = &[
    &diag::CONST_EVAL_EXTENSION_METHOD,
    &diag::CONST_EVAL_EXTENSION_TYPE_METHOD,
    &diag::CONST_EVAL_FOR_ELEMENT,
    &diag::CONST_EVAL_METHOD_INVOCATION,
    &diag::CONST_EVAL_PRIMITIVE_EQUALITY,
    &diag::CONST_EVAL_PROPERTY_ACCESS,
    &diag::CONST_EVAL_THROWS_EXCEPTION,
    &diag::CONST_EVAL_THROWS_IDBZE,
    &diag::CONST_EVAL_TYPE_BOOL_NUM_STRING,
    &diag::CONST_EVAL_TYPE_BOOL,
    &diag::CONST_EVAL_TYPE_BOOL_INT,
    &diag::CONST_EVAL_TYPE_INT,
    &diag::CONST_EVAL_TYPE_NUM,
    &diag::CONST_EVAL_TYPE_NUM_STRING,
    &diag::CONST_EVAL_TYPE_STRING,
    &diag::RECURSIVE_COMPILE_TIME_CONSTANT,
    &diag::CONST_CONSTRUCTOR_FIELD_TYPE_MISMATCH,
    &diag::CONST_CONSTRUCTOR_PARAM_TYPE_MISMATCH,
    &diag::CONST_TYPE_PARAMETER,
    &diag::CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF,
    &diag::CONST_SPREAD_EXPECTED_LIST_OR_SET,
    &diag::CONST_SPREAD_EXPECTED_MAP,
    &diag::EXPRESSION_IN_MAP,
    &diag::VARIABLE_TYPE_MISMATCH,
    &diag::NON_BOOL_CONDITION,
    &diag::NON_CONSTANT_DEFAULT_VALUE_FROM_DEFERRED_LIBRARY,
    &diag::NON_CONSTANT_MAP_KEY_FROM_DEFERRED_LIBRARY,
    &diag::NON_CONSTANT_MAP_VALUE_FROM_DEFERRED_LIBRARY,
    &diag::SET_ELEMENT_FROM_DEFERRED_LIBRARY,
    &diag::SPREAD_EXPRESSION_FROM_DEFERRED_LIBRARY,
    &diag::NON_CONSTANT_CASE_EXPRESSION_FROM_DEFERRED_LIBRARY,
    &diag::INVALID_ANNOTATION_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY,
    &diag::IF_ELEMENT_CONDITION_FROM_DEFERRED_LIBRARY,
    &diag::CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY,
    &diag::NON_CONSTANT_LIST_ELEMENT_FROM_DEFERRED_LIBRARY,
    &diag::NON_CONSTANT_RECORD_FIELD_FROM_DEFERRED_LIBRARY,
    &diag::PATTERN_CONSTANT_FROM_DEFERRED_LIBRARY,
    &diag::WRONG_NUMBER_OF_TYPE_ARGUMENTS_ELEMENT,
    &diag::WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION,
];

/// Dart `Argument.argumentExpression`: the expression of a named argument,
/// or the argument itself.
fn argument_expression(ast: &Ast, argument: NodeId) -> NodeId {
    match ast.cast::<NamedArgument>(argument) {
        Some(named) => ast[named].argument_expression.raw(),
        None => argument,
    }
}

/// Dart `ClassElementImpl.hasGenerativeConstConstructor`.
fn has_generative_const_constructor(ctx: &Ctx<'_>, class: ElementId) -> bool {
    let Some(interface) = class.cast::<dartr_element::InterfaceElement>() else {
        return false;
    };
    ctx.interface(interface)
        .constructors
        .iter()
        .any(|c| !is_factory_constructor(ctx, c.raw()) && is_const_constructor(ctx, c.raw()))
}

/// Dart `AstNodeNullableExtension.classMembers` of [node].
fn class_members(ast: &Ast, node: NodeId) -> Vec<NodeId> {
    let body_members = |body: NodeId| -> Vec<NodeId> {
        if let Some(body) = ast.cast::<BlockClassBody>(body) {
            ast.list_raw(ast[body].members).to_vec()
        } else if let Some(body) = ast.cast::<BlockEnumBody>(body) {
            ast.list_raw(ast[body].members).to_vec()
        } else {
            Vec::new()
        }
    };
    match ast.kind(node) {
        NodeKind::BlockClassBody | NodeKind::BlockEnumBody => body_members(node),
        NodeKind::ClassDeclaration => {
            body_members(ast[Id::<ClassDeclaration>::from_raw(node)].body.raw())
        }
        NodeKind::EnumDeclaration => {
            body_members(ast[Id::<EnumDeclaration>::from_raw(node)].body.raw())
        }
        NodeKind::ExtensionDeclaration => {
            body_members(ast[Id::<ExtensionDeclaration>::from_raw(node)].body.raw())
        }
        NodeKind::ExtensionTypeDeclaration => body_members(
            ast[Id::<ExtensionTypeDeclaration>::from_raw(node)]
                .body
                .raw(),
        ),
        NodeKind::MixinDeclaration => {
            body_members(ast[Id::<MixinDeclaration>::from_raw(node)].body.raw())
        }
        _ => Vec::new(),
    }
}

/// A context message "The first ... with this value." at [original].
fn first_value_message(unit: &ResolvedUnit, message: &str, original: NodeId) -> DiagnosticMessage {
    DiagnosticMessage {
        file_path: unit.path.to_string(),
        offset: i64::from(unit.ast.offset(original)),
        length: i64::from(unit.ast.length(original)),
        message: message.to_string(),
        url: None,
    }
}

impl AstVisitor for ConstantVerifier<'_, '_> {
    fn visit_annotation(&mut self, ast: &Ast, node: Id<Annotation>) {
        ast.visit_children(node, self);
        // check annotation creation
        let Some(element) = self.unit.tables.element.get(node).copied() else {
            return;
        };
        let base = member::base_element(&self.ctx, element);
        if base.tag() == Tag::Constructor {
            // should be 'const' constructor
            if !is_const_constructor(&self.ctx, base) {
                self.report_at_node(diag::non_constant_annotation_constructor(), node);
                return;
            }
            // should have arguments
            let Some(argument_list) = ast[node].arguments else {
                self.report_at_node(diag::no_annotation_constructor_arguments(), node);
                return;
            };
            // arguments should be constants
            self.validate_constant_arguments(argument_list);
        }
    }

    fn visit_anonymous_method_invocation(
        &mut self,
        ast: &Ast,
        node: Id<AnonymousMethodInvocation>,
    ) {
        ast.visit_children(node, self);
        self.validate_default_values(ast[node].parameters);
    }

    fn visit_constant_pattern(&mut self, ast: &Ast, node: Id<ConstantPattern>) {
        let expression = ast_ext::un_parenthesized(ast, ast[node].expression);
        if self.has_invalid_or_no_type(expression) {
            return;
        }

        let value = self.evaluate_and_report_error(
            expression.raw(),
            &diag::CONSTANT_PATTERN_WITH_NON_CONSTANT_EXPRESSION,
        );
        if let Constant::Value(value) = value {
            if self.features.is_enabled("patterns") {
                if let Some(values) = &mut self.constant_pattern_values {
                    values.insert(node.raw(), value.clone());
                }
                let ts = self.ts();
                if value.has_primitive_equality(&ts, self.features) {
                    let constant_type = value.ty;
                    let matched_value_type = self
                        .unit
                        .tables
                        .pattern_info
                        .get(node)
                        .and_then(|info| info.matched_value_type)
                        .map(|t| ts.extension_type_erasure(t));
                    if let Some(matched_value_type) = matched_value_type
                        && !self.can_be_equal(constant_type, matched_value_type)
                    {
                        self.report_at_node(
                            diag::constant_pattern_never_matches_value_type(
                                type_arg(&self.ctx, matched_value_type),
                                type_arg(&self.ctx, constant_type),
                            ),
                            node,
                        );
                        return;
                    }
                }
            }
            ast.visit_children(node, self);
        }
    }

    fn visit_constructor_declaration(&mut self, ast: &Ast, node: Id<ConstructorDeclaration>) {
        if let Some(const_keyword) = ast[node].const_keyword {
            // Check and report cycles.
            // Factory cycles are reported in elsewhere in
            // [ErrorVerifier._checkForRecursiveFactoryRedirect].
            if let Some(element) = self.declared_element(node) {
                if !self.engine.is_cycle_free(element)
                    && !is_factory_constructor(&self.ctx, element)
                    && let Some((offset, length)) = self.constructor_error_range(node)
                {
                    self.report(
                        diag::recursive_constant_constructor()
                            .to_diagnostic(offset as usize, length as usize),
                    );
                }

                let initializers = ast.list_raw(ast[node].initializers).to_vec();
                self.validate_constructor_initializers(&initializers);
                let enclosing = self.ctx.element_data(element).and_then(|d| d.enclosing);
                if ast[node].factory_keyword.is_none()
                    && let Some(enclosing) = enclosing
                    && !has_primary_constructor(&self.ctx, enclosing)
                    && let Some(parent) = ast.parent(node)
                {
                    let members = class_members(ast, parent);
                    self.validate_field_initializers(
                        &members,
                        const_keyword,
                        enclosing.tag() == Tag::Enum,
                    );
                }
            }
        }
        self.validate_default_values(Some(ast[node].parameters));
        ast.visit_children(node, self);
    }

    fn visit_constructor_reference(&mut self, ast: &Ast, node: Id<ConstructorReference>) {
        ast.visit_children(node, self);
        if ast_ext::in_constant_context(ast, node.raw()) || self.in_constant_expression(node.raw())
        {
            let constructor_name = ast[node].constructor_name;
            self.check_for_const_with_type_parameters(
                ast[constructor_name].type_.raw(),
                &diag::const_with_type_parameters_constructor_tearoff(),
                &IndexSet::new(),
            );
        }
    }

    fn visit_dot_shorthand_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<DotShorthandConstructorInvocation>,
    ) {
        if dot_shorthand_constructor_invocation_is_const(ast, node) {
            let constructor = self
                .unit
                .tables
                .element
                .get(ast[node].constructor_name)
                .copied();
            if let Some(constructor) = constructor
                && member::base_element(&self.ctx, constructor).tag() == Tag::Constructor
            {
                self.validate_constructor_invocation(
                    ast,
                    node.raw(),
                    constructor,
                    ast[node].argument_list,
                );
            }
        } else {
            ast.visit_children(node, self);
        }
    }

    fn visit_enum_constant_declaration(&mut self, ast: &Ast, node: Id<EnumConstantDeclaration>) {
        ast.visit_children(node, self);

        if let Some(arguments) = ast[node].arguments {
            self.validate_constant_arguments(ast[arguments].argument_list);
        }

        if let Some(element) = self.declared_element(node)
            && let Some(Constant::Invalid(result)) = self.engine.evaluation_result(element)
        {
            self.report_error(&result, None);
        }
    }

    fn visit_function_expression(&mut self, ast: &Ast, node: Id<FunctionExpression>) {
        ast.visit_children(node, self);
        self.validate_default_values(ast[node].parameters);
    }

    fn visit_function_reference(&mut self, ast: &Ast, node: Id<FunctionReference>) {
        ast.visit_children(node, self);
        if ast_ext::in_constant_context(ast, node.raw()) || self.in_constant_expression(node.raw())
        {
            let Some(type_arguments) = ast[node].type_arguments else {
                return;
            };
            for &type_argument in ast.list_raw(ast[type_arguments].arguments) {
                self.check_for_const_with_type_parameters(
                    type_argument,
                    &diag::const_with_type_parameters_function_tearoff(),
                    &IndexSet::new(),
                );
            }
        }
    }

    fn visit_generic_function_type(&mut self, ast: &Ast, node: Id<GenericFunctionType>) {
        // TODO(srawlins): Also check interface types (TypeName?).
        ast.visit_children(node, self);
        if let Some(parent) = ast.parent(node)
            && (ast.cast::<AsExpression>(parent).is_some()
                || ast.cast::<IsExpression>(parent).is_some())
            && ast_ext::in_constant_context(ast, parent)
        {
            self.check_for_const_with_type_parameters(
                node.raw(),
                &diag::const_with_type_parameters(),
                &IndexSet::new(),
            );
        }
    }

    fn visit_instance_creation_expression(
        &mut self,
        ast: &Ast,
        node: Id<InstanceCreationExpression>,
    ) {
        if ast_ext::instance_creation_is_const(ast, node) {
            let constructor_name = ast[node].constructor_name;
            let named_type = ast[constructor_name].type_;
            self.check_for_const_with_type_parameters(
                named_type.raw(),
                &diag::const_with_type_parameters(),
                &IndexSet::new(),
            );

            let constructor = self.unit.tables.element.get(constructor_name).copied();
            if let Some(constructor) = constructor {
                self.validate_constructor_invocation(
                    ast,
                    node.raw(),
                    constructor,
                    ast[node].argument_list,
                );
            }
        } else {
            ast.visit_children(node, self);
        }
    }

    fn visit_list_literal(&mut self, ast: &Ast, node: Id<ListLiteral>) {
        ast.visit_children(node, self);
        if list_literal_is_const(ast, node) {
            let Some(element_type) = self.type_argument(node.raw(), 0) else {
                return;
            };
            let mut verifier = ConstLiteralVerifier {
                diagnostic_code: &diag::NON_CONSTANT_LIST_ELEMENT,
                list_element_type: Some(element_type),
                set_config: None,
                map_config: None,
            };
            for &element in ast.list_raw(ast[node].elements) {
                verifier.verify(self, element);
            }
        }
    }

    fn visit_map_pattern(&mut self, ast: &Ast, node: Id<MapPattern>) {
        if let Some(type_arguments) = ast[node].type_arguments {
            ast.accept(type_arguments.raw(), self);
        }

        let ts = self.ts();
        let features = self.features;
        // Dart: a `HashMap` with `hashCode: (_) => 0` and this equality.
        let equals = |a: &DartObjectImpl, b: &DartObjectImpl| -> bool {
            if a.is_identical2(&ts, b).to_bool_value() == Some(true) {
                return true;
            }
            if a.has_primitive_equality(&ts, features) && b.has_primitive_equality(&ts, features) {
                return a.dart_eq(b, &ts);
            }
            false
        };
        let mut unique_keys: Vec<(DartObjectImpl, NodeId)> = Vec::new();
        let mut duplicate_keys: IndexMap<NodeId, NodeId> = IndexMap::new();
        for &element in ast.list_raw(ast[node].elements) {
            ast.accept(element, self);
            if let Some(entry) = ast.cast::<MapPatternEntry>(element) {
                let key = ast[entry].key.raw();
                let key_value =
                    self.evaluate_and_report_error(key, &diag::NON_CONSTANT_MAP_PATTERN_KEY);
                if let Constant::Value(key_value) = key_value {
                    if let Some(values) = &mut self.map_pattern_key_values {
                        values.insert(key, key_value.clone());
                    }
                    let existing_key = unique_keys
                        .iter()
                        .find(|(k, _)| equals(k, &key_value))
                        .map(|&(_, n)| n);
                    match existing_key {
                        Some(existing_key) => {
                            duplicate_keys.insert(key, existing_key);
                        }
                        None => unique_keys.push((key_value, key)),
                    }
                }
            }
        }

        for (&duplicate, &original) in &duplicate_keys {
            let message =
                first_value_message(self.unit, "The first key with this value.", original);
            self.report_at_node(
                diag::equal_keys_in_map_pattern().with_context_messages([message]),
                duplicate,
            );
        }
    }

    fn visit_method_declaration(&mut self, ast: &Ast, node: Id<MethodDeclaration>) {
        ast.visit_children(node, self);
        self.validate_default_values(ast[node].parameters);
    }

    fn visit_primary_constructor_body(&mut self, ast: &Ast, node: Id<PrimaryConstructorBody>) {
        let element = primary_constructor_body_declaration(ast, node)
            .and_then(|declaration| self.declared_element(declaration));
        if let Some(element) = element
            && element.tag() == Tag::Constructor
            && is_const_constructor(&self.ctx, element)
        {
            let initializers = ast.list_raw(ast[node].initializers).to_vec();
            self.validate_constructor_initializers(&initializers);
        }
        ast.visit_children(node, self);
    }

    fn visit_primary_constructor_declaration(
        &mut self,
        ast: &Ast,
        node: Id<PrimaryConstructorDeclaration>,
    ) {
        ast.visit_children(node, self);

        if let Some(element) = self.declared_element(node)
            && is_const_constructor(&self.ctx, element)
        {
            let members = ast
                .parent(node)
                .map(|parent| class_members(ast, parent))
                .unwrap_or_default();
            let enclosing = self.ctx.element_data(element).and_then(|d| d.enclosing);
            self.validate_primary_field_initializers(
                &members,
                ast[node].const_keyword.unwrap_or(ast[node].type_name),
                enclosing.is_some_and(|e| e.tag() == Tag::Enum),
            );
        }

        self.validate_default_values(Some(ast[node].formal_parameters));
    }

    fn visit_record_literal(&mut self, ast: &Ast, node: Id<RecordLiteral>) {
        ast.visit_children(node, self);

        if record_literal_is_const(ast, node) {
            for &field in ast.list_raw(ast[node].fields) {
                let field_expression = match ast.cast::<RecordLiteralNamedField>(field) {
                    Some(named) => ast[named].field_expression.raw(),
                    None => field,
                };
                self.evaluate_and_report_error(field_expression, &diag::NON_CONSTANT_RECORD_FIELD);
            }
        }
    }

    fn visit_relational_pattern(&mut self, ast: &Ast, node: Id<RelationalPattern>) {
        ast.visit_children(node, self);

        self.evaluate_and_report_error(
            ast[node].operand.raw(),
            &diag::NON_CONSTANT_RELATIONAL_PATTERN_EXPRESSION,
        );
    }

    fn visit_set_or_map_literal(&mut self, ast: &Ast, node: Id<SetOrMapLiteral>) {
        ast.visit_children(node, self);
        let (is_set, is_map) = set_or_map_kind(self.unit, &self.ctx, node);
        let elements = ast.list_raw(ast[node].elements).to_vec();
        if is_set {
            if set_or_map_literal_is_const(ast, node) {
                let Some(element_type) = self.type_argument(node.raw(), 0) else {
                    return;
                };
                let mut verifier = ConstLiteralVerifier {
                    diagnostic_code: &diag::NON_CONSTANT_SET_ELEMENT,
                    list_element_type: None,
                    set_config: Some(SetVerifierConfig {
                        element_type,
                        unique_values: ObjectMap::default(),
                        duplicate_elements: IndexMap::new(),
                    }),
                    map_config: None,
                };
                for &element in &elements {
                    verifier.verify(self, element);
                }
                let config = verifier.set_config.unwrap();
                for (&duplicate, &original) in &config.duplicate_elements {
                    let message = first_value_message(
                        self.unit,
                        "The first element with this value.",
                        original,
                    );
                    self.report_at_node(
                        diag::equal_elements_in_const_set().with_context_messages([message]),
                        duplicate,
                    );
                }
            }
        } else if is_map && set_or_map_literal_is_const(ast, node) {
            let (Some(key_type), Some(value_type)) = (
                self.type_argument(node.raw(), 0),
                self.type_argument(node.raw(), 1),
            ) else {
                return;
            };
            let mut verifier = ConstLiteralVerifier {
                diagnostic_code: &diag::NON_CONSTANT_MAP_ELEMENT,
                list_element_type: None,
                set_config: None,
                map_config: Some(MapVerifierConfig {
                    key_type,
                    value_type,
                    unique_keys: ObjectMap::default(),
                    duplicate_keys: IndexMap::new(),
                }),
            };
            for &entry in &elements {
                verifier.verify(self, entry);
            }
            let config = verifier.map_config.unwrap();
            for (&duplicate, &original) in &config.duplicate_keys {
                let message =
                    first_value_message(self.unit, "The first key with this value.", original);
                self.report_at_node(
                    diag::equal_keys_in_const_map().with_context_messages([message]),
                    duplicate,
                );
            }
        }
    }

    fn visit_switch_expression(&mut self, ast: &Ast, node: Id<SwitchExpression>) {
        self.with_constant_pattern_values(
            ast,
            node.raw(),
            |this, map_key_values, constant_values| {
                this.validate_switch_exhaustiveness(
                    node.raw(),
                    map_key_values,
                    constant_values,
                    true,
                    true,
                );
            },
        );
    }

    fn visit_switch_statement(&mut self, ast: &Ast, node: Id<SwitchStatement>) {
        self.with_constant_pattern_values(
            ast,
            node.raw(),
            |this, map_key_values, constant_values| {
                if this.features.is_enabled("patterns") {
                    let scrutinee_type = this
                        .static_type(ast[node].expression)
                        .unwrap_or(TypeId::INVALID);
                    let must_be_exhaustive = this.ts().is_always_exhaustive(scrutinee_type);
                    this.validate_switch_exhaustiveness(
                        node.raw(),
                        map_key_values,
                        constant_values,
                        must_be_exhaustive,
                        false,
                    );
                } else {
                    this.validate_switch_statement_null_safety(ast, node);
                }
            },
        );
    }

    fn visit_variable_declaration(&mut self, ast: &Ast, node: Id<VariableDeclaration>) {
        ast.visit_children(node, self);
        if ast[node].initializer.is_none() {
            return;
        }
        let (is_const, is_final) = variable_declaration_keywords(ast, node);
        if !(is_const || is_final) {
            return;
        }
        let Some(element) = self.declared_element(node) else {
            return;
        };
        if element.tag() == Tag::Field && !member::is_static(&self.ctx, ElemRef::Base(element)) {
            let enclosing_element = self.ctx.element_data(element).and_then(|d| d.enclosing);
            if let Some(enclosing_element) = enclosing_element
                && enclosing_element.tag() == Tag::Class
                && !has_generative_const_constructor(&self.ctx, enclosing_element)
            {
                // We report errors in the class fields only if there's a
                // generative const constructor in the class.
                return;
            }
        }

        // Variables marked "const" should have had their values computed by
        // ConstantValueComputer. Other variables will only have had their
        // values computed if the value was needed (e.g. final variables in a
        // class containing const constructors).
        let Some(result) = self.engine.evaluation_result(element) else {
            return;
        };
        if let Constant::Invalid(result) = result {
            if is_const {
                self.report_error(
                    &result,
                    Some(&diag::CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE),
                );
            } else {
                self.report_error(&result, None);
            }
        }
    }
}

impl ConstantVerifier<'_, '_> {
    /// Type argument [index] of the static type of the literal [node] (Dart
    /// `(node.staticType as InterfaceTypeImpl).typeArguments[index]`);
    /// `None` when the type is not an interface type with that argument.
    fn type_argument(&self, node: NodeId, index: usize) -> Option<TypeId> {
        let node_type = self.static_type(node)?;
        match *self.ctx.ty(node_type) {
            TypeKind::Interface { args, .. } => self.ctx.list(args).get(index).copied(),
            _ => None,
        }
    }
}

/// Dart `VariableDeclaration.isConst` / `isFinal` (the keyword of the
/// enclosing `VariableDeclarationList`).
fn variable_declaration_keywords(ast: &Ast, node: Id<VariableDeclaration>) -> (bool, bool) {
    let Some(list) = ast
        .parent(node)
        .and_then(|p| ast.cast::<VariableDeclarationList>(p))
    else {
        return (false, false);
    };
    let keyword = ast[list].keyword;
    (
        ast_ext::is_keyword(ast, keyword, "const"),
        ast_ext::is_keyword(ast, keyword, "final"),
    )
}

/// Dart `PrimaryConstructorBodyImpl.declaration`.
fn primary_constructor_body_declaration(
    ast: &Ast,
    node: Id<PrimaryConstructorBody>,
) -> Option<Id<PrimaryConstructorDeclaration>> {
    let declaration = ast.parent(node).and_then(|body| ast.parent(body))?;
    let name_part = match ast.kind(declaration) {
        NodeKind::ClassDeclaration => ast[Id::<ClassDeclaration>::from_raw(declaration)].name_part,
        NodeKind::EnumDeclaration => ast[Id::<EnumDeclaration>::from_raw(declaration)].name_part,
        NodeKind::ExtensionTypeDeclaration => {
            ast[Id::<ExtensionTypeDeclaration>::from_raw(declaration)].name_part
        }
        _ => return None,
    };
    ast.cast::<PrimaryConstructorDeclaration>(name_part)
}

// ---------------------------------------------------------------------------
// _ConstLiteralVerifier
// ---------------------------------------------------------------------------

/// Dart `Map<DartObject, Expression>`: keys by `DartObjectImpl ==`, in
/// insertion order.
#[derive(Default)]
struct ObjectMap {
    entries: Vec<(DartObjectImpl, NodeId)>,
    index: IndexMap<u64, Vec<usize>>,
}

impl ObjectMap {
    fn get(&self, ts: &TypeSystem<'_>, key: &DartObjectImpl) -> Option<NodeId> {
        self.index
            .get(&key.dart_hash())?
            .iter()
            .map(|&i| &self.entries[i])
            .find(|(k, _)| k.dart_eq(key, ts))
            .map(|&(_, n)| n)
    }

    /// Dart `map[key] = value` for a key that is not in the map.
    fn insert_new(&mut self, key: DartObjectImpl, value: NodeId) {
        let i = self.entries.len();
        self.index.entry(key.dart_hash()).or_default().push(i);
        self.entries.push((key, value));
    }
}

/// Dart `_MapVerifierConfig`.
struct MapVerifierConfig {
    key_type: TypeId,
    value_type: TypeId,
    unique_keys: ObjectMap,
    duplicate_keys: IndexMap<NodeId, NodeId>,
}

/// Dart `_SetVerifierConfig`.
struct SetVerifierConfig {
    element_type: TypeId,
    unique_values: ObjectMap,
    duplicate_elements: IndexMap<NodeId, NodeId>,
}

/// Dart `_ConstLiteralVerifier`.
struct ConstLiteralVerifier {
    diagnostic_code: &'static DiagnosticCode,
    list_element_type: Option<TypeId>,
    set_config: Option<SetVerifierConfig>,
    map_config: Option<MapVerifierConfig>,
}

impl ConstLiteralVerifier {
    fn verify(&mut self, verifier: &ConstantVerifier<'_, '_>, element: NodeId) -> bool {
        let ast = verifier.ast();
        let kind = ast.kind(element);
        if Expression::test(kind) {
            let value = verifier.evaluate_and_report_error(element, self.diagnostic_code);
            let Constant::Value(value) = value else {
                return false;
            };

            if let Some(list_element_type) = self.list_element_type {
                return self.validate_list_expression(verifier, list_element_type, element, &value);
            }

            if self.set_config.is_some() {
                return self.validate_set_expression(verifier, element, &value);
            }

            true
        } else if kind == NodeKind::ForElement {
            verifier.report_at_node(diag::const_eval_for_element(), element);
            false
        } else if let Some(element) = ast.cast::<IfElement>(element) {
            let condition_constant = verifier
                .evaluate_and_report_error(ast[element].expression.raw(), self.diagnostic_code);
            let Constant::Value(condition_constant) = condition_constant else {
                return false;
            };

            // The errors have already been reported.
            if !condition_constant.is_bool() {
                return false;
            }

            let condition_value = condition_constant.to_bool_value();

            let mut else_valid = true;
            let then_element = ast[element].then_element.raw();
            let else_element = ast[element].else_element.map(|e| e.raw());

            let Some(condition_value) = condition_value else {
                let then_valid = self.report_not_potential_constants(verifier, then_element);
                if let Some(else_element) = else_element {
                    else_valid = self.report_not_potential_constants(verifier, else_element);
                }
                return then_valid && else_valid;
            };

            // Only validate the relevant branch as per `conditionValue`. This
            // avoids issues like duplicate values showing up in a const set,
            // when they occur in each branch, like
            // `{if (x) ...[1] else [1, 2]}`.
            let then_valid = if condition_value {
                let then_valid = self.verify(verifier, then_element);
                if let Some(else_element) = else_element {
                    else_valid = self.report_not_potential_constants(verifier, else_element);
                }
                then_valid
            } else {
                let then_valid = self.report_not_potential_constants(verifier, then_element);
                if let Some(else_element) = else_element {
                    else_valid = self.verify(verifier, else_element);
                }
                then_valid
            };

            then_valid && else_valid
        } else if let Some(entry) = ast.cast::<MapLiteralEntry>(element) {
            self.validate_map_literal_entry(verifier, entry)
        } else if let Some(spread) = ast.cast::<SpreadElement>(element) {
            let value = verifier
                .evaluate_and_report_error(ast[spread].expression.raw(), self.diagnostic_code);
            let Constant::Value(value) = value else {
                return false;
            };

            if self.list_element_type.is_some() || self.set_config.is_some() {
                return self.validate_list_or_set_spread(verifier, spread, &value);
            }

            if self.map_config.is_some() {
                return self.validate_map_spread(verifier, spread, &value);
            }

            true
        } else if let Some(null_aware) = ast.cast::<NullAwareElement>(element) {
            let value_node = ast[null_aware].value.raw();
            let value = verifier.evaluate_and_report_error(value_node, self.diagnostic_code);
            let Constant::Value(value) = value else {
                return false;
            };

            if let Some(list_element_type) = self.list_element_type {
                let nullable = verifier.ts().make_nullable(list_element_type);
                return self.validate_list_expression(verifier, nullable, value_node, &value);
            }

            // If the value is `null`, skip verifying it with the set, as it
            // won't be added as an element.
            if self.set_config.is_some() && !verifier.ctx.is_dart_core_null(value.ty) {
                return self.validate_set_expression(verifier, value_node, &value);
            }

            true
        } else {
            // Dart throws `UnsupportedError` for an unhandled collection
            // element.
            false
        }
    }

    /// Returns whether the [node] is a potential constant.
    fn report_not_potential_constants(
        &self,
        verifier: &ConstantVerifier<'_, '_>,
        node: NodeId,
    ) -> bool {
        let not_potentially_constants = verifier.not_potentially_constants(node);
        if not_potentially_constants.is_empty() {
            return true;
        }

        let ast = verifier.ast();
        for not_const in not_potentially_constants {
            let diagnostic_code = if self.list_element_type.is_some() {
                diag::non_constant_list_element()
            } else if self.map_config.is_some() {
                let mut diagnostic_code = diag::non_constant_map_element();
                let mut parent = Some(not_const);
                while let Some(p) = parent {
                    if let Some(entry) = ast.cast::<MapLiteralEntry>(p) {
                        if ast[entry].key.raw() == not_const {
                            diagnostic_code = diag::non_constant_map_key();
                        } else {
                            diagnostic_code = diag::non_constant_map_value();
                        }
                        break;
                    }
                    parent = ast.parent(p);
                }
                diagnostic_code
            } else if self.set_config.is_some() {
                diag::non_constant_set_element()
            } else {
                // Dart throws `UnimplementedError`.
                continue;
            };
            verifier.report_at_node(diagnostic_code, not_const);
        }

        false
    }

    fn validate_list_expression(
        &self,
        verifier: &ConstantVerifier<'_, '_>,
        list_element_type: TypeId,
        expression: NodeId,
        value: &DartObjectImpl,
    ) -> bool {
        if !verifier.runtime_type_match(value, list_element_type) {
            let ctx = &verifier.ctx;
            if verifier.runtime_type_match(value, verifier.ts().make_nullable(list_element_type)) {
                verifier.report_at_node(
                    diag::list_element_type_not_assignable_nullability(
                        type_arg(ctx, value.ty),
                        type_arg(ctx, list_element_type),
                    ),
                    expression,
                );
            } else {
                verifier.report_at_node(
                    diag::list_element_type_not_assignable(
                        type_arg(ctx, value.ty),
                        type_arg(ctx, list_element_type),
                    ),
                    expression,
                );
            }
            return false;
        }

        true
    }

    fn validate_list_or_set_spread(
        &mut self,
        verifier: &ConstantVerifier<'_, '_>,
        element: Id<SpreadElement>,
        value: &DartObjectImpl,
    ) -> bool {
        let ast = verifier.ast();
        let list_value = value.to_list_value();
        let set_value = value.to_set_value();
        let iterable_value: Option<Vec<&DartObjectImpl>> = list_value
            .map(|l| l.iter().collect())
            .or_else(|| set_value.map(|s| s.iter().collect()));

        let Some(iterable_value) = iterable_value else {
            if value.is_null() && spread_is_null_aware(ast, element) {
                return true;
            }
            verifier.report_at_node(
                diag::const_spread_expected_list_or_set(),
                ast[element].expression,
            );
            return false;
        };

        let Some(set_config) = &mut self.set_config else {
            return true;
        };

        let ts = verifier.ts();
        if let Some(list_value) = list_value
            && !list_value
                .iter()
                .all(|e| e.has_primitive_equality(&ts, verifier.features))
        {
            verifier.report_at_node(
                diag::const_set_element_not_primitive_equality(type_arg(&verifier.ctx, value.ty)),
                element,
            );
            return false;
        }

        for item in iterable_value {
            let expression = ast[element].expression.raw();
            match set_config.unique_values.get(&ts, item) {
                Some(existing_value) => {
                    set_config
                        .duplicate_elements
                        .insert(expression, existing_value);
                }
                None => set_config
                    .unique_values
                    .insert_new(item.clone(), expression),
            }
        }

        true
    }

    fn validate_map_literal_entry(
        &mut self,
        verifier: &ConstantVerifier<'_, '_>,
        entry: Id<MapLiteralEntry>,
    ) -> bool {
        let Some(config) = &mut self.map_config else {
            return false;
        };
        let ast = verifier.ast();
        let ctx = &verifier.ctx;
        let ts = verifier.ts();

        let key_expression = ast[entry].key.raw();
        let value_expression = ast[entry].value.raw();

        let is_key_null_aware = ast[entry].key_question.is_some();
        let is_value_null_aware = ast[entry].value_question.is_some();

        let key_value =
            verifier.evaluate_and_report_error(key_expression, &diag::NON_CONSTANT_MAP_KEY);
        let value_value =
            verifier.evaluate_and_report_error(value_expression, &diag::NON_CONSTANT_MAP_VALUE);

        if let Constant::Value(key_value) = &key_value {
            let key_type = key_value.ty;
            let mut expected_key_type = config.key_type;
            if is_key_null_aware {
                expected_key_type = ts.make_nullable(expected_key_type);
            }

            if !verifier.runtime_type_match(key_value, expected_key_type) {
                if !is_key_null_aware
                    && verifier.runtime_type_match(key_value, ts.make_nullable(expected_key_type))
                {
                    verifier.report_at_node(
                        diag::map_key_type_not_assignable_nullability(
                            type_arg(ctx, key_type),
                            type_arg(ctx, expected_key_type),
                        ),
                        key_expression,
                    );
                } else {
                    verifier.report_at_node(
                        diag::map_key_type_not_assignable(
                            type_arg(ctx, key_type),
                            type_arg(ctx, expected_key_type),
                        ),
                        key_expression,
                    );
                }
            }

            if !key_value.has_primitive_equality(&ts, verifier.features) {
                verifier.report_at_node(
                    diag::const_map_key_not_primitive_equality(type_arg(ctx, key_type)),
                    key_expression,
                );
            }

            // Don't check the key for uniqueness if the key is null aware and
            // is `null` or the value is null aware and is `null`, since it
            // won't be added to the map in that case.
            let value_not_null = matches!(
                &value_value,
                Constant::Value(v) if !ctx.is_dart_core_null(v.ty)
            );
            if (!is_key_null_aware || !ctx.is_dart_core_null(key_value.ty))
                && (!is_value_null_aware || value_not_null)
            {
                match config.unique_keys.get(&ts, key_value) {
                    Some(existing_key) => {
                        config.duplicate_keys.insert(key_expression, existing_key);
                    }
                    None => config
                        .unique_keys
                        .insert_new(key_value.clone(), key_expression),
                }
            }
        }

        let mut expected_value_type = config.value_type;
        if is_value_null_aware {
            expected_value_type = ts.make_nullable(expected_value_type);
        }
        if let Constant::Value(value_value) = &value_value
            && !verifier.runtime_type_match(value_value, expected_value_type)
        {
            if !is_value_null_aware
                && verifier.runtime_type_match(value_value, ts.make_nullable(expected_value_type))
            {
                verifier.report_at_node(
                    diag::map_value_type_not_assignable_nullability(
                        type_arg(ctx, value_value.ty),
                        type_arg(ctx, expected_value_type),
                    ),
                    value_expression,
                );
            } else {
                verifier.report_at_node(
                    diag::map_value_type_not_assignable(
                        type_arg(ctx, value_value.ty),
                        type_arg(ctx, expected_value_type),
                    ),
                    value_expression,
                );
            }
        }

        true
    }

    fn validate_map_spread(
        &mut self,
        verifier: &ConstantVerifier<'_, '_>,
        element: Id<SpreadElement>,
        value: &DartObjectImpl,
    ) -> bool {
        let ast = verifier.ast();
        if value.is_null() && spread_is_null_aware(ast, element) {
            return true;
        }
        let Some(config) = &mut self.map_config else {
            return true;
        };
        let ts = verifier.ts();
        if let Some(map) = value.to_map_value() {
            // TODO(brianwilkerson): Figure out how to improve the error
            //  messages. They currently point to the whole spread
            //  expression, but the key and/or value being referenced might
            //  not be located there (if it's referenced through a const
            //  variable).
            let expression = ast[element].expression.raw();
            for key_value in map.keys() {
                match config.unique_keys.get(&ts, key_value) {
                    Some(existing_key) => {
                        config.duplicate_keys.insert(expression, existing_key);
                    }
                    None => config.unique_keys.insert_new(key_value.clone(), expression),
                }
            }
            return true;
        }
        verifier.report_at_node(diag::const_spread_expected_map(), ast[element].expression);
        false
    }

    fn validate_set_expression(
        &mut self,
        verifier: &ConstantVerifier<'_, '_>,
        expression: NodeId,
        value: &DartObjectImpl,
    ) -> bool {
        let Some(config) = &mut self.set_config else {
            return true;
        };
        let ctx = &verifier.ctx;
        let ts = verifier.ts();
        if !verifier.runtime_type_match(value, config.element_type) {
            if verifier.runtime_type_match(value, ts.make_nullable(config.element_type)) {
                verifier.report_at_node(
                    diag::set_element_type_not_assignable_nullability(
                        type_arg(ctx, value.ty),
                        type_arg(ctx, config.element_type),
                    ),
                    expression,
                );
            } else {
                verifier.report_at_node(
                    diag::set_element_type_not_assignable(
                        type_arg(ctx, value.ty),
                        type_arg(ctx, config.element_type),
                    ),
                    expression,
                );
            }
            return false;
        }

        if !value.has_primitive_equality(&ts, verifier.features) {
            verifier.report_at_node(
                diag::const_set_element_not_primitive_equality(type_arg(ctx, value.ty)),
                expression,
            );
            return false;
        }

        match config.unique_values.get(&ts, value) {
            Some(existing_value) => {
                config.duplicate_elements.insert(expression, existing_value);
            }
            None => config.unique_values.insert_new(value.clone(), expression),
        }

        true
    }
}

/// Dart `SpreadElement.isNullAware`.
fn spread_is_null_aware(ast: &Ast, element: Id<SpreadElement>) -> bool {
    ast.tokens.ty(ast[element].spread_operator) == TokenType::PERIOD_PERIOD_PERIOD_QUESTION
}
