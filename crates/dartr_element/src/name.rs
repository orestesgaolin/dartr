//! Interned names ([`Name`]) and the global name pool ([`NamePool`]).
//!
//! Every element and fragment name, every `Name` in a type (named
//! parameters, record fields) is a [`Name`]: a 4-byte id into the
//! [`NamePool`] of the [`crate::Generation`]. Names that the analyzer refers
//! to by constant (`new`, `call`, `==`, ...) are interned first, in a fixed
//! order, so that they are associated constants ([`Name::NEW`], ...).
//!
//! `Name` has no `Ord`: never sort by id. Sort by the text
//! ([`NamePool::get`]) when the Dart code sorts by name.

use std::fmt;
use std::num::NonZeroU32;

use crate::pool::{Pool, SHARDS};

/// An interned name (an identifier, an operator, a private name with its
/// leading `_`, or a synthetic name such as `new`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Name(NonZeroU32);

impl Name {
    #[inline(always)]
    pub fn index(self) -> u32 {
        self.0.get() - 1
    }

    #[inline(always)]
    const fn from_index(index: u32) -> Name {
        match NonZeroU32::new(index + 1) {
            Some(v) => Name(v),
            None => panic!("name index overflow"),
        }
    }

    /// The raw value. Only for canonical forms; never for output.
    #[inline(always)]
    pub fn raw(self) -> u32 {
        self.0.get()
    }
}

/// The empty name (Dart `''`; the shared type analyzer uses it for error
/// recovery).
impl Default for Name {
    fn default() -> Name {
        Name::EMPTY
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match WELL_KNOWN.get(self.index() as usize) {
            Some(s) => write!(f, "Name({s:?})"),
            None => write!(f, "Name#{}", self.index()),
        }
    }
}

macro_rules! well_known {
    ($($(#[$m:meta])* $name:ident = $text:literal,)*) => {
        const WELL_KNOWN: &[&str] = &[$($text,)*];

        mod well_known_index {
            #[allow(non_camel_case_types, clippy::upper_case_acronyms)]
            #[repr(u32)]
            pub enum Index { $($name,)* }
        }

        impl Name {
            $(
                $(#[$m])*
                pub const $name: Name = Name::from_index(well_known_index::Index::$name as u32);
            )*
        }
    };
}

well_known! {
    /// The empty name.
    EMPTY = "",
    /// The name of unnamed constructors (`ConstructorElement.name`).
    NEW = "new",
    CALL = "call",
    VALUES = "values",
    INDEX = "index",
    HASH_CODE = "hashCode",
    TO_STRING = "toString",
    NO_SUCH_METHOD = "noSuchMethod",
    RUNTIME_TYPE = "runtimeType",
    LOAD_LIBRARY = "loadLibrary",
    UNDERSCORE = "_",
    OBJECT = "Object",
    DYNAMIC = "dynamic",
    NEVER = "Never",
    FUNCTION = "Function",
    NULL = "Null",
    EQ = "==",
    PLUS = "+",
    MINUS = "-",
    STAR = "*",
    SLASH = "/",
    TILDE_SLASH = "~/",
    PERCENT = "%",
    LT = "<",
    GT = ">",
    LT_EQ = "<=",
    GT_EQ = ">=",
    INDEX_GET = "[]",
    INDEX_SET = "[]=",
    TILDE = "~",
    UNARY_MINUS = "unary-",
    AMP = "&",
    BAR = "|",
    CARET = "^",
    LT_LT = "<<",
    GT_GT = ">>",
    GT_GT_GT = ">>>",
}

/// The global name pool of a [`crate::Generation`].
pub struct NamePool {
    pool: Pool<Box<str>>,
}

impl Default for NamePool {
    fn default() -> Self {
        Self::new()
    }
}

impl NamePool {
    pub fn new() -> NamePool {
        let pool = Pool::new(SHARDS);
        for (i, s) in WELL_KNOWN.iter().enumerate() {
            let index = pool.intern_ref::<str>(s);
            debug_assert_eq!(index as usize, i);
        }
        NamePool { pool }
    }

    /// Interns [text].
    pub fn intern(&self, text: &str) -> Name {
        Name::from_index(self.pool.intern_ref::<str>(text))
    }

    /// The name with [text], when it was interned.
    pub fn lookup(&self, text: &str) -> Option<Name> {
        self.pool.lookup::<str>(text).map(Name::from_index)
    }

    /// The text of [name].
    #[inline]
    pub fn get(&self, name: Name) -> &str {
        self.pool.get(name.index())
    }

    pub fn len(&self) -> usize {
        self.pool.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pool.is_empty()
    }
}
