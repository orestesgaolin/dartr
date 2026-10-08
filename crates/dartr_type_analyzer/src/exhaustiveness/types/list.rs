// Dart source: pkg/_fe_analyzer_shared/lib/src/exhaustiveness/types/list.dart

use indexmap::{IndexMap, IndexSet};

use super::super::dart_template_buffer::DartTemplateBuffer;
use super::super::key::Key;
use super::super::shared::{ExhaustivenessCache, TypeOperations};
use super::super::space::Space;
use super::super::static_type::StaticType;
use super::super::witness::PropertyWitness;
use super::{
    CacheTypeBased, EnumOperations, Restriction, SealedClassOperations, TypeBasedKind,
    TypeBasedStaticType,
};

// [StaticType] for a list type which can be divided into subtypes of
// `ListPatternStaticType`.
//
// This is used to support exhaustiveness checking for list types by
// contextually dividing the list into relevant cases for checking.
//
// For instance, the exhaustiveness can be achieved by a single pattern
//
//     case [...]:
//
// or by two disjoint patterns:
//
//     case []:
//     case [_, ...]:
//
// When checking for exhaustiveness, witness candidates are created and tested
// against the available cases. This means that the chosen candidates must be
// matched by at least one case or the candidate is considered a witness of
// non-exhaustiveness.
//
// Looking at the first example, we could choose `[...]`, the list of
// arbitrary size, as a candidate. This works for the first example, since the
// case `[...]` matches the list of arbitrary size. But if we tried to use this
// on the second example it would fail, since neither `[]` nor `[_, ...]` fully
// matches the list of arbitrary size.
//
// A solution could be to choose candidates `[]` and `[_, ...]`, the empty list
// and the list of 1 or more elements. This would work for the first example,
// since `[...]` matches both the empty list and the list of 1 or more
// elements. It also works for the second example, since `[]` matches the empty
// list and `[_, ...]` matches the list of 1 or more elements.
//
// But now comes a third way of exhaustively matching a list:
//
//     case []:
//     case [_]:
//     case [_, _, ...]:
//
// and our candidates no longer work, since while `[]` does match the empty
// list, neither `[_]` nor `[_, _, ...]` matches the list of 1 or more
// elements.
//
// This shows us that there can be no fixed set of witness candidates that we
// can use to match a list type.
//
// What we do instead, is to create the set of witness candidates based on the
// cases that should match it. We find the maximal number, n, of fixed, i.e.
// non-rest, elements in the cases, and then create the lists of sizes 0 to n-1
// and the list of n or more elements as the witness candidates.
//
// (Dart class `ListTypeStaticType`; see [TypeBasedKind::ListType].)

