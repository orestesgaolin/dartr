// Dart source: pkg/analyzer/lib/src/generated/resolver.dart (the
// `visitX(node, {contextType})` methods of the expressions,
// insertGenericFunctionInstantiation, _insertImplicitCallReference,
// _shouldSkipImplicitCallReferenceDueToForm)

//! The expression visitors of [`ResolverVisitor`]. The core expressions are
//! ported here; every other expression kind is delegated to the module of
//! its resolver file (stubs until units C3–C9 port them).

use dartr_ast::*;
use dartr_element::{TypeId, TypeKind};
use dartr_flow::flow_analysis::FlowAnalysis;
use dartr_flow::shared_type::{SharedTypeSchemaView, SharedTypeView};
use dartr_flow::type_analyzer::TypeAnalyzer;
use dartr_syntax::TokenType;

use crate::resolver::ResolverVisitor;
use crate::{
    assignment_expression_resolver, binary_expression_resolver, constructor_reference_resolver,
    dot_shorthand_resolver, extension_member_resolver, function_expression_invocation_resolver,
    function_expression_resolver, function_reference_resolver,
    instance_creation_expression_resolver, method_invocation_resolver, pattern_resolver,
    postfix_expression_resolver, prefix_expression_resolver, prefixed_identifier_resolver,
    property_element_resolver, record_literal_resolver, simple_identifier_resolver,
    static_type_analyzer, typed_literal_resolver,
};

impl<'a> ResolverVisitor<'a> {
    // ------------------------------------------------------------ core

    pub fn visit_adjacent_strings(&mut self, node: Id<AdjacentStrings>, _context_type: TypeId) {
        self.check_unreachable_node(node);
        let strings = self.ast.list(self.ast[node].strings).to_vec();
        for string in strings {
            self.resolve_expression(string.upcast(), TypeId::UNKNOWN);
        }
        static_type_analyzer::visit_adjacent_strings(self, node);
    }

    pub fn visit_as_expression(&mut self, node: Id<AsExpression>, context_type: TypeId) {
        self.check_unreachable_node(node);
        let expression = self.ast[node].expression;
        self.resolve_expression(expression, TypeId::UNKNOWN);
        let ty = self.ast[node].type_;
        self.check_unreachable_node(ty);
        self.visit_node(ty.raw());
        static_type_analyzer::visit_as_expression(self, node);
        {
            let ast = &*self.ast;
            self.flow_analysis.as_expression(ast, self.tables, node);
        }
        let e = self.insert_generic_function_instantiation(node.upcast(), context_type);
        self.insert_implicit_call_reference(e, context_type);
        // Dart: `castFromNullableAlwaysFails` (a diagnostic of wave D).
    }

    pub fn visit_await_expression(&mut self, node: Id<AwaitExpression>, context_type: TypeId) {
        self.check_unreachable_node(node);
        let expression = self.ast[node].expression;
        let result =
            self.analyze_await_expression(node.upcast(), expression, SharedTypeSchemaView::new(context_type));
        self.pop_rewrite();
        self.record_static_type(node, result.type_.unwrap_type_view());
        let e = self.insert_generic_function_instantiation(node.upcast(), context_type);
        self.insert_implicit_call_reference(e, context_type);
    }

    pub fn visit_boolean_literal(&mut self, node: Id<BooleanLiteral>, _context_type: TypeId) {
        let value = self.ast[node].value;
        let info = self.flow_analysis.flow.as_mut().map(|f| f.boolean_literal(value));
        self.flow_analysis.store_expression_info(node.upcast(), info);
        self.check_unreachable_node(node);
        static_type_analyzer::visit_boolean_literal(self, node);
    }

