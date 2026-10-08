// Dart source: pkg/_fe_analyzer_shared/lib/src/exhaustiveness/types/map.dart

use indexmap::{IndexMap, IndexSet};

use super::super::dart_template_buffer::DartTemplateBuffer;
use super::super::key::{Key, MapKey};
use super::super::shared::{ExhaustivenessCache, TypeOperations};
use super::super::space::Space;
use super::super::static_type::StaticType;
use super::super::witness::PropertyWitness;
use super::{
    CacheTypeBased, EnumOperations, Restriction, SealedClassOperations, TypeBasedKind,
    TypeBasedStaticType,
};

impl<TO, EO, SO> ExhaustivenessCache<TO, EO, SO>
where
    TO: TypeOperations,
    EO: EnumOperations<Type = TO::Type>,
    SO: SealedClassOperations<Type = TO::Type>,
{
    /// Dart `new MapPatternStaticType<Type>(typeOperations, fieldLookup, type,
    /// restriction, name)`: [StaticType] for a map pattern type using a
    /// [MapTypeRestriction] for its uniqueness.
    pub(crate) fn new_map_pattern_static_type(
        &self,
        type_: TO::Type,
        restriction: MapTypeRestriction<TO::Type>,
        name: String,
    ) -> StaticType {
        self.new_type_based_static_type(TypeBasedStaticType::new(
            type_,
            false,
            Restriction::Map(restriction),
            Some(name),
            TypeBasedKind::MapPattern,
        ))
    }

    /// `MapPatternStaticType.spaceToText`.
    pub(crate) fn map_pattern_static_type_space_to_text(
        &self,
        this: &CacheTypeBased<TO, EO, SO>,
        additional_space_properties: &IndexMap<Key, Space>,
    ) -> String {
        let Restriction::Map(restriction) = &this.restriction else {
            unreachable!("MapPatternStaticType without MapTypeRestriction");
        };
        let mut buffer = String::new();
        buffer.push_str(&restriction.type_arguments_text);
        buffer.push('{');

        let mut first = true;
        for (key, space) in additional_space_properties {
            if !first {
                buffer.push_str(", ");
            }
            buffer.push_str(&format!("{}: {}", key.to_text(self), space.to_text(self)));
            first = false;
        }

        buffer.push('}');
        buffer
    }

    /// `MapPatternStaticType.witnessToDart`.
    pub(crate) fn map_pattern_static_type_witness_to_dart(
        &self,
        this: &CacheTypeBased<TO, EO, SO>,
        buffer: &mut dyn DartTemplateBuffer,
        witness_fields: &IndexMap<Key, &PropertyWitness>,
        for_correction: bool,
    ) {
        let Restriction::Map(restriction) = &this.restriction else {
            unreachable!("MapPatternStaticType without MapTypeRestriction");
        };
        buffer.write("{");
        let mut comma = "";
        for key in &restriction.keys {
            buffer.write(comma);
            buffer.write(&key.value_as_text);
            buffer.write(": ");
            if let Some(witness) = witness_fields.get(&Key::Map(key.clone())) {
                witness.witness_to_dart(self, buffer, for_correction);
            } else {
                buffer.write("_");
            }
            comma = ", ";
        }
        buffer.write("}");

        // If we have restrictions on the record type we create an and pattern.
        let mut additional_start = " && Object(";
        let mut additional_end = "";
        comma = "";
        for (key, field) in witness_fields {
            if !key.is_map_key() {
                buffer.write(additional_start);
                additional_start = "";
                additional_end = ")";
                buffer.write(comma);
                comma = ", ";

                buffer.write(&key.name());
                buffer.write(": ");
                field.witness_to_dart(self, buffer, for_correction);
            }
        }
        buffer.write(additional_end);
    }
}

/// Restriction object used for creating a unique `MapPatternStaticType` for a
/// map pattern.
///
/// The uniqueness is defined by the key and value types, the key values of
/// the map pattern, and whether the map pattern has a rest element.
///
/// This identity ensures that we can detect overlap between map patterns with
/// the same set of keys.
///
/// Dart `==` compares [key_type], [value_type] and the set of [keys]; see
/// `Restriction::eq_key`.
#[derive(Clone, Debug)]
pub struct MapTypeRestriction<T> {
    pub key_type: T,
    pub value_type: T,
    pub keys: IndexSet<MapKey>,
    pub type_arguments_text: String,
}

impl<T: Clone + Eq + std::hash::Hash + std::fmt::Debug + 'static> MapTypeRestriction<T> {
    pub fn new(
        key_type: T,
        value_type: T,
        keys: IndexSet<MapKey>,
        type_arguments_text: String,
    ) -> MapTypeRestriction<T> {
        MapTypeRestriction {
            key_type,
            value_type,
            keys,
            type_arguments_text,
        }
    }

    pub fn is_unrestricted(&self) -> bool {
        // The map pattern containing only a rest pattern covers the whole type.
        self.keys.is_empty()
    }

    pub fn is_subtype_of(
        &self,
        type_operations: &dyn TypeOperations<Type = T>,
        other: &Restriction<T>,
    ) -> bool {
        if other.is_unrestricted() {
            return true;
        }
        let Restriction::Map(other) = other else {
            return false;
        };
        if !type_operations.is_subtype_of(&self.key_type, &other.key_type) {
            return false;
        }
        if !type_operations.is_subtype_of(&self.value_type, &other.value_type) {
            return false;
        }
        other.keys.iter().all(|key| self.keys.contains(key))
    }
}

impl<T> std::fmt::Display for MapTypeRestriction<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut sb = String::new();
        sb.push_str(&self.type_arguments_text);
        sb.push('{');
        let mut comma = "";
        for key in &self.keys {
            sb.push_str(comma);
            sb.push_str(&key.to_string());
            sb.push_str(": ()");
            comma = ", ";
        }
        sb.push('}');
        f.write_str(&sb)
    }
}
