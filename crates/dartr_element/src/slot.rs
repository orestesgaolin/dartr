//! Storage helpers of the element model: [`Arena<T>`] (the per-kind vectors
//! of an [`crate::ElementStore`]) and [`OnceSlot<T>`] (data that linking
//! computes after the structure is built).

use std::any::Any;
use std::fmt;
use std::sync::OnceLock;

use parking_lot::Mutex;
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

/// An append-only vector with stable references and compact initial buckets
/// (capacities `1, 2, 4, 8, ...`, with bucket 0 stored directly in `b0` so
/// 0-element and 1-element arenas have zero bucket-table overhead).
///
/// - A builder phase that owns the store adds with [`Arena::push`] and
///   changes structural data with [`Arena::get_mut`] (`&mut`).
/// - The synthetic store of a generation and the store of a [`crate::LocalArena`]
///   add through a shared reference (`&self`), while other code reads.
pub struct Arena<T> {
    b0: AtomicPtr<T>,
    more: AtomicPtr<[AtomicPtr<T>; 24]>,
    len: AtomicU32,
    lock: Mutex<()>,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Arena {
            b0: AtomicPtr::new(std::ptr::null_mut()),
            more: AtomicPtr::new(std::ptr::null_mut()),
            len: AtomicU32::new(0),
            lock: Mutex::new(()),
        }
    }
}

impl<T> Arena<T> {
    /// Adds [value] and returns its index.
    #[inline]
    pub fn push(&self, value: T) -> u32 {
        let _guard = self.lock.lock();
        let i = self.len.load(Ordering::Relaxed);
        if i == 0 {
            let ptr = Box::into_raw(Box::new(value));
            self.b0.store(ptr, Ordering::Release);
            self.len.store(1, Ordering::Release);
            return 0;
        }
        let x = i + 1;
        let b = (31 - x.leading_zeros()) as usize;
        let off = (x ^ (1 << b)) as usize;
        assert!(b >= 1 && b <= 24, "arena overflow");
        let mut more = self.more.load(Ordering::Relaxed);
        if more.is_null() {
            let table: Box<[AtomicPtr<T>; 24]> =
                Box::new(std::array::from_fn(
                    |_| AtomicPtr::new(std::ptr::null_mut()),
                ));
            more = Box::into_raw(table);
            self.more.store(more, Ordering::Release);
        }
        let slot_atomic = unsafe { &(&*more)[b - 1] };
        let mut bucket = slot_atomic.load(Ordering::Relaxed);
        if bucket.is_null() {
            let cap = 1usize << b;
            let mut vec: Vec<std::mem::MaybeUninit<T>> = Vec::with_capacity(cap);
            bucket = vec.as_mut_ptr().cast::<T>();
            std::mem::forget(vec);
            slot_atomic.store(bucket, Ordering::Release);
        }
        unsafe {
            bucket.add(off).write(value);
        }
        self.len.store(i + 1, Ordering::Release);
        i
    }

    #[inline]
    pub fn get(&self, index: u32) -> &T {
        let len = self.len.load(Ordering::Acquire);
        assert!(index < len, "arena index {index} out of bounds {len}");
        if index == 0 {
            let ptr = self.b0.load(Ordering::Acquire);
            return unsafe { &*ptr };
        }
        let x = index + 1;
        let b = (31 - x.leading_zeros()) as usize;
        let off = (x ^ (1 << b)) as usize;
        let more = self.more.load(Ordering::Acquire);
        let bucket = unsafe { (&*more)[b - 1].load(Ordering::Acquire) };
        unsafe { &*bucket.add(off) }
    }

    #[inline]
    pub fn get_mut(&mut self, index: u32) -> &mut T {
        let len = *self.len.get_mut();
        assert!(index < len, "arena index {index} out of bounds {len}");
        if index == 0 {
            let ptr = *self.b0.get_mut();
            return unsafe { &mut *ptr };
        }
        let x = index + 1;
        let b = (31 - x.leading_zeros()) as usize;
        let off = (x ^ (1 << b)) as usize;
        let more = *self.more.get_mut();
        let bucket = unsafe { *(&mut *more)[b - 1].get_mut() };
        unsafe { &mut *bucket.add(off) }
    }

    /// The number of items.
    pub fn len(&self) -> usize {
        self.len.load(Ordering::Acquire) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The items with their index, in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)> {
        let len = self.len.load(Ordering::Acquire);
        (0..len).map(move |i| (i, self.get(i)))
    }

