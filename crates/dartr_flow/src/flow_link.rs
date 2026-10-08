// Dart source: pkg/_fe_analyzer_shared/lib/src/flow_analysis/flow_link.dart

//! An efficient immutable map data structure that tracks program state:
//! [`FlowLink`] and [`FlowLinkReader`].
//!
//! Dart `FlowLink<Link>` is an abstract base class whose subclasses add the
//! value (for flow analysis: `PromotionInfo`, which adds a `PromotionModel`).
//! Here [`FlowLink<T>`] is a generic struct holding the value of type `T`,
//! and links are shared with [`Rc`]. Dart `identical` on links is
//! [`Rc::ptr_eq`] (see [`link_identical`]); `null` is `None`.

use std::fmt;
use std::rc::Rc;

/// A shared, possibly absent link (Dart `Link?`).
pub type LinkRef<T> = Option<Rc<FlowLink<T>>>;

/// Dart `identical(a, b)` for two (possibly `null`) links.
pub fn link_identical<T>(a: &LinkRef<T>, b: &LinkRef<T>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        _ => false,
    }
}

/// Basis for an efficient immutable map data structure that can track
/// program state.
///
/// Each instance of [`FlowLink`] represents a key/value pair, where the
/// [`key`](Self::key) is a non-negative integer, and the value is stored in
/// [`value`](Self::value); a map is formed by chaining together multiple
/// [`FlowLink`] objects through [`previous`](Self::previous) pointers. In this
/// way, a collection of [`FlowLink`] objects can be used to model program
/// state, with each [`FlowLink`] representing a change to a single state
/// variable, and the `previous` pointer pointing to the previous state of
/// the program. In this interpretation, `None` represents the initial
/// program state, in which all state variables take on their default values.
///
/// Multiple [`FlowLink`] objects are allowed to point to the same `previous`
/// object; in this way, all [`FlowLink`] objects implicitly form a tree, with
/// `None` at the root. In the interpretation where a collection of
/// [`FlowLink`] objects are used to model program state, and a single
/// [`FlowLink`] represents a change to a single state variable, the tree
/// corresponds to the dominator tree. There are no "child" pointers, so the
/// tree may only be traversed in the leaf-to-root direction, and once a
/// branch is no longer needed it is reclaimed.
///
/// The [`FlowLinkReader`] type may be used to efficiently look up map
/// entries in a given [`FlowLink`] object. It makes use of the fact that
/// keys are non-negative integers to maintain a current state in a list.
pub struct FlowLink<T> {
    /// The integer key for this [`FlowLink`]. In the interpretation where a
    /// collection of [`FlowLink`] objects are used to model program state,
    /// and a single [`FlowLink`] represents a change to a single state
    /// variable, this key tells which state variable has changed.
    pub key: usize,

    /// Pointer allowing multiple [`FlowLink`] objects to be joined into a
    /// singly linked list. In the interpretation where a collection of
    /// [`FlowLink`] objects are used to model program state, and a single
    /// [`FlowLink`] represents a change to a single state variable, this
    /// pointer points to the state of the program prior to the change.
    pub previous: LinkRef<T>,

    /// Pointer to the nearest [`FlowLink`] in the `previous` chain whose
    /// `key` matches this one, or `None` if there is no matching
    /// [`FlowLink`]. This is used by [`FlowLinkReader`] to quickly update its
    /// state representation when traversing the implicit tree of
    /// [`FlowLink`] objects.
    pub previous_for_key: LinkRef<T>,

    /// The number of `previous` links that need to be traversed to reach
    /// `None`. This is used by [`FlowLinkReader`] to quickly find the common
    /// ancestor of two points in the implicit tree of [`FlowLink`] objects.
    depth: usize,

    /// The value stored in this link (the field added by the Dart subclass,
    /// e.g. `PromotionInfo.model`).
    pub value: T,
}

impl<T> FlowLink<T> {
    /// Creates a new [`FlowLink`] object. Caller is required to satisfy the
    /// invariant described in [`previous_for_key`](Self::previous_for_key).
    pub fn new(key: usize, previous: LinkRef<T>, previous_for_key: LinkRef<T>, value: T) -> Self {
        let depth = depth_of(&previous) + 1;
        let link = FlowLink {
            key,
            previous,
            previous_for_key,
            depth,
            value,
        };
        debug_assert!(link_identical(
            &link.previous_for_key,
            &link.compute_previous_for_key(key)
        ));
        link
    }

