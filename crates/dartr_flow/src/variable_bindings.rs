// Dart source: pkg/_fe_analyzer_shared/lib/src/type_inference/variable_bindings.dart

//! Tracking of declared pattern variables: the client side of
//! `VariableBinder` and `VariableBinderErrors`.
//!
//! Dart `VariableBinder` is an abstract class with state and concrete
//! methods (`add`, `casePatternStart`, `casePatternFinish`,
//! `logicalOrPatternStart`, `logicalOrPatternFinishLeft`,
//! `logicalOrPatternFinish`, `switchStatementSharedCaseScopeStart`,
//! `switchStatementSharedCaseScopeEmpty`,
//! `switchStatementSharedCaseScopeFinish`, `finish`) and one abstract
//! method (`joinPatternVariables`). Only the abstract member is fixed here
//! ([`VariableBinder`]); the state (`_variables`, `_sharedCaseScopes`) and
//! the concrete methods are left for unit A13.

use std::fmt::Debug;
use std::hash::Hash;

use crate::type_analyzer::{JoinedPatternVariableInconsistency, TypeAnalyzerErrorsBase};

/// The abstract member of `VariableBinder<Node, Variable>`, implemented by
/// the client.
///
/// Object safe once the associated types are fixed.
pub trait VariableBinder {
    /// The client's AST node.
    type Node: Copy + Eq + Hash + Debug;

    /// The client's variable.
    type Variable: Copy + Eq + Hash + Debug;

    /// The key of a join (Dart `Object key`): the logical-or pattern node,
    /// or the key of the shared case scope (the switch member).
    type Key: Clone + Debug;

    /// Returns a new variable that is a join of `components`.
    fn join_pattern_variables(
        &mut self,
        key: Self::Key,
        components: Vec<Self::Variable>,
        inconsistency: JoinedPatternVariableInconsistency,
    ) -> Self::Variable;
}

/// Interface used by the `VariableBinder` logic to report error conditions
/// up to the client during the "pre-visit" phase of type analysis.
///
/// Object safe once the associated types are fixed.
pub trait VariableBinderErrors: TypeAnalyzerErrorsBase {
    /// The client's AST node.
    type Node: Copy + Eq + Hash + Debug;

    /// The client's variable.
    type Variable: Copy + Eq + Hash + Debug;

    /// The client's name of a variable (Dart `String`).
    type Name: Copy + Eq + Hash + Debug;

    /// Called when a pattern attempts to declare the variable `duplicate`
    /// that has the same `name` as the `original` variable.
    fn duplicate_variable_pattern(
        &mut self,
        name: Self::Name,
        original: Self::Variable,
        duplicate: Self::Variable,
    );

    /// Called when one of the branches has the `variable` with the `name`,
    /// but the other branch does not.
    fn logical_or_pattern_branch_missing_variable(
        &mut self,
        node: Self::Node,
        has_in_left: bool,
        name: Self::Name,
        variable: Self::Variable,
    );
}
