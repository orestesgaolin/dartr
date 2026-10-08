// Dart source: pkg/_fe_analyzer_shared/lib/src/type_inference/variable_bindings.dart

//! Data structure for tracking declared pattern variables: the state and the
//! concrete methods of the Dart abstract class `VariableBinder`.
//!
//! The abstract member (`joinPatternVariables`) is the trait
//! [`dartr_flow::variable_bindings::VariableBinder`], implemented by the
//! client. The Dart class keeps the state (`_variables`,
//! `_sharedCaseScopes`) and the error reporter (`errors`) in the same object;
//! here the state is [`VariableBinderState`], and its methods receive the
//! client (`binder`) and the error reporter (`errors`) as arguments.
//!
//! To analyze a single `case pattern when guard`:
//! 1. Invoke [`case_pattern_start`](VariableBinderState::case_pattern_start).
//! 2. Invoke zero or more [`add`](VariableBinderState::add).
//! 3. Invoke [`case_pattern_finish`](VariableBinderState::case_pattern_finish),
//!    get the set of variables `VS`.
//! 4. Use `VS` to analyze the guard.
//!
//! To analyze a group of `case` members of a `switch` statement, sharing the
//! same body, and so having the shared set of pattern variables:
//! 1. Invoke [`switch_statement_shared_case_scope_start`](VariableBinderState::switch_statement_shared_case_scope_start).
//! 2. Analyze individual `case pattern when guard` clauses.
//! 3. Invoke [`switch_statement_shared_case_scope_empty`](VariableBinderState::switch_statement_shared_case_scope_empty)
//!    if there are labels, or a `default` member.
//! 4. Invoke [`switch_statement_shared_case_scope_finish`](VariableBinderState::switch_statement_shared_case_scope_finish)
//!    to get the set of variables `VS`, and use it to analyze the shared
//!    body.

use std::hash::Hash;

use dartr_flow::type_analyzer::JoinedPatternVariableInconsistency;
use dartr_flow::variable_bindings::{VariableBinder, VariableBinderErrors};
use indexmap::IndexMap;

/// The state of a Dart `VariableBinder`.
///
/// `N` is the client's name (Dart `String`), `V` the client's variable, `K`
/// the key of a join ([`VariableBinder::Key`]).
#[derive(Debug)]
pub struct VariableBinderState<N, V, K> {
    /// The stack of variable sets, starting with an empty one on
    /// `casePatternStart`, or `logicalOrPatternStart`.
    variables: Vec<IndexMap<N, V>>,

    /// The stack of variable sets for potentially nested (e.g. `switch` in a
    /// closure in a `when` clause) groups of `case` members of a `switch`
    /// statement.
    shared_case_scopes: Vec<SharedCaseScope<N, V, K>>,
}

impl<N, V, K> Default for VariableBinderState<N, V, K> {
    fn default() -> Self {
        VariableBinderState {
            variables: Vec::new(),
            shared_case_scopes: Vec::new(),
        }
    }
}

