// Dart source: pkg/analyzer/lib/src/generated/error_verifier.dart
// (ErrorVerifier section: statements and expressions (D5))

//! An `ErrorVerifier` section (see the module documentation of
//! [`super`]). The `visit_x` methods are the Dart `visitX` overrides; the
//! body `self.visit_children(node)` is Dart `super.visitX(node)`.
//!
//! Not ported: the context messages of `referencedBeforeDeclaration` and
//! of the short-circuiting null-aware operators (the diagnostics are
//! reported without them).

use dartr_ast::*;
use dartr_diagnostics::diag;
use dartr_element::diagnostics::type_arg;
use dartr_element::{
    EId, ElemRef, ElementId, ExecutableElement, FormalParameterElement, FragmentFlags,
    InterfaceElement, Tag, TypeAliasElement, TypeId, TypeKind, VariableElement,
};
use dartr_syntax::{TokenId, TokenType};
use dartr_typesystem::{TypeExt, member};
use indexmap::IndexSet;

use super::{EnclosingExecutableContext, ErrorVerifier, NullAwareKind, ThisContext};
use crate::ast_ext;
use crate::element_ext;
use crate::error::{
    async_return_visitor, const_argument_verifier,
    literal_element_verifier::LiteralElementVerifier, required_parameters_verifier,
    return_type_verifier, type_arguments_verifier, use_result_verifier,
};
use crate::error_detection_helpers::{ErrorDetectionHelpers, NonAssignabilityReporter};

