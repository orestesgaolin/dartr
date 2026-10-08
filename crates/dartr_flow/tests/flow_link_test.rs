//! Port of `pkg/_fe_analyzer_shared/test/flow_analysis/flow_link_test.dart`.
//!
//! Dart `group` blocks are modules (`get`, `diff`). Each Dart `test(...)` is
//! one `#[test]` function; its Dart name is given in the doc comment.
//!
//! Dart `_Link` (a `FlowLink` subclass with a `debugName`) is
//! `FlowLink<&'static str>`: the value is the debug name. Dart `identical`
//! is [`link_identical`] (`Rc::ptr_eq`).

use std::collections::BTreeMap;
use std::rc::Rc;

use dartr_flow::flow_link::{FlowLink, FlowLinkDiffEntry, FlowLinkReader, LinkRef, link_identical};

type Link = FlowLink<&'static str>;

/// Dart `_Link(debugName, key:, previous:, previousForKey:)`.
fn link(
    name: &'static str,
    key: usize,
    previous: Option<&Rc<Link>>,
    previous_for_key: Option<&Rc<Link>>,
) -> Rc<Link> {
    Rc::new(FlowLink::new(
        key,
        previous.map(Rc::clone),
        previous_for_key.map(Rc::clone),
        name,
    ))
}

/// Wraps a link as a (non-null) `LinkRef`.
fn at(link: &Rc<Link>) -> LinkRef<&'static str> {
    Some(Rc::clone(link))
}

/// Dart `check(actual).identicalTo(expected)`.
fn assert_identical(actual: &LinkRef<&'static str>, expected: &LinkRef<&'static str>) {
    assert!(
        link_identical(actual, expected),
        "expected {expected:?}, got {actual:?}"
    );
}

/// Dart extension `toMap()` on `List<FlowLinkDiffEntry<_Link>>`: each key
/// must occur at most once.
fn to_map<'a>(
    entries: &'a [FlowLinkDiffEntry<&'static str>],
) -> BTreeMap<usize, &'a FlowLinkDiffEntry<&'static str>> {
    let mut result = BTreeMap::new();
    for entry in entries {
        assert!(
            result.insert(entry.key, entry).is_none(),
            "duplicate diff entry for key {}",
            entry.key
        );
    }
    result
}

mod get {
    use super::*;

    /// Dart: `get: handles forward step`
    #[test]
    fn handles_forward_step() {
        let a = link("A", 0, None, None);
        let b = link("B", 0, Some(&a), Some(&a));
        let c = link("C", 0, Some(&b), Some(&b));
        let mut reader = FlowLinkReader::new();
        assert_identical(&reader.get(&None, 0), &None);
        assert_identical(&reader.get(&at(&a), 0), &at(&a));
        assert_identical(&reader.get(&at(&b), 0), &at(&b));
        assert_identical(&reader.get(&at(&c), 0), &at(&c));
    }

    /// Dart: `get: handles backward step`
    #[test]
    fn handles_backward_step() {
        let a = link("A", 0, None, None);
        let b = link("B", 0, Some(&a), Some(&a));
        let c = link("C", 0, Some(&b), Some(&b));
        let mut reader = FlowLinkReader::new();
        assert_identical(&reader.get(&at(&c), 0), &at(&c));
        assert_identical(&reader.get(&at(&b), 0), &at(&b));
        assert_identical(&reader.get(&at(&a), 0), &at(&a));
        assert_identical(&reader.get(&None, 0), &None);
    }

    /// Dart: `get: handles side step`
    #[test]
    fn handles_side_step() {
        //  B   C
        //   \ /
        //    A
        let a = link("A", 0, None, None);
        let b = link("B", 0, Some(&a), Some(&a));
        let c = link("C", 0, Some(&a), Some(&a));
        let mut reader = FlowLinkReader::new();
        assert_identical(&reader.get(&at(&b), 0), &at(&b));
        assert_identical(&reader.get(&at(&c), 0), &at(&c));
    }

    /// Dart: `get: handles multiple cache entries`
    #[test]
    fn handles_multiple_cache_entries() {
        let a = link("A", 0, None, None);
        let b = link("B", 1, Some(&a), None);
        let c = link("C", 2, Some(&b), None);
        let mut reader = FlowLinkReader::new();
        assert_identical(&reader.get(&at(&a), 1), &None);
        assert_identical(&reader.get(&at(&a), 2), &None);
        assert_identical(&reader.get(&at(&c), 1), &at(&b));
        assert_identical(&reader.get(&at(&c), 2), &at(&c));
    }
}

mod diff {
    use super::*;

    /// Dart: `diff: trivial null`
    #[test]
    fn trivial_null() {
        let mut reader = FlowLinkReader::new();
        let diff = reader.diff(&None, &None);
        assert_identical(&diff.ancestor, &None);
        assert!(diff.entries.is_empty());
    }

    /// Dart: `diff: trivial non-null`
    #[test]
    fn trivial_non_null() {
        let a = link("A", 0, None, None);
        let mut reader = FlowLinkReader::new();
        let diff = reader.diff(&at(&a), &at(&a));
        assert_identical(&diff.ancestor, &at(&a));
        assert!(diff.entries.is_empty());
    }