impl<N, V, K> VariableBinderState<N, V, K>
where
    N: Copy + Eq + Hash,
    V: Copy,
    K: Clone + PartialEq,
{
    /// Creates an empty state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Updates the set of bindings to account for the presence of a variable
    /// pattern. `name` is the name of the variable, `variable` is the object
    /// that represents it in the client.
    pub fn add<E>(&mut self, errors: Option<&mut E>, name: N, variable: V) -> bool
    where
        E: VariableBinderErrors<Name = N, Variable = V> + ?Sized,
    {
        let last = self.variables.last_mut().expect("casePatternStart");
        match last.get(&name) {
            None => {
                last.insert(name, variable);
                true
            }
            Some(&existing) => {
                if let Some(errors) = errors {
                    errors.duplicate_variable_pattern(name, existing, variable);
                }
                false
            }
        }
    }

    /// Should be invoked after visiting a `case pattern` structure. Returns
    /// all the accumulated variables (individual and joined).
    ///
    /// If `shared_case_scope_key` is provided, it expected to be the same as
    /// the key of the last shared case scope, and the resulting set will be
    /// joined with the current shared case scope.
    pub fn case_pattern_finish(&mut self, shared_case_scope_key: Option<&K>) -> IndexMap<N, V> {
        let variables = self.variables.pop().expect("casePatternStart");

        if let Some(shared_case_scope_key) = shared_case_scope_key {
            let shared_scope = self
                .shared_case_scopes
                .last_mut()
                .expect("switchStatementSharedCaseScopeStart");
            debug_assert!(shared_scope.key == *shared_case_scope_key);
            shared_scope.add_all(&variables);
        }

        variables
    }

    /// Notifies that are new `case pattern` structure is about to be
    /// visited.
    pub fn case_pattern_start(&mut self) {
        self.variables.push(IndexMap::new());
    }

    /// Notifies that this instance is about to be discarded.
    pub fn finish(&self) {
        debug_assert!(self.variables.is_empty());
        debug_assert!(self.shared_case_scopes.is_empty());
    }

    /// Updates the binder after visiting a logical-or pattern, joins
    /// variables from them. If some variables are in one side of the
    /// pattern, but not in another, they are still joined, but marked as not
    /// consistent.
    ///
    /// The join key is `node` (Dart `key: node`).
    pub fn logical_or_pattern_finish<B, E>(
        &mut self,
        binder: &mut B,
        mut errors: Option<&mut E>,
        node: B::Node,
    ) where
        B: VariableBinder<Variable = V, Key = K> + ?Sized,
        B::Node: Into<K>,
        E: VariableBinderErrors<Node = B::Node, Name = N, Variable = V> + ?Sized,
    {
        let mut right = self.variables.pop().expect("logicalOrPatternFinishLeft");
        let left = self.variables.pop().expect("logicalOrPatternStart");
        for (name, left_variable) in left {
            let right_variable = right.shift_remove(&name);
            match right_variable {
                Some(right_variable) => {
                    let joined = binder.join_pattern_variables(
                        node.into(),
                        vec![left_variable, right_variable],
                        JoinedPatternVariableInconsistency::None,
                    );
                    self.add(errors.as_deref_mut(), name, joined);
                }
                None => {
                    if let Some(errors) = errors.as_deref_mut() {
                        errors.logical_or_pattern_branch_missing_variable(
                            node,
                            true,
                            name,
                            left_variable,
                        );
                    }
                    let joined = binder.join_pattern_variables(
                        node.into(),
                        vec![left_variable],
                        JoinedPatternVariableInconsistency::LogicalOr,
                    );
                    self.add(errors.as_deref_mut(), name, joined);
                }
            }
        }
        for (name, right_variable) in right {
            if let Some(errors) = errors.as_deref_mut() {
                errors.logical_or_pattern_branch_missing_variable(
                    node,
                    false,
                    name,
                    right_variable,
                );
            }
            let joined = binder.join_pattern_variables(
                node.into(),
                vec![right_variable],
                JoinedPatternVariableInconsistency::LogicalOr,
            );
            self.add(errors.as_deref_mut(), name, joined);
        }
    }

    /// Notifies that the LHS of a logical-or pattern was visited, and the
    /// RHS is about to be visited.
    pub fn logical_or_pattern_finish_left(&mut self) {
        self.variables.push(IndexMap::new());
    }

    /// Notifies that we are about to start visiting a logical-or pattern.
    pub fn logical_or_pattern_start(&mut self) {
        self.variables.push(IndexMap::new());
    }

    /// Notifies that the `default` case head, or a label, was found, so that
    /// all the variables of the current shared case scope are not
    /// consistent.
    pub fn switch_statement_shared_case_scope_empty(&mut self, key: &K) {
        let shared_scope = self
            .shared_case_scopes
            .last_mut()
            .expect("switchStatementSharedCaseScopeStart");
        debug_assert!(shared_scope.key == *key);
        shared_scope.has_label = true;
        shared_scope.add_all(&IndexMap::new());
    }

    /// Notifies that computing of the shared case scope was finished,
    /// returns the joined set of variables. The variables have not been
    /// checked to have the same types (because we have not done inference,
    /// so we don't know types for many of them), so some of them might
    /// become not consistent later.
    pub fn switch_statement_shared_case_scope_finish<B>(
        &mut self,
        binder: &mut B,
        key: K,
    ) -> IndexMap<N, V>
    where
        B: VariableBinder<Variable = V, Key = K> + ?Sized,
    {
        debug_assert!(self.variables.is_empty());
        let shared_scope = self
            .shared_case_scopes
            .pop()
            .expect("switchStatementSharedCaseScopeStart");
        debug_assert!(shared_scope.key == key);

        let mut result = IndexMap::new();
        for (name, shared_variable) in shared_scope.variables {
            let variables = shared_variable.variables;
            if shared_variable.all_cases && variables.len() == 1 {
                result.insert(name, variables[0]);
            } else {
                let inconsistency = if shared_variable.all_cases {
                    JoinedPatternVariableInconsistency::None
                } else if shared_scope.has_label {
                    JoinedPatternVariableInconsistency::SharedCaseHasLabel
                } else {
                    JoinedPatternVariableInconsistency::SharedCaseAbsent
                };
                result.insert(
                    name,
                    binder.join_pattern_variables(key.clone(), variables, inconsistency),
                );
            }
        }
        result
    }

    /// Notifies that computing new shared case scope should be started.
    pub fn switch_statement_shared_case_scope_start(&mut self, key: K) {
        debug_assert!(self.variables.is_empty());
        self.shared_case_scopes.push(SharedCaseScope::new(key));
    }
}