impl ErrorVerifier<'_> {
    /// Dart `visitAnnotation`.
    pub(super) fn visit_annotation(&mut self, node: Id<Annotation>) {
        self.check_for_invalid_annotation_from_deferred_library(node);
        required_parameters_verifier::visit_annotation(self, node);
        self.visit_children(node);
    }

    /// Dart `visitAnonymousMethodInvocation`.
    pub(super) fn visit_anonymous_method_invocation(
        &mut self,
        node: Id<AnonymousMethodInvocation>,
    ) {
        let target = self.ast[node].target;
        if let Some(target) = target {
            self.check_for_use_of_void_result(target);
            self.accept(target);
        }
        const_argument_verifier::visit_anonymous_method_invocation(self, node);

        let parameters = self.ast[node].parameters;
        let body = self.ast[node].body;
        let visit_with_this_context = |this: &mut Self| {
            let visit_rest = |this: &mut Self| {
                this.accept_opt(parameters);
                this.accept(body);
            };
            if parameters.is_none() {
                this.with_this_context(ThisContext::InstanceMemberBody, visit_rest);
            } else {
                visit_rest(this);
            }
        };

        if self.ast.is::<AnonymousBlockBody>(body) {
            // Dart `fragment.returnType = returnType`: the element is not
            // changed here (the resolver owns it).
            let element = self.declared_element(node.raw());
            let context = EnclosingExecutableContext {
                element,
                is_asynchronous: false,
                is_generator: false,
                ..EnclosingExecutableContext::empty()
            };
            self.with_enclosing_executable(context, visit_with_this_context);
        } else {
            visit_with_this_context(self);
        }
    }

    /// Dart `visitAsExpression`.
    pub(super) fn visit_as_expression(&mut self, node: Id<AsExpression>) {
        let type_ = self.ast[node].type_;
        self.check_for_type_annotation_deferred_class(Some(type_));
        self.visit_children(node);
    }

    /// Dart `visitAssignedVariablePattern`.
    pub(super) fn visit_assigned_variable_pattern(&mut self, node: Id<AssignedVariablePattern>) {
        self.check_for_assignment_to_primary_constructor_parameter(node.raw());
        self.visit_children(node);
    }

    /// Dart `visitAssignmentExpression`.
    pub(super) fn visit_assignment_expression(&mut self, node: Id<AssignmentExpression>) {
        let operator_type = self.ast.tokens.ty(self.ast[node].operator);
        let lhs = self.ast[node].left_hand_side;
        if operator_type == TokenType::QUESTION_QUESTION_EQ
            && let Some(read_type) = self.tables.read_type.get(node).copied()
        {
            let rhs = self.ast[node].right_hand_side;
            self.check_for_dead_null_coalesce(read_type, rhs);
        }
        self.check_for_assignment_to_final(lhs);
        self.check_for_assignment_to_primary_constructor_parameter(lhs.raw());

        const_argument_verifier::visit_assignment_expression(self, node);
        self.visit_children(node);
    }

    /// Dart `visitAwaitExpression`.
    pub(super) fn visit_await_expression(&mut self, node: Id<AwaitExpression>) {
        let expression = self.ast[node].expression;
        self.check_for_use_of_void_result(expression);
        self.check_for_await_in_late_local_variable_initializer(node);
        self.check_for_await_of_incompatible_type(node);
        self.visit_children(node);
    }

    /// Dart `visitBinaryExpression`.
    pub(super) fn visit_binary_expression(&mut self, node: Id<BinaryExpression>) {
        let ty = self.ast.tokens.ty(self.ast[node].operator);
        let left_operand = self.ast[node].left_operand;
        let right_operand = self.ast[node].right_operand;
        if ty == TokenType::AMPERSAND_AMPERSAND || ty == TokenType::BAR_BAR {
            self.check_for_use_of_void_result(right_operand);
        } else {
            // Assignability checking is done by the resolver.
        }

        if ty == TokenType::QUESTION_QUESTION
            && let Some(left_type) = self.static_type(left_operand)
        {
            self.check_for_dead_null_coalesce(left_type, right_operand);
        }

        self.check_for_use_of_void_result(left_operand);
        const_argument_verifier::visit_binary_expression(self, node);

        self.visit_children(node);
    }

    /// Dart `visitBlock`.
    pub(super) fn visit_block(&mut self, node: Id<Block>) {
        let statements = self.ast.list(self.ast[node].statements).to_vec();
        self.with_hidden_elements_for_statements(&statements, |this| {
            let list = this.ast[node].statements;
            this.with_duplicate_definition_verifier(|d, this| d.check_statements(this, list));
            this.visit_children(node);
        });
    }

    /// Dart `visitBreakStatement`.
    pub(super) fn visit_break_statement(&mut self, node: Id<BreakStatement>) {
        if let Some(label_node) = self.ast[node].label {
            // Dart `labelElement is LabelElementImpl &&
            // labelElement.isOnSwitchMember`: the label is on a switch
            // member when the target of the statement is that member.
            let is_label = self
                .element(label_node)
                .is_some_and(|e| member::base_element(&self.ctx, e).tag() == Tag::Label);
            let on_switch_member = self
                .rt
                .break_continue_target
                .get(node)
                .is_some_and(|&t| self.ast.is::<SwitchMember>(t));
            if is_label && on_switch_member {
                self.report_at(diag::break_label_on_switch_member(), label_node);
            }
        }
    }

    /// Dart `visitCatchClause`.
    pub(super) fn visit_catch_clause(&mut self, node: Id<CatchClause>) {
        self.with_duplicate_definition_verifier(|d, this| d.check_catch_clause(this, node));
        self.enclosing_executable.catch_clause_level += 1;
        let exception_type = self.ast[node].exception_type;
        self.check_for_type_annotation_deferred_class(exception_type);
        self.visit_children(node);
        self.enclosing_executable.catch_clause_level -= 1;
    }

    /// Dart `visitConstructorReference`.
    pub(super) fn visit_constructor_reference(&mut self, node: Id<ConstructorReference>) {
        const_argument_verifier::visit_constructor_reference(self, node);
        type_arguments_verifier::check_constructor_reference(self, node);
        let constructor_name = self.ast[node].constructor_name;
        let element = self.element(constructor_name);
        self.check_for_invalid_generative_constructor_reference(constructor_name.raw(), element);
    }

    /// Dart `visitDotShorthandInvocation`.
    pub(super) fn visit_dot_shorthand_invocation(&mut self, node: Id<DotShorthandInvocation>) {
        required_parameters_verifier::visit_dot_shorthand_invocation(self, node);
        self.visit_children(node);
    }

    /// Dart `visitExpressionFunctionBody`.
    pub(super) fn visit_expression_function_body(&mut self, node: Id<ExpressionFunctionBody>) {
        let parent_is_primary_constructor_body = self
            .ast
            .parent(node)
            .is_some_and(|p| self.ast.is::<PrimaryConstructorBody>(p));
        if !parent_is_primary_constructor_body {
            self.with_return_type_context(|this, context| {
                return_type_verifier::verify_expression_function_body(this, context, node)
            });
        }
        self.visit_children(node);
    }

    /// Dart `visitForEachPartsWithDeclaration`.
    pub(super) fn visit_for_each_parts_with_declaration(
        &mut self,
        node: Id<ForEachPartsWithDeclaration>,
    ) {
        let loop_variable = self.ast[node].loop_variable;
        let element = self.declared_element(loop_variable.raw());
        if self.check_for_each_parts(node.raw(), element)
            && let Some(keyword) = self.ast[loop_variable].keyword
            && self.ast.tokens.lexeme(keyword) == "const"
        {
            self.report_at_token(diag::for_in_with_const_variable(), keyword);
        }
        self.visit_children(node);
    }

    /// Dart `visitForEachPartsWithIdentifier`.
    pub(super) fn visit_for_each_parts_with_identifier(
        &mut self,
        node: Id<ForEachPartsWithIdentifier>,
    ) {
        let identifier = self.ast[node].identifier;
        let element = self
            .element(identifier)
            .map(|e| member::base_element(&self.ctx, e));
        if self.check_for_each_parts(node.raw(), element) {
            self.check_for_assignment_to_final(identifier.upcast());
        }
        self.visit_children(node);
    }

    /// Dart `visitForElement`.
    pub(super) fn visit_for_element(&mut self, node: Id<ForElement>) {
        let parts = self.ast[node].for_loop_parts;
        self.with_hidden_elements_for_for_parts(parts, |this| this.visit_children(node));
    }

    /// Dart `visitForPartsWithDeclarations`.
    pub(super) fn visit_for_parts_with_declarations(&mut self, node: Id<ForPartsWithDeclarations>) {
        let variables = self.ast[node].variables;
        self.with_duplicate_definition_verifier(|d, this| d.check_for_variables(this, variables));
        self.visit_children(node);
    }

    /// Dart `visitForStatement`.
    pub(super) fn visit_for_statement(&mut self, node: Id<ForStatement>) {
        let parts = self.ast[node].for_loop_parts;
        self.with_hidden_elements_for_for_parts(parts, |this| this.visit_children(node));
    }

    /// Dart `visitFunctionExpressionInvocation`.
    pub(super) fn visit_function_expression_invocation(
        &mut self,
        node: Id<FunctionExpressionInvocation>,
    ) {
        let function_expression = self.ast[node].function;
        if self.ast.is::<ExtensionOverride>(function_expression) {
            self.visit_children(node);
            return;
        }

        type_arguments_verifier::check_function_expression_invocation(self, node);
        required_parameters_verifier::visit_function_expression_invocation(self, node);
        const_argument_verifier::visit_function_expression_invocation(self, node);
        use_result_verifier::check_function_expression_invocation(self, node);
        self.visit_children(node);
    }

    /// Dart `visitFunctionReference`.
    pub(super) fn visit_function_reference(&mut self, node: Id<FunctionReference>) {
        type_arguments_verifier::check_function_reference(self, node);
        self.visit_children(node);
    }

    /// Dart `visitGuardedPattern`.
    pub(super) fn visit_guarded_pattern(&mut self, node: Id<GuardedPattern>) {
        let pattern = self.ast[node].pattern;
        self.with_hidden_elements_guarded_pattern(node, |this| this.accept(pattern));
        let when_clause = self.ast[node].when_clause;
        self.accept_opt(when_clause);
    }

    /// Dart `visitImportPrefixReference`.
    pub(super) fn visit_import_prefix_reference(&mut self, node: Id<ImportPrefixReference>) {
        let name = self.ast[node].name;
        let element = self.element(node);
        self.check_for_reference_before_declaration(name, element);
    }

    /// Dart `visitIndexExpression`.
    pub(super) fn visit_index_expression(&mut self, node: Id<IndexExpression>) {
        // Note: `node.isNullAware` produces the wrong behavior because it
        // considers all sections of a null-aware cascade to be null-aware,
        // so it's necessary to look directly at the operator.
        let question = self.ast[node].question;
        let period = self.ast[node].period;
        let is_null_aware = question.is_some()
            || period.is_some_and(|p| self.ast.tokens.ty(p) == TokenType::QUESTION_PERIOD_PERIOD);
        if is_null_aware && let Some(target) = self.index_expression_real_target(node) {
            let is_cascaded = period.is_some();
            let operator = question.or(period).unwrap_or(self.ast[node].left_bracket);
            self.check_for_unnecessary_null_aware(
                target,
                operator,
                if is_cascaded {
                    NullAwareKind::Cascaded
                } else {
                    NullAwareKind::IndexExpression
                },
            );
        }

        self.visit_children(node);
    }

    /// Dart `visitIntegerLiteral`.
    pub(super) fn visit_integer_literal(&mut self, node: Id<IntegerLiteral>) {
        self.check_for_out_of_range(node);
        self.visit_children(node);
    }

    /// Dart `visitInterpolationExpression`.
    pub(super) fn visit_interpolation_expression(&mut self, node: Id<InterpolationExpression>) {
        let expression = self.ast[node].expression;
        self.check_for_use_of_void_result(expression);
        self.visit_children(node);
    }

    /// Dart `visitIsExpression`.
    pub(super) fn visit_is_expression(&mut self, node: Id<IsExpression>) {
        let type_ = self.ast[node].type_;
        self.check_for_type_annotation_deferred_class(Some(type_));
        let expression = self.ast[node].expression;
        self.check_for_use_of_void_result(expression);
        self.visit_children(node);
    }

    /// Dart `visitListLiteral`.
    pub(super) fn visit_list_literal(&mut self, node: Id<ListLiteral>) {
        type_arguments_verifier::check_list_literal(self, node);
        self.check_for_list_element_type_not_assignable(node);

        self.visit_children(node);
    }

    /// Dart `visitMapLiteralEntry`.
    pub(super) fn visit_map_literal_entry(&mut self, node: Id<MapLiteralEntry>) {
        if let Some(key_question) = self.ast[node].key_question {
            let key = self.ast[node].key;
            self.check_for_unnecessary_null_aware(key, key_question, NullAwareKind::MapEntryKey);
        }
        if let Some(value_question) = self.ast[node].value_question {
            let value = self.ast[node].value;
            self.check_for_unnecessary_null_aware(
                value,
                value_question,
                NullAwareKind::MapEntryValue,
            );
        }
        self.visit_children(node);
    }

    /// Dart `visitMethodInvocation`.
    pub(super) fn visit_method_invocation(&mut self, node: Id<MethodInvocation>) {
        let target = ast_ext::method_invocation_real_target(self.ast, node);
        let method_name = self.ast[node].method_name;
        if let Some(target) = target {
            let type_reference = self.get_type_reference(target);
            self.check_for_static_access_to_instance_member(type_reference, method_name);
            let node_target = self.ast[node].target;
            self.check_for_instance_access_to_static_member(
                type_reference,
                node_target,
                method_name,
            );
            // Note: `node.isNullAware` produces the wrong behavior because
            // it considers all sections of a null-aware cascade to be
            // null-aware, so it's necessary to look directly at the
            // operator.
            if let Some(operator) = self.ast[node].operator {
                let ty = self.ast.tokens.ty(operator);
                if ty == TokenType::QUESTION_PERIOD || ty == TokenType::QUESTION_PERIOD_PERIOD {
                    let kind = if ast_ext::method_invocation_is_cascaded(self.ast, node) {
                        NullAwareKind::Cascaded
                    } else {
                        NullAwareKind::Access
                    };
                    self.check_for_unnecessary_null_aware(target, operator, kind);
                }
            }
        } else {
            self.check_for_unqualified_reference_to_non_local_static_member(method_name);
        }
        type_arguments_verifier::check_method_invocation(self, node);
        required_parameters_verifier::visit_method_invocation(self, node);
        const_argument_verifier::visit_method_invocation(self, node);
        use_result_verifier::check_method_invocation(self, node);
        self.visit_children(node);
    }

    /// Dart `visitNativeFunctionBody`.
    pub(super) fn visit_native_function_body(&mut self, node: Id<NativeFunctionBody>) {
        self.check_for_native_function_body_in_non_sdk_code(node);
        self.visit_children(node);
    }

    /// Dart `visitNullAwareElement`.
    pub(super) fn visit_null_aware_element(&mut self, node: Id<NullAwareElement>) {
        let value = self.ast[node].value;
        let question = self.ast[node].question;
        self.check_for_unnecessary_null_aware(value, question, NullAwareKind::Element);
        self.visit_children(node);
    }

    /// Dart `visitPatternVariableDeclarationStatement`.
    pub(super) fn visit_pattern_variable_declaration_statement(
        &mut self,
        node: Id<PatternVariableDeclarationStatement>,
    ) {
        self.visit_children(node);
        let declaration = self.ast[node].declaration;
        let elements = self.pattern_variable_declaration_elements(declaration);
        if let Some(hidden) = self.hidden_elements.as_mut() {
            for variable in elements {
                hidden.declare(variable);
            }
        }
    }

    /// Dart `visitPostfixExpression`.
    pub(super) fn visit_postfix_expression(&mut self, node: Id<PostfixExpression>) {
        let operand = self.ast[node].operand;
        let operator = self.ast[node].operator;
        if self.ast.tokens.ty(operator) == TokenType::BANG {
            self.check_for_use_of_void_result(node.upcast());
            self.check_for_unnecessary_null_aware(operand, operator, NullAwareKind::NullCheck);
        } else {
            self.check_for_assignment_to_final(operand);
            self.check_for_assignment_to_primary_constructor_parameter(operand.raw());
            self.check_for_int_not_assignable(operand);
        }
        self.visit_children(node);
    }

    /// Dart `visitPrefixedIdentifier`.
    pub(super) fn visit_prefixed_identifier(&mut self, node: Id<PrefixedIdentifier>) {
        const_argument_verifier::visit_prefixed_identifier(self, node);
        let parent_is_annotation = self
            .ast
            .parent(node)
            .is_some_and(|p| self.ast.is::<Annotation>(p));
        if !parent_is_annotation {
            let prefix = self.ast[node].prefix;
            let type_reference = self.get_type_reference(prefix.upcast());
            let name = self.ast[node].identifier;
            self.check_for_static_access_to_instance_member(type_reference, name);
            self.check_for_instance_access_to_static_member(
                type_reference,
                Some(prefix.upcast()),
                name,
            );
        }
        self.visit_children(node);
    }

    /// Dart `visitPrefixExpression`.
    pub(super) fn visit_prefix_expression(&mut self, node: Id<PrefixExpression>) {
        let operator_type = self.ast.tokens.ty(self.ast[node].operator);
        let operand = self.ast[node].operand;
        if operator_type != TokenType::BANG {
            if ast_ext::is_increment_operator(operator_type) {
                self.check_for_assignment_to_final(operand);
                self.check_for_assignment_to_primary_constructor_parameter(operand.raw());
            }
            self.check_for_use_of_void_result(operand);
            self.check_for_int_not_assignable(operand);
        }
        self.visit_children(node);
    }

    /// Dart `visitPropertyAccess`.
    pub(super) fn visit_property_access(&mut self, node: Id<PropertyAccess>) {
        const_argument_verifier::visit_property_access(self, node);
        let target = self.property_access_real_target(node);
        let property_name = self.ast[node].property_name;
        if let Some(target) = target {
            let type_reference = self.get_type_reference(target);
            self.check_for_static_access_to_instance_member(type_reference, property_name);
            let node_target = self.ast[node].target;
            self.check_for_instance_access_to_static_member(
                type_reference,
                node_target,
                property_name,
            );
            // Note: `node.isNullAware` produces the wrong behavior because
            // it considers all sections of a null-aware cascade to be
            // null-aware, so it's necessary to look directly at the
            // operator.
            let operator = self.ast[node].operator;
            let ty = self.ast.tokens.ty(operator);
            if ty == TokenType::QUESTION_PERIOD || ty == TokenType::QUESTION_PERIOD_PERIOD {
                let kind = if ast_ext::property_access_is_cascaded(self.ast, node) {
                    NullAwareKind::Cascaded
                } else {
                    NullAwareKind::Access
                };
                self.check_for_unnecessary_null_aware(target, operator, kind);
            }
        }
        use_result_verifier::check_property_access(self, node);
        self.visit_children(node);
    }

    /// Dart `visitRethrowExpression`.
    pub(super) fn visit_rethrow_expression(&mut self, node: Id<RethrowExpression>) {
        self.check_for_rethrow_outside_catch(node);
        self.visit_children(node);
    }

    /// Dart `visitReturnStatement`.
    pub(super) fn visit_return_statement(&mut self, node: Id<ReturnStatement>) {
        if self.ast[node].expression.is_none() {
            self.enclosing_executable.returns_without.push(node.raw());
        } else {
            self.enclosing_executable.returns_with.push(node.raw());
            self.report_missing_await_in_try_block(node);
        }
        self.with_return_type_context(|this, context| {
            return_type_verifier::verify_return_statement(this, context, node)
        });
        self.visit_children(node);
    }

    /// Dart `visitSetOrMapLiteral`.
    pub(super) fn visit_set_or_map_literal(&mut self, node: Id<SetOrMapLiteral>) {
        let ty = self.static_type(node);
        let is_map = ty.is_some_and(|t| self.ctx.is_dart_core_map(t));
        let is_set = ty.is_some_and(|t| self.ctx.is_dart_core_set(t));
        if is_map {
            type_arguments_verifier::check_map_literal(self, node);
            self.check_for_map_type_not_assignable(node);
            self.check_for_non_const_map_as_expression_statement3(node);
        } else if is_set {
            type_arguments_verifier::check_set_literal(self, node);
            self.check_for_set_element_type_not_assignable3(node);
        }
        self.visit_children(node);
    }

    /// Dart `visitSimpleIdentifier`.
    pub(super) fn visit_simple_identifier(&mut self, node: Id<SimpleIdentifier>) {
        const_argument_verifier::visit_simple_identifier(self, node);
        let token = self.ast[node].token;
        let write_or_read_element = self.write_or_read_element(node.raw());
        self.check_for_ambiguous_import(token, write_or_read_element);
        let element = self.element(node);
        self.check_for_reference_before_declaration(token, element);
        self.check_for_invalid_instance_member_access(node);
        self.check_for_type_parameter_referenced_by_static(token, element);
        if !self.is_unqualified_reference_to_non_local_static_member_allowed(node) {
            self.check_for_unqualified_reference_to_non_local_static_member(node);
        }
        use_result_verifier::check_simple_identifier(self, node);
        self.visit_children(node);
    }

    /// Dart `visitSpreadElement`.
    pub(super) fn visit_spread_element(&mut self, node: Id<SpreadElement>) {
        let spread_operator = self.ast[node].spread_operator;
        if self.ast.tokens.lexeme(spread_operator) == "...?" {
            let expression = self.ast[node].expression;
            self.check_for_unnecessary_null_aware(
                expression,
                spread_operator,
                NullAwareKind::Spread,
            );
        }
        self.visit_children(node);
    }

    /// Dart `visitSwitchCase`.
    pub(super) fn visit_switch_case(&mut self, node: Id<SwitchCase>) {
        let statements = self.ast.list(self.ast[node].statements).to_vec();
        self.with_hidden_elements_for_statements(&statements, |this| {
            let list = this.ast[node].statements;
            this.with_duplicate_definition_verifier(|d, this| d.check_statements(this, list));
            this.visit_children(node);
        });
    }

    /// Dart `visitSwitchDefault`.
    pub(super) fn visit_switch_default(&mut self, node: Id<SwitchDefault>) {
        let statements = self.ast.list(self.ast[node].statements).to_vec();
        self.with_hidden_elements_for_statements(&statements, |this| {
            let list = this.ast[node].statements;
            this.with_duplicate_definition_verifier(|d, this| d.check_statements(this, list));
            this.visit_children(node);
        });
    }

    /// Dart `visitSwitchExpression`.
    pub(super) fn visit_switch_expression(&mut self, node: Id<SwitchExpression>) {
        let expression = self.ast[node].expression;
        self.check_for_use_of_void_result(expression);
        self.visit_children(node);
    }

    /// Dart `visitSwitchPatternCase`.
    pub(super) fn visit_switch_pattern_case(&mut self, node: Id<SwitchPatternCase>) {
        let statements = self.ast.list(self.ast[node].statements).to_vec();
        self.with_hidden_elements_for_statements(&statements, |this| {
            let list = this.ast[node].statements;
            this.with_duplicate_definition_verifier(|d, this| d.check_statements(this, list));
            this.visit_children(node);
        });
    }

    /// Dart `visitSwitchStatement`.
    pub(super) fn visit_switch_statement(&mut self, node: Id<SwitchStatement>) {
        let expression = self.ast[node].expression;
        self.check_for_use_of_void_result(expression);
        self.check_for_missing_enum_constant_in_switch(node);
        self.visit_children(node);
    }

    /// Dart `visitThisExpression`.
    pub(super) fn visit_this_expression(&mut self, node: Id<ThisExpression>) {
        self.check_for_invalid_reference_to_this(node);
        self.visit_children(node);
    }

    /// Dart `visitThrowExpression`.
    pub(super) fn visit_throw_expression(&mut self, node: Id<ThrowExpression>) {
        self.check_for_const_eval_throws_exception(node);
        let expression = self.ast[node].expression;
        self.check_for_use_of_void_result(expression);
        self.check_for_throw_of_invalid_type(node);
        self.visit_children(node);
    }

    /// Dart `visitVariableDeclarationStatement`.
    pub(super) fn visit_variable_declaration_statement(
        &mut self,
        node: Id<VariableDeclarationStatement>,
    ) {
        let variables = self.ast[node].variables;
        let is_late = self.ast[variables].late_keyword.is_some();
        self.is_in_late_local_variable.push(is_late);

        let is_const = self.ast[variables]
            .keyword
            .is_some_and(|k| self.ast.tokens.lexeme(k) == "const");
        if is_const {
            let declarations = self.ast.list(self.ast[variables].variables).to_vec();
            for variable in declarations {
                if self.ast[variable].initializer.is_none() {
                    let name = self.ast[variable].name;
                    let lexeme = self.ast.tokens.lexeme(name).to_string();
                    self.report_at_token(diag::const_not_initialized(&lexeme), name);
                }
            }
        }

        self.visit_children(node);

        self.is_in_late_local_variable.pop();
    }

    // ------------------------------------------------------------ helpers

    /// Dart `PatternVariableDeclarationImpl.elements`.
    fn pattern_variable_declaration_elements(
        &self,
        declaration: Id<PatternVariableDeclaration>,
    ) -> Vec<ElementId> {
        self.rt
            .pattern_variable_declaration_elements
            .get(declaration)
            .cloned()
            .unwrap_or_default()
    }

    /// Dart `writeOrReadElement` of an identifier (`_writeElement(node) ??
    /// element`, ast/extensions.dart).
    fn write_or_read_element(&self, node: NodeId) -> Option<ElemRef> {
        self.write_element_of(node).or_else(|| self.element(node))
    }

    /// Dart `_writeElement(node)` (ast/extensions.dart).
    fn write_element_of(&self, node: NodeId) -> Option<ElemRef> {
        let parent = self.ast.parent(node)?;
        if let Some(p) = self.ast.cast::<AssignmentExpression>(parent) {
            if self.ast[p].left_hand_side.raw() == node {
                return self.tables.write_element.get(p).copied();
            }
            return None;
        }
        if let Some(p) = self.ast.cast::<PostfixExpression>(parent) {
            if self.ast[p].operand.raw() == node {
                return self.tables.write_element.get(p).copied();
            }
            return None;
        }
        if let Some(p) = self.ast.cast::<PrefixExpression>(parent) {
            if self.ast[p].operand.raw() == node {
                return self.tables.write_element.get(p).copied();
            }
            return None;
        }
        if let Some(p) = self.ast.cast::<PrefixedIdentifier>(parent) {
            if self.ast[p].identifier.raw() == node {
                return self.write_element_of(parent);
            }
            return None;
        }
        if let Some(p) = self.ast.cast::<PropertyAccess>(parent) {
            if self.ast[p].property_name.raw() == node {
                return self.write_element_of(parent);
            }
            return None;
        }
        None
    }

    /// The nearest enclosing cascade of [node] (Dart `_ancestorCascade`).
    fn enclosing_cascade(&self, node: NodeId) -> Option<Id<CascadeExpression>> {
        let parent = self.ast.parent(node)?;
        self.ast
            .this_or_ancestor_of_type::<CascadeExpression>(parent)
    }

    /// Dart `IndexExpression.realTarget`.
    fn index_expression_real_target(&self, node: Id<IndexExpression>) -> Option<Id<Expression>> {
        if self.ast[node].period.is_some() {
            return self
                .enclosing_cascade(node.raw())
                .map(|c| self.ast[c].target);
        }
        self.ast[node].target
    }

    /// Dart `PropertyAccess.realTarget`.
    fn property_access_real_target(&self, node: Id<PropertyAccess>) -> Option<Id<Expression>> {
        if ast_ext::property_access_is_cascaded(self.ast, node) {
            return self
                .enclosing_cascade(node.raw())
                .map(|c| self.ast[c].target);
        }
        self.ast[node].target
    }

    /// Whether [e] is a Dart `InterfaceElement`.
    fn is_interface_element(e: ElementId) -> bool {
        e.is::<InterfaceElement>()
    }

    /// Dart `_checkForAssignmentToFinal(expression)`: verifies that the
    /// given [expression] is not final.
    fn check_for_assignment_to_final(&mut self, expression: Id<Expression>) {
        // TODO(scheglov): Check SimpleIdentifier(s) as all other nodes.
        let Some(identifier) = self.ast.cast::<SimpleIdentifier>(expression) else {
            return;
        };

        // Already handled in the assignment resolver.
        if self
            .ast
            .parent(identifier)
            .is_some_and(|p| self.ast.is::<AssignmentExpression>(p))
        {
            return;
        }

        let ctx = self.ctx;
        let Some(element) = self.element(identifier) else {
            return;
        };
        let element = member::base_element(&ctx, element);
        if element.is::<VariableElement>() {
            if element_ext::is_const(&ctx, element) {
                self.report_at(diag::assignment_to_const(), identifier);
            }
        } else if element.tag() == Tag::Getter {
            let Some(variable) = member::variable(&ctx, ElemRef::Base(element)) else {
                return;
            };
            let variable = member::base_element(&ctx, variable);
            let variable_name = ctx.element_name(variable).unwrap_or("").to_string();
            if element_ext::is_const(&ctx, variable) {
                self.report_at(diag::assignment_to_const(), identifier);
            } else if variable.tag() == Tag::Field
                && element_ext::first_fragment_flags(&ctx, variable)
                    .contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER)
            {
                let class_name = ctx
                    .element_data(variable)
                    .and_then(|d| d.enclosing)
                    .and_then(|e| ctx.element_name(e))
                    .unwrap_or("")
                    .to_string();
                self.report_at(
                    diag::assignment_to_final_no_setter(&variable_name, &class_name),
                    identifier,
                );
            } else {
                self.report_at(diag::assignment_to_final(&variable_name), identifier);
            }
        } else if matches!(element.tag(), Tag::LocalFunction | Tag::TopLevelFunction) {
            self.report_at(diag::assignment_to_function(), identifier);
        } else if element.tag() == Tag::Method {
            self.report_at(diag::assignment_to_method(), identifier);
        } else if Self::is_interface_element(element)
            || matches!(element.tag(), Tag::Dynamic | Tag::TypeParameter)
        {
            self.report_at(diag::assignment_to_type(), identifier);
        }
    }

    /// Dart `_checkForAssignmentToPrimaryConstructorParameter(node)`.
    fn check_for_assignment_to_primary_constructor_parameter(&mut self, node: NodeId) {
        if !self.ast.is::<AssignedVariablePattern>(node) && !self.ast.is::<SimpleIdentifier>(node) {
            return;
        }
        let ctx = self.ctx;
        let Some(formal_parameter) = self.element(node).map(|e| member::base_element(&ctx, e))
        else {
            return;
        };
        if !formal_parameter.is::<FormalParameterElement>() {
            return;
        }
        let Some(enclosing) = ctx.element_data(formal_parameter).and_then(|d| d.enclosing) else {
            return;
        };
        if enclosing.tag() == Tag::Constructor
            && element_ext::first_fragment_flags(&ctx, enclosing)
                .contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_PRIMARY)
        {
            match self.this_context() {
                ThisContext::ConstructorInitializers | ThisContext::InstanceFieldDeclaration => {
                    self.report_at(diag::assignment_to_primary_constructor_parameter(), node);
                }
                _ => {
                    // OK
                }
            }
        }
    }

    /// Dart `_checkForAwaitInLateLocalVariableInitializer(node)`.
    fn check_for_await_in_late_local_variable_initializer(&mut self, node: Id<AwaitExpression>) {
        if self
            .is_in_late_local_variable
            .last()
            .copied()
            .unwrap_or(false)
        {
            let await_keyword = self.ast[node].await_keyword;
            self.report_at_token(
                diag::await_in_late_local_variable_initializer(),
                await_keyword,
            );
        }
    }

    /// Dart `_checkForAwaitOfIncompatibleType(node)`.
    fn check_for_await_of_incompatible_type(&mut self, node: Id<AwaitExpression>) {
        let expression = self.ast[node].expression;
        let Some(expression_type) = self.static_type(expression) else {
            return;
        };
        if self.type_system.is_incompatible_with_await(expression_type) {
            let await_keyword = self.ast[node].await_keyword;
            self.report_at_token(diag::await_of_incompatible_type(), await_keyword);
        }
    }

    /// Dart `_checkForConstEvalThrowsException(expression)`.
    fn check_for_const_eval_throws_exception(&mut self, expression: Id<ThrowExpression>) {
        if self.enclosing_executable.is_const_constructor {
            self.report_at(diag::const_constructor_throws_exception(), expression);
        }
    }

    /// Dart `_checkForDeadNullCoalesce(lhsType, rhs)`.
    fn check_for_dead_null_coalesce(&mut self, lhs_type: TypeId, rhs: Id<Expression>) {
        if self.type_system.is_strictly_non_nullable(lhs_type) {
            self.report_at(diag::dead_null_aware_expression(), rhs);
        }
    }

    /// Dart `_checkForEachParts(node, variableElement)`. [node] is a
    /// `ForEachParts` (with a declaration or an identifier).
    fn check_for_each_parts(&mut self, node: NodeId, variable_element: Option<ElementId>) -> bool {
        let iterable = if let Some(n) = self.ast.cast::<ForEachPartsWithDeclaration>(node) {
            self.ast[n].iterable
        } else if let Some(n) = self.ast.cast::<ForEachPartsWithIdentifier>(node) {
            self.ast[n].iterable
        } else {
            return false;
        };
        if self.check_for_use_of_void_result(iterable) {
            return false;
        }

        let Some(mut iterable_type) = self.static_type(iterable) else {
            return false;
        };

        let parent = self.ast.parent(node);
        let await_keyword = parent.and_then(|p| {
            if let Some(f) = self.ast.cast::<ForStatement>(p) {
                self.ast[f].await_keyword
            } else if let Some(f) = self.ast.cast::<ForElement>(p) {
                self.ast[f].await_keyword
            } else {
                None
            }
        });

        // Use an explicit string instead of [loopType] to remove the "<E>".
        let loop_named_type = if await_keyword.is_some() {
            "Stream"
        } else {
            "Iterable"
        };

        let ctx = self.ctx;
        let strict_casts = self.strict_casts();
        if matches!(ctx.ty(iterable_type), TypeKind::Dynamic) && strict_casts {
            self.report_at(
                diag::for_in_of_invalid_type(type_arg(&ctx, iterable_type), loop_named_type),
                iterable,
            );
            return false;
        }

        // TODO(scheglov): use NullableDereferenceVerifier
        if self.type_system.is_nullable(iterable_type) {
            return false;
        }

        // The type of the loop variable.
        let Some(variable_element) = variable_element.filter(|e| e.is::<VariableElement>()) else {
            return false;
        };
        let variable_type = element_ext::variable_type(&ctx, variable_element);

        // The object being iterated has to implement Iterable<T> for some T
        // that is assignable to the variable's type.
        iterable_type = self.type_system.resolve_to_bound(iterable_type);

        let required_sequence_type = if await_keyword.is_some() {
            ctx.tp.stream_dynamic_type()
        } else {
            ctx.tp.iterable_dynamic_type()
        };

        if self.type_system.is_top(iterable_type) {
            iterable_type = required_sequence_type;
        }

        if !self
            .type_system
            .is_assignable_to(iterable_type, required_sequence_type, strict_casts)
        {
            self.report_at(
                diag::for_in_of_invalid_type(type_arg(&ctx, iterable_type), loop_named_type),
                iterable,
            );
            return false;
        }

        let sequence_element = if await_keyword.is_some() {
            ctx.tp.stream_element().upcast()
        } else {
            ctx.tp.iterable_element().upcast()
        };
        let sequence_element_type = ctx
            .as_instance_of(iterable_type, sequence_element)
            .and_then(|t| ctx.type_arguments(t).first().copied());

        let Some(sequence_element_type) = sequence_element_type else {
            return true;
        };

        // The element value is implicitly assigned to the loop variable. If
        // the element type is `void`, the value can only be discarded into a
        // `void` loop variable.
        if matches!(ctx.ty(sequence_element_type), TypeKind::Void)
            && !matches!(ctx.ty(variable_type), TypeKind::Void)
        {
            self.report_at(diag::use_of_void_result(), iterable);
            return false;
        }

        if !self
            .type_system
            .is_assignable_to(sequence_element_type, variable_type, strict_casts)
        {
            // A for-in loop is specified to desugar to a different set of
            // statements which include an assignment of the sequence
            // element's `iterator`'s `current` value, at which point
            // "implicit tear-off conversion" may be performed. We do not
            // perform this desugaring; instead we allow a special
            // assignability here.
            let implicit_call_method =
                self.get_implicit_call_method(sequence_element_type, variable_type);
            match implicit_call_method {
                None => {
                    self.report_at(
                        diag::for_in_of_invalid_element_type(
                            type_arg(&ctx, iterable_type),
                            loop_named_type,
                            type_arg(&ctx, variable_type),
                        ),
                        iterable,
                    );
                }
                Some(call_method) => {
                    let tearoff_type = member::type_(&ctx, call_method);
                    // An implicit tear-off conversion does occur on the
                    // values of the iterator, but this does not guarantee
                    // their assignability. Dart instantiates a generic
                    // tear-off with `inferFunctionTypeInstantiation`: not
                    // ported, a generic tear-off is not reported.
                    let is_generic = matches!(
                        *ctx.ty(tearoff_type),
                        TypeKind::Function(f) if !ctx.list(f.type_params).is_empty()
                    );
                    if !is_generic
                        && !self.type_system.is_assignable_to(
                            tearoff_type,
                            variable_type,
                            strict_casts,
                        )
                    {
                        self.report_at(
                            diag::for_in_of_invalid_element_type(
                                type_arg(&ctx, iterable_type),
                                loop_named_type,
                                type_arg(&ctx, variable_type),
                            ),
                            iterable,
                        );
                    }
                }
            }
        }

        true
    }

    /// Dart `_checkForInstanceAccessToStaticMember(typeReference, target,
    /// name)`. In this version of the analyzer the method reports nothing
    /// (the resolver reports `instance_access_to_static_member`).
    fn check_for_instance_access_to_static_member(
        &mut self,
        type_reference: Option<EId<InterfaceElement>>,
        target: Option<Id<Expression>>,
        name: Id<SimpleIdentifier>,
    ) {
        let _ = (type_reference, target, name);
    }

    /// Dart `_checkForIntNotAssignable(argument)`: verifies that an `int`
    /// can be assigned to the parameter that corresponds to [argument] (the
    /// operand of a prefix or postfix increment or decrement).
    fn check_for_int_not_assignable(&mut self, argument: Id<Expression>) {
        // Dart `argument.correspondingParameter`: the first parameter of
        // the operator method of the prefix or postfix expression.
        let ctx = self.ctx;
        let Some(parent) = self.ast.parent(argument) else {
            return;
        };
        if !self.ast.is::<PrefixExpression>(parent) && !self.ast.is::<PostfixExpression>(parent) {
            return;
        }
        let Some(element) = self.element(parent) else {
            return;
        };
        let parameters = member::formal_parameters(&ctx, element);
        let Some(&parameter) = parameters.first() else {
            return;
        };
        let parameter_type = member::type_(&ctx, parameter);
        let int_type = ctx.tp.int_type();
        self.check_for_argument_type_not_assignable(
            argument,
            parameter_type,
            int_type,
            NonAssignabilityReporter::ForArgument,
        );
    }

    /// Dart `_checkForInvalidAnnotationFromDeferredLibrary(annotation)`.
    fn check_for_invalid_annotation_from_deferred_library(&mut self, annotation: Id<Annotation>) {
        let name_identifier = self.ast[annotation].name;
        let Some(prefixed) = self.ast.cast::<PrefixedIdentifier>(name_identifier) else {
            return;
        };
        if self.prefixed_identifier_is_deferred(prefixed) {
            self.report_at(
                diag::invalid_annotation_from_deferred_library(),
                name_identifier,
            );
        }
    }

    /// Dart `PrefixedIdentifier.isDeferred`: the prefix is a deferred
    /// import prefix.
    fn prefixed_identifier_is_deferred(&self, node: Id<PrefixedIdentifier>) -> bool {
        let prefix = self.ast[node].prefix;
        let Some(ElemRef::Base(element)) = self.element(prefix) else {
            return false;
        };
        if element.tag() != Tag::Prefix {
            return false;
        }
        let ctx = self.ctx;
        ctx.element_data(element)
            .and_then(|d| d.first_fragment.cast::<dartr_element::PrefixFragment>())
            .is_some_and(|f| ctx.fragment(f).is_deferred)
    }

    /// Dart `_checkForInvalidInstanceMemberAccess(identifier)`.
    fn check_for_invalid_instance_member_access(&mut self, identifier: Id<SimpleIdentifier>) {
        if self.is_in_comment {
            return;
        }

        let this_context = self.this_context();
        if this_context.allows_this() {
            return;
        }

        let ctx = self.ctx;
        let Some(element) = self.write_or_read_element(identifier.raw()) else {
            return;
        };
        let base = member::base_element(&ctx, element);
        if !matches!(base.tag(), Tag::Method | Tag::Getter | Tag::Setter) {
            return;
        }
        // static element
        if member::is_static(&ctx, element) {
            return;
        }
        // not a class member
        let Some(enclosing_element) = ctx.element_data(base).and_then(|d| d.enclosing) else {
            return;
        };
        if !enclosing_element.is::<dartr_element::InstanceElement>() {
            return;
        }
        let Some(parent) = self.ast.parent(identifier) else {
            return;
        };
        // qualified method invocation
        if let Some(p) = self.ast.cast::<MethodInvocation>(parent)
            && self.ast[p].method_name == identifier
            && ast_ext::method_invocation_real_target(self.ast, p).is_some()
        {
            return;
        }
        // qualified property access
        if let Some(p) = self.ast.cast::<PropertyAccess>(parent)
            && self.ast[p].property_name == identifier
        {
            return;
        }
        if let Some(p) = self.ast.cast::<PrefixedIdentifier>(parent)
            && self.ast[p].identifier == identifier
        {
            return;
        }

        match this_context {
            ThisContext::ConstructorInitializers
            | ThisContext::InstanceFieldDeclaration
            | ThisContext::StaticFieldDeclaration
            | ThisContext::TopLevel => {
                let name = ast_ext::identifier_name(self.ast, identifier).to_string();
                self.report_at(
                    diag::implicit_this_reference_in_initializer(&name),
                    identifier,
                );
            }
            ThisContext::FactoryConstructorBody => {
                self.report_at(diag::instance_member_access_from_factory(), identifier);
            }
            ThisContext::StaticMemberBody => {
                self.report_at(diag::instance_member_access_from_static(), identifier);
            }
            ThisContext::GenerativeConstructorBody
            | ThisContext::InstanceMemberBody
            | ThisContext::LateInstanceFieldDeclaration => {
                // Dart: `throw StateError('Should not be reached')`.
            }
        }
    }

    /// Dart `_checkForInvalidReferenceToThis(expression)`.
    fn check_for_invalid_reference_to_this(&mut self, expression: Id<ThisExpression>) {
        if !self.this_context().allows_this() {
            self.report_at(diag::invalid_reference_to_this(), expression);
        }
    }

    /// The type arguments of the static type of [literal] (Dart
    /// `(literal.typeOrThrow as InterfaceTypeImpl).typeArguments`).
    fn literal_type_arguments(&self, literal: NodeId) -> Option<Vec<TypeId>> {
        let ty = self.static_type(literal)?;
        if !matches!(self.ctx.ty(ty), TypeKind::Interface { .. }) {
            return None;
        }
        Some(self.ctx.type_arguments(ty).to_vec())
    }

    /// Dart `_checkForListElementTypeNotAssignable(literal)`.
    fn check_for_list_element_type_not_assignable(&mut self, literal: Id<ListLiteral>) {
        let Some(type_arguments) = self.literal_type_arguments(literal.raw()) else {
            return;
        };
        let Some(&list_element_type) = type_arguments.first() else {
            return;
        };
        let verifier = LiteralElementVerifier {
            for_list: true,
            element_type: Some(list_element_type),
            ..LiteralElementVerifier::default()
        };
        let elements = self.ast.list(self.ast[literal].elements).to_vec();
        for element in elements {
            verifier.verify(self, element);
        }
    }

    /// Dart `_checkForMapTypeNotAssignable(literal)`.
    fn check_for_map_type_not_assignable(&mut self, literal: Id<SetOrMapLiteral>) {
        let Some(type_arguments) = self.literal_type_arguments(literal.raw()) else {
            return;
        };
        // It is possible for the number of type arguments to be
        // inconsistent when the literal is ambiguous and a non-map type was
        // selected.
        if type_arguments.len() == 2 {
            let verifier = LiteralElementVerifier {
                for_map: true,
                map_key_type: Some(type_arguments[0]),
                map_value_type: Some(type_arguments[1]),
                ..LiteralElementVerifier::default()
            };
            let elements = self.ast.list(self.ast[literal].elements).to_vec();
            for element in elements {
                verifier.verify(self, element);
            }
        }
    }

    /// Dart `_checkForMissingEnumConstantInSwitch(statement)`.
    fn check_for_missing_enum_constant_in_switch(&mut self, statement: Id<SwitchStatement>) {
        if self
            .unit
            .features
            .is_experiment_enabled(dartr_parser::experimental_flags::ExperimentalFlag::Patterns)
        {
            // Exhaustiveness checking cover this warning.
            return;
        }

        let ctx = self.ctx;
        let expression = self.ast[statement].expression;
        let Some(expression_type) = self.static_type(expression) else {
            return;
        };
        let TypeKind::Interface { element, .. } = *ctx.ty(expression_type) else {
            return;
        };
        if element.raw().tag() != Tag::Enum {
            return;
        }
        let mut has_case_null = false;
        let mut constant_names: IndexSet<String> =
            element_ext::enum_constants(&ctx, element.upcast())
                .into_iter()
                .filter_map(|f| ctx.element_name(f).map(str::to_string))
                .collect();

        let members = self.ast.list(self.ast[statement].members).to_vec();
        for member_node in members {
            let mut case_constant = None;
            if let Some(c) = self.ast.cast::<SwitchCase>(member_node) {
                case_constant = Some(self.ast[c].expression);
            } else if let Some(c) = self.ast.cast::<SwitchPatternCase>(member_node) {
                let guarded_pattern = self.ast[c].guarded_pattern;
                if self.ast[guarded_pattern].when_clause.is_none() {
                    let mut pattern = self.ast[guarded_pattern].pattern;
                    while let Some(p) = self.ast.cast::<ParenthesizedPattern>(pattern) {
                        pattern = self.ast[p].pattern;
                    }
                    if let Some(p) = self.ast.cast::<ConstantPattern>(pattern) {
                        case_constant = Some(self.ast[p].expression);
                    }
                }
            }
            if let Some(case_constant) = case_constant {
                let expression = ast_ext::un_parenthesized(self.ast, case_constant);
                if self.ast.is::<NullLiteral>(expression) {
                    has_case_null = true;
                } else if let Some(constant_name) = self.get_constant_name(expression) {
                    constant_names.shift_remove(&constant_name);
                }
            }
            if self.ast.is::<SwitchDefault>(member_node) {
                return;
            }
        }

        let offset = self.ast.offset(statement);
        let end = ast_ext::token_end(self.ast, self.ast[statement].right_parenthesis);
        for constant_name in constant_names {
            self.report(
                diag::missing_enum_constant_in_switch(&constant_name)
                    .at_offset(offset as usize, (end - offset) as usize),
            );
        }

        if self.type_system.is_nullable(expression_type) && !has_case_null {
            self.report(
                diag::missing_enum_constant_in_switch("null")
                    .at_offset(offset as usize, (end - offset) as usize),
            );
        }
    }

    /// Dart `_checkForNativeFunctionBodyInNonSdkCode(body)`.
    fn check_for_native_function_body_in_non_sdk_code(&mut self, body: Id<NativeFunctionBody>) {
        if !self.is_in_system_library {
            self.report_at(diag::native_function_body_in_non_sdk_code(), body);
        }
    }

    /// Dart `_checkForNonConstMapAsExpressionStatement3(literal)`.
    fn check_for_non_const_map_as_expression_statement3(&mut self, literal: Id<SetOrMapLiteral>) {
        // "const"
        if self.ast[literal].const_keyword.is_some() {
            return;
        }
        // has type arguments
        if self.ast[literal].type_arguments.is_some() {
            return;
        }
        // prepare statement
        let Some(statement) = self
            .ast
            .this_or_ancestor_of_type::<ExpressionStatement>(literal.raw())
        else {
            return;
        };
        // OK, statement does not start with map
        if self.ast.begin_token(statement.raw()) != self.ast.begin_token(literal.raw()) {
            return;
        }

        self.report_at(diag::non_const_map_as_expression_statement(), literal);
    }

    /// Dart `_checkForOutOfRange(node)`.
    fn check_for_out_of_range(&mut self, node: Id<IntegerLiteral>) {
        let literal = self.ast[node].literal;
        let lexeme = self.ast.tokens.lexeme(literal).to_string();
        let ty = self.ast.tokens.ty(literal);
        let source = if ty == TokenType::INT_WITH_SEPARATORS
            || ty == TokenType::HEXADECIMAL_WITH_SEPARATORS
        {
            lexeme.replace('_', "")
        } else {
            lexeme.clone()
        };
        // Dart `node.immediatelyNegated`.
        let is_negated = self.ast.parent(node).is_some_and(|p| {
            self.ast
                .cast::<PrefixExpression>(p)
                .is_some_and(|p| self.ast.tokens.ty(self.ast[p].operator) == TokenType::MINUS)
        });

        let ctx = self.ctx;
        let treated_as_double = self
            .static_type(node)
            .is_some_and(|t| ctx.dart_eq(t, ctx.tp.double_type()));
        let valid = if treated_as_double {
            integer_literal::is_valid_as_double(&source)
        } else {
            integer_literal::is_valid_as_integer(&source, is_negated)
        };

        if !valid {
            let literal_text = if is_negated {
                format!("-{lexeme}")
            } else {
                lexeme
            };
            let d = if treated_as_double {
                // Suggest the nearest valid double (as a BigInt, for
                // printing).
                let closest_double = integer_literal::nearest_valid_double_string(&source);
                diag::integer_literal_imprecise_as_double(&literal_text, &closest_double)
            } else {
                diag::integer_literal_out_of_range(&literal_text)
            };
            self.report_at(d, node);
        }
    }

    /// Dart `_checkForReferenceBeforeDeclaration(nameToken:, element:)`.
    fn check_for_reference_before_declaration(
        &mut self,
        name_token: TokenId,
        element: Option<ElemRef>,
    ) {
        let Some(element) = element else {
            return;
        };
        let element = member::base_element(&self.ctx, element);
        if self
            .hidden_elements
            .as_ref()
            .is_some_and(|h| h.contains(element))
        {
            // Dart `DiagnosticFactory.referencedBeforeDeclaration`: the
            // context message is not ported.
            let name = self.ast.tokens.lexeme(name_token).to_string();
            self.report_at_token(diag::referenced_before_declaration(&name), name_token);
        }
    }

    /// Dart `_checkForRethrowOutsideCatch(expression)`.
    fn check_for_rethrow_outside_catch(&mut self, expression: Id<RethrowExpression>) {
        if self.enclosing_executable.catch_clause_level == 0 {
            self.report_at(diag::rethrow_outside_catch(), expression);
        }
    }

    /// Dart `_checkForSetElementTypeNotAssignable3(literal)`.
    fn check_for_set_element_type_not_assignable3(&mut self, literal: Id<SetOrMapLiteral>) {
        let Some(type_arguments) = self.literal_type_arguments(literal.raw()) else {
            return;
        };
        // It is possible for the number of type arguments to be
        // inconsistent when the literal is ambiguous and a non-set type was
        // selected.
        if type_arguments.len() == 1 {
            let verifier = LiteralElementVerifier {
                for_set: true,
                element_type: Some(type_arguments[0]),
                ..LiteralElementVerifier::default()
            };
            let elements = self.ast.list(self.ast[literal].elements).to_vec();
            for element in elements {
                verifier.verify(self, element);
            }
        }
    }

    /// Dart `_checkForStaticAccessToInstanceMember(typeReference, name)`.
    fn check_for_static_access_to_instance_member(
        &mut self,
        type_reference: Option<EId<InterfaceElement>>,
        name: Id<SimpleIdentifier>,
    ) {
        // OK, in comment
        if self.is_in_comment {
            return;
        }
        // OK, target is not a type
        if type_reference.is_none() {
            return;
        }
        let ctx = self.ctx;
        let Some(element) = self.element(name) else {
            return;
        };
        let base = member::base_element(&ctx, element);
        if base.is::<ExecutableElement>() {
            // OK, static
            if member::is_static(&ctx, element) || base.tag() == Tag::Constructor {
                return;
            }
            let name_str = ast_ext::identifier_name(self.ast, name).to_string();
            self.report_at(diag::static_access_to_instance_member(&name_str), name);
        }
    }

    /// Dart `_checkForThrowOfInvalidType(node)`.
    fn check_for_throw_of_invalid_type(&mut self, node: Id<ThrowExpression>) {
        let expression = self.ast[node].expression;
        let Some(ty) = self.static_type(expression) else {
            return;
        };
        let object_none = self.type_system.object_none();
        if !self
            .type_system
            .is_assignable_to(ty, object_none, self.strict_casts())
        {
            self.report_at(
                diag::throw_of_invalid_type(type_arg(&self.ctx, ty)),
                expression,
            );
        }
    }

    /// Dart `_checkForUnnecessaryNullAware(target, operator, kind:)`.
    fn check_for_unnecessary_null_aware(
        &mut self,
        target: Id<Expression>,
        operator: TokenId,
        kind: NullAwareKind,
    ) {
        if self.ast.is::<SuperExpression>(target) {
            return;
        }

        let ctx = self.ctx;
        let mut target_type = self.static_type(target);
        if let Some(extension_override) = self.ast.cast::<ExtensionOverride>(target) {
            let argument_list = self.ast[extension_override].argument_list;
            let arguments = self.ast.list(self.ast[argument_list].arguments).to_vec();
            if arguments.len() == 1 {
                let argument = arguments[0];
                let expression = match self.ast.cast::<NamedArgument>(argument) {
                    Some(named) => self.ast[named].argument_expression,
                    None => Id::from_raw(argument.raw()),
                };
                target_type = Some(self.static_type(expression).unwrap_or(TypeId::DYNAMIC));
            } else {
                return;
            }
        }

        match target_type {
            None => {
                // The "target" might be an identifier that names a type, and
                // the rest of the expression might be a reference to a
                // static member of that type, e.g. `int?.parse(...)`. In
                // which case the diagnostic should be reported.
                if !self.ast.is::<Identifier>(target) {
                    return;
                }
                let Some(target_element) = self.element(target) else {
                    return;
                };
                let target_element = member::base_element(&ctx, target_element);
                if !target_element.is::<InterfaceElement>()
                    && target_element.tag() != Tag::Extension
                    && target_element.tag() != Tag::TypeAlias
                {
                    return;
                }
            }
            Some(target_type) => {
                if !self.type_system.is_strictly_non_nullable(target_type) {
                    // The warning shouldn't be reported because the target
                    // type is potentially nullable.
                    return;
                }
            }
        }

        let previous_operator = if kind.can_participate_in_short_circuiting() {
            self.previous_short_circuiting_operator(Some(target))
        } else {
            None
        };
        let because_of_short_circuiting = previous_operator.is_some();
        // Dart `.withContextMessages(...)` for the short circuiting
        // operator: not ported.
        let locatable = kind.locatable_diagnostic(because_of_short_circuiting);

        if kind == NullAwareKind::IndexExpression {
            let offset = self.ast.tokens.get(operator).offset;
            let next = self.ast.tokens.next(operator);
            let end = self.ast.tokens.get(next).end();
            self.report(locatable.at_offset(offset as usize, (end - offset) as usize));
        } else {
            self.report_at_token(locatable, operator);
        }
    }

    /// Dart `previousShortCircuitingOperator(target)` of
    /// `_checkForUnnecessaryNullAware`: if the operator is not valid because
    /// the target already makes use of a null aware operator, the null
    /// aware operator of the target.
    fn previous_short_circuiting_operator(
        &self,
        target: Option<Id<Expression>>,
    ) -> Option<TokenId> {
        let target = target?;
        if let Some(t) = self.ast.cast::<PropertyAccess>(target) {
            let operator = self.ast[t].operator;
            if self.ast.tokens.ty(operator) == TokenType::QUESTION_PERIOD {
                let real_target = self.property_access_real_target(t);
                return self
                    .previous_short_circuiting_operator(real_target)
                    .or(Some(operator));
            }
        } else if let Some(t) = self.ast.cast::<IndexExpression>(target) {
            if let Some(question) = self.ast[t].question {
                let real_target = self.index_expression_real_target(t);
                return self
                    .previous_short_circuiting_operator(real_target)
                    .or(Some(question));
            }
        } else if let Some(t) = self.ast.cast::<MethodInvocation>(target)
            && let Some(operator) = self.ast[t].operator
            && self.ast.tokens.ty(operator) == TokenType::QUESTION_PERIOD
        {
            let real_target = ast_ext::method_invocation_real_target(self.ast, t);
            return self
                .previous_short_circuiting_operator(real_target)
                .or(Some(operator));
        }
        None
    }

    /// Dart `_checkForUnqualifiedReferenceToNonLocalStaticMember(name)`:
    /// checks that if [name] is a reference to a static member it is
    /// defined in the enclosing class rather than in a superclass.
    fn check_for_unqualified_reference_to_non_local_static_member(
        &mut self,
        name: Id<SimpleIdentifier>,
    ) {
        if let Some(parent) = self.ast.parent(name)
            && (self.ast.is::<DotShorthandPropertyAccess>(parent)
                || self.ast.is::<DotShorthandInvocation>(parent))
        {
            return;
        }

        let ctx = self.ctx;
        let Some(element) = self.write_or_read_element(name.raw()) else {
            return;
        };
        let base = member::base_element(&ctx, element);
        if base.tag() == Tag::TypeParameter {
            return;
        }

        let Some(enclosing_element) = member::enclosing_element(&ctx, element) else {
            return;
        };

        if self.enclosing_class.map(|c| c.raw()) == Some(enclosing_element) {
            return;
        }
        if !enclosing_element.is::<InterfaceElement>() {
            return;
        }
        if base.is::<ExecutableElement>() && !member::is_static(&ctx, element) {
            return;
        }
        if let Some(parent) = self.ast.parent(name)
            && let Some(invocation) = self.ast.cast::<MethodInvocation>(parent)
            && self.ast[invocation].method_name == name
        {
            // Invalid methods are reported in
            // [MethodInvocationResolver._reportInstanceAccessToStaticMember].
            return;
        }
        let display_name = ctx
            .element_name(enclosing_element)
            .unwrap_or("")
            .to_string();
        if self.enclosing_extension.is_some() {
            self.report_at(
                diag::unqualified_reference_to_static_member_of_extended_type(&display_name),
                name,
            );
        } else {
            self.report_at(
                diag::unqualified_reference_to_non_local_static_member(&display_name),
                name,
            );
        }
    }

    /// Dart `_getConstantName(expression)`.
    fn get_constant_name(&self, expression: Id<Expression>) -> Option<String> {
        // TODO(brianwilkerson): Convert this to return the element
        // representing the constant.
        let ast = self.ast;
        let name = ast
            .cast::<SimpleIdentifier>(expression)
            .or_else(|| {
                ast.cast::<PrefixedIdentifier>(expression)
                    .map(|e| ast[e].identifier)
            })
            .or_else(|| {
                ast.cast::<PropertyAccess>(expression)
                    .map(|e| ast[e].property_name)
            })?;
        Some(ast_ext::identifier_name(self.ast, name).to_string())
    }

    /// Dart `_isUnqualifiedReferenceToNonLocalStaticMemberAllowed(identifier)`.
    fn is_unqualified_reference_to_non_local_static_member_allowed(
        &self,
        identifier: Id<SimpleIdentifier>,
    ) -> bool {
        let Some(parent) = self.ast.parent(identifier) else {
            return false;
        };
        // Dart `identifier.inDeclarationContext()`.
        if let Some(import) = self.ast.cast::<ImportDirective>(parent)
            && self.ast[import].prefix == Some(identifier)
        {
            return true;
        }
        if self.ast.is::<Label>(parent)
            && let Some(parent2) = self.ast.parent(parent)
            && (self.ast.is::<Statement>(parent2) || self.ast.is::<SwitchMember>(parent2))
        {
            return true;
        }
        if let Some(p) = self.ast.cast::<Annotation>(parent) {
            return self.ast[p].constructor_name == Some(identifier);
        }
        if self.ast.is::<CommentReference>(parent) {
            return true;
        }
        if let Some(p) = self.ast.cast::<ConstructorName>(parent) {
            return self.ast[p].name == Some(identifier);
        }
        if let Some(p) = self.ast.cast::<MethodInvocation>(parent) {
            return self.ast[p].method_name == identifier;
        }
        if let Some(p) = self.ast.cast::<PrefixedIdentifier>(parent) {
            return self.ast[p].identifier == identifier;
        }
        if let Some(p) = self.ast.cast::<PropertyAccess>(parent) {
            return self.ast[p].property_name == identifier;
        }
        if let Some(p) = self.ast.cast::<SuperConstructorInvocation>(parent) {
            return self.ast[p].constructor_name == Some(identifier);
        }
        false
    }

    /// Dart `_reportMissingAwaitInTryBlock(node)`.
    fn report_missing_await_in_try_block(&mut self, node: Id<ReturnStatement>) {
        async_return_visitor::report_missing_await_in_try_block(self, node);
    }

    /// Dart `_withHiddenElementsForForParts(forLoopParts, f)`.
    fn with_hidden_elements_for_for_parts(
        &mut self,
        for_loop_parts: Id<ForLoopParts>,
        f: impl FnOnce(&mut Self),
    ) {
        if let Some(parts) = self.ast.cast::<ForPartsWithDeclarations>(for_loop_parts) {
            let variables = self.ast[parts].variables;
            let declarations = self.ast.list(self.ast[variables].variables).to_vec();
            let elements: IndexSet<ElementId> = declarations
                .into_iter()
                .filter_map(|v| self.declared_element(v.raw()))
                .collect();
            self.with_hidden_elements(elements, f);
        } else {
            f(self);
        }
    }

    /// Dart `_withHiddenElementsForStatements(statements, f)`.
    fn with_hidden_elements_for_statements(
        &mut self,
        statements: &[Id<Statement>],
        f: impl FnOnce(&mut Self),
    ) {
        let elements = self.elements_in_statements(statements);
        self.with_hidden_elements(elements, f);
    }

    /// Dart `BlockScope.elementsInStatements(statements)`.
    fn elements_in_statements(&self, statements: &[Id<Statement>]) -> IndexSet<ElementId> {
        let mut elements = IndexSet::new();
        for &statement in statements {
            let mut statement = statement;
            if let Some(labeled) = self.ast.cast::<LabeledStatement>(statement) {
                statement = self.ast[labeled].statement;
            }
            if let Some(s) = self
                .ast
                .cast::<PatternVariableDeclarationStatement>(statement)
            {
                let declaration = self.ast[s].declaration;
                elements.extend(self.pattern_variable_declaration_elements(declaration));
            } else if let Some(s) = self.ast.cast::<VariableDeclarationStatement>(statement) {
                let variables = self.ast[s].variables;
                for &variable in self.ast.list(self.ast[variables].variables) {
                    if let Some(e) = self.declared_element(variable.raw()) {
                        elements.insert(e);
                    }
                }
            } else if let Some(s) = self.ast.cast::<FunctionDeclarationStatement>(statement) {
                let declaration = self.ast[s].function_declaration;
                if let Some(e) = self.declared_element(declaration.raw()) {
                    elements.insert(e);
                }
            }
        }
        elements
    }

    /// Dart `_withHiddenElementsGuardedPattern(guardedPattern, f)`.
    fn with_hidden_elements_guarded_pattern(
        &mut self,
        guarded_pattern: Id<GuardedPattern>,
        f: impl FnOnce(&mut Self),
    ) {
        let elements: IndexSet<ElementId> = self
            .rt
            .guarded_pattern_variables
            .get(guarded_pattern)
            .map(|variables| variables.values().copied().collect())
            .unwrap_or_default();
        self.with_hidden_elements(elements, f);
    }

    // Helpers that other sections call.

    /// Dart `_checkForAmbiguousImport(name:, element:)`.
    pub(crate) fn check_for_ambiguous_import(&mut self, name: TokenId, element: Option<ElemRef>) {
        let Some(ElemRef::Base(element)) = element else {
            return;
        };
        let Some(multiply_defined) = element.cast::<dartr_element::MultiplyDefinedElement>() else {
            return;
        };
        let ctx = self.ctx;
        let conflicting_members = ctx.get(multiply_defined).conflicting_elements.clone();
        let mut library_names: Vec<String> = conflicting_members
            .iter()
            .map(|&e| self.get_library_name(Some(e)))
            .collect();
        library_names.sort();
        let lexeme = self.ast.tokens.lexeme(name).to_string();
        self.report_at_token(
            diag::ambiguous_import(
                &lexeme,
                &quoted_and_comma_separated_with_and(&library_names),
            ),
            name,
        );
    }

    /// Dart `ErrorVerifier.getTypeReference(expression)`: the class that
    /// [expression] references (an identifier of a class, or of a type
    /// alias of an interface type).
    pub(crate) fn get_type_reference(
        &self,
        expression: Id<Expression>,
    ) -> Option<EId<InterfaceElement>> {
        if !self.ast.is::<Identifier>(expression) {
            return None;
        }
        let ctx = self.ctx;
        let element = member::base_element(&ctx, self.element(expression)?);
        if let Some(interface) = element.cast::<InterfaceElement>() {
            return Some(interface);
        }
        if let Some(alias) = element.cast::<TypeAliasElement>() {
            let aliased_type = ctx.get(alias).aliased_type.get()?;
            if let TypeKind::Interface { element, .. } = *ctx.ty(aliased_type) {
                return Some(element);
            }
        }
        None
    }
}