    /// Returns `(count, used_element_bytes, allocated_capacity_bytes)`.
    pub fn mem_bytes(&self) -> (usize, usize, usize) {
        let count = self.len.load(Ordering::Relaxed) as usize;
        let elem_size = std::mem::size_of::<T>();
        if count == 0 {
            return (0, 0, 0);
        }
        if count == 1 {
            return (1, elem_size, elem_size);
        }
        let max_b = (31 - (count as u32).leading_zeros()) as usize;
        let cap_slots = (1usize << (max_b + 1)) - 1;
        let table_bytes = 24 * std::mem::size_of::<AtomicPtr<T>>();
        (
            count,
            count * elem_size,
            cap_slots * elem_size + table_bytes,
        )
    }
}

impl<T> Drop for Arena<T> {
    fn drop(&mut self) {
        let len = *self.len.get_mut();
        let b0 = *self.b0.get_mut();
        if !b0.is_null() && len > 0 {
            unsafe {
                drop(Box::from_raw(b0));
            }
        }
        let more = *self.more.get_mut();
        if !more.is_null() {
            let table = unsafe { Box::from_raw(more) };
            for (idx, slot) in table.into_iter().enumerate() {
                let b = idx + 1;
                let bucket = slot.into_inner();
                if !bucket.is_null() {
                    let cap = 1usize << b;
                    let start_index = (1u32 << b) - 1;
                    let init_count = if len > start_index {
                        ((len - start_index) as usize).min(cap)
                    } else {
                        0
                    };
                    unsafe {
                        let slice = std::ptr::slice_from_raw_parts_mut(bucket, init_count);
                        std::ptr::drop_in_place(slice);
                        let _ =
                            Vec::<std::mem::MaybeUninit<T>>::from_raw_parts(bucket.cast(), 0, cap);
                    }
                }
            }
        }
    }
}

// SAFETY: `Arena<T>` owns its `T` elements and synchronizes concurrent pushes
// with `lock` and `Release`/`Acquire` atomic orderings.
unsafe impl<T: Send> Send for Arena<T> {}
unsafe impl<T: Send + Sync> Sync for Arena<T> {}

impl<T: fmt::Debug> fmt::Debug for Arena<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter().map(|(_, v)| v)).finish()
    }
}

/// A value that is set once, after the element was created (a Dart `late`
/// field or a field that a later link phase assigns). It keeps a frozen
/// store `Sync` and lets recursive link-time computations write while they
/// hold shared borrows (design §1.2).
///
/// [`OnceSlot::set_once`] panics on a second set: in the Dart code a second
/// assignment would be a bug, so this finds porting bugs early.
pub struct OnceSlot<T>(OnceLock<T>);

impl<T> Default for OnceSlot<T> {
    fn default() -> Self {
        OnceSlot(OnceLock::new())
    }
}

impl<T> OnceSlot<T> {
    pub const fn new() -> Self {
        OnceSlot(OnceLock::new())
    }

    /// A slot that is already set (data known when the element is created).
    pub fn with(value: T) -> Self {
        let slot = Self::new();
        let _ = slot.0.set(value);
        slot
    }

    /// Sets the value. Panics when it is already set.
    #[track_caller]
    pub fn set_once(&self, value: T) {
        if self.0.set(value).is_err() {
            panic!("OnceSlot set twice");
        }
    }

    /// The value; panics when it is not set yet (a Dart `late` read before
    /// the write).
    #[track_caller]
    pub fn get(&self) -> &T {
        self.0.get().expect("OnceSlot read before it was set")
    }

    /// The value, when it is set.
    pub fn try_get(&self) -> Option<&T> {
        self.0.get()
    }

    pub fn is_set(&self) -> bool {
        self.0.get().is_some()
    }

    /// The value, computed by [f] on the first call (lazy caches). If two
    /// threads race, both compute and one result is dropped; [f] must be
    /// deterministic.
    pub fn get_or_init(&self, f: impl FnOnce() -> T) -> &T {
        self.0.get_or_init(f)
    }

    /// Mutable access while a builder phase owns the store.
    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.0.get_mut()
    }

    /// Replaces the value while a builder phase owns the store (`&mut`), for
    /// the few Dart fields that a link phase writes more than once.
    pub fn replace(&mut self, value: T) {
        self.0 = OnceLock::new();
        let _ = self.0.set(value);
    }
}

impl<T: fmt::Debug> fmt::Debug for OnceSlot<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.get() {
            Some(v) => v.fmt(f),
            None => write!(f, "<unset>"),
        }
    }
}

/// A value that link phases (or, for local elements, the resolver) may
/// assign more than once: a Dart non-final field such as
/// `InterfaceElementImpl._supertype` or `FormalParameterElementImpl._type`.
/// Reads copy the value. Use [`OnceSlot`] for Dart `late final` fields and
/// lazy caches instead.
pub struct VarSlot<T: Copy>(parking_lot::RwLock<Option<T>>);

impl<T: Copy> Default for VarSlot<T> {
    fn default() -> Self {
        VarSlot(parking_lot::RwLock::new(None))
    }
}

