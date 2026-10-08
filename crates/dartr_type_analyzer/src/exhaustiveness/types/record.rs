// Dart source: pkg/_fe_analyzer_shared/lib/src/exhaustiveness/types/record.dart

use indexmap::IndexMap;

use super::super::dart_template_buffer::DartTemplateBuffer;
use super::super::key::Key;
use super::super::shared::{ExhaustivenessCache, TypeOperations};
use super::super::space::Space;
use super::super::static_type::{StaticType, StaticTypeArena};
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
    /// Dart `new RecordStaticType(typeOperations, fieldLookup, type)`:
    /// [StaticType] for a record type.
    pub(crate) fn new_record_static_type(&self, type_: TO::Type) -> StaticType {
        self.new_type_based_static_type(TypeBasedStaticType::new(
            type_,
            false,
            Restriction::Unrestricted,
            None,
            TypeBasedKind::Record,
        ))
    }

    /// `RecordStaticType.spaceToText`.
    pub(crate) fn record_static_type_space_to_text(
        &self,
        this: &CacheTypeBased<TO, EO, SO>,
        space_properties: &IndexMap<Key, Space>,
        additional_space_properties: &IndexMap<Key, Space>,
    ) -> String {
        let mut buffer = String::new();
        buffer.push('(');
        let mut comma = "";
        for (key, static_type) in self.type_based_fields(this).iter() {
            let text = match space_properties.get(key) {
                Some(space) => space.to_text(self),
                None => self.name(*static_type),
            };
            if let Key::RecordIndex(_) = key {
                buffer.push_str(comma);
                comma = ", ";
                buffer.push_str(&text);
            } else if let Key::RecordName(_) = key {
                buffer.push_str(comma);
                comma = ", ";
                buffer.push_str(&format!("{}: {}", key.name(), text));
            }
        }
        buffer.push(')');
        let mut additional_start = "(";
        let mut additional_end = "";
        comma = "";
        for (key, value) in space_properties {
            if !key.is_record_key() {
                buffer.push_str(additional_start);
                additional_start = "";
                additional_end = ")";
                buffer.push_str(comma);
                comma = ", ";
                buffer.push_str(&format!("{}: {}", key.name(), value.to_text(self)));
            }
        }
        for (key, value) in additional_space_properties {
            if !key.is_record_key() {
                buffer.push_str(additional_start);
                additional_start = "";
                additional_end = ")";
                buffer.push_str(comma);
                comma = ", ";
                buffer.push_str(&format!("{}: {}", key.name(), value.to_text(self)));
            }
        }
        buffer.push_str(additional_end);
        buffer
    }

    /// `RecordStaticType.witnessToDart`.
    pub(crate) fn record_static_type_witness_to_dart(
        &self,
        this: &CacheTypeBased<TO, EO, SO>,
        buffer: &mut dyn DartTemplateBuffer,
        witness_fields: &IndexMap<Key, &PropertyWitness>,
        for_correction: bool,
    ) {
        buffer.write("(");
        let mut comma = "";
        for key in self.type_based_fields(this).keys() {
            if let Key::RecordIndex(_) = key {
                buffer.write(comma);
                comma = ", ";

                if let Some(field) = witness_fields.get(key) {
                    field.witness_to_dart(self, buffer, for_correction);
                } else {
                    buffer.write("_");
                }
            } else if let Key::RecordName(_) = key {
                buffer.write(comma);
                comma = ", ";

                buffer.write(&key.name());
                buffer.write(": ");
                if let Some(field) = witness_fields.get(key) {
                    field.witness_to_dart(self, buffer, for_correction);
                } else {
                    buffer.write("_");
                }
            }
        }
        buffer.write(")");

        // If we have restrictions on the record type we create an and pattern.
        let mut additional_start = " && Object(";
        let mut additional_end = "";
        comma = "";
        for (key, field) in witness_fields {
            if !key.is_record_key() {
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
