// Dart source: pkg/analyzer/lib/src/dart/ast/ast.dart (resolvePattern and
// computePatternSchema of the DartPatternImpl subclasses)

//! The pattern dispatch targets of [`ResolverVisitor`]: Dart
//! `DartPatternImpl.resolvePattern` and `computePatternSchema` of each
//! pattern kind. The helpers they share (`resolveMapPattern`,
//! `buildSharedPatternFields`, `checkPatternNeverMatchesValueType`, ...) are
//! in [`crate::pattern_resolver`].
//!
//! `inferenceLogWriter?.enterPattern` / `exitPattern` (the inference log
//! for tests) is not ported.

use dartr_ast::*;
use dartr_element::{PromotableElement, TypeId};
use dartr_flow::shared_type::SharedTypeView;
use dartr_flow::type_analyzer::TypeAnalyzer;

use crate::element_ext;
use crate::list_pattern_resolver;
use crate::pattern_resolver::{
    self, annotation_type, build_shared_pattern_fields, check_pattern_never_matches_value_type,
};
use crate::resolver::{PatternResultOf, ResolverVisitor, SchemaOf, SharedMatchContext};

impl<'a> ResolverVisitor<'a> {
    /// Dart `AssignedVariablePatternImpl.resolvePattern`.
    pub(crate) fn resolve_pattern_assigned_variable_pattern(
        &mut self,
        node: Id<AssignedVariablePattern>,
        context: &SharedMatchContext,
    ) -> PatternResultOf {
        pattern_resolver::resolve_assigned_variable_pattern(self, node, context)
    }

    /// Dart `AssignedVariablePatternImpl.computePatternSchema`.
    pub(crate) fn compute_pattern_schema_assigned_variable_pattern(
        &mut self,
        node: Id<AssignedVariablePattern>,
    ) -> SchemaOf {
        let element = self
            .base_element(node)
            .and_then(|e| e.cast::<PromotableElement>());
        match element {
            // The schema reads the flow analysis state of the variable.
            Some(element) if self.flow_analysis.is_active() => {
                self.analyze_assigned_variable_pattern_schema(element)
            }
            _ => SchemaOf::new(TypeId::UNKNOWN),
        }
    }

    /// Dart `CastPatternImpl.resolvePattern`.
    pub(crate) fn resolve_pattern_cast_pattern(
        &mut self,
        node: Id<CastPattern>,
        context: &SharedMatchContext,
    ) -> PatternResultOf {
        let type_ = self.ast[node].type_;
        self.visit_node(type_.raw());
        let required_type = annotation_type(self, type_.raw());
        let inner_pattern = self.ast[node].pattern;

        let analysis_result = self.analyze_cast_pattern(
            context,
            node.upcast(),
            inner_pattern,
            SharedTypeView::new(required_type),
        );

        check_pattern_never_matches_value_type(
            self,
            context,
            node.upcast(),
            required_type,
            analysis_result.matched_value_type.unwrap_type_view(),
        );
        analysis_result
    }

    /// Dart `CastPatternImpl.computePatternSchema`.
    pub(crate) fn compute_pattern_schema_cast_pattern(
        &mut self,
        node: Id<CastPattern>,
    ) -> SchemaOf {
        let _ = node;
        self.analyze_cast_pattern_schema()
    }

    /// Dart `ConstantPatternImpl.resolvePattern`.
    pub(crate) fn resolve_pattern_constant_pattern(
        &mut self,
        node: Id<ConstantPattern>,
        context: &SharedMatchContext,
    ) -> PatternResultOf {
        let expression = self.ast[node].expression;
        let analysis_result = self.analyze_constant_pattern(context, node.raw(), expression);
        // Dart `expression = popRewrite()!`: the rewrite already replaced the
        // expression in the AST.
        self.pop_rewrite();
        analysis_result.into()
    }

    /// Dart `ConstantPatternImpl.computePatternSchema`.
    pub(crate) fn compute_pattern_schema_constant_pattern(
        &mut self,
        node: Id<ConstantPattern>,
    ) -> SchemaOf {
        let _ = node;
        self.analyze_constant_pattern_schema()
    }

