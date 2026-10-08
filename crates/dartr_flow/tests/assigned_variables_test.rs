//! Port of `pkg/_fe_analyzer_shared/test/flow_analysis/assigned_variables_test.dart`.
//!
//! Dart `AssignedVariablesForTesting<_Node, _Variable>` is
//! [`AssignedVariablesImpl<Node, Variable>`]; its testing queries are
//! inherent methods. Dart `group` blocks are modules; each Dart `test(...)`
//! is one `#[test]` function, with its Dart name in the doc comment.
//!
//! Dart sets are compared as sets (Dart `unorderedEquals` or `{...}`
//! literals), so the helpers collect into a `BTreeSet`. Dart `isEmpty` is
//! `is_empty()`.
//!
//! Dart `_Node()` is a fresh object with identity; here it is a `Node` with a
//! unique id. Dart `_Variable('v1')` is `Variable("v1")`; every test uses
//! distinct names.
//!
//! The Dart "Final assertions" tests check `assert` failures. They are
//! `#[should_panic]` and need debug assertions (the default for `cargo test`).

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU32, Ordering};

use indexmap::IndexSet;

use dartr_flow::assigned_variables::{AssignedVariables, AssignedVariablesImpl};
use dartr_flow::flow_analysis::PromotionKey;

/// Dart `_Node`: an object with identity.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct Node(u32);

static NEXT_NODE_ID: AtomicU32 = AtomicU32::new(0);

/// Dart `_Node()`: a new node that is not equal to any other node.
fn new_node() -> Node {
    Node(NEXT_NODE_ID.fetch_add(1, Ordering::Relaxed))
}

/// Dart `_Variable(name)`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct Variable(&'static str);

type Av = AssignedVariablesImpl<Node, Variable>;

/// Dart `AssignedVariablesForTesting<_Node, _Variable>()`.
fn new_av() -> Av {
    AssignedVariablesImpl::new()
}

/// Dart `assignedVariables.keyForVariable(v)` for each variable, as a set.
fn expected_keys(av: &Av, variables: &[Variable]) -> BTreeSet<PromotionKey> {
    variables.iter().map(|v| av.key_for_variable(*v)).collect()
}

/// Compares a Dart `Set<int>` (as an `IndexSet`) with an expected set.
fn assert_keys(actual: &IndexSet<PromotionKey>, expected: &BTreeSet<PromotionKey>) {
    let actual: BTreeSet<PromotionKey> = actual.iter().copied().collect();
    assert_eq!(&actual, expected);
}

mod read_and_write_anywhere {
    use super::*;

    /// Dart: `readCapturedAnywhere records reads in closures`
    #[test]
    fn read_captured_anywhere_records_reads_in_closures() {
        let mut av = new_av();
        let (v1, v2, v3) = (Variable("v1"), Variable("v2"), Variable("v3"));
        av.declare(v1, false);
        av.declare(v2, false);
        av.declare(v3, false);
        av.read(v1);
        av.begin_node();
        av.read(v2);
        av.end_node(new_node(), true);
        av.read(v3);
        av.finish();
        assert_keys(av.read_captured_anywhere(), &expected_keys(&av, &[v2]));
    }

    /// Dart: `readCapturedAnywhere does not record variables local to a closure`
    #[test]
    fn read_captured_anywhere_does_not_record_variables_local_to_a_closure() {
        let mut av = new_av();
        let (v1, v2) = (Variable("v1"), Variable("v2"));
        av.declare(v1, false);
        av.begin_node();
        av.declare(v2, false);
        av.read(v1);
        av.read(v2);
        av.end_node(new_node(), true);
        av.finish();
        assert_keys(av.read_captured_anywhere(), &expected_keys(&av, &[v1]));
    }

    /// Dart: `capturedAnywhere records assignments in closures`
    #[test]
    fn captured_anywhere_records_assignments_in_closures() {
        let mut av = new_av();
        let (v1, v2, v3) = (Variable("v1"), Variable("v2"), Variable("v3"));
        av.declare(v1, false);
        av.declare(v2, false);
        av.declare(v3, false);
        av.write(v1);
        av.begin_node();
        av.write(v2);
        av.end_node(new_node(), true);
        av.write(v3);
        av.finish();
        assert_keys(av.captured_anywhere(), &expected_keys(&av, &[v2]));
    }

