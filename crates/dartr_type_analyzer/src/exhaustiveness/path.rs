// Dart source: pkg/_fe_analyzer_shared/lib/src/exhaustiveness/path.dart

use std::fmt;
use std::rc::Rc;

use super::key::Key;

/// A path that describes location of a `SingleSpace` from the root of
/// enclosing `Space`.
///
/// The Dart classes `_Root` and `_Step` are `None` and `Some(step)`.
#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct Path(Option<Rc<Step>>);

/// A single step in a path that holds the [parent] pointer and the [key] for
/// the step.
#[derive(PartialEq, Eq, Hash)]
struct Step {
    parent: Path,
    key: Key,
}

impl Path {
    /// Create root path.
    pub const fn root() -> Path {
        Path(None)
    }

    /// Returns a path that adds a step by the [key] to the current path.
    pub fn add(&self, key: Key) -> Path {
        Path(Some(Rc::new(Step {
            parent: self.clone(),
            key,
        })))
    }

    fn to_list_internal(&self, list: &mut Vec<Key>) {
        if let Some(step) = &self.0 {
            step.parent.to_list_internal(list);
            list.push(step.key.clone());
        }
    }

    /// Returns a list of the keys from the root to this path.
    pub fn to_list(&self) -> Vec<Key> {
        let mut list = vec![];
        self.to_list_internal(&mut list);
        list
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            None => f.write_str("@"),
            Some(step) => {
                if step.parent.0.is_none() {
                    f.write_str(&step.key.name())
                } else {
                    write!(f, "{}.{}", step.parent, step.key.name())
                }
            }
        }
    }
}

impl fmt::Debug for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