    /// Dart `DeclaredVariablePatternImpl.resolvePattern`.
    pub(crate) fn resolve_pattern_declared_variable_pattern(
        &mut self,
        node: Id<DeclaredVariablePattern>,
        context: &SharedMatchContext,
    ) -> PatternResultOf {
        let declared_type = self.ast[node]
            .type_
            .map(|t| SharedTypeView::new(annotation_type(self, t.raw())));
        let declared_element = self
            .declared_element(node)
            .and_then(|e| e.cast::<PromotableElement>());
        let Some(declared_element) = declared_element else {
            // Recovery: the element binding pass did not bind the variable.
            // Dart has always a variable here; without one the pattern is
            // analyzed like a wildcard pattern with the same type.
            let context = context.with_unnecessary_wildcard_kind(None);
            return self
                .analyze_wildcard_pattern(&context, node.upcast(), declared_type)
                .into();
        };
        // Dart `declaredElement.name ?? ''`.
        let name = match self
            .ctx
            .element_data(declared_element.raw())
            .and_then(|d| d.name)
        {
            Some(name) => name,
            None => self.ctx.name(""),
        };
        let result = self.analyze_declared_variable_pattern(
            context,
            node.upcast(),
            declared_element,
            name,
            declared_type,
        );
        let static_type = result.static_type.unwrap_type_view();
        if let Some(local) = declared_element
            .raw()
            .cast::<dartr_element::LocalVariableElement>()
        {
            element_ext::set_local_variable_type(&self.ctx, local, static_type);
        }

        check_pattern_never_matches_value_type(
            self,
            context,
            node.upcast(),
            static_type,
            result.matched_value_type.unwrap_type_view(),
        );
        result.into()
    }

    /// Dart `DeclaredVariablePatternImpl.computePatternSchema`.
    pub(crate) fn compute_pattern_schema_declared_variable_pattern(
        &mut self,
        node: Id<DeclaredVariablePattern>,
    ) -> SchemaOf {
        let declared_type = self.ast[node]
            .type_
            .map(|t| SharedTypeView::new(annotation_type(self, t.raw())));
        self.analyze_declared_variable_pattern_schema(declared_type)
    }

    /// Dart `ListPatternImpl.resolvePattern`.
    pub(crate) fn resolve_pattern_list_pattern(
        &mut self,
        node: Id<ListPattern>,
        context: &SharedMatchContext,
    ) -> PatternResultOf {
        list_pattern_resolver::resolve(self, node, context)
    }

    /// Dart `ListPatternImpl.computePatternSchema`.
    pub(crate) fn compute_pattern_schema_list_pattern(
        &mut self,
        node: Id<ListPattern>,
    ) -> SchemaOf {
        let element_type = self.ast[node]
            .type_arguments
            .and_then(|t| {
                pattern_resolver::type_argument_types(self, t)
                    .first()
                    .copied()
            })
            .map(SharedTypeView::new);
        let elements = self.ast.list_raw(self.ast[node].elements).to_vec();
        self.analyze_list_pattern_schema(element_type, &elements)
    }

    /// Dart `LogicalAndPatternImpl.resolvePattern`.
    pub(crate) fn resolve_pattern_logical_and_pattern(
        &mut self,
        node: Id<LogicalAndPattern>,
        context: &SharedMatchContext,
    ) -> PatternResultOf {
        let left = self.ast[node].left_operand;
        let right = self.ast[node].right_operand;
        self.analyze_logical_and_pattern(context, node.upcast(), left.raw(), right.raw())
    }

    /// Dart `LogicalAndPatternImpl.computePatternSchema`.
    pub(crate) fn compute_pattern_schema_logical_and_pattern(
        &mut self,
        node: Id<LogicalAndPattern>,
    ) -> SchemaOf {
        let left = self.ast[node].left_operand;
        let right = self.ast[node].right_operand;
        self.analyze_logical_and_pattern_schema(left.raw(), right.raw())
    }

