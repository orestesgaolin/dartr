//! Interning pools: the storage behind [`crate::Name`], [`crate::TypeId`],
//! [`crate::ListId`], [`crate::SubstId`], [`crate::MemberId`] and
//! [`crate::AliasId`].
//!
//! A [`Pool`] is append-only. Items live in a `boxcar::Vec` (lock-free reads;
//! a reference stays valid as long as the pool lives). Deduplication uses
//! `SHARDS` hash tables, each behind a `parking_lot::Mutex`; the shard is
//! chosen by the top bits of the hash. The find-or-insert of one value runs
//! under its shard lock, so two threads that intern equal values get the same
//! index.

use std::borrow::Borrow;
use std::hash::{BuildHasher, Hash};

use hashbrown::HashTable;
use parking_lot::Mutex;
use rustc_hash::FxBuildHasher;

/// The number of shards of a global pool.
pub const SHARDS: usize = 64;

/// One shard: entries (hash, index into `items`).
type Shard = Mutex<HashTable<(u64, u32)>>;

/// An append-only interning pool.
pub struct Pool<T> {
    /// Entries: (hash, index into `items`).
    shards: Box<[Shard]>,
    shift: u32,
    items: boxcar::Vec<T>,
}

impl<T: Hash + Eq> Pool<T> {
    /// A pool with [shards] shards (a power of two; 1 for a local pool).
    pub fn new(shards: usize) -> Pool<T> {
        assert!(shards.is_power_of_two());
        Pool {
            shards: (0..shards).map(|_| Mutex::new(HashTable::new())).collect(),
            shift: 64 - shards.trailing_zeros(),
            items: boxcar::Vec::new(),
        }
    }

    #[inline]
    fn hash<Q: Hash + ?Sized>(value: &Q) -> u64 {
        FxBuildHasher.hash_one(value)
    }

    #[inline]
    fn shard(&self, hash: u64) -> &Shard {
        if self.shards.len() == 1 {
            &self.shards[0]
        } else {
            &self.shards[(hash >> self.shift) as usize]
        }
    }

    /// Returns the index of [value], adding it when it is new.
    pub fn intern(&self, value: T) -> u32 {
        let hash = Self::hash(&value);
        let mut table = self.shard(hash).lock();
        if let Some(&(_, index)) =
            table.find(hash, |&(h, i)| h == hash && self.items[i as usize] == value)
        {
            return index;
        }
        let index = u32::try_from(self.items.push(value)).expect("pool overflow");
        table.insert_unique(hash, (hash, index), |&(h, _)| h);
        index
    }

    /// Like [`Pool::intern`], but converts [value] to `T` only when it is new
    /// (for `str` -> `Box<str>`, `[X]` -> `Box<[X]>`).
    pub fn intern_ref<Q>(&self, value: &Q) -> u32
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
        for<'q> &'q Q: Into<T>,
    {
        let hash = Self::hash(value);
        let mut table = self.shard(hash).lock();
        if let Some(&(_, index)) = table.find(hash, |&(h, i)| {
            h == hash && self.items[i as usize].borrow() == value
        }) {
            return index;
        }
        let index = u32::try_from(self.items.push(value.into())).expect("pool overflow");
        table.insert_unique(hash, (hash, index), |&(h, _)| h);
        index
    }

    /// The index of [value], when it was interned.
    pub fn lookup<Q>(&self, value: &Q) -> Option<u32>
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let hash = Self::hash(value);
        let table = self.shard(hash).lock();
        table
            .find(hash, |&(h, i)| {
                h == hash && self.items[i as usize].borrow() == value
            })
            .map(|&(_, i)| i)
    }

    /// The item with [index]. Panics when the index was not returned by
    /// this pool.
    #[inline]
    pub fn get(&self, index: u32) -> &T {
        &self.items[index as usize]
    }

    /// The number of items.
    pub fn len(&self) -> usize {
        self.items.count()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