    pub fn visit_cascade_expression(&mut self, node: Id<CascadeExpression>, context_type: TypeId) {
        self.check_unreachable_node(node);
        let target = self.ast[node].target;
        let target = self.resolve_expression(target, context_type);
        let target_type = self.static_type(target).unwrap_or(TypeId::DYNAMIC);
        let is_null_aware = self.cascade_is_null_aware(node);
        let target_info = self.flow_analysis.get_expression_info(Some(target));
        self.flow()
            .cascade_expression_after_target(target_info, SharedTypeView::new(target_type), is_null_aware, None);
        let sections = self.ast.list(self.ast[node].cascade_sections).to_vec();
        for section in sections {
            self.resolve_expression(section, TypeId::UNKNOWN);
        }
        static_type_analyzer::visit_cascade_expression(self, node);
        if is_null_aware {
            dartr_flow::flow_analysis::FlowAnalysisNullShortingInterface::null_aware_access_end(self.flow());
        }
        let info = self.flow().cascade_expression_end();
        self.flow_analysis.store_expression_info(node.upcast(), Some(info));
        self.insert_implicit_call_reference(node.upcast(), context_type);
    }

    /// Dart `CascadeExpression.isNullAware`: the first section starts with
    /// `?..`.
    fn cascade_is_null_aware(&self, node: Id<CascadeExpression>) -> bool {
        let sections = self.ast.list(self.ast[node].cascade_sections);
        let Some(&first) = sections.first() else {
            return false;
        };
        // The `..` / `?..` token is the begin token of the first section.
        let begin = self.ast.begin_token(first.raw());
        self.ast.tokens.ty(begin) == TokenType::QUESTION_PERIOD_PERIOD
    }

    pub fn visit_conditional_expression(&mut self, node: Id<ConditionalExpression>, context_type: TypeId) {
        self.check_unreachable_node(node);
        let flow_active = self.flow_analysis.is_active();
        if flow_active {
            self.flow().conditional_condition_begin();
        }
        let condition = self.ast[node].condition;
        let bool_type = self.ctx.tp.bool_type();
        let condition = self.resolve_expression(condition, bool_type);
        self.check_for_non_bool_condition(condition);

        if flow_active {
            let info = self.flow_analysis.get_expression_info(Some(condition));
            self.flow().conditional_then_begin(info, node.raw());
            let then = self.ast[node].then_expression;
            self.check_unreachable_node(then);
        }
        let then_expression = self.ast[node].then_expression;
        let then_expression = self.resolve_expression(then_expression, context_type);

        let else_expression = self.ast[node].else_expression;
        if flow_active {
            let info = self.flow_analysis.get_expression_info(Some(then_expression));
            let then_type = self.type_or_throw(then_expression);
            self.flow().conditional_else_begin(info, SharedTypeView::new(then_type));
            self.check_unreachable_node(else_expression);
        }
        let else_expression = self.resolve_expression(else_expression, context_type);

        static_type_analyzer::visit_conditional_expression(self, node, context_type);
        if flow_active {
            let ty = self.type_or_throw(node);
            let info = self.flow_analysis.get_expression_info(Some(else_expression));
            let else_type = self.type_or_throw(else_expression);
            let result = self.flow().conditional_end(
                SharedTypeView::new(ty),
                info,
                SharedTypeView::new(else_type),
            );
            self.flow_analysis.store_expression_info(node.upcast(), Some(result));
        }
        self.insert_implicit_call_reference(node.upcast(), context_type);
    }

    pub fn visit_double_literal(&mut self, node: Id<DoubleLiteral>, _context_type: TypeId) {
        self.check_unreachable_node(node);
        static_type_analyzer::visit_double_literal(self, node);
    }

    pub fn visit_integer_literal(&mut self, node: Id<IntegerLiteral>, context_type: TypeId) {
        self.check_unreachable_node(node);
        static_type_analyzer::visit_integer_literal(self, node, context_type);
    }

