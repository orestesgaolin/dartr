// Dart source: pkg/analyzer/lib/src/dart/resolver/shared_type_analyzer.dart

//! [`SharedTypeAnalyzerErrors`]: the errors that the shared type analyzer
//! reports, converted to the analyzer's diagnostics.
//!
//! The Dart class reports through the `DiagnosticReporter` at once. Here the
//! errors object cannot borrow the AST (the resolver owns it mutably), so
//! each call is recorded as a [`SharedError`] and the resolver converts it
//! ([`to_diagnostic`]) before it reports its next diagnostic
//! ([`ResolverVisitor::report`]), which keeps the report order.

use dartr_ast::{
    CastPattern, DartPattern, Expression, Id, NodeId, NullAssertPattern, NullCheckPattern,
    RelationalPattern, Statement, SwitchStatement,
};
use dartr_diagnostics::{LocatableDiagnostic, LocatedDiagnostic, diag};
use dartr_element::{EId, Name, PromotableElement, TypeId, diagnostics::type_arg};
use dartr_flow::shared_type::SharedTypeView;
use dartr_flow::type_analysis_result::UnnecessaryWildcardKind;
use dartr_flow::type_analyzer::{RecordPatternField, TypeAnalyzerErrors, TypeAnalyzerErrorsBase};

use crate::resolver::ResolverVisitor;

/// One recorded call of the shared type analyzer's error interface.
#[derive(Clone, Debug)]
pub enum SharedError {
    CaseExpressionTypeMismatch {
        case_expression: Id<Expression>,
        scrutinee_type: TypeId,
        case_expression_type: TypeId,
    },
    DuplicateAssignmentPatternVariable {
        variable: EId<PromotableElement>,
        original: Id<DartPattern>,
        duplicate: Id<DartPattern>,
    },
    DuplicateRecordPatternField {
        pattern: Id<DartPattern>,
        name: Name,
        original: NodeId,
        duplicate: NodeId,
    },
    DuplicateRestPattern {
        original: NodeId,
        duplicate: NodeId,
    },
    EmptyMapPattern(Id<DartPattern>),
    InconsistentJoinedPatternVariable {
        variable: EId<PromotableElement>,
        component: EId<PromotableElement>,
    },
    MatchedTypeIsStrictlyNonNullable(Id<DartPattern>),
    MatchedTypeIsSubtypeOfRequired(Id<DartPattern>),
    NonBooleanCondition(Id<Expression>),
    PatternForInExpressionIsNotIterable {
        expression: Id<Expression>,
        expression_type: TypeId,
    },
    PatternTypeMismatchInIrrefutableContext {
        pattern: Id<DartPattern>,
        matched_type: TypeId,
        required_type: TypeId,
    },
    RefutablePatternInIrrefutableContext(NodeId),
    RelationalPatternOperandTypeNotAssignable {
        pattern: Id<DartPattern>,
        operand_type: TypeId,
        parameter_type: TypeId,
    },
    RelationalPatternOperatorReturnTypeNotAssignableToBool(Id<DartPattern>),
    RestPatternInMap(NodeId),
    SwitchCaseCompletesNormally {
        node: Id<Statement>,
        case_index: usize,
    },
    UnnecessaryWildcardPattern(Id<DartPattern>),
}

/// Dart `SharedTypeAnalyzerErrors`: records the errors (see the module
/// documentation).
#[derive(Debug, Default)]
pub struct SharedTypeAnalyzerErrors {
    pending: Vec<SharedError>,
    /// Dart `RecordPatternImpl.hasDuplicateNamedField` of the record
    /// patterns with a duplicate named field.
    pub has_duplicate_named_field: Vec<Id<DartPattern>>,
}

impl SharedTypeAnalyzerErrors {
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn take(&mut self) -> Vec<SharedError> {
        std::mem::take(&mut self.pending)
    }
}

impl TypeAnalyzerErrorsBase for SharedTypeAnalyzerErrors {
    fn assert_in_error_recovery(&mut self) {}
}

impl TypeAnalyzerErrors for SharedTypeAnalyzerErrors {
    type Node = NodeId;
    type Statement = Id<Statement>;
    type Expression = Id<Expression>;
    type Variable = EId<PromotableElement>;
    type Pattern = Id<DartPattern>;
    type Error = ();
    type Type = TypeId;
    type Name = Name;

    fn case_expression_type_mismatch(
        &mut self,
        _scrutinee: Id<Expression>,
        case_expression: Id<Expression>,
        scrutinee_type: SharedTypeView<TypeId>,
        case_expression_type: SharedTypeView<TypeId>,
    ) {
        self.pending.push(SharedError::CaseExpressionTypeMismatch {
            case_expression,
            scrutinee_type: scrutinee_type.unwrap_type_view(),
            case_expression_type: case_expression_type.unwrap_type_view(),
        });
    }

    fn duplicate_assignment_pattern_variable(
        &mut self,
        variable: EId<PromotableElement>,
        original: Id<DartPattern>,
        duplicate: Id<DartPattern>,
    ) {
        self.pending
            .push(SharedError::DuplicateAssignmentPatternVariable {
                variable,
                original,
                duplicate,
            });
    }