impl<T: Copy> VarSlot<T> {
    pub const fn new() -> Self {
        VarSlot(parking_lot::RwLock::new(None))
    }

    /// A slot with an initial value (a Dart field initializer, for example
    /// `TypeImpl _type = InvalidTypeImpl.instance`).
    pub fn with(value: T) -> Self {
        VarSlot(parking_lot::RwLock::new(Some(value)))
    }

    /// The value; `None` when it was never assigned (Dart `null`).
    #[inline]
    pub fn get(&self) -> Option<T> {
        *self.0.read()
    }

    /// The value; panics when it was never assigned.
    #[track_caller]
    pub fn expect(&self) -> T {
        self.get().expect("VarSlot read before it was set")
    }

    /// Assigns the value (Dart `field = value`).
    #[inline]
    pub fn set(&self, value: Option<T>) {
        *self.0.write() = value;
    }
}

impl<T: Copy + fmt::Debug> fmt::Debug for VarSlot<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.get() {
            Some(v) => v.fmt(f),
            None => write!(f, "<null>"),
        }
    }
}

/// A `bool` field that link phases or the resolver assign through a shared
/// reference (Dart non-final `bool` fields).
#[derive(Default)]
pub struct BoolSlot(std::sync::atomic::AtomicBool);

impl BoolSlot {
    pub const fn new(value: bool) -> Self {
        BoolSlot(std::sync::atomic::AtomicBool::new(value))
    }

    #[inline]
    pub fn get(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Relaxed)
    }

    #[inline]
    pub fn set(&self, value: bool) {
        self.0.store(value, std::sync::atomic::Ordering::Relaxed)
    }
}

impl fmt::Debug for BoolSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}

macro_rules! flag_cell {
    ($(#[$m:meta])* $name:ident, $flags:ty, $atomic:ty, $int:ty) => {
        $(#[$m])*
        #[derive(Default)]
        pub struct $name($atomic);

        impl $name {
            pub fn new(flags: $flags) -> Self {
                $name(<$atomic>::new(flags.bits()))
            }

            /// All flags.
            #[inline]
            pub fn get(&self) -> $flags {
                <$flags>::from_bits(self.0.load(std::sync::atomic::Ordering::Relaxed))
            }

            /// Dart `hasFlag(flag)`.
            #[inline]
            pub fn has(&self, flag: $flags) -> bool {
                self.get().contains(flag)
            }

            /// Dart `setFlag(flag, value)`.
            #[inline]
            pub fn set(&self, flag: $flags, value: bool) {
                use std::sync::atomic::Ordering::Relaxed;
                if value {
                    self.0.fetch_or(flag.bits(), Relaxed);
                } else {
                    self.0.fetch_and(!flag.bits(), Relaxed);
                }
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.get().fmt(f)
            }
        }
    };
}

flag_cell!(
    /// The [`crate::ElementFlags`] of an element (Dart `ElementImpl._flags`),
    /// settable through a shared reference.
    ElementFlagCell,
    crate::ElementFlags,
    std::sync::atomic::AtomicU32,
    u32
);
flag_cell!(
    /// The [`crate::FragmentFlags`] of a fragment (Dart `FragmentImpl._flags`).
    FragmentFlagCell,
    crate::FragmentFlags,
    std::sync::atomic::AtomicU64,
    u64
);

/// A lazy cache of a type that this crate cannot name, attached to an
/// element by a higher crate (for example the inheritance data of
/// `dartr_typesystem::inheritance_manager3`, the per-element entry of
/// `InheritanceManager3._interfaces`, design §2.3). It is not a Dart field.
///
/// Each field of this type has one owner module that decides the stored
/// type; [`ElementCache::get_or_init`] panics when a reader asks for another
/// type.
#[derive(Default)]
pub struct ElementCache(OnceLock<Box<dyn Any + Send + Sync>>);

impl ElementCache {
    pub const fn new() -> Self {
        ElementCache(OnceLock::new())
    }

    /// The cached value, created by [f] on the first call. If two threads
    /// race, both run [f] and one result is dropped; [f] must not depend on
    /// the race.
    pub fn get_or_init<T: Any + Send + Sync>(&self, f: impl FnOnce() -> T) -> &T {
        self.0
            .get_or_init(|| Box::new(f()))
            .downcast_ref::<T>()
            .expect("ElementCache read with another type")
    }

    /// The cached value, when it was created.
    pub fn try_get<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.0.get().map(|b| {
            b.downcast_ref::<T>()
                .expect("ElementCache read with another type")
        })
    }
}

impl fmt::Debug for ElementCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.get().is_some() {
            write!(f, "ElementCache(set)")
        } else {
            write!(f, "ElementCache(empty)")
        }
    }
}