    pub fn visit_is_expression(&mut self, node: Id<IsExpression>, _context_type: TypeId) {
        self.check_unreachable_node(node);
        let expression = self.ast[node].expression;
        self.resolve_expression(expression, TypeId::UNKNOWN);
        let ty = self.ast[node].type_;
        self.check_unreachable_node(ty);
        self.visit_node(ty.raw());
        static_type_analyzer::visit_is_expression(self, node);
        let ast = &*self.ast;
        self.flow_analysis.is_expression(ast, self.tables, node);
    }

    pub fn visit_null_literal(&mut self, node: Id<NullLiteral>, _context_type: TypeId) {
        let null_type = self.ctx.tp.null_type();
        let info = self
            .flow_analysis
            .flow
            .as_mut()
            .map(|f| f.null_literal(SharedTypeView::new(null_type)));
        self.flow_analysis.store_expression_info(node.upcast(), info);
        self.check_unreachable_node(node);
        static_type_analyzer::visit_null_literal(self, node);
    }

    pub fn visit_parenthesized_expression(&mut self, node: Id<ParenthesizedExpression>, context_type: TypeId) {
        self.check_unreachable_node(node);
        let expression = self.ast[node].expression;
        let expression = self.resolve_expression(expression, context_type);
        static_type_analyzer::visit_parenthesized_expression(self, node);
        let info = self.flow_analysis.get_expression_info(Some(expression));
        let result = self
            .flow_analysis
            .flow
            .as_mut()
            .and_then(|f| f.parenthesized_expression(info));
        self.flow_analysis.store_expression_info(node.upcast(), result);
    }

    pub fn visit_rethrow_expression(&mut self, node: Id<RethrowExpression>, _context_type: TypeId) {
        self.check_unreachable_node(node);
        static_type_analyzer::visit_rethrow_expression(self, node);
        if let Some(flow) = self.flow_analysis.flow.as_mut() {
            flow.handle_exit();
        }
    }

    pub fn visit_simple_string_literal(&mut self, node: Id<SimpleStringLiteral>, _context_type: TypeId) {
        self.check_unreachable_node(node);
        static_type_analyzer::visit_simple_string_literal(self, node);
    }

    pub fn visit_string_interpolation(&mut self, node: Id<StringInterpolation>, _context_type: TypeId) {
        self.check_unreachable_node(node);
        self.visit_children(node);
        static_type_analyzer::visit_string_interpolation(self, node);
    }

    pub fn visit_super_expression(&mut self, node: Id<SuperExpression>, _context_type: TypeId) {
        self.check_unreachable_node(node);
        // Dart `elementResolver.visitSuperExpression`: reports
        // `superInExtension` / `superInExtensionType` / `superInInvalidContext`
        // (wave D).
        static_type_analyzer::visit_super_expression(self, node);
    }

    pub fn visit_symbol_literal(&mut self, node: Id<SymbolLiteral>, _context_type: TypeId) {
        self.check_unreachable_node(node);
        static_type_analyzer::visit_symbol_literal(self, node);
    }

    pub fn visit_this_expression(&mut self, node: Id<ThisExpression>, context_type: TypeId) {
        self.check_unreachable_node(node);
        static_type_analyzer::visit_this_expression(self, node);
        self.insert_implicit_call_reference(node.upcast(), context_type);
    }

    pub fn visit_throw_expression(&mut self, node: Id<ThrowExpression>, _context_type: TypeId) {
        self.check_unreachable_node(node);
        let expression = self.ast[node].expression;
        let object_type = self.ctx.tp.object_type();
        self.resolve_expression(expression, object_type);
        static_type_analyzer::visit_throw_expression(self, node);
        if let Some(flow) = self.flow_analysis.flow.as_mut() {
            flow.handle_exit();
        }
    }

    pub fn visit_type_literal(&mut self, node: Id<TypeLiteral>, _context_type: TypeId) {
        self.check_unreachable_node(node);
        self.visit_children(node);
        let type_type = self.ctx.tp.type_type();
        self.record_static_type(node, type_type);
    }

