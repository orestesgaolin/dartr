//! [`LookupMap`] / [`LookupSet`]: hash maps for pure lookups (Dart maps that
//! are never iterated). They have no iteration API, so their random order
//! cannot reach any output (design §2.5). A Dart `Map`/`Set` that is
//! iterated becomes `indexmap::IndexMap` / `IndexSet` (insertion order, like
//! Dart's default `LinkedHashMap`).

#![allow(clippy::disallowed_types)]

use std::borrow::Borrow;
use std::hash::Hash;

use rustc_hash::FxBuildHasher;

/// A hash map without iteration.
#[derive(Clone, Debug)]
pub struct LookupMap<K, V>(hashbrown::HashMap<K, V, FxBuildHasher>);

impl<K, V> Default for LookupMap<K, V> {
    fn default() -> Self {
        LookupMap(hashbrown::HashMap::with_hasher(FxBuildHasher))
    }
}

impl<K: Hash + Eq, V> LookupMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get<Q: Hash + Eq + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        self.0.get(key)
    }

    pub fn get_mut<Q: Hash + Eq + ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
    {
        self.0.get_mut(key)
    }

    pub fn contains_key<Q: Hash + Eq + ?Sized>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
    {
        self.0.contains_key(key)
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.0.insert(key, value)
    }

    pub fn remove<Q: Hash + Eq + ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
    {
        self.0.remove(key)
    }

    /// Dart `putIfAbsent`.
    pub fn get_or_insert_with(&mut self, key: K, f: impl FnOnce() -> V) -> &mut V {
        self.0.entry(key).or_insert_with(f)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A hash set without iteration.
#[derive(Clone, Debug)]
pub struct LookupSet<T>(hashbrown::HashSet<T, FxBuildHasher>);

impl<T> Default for LookupSet<T> {
    fn default() -> Self {
        LookupSet(hashbrown::HashSet::with_hasher(FxBuildHasher))
    }
}

impl<T: Hash + Eq> LookupSet<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds [value]; `true` when it was new (Dart `Set.add`).
    pub fn insert(&mut self, value: T) -> bool {
        self.0.insert(value)
    }

    pub fn contains<Q: Hash + Eq + ?Sized>(&self, value: &Q) -> bool
    where
        T: Borrow<Q>,
    {
        self.0.contains(value)
    }

    pub fn remove<Q: Hash + Eq + ?Sized>(&mut self, value: &Q) -> bool
    where
        T: Borrow<Q>,
    {
        self.0.remove(value)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