    /// Debug only: computes the correct value for `previous_for_key`, to
    /// check that the caller supplied the appropriate value to the
    /// constructor.
    fn compute_previous_for_key(&self, key: usize) -> LinkRef<T> {
        let mut link = self.previous.clone();
        while let Some(l) = link {
            if l.key == key {
                return Some(l);
            }
            link = l.previous.clone();
        }
        None
    }
}

impl<T: fmt::Debug> fmt::Debug for FlowLink<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlowLink")
            .field("key", &self.key)
            .field("value", &self.value)
            .finish_non_exhaustive()
    }
}

/// Gets the `_depth` of `link`, or `0` if `link` is `None`.
fn depth_of<T>(link: &LinkRef<T>) -> usize {
    match link {
        None => 0,
        Some(l) => l.depth,
    }
}

/// Information about a difference between two program states, returned by
/// [`FlowLinkReader::diff`].
pub struct FlowLinkDiffEntry<T> {
    /// The key that differs between the [`FlowLink`] maps passed to
    /// [`FlowLinkReader::diff`].
    pub key: usize,

    /// During a diff operation, the first [`FlowLink`] associated with `key`
    /// that was found while walking the `previous` chain for the `left`
    /// argument, or `None` if no such key has been found yet.
    first_left: LinkRef<T>,

    /// During a diff operation, the first [`FlowLink`] associated with `key`
    /// that was found while walking the `previous` chain for the `right`
    /// argument, or `None` if no such key has been found yet.
    first_right: LinkRef<T>,

    /// During a diff operation, the value of `previous_for_key` that was most
    /// recently encountered while walking the `previous` chains for both the
    /// `left` and `right` arguments.
    previous_for_key: LinkRef<T>,
}

impl<T> FlowLinkDiffEntry<T> {
    /// The [`FlowLink`] associated with `key` in the common ancestor of the
    /// two [`FlowLink`] maps passed to [`FlowLinkReader::diff`], or `None` if
    /// the common ancestor doesn't associate any [`FlowLink`] with `key`.
    pub fn ancestor(&self) -> LinkRef<T> {
        // This is called by a client after the `diff` operation has
        // completed. Therefore, `previous_for_key` comes from the `FlowLink`
        // that the `diff` operation visited last; i.e. the one closest to the
        // common ancestor node. So it *is* the common ancestor for the given
        // key.
        self.previous_for_key.clone()
    }

    /// The [`FlowLink`] associated with `key` in the `left` [`FlowLink`] map
    /// passed to [`FlowLinkReader::diff`], or `None` if the `left` map
    /// doesn't associate any [`FlowLink`] with `key`.
    pub fn left(&self) -> LinkRef<T> {
        // Either the first link encountered while traversing the left side,
        // or no link with the given key was encountered on the left side, in
        // which case `previous_for_key` is the common ancestor for the key.
        self.first_left
            .clone()
            .or_else(|| self.previous_for_key.clone())
    }

    /// The [`FlowLink`] associated with `key` in the `right` [`FlowLink`] map
    /// passed to [`FlowLinkReader::diff`], or `None` if the `right` map
    /// doesn't associate any [`FlowLink`] with `key`.
    pub fn right(&self) -> LinkRef<T> {
        self.first_right
            .clone()
            .or_else(|| self.previous_for_key.clone())
    }
}

/// The result of [`FlowLinkReader::diff`]: the common ancestor and the list
/// of differences (Dart record `({Link? ancestor, List<...> entries})`).
pub struct FlowLinkDiff<T> {
    /// The common ancestor of `left` and `right`.
    pub ancestor: LinkRef<T>,
    /// The differences among `left`, `right`, and their common ancestor.
    pub entries: Vec<FlowLinkDiffEntry<T>>,
}

