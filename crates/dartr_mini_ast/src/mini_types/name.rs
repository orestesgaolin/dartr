// Dart source: pkg/_fe_analyzer_shared/test/mini_types.dart

//! [`Name`]: an interned string (no Dart counterpart; Dart uses `String`).

use std::collections::HashSet;
use std::fmt;
use std::sync::{LazyLock, Mutex};

/// An interned string, used for type names, type parameter names, record
/// field names and named parameter names (Dart `String`).
///
/// `Copy + Eq + Hash`, as required for `SharedTypeOperations::Name`. Equality
/// and ordering are those of the string contents, so the order is the order
/// of Dart `String.compareTo` for ASCII names.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Name(&'static str);

/// The global set of interned strings. The strings are leaked, which is
/// acceptable for test support code.
static NAMES: LazyLock<Mutex<HashSet<&'static str>>> = LazyLock::new(Default::default);

impl Name {
    /// Interns [s].
    pub fn new(s: &str) -> Name {
        let mut names = NAMES.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(&existing) = names.get(s) {
            return Name(existing);
        }
        let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
        names.insert(leaked);
        Name(leaked)
    }

    /// The string contents.
    pub fn as_str(self) -> &'static str {
        self.0
    }
}

impl From<&str> for Name {
    fn from(s: &str) -> Name {
        Name::new(s)
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl PartialEq<str> for Name {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for Name {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}