    pub fn visit_simple_identifier(&mut self, node: Id<SimpleIdentifier>, context_type: TypeId) {
        simple_identifier_resolver::visit_simple_identifier(self, node, context_type);
        // The identifier may be rewritten; continue with the node in its
        // place.
        let current = self.peek_rewrite().unwrap_or(node.upcast());
        let e = self.insert_generic_function_instantiation(current, context_type);
        self.insert_implicit_call_reference(e, context_type);
    }

    pub fn visit_function_expression(&mut self, node: Id<FunctionExpression>, context_type: TypeId) {
        function_expression_resolver::visit_function_expression(self, node, context_type);
    }

    // ------------------------------------------------------------ delegated

    pub fn visit_anonymous_method_invocation(&mut self, node: Id<AnonymousMethodInvocation>, context_type: TypeId) {
        function_expression_resolver::visit_anonymous_method_invocation(self, node, context_type);
    }

    pub fn visit_assignment_expression(&mut self, node: Id<AssignmentExpression>, context_type: TypeId) {
        self.check_unreachable_node(node);
        assignment_expression_resolver::visit_assignment_expression(self, node, context_type);
    }

    pub fn visit_binary_expression(&mut self, node: Id<BinaryExpression>, context_type: TypeId) {
        self.check_unreachable_node(node);
        binary_expression_resolver::visit_binary_expression(self, node, context_type);
    }

    pub fn visit_constructor_reference(&mut self, node: Id<ConstructorReference>, context_type: TypeId) {
        constructor_reference_resolver::visit_constructor_reference(self, node, context_type);
    }

    pub fn visit_dot_shorthand_constructor_invocation(
        &mut self,
        node: Id<DotShorthandConstructorInvocation>,
        context_type: TypeId,
    ) {
        dot_shorthand_resolver::visit_dot_shorthand_constructor_invocation(self, node, context_type);
    }

    pub fn visit_dot_shorthand_invocation(&mut self, node: Id<DotShorthandInvocation>, context_type: TypeId) {
        dot_shorthand_resolver::visit_dot_shorthand_invocation(self, node, context_type);
    }

    pub fn visit_dot_shorthand_property_access(
        &mut self,
        node: Id<DotShorthandPropertyAccess>,
        context_type: TypeId,
    ) {
        dot_shorthand_resolver::visit_dot_shorthand_property_access(self, node, context_type);
    }

    pub fn visit_extension_override(&mut self, node: Id<ExtensionOverride>, context_type: TypeId) {
        extension_member_resolver::visit_extension_override(self, node, context_type);
    }

    pub fn visit_function_expression_invocation(
        &mut self,
        node: Id<FunctionExpressionInvocation>,
        context_type: TypeId,
    ) {
        function_expression_invocation_resolver::visit_function_expression_invocation(self, node, context_type);
    }

    pub fn visit_function_reference(&mut self, node: Id<FunctionReference>, context_type: TypeId) {
        function_reference_resolver::visit_function_reference(self, node, context_type);
    }

    pub fn visit_implicit_call_reference(&mut self, node: Id<ImplicitCallReference>, context_type: TypeId) {
        function_reference_resolver::visit_implicit_call_reference(self, node, context_type);
    }

    pub fn visit_index_expression(&mut self, node: Id<IndexExpression>, context_type: TypeId) {
        property_element_resolver::visit_index_expression(self, node, context_type);
    }

    pub fn visit_instance_creation_expression(
        &mut self,
        node: Id<InstanceCreationExpression>,
        context_type: TypeId,
    ) {
        instance_creation_expression_resolver::visit_instance_creation_expression(self, node, context_type);
    }

    pub fn visit_list_literal(&mut self, node: Id<ListLiteral>, context_type: TypeId) {
        typed_literal_resolver::visit_list_literal(self, node, context_type);
    }