/// Efficient mechanism for looking up entries in the map formed implicitly
/// by a linked list of [`FlowLink`] objects, and for finding the difference
/// between two such maps.
///
/// This type works by maintaining a "current" pointer recording the
/// [`FlowLink`] that was most recently passed to [`get`](Self::get), and a
/// cache of the state of all state variables implied by that [`FlowLink`]
/// object. The cache can be updated in O(n) time, where n is the number of
/// tree edges between one state and another. Accordingly, for maximum
/// efficiency, the caller should try not to jump around the tree too much in
/// successive calls to [`get`](Self::get).
pub struct FlowLinkReader<T> {
    /// The [`FlowLink`] pointer most recently passed to `set_current`.
    current: LinkRef<T>,

    /// A cache of the lookup results that should be returned by `get` for
    /// each possible integer key, for the `current` link.
    cache: Vec<LinkRef<T>>,

    /// Temporary scratch area used by `diff_core`. Each non-`None` entry
    /// represents an index into the list of entries that `diff_core` will
    /// return; that entry has the same integer key as the corresponding
    /// index into this list.
    diff_indices: Vec<Option<usize>>,
}

impl<T> Default for FlowLinkReader<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> FlowLinkReader<T> {
    /// Creates a reader with an empty cache.
    pub fn new() -> Self {
        FlowLinkReader {
            current: None,
            cache: Vec::new(),
            diff_indices: Vec::new(),
        }
    }

    /// Computes the difference between [`FlowLink`] states represented by
    /// `left` and `right`.
    ///
    /// Two values are returned: the common ancestor of `left` and `right`,
    /// and a list of [`FlowLinkDiffEntry`] objects representing the
    /// difference among `left`, `right`, and their common ancestor.
    ///
    /// If `left` and `right` are identical, this method has time complexity
    /// `O(1)`. Otherwise, it has time complexity `O(n)`, where `n` is the
    /// number of edges between `left` and `right` in the implicit
    /// [`FlowLink`] tree.
    pub fn diff(&mut self, left: &LinkRef<T>, right: &LinkRef<T>) -> FlowLinkDiff<T> {
        if link_identical(left, right) {
            return FlowLinkDiff {
                ancestor: left.clone(),
                entries: Vec::new(),
            };
        }
        let mut entries = Vec::new();
        let ancestor = self.diff_core(left.clone(), right.clone(), &mut entries);
        FlowLinkDiff { ancestor, entries }
    }

    /// Looks up the first entry in the linked list formed by
    /// [`FlowLink::previous`], starting at `link`, whose key matches `key`.
    /// If there is no such entry, `None` is returned.
    ///
    /// If `link` is `None` or matches `current`, this method has time
    /// complexity `O(1)`. In this circumstance, `current` is unchanged.
    ///
    /// Otherwise, this method has time complexity `O(n)`, where `n` is the
    /// number of edges between `current` and `link` in the implicit
    /// [`FlowLink`] tree. In this circumstance, `current` is set to `link`.
    pub fn get(&mut self, link: &LinkRef<T>, key: usize) -> LinkRef<T> {
        if link.is_none() {
            return None;
        }
        self.set_current(link);
        self.cache.get(key).cloned().flatten()
    }

