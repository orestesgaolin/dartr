// Dart source: pkg/_fe_analyzer_shared/lib/src/type_inference/promotion_key_store.dart

//! [`PromotionKeyStore`]: assigns a unique integer to everything that might
//! undergo promotion.

use indexmap::IndexMap;
use std::cell::RefCell;
use std::hash::Hash;

use crate::flow_analysis::PromotionKey;

/// This data structure assigns a unique integer identifier to everything
/// that might undergo promotion in the user's code (local variables and
/// properties). An integer identifier is also assigned to `this` (even
/// though `this` is not promotable), because promotable properties can be
/// reached using `this` as a starting point.
///
/// Flow analysis allocates keys while it only holds shared references to
/// its helper (Dart objects are freely mutable), so the store uses interior
/// mutability; all methods take `&self`.
#[derive(Debug)]
pub struct PromotionKeyStore<V> {
    /// Special promotion key to represent `this` (Dart `late final`,
    /// allocated on first use).
    this_promotion_key: RefCell<Option<PromotionKey>>,

    /// Dart `Map<Variable, int>.identity()`. Only used for lookup, never
    /// iterated, so a `IndexMap` is fine.
    variable_keys: RefCell<IndexMap<V, PromotionKey>>,

    /// List whose `i`th entry is the variable corresponding to promotion key
    /// `i`, or `None`, if promotion key `i` does not correspond to a specific
    /// variable.
    key_to_variable: RefCell<Vec<Option<V>>>,
}

impl<V: Copy + Eq + Hash> Default for PromotionKeyStore<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: Copy + Eq + Hash> PromotionKeyStore<V> {
    /// Creates an empty store.
    pub fn new() -> Self {
        PromotionKeyStore {
            this_promotion_key: RefCell::new(None),
            variable_keys: RefCell::new(IndexMap::new()),
            key_to_variable: RefCell::new(Vec::new()),
        }
    }

    /// Special promotion key to represent `this`.
    pub fn this_promotion_key(&self) -> PromotionKey {
        if let Some(key) = *self.this_promotion_key.borrow() {
            return key;
        }
        let key = self.make_new_key(None);
        *self.this_promotion_key.borrow_mut() = Some(key);
        key
    }

    /// The promotion key of `variable`, allocated on first use.
    pub fn key_for_variable(&self, variable: V) -> PromotionKey {
        if let Some(key) = self.variable_keys.borrow().get(&variable) {
            return *key;
        }
        let key = self.make_new_key(Some(variable));
        self.variable_keys.borrow_mut().insert(variable, key);
        key
    }

    /// The promotion key of `variable` if one has been allocated already.
    ///
    /// Not in Dart: used by the queries of flow analysis that take `&self`
    /// (`isAssigned`, `promotedType`, ...). A key allocated by Dart in such a
    /// query has no promotion information, so the result is the same.
    pub fn existing_key_for_variable(&self, variable: V) -> Option<PromotionKey> {
        self.variable_keys.borrow().get(&variable).copied()
    }

    /// Creates a fresh promotion key that hasn't been used before (and won't
    /// be reused again). This is used by flow analysis to model the
    /// synthetic variables used during pattern matching to cache the values
    /// that the pattern, and its subpatterns, are being matched against. It
    /// is also used to track the values returned by property gets.
    pub fn make_temporary_key(&self) -> PromotionKey {
        self.make_new_key(None)
    }

    /// Gets the variable corresponding to `variable_key`, or `None` if
    /// `variable_key` does not correspond to a specific variable.
    pub fn variable_for_key(&self, variable_key: PromotionKey) -> Option<V> {
        self.key_to_variable.borrow()[variable_key as usize]
    }

    /// Creates a fresh promotion key. If a `variable` is provided, it is
    /// stored for later retrieval by [`variable_for_key`](Self::variable_for_key).
    fn make_new_key(&self, variable: Option<V>) -> PromotionKey {
        let mut key_to_variable = self.key_to_variable.borrow_mut();
        let key = key_to_variable.len() as PromotionKey;
        key_to_variable.push(variable);
        key
    }
}
