// Dart source: pkg/_fe_analyzer_shared/lib/src/type_inference/null_shorting.dart

//! Null shorting logic to be shared between the analyzer and the CFE.
//!
//! Dart has an interface `TypeAnalysisNullShortingInterface` and a mixin
//! `NullShortingMixin` that implements it (with a stack of guards as
//! state). The interface has no other implementation, so both are one
//! trait here: [`TypeAnalysisNullShortingInterface`]. The mixin's state
//! (`_guards`) is reached through the required methods
//! [`guards`](TypeAnalysisNullShortingInterface::guards) and
//! [`guards_mut`](TypeAnalysisNullShortingInterface::guards_mut).

use std::fmt::Debug;
use std::hash::Hash;

use crate::flow_analysis::FlowAnalysisNullShortingInterface;
use crate::flow_analysis_operations::FlowAnalysisTypeOperations;
use crate::shared_type::{TypeOf, TypeView};
use crate::type_analysis_result::ExpressionTypeAnalysisResult;

/// Shorthand: the expression analysis result of a null shorting client `A`.
pub type ExpressionResultOf<A> = ExpressionTypeAnalysisResult<
    TypeOf<<A as TypeAnalysisNullShortingInterface>::Operations>,
    <<A as TypeAnalysisNullShortingInterface>::Flow as FlowAnalysisNullShortingInterface>::ExpressionInfo,
>;

/// Shorthand: the flow analysis `ExpressionInfo` of a null shorting client
/// `A`.
pub type ExpressionInfoOf<A> =
    <<A as TypeAnalysisNullShortingInterface>::Flow as FlowAnalysisNullShortingInterface>::ExpressionInfo;

/// Null shorting: `TypeAnalysisNullShortingInterface` together with
/// `NullShortingMixin<Guard, Expression, Variable>`.
///
/// This trait should be implemented by the same type that implements
/// [`TypeAnalyzer`](crate::type_analyzer::TypeAnalyzer).
///
/// Not object safe in practice (the shared code uses it as a generic
/// bound).
pub trait TypeAnalysisNullShortingInterface {
    /// The client's expression node.
    type Expression: Copy + Eq + Hash + Debug;

    /// The client's variable.
    type Variable: Copy + Eq + Hash + Debug;

    /// The operations used to access types and check subtyping (Dart getter
    /// `operations`, typically a `TypeAnalyzerOperations`).
    type Operations: FlowAnalysisTypeOperations;

    /// The client's flow analysis object (Dart getter `flow`).
    type Flow: FlowAnalysisNullShortingInterface<
            Expression = Self::Expression,
            Variable = Self::Variable,
            Type = TypeOf<Self::Operations>,
        >;

    /// The data structure used by the client to desugar null-aware accesses
    /// (Dart type parameter `Guard`). The analyzer doesn't desugar and uses
    /// `()` (Dart `Null`).
    type Guard;

    /// Returns the client's flow analysis object.
    fn flow(&mut self) -> &mut Self::Flow;

    /// The operations, used to access types and check subtyping.
    fn operations(&self) -> &Self::Operations;

    /// Stack of guards associated with null-shorting operations that haven't
    /// been terminated yet (the state of `NullShortingMixin`, Dart field
    /// `_guards`).
    fn guards(&self) -> &[Self::Guard];

    /// Mutable access to [`guards`](Self::guards).
    fn guards_mut(&mut self) -> &mut Vec<Self::Guard>;

    /// Returns the number of null-shorting operations that haven't been
    /// terminated yet.
    fn null_shorting_depth(&self) -> usize {
        self.guards().len()
    }

    /// Terminates one or more null-shorting operations that were previously
    /// started using [`start_null_shorting`](Self::start_null_shorting).
    ///
    /// Call this at the point where the null-shorting flow control path
    /// rejoins the main path (for `i?.toString()`, after analyzing the call
    /// to `toString()`). `target_depth` is a value previously returned by
    /// [`null_shorting_depth`](Self::null_shorting_depth); it must be
    /// strictly less than the current depth. `inner_result` is the result
    /// for the expression before termination of null shorting
    /// (`i.toString()`).
    ///
    /// Returns the result for the full expression; its type accounts for the
    /// fact that the expression might evaluate to `null` (`String?`).
    fn finish_null_shorting(
        &mut self,
        target_depth: usize,
        inner_result: ExpressionResultOf<Self>,
        whole_expression: Self::Expression,
    ) -> ExpressionResultOf<Self> {
        let _ = whole_expression;
        debug_assert!(target_depth < self.null_shorting_depth());
        let inferred_type = self.operations().make_nullable(inner_result.type_);
        let mut inner_result = inner_result;
        loop {
            // End non-nullable promotion of the null-aware variable.
            self.flow().null_aware_access_end();
            let guard = self.guards_mut().pop().expect("null shorting guard");
            inner_result = self.handle_null_shorting_step(inner_result, guard, inferred_type);
            debug_assert!(inner_result.type_ == inferred_type);
            if self.null_shorting_depth() <= target_depth {
                break;
            }
        }
        self.handle_null_shorting_finished(inferred_type);
        inner_result
    }

    /// Hook called by [`finish_null_shorting`](Self::finish_null_shorting)
    /// after terminating all the null shorting that needs to be terminated
    /// for a given expression. `inferred_type` is the (nullable) type of the
    /// final expression.
    fn handle_null_shorting_finished(&mut self, inferred_type: TypeView<Self::Operations>) {
        let _ = inferred_type;
    }

    /// Hook called by [`finish_null_shorting`](Self::finish_null_shorting)
    /// after terminating a single null-shorting operation.
    ///
    /// `guard` is the value that was passed to
    /// [`start_null_shorting`](Self::start_null_shorting). Returns the result
    /// of analyzing the expression after termination of null shorting.
    fn handle_null_shorting_step(
        &mut self,
        inner_result: ExpressionResultOf<Self>,
        guard: Self::Guard,
        inferred_type: TypeView<Self::Operations>,
    ) -> ExpressionResultOf<Self> {
        let _ = (inner_result, guard);
        ExpressionTypeAnalysisResult::new(inferred_type)
    }

    /// Starts null shorting for a null-aware expression that participates in
    /// null-shorting. There is no "stop" method; `TypeAnalyzer.analyzeExpression`
    /// stops it.
    ///
    /// `target_info` / `target_type` describe the target of the null-aware
    /// operation (left of `?.`). `guard` is passed back to
    /// [`handle_null_shorting_step`](Self::handle_null_shorting_step).
    /// `guard_variable` is the variable used for desugaring, if any.
    ///
    /// Returns the flow analysis expression info for the target (assuming it
    /// is not null).
    fn start_null_shorting(
        &mut self,
        guard: Self::Guard,
        target_info: Option<ExpressionInfoOf<Self>>,
        target_type: TypeView<Self::Operations>,
        guard_variable: Option<Self::Variable>,
    ) -> Option<ExpressionInfoOf<Self>> {
        // Ensure the initializer of the null-aware variable is promoted to
        // non-nullable.
        let target_info =
            self.flow()
                .null_aware_access_right_begin(target_info, target_type, guard_variable);
        self.guards_mut().push(guard);
        target_info
    }
}
