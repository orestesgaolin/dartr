// Dart source: pkg/_fe_analyzer_shared/lib/src/type_inference/assigned_variables.dart

//! The public API of `AssignedVariables`: the pre-pass that records which
//! variables are declared, read, written and captured in each loop,
//! closure, `try` statement and late initializer.
//!
//! Dart `AssignedVariables` is a concrete class. Here its public API is the
//! trait [`AssignedVariables`], so that the resolver (C1/C2) can be written
//! before the implementation (unit A11, together with `PromotionKeyStore`
//! and `AssignedVariablesForTesting`) exists.

use indexmap::{IndexMap, IndexSet};
use std::fmt::Debug;
use std::hash::Hash;

use crate::flow_analysis::PromotionKey;
use crate::promotion_key_store::PromotionKeyStore;

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

/// The implementation of [`AssignedVariables`] (the concrete Dart class
/// `AssignedVariables`), plus the queries of `AssignedVariablesForTesting`.
///
/// It owns the [`PromotionKeyStore`] that flow analysis uses (Dart: flow
/// analysis reads `assignedVariables.promotionKeyStore`), so flow analysis
/// takes ownership of this object
/// ([`FlowAnalysisImpl::new`](crate::flow_analysis_impl::FlowAnalysisImpl::new)).
#[derive(Debug)]
pub struct AssignedVariablesImpl<N, V> {
    /// Mapping from a node to the info for that node. Only used for lookup.
    info: IndexMap<N, AssignedVariablesNodeInfo>,

    /// Info for the variables written or captured anywhere in the code being
    /// analyzed.
    anywhere: AssignedVariablesNodeInfo,

    /// Stack of info for nodes that have been entered but not yet left.
    stack: Vec<AssignedVariablesNodeInfo>,

    /// The number of info objects that have been retrieved by `defer_node`
    /// but not yet sent to `store_info` (Dart keeps an identity set of them
    /// when assertions are enabled).
    deferred_infos: usize,

    /// Keeps track of whether `finish` has been called.
    is_finished: bool,

    /// The [`PromotionKeyStore`], which tracks the unique integer assigned to
    /// everything in the control flow that might be promotable.
    pub promotion_key_store: PromotionKeyStore<V>,
}

impl<N: Copy + Eq + Hash + Debug, V: Copy + Eq + Hash + Debug> Default
    for AssignedVariablesImpl<N, V>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<N: Copy + Eq + Hash + Debug, V: Copy + Eq + Hash + Debug> AssignedVariablesImpl<N, V> {
    /// Creates an empty `AssignedVariables`.
    pub fn new() -> Self {
        AssignedVariablesImpl {
            info: IndexMap::new(),
            anywhere: AssignedVariablesNodeInfo::default(),
            stack: vec![AssignedVariablesNodeInfo::default()],
            deferred_infos: 0,
            is_finished: false,
            promotion_key_store: PromotionKeyStore::new(),
        }
    }

    // ------------------------------------------- AssignedVariablesForTesting

    /// `capturedAnywhere`.
    pub fn captured_anywhere(&self) -> &IndexSet<PromotionKey> {
        &self.anywhere.captured
    }

    /// `declaredAtTopLevel`.
    pub fn declared_at_top_level(&self) -> &IndexSet<PromotionKey> {
        &self.stack[0].declared
    }

    /// `readAnywhere`.
    pub fn read_anywhere(&self) -> &IndexSet<PromotionKey> {
        &self.anywhere.read
    }

    /// `readCapturedAnywhere`.
    pub fn read_captured_anywhere(&self) -> &IndexSet<PromotionKey> {
        &self.anywhere.read_captured
    }

    /// `writtenAnywhere`.
    pub fn written_anywhere(&self) -> &IndexSet<PromotionKey> {
        &self.anywhere.written
    }

    /// `capturedInNode`.
    pub fn captured_in_node(&self, node: N) -> &IndexSet<PromotionKey> {
        &self.get_info_for_node(node).captured
    }

    /// `declaredInNode`.
    pub fn declared_in_node(&self, node: N) -> &IndexSet<PromotionKey> {
        &self.get_info_for_node(node).declared
    }

    /// `isTracked`.
    pub fn is_tracked(&self, node: N) -> bool {
        self.info.contains_key(&node)
    }

    /// `keyForVariable`.
    pub fn key_for_variable(&self, variable: V) -> PromotionKey {
        self.promotion_key_store.key_for_variable(variable)
    }

    /// `readCapturedInNode`.
    pub fn read_captured_in_node(&self, node: N) -> &IndexSet<PromotionKey> {
        &self.get_info_for_node(node).read_captured
    }

    /// `readInNode`.
    pub fn read_in_node(&self, node: N) -> &IndexSet<PromotionKey> {
        &self.get_info_for_node(node).read
    }

    /// `variableForKey`.
    pub fn variable_for_key(&self, key: PromotionKey) -> V {
        self.promotion_key_store.variable_for_key(key).unwrap()
    }

    /// `writtenInNode`.
    pub fn written_in_node(&self, node: N) -> &IndexSet<PromotionKey> {
        &self.get_info_for_node(node).written
    }
}

/// `set.removeAll(other)`, keeping the order of the remaining elements.
fn remove_all(set: &mut IndexSet<PromotionKey>, other: &IndexSet<PromotionKey>) {
    set.retain(|k| !other.contains(k));
}