    /// Dart: `capturedAnywhere does not record variables local to a closure`
    #[test]
    fn captured_anywhere_does_not_record_variables_local_to_a_closure() {
        let mut av = new_av();
        let (v1, v2) = (Variable("v1"), Variable("v2"));
        av.declare(v1, false);
        av.begin_node();
        av.declare(v2, false);
        av.write(v1);
        av.write(v2);
        av.end_node(new_node(), true);
        av.finish();
        assert_keys(av.captured_anywhere(), &expected_keys(&av, &[v1]));
    }

    /// Dart: `readAnywhere records all reads`
    #[test]
    fn read_anywhere_records_all_reads() {
        let mut av = new_av();
        let (v1, v2, v3) = (Variable("v1"), Variable("v2"), Variable("v3"));
        av.declare(v1, false);
        av.declare(v2, false);
        av.declare(v3, false);
        av.read(v1);
        av.begin_node();
        av.read(v2);
        av.end_node(new_node(), true);
        av.read(v3);
        av.finish();
        assert_keys(av.read_anywhere(), &expected_keys(&av, &[v1, v2, v3]));
    }

    /// Dart: `writtenAnywhere records all assignments`
    #[test]
    fn written_anywhere_records_all_assignments() {
        let mut av = new_av();
        let (v1, v2, v3) = (Variable("v1"), Variable("v2"), Variable("v3"));
        av.declare(v1, false);
        av.declare(v2, false);
        av.declare(v3, false);
        av.write(v1);
        av.begin_node();
        av.write(v2);
        av.end_node(new_node(), true);
        av.write(v3);
        av.finish();
        assert_keys(av.written_anywhere(), &expected_keys(&av, &[v1, v2, v3]));
    }
}

mod read_in_node {
    use super::*;

    /// Dart: `readInNode ignores reads outside the node`
    #[test]
    fn ignores_reads_outside_the_node() {
        let mut av = new_av();
        let (v1, v2) = (Variable("v1"), Variable("v2"));
        av.declare(v1, false);
        av.declare(v2, false);
        av.read(v1);
        av.begin_node();
        let node = new_node();
        av.end_node(node, false);
        av.read(v2);
        av.finish();
        assert!(av.read_in_node(node).is_empty());
    }

    /// Dart: `readInNode records reads inside the node`
    #[test]
    fn records_reads_inside_the_node() {
        let mut av = new_av();
        let v1 = Variable("v1");
        av.declare(v1, false);
        av.begin_node();
        av.read(v1);
        let node = new_node();
        av.end_node(node, false);
        av.finish();
        assert_keys(av.read_in_node(node), &expected_keys(&av, &[v1]));
    }

    /// Dart: `readInNode records reads in a nested node`
    #[test]
    fn records_reads_in_a_nested_node() {
        let mut av = new_av();
        let v1 = Variable("v1");
        av.declare(v1, false);
        av.begin_node();
        av.begin_node();
        av.read(v1);
        av.end_node(new_node(), false);
        let node = new_node();
        av.end_node(node, false);
        av.finish();
        assert_keys(av.read_in_node(node), &expected_keys(&av, &[v1]));
    }

    /// Dart: `readInNode records reads in a closure`
    #[test]
    fn records_reads_in_a_closure() {
        let mut av = new_av();
        let v1 = Variable("v1");
        av.declare(v1, false);
        av.begin_node();
        av.read(v1);
        let node = new_node();
        av.end_node(node, true);
        av.finish();
        assert_keys(av.read_in_node(node), &expected_keys(&av, &[v1]));
    }
}

mod written_in_node {
    use super::*;