/// Dart `Iterable<String>.quotedAndCommaSeparatedWithAnd`.
fn quoted_and_comma_separated_with_and(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [first] => format!("'{first}'"),
        [first, second] => format!("'{first}' and '{second}'"),
        [rest @ .., last] => {
            let mut buffer = rest
                .iter()
                .map(|s| format!("'{s}'"))
                .collect::<Vec<_>>()
                .join(", ");
            buffer.push_str(&format!(", and '{last}'"));
            buffer
        }
    }
}

/// Dart `IntegerLiteralImpl.isValidAsInteger`, `isValidAsDouble` and
/// `nearestValidDouble` (with a small unsigned big integer, Dart
/// `BigInt`).
mod integer_literal {
    /// An unsigned big integer: little-endian 32-bit limbs.
    struct BigUint(Vec<u32>);

    impl BigUint {
        /// Dart `BigInt.tryParse(source)` for a decimal or `0x`
        /// hexadecimal literal without a sign.
        fn parse(source: &str) -> Option<BigUint> {
            let (digits, radix) = match source
                .strip_prefix("0x")
                .or_else(|| source.strip_prefix("0X"))
            {
                Some(hex) => (hex, 16u32),
                None => (source, 10u32),
            };
            if digits.is_empty() {
                return None;
            }
            let mut limbs: Vec<u32> = Vec::new();
            for c in digits.chars() {
                let digit = c.to_digit(radix)?;
                let mut carry = digit as u64;
                for limb in limbs.iter_mut() {
                    let v = (*limb as u64) * radix as u64 + carry;
                    *limb = v as u32;
                    carry = v >> 32;
                }
                if carry != 0 {
                    limbs.push(carry as u32);
                }
            }
            while limbs.last() == Some(&0) {
                limbs.pop();
            }
            Some(BigUint(limbs))
        }