/// Dart `_SharedCaseScope`.
#[derive(Debug)]
struct SharedCaseScope<N, V, K> {
    key: K,
    is_empty: bool,
    has_label: bool,
    variables: IndexMap<N, SharedCaseScopeVariable<V>>,
}

impl<N: Copy + Eq + Hash, V: Copy, K> SharedCaseScope<N, V, K> {
    fn new(key: K) -> Self {
        SharedCaseScope {
            key,
            is_empty: true,
            has_label: false,
            variables: IndexMap::new(),
        }
    }

    /// Adds `new_variables` to `variables`, marking absent variables as not
    /// consistent. If `is_empty`, just sets given variables as the starting
    /// set.
    fn add_all(&mut self, new_variables: &IndexMap<N, V>) {
        if self.is_empty {
            self.is_empty = false;
            for (&name, &variable) in new_variables {
                self.get_variable(name).variables.push(variable);
            }
        } else {
            for (name, variable) in self.variables.iter_mut() {
                match new_variables.get(name) {
                    Some(&new_variable) => variable.variables.push(new_variable),
                    None => variable.all_cases = false,
                }
            }
            for (&name, &new_variable) in new_variables {
                if !self.variables.contains_key(&name) {
                    let variable = self.get_variable(name);
                    variable.all_cases = false;
                    variable.variables.push(new_variable);
                }
            }
        }
    }

    fn get_variable(&mut self, name: N) -> &mut SharedCaseScopeVariable<V> {
        self.variables
            .entry(name)
            .or_insert_with(SharedCaseScopeVariable::new)
    }
}

/// Dart `_SharedCaseScopeVariable`.
#[derive(Debug)]
struct SharedCaseScopeVariable<V> {
    all_cases: bool,
    variables: Vec<V>,
}

impl<V> SharedCaseScopeVariable<V> {
    fn new() -> Self {
        SharedCaseScopeVariable {
            all_cases: true,
            variables: Vec::new(),
        }
    }
}
