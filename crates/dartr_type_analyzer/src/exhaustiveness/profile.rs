// Dart source: pkg/_fe_analyzer_shared/lib/src/exhaustiveness/profile.dart

//! Simple counters for profiling the exhaustiveness algorithm.
//!
//! The Dart library globals are thread-local here.

use std::cell::{Cell, RefCell};

use indexmap::IndexMap;

thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static COUNTS: RefCell<IndexMap<String, usize>> = RefCell::new(IndexMap::new());
}

/// Dart `enabled`.
pub fn enabled() -> bool {
    ENABLED.with(|e| e.get())
}

/// Sets Dart `enabled`.
pub fn set_enabled(value: bool) {
    ENABLED.with(|e| e.set(value));
}

pub fn count(name: &str, subname: Option<&str>) {
    if !enabled() {
        return;
    }
    COUNTS.with(|counts| {
        *counts.borrow_mut().entry(name.to_string()).or_insert(0) += 1;
    });

    if let Some(subname) = subname {
        count(&format!("{name}/{subname}"), None);
    }
}

/// Returns the counts in the format printed by Dart `log()`.
pub fn log_to_string() -> String {
    COUNTS.with(|counts| {
        let counts = counts.borrow();
        let mut names: Vec<&String> = counts.keys().collect();
        names.sort();
        let name_length = names.iter().fold(0, |length, name| length.max(name.len()));
        let count_length = counts
            .values()
            .fold(0, |length, count| length.max(count.to_string().len()));
        let mut result = String::new();
        for name in names {
            result.push_str(&format!(
                "{:<name_length$} = {:>count_length$}\n",
                name, counts[name]
            ));
        }
        result
    })
}

pub fn log() {
    print!("{}", log_to_string());
}

pub fn reset() {
    COUNTS.with(|counts| counts.borrow_mut().clear());
}

pub fn run(callback: impl FnOnce()) {
    reset();
    struct Finally;
    impl Drop for Finally {
        fn drop(&mut self) {
            log();
            reset();
        }
    }
    let _finally = Finally;
    callback();
}