    /// The core algorithm used by [`diff`](Self::diff) and `set_current`.
    /// Computes a difference between `left` and `right`, adding diff entries
    /// to `entries`. The return value is the common ancestor of `left` and
    /// `right`.
    fn diff_core(
        &mut self,
        mut left: LinkRef<T>,
        mut right: LinkRef<T>,
        entries: &mut Vec<FlowLinkDiffEntry<T>>,
    ) -> LinkRef<T> {
        // The core strategy is to traverse the implicit [FlowLink] tree,
        // starting at `left` and `right`, and taking single steps through the
        // linked list formed by `FlowLink.previous`, until a common ancestor
        // is found. For each step, an entry is added to the `entries` list
        // for the corresponding key (or a previously made entry is updated),
        // and `diff_indices` is modified to keep track of which keys have
        // corresponding entries.

        // Walk `left` and `right` back to their common ancestor.
        let mut left_depth = depth_of(&left);
        let mut right_depth = depth_of(&right);
        if left_depth > right_depth {
            loop {
                // `left.depth > right.depth`, therefore `left.depth > 0`, so
                // `left != null`.
                left = self.step_left(left.unwrap(), entries);
                left_depth -= 1;
                debug_assert_eq!(left_depth, depth_of(&left));
                if left_depth <= right_depth {
                    break;
                }
            }
        } else {
            while right_depth > left_depth {
                right = self.step_right(right.unwrap(), entries);
                right_depth -= 1;
                debug_assert_eq!(right_depth, depth_of(&right));
            }
        }
        while !link_identical(&left, &right) {
            debug_assert_eq!(depth_of(&left), depth_of(&right));
            // The only possible value with a depth of `0` is `None`.
            // Therefore, since `left.depth == right.depth`, `left` and
            // `right` must either be both `None` or both non-`None`. Since
            // they're not identical to one another, it follows that they're
            // both non-`None`.
            left = self.step_left(left.unwrap(), entries);
            right = self.step_right(right.unwrap(), entries);
        }

        // Clear `diff_indices` for the next call to this method. (Dart does
        // this in a `finally` clause.)
        for (i, entry) in entries.iter().enumerate() {
            // Since `diff_indices` was constructed as an index into
            // `entries`, we know that `diff_indices[entry.key] == i`.
            debug_assert_eq!(self.diff_indices[entry.key], Some(i));
            self.diff_indices[entry.key] = None;
        }

        left
    }

    /// Takes a single step from `left` through the linked list formed by
    /// `FlowLink.previous`, updating `entries` and `diff_indices` as
    /// appropriate.
    fn step_left(
        &mut self,
        left: Rc<FlowLink<T>>,
        entries: &mut Vec<FlowLinkDiffEntry<T>>,
    ) -> LinkRef<T> {
        let key = left.key;
        match self.diff_indices.get(key).copied().flatten() {
            None => {
                // No diff entry has been created for this key yet, so create
                // one.
                list_set(&mut self.diff_indices, key, Some(entries.len()));
                entries.push(FlowLinkDiffEntry {
                    key,
                    first_left: Some(left.clone()),
                    first_right: None,
                    previous_for_key: left.previous_for_key.clone(),
                });
            }
            Some(index) => {
                // A diff entry for this key has already been created, so
                // update it.
                let entry = &mut entries[index];
                if entry.first_left.is_none() {
                    entry.first_left = Some(left.clone());
                }
                entry.previous_for_key = left.previous_for_key.clone();
            }
        }
        left.previous.clone()
    }

    /// Takes a single step from `right` through the linked list formed by
    /// `FlowLink.previous`, updating `entries` and `diff_indices` as
    /// appropriate.
    fn step_right(
        &mut self,
        right: Rc<FlowLink<T>>,
        entries: &mut Vec<FlowLinkDiffEntry<T>>,
    ) -> LinkRef<T> {
        let key = right.key;
        match self.diff_indices.get(key).copied().flatten() {
            None => {
                list_set(&mut self.diff_indices, key, Some(entries.len()));
                entries.push(FlowLinkDiffEntry {
                    key,
                    first_left: None,
                    first_right: Some(right.clone()),
                    previous_for_key: right.previous_for_key.clone(),
                });
            }
            Some(index) => {
                let entry = &mut entries[index];
                if entry.first_right.is_none() {
                    entry.first_right = Some(right.clone());
                }
                entry.previous_for_key = right.previous_for_key.clone();
            }
        }
        right.previous.clone()
    }

    /// Sets `current` to `value`, updating `cache` in the process.
    fn set_current(&mut self, value: &LinkRef<T>) {
        if link_identical(value, &self.current) {
            return;
        }
        let mut entries = Vec::new();
        let current = self.current.clone();
        self.diff_core(current, value.clone(), &mut entries);
        for entry in &entries {
            list_set(&mut self.cache, entry.key, entry.right());
        }
        self.current = value.clone();
    }
}

/// Stores `value` in the `index`th entry of `list`, increasing the length of
/// `list` if necessary (Dart extension method `set` on `List<T?>`).
fn list_set<V: Default>(list: &mut Vec<V>, index: usize, value: V) {
    while index >= list.len() {
        list.push(V::default());
    }
    list[index] = value;
}