    pub fn visit_method_invocation(&mut self, node: Id<MethodInvocation>, context_type: TypeId) {
        method_invocation_resolver::visit_method_invocation(self, node, context_type);
    }

    pub fn visit_pattern_assignment(&mut self, node: Id<PatternAssignment>, context_type: TypeId) {
        let _ = context_type;
        // STUB (patterns): fallback.
        let _ = pattern_resolver::handle_switch_before_alternative;
        self.fallback_expression(node.upcast());
    }

    pub fn visit_postfix_expression(&mut self, node: Id<PostfixExpression>, context_type: TypeId) {
        postfix_expression_resolver::visit_postfix_expression(self, node, context_type);
    }

    pub fn visit_prefixed_identifier(&mut self, node: Id<PrefixedIdentifier>, context_type: TypeId) {
        prefixed_identifier_resolver::visit_prefixed_identifier(self, node, context_type);
    }

    pub fn visit_prefix_expression(&mut self, node: Id<PrefixExpression>, context_type: TypeId) {
        prefix_expression_resolver::visit_prefix_expression(self, node, context_type);
    }

    pub fn visit_property_access(&mut self, node: Id<PropertyAccess>, context_type: TypeId) {
        property_element_resolver::visit_property_access(self, node, context_type);
    }

    pub fn visit_record_literal(&mut self, node: Id<RecordLiteral>, context_type: TypeId) {
        record_literal_resolver::visit_record_literal(self, node, context_type);
    }

    pub fn visit_set_or_map_literal(&mut self, node: Id<SetOrMapLiteral>, context_type: TypeId) {
        typed_literal_resolver::visit_set_or_map_literal(self, node, context_type);
    }

    pub fn visit_switch_expression(&mut self, node: Id<SwitchExpression>, context_type: TypeId) {
        let _ = context_type;
        // STUB (patterns): fallback.
        self.fallback_expression(node.upcast());
    }

    // ------------------------------------------------------------ helpers

    /// Dart `insertGenericFunctionInstantiation(expression, contextType:)`:
    /// if generic function instantiation applies to [expression], wraps it
    /// in a `FunctionReference` and returns that node; otherwise returns
    /// [expression].
    pub fn insert_generic_function_instantiation(
        &mut self,
        expression: Id<Expression>,
        context_type: TypeId,
    ) -> Id<Expression> {
        if !self.is_constructor_tearoffs_enabled() {
            // Temporarily, only create `ImplicitCallReference` nodes under
            // the 'constructor-tearoffs' feature.
            return expression;
        }
        let Some(parent) = self.ast.parent(expression) else {
            return expression;
        };
        // Don't rewrite function declarations.
        if self.ast.is::<FunctionDeclaration>(parent) {
            return expression;
        }
        let Some(static_type) = self.static_type(expression) else {
            return expression;
        };
        let TypeKind::Function(f) = *self.ctx.ty(static_type) else {
            return expression;
        };
        if self.ctx.list(f.type_params).is_empty() {
            return expression;
        }
        let context = self.type_system.flatten(context_type);
        match *self.ctx.ty(context) {
            TypeKind::Function(cf) if self.ctx.list(cf.type_params).is_empty() => {}
            _ => return expression,
        }
        let type_argument_types = self.infer_function_type_instantiation(context, static_type, expression);
        let mut static_type = static_type;
        if !type_argument_types.is_empty() {
            use dartr_typesystem::TypeExt;
            static_type = self.ctx.instantiate_function_type(static_type, &type_argument_types);
        }
        let reference = self.ast.add(FunctionReference {
            function: expression,
            type_arguments: None,
        });
        self.replace_expression(expression, reference.upcast(), Some(parent));
        let list = self.ctx.intern_list(&type_argument_types);
        self.tables.type_arg_types.insert(reference, list);
        self.set_static_type(reference, static_type);
        reference.upcast()
    }