        fn bit_length(&self) -> u64 {
            match self.0.last() {
                None => 0,
                Some(&top) => (self.0.len() as u64 - 1) * 32 + (32 - top.leading_zeros() as u64),
            }
        }

        fn bit(&self, i: u64) -> bool {
            let limb = (i / 32) as usize;
            limb < self.0.len() && (self.0[limb] >> (i % 32)) & 1 == 1
        }

        /// Whether the bits below [n] are all zero.
        fn low_bits_zero(&self, n: u64) -> bool {
            (0..n).all(|i| !self.bit(i))
        }

        /// The value as a `u128`, if it fits.
        fn to_u128(&self) -> Option<u128> {
            if self.0.len() > 4 {
                return None;
            }
            let mut v: u128 = 0;
            for &limb in self.0.iter().rev() {
                v = (v << 32) | limb as u128;
            }
            Some(v)
        }

        /// Dart `BigInt.toDouble()` (round to nearest, ties to even;
        /// infinity when too large).
        fn to_f64(&self) -> f64 {
            let bits = self.bit_length();
            if bits <= 64 {
                return self.to_u128().map(|v| v as f64).unwrap_or(f64::INFINITY);
            }
            if bits > 1024 {
                return f64::INFINITY;
            }
            // The top 53 bits, then round with the next bit and the sticky
            // bits.
            let shift = bits - 53;
            let mut mantissa: u64 = 0;
            for i in (shift..bits).rev() {
                mantissa = (mantissa << 1) | self.bit(i) as u64;
            }
            let round_bit = self.bit(shift - 1);
            let sticky = !self.low_bits_zero(shift - 1);
            if round_bit && (sticky || mantissa & 1 == 1) {
                mantissa += 1;
            }
            let mut exponent = shift as i32;
            if mantissa == 1 << 53 {
                mantissa >>= 1;
                exponent += 1;
            }
            if exponent + 53 > 1024 {
                return f64::INFINITY;
            }
            (mantissa as f64) * 2f64.powi(exponent)
        }
    }

