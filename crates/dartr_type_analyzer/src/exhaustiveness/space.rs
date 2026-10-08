// Dart source: pkg/_fe_analyzer_shared/lib/src/exhaustiveness/space.dart

use std::hash::{Hash, Hasher};
use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};

use super::key::Key;
use super::path::Path;
use super::static_type::{StaticType, StaticTypeArena};

/// The main pattern for matching types and destructuring.
///
/// It has a type which determines the type of values it contains. The type may
/// be [StaticType::NULLABLE_OBJECT] to indicate that it doesn't filter by type.
///
/// It may also contain zero or more named properties. The pattern then only
/// matches values where the property values are matched by the corresponding
/// property patterns.
///
/// Equality and hashing follow Dart: the type and the properties are compared,
/// the additional properties are not.
#[derive(Clone, Debug)]
pub struct SingleSpace {
    /// The type of values the pattern matches.
    pub type_: StaticType,

    /// Any property subpatterns the pattern matches.
    pub properties: IndexMap<Key, Space>,

    /// Additional properties for map/list semantics.
    pub additional_properties: IndexMap<Key, Space>,
}

impl SingleSpace {
    /// Dart `SingleSpace.empty`.
    pub fn empty() -> SingleSpace {
        SingleSpace::new(StaticType::NEVER_TYPE)
    }

    /// Creates a single space without properties.
    pub fn new(type_: StaticType) -> SingleSpace {
        SingleSpace::with_properties(type_, IndexMap::new(), IndexMap::new())
    }

    pub fn with_properties(
        type_: StaticType,
        properties: IndexMap<Key, Space>,
        additional_properties: IndexMap<Key, Space>,
    ) -> SingleSpace {
        SingleSpace {
            type_,
            properties,
            additional_properties,
        }
    }

    /// Dart `SingleSpace.toString`.
    pub fn to_text(&self, types: &dyn StaticTypeArena) -> String {
        types.space_to_text(self.type_, &self.properties, &self.additional_properties)
    }
}

impl PartialEq for SingleSpace {
    fn eq(&self, other: &SingleSpace) -> bool {
        if self.type_ != other.type_ {
            return false;
        }
        if self.properties.len() != other.properties.len() {
            return false;
        }
        for (key, value) in &self.properties {
            if other.properties.get(key) != Some(value) {
                return false;
            }
        }
        true
    }
}

impl Eq for SingleSpace {}

impl Hash for SingleSpace {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Dart hashes the type and the unordered keys and values. The number
        // of properties is consistent with `==` and keeps this simple.
        self.type_.hash(state);
        self.properties.len().hash(state);
    }
}

/// A set of runtime values encoded as a union of [SingleSpace]s.
///
/// This is used to support logical-or patterns without having to eagerly
/// expand the subpatterns in the parent context.
///
/// A shared handle: Dart `Space` has no `==` override, so equality and hashing
/// are object identity (`Rc` pointer identity).
#[derive(Clone, Debug)]
pub struct Space(Rc<SpaceData>);

#[derive(Debug)]
struct SpaceData {
    /// The path of getters that led from the original matched value to value
    /// matched by this pattern. Used to generate a human-readable witness.
    path: Path,

    single_spaces: Vec<Rc<SingleSpace>>,
}

impl Space {
    /// Create an empty space.
    pub fn empty(path: Path) -> Space {
        Space::new_internal(path, vec![Rc::new(SingleSpace::empty())])
    }

    /// Dart `Space(path, type)`.
    pub fn new(path: Path, type_: StaticType) -> Space {
        Space::with_properties(path, type_, IndexMap::new(), IndexMap::new())
    }

    /// Dart `Space(path, type, properties: ..., additionalProperties: ...)`.
    pub fn with_properties(
        path: Path,
        type_: StaticType,
        properties: IndexMap<Key, Space>,
        additional_properties: IndexMap<Key, Space>,
    ) -> Space {
        Space::new_internal(
            path,
            vec![Rc::new(SingleSpace::with_properties(
                type_,
                properties,
                additional_properties,
            ))],
        )
    }

    /// Dart `Space._`.
    fn new_internal(path: Path, single_spaces: Vec<Rc<SingleSpace>>) -> Space {
        Space(Rc::new(SpaceData {
            path,
            single_spaces,
        }))
    }

    /// Dart `Space.fromSingleSpaces`.
    pub fn from_single_spaces(
        types: &dyn StaticTypeArena,
        path: Path,
        single_spaces: Vec<Rc<SingleSpace>>,
    ) -> Space {
        let mut single_spaces_set: IndexSet<Rc<SingleSpace>> = IndexSet::new();

        let empty = SingleSpace::empty();
        for single_space in single_spaces {
            // Discard empty space.
            if *single_space == empty {
                continue;
            }

            single_spaces_set.insert(single_space);
        }

        let mut single_spaces_list: Vec<Rc<SingleSpace>> = single_spaces_set.into_iter().collect();
        if single_spaces_list.is_empty() {
            single_spaces_list.push(Rc::new(empty));
        } else if single_spaces_list.len() == 2 {
            if single_spaces_list[0].type_ == StaticType::NULL_TYPE
                && single_spaces_list[0].properties.is_empty()
                && single_spaces_list[1].properties.is_empty()
            {
                single_spaces_list = vec![Rc::new(SingleSpace::new(
                    types.nullable(single_spaces_list[1].type_),
                ))];
            } else if single_spaces_list[1].type_ == StaticType::NULL_TYPE
                && single_spaces_list[1].properties.is_empty()
                && single_spaces_list[0].properties.is_empty()
            {
                single_spaces_list = vec![Rc::new(SingleSpace::new(
                    types.nullable(single_spaces_list[0].type_),
                ))];
            }
        }
        Space::new_internal(path, single_spaces_list)
    }

    pub fn union(&self, types: &dyn StaticTypeArena, other: &Space) -> Space {
        Space::from_single_spaces(
            types,
            self.path().clone(),
            self.single_spaces()
                .iter()
                .chain(other.single_spaces().iter())
                .cloned()
                .collect(),
        )
    }

    /// The path of getters that led from the original matched value to value
    /// matched by this pattern. Used to generate a human-readable witness.
    pub fn path(&self) -> &Path {
        &self.0.path
    }

    pub fn single_spaces(&self) -> &[Rc<SingleSpace>] {
        &self.0.single_spaces
    }

    /// Dart `Space.toString`.
    pub fn to_text(&self, types: &dyn StaticTypeArena) -> String {
        self.single_spaces()
            .iter()
            .map(|s| s.to_text(types))
            .collect::<Vec<_>>()
            .join("|")
    }
}

impl PartialEq for Space {
    fn eq(&self, other: &Space) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Space {}

impl Hash for Space {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(Rc::as_ptr(&self.0), state);
    }
}
