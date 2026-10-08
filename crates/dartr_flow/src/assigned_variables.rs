// Dart source: pkg/_fe_analyzer_shared/lib/src/type_inference/assigned_variables.dart

//! The public API of `AssignedVariables`: the pre-pass that records which
//! variables are declared, read, written and captured in each loop,
//! closure, `try` statement and late initializer.
//!
//! Dart `AssignedVariables` is a concrete class. Here its public API is the
//! trait [`AssignedVariables`], so that the resolver (C1/C2) can be written
//! before the implementation (unit A11, together with `PromotionKeyStore`
//! and `AssignedVariablesForTesting`) exists.

use indexmap::IndexSet;
use std::fmt::Debug;
use std::hash::Hash;

use crate::flow_analysis::PromotionKey;

/// Information tracked by [`AssignedVariables`] for a single node. The sets
/// hold promotion keys of variables.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct AssignedVariablesNodeInfo {
    /// The set of local variables that are potentially read in the node.
    pub read: IndexSet<PromotionKey>,

    /// The set of local variables that are potentially written in the node.
    pub written: IndexSet<PromotionKey>,

    /// The set of local variables for which a potential read is captured by
    /// a local function or closure inside the node.
    pub read_captured: IndexSet<PromotionKey>,

    /// The set of local variables for which a potential write is captured by
    /// a local function or closure inside the node.
    pub captured: IndexSet<PromotionKey>,

    /// The set of local variables that are declared in the node.
    pub declared: IndexSet<PromotionKey>,
}

/// Records which variables are definitely or potentially assigned in which
/// AST nodes (pre-pass of flow analysis).
///
/// In the pre-traversal (before flow analysis), call
/// [`begin_node`](Self::begin_node) and [`end_node`](Self::end_node) around
/// each "node": a loop statement, switch statement, try statement, loop
/// collection element, local function, closure, or late variable
/// initializer. The span should cover any code that might be crossed by a
/// backwards jump (in a "for" loop: condition, updaters and body, not the
/// initializers). Call
/// [`declare`](Self::declare), [`read`](Self::read) and
/// [`write`](Self::write) for the variable declarations, reads and writes in
/// between.
///
/// Object safe once the associated types are fixed.
pub trait AssignedVariables {
    /// The client's AST node.
    type Node: Copy + Eq + Hash + Debug;

    /// The client's variable.
    type Variable: Copy + Eq + Hash + Debug;

    /// Information about all variables, anywhere in the code being analyzed
    /// (Dart field `anywhere`).
    fn anywhere(&self) -> &AssignedVariablesNodeInfo;

    /// Whether [`finish`](Self::finish) has been called.
    fn is_finished(&self) -> bool;

    /// Starts tracking a node: pushes a new set of information.
    fn begin_node(&mut self);

    /// Records a declaration of `variable` in the current node.
    ///
    /// Dart default: `ignoreDuplicates = false`.
    fn declare(&mut self, variable: Self::Variable, ignore_duplicates: bool);

    /// Stops tracking the current node and returns its information, without
    /// associating it with a node yet (use [`store_info`](Self::store_info)
    /// later). Variables declared in the node are removed from the sets that
    /// are merged into the enclosing node.
    ///
    /// Dart default: `isClosureOrLateVariableInitializer = false`.
    fn defer_node(
        &mut self,
        is_closure_or_late_variable_initializer: bool,
    ) -> AssignedVariablesNodeInfo;

    /// Discards the effects of the most recent unmatched call to
    /// [`begin_node`](Self::begin_node): its information is merged into the
    /// enclosing node. (Used by the front end for `try/catch/finally`.)
    fn discard_node(&mut self);

    /// Stops tracking the current node and associates its information with
    /// `node`.
    ///
    /// Dart default: `isClosureOrLateVariableInitializer = false`.
    fn end_node(&mut self, node: Self::Node, is_closure_or_late_variable_initializer: bool);

    /// Call this after visiting the code to be analyzed, to check invariants.
    fn finish(&mut self);

    /// Gets the info for the given `node`. Panics if there is none (Dart
    /// throws a `StateError`).
    fn get_info_for_node(&self, node: Self::Node) -> &AssignedVariablesNodeInfo;

    /// Call between [`begin_node`](Self::begin_node) and
    /// [`end_node`](Self::end_node)/[`defer_node`](Self::defer_node) if it is
    /// necessary to temporarily process some code outside the current node.
    /// Returns a value to pass to [`push_node`](Self::push_node).
    fn pop_node(&mut self) -> AssignedVariablesNodeInfo;

    /// Un-does the effect of [`pop_node`](Self::pop_node).
    fn push_node(&mut self, node: AssignedVariablesNodeInfo);

    /// Records a read of `variable` in the current node.
    fn read(&mut self, variable: Self::Variable);

    /// Moves the information recorded for `from` to `to` (when the client
    /// replaces an AST node).
    fn reassign_info(&mut self, from: Self::Node, to: Self::Node);

    /// Associates `info` (returned by [`defer_node`](Self::defer_node)) with
    /// `node`.
    fn store_info(&mut self, node: Self::Node, info: AssignedVariablesNodeInfo);

    /// Records a write of `variable` in the current node.
    fn write(&mut self, variable: Self::Variable);
}
