// Dart source: none (the Dart type algorithms recurse on the VM stack and
// fail with a `StackOverflowError` on some recursive bounds, for example
// `X extends FutureOr<X>`).

//! [`enter`]: a recursion depth guard for the recursive type algorithms
//! (`isSubtypeOf`, `isNonNullable`, `isNullable`, `isObject`, `UP`). Where the
//! Dart code would overflow the stack, the port panics at a fixed depth.
//! The library analyzer catches the panic per unit, the same as the
//! analyzer fails the analysis of that library. Without the guard the
//! native recursion runs for a long time before it aborts the process.

use std::cell::Cell;

/// The total depth of guarded calls at which [`enter`] panics.
pub const MAX_DEPTH: u32 = 2000;

thread_local! {
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Decrements the depth when it goes out of scope (also during a panic).
pub struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    }
}

/// Enters one guarded call of [what]; panics with a message that starts
/// with `StackOverflowError` when the guarded calls are nested more than
/// [`MAX_DEPTH`] deep.
pub fn enter(what: &str) -> Guard {
    let depth = DEPTH.with(|d| {
        let v = d.get() + 1;
        d.set(v);
        v
    });
    let guard = Guard;
    if depth > MAX_DEPTH {
        panic!("StackOverflowError: {what} recursion deeper than {MAX_DEPTH}");
    }
    guard
}