impl<N: Copy + Eq + Hash + Debug, V: Copy + Eq + Hash + Debug> AssignedVariables
    for AssignedVariablesImpl<N, V>
{
    type Node = N;
    type Variable = V;

    fn anywhere(&self) -> &AssignedVariablesNodeInfo {
        &self.anywhere
    }

    fn is_finished(&self) -> bool {
        self.is_finished
    }

    fn begin_node(&mut self) {
        debug_assert!(!self.is_finished);
        self.stack.push(AssignedVariablesNodeInfo::default());
    }

    fn declare(&mut self, variable: V, ignore_duplicates: bool) {
        debug_assert!(!self.is_finished);
        let variable_key = self.promotion_key_store.key_for_variable(variable);
        let newly_declared = self.stack.last_mut().unwrap().declared.insert(variable_key);
        debug_assert!(ignore_duplicates || newly_declared);
        let newly_declared = self.anywhere.declared.insert(variable_key);
        debug_assert!(ignore_duplicates || newly_declared);
    }

    fn defer_node(
        &mut self,
        is_closure_or_late_variable_initializer: bool,
    ) -> AssignedVariablesNodeInfo {
        debug_assert!(!self.is_finished);
        let mut info = self.stack.pop().unwrap();
        let declared = info.declared.clone();
        remove_all(&mut info.read, &declared);
        remove_all(&mut info.written, &declared);
        remove_all(&mut info.read_captured, &declared);
        remove_all(&mut info.captured, &declared);
        let last = self.stack.last_mut().unwrap();
        last.read.extend(info.read.iter().copied());
        last.written.extend(info.written.iter().copied());
        last.read_captured
            .extend(info.read_captured.iter().copied());
        last.captured.extend(info.captured.iter().copied());
        if is_closure_or_late_variable_initializer {
            last.read_captured.extend(info.read.iter().copied());
            self.anywhere
                .read_captured
                .extend(info.read.iter().copied());
            last.captured.extend(info.written.iter().copied());
            self.anywhere.captured.extend(info.written.iter().copied());
        }
        self.deferred_infos += 1;
        info
    }

    fn discard_node(&mut self) {
        debug_assert!(!self.is_finished);
        let discarded = self.stack.pop().unwrap();
        let last = self.stack.last_mut().unwrap();
        last.declared.extend(discarded.declared);
        last.read.extend(discarded.read);
        last.written.extend(discarded.written);
        last.read_captured.extend(discarded.read_captured);
        last.captured.extend(discarded.captured);
    }

    fn end_node(&mut self, node: N, is_closure_or_late_variable_initializer: bool) {
        debug_assert!(!self.is_finished);
        let info = self.defer_node(is_closure_or_late_variable_initializer);
        self.store_info(node, info);
    }

    fn finish(&mut self) {
        debug_assert!(!self.is_finished);
        debug_assert_eq!(self.deferred_infos, 0, "Deferred infos not stored");
        debug_assert_eq!(self.stack.len(), 1, "Unexpected stack: {:?}", self.stack);
        if cfg!(debug_assertions) {
            let last = self.stack.last().unwrap();
            let vars = |keys: Vec<PromotionKey>| -> Vec<Option<V>> {
                keys.into_iter()
                    .map(|k| self.promotion_key_store.variable_for_key(k))
                    .collect()
            };
            let undeclared_reads: Vec<PromotionKey> =
                last.read.difference(&last.declared).copied().collect();
            assert!(
                undeclared_reads.is_empty(),
                "Variables read from but not declared: {:?}",
                vars(undeclared_reads)
            );
            let undeclared_writes: Vec<PromotionKey> =
                last.written.difference(&last.declared).copied().collect();
            assert!(
                undeclared_writes.is_empty(),
                "Variables written to but not declared: {:?}",
                vars(undeclared_writes)
            );
            // Note that it's not necessary to check `last.captured` and
            // `last.read_captured`, because a variable can't be captured (or
            // read captured) without writing (or reading) it.
        }
        self.is_finished = true;
    }

    fn get_info_for_node(&self, node: N) -> &AssignedVariablesNodeInfo {
        match self.info.get(&node) {
            Some(info) => info,
            None => panic!(
                "No information for {:?} in {{{:?}}}",
                node,
                self.info.keys().collect::<Vec<_>>()
            ),
        }
    }

    fn pop_node(&mut self) -> AssignedVariablesNodeInfo {
        debug_assert!(!self.is_finished);
        self.stack.pop().unwrap()
    }

    fn push_node(&mut self, node: AssignedVariablesNodeInfo) {
        debug_assert!(!self.is_finished);
        self.stack.push(node);
    }

    fn read(&mut self, variable: V) {
        debug_assert!(!self.is_finished);
        let variable_key = self.promotion_key_store.key_for_variable(variable);
        self.stack.last_mut().unwrap().read.insert(variable_key);
        self.anywhere.read.insert(variable_key);
    }

    fn reassign_info(&mut self, from: N, to: N) {
        debug_assert!(!self.info.contains_key(&to), "Node {to:?} already has info");
        let info = self.info.shift_remove(&from);
        debug_assert!(info.is_some(), "No information for {from:?}");
        self.info.insert(to, info.unwrap());
    }

    fn store_info(&mut self, node: N, info: AssignedVariablesNodeInfo) {
        debug_assert!(!self.is_finished);
        // Caller should not try to store the same piece of info more than
        // once.
        debug_assert!(self.deferred_infos > 0);
        self.deferred_infos -= 1;
        self.info.insert(node, info);
    }

    fn write(&mut self, variable: V) {
        debug_assert!(!self.is_finished);
        let variable_key = self.promotion_key_store.key_for_variable(variable);
        self.stack.last_mut().unwrap().written.insert(variable_key);
        self.anywhere.written.insert(variable_key);
    }
}