    /// Dart: `writtenInNode ignores assignments outside the node`
    #[test]
    fn ignores_assignments_outside_the_node() {
        let mut av = new_av();
        let (v1, v2) = (Variable("v1"), Variable("v2"));
        av.declare(v1, false);
        av.declare(v2, false);
        av.write(v1);
        av.begin_node();
        let node = new_node();
        av.end_node(node, false);
        av.write(v2);
        av.finish();
        assert!(av.written_in_node(node).is_empty());
    }

    /// Dart: `writtenInNode records assignments inside the node`
    #[test]
    fn records_assignments_inside_the_node() {
        let mut av = new_av();
        let v1 = Variable("v1");
        av.declare(v1, false);
        av.begin_node();
        av.write(v1);
        let node = new_node();
        av.end_node(node, false);
        av.finish();
        assert_keys(av.written_in_node(node), &expected_keys(&av, &[v1]));
    }

    /// Dart: `writtenInNode records assignments in a nested node`
    #[test]
    fn records_assignments_in_a_nested_node() {
        let mut av = new_av();
        let v1 = Variable("v1");
        av.declare(v1, false);
        av.begin_node();
        av.begin_node();
        av.write(v1);
        av.end_node(new_node(), false);
        let node = new_node();
        av.end_node(node, false);
        av.finish();
        assert_keys(av.written_in_node(node), &expected_keys(&av, &[v1]));
    }

    /// Dart: `writtenInNode records assignments in a closure`
    #[test]
    fn records_assignments_in_a_closure() {
        let mut av = new_av();
        let v1 = Variable("v1");
        av.declare(v1, false);
        av.begin_node();
        av.write(v1);
        let node = new_node();
        av.end_node(node, true);
        av.finish();
        assert_keys(av.written_in_node(node), &expected_keys(&av, &[v1]));
    }
}

mod captures_in_node {
    use super::*;

    /// Dart: `readCapturedInNode ignores reads in non-nested closures`
    #[test]
    fn read_captured_in_node_ignores_reads_in_non_nested_closures() {
        let mut av = new_av();
        let (v1, v2) = (Variable("v1"), Variable("v2"));
        av.declare(v1, false);
        av.declare(v2, false);
        av.begin_node();
        av.read(v1);
        av.end_node(new_node(), true);
        av.begin_node();
        let node = new_node();
        av.end_node(node, false);
        av.begin_node();
        av.read(v2);
        av.end_node(new_node(), true);
        av.finish();
        assert!(av.read_captured_in_node(node).is_empty());
    }

    /// Dart: `readCapturedInNode records assignments in nested closures`
    #[test]
    fn read_captured_in_node_records_reads_in_nested_closures() {
        let mut av = new_av();
        let v1 = Variable("v1");
        av.declare(v1, false);
        av.begin_node();
        av.begin_node();
        av.begin_node();
        av.read(v1);
        av.end_node(new_node(), true);
        let inner_node = new_node();
        av.end_node(inner_node, false);
        let outer_node = new_node();
        av.end_node(outer_node, false);
        av.finish();
        assert_keys(
            av.read_captured_in_node(inner_node),
            &expected_keys(&av, &[v1]),
        );
        assert_keys(
            av.read_captured_in_node(outer_node),
            &expected_keys(&av, &[v1]),
        );
    }

    /// Dart: `capturedInNode ignores assignments in non-nested closures`
    #[test]
    fn captured_in_node_ignores_assignments_in_non_nested_closures() {
        let mut av = new_av();
        let (v1, v2) = (Variable("v1"), Variable("v2"));
        av.declare(v1, false);
        av.declare(v2, false);
        av.begin_node();
        av.write(v1);
        av.end_node(new_node(), true);
        av.begin_node();
        let node = new_node();
        av.end_node(node, false);
        av.begin_node();
        av.write(v2);
        av.end_node(new_node(), true);
        av.finish();
        assert!(av.captured_in_node(node).is_empty());
    }

