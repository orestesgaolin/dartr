// Dart source: pkg/analyzer/lib/src/generated/error_verifier.dart
// (ErrorVerifier section: statements and expressions (D5))

//! An `ErrorVerifier` section (see the module documentation of
//! [`super`]). The `visit_x` methods are the Dart `visitX` overrides; the
//! body `self.visit_children(node)` is Dart `super.visitX(node)`.

use dartr_ast::*;
use dartr_element::{EId, ElemRef, InterfaceElement};
use dartr_syntax::TokenId;

use super::ErrorVerifier;

impl ErrorVerifier<'_> {
    /// Dart `visitAnnotation`.
    pub(super) fn visit_annotation(&mut self, node: Id<Annotation>) {
        self.visit_children(node);
    }

    /// Dart `visitAnonymousMethodInvocation`.
    pub(super) fn visit_anonymous_method_invocation(
        &mut self,
        node: Id<AnonymousMethodInvocation>,
    ) {
        self.visit_children(node);
    }

    /// Dart `visitAsExpression`.
    pub(super) fn visit_as_expression(&mut self, node: Id<AsExpression>) {
        self.visit_children(node);
    }

    /// Dart `visitAssignedVariablePattern`.
    pub(super) fn visit_assigned_variable_pattern(&mut self, node: Id<AssignedVariablePattern>) {
        self.visit_children(node);
    }

    /// Dart `visitAssignmentExpression`.
    pub(super) fn visit_assignment_expression(&mut self, node: Id<AssignmentExpression>) {
        self.visit_children(node);
    }

    /// Dart `visitAwaitExpression`.
    pub(super) fn visit_await_expression(&mut self, node: Id<AwaitExpression>) {
        self.visit_children(node);
    }

    /// Dart `visitBinaryExpression`.
    pub(super) fn visit_binary_expression(&mut self, node: Id<BinaryExpression>) {
        self.visit_children(node);
    }

    /// Dart `visitBlock`.
    pub(super) fn visit_block(&mut self, node: Id<Block>) {
        self.visit_children(node);
    }

    /// Dart `visitBreakStatement`.
    pub(super) fn visit_break_statement(&mut self, node: Id<BreakStatement>) {
        self.visit_children(node);
    }

    /// Dart `visitCatchClause`.
    pub(super) fn visit_catch_clause(&mut self, node: Id<CatchClause>) {
        self.visit_children(node);
    }

    /// Dart `visitConstructorReference`.
    pub(super) fn visit_constructor_reference(&mut self, node: Id<ConstructorReference>) {
        self.visit_children(node);
    }

    /// Dart `visitDotShorthandInvocation`.
    pub(super) fn visit_dot_shorthand_invocation(&mut self, node: Id<DotShorthandInvocation>) {
        self.visit_children(node);
    }

    /// Dart `visitExpressionFunctionBody`.
    pub(super) fn visit_expression_function_body(&mut self, node: Id<ExpressionFunctionBody>) {
        self.visit_children(node);
    }

    /// Dart `visitForEachPartsWithDeclaration`.
    pub(super) fn visit_for_each_parts_with_declaration(
        &mut self,
        node: Id<ForEachPartsWithDeclaration>,
    ) {
        self.visit_children(node);
    }

    /// Dart `visitForEachPartsWithIdentifier`.
    pub(super) fn visit_for_each_parts_with_identifier(
        &mut self,
        node: Id<ForEachPartsWithIdentifier>,
    ) {
        self.visit_children(node);
    }

    /// Dart `visitForElement`.
    pub(super) fn visit_for_element(&mut self, node: Id<ForElement>) {
        self.visit_children(node);
    }

    /// Dart `visitForPartsWithDeclarations`.
    pub(super) fn visit_for_parts_with_declarations(&mut self, node: Id<ForPartsWithDeclarations>) {
        self.visit_children(node);
    }

    /// Dart `visitForStatement`.
    pub(super) fn visit_for_statement(&mut self, node: Id<ForStatement>) {
        self.visit_children(node);
    }

    /// Dart `visitFunctionExpressionInvocation`.
    pub(super) fn visit_function_expression_invocation(
        &mut self,
        node: Id<FunctionExpressionInvocation>,
    ) {
        self.visit_children(node);
    }

    /// Dart `visitFunctionReference`.
    pub(super) fn visit_function_reference(&mut self, node: Id<FunctionReference>) {
        self.visit_children(node);
    }

    /// Dart `visitGuardedPattern`.
    pub(super) fn visit_guarded_pattern(&mut self, node: Id<GuardedPattern>) {
        self.visit_children(node);
    }

    /// Dart `visitImportPrefixReference`.
    pub(super) fn visit_import_prefix_reference(&mut self, node: Id<ImportPrefixReference>) {
        self.visit_children(node);
    }

    /// Dart `visitIndexExpression`.
    pub(super) fn visit_index_expression(&mut self, node: Id<IndexExpression>) {
        self.visit_children(node);
    }

    /// Dart `visitIntegerLiteral`.
    pub(super) fn visit_integer_literal(&mut self, node: Id<IntegerLiteral>) {
        self.visit_children(node);
    }

    /// Dart `visitInterpolationExpression`.
    pub(super) fn visit_interpolation_expression(&mut self, node: Id<InterpolationExpression>) {
        self.visit_children(node);
    }

    /// Dart `visitIsExpression`.
    pub(super) fn visit_is_expression(&mut self, node: Id<IsExpression>) {
        self.visit_children(node);
    }

    /// Dart `visitListLiteral`.
    pub(super) fn visit_list_literal(&mut self, node: Id<ListLiteral>) {
        self.visit_children(node);
    }

    /// Dart `visitMapLiteralEntry`.
    pub(super) fn visit_map_literal_entry(&mut self, node: Id<MapLiteralEntry>) {
        self.visit_children(node);
    }

    /// Dart `visitMethodInvocation`.
    pub(super) fn visit_method_invocation(&mut self, node: Id<MethodInvocation>) {
        self.visit_children(node);
    }

    /// Dart `visitNativeFunctionBody`.
    pub(super) fn visit_native_function_body(&mut self, node: Id<NativeFunctionBody>) {
        self.visit_children(node);
    }

    /// Dart `visitNullAwareElement`.
    pub(super) fn visit_null_aware_element(&mut self, node: Id<NullAwareElement>) {
        self.visit_children(node);
    }

    /// Dart `visitPatternVariableDeclarationStatement`.
    pub(super) fn visit_pattern_variable_declaration_statement(
        &mut self,
        node: Id<PatternVariableDeclarationStatement>,
    ) {
        self.visit_children(node);
    }

    /// Dart `visitPostfixExpression`.
    pub(super) fn visit_postfix_expression(&mut self, node: Id<PostfixExpression>) {
        self.visit_children(node);
    }

    /// Dart `visitPrefixedIdentifier`.
    pub(super) fn visit_prefixed_identifier(&mut self, node: Id<PrefixedIdentifier>) {
        self.visit_children(node);
    }

    /// Dart `visitPrefixExpression`.
    pub(super) fn visit_prefix_expression(&mut self, node: Id<PrefixExpression>) {
        self.visit_children(node);
    }

    /// Dart `visitPropertyAccess`.
    pub(super) fn visit_property_access(&mut self, node: Id<PropertyAccess>) {
        self.visit_children(node);
    }

    /// Dart `visitRethrowExpression`.
    pub(super) fn visit_rethrow_expression(&mut self, node: Id<RethrowExpression>) {
        self.visit_children(node);
    }

    /// Dart `visitReturnStatement`.
    pub(super) fn visit_return_statement(&mut self, node: Id<ReturnStatement>) {
        self.visit_children(node);
    }

    /// Dart `visitSetOrMapLiteral`.
    pub(super) fn visit_set_or_map_literal(&mut self, node: Id<SetOrMapLiteral>) {
        self.visit_children(node);
    }

    /// Dart `visitSimpleIdentifier`.
    pub(super) fn visit_simple_identifier(&mut self, node: Id<SimpleIdentifier>) {
        self.visit_children(node);
    }

    /// Dart `visitSpreadElement`.
    pub(super) fn visit_spread_element(&mut self, node: Id<SpreadElement>) {
        self.visit_children(node);
    }

    /// Dart `visitSwitchCase`.
    pub(super) fn visit_switch_case(&mut self, node: Id<SwitchCase>) {
        self.visit_children(node);
    }

    /// Dart `visitSwitchDefault`.
    pub(super) fn visit_switch_default(&mut self, node: Id<SwitchDefault>) {
        self.visit_children(node);
    }

    /// Dart `visitSwitchExpression`.
    pub(super) fn visit_switch_expression(&mut self, node: Id<SwitchExpression>) {
        self.visit_children(node);
    }

    /// Dart `visitSwitchPatternCase`.
    pub(super) fn visit_switch_pattern_case(&mut self, node: Id<SwitchPatternCase>) {
        self.visit_children(node);
    }

    /// Dart `visitSwitchStatement`.
    pub(super) fn visit_switch_statement(&mut self, node: Id<SwitchStatement>) {
        self.visit_children(node);
    }

    /// Dart `visitThisExpression`.
    pub(super) fn visit_this_expression(&mut self, node: Id<ThisExpression>) {
        self.visit_children(node);
    }

    /// Dart `visitThrowExpression`.
    pub(super) fn visit_throw_expression(&mut self, node: Id<ThrowExpression>) {
        self.visit_children(node);
    }

    /// Dart `visitVariableDeclarationStatement`.
    pub(super) fn visit_variable_declaration_statement(
        &mut self,
        node: Id<VariableDeclarationStatement>,
    ) {
        self.visit_children(node);
    }

    // The helpers of this section (Dart private methods) to port here:
    // _checkForAssignmentToFinal, _checkForAssignmentToPrimaryConstructorParameter, _checkForAwaitInLateLocalVariableInitializer
    // _checkForAwaitOfIncompatibleType, _checkForConstEvalThrowsException, _checkForDeadNullCoalesce
    // _checkForEachParts, _checkForInstanceAccessToStaticMember, _checkForIntNotAssignable
    // _checkForInvalidAnnotationFromDeferredLibrary, _checkForInvalidInstanceMemberAccess, _checkForInvalidReferenceToThis
    // _checkForListElementTypeNotAssignable, _checkForMapTypeNotAssignable, _checkForMissingEnumConstantInSwitch
    // _checkForNativeFunctionBodyInNonSdkCode, _checkForNonConstMapAsExpressionStatement3, _checkForOutOfRange
    // _checkForReferenceBeforeDeclaration, _checkForRethrowOutsideCatch, _checkForSetElementTypeNotAssignable3
    // _checkForStaticAccessToInstanceMember, _checkForThrowOfInvalidType, _checkForUnnecessaryNullAware
    // _checkForUnqualifiedReferenceToNonLocalStaticMember, _getConstantName, _isUnqualifiedReferenceToNonLocalStaticMemberAllowed
    // _reportMissingAwaitInTryBlock, _withHiddenElements, _withHiddenElementsForForParts
    // _withHiddenElementsForStatements, _withHiddenElementsGuardedPattern, getTypeReference

    // Helpers that other sections call. STUB (D5): they report nothing yet;
    // keep the signatures (or update the callers) when porting them.

    /// Dart `_checkForAmbiguousImport(name:, element:)`.
    pub(crate) fn check_for_ambiguous_import(&mut self, name: TokenId, element: Option<ElemRef>) {
        let _ = (name, element);
    }

    /// Dart `ErrorVerifier.getTypeReference(expression)`: the class that
    /// [expression] references (an identifier of a class, or of a type
    /// alias of an interface type).
    pub(crate) fn get_type_reference(
        &self,
        expression: Id<Expression>,
    ) -> Option<EId<InterfaceElement>> {
        let _ = expression;
        None
    }
}