    fn duplicate_record_pattern_field(
        &mut self,
        object_or_record_pattern: Id<DartPattern>,
        name: Name,
        original: RecordPatternField<NodeId, Id<DartPattern>, Name>,
        duplicate: RecordPatternField<NodeId, Id<DartPattern>, Name>,
    ) {
        self.has_duplicate_named_field
            .push(object_or_record_pattern);
        self.pending.push(SharedError::DuplicateRecordPatternField {
            pattern: object_or_record_pattern,
            name,
            original: original.node,
            duplicate: duplicate.node,
        });
    }

    fn duplicate_rest_pattern(
        &mut self,
        _map_or_list_pattern: Id<DartPattern>,
        original: NodeId,
        duplicate: NodeId,
    ) {
        self.pending.push(SharedError::DuplicateRestPattern {
            original,
            duplicate,
        });
    }

    fn empty_map_pattern(&mut self, pattern: Id<DartPattern>) {
        self.pending.push(SharedError::EmptyMapPattern(pattern));
    }

    fn inconsistent_joined_pattern_variable(
        &mut self,
        variable: EId<PromotableElement>,
        component: EId<PromotableElement>,
    ) {
        self.pending
            .push(SharedError::InconsistentJoinedPatternVariable {
                variable,
                component,
            });
    }

    fn matched_type_is_strictly_non_nullable(
        &mut self,
        pattern: Id<DartPattern>,
        _matched_type: SharedTypeView<TypeId>,
    ) -> Option<()> {
        self.pending
            .push(SharedError::MatchedTypeIsStrictlyNonNullable(pattern));
        Some(())
    }

    fn matched_type_is_subtype_of_required(
        &mut self,
        pattern: Id<DartPattern>,
        _matched_type: SharedTypeView<TypeId>,
        _required_type: SharedTypeView<TypeId>,
    ) {
        self.pending
            .push(SharedError::MatchedTypeIsSubtypeOfRequired(pattern));
    }

    fn non_boolean_condition(&mut self, node: Id<Expression>) {
        self.pending.push(SharedError::NonBooleanCondition(node));
    }

    fn pattern_for_in_expression_is_not_iterable(
        &mut self,
        _node: NodeId,
        expression: Id<Expression>,
        expression_type: SharedTypeView<TypeId>,
    ) {
        self.pending
            .push(SharedError::PatternForInExpressionIsNotIterable {
                expression,
                expression_type: expression_type.unwrap_type_view(),
            });
    }

    fn pattern_type_mismatch_in_irrefutable_context(
        &mut self,
        pattern: Id<DartPattern>,
        _context: NodeId,
        matched_type: SharedTypeView<TypeId>,
        required_type: SharedTypeView<TypeId>,
    ) {
        self.pending
            .push(SharedError::PatternTypeMismatchInIrrefutableContext {
                pattern,
                matched_type: matched_type.unwrap_type_view(),
                required_type: required_type.unwrap_type_view(),
            });
    }

    fn refutable_pattern_in_irrefutable_context(&mut self, pattern: NodeId, _context: NodeId) {
        self.pending
            .push(SharedError::RefutablePatternInIrrefutableContext(pattern));
    }

    fn relational_pattern_operand_type_not_assignable(
        &mut self,
        pattern: Id<DartPattern>,
        operand_type: SharedTypeView<TypeId>,
        parameter_type: SharedTypeView<TypeId>,
    ) {
        self.pending
            .push(SharedError::RelationalPatternOperandTypeNotAssignable {
                pattern,
                operand_type: operand_type.unwrap_type_view(),
                parameter_type: parameter_type.unwrap_type_view(),
            });
    }

    fn relational_pattern_operator_return_type_not_assignable_to_bool(
        &mut self,
        pattern: Id<DartPattern>,
        _return_type: SharedTypeView<TypeId>,
    ) {
        self.pending
            .push(SharedError::RelationalPatternOperatorReturnTypeNotAssignableToBool(pattern));
    }

    fn rest_pattern_in_map(&mut self, _node: Id<DartPattern>, element: NodeId) {
        self.pending.push(SharedError::RestPatternInMap(element));
    }

    fn switch_case_completes_normally(&mut self, node: Id<Statement>, case_index: usize) {
        self.pending
            .push(SharedError::SwitchCaseCompletesNormally { node, case_index });
    }

    fn unnecessary_wildcard_pattern(
        &mut self,
        pattern: Id<DartPattern>,
        kind: UnnecessaryWildcardKind,
    ) {
        match kind {
            UnnecessaryWildcardKind::LogicalAndPatternOperand => {
                self.pending
                    .push(SharedError::UnnecessaryWildcardPattern(pattern));
            }
        }
    }
}

