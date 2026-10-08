// Dart source: none (Rust helpers for translated Dart tests).

//! Helpers that replace Dart language features used by the tests.
//!
//! - [`Late`]: a Dart `late` local variable that closures (DSL callbacks)
//!   assign and read. It is a `Copy` handle, so `move` closures can capture
//!   it without cloning.
//! - [`asserts_enabled`]: Dart `_asserts` (whether `assert` is checked).

use std::any::Any;
use std::cell::RefCell;
use std::marker::PhantomData;

thread_local! {
    static LATE_SLOTS: RefCell<Vec<Option<Box<dyn Any>>>> = const { RefCell::new(Vec::new()) };
}

/// A Dart `late` (or mutable captured) local variable: `late SsaNode s;` is
/// `let s = Late::<SsaNode<MiniAstTypes>>::new();`, `s = v` is `s.set(v)`,
/// reading `s` is `s.get()` (panics if unset, like Dart's
/// `LateInitializationError`). The value lives in a thread-local slot (cargo
/// runs each test on its own thread).
pub struct Late<T: 'static> {
    slot: usize,
    _marker: PhantomData<fn() -> T>,
}

impl<T: 'static> Clone for Late<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> Copy for Late<T> {}

impl<T: Clone + 'static> Default for Late<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + 'static> Late<T> {
    /// An unassigned variable.
    pub fn new() -> Self {
        let slot = LATE_SLOTS.with(|s| {
            let mut s = s.borrow_mut();
            s.push(None);
            s.len() - 1
        });
        Late {
            slot,
            _marker: PhantomData,
        }
    }

    /// A variable with an initial value.
    pub fn with(value: T) -> Self {
        let l = Self::new();
        l.set(value);
        l
    }

    /// Assigns the variable.
    pub fn set(self, value: T) {
        LATE_SLOTS.with(|s| s.borrow_mut()[self.slot] = Some(Box::new(value)));
    }

    /// Reads the variable; panics if it was never assigned.
    pub fn get(self) -> T {
        self.try_get()
            .expect("LateInitializationError: late variable read before assignment")
    }

    /// Reads the variable, or `None` if it was never assigned.
    pub fn try_get(self) -> Option<T> {
        LATE_SLOTS.with(|s| {
            s.borrow()[self.slot]
                .as_ref()
                .map(|b| b.downcast_ref::<T>().expect("Late slot type").clone())
        })
    }
}

/// Dart `_asserts`: whether `assert` statements are checked (here: Rust
/// debug assertions, which flow analysis uses for the Dart asserts).
pub fn asserts_enabled() -> bool {
    cfg!(debug_assertions)
}

/// Dart `expect(() => f(), _asserts)`: when assertions are enabled, `f`
/// must panic; otherwise it must not.
pub fn expect_asserts(f: impl FnOnce()) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    if asserts_enabled() {
        assert!(result.is_err(), "expected an assertion failure");
    } else {
        assert!(result.is_ok(), "expected no failure");
    }
}