    /// Dart `IntegerLiteralImpl.isValidAsDouble(source)`.
    pub fn is_valid_as_double(source: &str) -> bool {
        // Less than 16 characters must be a valid double since it's less
        // than 9007199254740992, 0x10000000000000, both 16 characters and
        // 53 bits.
        if source.len() < 16 {
            return true;
        }
        let Some(full_precision) = BigUint::parse(source) else {
            return false;
        };
        let bit_length = full_precision.bit_length();
        if bit_length <= 53 {
            return true;
        }
        // This would overflow the exponent (larger than maximum double).
        // A value with at most 53 significant bits is larger than
        // `double.maxFinite` exactly when it has more than 1024 bits.
        if bit_length > 1024 {
            return false;
        }
        full_precision.low_bits_zero(bit_length - 53)
    }

    /// Dart `IntegerLiteralImpl.isValidAsInteger(source, isNegative)`
    /// (Dart `int.tryParse` of the VM: decimal literals in the 64-bit
    /// range, hexadecimal literals up to 2^64 - 1, or down to -2^63 when
    /// negative).
    pub fn is_valid_as_integer(source: &str, is_negative: bool) -> bool {
        let is_hex = source.starts_with("0x") || source.starts_with("0X");
        let Some(value) = BigUint::parse(source).and_then(|v| v.to_u128()) else {
            return false;
        };
        if is_negative {
            value <= 1u128 << 63
        } else if is_hex {
            value <= u64::MAX as u128
        } else {
            value <= i64::MAX as u128
        }
    }

