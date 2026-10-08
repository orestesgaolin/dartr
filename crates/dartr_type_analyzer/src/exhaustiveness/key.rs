// Dart source: pkg/_fe_analyzer_shared/lib/src/exhaustiveness/key.dart

//! Keys of properties and additional properties (map entries, list elements).
//!
//! The Dart class hierarchy `Key` / `MapKey` / `ListKey` (`HeadKey`,
//! `TailKey`, `RestKey`) / `NameKey` / `RecordKey` (`RecordIndexKey`,
//! `RecordNameKey`) / `ExtensionKey` is the enum [`Key`]. Equality and hashing
//! follow the Dart `==` / `hashCode` overrides. In particular `RecordIndexKey`
//! and `RecordNameKey` extend `NameKey` in Dart, so a `NameKey('x')` is equal
//! to a `RecordNameKey('x')`.

use std::any::Any;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use super::static_type::{StaticType, StaticTypeArena};

/// A value with Dart `==` / `hashCode` semantics, stored type-erased.
///
/// This models the Dart `Object` values used as identities: the constant
/// value of a [MapKey], the unique value of `IdentityRestriction`, and the
/// enum elements. Two values are equal only if they have the same Rust type
/// and are equal by that type's [`Eq`].
pub trait IdentityValue: Any + fmt::Debug {
    /// Dart `==`.
    fn eq_dyn(&self, other: &dyn IdentityValue) -> bool;

    /// Dart `hashCode`.
    fn hash_dyn(&self, state: &mut dyn Hasher);

    /// Returns `self` as [`Any`], so a client can downcast it.
    fn as_any(&self) -> &dyn Any;
}

impl<T: Any + fmt::Debug + Eq + Hash> IdentityValue for T {
    fn eq_dyn(&self, other: &dyn IdentityValue) -> bool {
        other
            .as_any()
            .downcast_ref::<T>()
            .is_some_and(|other| self == other)
    }

    fn hash_dyn(&self, mut state: &mut dyn Hasher) {
        self.hash(&mut state);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A shared, type-erased Dart `Object` value with `==` / `hashCode`
/// semantics. See [`IdentityValue`].
#[derive(Clone)]
pub struct Identity(Rc<dyn IdentityValue>);

impl Identity {
    /// Wraps [value].
    pub fn new<T: Any + fmt::Debug + Eq + Hash>(value: T) -> Identity {
        Identity(Rc::new(value))
    }

    /// Returns the wrapped value as [`Any`], so a client can downcast it.
    pub fn as_any(&self) -> &dyn Any {
        (*self.0).as_any()
    }

    /// Returns the wrapped value if it has type `T`.
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }
}

impl PartialEq for Identity {
    fn eq(&self, other: &Identity) -> bool {
        Rc::ptr_eq(&self.0, &other.0) || (*self.0).eq_dyn(&*other.0)
    }
}

impl Eq for Identity {}

impl Hash for Identity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (*self.0).hash_dyn(state);
    }
}

impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (*self.0).fmt(f)
    }
}

/// Compares two strings like Dart `String.compareTo`, that is, by UTF-16 code
/// units.
pub(crate) fn compare_strings(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// A value that defines the key of an additional field.
///
/// This is used for accessing entries in maps and elements in lists.
///
/// The variants are the Dart subclasses of `Key`.
#[derive(Clone, Debug)]
pub enum Key {
    /// An entry in a map whose key is a constant value.
    Map(MapKey),

    /// An element in a list accessed by an `index` from the start of the list.
    /// (Dart `HeadKey`.)
    Head(usize),

    /// An element in a list accessed by an `index` from the end of the list,
    /// that is, the `index`th last element. (Dart `TailKey`.)
    Tail(usize),

    /// A sublist of a list from the `head_size`th index to the `tail_size`th
    /// last index. (Dart `RestKey`.)
    Rest { head_size: usize, tail_size: usize },

    /// Key for a regular object member. (Dart `NameKey`.)
    Name(String),

    /// Specialized [Key::Name] for an indexed record field. (Dart
    /// `RecordIndexKey`.)
    RecordIndex(usize),

    /// Specialized [Key::Name] for a named record field. (Dart
    /// `RecordNameKey`.)
    RecordName(String),

    /// Key for an extension member. (Dart `ExtensionKey`.)
    Extension(ExtensionKey),
}

/// An entry in a map whose key is a constant [value].
#[derive(Clone, Debug)]
pub struct MapKey {
    pub value: Identity,
    pub value_as_text: String,
}

impl MapKey {
    pub fn new(value: Identity, value_as_text: impl Into<String>) -> MapKey {
        MapKey {
            value,
            value_as_text: value_as_text.into(),
        }
    }

    pub fn name(&self) -> String {
        format!("[{}]", self.value_as_text)
    }
}

impl PartialEq for MapKey {
    fn eq(&self, other: &MapKey) -> bool {
        self.value == other.value
    }
}

impl Eq for MapKey {}

impl Hash for MapKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl fmt::Display for MapKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value_as_text)
    }
}

