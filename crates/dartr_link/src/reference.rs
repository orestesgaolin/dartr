// Dart source: pkg/analyzer/lib/src/summary2/reference.dart

//! Symbolic references to the declarations of a library (Dart `Reference`):
//! `<library uri>::@class::A::@method::foo`. Each library has a
//! [`LibraryReference`] that owns its references in a vector; a
//! [`RefId`] indexes it.
//!
//! The linker declares a reference for every element it builds (the keys of
//! unnamed and duplicate declarations come from [`LibraryReferenceBuilder`]).
//! A later summary format (design §2.4 v2) writes references symbolically
//! and binds them to elements on load.
//!
//! Difference: the Dart `RootReference` (all libraries of a session) is not
//! ported; a reference to another library is its URI plus a [`RefId`] in
//! that library's table.

use std::sync::Arc;

use dartr_element::ElementId;
use indexmap::IndexMap;

/// Index of a reference in a [`LibraryReference`]; 0 is the library itself.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RefId(pub u32);

impl RefId {
    pub const LIBRARY: RefId = RefId(0);
}

/// Dart `_TopLevelReferenceKind`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TopLevelReferenceKind {
    Class,
    Enum,
    Extension,
    ExtensionType,
    Mixin,
    TypeAlias,
    Function,
    Getter,
    Setter,
    TopLevelVariable,
}

impl TopLevelReferenceKind {
    pub const COUNT: usize = 10;

    pub fn debug_name(self) -> &'static str {
        match self {
            TopLevelReferenceKind::Class => "@class",
            TopLevelReferenceKind::Enum => "@enum",
            TopLevelReferenceKind::Extension => "@extension",
            TopLevelReferenceKind::ExtensionType => "@extensionType",
            TopLevelReferenceKind::Mixin => "@mixin",
            TopLevelReferenceKind::TypeAlias => "@typeAlias",
            TopLevelReferenceKind::Function => "@function",
            TopLevelReferenceKind::Getter => "@getter",
            TopLevelReferenceKind::Setter => "@setter",
            TopLevelReferenceKind::TopLevelVariable => "@topLevelVariable",
        }
    }

    pub fn has_members(self) -> bool {
        matches!(
            self,
            TopLevelReferenceKind::Class
                | TopLevelReferenceKind::Enum
                | TopLevelReferenceKind::Extension
                | TopLevelReferenceKind::ExtensionType
                | TopLevelReferenceKind::Mixin
        )
    }
}

/// Dart `_MemberReferenceKind`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MemberReferenceKind {
    Constructor,
    Field,
    Getter,
    Setter,
    Method,
}

impl MemberReferenceKind {
    pub const COUNT: usize = 5;

    pub fn debug_name(self) -> &'static str {
        match self {
            MemberReferenceKind::Constructor => "@constructor",
            MemberReferenceKind::Field => "@field",
            MemberReferenceKind::Getter => "@getter",
            MemberReferenceKind::Setter => "@setter",
            MemberReferenceKind::Method => "@method",
        }
    }
}

/// Dart `_BuiltInReferenceKind`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BuiltInReferenceKind {
    Dynamic,
    Never,
}

impl BuiltInReferenceKind {
    pub fn identifier(self) -> &'static str {
        match self {
            BuiltInReferenceKind::Dynamic => "dynamic",
            BuiltInReferenceKind::Never => "Never",
        }
    }
}

/// The kind of one reference (the Dart `Reference` subclasses).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReferenceKind {
    /// `LibraryReference`.
    Library,
    /// `TopLevelReference` (`MemberContainerReference` when the kind has
    /// members).
    TopLevel {
        kind: TopLevelReferenceKind,
        key: Arc<str>,
    },
    /// `MemberReference`.
    Member {
        container: RefId,
        kind: MemberReferenceKind,
        key: Arc<str>,
    },
    /// `BuiltInReference`.
    BuiltIn(BuiltInReferenceKind),
}