    /// Dart `LogicalOrPatternImpl.resolvePattern`.
    pub(crate) fn resolve_pattern_logical_or_pattern(
        &mut self,
        node: Id<LogicalOrPattern>,
        context: &SharedMatchContext,
    ) -> PatternResultOf {
        let left = self.ast[node].left_operand;
        let right = self.ast[node].right_operand;
        let analysis_result =
            self.analyze_logical_or_pattern(context, node.upcast(), left.raw(), right.raw());
        // Dart `nullSafetyDeadCodeVerifier.flowEnd(rightOperand)`: wave D.
        analysis_result.into()
    }

    /// Dart `LogicalOrPatternImpl.computePatternSchema`.
    pub(crate) fn compute_pattern_schema_logical_or_pattern(
        &mut self,
        node: Id<LogicalOrPattern>,
    ) -> SchemaOf {
        let left = self.ast[node].left_operand;
        let right = self.ast[node].right_operand;
        self.analyze_logical_or_pattern_schema(left.raw(), right.raw())
    }

    /// Dart `MapPatternImpl.resolvePattern`.
    pub(crate) fn resolve_pattern_map_pattern(
        &mut self,
        node: Id<MapPattern>,
        context: &SharedMatchContext,
    ) -> PatternResultOf {
        pattern_resolver::resolve_map_pattern(self, node, context)
    }

    /// Dart `MapPatternImpl.computePatternSchema`.
    pub(crate) fn compute_pattern_schema_map_pattern(&mut self, node: Id<MapPattern>) -> SchemaOf {
        let type_arguments = pattern_resolver::map_pattern_type_arguments(self, node);
        let elements = self.ast.list_raw(self.ast[node].elements).to_vec();
        self.analyze_map_pattern_schema(type_arguments, &elements)
    }

    /// Dart `NullAssertPatternImpl.resolvePattern`.
    pub(crate) fn resolve_pattern_null_assert_pattern(
        &mut self,
        node: Id<NullAssertPattern>,
        context: &SharedMatchContext,
    ) -> PatternResultOf {
        let pattern = self.ast[node].pattern;
        self.analyze_null_check_or_assert_pattern(context, node.upcast(), pattern, true)
            .into()
    }

    /// Dart `NullAssertPatternImpl.computePatternSchema`.
    pub(crate) fn compute_pattern_schema_null_assert_pattern(
        &mut self,
        node: Id<NullAssertPattern>,
    ) -> SchemaOf {
        let pattern = self.ast[node].pattern;
        self.analyze_null_check_or_assert_pattern_schema(pattern, true)
    }

    /// Dart `NullCheckPatternImpl.resolvePattern`.
    pub(crate) fn resolve_pattern_null_check_pattern(
        &mut self,
        node: Id<NullCheckPattern>,
        context: &SharedMatchContext,
    ) -> PatternResultOf {
        let pattern = self.ast[node].pattern;
        self.analyze_null_check_or_assert_pattern(context, node.upcast(), pattern, false)
            .into()
    }

    /// Dart `NullCheckPatternImpl.computePatternSchema`.
    pub(crate) fn compute_pattern_schema_null_check_pattern(
        &mut self,
        node: Id<NullCheckPattern>,
    ) -> SchemaOf {
        let pattern = self.ast[node].pattern;
        self.analyze_null_check_or_assert_pattern_schema(pattern, false)
    }

    /// Dart `ObjectPatternImpl.resolvePattern`.
    pub(crate) fn resolve_pattern_object_pattern(
        &mut self,
        node: Id<ObjectPattern>,
        context: &SharedMatchContext,
    ) -> PatternResultOf {
        let fields = build_shared_pattern_fields(self, self.ast[node].fields, true);
        let result = self.analyze_object_pattern(context, node.upcast(), &fields);

        check_pattern_never_matches_value_type(
            self,
            context,
            node.upcast(),
            result.required_type.unwrap_type_view(),
            result.matched_value_type.unwrap_type_view(),
        );
        result.into()
    }

    /// Dart `ObjectPatternImpl.computePatternSchema`.
    pub(crate) fn compute_pattern_schema_object_pattern(
        &mut self,
        node: Id<ObjectPattern>,
    ) -> SchemaOf {
        let type_ = annotation_type(self, self.ast[node].type_.raw());
        self.analyze_object_pattern_schema(SharedTypeView::new(type_))
    }

