// Dart source: pkg/_fe_analyzer_shared/lib/src/exhaustiveness/test_helper.dart

use indexmap::IndexSet;

use super::exhaustive::NonExhaustiveness;
use super::key::Key;
use super::space::Space;
use super::static_type::{ObjectPropertyLookup, StaticType, StaticTypeArena};

/// Tags used for id-testing of exhaustiveness.
pub struct Tags;

impl Tags {
    pub const ERROR: &'static str = "error";
    pub const SCRUTINEE_TYPE: &'static str = "type";
    pub const SCRUTINEE_FIELDS: &'static str = "fields";
    pub const SPACE: &'static str = "space";
    pub const SUBTYPES: &'static str = "subtypes";
    pub const EXPANDED_SUBTYPES: &'static str = "expandedSubtypes";
    pub const CHECKING_ORDER: &'static str = "checkingOrder";
}

/// Returns a textual representation for [space] used for testing.
pub fn spaces_to_text(types: &dyn StaticTypeArena, space: &Space) -> String {
    let text = space.to_text(types);
    if text.starts_with('[') && (text.ends_with(']') || text.ends_with("]?")) {
        // Avoid list-like syntax which collides with the [Features] encoding.
        return format!("<{text}>");
    }
    text
}

/// Returns a textual representation for [fields_of_interest] used for
/// testing.
pub fn fields_to_text(
    type_: StaticType,
    object_field_lookup: &dyn ObjectPropertyLookup,
    fields_of_interest: &IndexSet<Key>,
) -> String {
    let types = object_field_lookup.static_types();
    let mut sorted_names: Vec<Key> = fields_of_interest.iter().cloned().collect();
    sorted_names.sort_by(|a, b| a.compare_to(b, types));
    let mut sb = String::new();
    let mut comma = "";
    sb.push('{');
    for key in &sorted_names {
        sb.push_str(comma);
        if let Key::Extension(key) = key {
            sb.push_str(&types.name(key.receiver_type));
            sb.push('.');
            sb.push_str(&key.name);
            sb.push(':');
            sb.push_str(&static_type_to_text(types, key.type_));
        } else {
            let field_type = types.get_property_type(type_, object_field_lookup, key);
            sb.push_str(&key.name());
            sb.push(':');
            if let Some(field_type) = field_type {
                sb.push_str(&static_type_to_text(types, field_type));
            } else {
                sb.push('-');
            }
        }
        comma = ",";
    }
    sb.push('}');
    sb
}

/// Returns a textual representation for [type_] used for testing.
pub fn static_type_to_text(types: &dyn StaticTypeArena, type_: StaticType) -> String {
    types.name(type_)
}

/// Returns a textual representation of [types_] used for testing.
pub fn types_to_text(types: &dyn StaticTypeArena, types_: &[StaticType]) -> Option<String> {
    if types_.is_empty() {
        return None;
    }
    // TODO(johnniwinther): Sort types.
    let mut sb = String::new();
    let mut comma = "";
    sb.push('{');
    for subtype in types_ {
        sb.push_str(comma);
        sb.push_str(&static_type_to_text(types, *subtype));
        comma = ",";
    }
    sb.push('}');
    Some(sb)
}

pub fn non_exhaustiveness_to_text(
    types: &dyn StaticTypeArena,
    non_exhaustiveness: &NonExhaustiveness,
) -> String {
    let mut sb = String::new();
    sb.push_str("non-exhaustive:");
    let mut delimiter = "";
    for witness in &non_exhaustiveness.witnesses {
        sb.push_str(delimiter);
        let witness_text = witness.as_witness(types);
        let correction_text = witness.as_correction(types);
        if witness_text != correction_text {
            sb.push_str(&format!("{witness_text}/{correction_text}"));
        } else {
            sb.push_str(&witness_text);
        }
        delimiter = ";";
    }
    sb
}
