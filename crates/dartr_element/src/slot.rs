//! Storage helpers of the element model: [`Arena<T>`] (the per-kind vectors
//! of an [`crate::ElementStore`]) and [`OnceSlot<T>`] (data that linking
//! computes after the structure is built).

use std::fmt;
use std::sync::OnceLock;

/// An append-only vector with stable references (a `boxcar::Vec`).
///
/// - A builder phase that owns the store adds with [`Arena::push`] and
///   changes structural data with [`Arena::get_mut`] (`&mut`).
/// - The synthetic store of a generation and the store of a [`crate::LocalArena`]
///   add through a shared reference (`&self`), while other code reads.
pub struct Arena<T> {
    items: boxcar::Vec<T>,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Arena {
            items: boxcar::Vec::new(),
        }
    }
}

impl<T> Arena<T> {
    /// Adds [value] and returns its index.
    #[inline]
    pub fn push(&self, value: T) -> u32 {
        u32::try_from(self.items.push(value)).expect("arena overflow")
    }

    #[inline]
    pub fn get(&self, index: u32) -> &T {
        &self.items[index as usize]
    }

    #[inline]
    pub fn get_mut(&mut self, index: u32) -> &mut T {
        self.items.get_mut(index as usize).expect("arena index")
    }

    /// The number of items.
    pub fn len(&self) -> usize {
        self.items.count()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The items with their index, in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)> {
        self.items.iter().map(|(i, v)| (i as u32, v))
    }
}

impl<T: fmt::Debug> fmt::Debug for Arena<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.items.iter().map(|(_, v)| v))
            .finish()
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