    /// Dart: `capturedInNode records assignments in nested closures`
    #[test]
    fn captured_in_node_records_assignments_in_nested_closures() {
        let mut av = new_av();
        let v1 = Variable("v1");
        av.declare(v1, false);
        av.begin_node();
        av.begin_node();
        av.begin_node();
        av.write(v1);
        av.end_node(new_node(), true);
        let inner_node = new_node();
        av.end_node(inner_node, false);
        let outer_node = new_node();
        av.end_node(outer_node, false);
        av.finish();
        assert_keys(av.captured_in_node(inner_node), &expected_keys(&av, &[v1]));
        assert_keys(av.captured_in_node(outer_node), &expected_keys(&av, &[v1]));
    }
}

mod percolation {
    use super::*;

    /// Dart: `Variables do not percolate beyond the scope they were declared in`
    /// group, test `Non-closure scope`
    #[test]
    fn non_closure_scope() {
        let mut av = new_av();
        let (v1, v2) = (Variable("v1"), Variable("v2"));
        av.begin_node();
        av.begin_node();
        av.declare(v1, false);
        av.declare(v2, false);
        av.read(v1);
        av.write(v1);
        av.begin_node();
        av.read(v2);
        av.write(v2);
        av.end_node(new_node(), true);
        let inner_node = new_node();
        av.end_node(inner_node, false);
        let outer_node = new_node();
        av.end_node(outer_node, false);
        av.finish();
        for node in [inner_node, outer_node] {
            assert!(av.read_in_node(node).is_empty());
            assert!(av.written_in_node(node).is_empty());
            assert!(av.read_captured_in_node(node).is_empty());
            assert!(av.captured_in_node(node).is_empty());
        }
    }

    /// Dart: `Variables do not percolate beyond the scope they were declared in`
    /// group, test `Closure scope`
    #[test]
    fn closure_scope() {
        let mut av = new_av();
        let (v1, v2) = (Variable("v1"), Variable("v2"));
        av.begin_node();
        av.begin_node();
        av.declare(v1, false);
        av.declare(v2, false);
        av.read(v1);
        av.write(v1);
        av.begin_node();
        av.read(v2);
        av.write(v2);
        av.end_node(new_node(), true);
        let inner_node = new_node();
        av.end_node(inner_node, true);
        let outer_node = new_node();
        av.end_node(outer_node, false);
        av.finish();
        for node in [inner_node, outer_node] {
            assert!(av.read_in_node(node).is_empty());
            assert!(av.written_in_node(node).is_empty());
            assert!(av.read_captured_in_node(node).is_empty());
            assert!(av.captured_in_node(node).is_empty());
        }
    }
}

mod discard_node {
    use super::*;

    /// Dart: `discardNode percolates declarations to enclosing node`
    #[test]
    fn percolates_declarations_to_enclosing_node() {
        let mut av = new_av();
        let (v1, v2) = (Variable("v1"), Variable("v2"));
        let node = new_node();
        av.begin_node();
        av.begin_node();
        av.declare(v1, false);
        av.declare(v2, false);
        av.discard_node();
        av.end_node(node, false);
        av.finish();
        assert_keys(av.declared_in_node(node), &expected_keys(&av, &[v1, v2]));
    }

    /// Dart: `discardNode percolates reads to enclosing node`
    #[test]
    fn percolates_reads_to_enclosing_node() {
        let mut av = new_av();
        let (v1, v2) = (Variable("v1"), Variable("v2"));
        let node = new_node();
        av.declare(v1, false);
        av.declare(v2, false);
        av.begin_node();
        av.begin_node();
        av.read(v1);
        av.read(v2);
        av.discard_node();
        av.end_node(node, false);
        av.finish();
        assert_keys(av.read_in_node(node), &expected_keys(&av, &[v1, v2]));
    }

    /// Dart: `discardNode percolates writes to enclosing node`
    #[test]
    fn percolates_writes_to_enclosing_node() {
        let mut av = new_av();
        let (v1, v2) = (Variable("v1"), Variable("v2"));
        let node = new_node();
        av.declare(v1, false);
        av.declare(v2, false);
        av.begin_node();
        av.begin_node();
        av.write(v1);
        av.write(v2);
        av.discard_node();
        av.end_node(node, false);
        av.finish();
        assert_keys(av.written_in_node(node), &expected_keys(&av, &[v1, v2]));
    }