/// One reference.
#[derive(Clone, Debug)]
pub struct Reference {
    pub kind: ReferenceKind,
    /// Dart `Reference.element`.
    pub element: Option<ElementId>,
    /// The members of a member container, per kind and key.
    members: Vec<IndexMap<Arc<str>, RefId>>,
}

/// Dart `LibraryReference`: the references of one library.
#[derive(Clone, Debug)]
pub struct LibraryReference {
    pub uri_string: Arc<str>,
    references: Vec<Reference>,
    top_levels_by_kind: Vec<IndexMap<Arc<str>, RefId>>,
    built_ins: [Option<RefId>; 2],
}

impl LibraryReference {
    pub fn new(uri_string: Arc<str>) -> LibraryReference {
        LibraryReference {
            uri_string,
            references: vec![Reference {
                kind: ReferenceKind::Library,
                element: None,
                members: Vec::new(),
            }],
            top_levels_by_kind: vec![IndexMap::new(); TopLevelReferenceKind::COUNT],
            built_ins: [None; 2],
        }
    }

    pub fn get(&self, id: RefId) -> &Reference {
        &self.references[id.0 as usize]
    }

    pub fn set_element(&mut self, id: RefId, element: ElementId) {
        self.references[id.0 as usize].element = Some(element);
    }

    fn push(&mut self, kind: ReferenceKind) -> RefId {
        let id = RefId(self.references.len() as u32);
        let members = match &kind {
            ReferenceKind::TopLevel { kind, .. } if kind.has_members() => {
                vec![IndexMap::new(); MemberReferenceKind::COUNT]
            }
            _ => Vec::new(),
        };
        self.references.push(Reference {
            kind,
            element: None,
            members,
        });
        id
    }

    /// Dart `_declareTopLevel` / `_declareMemberContainer`: panics on a
    /// duplicate key (Dart `StateError`).
    pub fn declare_top_level(&mut self, kind: TopLevelReferenceKind, key: &str) -> RefId {
        if self.top_levels_by_kind[kind as usize].contains_key(key) {
            panic!("Duplicate reference key: {key}");
        }
        self.get_or_create_top_level(kind, key)
    }

    /// Dart `_getOrCreateTopLevel`.
    pub fn get_or_create_top_level(&mut self, kind: TopLevelReferenceKind, key: &str) -> RefId {
        if let Some(&id) = self.top_levels_by_kind[kind as usize].get(key) {
            return id;
        }
        let key: Arc<str> = key.into();
        let id = self.push(ReferenceKind::TopLevel {
            kind,
            key: key.clone(),
        });
        self.top_levels_by_kind[kind as usize].insert(key, id);
        id
    }

    /// Dart `MemberContainerReference._declareMember`.
    pub fn declare_member(
        &mut self,
        container: RefId,
        kind: MemberReferenceKind,
        key: &str,
    ) -> RefId {
        if self.references[container.0 as usize].members[kind as usize].contains_key(key) {
            panic!("Duplicate reference key: {key}");
        }
        self.get_or_create_member(container, kind, key)
    }

    /// Dart `MemberContainerReference._getOrCreateMember`.
    pub fn get_or_create_member(
        &mut self,
        container: RefId,
        kind: MemberReferenceKind,
        key: &str,
    ) -> RefId {
        if let Some(&id) = self.references[container.0 as usize].members[kind as usize].get(key) {
            return id;
        }
        let key: Arc<str> = key.into();
        let id = self.push(ReferenceKind::Member {
            container,
            kind,
            key: key.clone(),
        });
        self.references[container.0 as usize].members[kind as usize].insert(key, id);
        id
    }

    /// Dart `dynamicRef` / `neverRef` (`dart:core` only).
    pub fn built_in(&mut self, kind: BuiltInReferenceKind) -> RefId {
        if let Some(id) = self.built_ins[kind as usize] {
            return id;
        }
        let id = self.push(ReferenceKind::BuiltIn(kind));
        self.built_ins[kind as usize] = Some(id);
        id
    }

