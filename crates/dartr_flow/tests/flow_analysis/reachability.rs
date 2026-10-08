// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_test.dart
// (lines 3593-3720: group 'Reachability')

//! Dart group `Reachability`: tests of the [`Reachability`] data model.

use super::common::*;

/// Dart `expect(a, same(b))` for a `Reachability?` and a `Reachability`.
fn same(a: Option<&Reachability>, b: &Reachability) -> bool {
    Reachability::opt_ptr_eq(a, Some(b))
}

#[test]
fn initial_state() {
    let _h = set_up();
    assert!(Reachability::initial().parent.is_none());
    assert!(Reachability::initial().locally_reachable);
    assert!(Reachability::initial().overall_reachable);
}

#[test]
fn split() {
    let _h = set_up();
    let reachable_split = Reachability::initial().split();
    assert!(same(
        reachable_split.parent.as_ref(),
        &Reachability::initial()
    ));
    assert!(reachable_split.overall_reachable);
    assert!(reachable_split.locally_reachable);
    let unreachable = reachable_split.set_unreachable();
    let unreachable_split = unreachable.split();
    assert!(same(unreachable_split.parent.as_ref(), &unreachable));
    assert!(!unreachable_split.overall_reachable);
    assert!(unreachable_split.locally_reachable);
}

#[test]
fn unsplit() {
    let _h = set_up();
    let base = Reachability::initial().split();
    let reachable_split = base.split();
    let reachable_split_unsplit = reachable_split.unsplit();
    assert!(Reachability::opt_ptr_eq(
        reachable_split_unsplit.parent.as_ref(),
        base.parent.as_ref()
    ));
    assert!(reachable_split_unsplit.overall_reachable);
    assert!(reachable_split_unsplit.locally_reachable);
    let reachable_split_unreachable = reachable_split.set_unreachable();
    let reachable_split_unreachable_unsplit = reachable_split_unreachable.unsplit();
    assert!(Reachability::opt_ptr_eq(
        reachable_split_unreachable_unsplit.parent.as_ref(),
        base.parent.as_ref()
    ));
    assert!(!reachable_split_unreachable_unsplit.overall_reachable);
    assert!(!reachable_split_unreachable_unsplit.locally_reachable);
    let unreachable = base.set_unreachable();
    let unreachable_split = unreachable.split();
    let unreachable_split_unsplit = unreachable_split.unsplit();
    assert!(unreachable_split_unsplit.ptr_eq(&unreachable));
    let unreachable_split_unreachable = unreachable_split.set_unreachable();
    let unreachable_split_unreachable_unsplit = unreachable_split_unreachable.unsplit();
    assert!(unreachable_split_unreachable_unsplit.ptr_eq(&unreachable));
}

#[test]
fn set_unreachable() {
    let _h = set_up();
    let reachable = Reachability::initial().split();
    let unreachable = reachable.set_unreachable();
    assert!(Reachability::opt_ptr_eq(
        unreachable.parent.as_ref(),
        reachable.parent.as_ref()
    ));
    assert!(!unreachable.locally_reachable);
    assert!(!unreachable.overall_reachable);
    assert!(unreachable.set_unreachable().ptr_eq(&unreachable));
    let provisionally_reachable = unreachable.split();
    let provisionally_unreachable = provisionally_reachable.set_unreachable();
    assert!(Reachability::opt_ptr_eq(
        provisionally_unreachable.parent.as_ref(),
        provisionally_reachable.parent.as_ref()
    ));
    assert!(!provisionally_unreachable.locally_reachable);
    assert!(!provisionally_unreachable.overall_reachable);
    assert!(
        provisionally_unreachable
            .set_unreachable()
            .ptr_eq(&provisionally_unreachable)
    );
}

#[test]
fn rebase_forward() {
    let _h = set_up();
    let previous = Reachability::initial();
    let reachable = previous.split();
    let reachable2 = previous.split();
    let unreachable = reachable.set_unreachable();
    let unreachable_previous = previous.set_unreachable();
    let reachable3 = unreachable_previous.split();
    assert!(reachable.rebase_forward(&reachable).ptr_eq(&reachable));
    assert!(reachable.rebase_forward(&reachable2).ptr_eq(&reachable2));
    assert!(reachable.rebase_forward(&unreachable).ptr_eq(&unreachable));
    assert!(same(
        unreachable.rebase_forward(&reachable).parent.as_ref(),
        &previous
    ));
    assert!(!unreachable.rebase_forward(&reachable).locally_reachable);
    assert!(
        unreachable
            .rebase_forward(&unreachable)
            .ptr_eq(&unreachable)
    );
    assert!(
        reachable
            .rebase_forward(&unreachable_previous)
            .ptr_eq(&unreachable_previous)
    );
    assert!(same(
        unreachable_previous
            .rebase_forward(&reachable)
            .parent
            .as_ref(),
        &previous
    ));
    assert!(
        !unreachable_previous
            .rebase_forward(&reachable)
            .locally_reachable
    );
    assert!(reachable.rebase_forward(&reachable3).ptr_eq(&reachable3));
    assert!(same(
        reachable3.rebase_forward(&reachable).parent.as_ref(),
        &previous
    ));
    assert!(!reachable3.rebase_forward(&reachable).locally_reachable);
}

#[test]
fn common_ancestor() {
    let _h = set_up();
    let parent1 = Reachability::initial();
    let parent2 = parent1.set_unreachable();
    let child1 = parent1.split();
    let child2 = parent1.split();
    let child3 = child1.split();
    let child4 = child2.split();
    let ca =
        |a: Option<&Reachability>, b: Option<&Reachability>| Reachability::common_ancestor(a, b);
    assert!(ca(None, None).is_none());
    assert!(ca(None, Some(&parent1)).is_none());
    assert!(ca(Some(&parent1), None).is_none());
    assert!(ca(None, Some(&child1)).is_none());
    assert!(ca(Some(&child1), None).is_none());
    assert!(ca(None, Some(&child3)).is_none());
    assert!(ca(Some(&child3), None).is_none());
    assert!(same(ca(Some(&parent1), Some(&parent1)).as_ref(), &parent1));
    assert!(ca(Some(&parent1), Some(&parent2)).is_none());
    assert!(ca(Some(&parent2), Some(&child1)).is_none());
    assert!(ca(Some(&child1), Some(&parent2)).is_none());
    assert!(ca(Some(&parent2), Some(&child3)).is_none());
    assert!(ca(Some(&child3), Some(&parent2)).is_none());
    assert!(same(ca(Some(&parent1), Some(&child1)).as_ref(), &parent1));
    assert!(same(ca(Some(&child1), Some(&parent1)).as_ref(), &parent1));
    assert!(same(ca(Some(&parent1), Some(&child3)).as_ref(), &parent1));
    assert!(same(ca(Some(&child3), Some(&parent1)).as_ref(), &parent1));
    assert!(same(ca(Some(&child1), Some(&child1)).as_ref(), &child1));
    assert!(same(ca(Some(&child1), Some(&child2)).as_ref(), &parent1));
    assert!(same(ca(Some(&child1), Some(&child3)).as_ref(), &child1));
    assert!(same(ca(Some(&child3), Some(&child1)).as_ref(), &child1));
    assert!(same(ca(Some(&child1), Some(&child4)).as_ref(), &parent1));
    assert!(same(ca(Some(&child4), Some(&child1)).as_ref(), &parent1));
    assert!(same(ca(Some(&child3), Some(&child3)).as_ref(), &child3));
    assert!(same(ca(Some(&child3), Some(&child4)).as_ref(), &parent1));
}