    /// Dart `BigInt.from(IntegerLiteralImpl.nearestValidDouble(source))
    /// .toString()`.
    pub fn nearest_valid_double_string(source: &str) -> String {
        let Some(value) = BigUint::parse(source) else {
            return String::new();
        };
        let d = value.to_f64().min(f64::MAX);
        double_to_integer_string(d)
    }

    /// The decimal digits of the integer value of [d] (a finite,
    /// non-negative whole number).
    fn double_to_integer_string(d: f64) -> String {
        let bits = d.to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as i32;
        let fraction = bits & ((1 << 52) - 1);
        if exponent == 0 {
            return "0".to_string();
        }
        let mantissa = fraction | (1 << 52);
        let shift = exponent - 1075;
        // The value is mantissa * 2^shift; shift >= 0 for the values here.
        let mut limbs: Vec<u32> = vec![mantissa as u32, (mantissa >> 32) as u32];
        if shift < 0 {
            let v = mantissa >> (-shift);
            return v.to_string();
        }
        for _ in 0..shift {
            let mut carry = 0u32;
            for limb in limbs.iter_mut() {
                let v = ((*limb as u64) << 1) | carry as u64;
                *limb = v as u32;
                carry = (v >> 32) as u32;
            }
            if carry != 0 {
                limbs.push(carry);
            }
        }
        // Repeated division by 10^9.
        let mut digits = Vec::<String>::new();
        while limbs.iter().any(|&l| l != 0) {
            let mut remainder = 0u64;
            for limb in limbs.iter_mut().rev() {
                let v = (remainder << 32) | *limb as u64;
                *limb = (v / 1_000_000_000) as u32;
                remainder = v % 1_000_000_000;
            }
            digits.push(format!("{remainder:09}"));
            while limbs.last() == Some(&0) {
                limbs.pop();
            }
        }
        let mut s: String = digits.into_iter().rev().collect();
        let trimmed = s.trim_start_matches('0');
        s = if trimmed.is_empty() {
            "0".to_string()
        } else {
            trimmed.to_string()
        };
        s
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn validity_of_integer_literals() {
            assert!(is_valid_as_integer("9223372036854775807", false));
            assert!(!is_valid_as_integer("9223372036854775808", false));
            assert!(is_valid_as_integer("9223372036854775808", true));
            assert!(is_valid_as_integer("0xFFFFFFFFFFFFFFFF", false));
            assert!(!is_valid_as_integer("0xFFFFFFFFFFFFFFFF", true));
            assert!(!is_valid_as_integer("0x10000000000000000", false));
            assert!(is_valid_as_double("9007199254740992"));
            assert!(!is_valid_as_double("9007199254740993"));
            assert_eq!(
                nearest_valid_double_string("9007199254740993"),
                "9007199254740992"
            );
        }
    }
}
