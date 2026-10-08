// Dart source: pkg/_fe_analyzer_shared/lib/src/exhaustiveness/witness.dart

use std::cell::OnceCell;

use indexmap::IndexMap;

use super::dart_template_buffer::{DartTemplateBuffer, SimpleDartBuffer};
use super::key::Key;
use super::path::Path;
use super::static_type::{StaticType, StaticTypeArena};

/// Describes a pattern that matches the value or a field accessed from it.
///
/// Used only to generate the witness description.
#[derive(Clone, Debug)]
pub struct Predicate {
    /// The path of getters that led from the original matched value to the
    /// value tested by this predicate.
    pub path: Path,

    /// The static type of the context. [value_type] is a subtype of this type.
    pub static_type: StaticType,

    /// The type this predicate tests.
    // TODO(johnniwinther): In order to model exhaustiveness on enum types,
    // bool values, and maybe integers at some point, we may later want a
    // separate kind of predicate that means "this value was equal to this
    // constant".
    pub value_type: StaticType,
}

impl Predicate {
    pub fn new(path: Path, static_type: StaticType, value_type: StaticType) -> Predicate {
        Predicate {
            path,
            static_type,
            value_type,
        }
    }

    /// Dart `Predicate.toString`.
    pub fn to_text(&self, types: &dyn StaticTypeArena) -> String {
        format!(
            "Predicate(path={},type={})",
            self.path,
            types.name(self.value_type)
        )
    }
}

/// Witness that show an unmatched case.
///
/// This is used to builds a human-friendly pattern-like string for the witness
/// matched by [predicates].
///
/// For example, given:
///
/// ```text
///     [] is U
///     ['w'] is T
///     ['w', 'x'] is B
///     ['w', 'y'] is B
///     ['z'] is T
///     ['z', 'x'] is C
///     ['z', 'y'] is B
/// ```
///
/// the [to_text] produces:
///
/// ```text
///     'U(w: T(x: B, y: B), z: T(x: C, y: B))'
/// ```
#[derive(Debug)]
pub struct Witness {
    predicates: Vec<Predicate>,
    witness: OnceCell<PropertyWitness>,
}

impl Witness {
    pub fn new(predicates: Vec<Predicate>) -> Witness {
        Witness {
            predicates,
            witness: OnceCell::new(),
        }
    }

    fn witness(&self) -> &PropertyWitness {
        self.witness.get_or_init(|| self.build_witness())
    }

    fn build_witness(&self) -> PropertyWitness {
        let mut witness = PropertyWitness::new();

        for predicate in &self.predicates {
            let mut here = &mut witness;
            for field in predicate.path.to_list() {
                here = here.properties.entry(field).or_default();
            }
            here.static_type = predicate.static_type;
            here.value_type = predicate.value_type;
        }
        witness
    }

    pub fn as_witness(&self, types: &dyn StaticTypeArena) -> String {
        self.witness().as_witness(types)
    }

    pub fn as_correction(&self, types: &dyn StaticTypeArena) -> String {
        self.witness().as_correction(types)
    }

    /// Writes a representation of this witness to the given [buffer].
    pub fn to_dart(
        &self,
        types: &dyn StaticTypeArena,
        buffer: &mut dyn DartTemplateBuffer,
        for_correction: bool,
    ) {
        self.witness()
            .witness_to_dart(types, buffer, for_correction);
    }

    /// Dart `Witness.toString`.
    pub fn to_text(&self, types: &dyn StaticTypeArena) -> String {
        self.witness().to_text(types)
    }
}

/// Helper class used to turn a list of [Predicate]s into a string.
#[derive(Debug)]
pub struct PropertyWitness {
    pub static_type: StaticType,
    pub value_type: StaticType,
    pub properties: IndexMap<Key, PropertyWitness>,
}

impl Default for PropertyWitness {
    fn default() -> Self {
        PropertyWitness::new()
    }
}

impl PropertyWitness {
    pub fn new() -> PropertyWitness {
        PropertyWitness {
            static_type: StaticType::NULLABLE_OBJECT,
            value_type: StaticType::NULLABLE_OBJECT,
            properties: IndexMap::new(),
        }
    }

    pub fn witness_to_dart(
        &self,
        types: &dyn StaticTypeArena,
        buffer: &mut dyn DartTemplateBuffer,
        for_correction: bool,
    ) {
        if !self.properties.is_empty() {
            let mut witness_fields_by_type: IndexMap<StaticType, IndexMap<Key, &PropertyWitness>> =
                IndexMap::new();
            for (key, witness) in &self.properties {
                if for_correction && witness.is_trivial(types) {
                    continue;
                }
                if let Key::Extension(extension_key) = key {
                    witness_fields_by_type
                        .entry(extension_key.receiver_type)
                        .or_default()
                        .insert(key.clone(), witness);
                } else {
                    witness_fields_by_type
                        .entry(self.value_type)
                        .or_default()
                        .insert(key.clone(), witness);
                }
            }
            if !witness_fields_by_type.is_empty() {
                let mut and = "";
                for (type_, witness_fields) in &witness_fields_by_type {
                    buffer.write(and);
                    and = " && ";
                    types.witness_to_dart(*type_, buffer, self, witness_fields, for_correction);
                }
            } else {
                types.witness_to_dart(
                    self.value_type,
                    buffer,
                    self,
                    &IndexMap::new(),
                    for_correction,
                );
            }
        } else {
            types.witness_to_dart(
                self.value_type,
                buffer,
                self,
                &IndexMap::new(),
                for_correction,
            );
        }
    }

    pub fn is_trivial(&self, types: &dyn StaticTypeArena) -> bool {
        if !types.is_subtype_of(self.static_type, self.value_type) {
            return false;
        }
        for property in self.properties.values() {
            if !property.is_trivial(types) {
                return false;
            }
        }
        true
    }

    /// Returns the witness as pattern syntax including all subproperties.
    pub fn as_witness(&self, types: &dyn StaticTypeArena) -> String {
        let mut buffer = SimpleDartBuffer::new();
        self.witness_to_dart(types, &mut buffer, false);
        buffer.to_string()
    }

    /// Return the witness as pattern syntax without subproperties that fully
    /// match the static type.
    pub fn as_correction(&self, types: &dyn StaticTypeArena) -> String {
        let mut buffer = SimpleDartBuffer::new();
        self.witness_to_dart(types, &mut buffer, true);
        buffer.to_string()
    }

    /// Dart `PropertyWitness.toString`.
    pub fn to_text(&self, types: &dyn StaticTypeArena) -> String {
        self.as_witness(types)
    }
}