impl<TO, EO, SO> ExhaustivenessCache<TO, EO, SO>
where
    TO: TypeOperations,
    EO: EnumOperations<Type = TO::Type>,
    SO: SealedClassOperations<Type = TO::Type>,
{
    /// Dart `new ListTypeStaticType(typeOperations, fieldLookup, type)`.
    pub(crate) fn new_list_type_static_type(&self, type_: TO::Type) -> StaticType {
        self.new_type_based_static_type(TypeBasedStaticType::new(
            type_,
            false,
            Restriction::Unrestricted,
            None,
            TypeBasedKind::ListType,
        ))
    }

    /// `ListTypeStaticType.getSubtypes`.
    pub(crate) fn list_type_static_type_get_subtypes(
        &self,
        this: &CacheTypeBased<TO, EO, SO>,
        keys_of_interest: &IndexSet<Key>,
    ) -> Vec<StaticType> {
        let mut max_head_size = 0;
        let mut max_tail_size = 0;
        for key in keys_of_interest {
            if let Key::Head(index) = key {
                if *index >= max_head_size {
                    max_head_size = index + 1;
                }
            } else if let Key::Tail(index) = key
                && *index >= max_tail_size
            {
                max_tail_size = index + 1;
            }
        }
        let max_size = max_head_size + max_tail_size;
        let mut subtypes: Vec<StaticType> = vec![];
        let element_type = self
            .type_operations
            .get_list_element_type(&this.type_)
            .expect("List element type");
        let type_argument_text = if self.type_operations.is_dynamic(&element_type) {
            String::new()
        } else {
            format!("<{}>", self.type_operations.type_to_string(&element_type))
        };
        for size in 0..max_size {
            let identity = ListTypeRestriction::new(
                element_type.clone(),
                type_argument_text.clone(),
                size,
                false,
            );
            let name = identity.to_string();
            subtypes.push(self.new_list_pattern_static_type(this.type_.clone(), identity, name));
        }
        let identity = ListTypeRestriction::new(element_type, type_argument_text, max_size, true);
        let name = identity.to_string();
        subtypes.push(self.new_list_pattern_static_type(this.type_.clone(), identity, name));
        subtypes
    }

    /// Dart `new ListPatternStaticType<Type>(typeOperations, fieldLookup, type,
    /// restriction, name)`: [StaticType] for a list pattern type using a
    /// [ListTypeRestriction] for its uniqueness.
    pub(crate) fn new_list_pattern_static_type(
        &self,
        type_: TO::Type,
        restriction: ListTypeRestriction<TO::Type>,
        name: String,
    ) -> StaticType {
        self.new_type_based_static_type(TypeBasedStaticType::new(
            type_,
            false,
            Restriction::List(restriction),
            Some(name),
            TypeBasedKind::ListPattern,
        ))
    }

    /// `ListPatternStaticType.spaceToText`.
    pub(crate) fn list_pattern_static_type_space_to_text(
        &self,
        this: &CacheTypeBased<TO, EO, SO>,
        additional_space_properties: &IndexMap<Key, Space>,
    ) -> String {
        let Restriction::List(restriction) = &this.restriction else {
            unreachable!("ListPatternStaticType without ListTypeRestriction");
        };
        let mut buffer = String::new();
        buffer.push_str(&restriction.type_argument_text);
        buffer.push('[');

        let mut first = true;
        for (key, space) in additional_space_properties {
            if !first {
                buffer.push_str(", ");
            }
            if let Key::Rest { .. } = key {
                buffer.push_str("...");
            }
            buffer.push_str(&space.to_text(self));
            first = false;
        }

        buffer.push(']');
        buffer
    }

    /// `ListPatternStaticType.witnessToDart`.
    pub(crate) fn list_pattern_static_type_witness_to_dart(
        &self,
        this: &CacheTypeBased<TO, EO, SO>,
        buffer: &mut dyn DartTemplateBuffer,
        witness_fields: &IndexMap<Key, &PropertyWitness>,
        for_correction: bool,
    ) {
        let Restriction::List(restriction) = &this.restriction else {
            unreachable!("ListPatternStaticType without ListTypeRestriction");
        };
        let mut max_head_size = 0;
        let mut max_tail_size = 0;
        let mut rest_witness: Option<&PropertyWitness> = None;
        for (key, value) in witness_fields {
            match key {
                Key::Head(index) if *index >= max_head_size => {
                    max_head_size = index + 1;
                }
                // Dart compares the tail index with `maxHeadSize` here.
                Key::Tail(index) if *index >= max_head_size => {
                    max_tail_size = index + 1;
                }
                Key::Rest { .. } => {
                    // TODO(johnniwinther): Can the rest key have head/tail
                    // sizes that don't match the found max head/tail sizes?
                    rest_witness = Some(value);
                }
                _ => {}
            }
        }
        if max_head_size + max_tail_size < restriction.size {
            max_head_size = restriction.size - max_tail_size;
        }
        buffer.write("[");
        let mut comma = "";
        for index in 0..max_head_size {
            buffer.write(comma);
            let key = Key::Head(index);
            if let Some(witness) = witness_fields.get(&key) {
                witness.witness_to_dart(self, buffer, for_correction);
            } else {
                buffer.write("_");
            }
            comma = ", ";
        }
        if restriction.has_rest {
            buffer.write(comma);
            buffer.write("...");
            if let Some(rest_witness) = rest_witness {
                rest_witness.witness_to_dart(self, buffer, for_correction);
            }
            comma = ", ";
        }
        for index in (0..max_tail_size).rev() {
            buffer.write(comma);
            let key = Key::Tail(index);
            if let Some(witness) = witness_fields.get(&key) {
                witness.witness_to_dart(self, buffer, for_correction);
            } else {
                buffer.write("_");
            }
            comma = ", ";
        }
        buffer.write("]");

        // If we have restrictions on the record type we create an and pattern.
        let mut additional_start = " && Object(";
        let mut additional_end = "";
        comma = "";
        for (key, field) in witness_fields {
            if !key.is_list_key() {
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

/// Restriction object used for creating a unique `ListPatternStaticType` for
/// a list pattern.
///
/// The uniqueness is defined by the element type, the number of elements at
/// the start of the list, whether the list pattern has a rest element, and the
/// number elements at the end of the list, after the rest element.
///
/// Dart `==` compares [element_type], [size] and [has_rest]; see
/// `Restriction::eq_key`.
#[derive(Clone, Debug)]
pub struct ListTypeRestriction<T> {
    pub element_type: T,
    pub size: usize,
    pub has_rest: bool,
    pub type_argument_text: String,
}

impl<T: Clone + Eq + std::hash::Hash + std::fmt::Debug + 'static> ListTypeRestriction<T> {
    pub fn new(
        element_type: T,
        type_argument_text: String,
        size: usize,
        has_rest: bool,
    ) -> ListTypeRestriction<T> {
        ListTypeRestriction {
            element_type,
            size,
            has_rest,
            type_argument_text,
        }
    }

    pub fn is_unrestricted(&self) -> bool {
        // The map pattern containing only a rest pattern covers the whole type.
        self.has_rest && self.size == 0
    }

    pub fn is_subtype_of(
        &self,
        type_operations: &dyn TypeOperations<Type = T>,
        other: &Restriction<T>,
    ) -> bool {
        if other.is_unrestricted() {
            return true;
        }
        let Restriction::List(other) = other else {
            return false;
        };
        if !type_operations.is_subtype_of(&self.element_type, &other.element_type) {
            return false;
        }
        if other.has_rest {
            self.size >= other.size
        } else if self.has_rest {
            false
        } else {
            self.size == other.size
        }
    }
}

impl<T> std::fmt::Display for ListTypeRestriction<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut sb = String::new();
        sb.push_str(&self.type_argument_text);
        sb.push('[');
        let mut comma = "";
        for _ in 0..self.size {
            sb.push_str(comma);
            sb.push_str("()");
            comma = ", ";
        }
        if self.has_rest {
            sb.push_str(comma);
            sb.push_str("...");
        }
        sb.push(']');
        f.write_str(&sb)
    }
}