    /// Dart: `diff: finds common ancestor`
    #[test]
    fn finds_common_ancestor() {
        // E
        // |
        // D   G
        // |   |
        // C   F
        //  \ /
        //   B
        //   |
        //   A
        let a = link("A", 0, None, None);
        let b = link("B", 0, Some(&a), Some(&a));
        let c = link("C", 0, Some(&b), Some(&b));
        let d = link("D", 0, Some(&c), Some(&c));
        let e = link("E", 0, Some(&d), Some(&d));
        let f = link("F", 0, Some(&b), Some(&b));
        let g = link("G", 0, Some(&f), Some(&f));
        let mut reader = FlowLinkReader::new();
        assert_identical(&reader.diff(&at(&e), &at(&g)).ancestor, &at(&b));
    }

    /// Dart: `diff: stepLeft twice for the same key`
    #[test]
    fn step_left_twice_for_the_same_key() {
        let a = link("A", 0, None, None);
        let b = link("B", 0, Some(&a), Some(&a));
        let c = link("C", 0, Some(&b), Some(&b));
        let mut reader = FlowLinkReader::new();
        let diff = reader.diff(&at(&c), &at(&a));
        assert_eq!(diff.entries.len(), 1);
        let entry = &diff.entries[0];
        assert_eq!(entry.key, 0);
        assert_identical(&entry.ancestor(), &at(&a));
        assert_identical(&entry.left(), &at(&c));
        assert_identical(&entry.right(), &at(&a));
    }

    /// Dart: `diff: stepLeft handles multiple keys`
    #[test]
    fn step_left_handles_multiple_keys() {
        let a = link("A", 0, None, None);
        let b = link("B", 1, Some(&a), None);
        let c = link("C", 0, Some(&b), Some(&a));
        let d = link("D", 1, Some(&c), Some(&b));
        let mut reader = FlowLinkReader::new();
        let diff = reader.diff(&at(&d), &at(&b));
        assert_eq!(diff.entries.len(), 2);
        let entries = to_map(&diff.entries);
        assert_identical(&entries[&0].ancestor(), &at(&a));
        assert_identical(&entries[&0].left(), &at(&c));
        assert_identical(&entries[&0].right(), &at(&a));
        assert_identical(&entries[&1].ancestor(), &at(&b));
        assert_identical(&entries[&1].left(), &at(&d));
        assert_identical(&entries[&1].right(), &at(&b));
    }

    /// Dart: `diff: stepRight twice for the same key`
    #[test]
    fn step_right_twice_for_the_same_key() {
        let a = link("A", 0, None, None);
        let b = link("B", 0, Some(&a), Some(&a));
        let c = link("C", 0, Some(&b), Some(&b));
        let mut reader = FlowLinkReader::new();
        let diff = reader.diff(&at(&a), &at(&c));
        assert_eq!(diff.entries.len(), 1);
        let entry = &diff.entries[0];
        assert_eq!(entry.key, 0);
        assert_identical(&entry.ancestor(), &at(&a));
        assert_identical(&entry.left(), &at(&a));
        assert_identical(&entry.right(), &at(&c));
    }

    /// Dart: `diff: stepRight handles multiple keys`
    #[test]
    fn step_right_handles_multiple_keys() {
        let a = link("A", 0, None, None);
        let b = link("B", 1, Some(&a), None);
        let c = link("C", 0, Some(&b), Some(&a));
        let d = link("D", 1, Some(&c), Some(&b));
        let mut reader = FlowLinkReader::new();
        let diff = reader.diff(&at(&b), &at(&d));
        assert_eq!(diff.entries.len(), 2);
        let entries = to_map(&diff.entries);
        assert_identical(&entries[&0].ancestor(), &at(&a));
        assert_identical(&entries[&0].left(), &at(&a));
        assert_identical(&entries[&0].right(), &at(&c));
        assert_identical(&entries[&1].ancestor(), &at(&b));
        assert_identical(&entries[&1].left(), &at(&b));
        assert_identical(&entries[&1].right(), &at(&d));
    }

    /// Dart: `diff: multiple keys with stepLeft and stepRight`
    #[test]
    fn multiple_keys_with_step_left_and_step_right() {
        // B(0)   D(1)
        // |      |
        // A(1)   C(0)
        //  \    /
        //   null
        let a = link("A", 1, None, None);
        let b = link("B", 0, Some(&a), None);
        let c = link("C", 0, None, None);
        let d = link("D", 1, Some(&c), None);
        let mut reader = FlowLinkReader::new();
        let diff = reader.diff(&at(&b), &at(&d));
        assert_eq!(diff.entries.len(), 2);
        let entries = to_map(&diff.entries);
        assert_identical(&entries[&0].ancestor(), &None);
        assert_identical(&entries[&0].left(), &at(&b));
        assert_identical(&entries[&0].right(), &at(&c));
        assert_identical(&entries[&1].ancestor(), &None);
        assert_identical(&entries[&1].left(), &at(&a));
        assert_identical(&entries[&1].right(), &at(&d));
    }

    /// Dart: `diff: diffs only on one side`
    #[test]
    fn diffs_only_on_one_side() {
        // A(1)   B(0)
        //  \    /
        //   null
        let a = link("A", 1, None, None);
        let b = link("B", 0, None, None);
        let mut reader = FlowLinkReader::new();
        let diff = reader.diff(&at(&a), &at(&b));
        assert_eq!(diff.entries.len(), 2);
        let entries = to_map(&diff.entries);
        assert_identical(&entries[&0].ancestor(), &None);
        assert_identical(&entries[&0].left(), &None);
        assert_identical(&entries[&0].right(), &at(&b));
        assert_identical(&entries[&1].ancestor(), &None);
        assert_identical(&entries[&1].left(), &at(&a));
        assert_identical(&entries[&1].right(), &None);
    }
}