    /// Dart `ParenthesizedPatternImpl.resolvePattern`.
    pub(crate) fn resolve_pattern_parenthesized_pattern(
        &mut self,
        node: Id<ParenthesizedPattern>,
        context: &SharedMatchContext,
    ) -> PatternResultOf {
        let pattern = self.ast[node].pattern;
        self.dispatch_pattern(context, pattern.raw())
    }

    /// Dart `ParenthesizedPatternImpl.computePatternSchema`.
    pub(crate) fn compute_pattern_schema_parenthesized_pattern(
        &mut self,
        node: Id<ParenthesizedPattern>,
    ) -> SchemaOf {
        let pattern = self.ast[node].pattern;
        self.dispatch_pattern_schema(pattern.raw())
    }

    /// Dart `RecordPatternImpl.resolvePattern`.
    pub(crate) fn resolve_pattern_record_pattern(
        &mut self,
        node: Id<RecordPattern>,
        context: &SharedMatchContext,
    ) -> PatternResultOf {
        let fields = build_shared_pattern_fields(self, self.ast[node].fields, false);
        let result = self.analyze_record_pattern(context, node.upcast(), &fields);

        // Dart `hasDuplicateNamedField` (set by the errors object).
        let pattern: Id<DartPattern> = node.upcast();
        if !self.errors.has_duplicate_named_field.contains(&pattern) {
            check_pattern_never_matches_value_type(
                self,
                context,
                pattern,
                result.required_type.unwrap_type_view(),
                result.matched_value_type.unwrap_type_view(),
            );
        }
        result.into()
    }

    /// Dart `RecordPatternImpl.computePatternSchema`.
    pub(crate) fn compute_pattern_schema_record_pattern(
        &mut self,
        node: Id<RecordPattern>,
    ) -> SchemaOf {
        let fields = build_shared_pattern_fields(self, self.ast[node].fields, false);
        self.analyze_record_pattern_schema(&fields)
    }

    /// Dart `RelationalPatternImpl.resolvePattern`.
    pub(crate) fn resolve_pattern_relational_pattern(
        &mut self,
        node: Id<RelationalPattern>,
        context: &SharedMatchContext,
    ) -> PatternResultOf {
        let operand = self.ast[node].operand;
        let analysis_result = self.analyze_relational_pattern(context, node.upcast(), operand);
        self.pop_rewrite();
        analysis_result.into()
    }

    /// Dart `RelationalPatternImpl.computePatternSchema`.
    pub(crate) fn compute_pattern_schema_relational_pattern(
        &mut self,
        node: Id<RelationalPattern>,
    ) -> SchemaOf {
        let _ = node;
        self.analyze_relational_pattern_schema()
    }

    /// Dart `WildcardPatternImpl.resolvePattern`.
    pub(crate) fn resolve_pattern_wildcard_pattern(
        &mut self,
        node: Id<WildcardPattern>,
        context: &SharedMatchContext,
    ) -> PatternResultOf {
        let declared_type = self.ast[node].type_.map(|t| annotation_type(self, t.raw()));
        let analysis_result = self.analyze_wildcard_pattern(
            context,
            node.upcast(),
            declared_type.map(SharedTypeView::new),
        );

        if let Some(declared_type) = declared_type {
            check_pattern_never_matches_value_type(
                self,
                context,
                node.upcast(),
                declared_type,
                analysis_result.matched_value_type.unwrap_type_view(),
            );
        }
        analysis_result.into()
    }

    /// Dart `WildcardPatternImpl.computePatternSchema`.
    pub(crate) fn compute_pattern_schema_wildcard_pattern(
        &mut self,
        node: Id<WildcardPattern>,
    ) -> SchemaOf {
        let declared_type = self.ast[node]
            .type_
            .map(|t| SharedTypeView::new(annotation_type(self, t.raw())));
        self.analyze_declared_variable_pattern_schema(declared_type)
    }
}