    /// Dart `typeSystem.inferFunctionTypeInstantiation(context, fnType,
    /// diagnosticReporter:, errorNode:, ...)` with the resolver's options.
    pub fn infer_function_type_instantiation(
        &mut self,
        context: TypeId,
        fn_type: TypeId,
        error_node: Id<Expression>,
    ) -> Vec<TypeId> {
        use dartr_typesystem::generic_inferrer::{InferenceErrorEntity, InferenceErrorEntityKind, InferenceFlags};
        let flags = InferenceFlags {
            // If the constructor-tearoffs feature is enabled, then so is
            // generic-metadata.
            generic_metadata_is_enabled: true,
            inference_using_bounds_is_enabled: self.inference_using_bounds_is_enabled(),
            strict_inference: self.unit.options.strict_inference,
        };
        let entity = InferenceErrorEntity {
            offset: self.ast.offset(error_node) as usize,
            length: self.ast.length(error_node) as usize,
            is_invocation_in_as_expression: false,
            kind: InferenceErrorEntityKind::Expression {
                static_type: self.static_type(error_node),
            },
        };
        let mut reported = Vec::new();
        let result = {
            let mut listener = |d: dartr_diagnostics::Diagnostic| reported.push(d);
            let mut reporter = dartr_diagnostics::DiagnosticReporter::new(&mut listener);
            self.type_system.infer_function_type_instantiation(
                context,
                fn_type,
                Some(&mut reporter),
                Some(entity),
                self.flow_analysis.type_operations,
                flags,
                None,
                Some(error_node.raw()),
            )
        };
        self.flush_type_analyzer_errors();
        if self.lock_level == 0 {
            self.diagnostics.extend(reported);
        }
        result
    }

    /// Dart `_insertImplicitCallReference(expression, contextType:)`: if
    /// [expression] should be treated as `expression.call`, wraps it in an
    /// `ImplicitCallReference`.
    pub fn insert_implicit_call_reference(&mut self, expression: Id<Expression>, context_type: TypeId) {
        let parent = self.ast.parent(expression);
        if self.should_skip_implicit_call_reference_due_to_form(expression, parent) {
            return;
        }
        if self.static_type(expression).is_none() {
            return;
        }
        let context = match parent.and_then(|p| self.ast.cast::<AssignmentExpression>(p)) {
            Some(assignment) => match self.tables.write_type.get(assignment) {
                Some(&t) => t,
                None => return,
            },
            None => context_type,
        };
        function_reference_resolver::insert_implicit_call_reference(self, expression, context);
    }

    /// Dart `_shouldSkipImplicitCallReferenceDueToForm`.
    fn should_skip_implicit_call_reference_due_to_form(
        &self,
        mut expression: Id<Expression>,
        mut parent: Option<NodeId>,
    ) -> bool {
        while let Some(p) = parent.and_then(|p| self.ast.cast::<ParenthesizedExpression>(p)) {
            expression = p.upcast();
            parent = self.ast.parent(p);
        }
        let Some(parent) = parent else {
            return false;
        };
        if let Some(c) = self.ast.cast::<CascadeExpression>(parent) {
            if self.ast[c].target == expression {
                // Do not perform an "implicit tear-off conversion" here. It
                // should only be performed on [parent]. See
                // https://github.com/dart-lang/language/issues/1873.
                return true;
            }
        }
        if let Some(c) = self.ast.cast::<ConditionalExpression>(parent) {
            if self.ast[c].then_expression == expression || self.ast[c].else_expression == expression {
                // Do not perform an "implicit tear-off conversion" on the
                // branches of a conditional expression.
                return true;
            }
        }
        if let Some(b) = self.ast.cast::<BinaryExpression>(parent) {
            if self.ast.tokens.ty(self.ast[b].operator) == TokenType::QUESTION_QUESTION {
                // Do not perform an "implicit tear-off conversion" on the
                // branches of a `??` operator.
                return true;
            }
        }
        false
    }
}