/// Key for an extension member.
#[derive(Clone, Debug)]
pub struct ExtensionKey {
    pub receiver_type: StaticType,
    pub name: String,
    pub type_: StaticType,
}

impl ExtensionKey {
    pub fn new(receiver_type: StaticType, name: impl Into<String>, type_: StaticType) -> Self {
        ExtensionKey {
            receiver_type,
            name: name.into(),
            type_,
        }
    }
}

impl PartialEq for ExtensionKey {
    fn eq(&self, other: &ExtensionKey) -> bool {
        self.receiver_type == other.receiver_type && self.name == other.name
    }
}

impl Eq for ExtensionKey {}

impl Hash for ExtensionKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.receiver_type.hash(state);
        self.name.hash(state);
    }
}

impl Key {
    /// Creates a `NameKey`.
    pub fn name_key(name: impl Into<String>) -> Key {
        Key::Name(name.into())
    }

    /// Creates a `RecordNameKey`.
    pub fn record_name_key(name: impl Into<String>) -> Key {
        Key::RecordName(name.into())
    }

    pub fn name(&self) -> String {
        match self {
            Key::Map(key) => key.name(),
            Key::Head(index) => format!("[{index}]"),
            Key::Tail(index) => format!("[{}]", -(*index as i64 + 1)),
            Key::Rest {
                head_size,
                tail_size,
            } => format!("[{head_size}:{}]", -(*tail_size as i64)),
            Key::Name(name) | Key::RecordName(name) => name.clone(),
            Key::RecordIndex(index) => format!("${}", index + 1),
            Key::Extension(key) => key.name.clone(),
        }
    }

    /// Dart `is NameKey` (includes `RecordIndexKey` and `RecordNameKey`).
    pub fn is_name_key(&self) -> bool {
        matches!(
            self,
            Key::Name(_) | Key::RecordIndex(_) | Key::RecordName(_)
        )
    }

    /// Dart `is ListKey`.
    pub fn is_list_key(&self) -> bool {
        matches!(self, Key::Head(_) | Key::Tail(_) | Key::Rest { .. })
    }

    /// Dart `is RecordKey`.
    pub fn is_record_key(&self) -> bool {
        matches!(self, Key::RecordIndex(_) | Key::RecordName(_))
    }

    /// Dart `is MapKey`.
    pub fn is_map_key(&self) -> bool {
        matches!(self, Key::Map(_))
    }