    /// Dart `Reference.debugString`.
    pub fn debug_string(&self, id: RefId) -> String {
        match &self.get(id).kind {
            ReferenceKind::Library => self.uri_string.to_string(),
            ReferenceKind::TopLevel { kind, key } => {
                format!("{}::{}::{key}", self.uri_string, kind.debug_name())
            }
            ReferenceKind::Member {
                container,
                kind,
                key,
            } => format!(
                "{}::{}::{key}",
                self.debug_string(*container),
                kind.debug_name()
            ),
            ReferenceKind::BuiltIn(kind) => format!("{}::{}", self.uri_string, kind.identifier()),
        }
    }
}

/// Dart `_KeyAllocator`.
#[derive(Clone, Debug)]
struct KeyAllocator {
    next_duplicate_index_by_kind: Vec<IndexMap<String, u32>>,
    next_unnamed_index: u32,
}

impl KeyAllocator {
    fn new(kind_count: usize) -> KeyAllocator {
        KeyAllocator {
            next_duplicate_index_by_kind: vec![IndexMap::new(); kind_count],
            next_unnamed_index: 0,
        }
    }

    fn allocate(&mut self, kind_index: usize, name: Option<&str>) -> String {
        let base_name = match name {
            Some(n) => n.to_string(),
            None => {
                let n = format!("#{}", self.next_unnamed_index);
                self.next_unnamed_index += 1;
                n
            }
        };
        let by_base_name = &mut self.next_duplicate_index_by_kind[kind_index];
        let duplicate_index = by_base_name.get(&base_name).copied().unwrap_or(0);
        by_base_name.insert(base_name.clone(), duplicate_index + 1);
        if duplicate_index == 0 {
            base_name
        } else {
            format!("{base_name}#{duplicate_index}")
        }
    }
}

/// Dart `LibraryReferenceBuilder`: declares references with unique keys.
#[derive(Clone, Debug)]
pub struct LibraryReferenceBuilder {
    pub reference: LibraryReference,
    top_level_keys: KeyAllocator,
    member_keys: IndexMap<RefId, KeyAllocator>,
}

impl LibraryReferenceBuilder {
    pub fn new(reference: LibraryReference) -> LibraryReferenceBuilder {
        LibraryReferenceBuilder {
            reference,
            top_level_keys: KeyAllocator::new(TopLevelReferenceKind::COUNT),
            member_keys: IndexMap::new(),
        }
    }

    /// Dart `declareClass`, `declareEnum`, ..., `declareTypeAlias`.
    pub fn declare_top_level(&mut self, kind: TopLevelReferenceKind, name: Option<&str>) -> RefId {
        let key = self.top_level_keys.allocate(kind as usize, name);
        self.reference.declare_top_level(kind, &key)
    }

    /// Dart `declareMemberConstructor`, `declareMemberField`, ...
    pub fn declare_member(
        &mut self,
        container: RefId,
        kind: MemberReferenceKind,
        name: Option<&str>,
    ) -> RefId {
        let key = self
            .member_keys
            .entry(container)
            .or_insert_with(|| KeyAllocator::new(MemberReferenceKind::COUNT))
            .allocate(kind as usize, name);
        self.reference.declare_member(container, kind, &key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_of_unnamed_and_duplicate_declarations() {
        let mut b = LibraryReferenceBuilder::new(LibraryReference::new("package:a/a.dart".into()));
        let a = b.declare_top_level(TopLevelReferenceKind::Class, Some("A"));
        let a2 = b.declare_top_level(TopLevelReferenceKind::Class, Some("A"));
        let e = b.declare_top_level(TopLevelReferenceKind::Extension, None);
        let m = b.declare_member(a, MemberReferenceKind::Method, Some("m"));
        let r = &b.reference;
        assert_eq!(r.debug_string(a), "package:a/a.dart::@class::A");
        assert_eq!(r.debug_string(a2), "package:a/a.dart::@class::A#1");
        assert_eq!(r.debug_string(e), "package:a/a.dart::@extension::#0");
        assert_eq!(r.debug_string(m), "package:a/a.dart::@class::A::@method::m");
    }
}