/// Converts a recorded error to the diagnostic that the Dart
/// `SharedTypeAnalyzerErrors` reports.
pub fn to_diagnostic(rv: &ResolverVisitor<'_>, error: SharedError) -> Option<LocatedDiagnostic> {
    let ctx = &rv.ctx;
    let ast = &*rv.ast;
    let at = |d: LocatableDiagnostic, node: NodeId| {
        d.at_offset(ast.offset(node) as usize, ast.length(node) as usize)
    };
    let at_token = |d: LocatableDiagnostic, token: dartr_syntax::TokenId| {
        let t = ast.tokens.get(token);
        d.at_offset(t.offset as usize, (t.end() - t.offset) as usize)
    };
    Some(match error {
        SharedError::CaseExpressionTypeMismatch {
            case_expression,
            scrutinee_type,
            case_expression_type,
        } => at(
            diag::case_expression_type_is_not_switch_expression_subtype(
                type_arg(ctx, case_expression_type),
                type_arg(ctx, scrutinee_type),
            ),
            case_expression.raw(),
        ),
        SharedError::EmptyMapPattern(pattern) => at(diag::empty_map_pattern(), pattern.raw()),
        SharedError::MatchedTypeIsStrictlyNonNullable(pattern) => {
            if let Some(p) = ast.cast::<NullAssertPattern>(pattern) {
                at_token(diag::unnecessary_null_assert_pattern(), ast[p].operator)
            } else if let Some(p) = ast.cast::<NullCheckPattern>(pattern) {
                at_token(diag::unnecessary_null_check_pattern(), ast[p].operator)
            } else {
                unimplemented!("{:?}", ast.kind(pattern))
            }
        }
        SharedError::MatchedTypeIsSubtypeOfRequired(pattern) => {
            let p = ast.cast::<CastPattern>(pattern).expect("CastPattern");
            at_token(diag::unnecessary_cast_pattern(), ast[p].as_token)
        }
        SharedError::NonBooleanCondition(node) => at(diag::non_bool_condition(), node.raw()),
        SharedError::PatternForInExpressionIsNotIterable {
            expression,
            expression_type,
        } => at(
            diag::for_in_of_invalid_type(type_arg(ctx, expression_type), "Iterable"),
            expression.raw(),
        ),
        SharedError::PatternTypeMismatchInIrrefutableContext {
            pattern,
            matched_type,
            required_type,
        } => at(
            diag::pattern_type_mismatch_in_irrefutable_context(
                type_arg(ctx, matched_type),
                type_arg(ctx, required_type),
            ),
            pattern.raw(),
        ),
        SharedError::RefutablePatternInIrrefutableContext(pattern) => {
            at(diag::refutable_pattern_in_irrefutable_context(), pattern)
        }
        SharedError::RelationalPatternOperandTypeNotAssignable {
            pattern,
            operand_type,
            parameter_type,
        } => {
            let p = ast
                .cast::<RelationalPattern>(pattern)
                .expect("RelationalPattern");
            at(
                diag::relational_pattern_operand_type_not_assignable(
                    type_arg(ctx, operand_type),
                    type_arg(ctx, parameter_type),
                    ast.tokens.lexeme(ast[p].operator),
                ),
                ast[p].operand.raw(),
            )
        }
        SharedError::RelationalPatternOperatorReturnTypeNotAssignableToBool(pattern) => {
            let p = ast
                .cast::<RelationalPattern>(pattern)
                .expect("RelationalPattern");
            at_token(
                diag::relational_pattern_operator_return_type_not_assignable_to_bool(),
                ast[p].operator,
            )
        }
        SharedError::RestPatternInMap(element) => at(diag::rest_element_in_map_pattern(), element),
        SharedError::SwitchCaseCompletesNormally { node, case_index } => {
            let s = ast.cast::<SwitchStatement>(node).expect("SwitchStatement");
            let member = ast.list_raw(ast[s].members)[case_index];
            // Dart `member.keyword`: the first token after the labels.
            let keyword = crate::shared_type_analyzer::switch_member_keyword(ast, member);
            at_token(diag::switch_case_completes_normally(), keyword)
        }
        SharedError::UnnecessaryWildcardPattern(pattern) => {
            at(diag::unnecessary_wildcard_pattern(), pattern.raw())
        }
        // Diagnostic factories (`DiagnosticFactory().duplicate...`) with
        // context messages, and `inconsistentJoinedPatternVariable`.
        SharedError::DuplicateAssignmentPatternVariable { .. }
        | SharedError::DuplicateRecordPatternField { .. }
        | SharedError::DuplicateRestPattern { .. }
        | SharedError::InconsistentJoinedPatternVariable { .. } => {
            return crate::pattern_resolver::shared_error_diagnostic(rv, &error);
        }
    })
}

/// Dart `SwitchMember.keyword` of [member].
pub fn switch_member_keyword(ast: &dartr_ast::Ast, member: NodeId) -> dartr_syntax::TokenId {
    use dartr_ast::{SwitchCase, SwitchDefault, SwitchPatternCase};
    if let Some(m) = ast.cast::<SwitchCase>(member) {
        ast[m].keyword
    } else if let Some(m) = ast.cast::<SwitchDefault>(member) {
        ast[m].keyword
    } else if let Some(m) = ast.cast::<SwitchPatternCase>(member) {
        ast[m].keyword
    } else {
        unreachable!("not a switch member")
    }
}