    /// Dart `Key.compareTo`.
    ///
    /// [types] is needed because `ExtensionKey` compares the names of the
    /// receiver types.
    pub fn compare_to(&self, other: &Key, types: &dyn StaticTypeArena) -> Ordering {
        match self {
            Key::Map(key) => match other {
                Key::Map(other) => compare_strings(&key.value_as_text, &other.value_as_text),
                // Map keys after list keys.
                Key::Head(_) | Key::Rest { .. } | Key::Tail(_) => Ordering::Greater,
                // Map keys before record index, name and extension keys,
                _ => Ordering::Less,
            },
            Key::Head(index) => match other {
                Key::Head(other_index) => index.cmp(other_index),
                // Head keys before other keys,
                _ => Ordering::Less,
            },
            Key::Tail(index) => match other {
                Key::Tail(other_index) => index.cmp(other_index).reverse(),
                // Tail keys after head and rest keys.
                Key::Head(_) | Key::Rest { .. } => Ordering::Greater,
                // Tail keys before map, record index, name and extension keys,
                _ => Ordering::Less,
            },
            Key::Rest {
                head_size,
                tail_size,
            } => match other {
                Key::Rest {
                    head_size: other_head_size,
                    tail_size: other_tail_size,
                } => {
                    let mut result = head_size.cmp(other_head_size);
                    if result == Ordering::Equal {
                        result = tail_size.cmp(other_tail_size).reverse();
                    }
                    result
                }
                // Rest keys after head keys.
                Key::Head(_) => Ordering::Greater,
                // Rest keys before tail, map, record index, name and extension
                // keys,
                _ => Ordering::Less,
            },
            Key::Name(name) | Key::RecordName(name) => match other {
                // Name keys after record index keys.
                Key::RecordIndex(_) => Ordering::Greater,
                Key::Name(_) | Key::RecordName(_) => compare_strings(name, &other.name()),
                // Name keys before extension keys.
                Key::Extension(_) => Ordering::Less,
                // Name keys after other keys.
                _ => Ordering::Greater,
            },
            Key::RecordIndex(index) => match other {
                Key::RecordIndex(other_index) => index.cmp(other_index),
                // Record index keys before name keys.
                Key::Name(_) | Key::RecordName(_) => Ordering::Less,
                // Record index keys before extension keys.
                Key::Extension(_) => Ordering::Less,
                // Record index keys after other keys.
                _ => Ordering::Greater,
            },
            Key::Extension(key) => match other {
                Key::Extension(other) => {
                    // Sorting is only used for a stable choice of witness, so
                    // it's ok that in edge cases `receiverType.name` is not
                    // unique.
                    let mut result = compare_strings(
                        &types.name(key.receiver_type),
                        &types.name(other.receiver_type),
                    );
                    if result == Ordering::Equal {
                        result = compare_strings(&key.name, &other.name);
                    }
                    result
                }
                // Extension keys after other keys.
                _ => Ordering::Greater,
            },
        }
    }

    /// Dart `Key.toString`.
    pub fn to_text(&self, types: &dyn StaticTypeArena) -> String {
        match self {
            Key::Map(key) => key.value_as_text.clone(),
            Key::Head(index) => format!("HeadKey({index})"),
            Key::Tail(index) => format!("TailKey({index})"),
            Key::Rest {
                head_size,
                tail_size,
            } => format!("RestKey({head_size},{tail_size})"),
            Key::Name(_) | Key::RecordIndex(_) | Key::RecordName(_) => {
                format!("NameKey({})", self.name())
            }
            Key::Extension(key) => format!(
                "ExtensionKey({}.{}:{})",
                types.name(key.receiver_type),
                key.name,
                types.name(key.type_)
            ),
        }
    }
}

impl PartialEq for Key {
    fn eq(&self, other: &Key) -> bool {
        match (self, other) {
            (Key::Map(a), Key::Map(b)) => a == b,
            (Key::Head(a), Key::Head(b)) => a == b,
            (Key::Tail(a), Key::Tail(b)) => a == b,
            (
                Key::Rest {
                    head_size: a_head,
                    tail_size: a_tail,
                },
                Key::Rest {
                    head_size: b_head,
                    tail_size: b_tail,
                },
            ) => a_head == b_head && a_tail == b_tail,
            (Key::Extension(a), Key::Extension(b)) => a == b,
            // `NameKey.==` is `other is NameKey && name == other.name`, and is
            // inherited by `RecordIndexKey` and `RecordNameKey`.
            _ if self.is_name_key() && other.is_name_key() => self.name() == other.name(),
            _ => false,
        }
    }
}

impl Eq for Key {}

impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Key::Map(key) => key.hash(state),
            Key::Head(index) | Key::Tail(index) => index.hash(state),
            Key::Rest {
                head_size,
                tail_size,
            } => {
                head_size.hash(state);
                tail_size.hash(state);
            }
            Key::Name(_) | Key::RecordIndex(_) | Key::RecordName(_) => self.name().hash(state),
            Key::Extension(key) => key.hash(state),
        }
    }
}