    /// Dart: `discardNode percolates read captures to enclosing node`
    #[test]
    fn percolates_read_captures_to_enclosing_node() {
        let mut av = new_av();
        let (v1, v2) = (Variable("v1"), Variable("v2"));
        let node = new_node();
        av.declare(v1, false);
        av.declare(v2, false);
        av.begin_node();
        av.begin_node();
        av.begin_node();
        av.read(v1);
        av.read(v2);
        av.end_node(new_node(), true);
        av.discard_node();
        av.end_node(node, false);
        av.finish();
        assert_keys(
            av.read_captured_in_node(node),
            &expected_keys(&av, &[v1, v2]),
        );
    }

    /// Dart: `discardNode percolates write captures to enclosing node`
    #[test]
    fn percolates_write_captures_to_enclosing_node() {
        let mut av = new_av();
        let (v1, v2) = (Variable("v1"), Variable("v2"));
        let node = new_node();
        av.declare(v1, false);
        av.declare(v2, false);
        av.begin_node();
        av.begin_node();
        av.begin_node();
        av.write(v1);
        av.write(v2);
        av.end_node(new_node(), true);
        av.discard_node();
        av.end_node(node, false);
        av.finish();
        assert_keys(av.captured_in_node(node), &expected_keys(&av, &[v1, v2]));
    }
}

mod defer_node {
    use super::*;

    /// Dart: `deferNode allows deferring of node info`
    #[test]
    fn allows_deferring_of_node_info() {
        let mut av = new_av();
        let (v1, v2, v3, v4) = (
            Variable("v1"),
            Variable("v2"),
            Variable("v3"),
            Variable("v4"),
        );
        av.declare(v1, false);
        av.declare(v2, false);
        av.declare(v4, false);
        av.begin_node();
        av.write(v1);
        av.declare(v3, false);
        av.begin_node();
        av.write(v2);
        av.end_node(new_node(), true);
        let info = av.defer_node(false);
        av.begin_node();
        av.write(v4);
        av.end_node(new_node(), false);
        let node = new_node();
        av.store_info(node, info);
        av.finish();
        assert_keys(av.declared_in_node(node), &expected_keys(&av, &[v3]));
        assert_keys(av.written_in_node(node), &expected_keys(&av, &[v1, v2]));
        assert_keys(av.captured_in_node(node), &expected_keys(&av, &[v2]));
    }
}

// Dart `assert` is ported as `debug_assert!`, so these tests need debug assertions.
#[cfg(debug_assertions)]
mod final_assertions {
    use super::*;

    /// Dart: `Final assertions:` group, test `finish may not be called twice`
    #[test]
    #[should_panic(expected = "assertion failed: !self.is_finished")]
    fn finish_may_not_be_called_twice() {
        let mut av = new_av();
        av.finish();
        av.finish();
    }

    /// Dart: `Final assertions:` group, test `all deferred infos must be stored`
    #[test]
    #[should_panic(expected = "Deferred infos not stored")]
    fn all_deferred_infos_must_be_stored() {
        let mut av = new_av();
        av.begin_node();
        let _info = av.defer_node(false);
        av.finish();
    }

    /// Dart: `Final assertions:` group, test `all open nodes must be closed or deferred`
    #[test]
    #[should_panic(expected = "Unexpected stack")]
    fn all_open_nodes_must_be_closed_or_deferred() {
        let mut av = new_av();
        av.begin_node();
        av.finish();
    }

    /// Dart: `Final assertions:` group, test `all read variables must be declared`
    #[test]
    #[should_panic(expected = "Variables read from but not declared")]
    fn all_read_variables_must_be_declared() {
        let mut av = new_av();
        av.read(Variable("v1"));
        av.finish();
    }

    /// Dart: `Final assertions:` group, test `all written variables must be declared`
    #[test]
    #[should_panic(expected = "Variables written to but not declared")]
    fn all_written_variables_must_be_declared() {
        let mut av = new_av();
        av.write(Variable("v1"));
        av.finish();
    }
}
