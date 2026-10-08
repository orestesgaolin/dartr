// Dart source: pkg/_fe_analyzer_shared/lib/src/exhaustiveness/types.dart

//! The [StaticType]s based on a client `Type`.
//!
//! The Dart class hierarchy below `TypeBasedStaticType` is the struct
//! [TypeBasedStaticType] with the subclass-specific data in
//! [TypeBasedKind]. The methods are methods of the
//! [ExhaustivenessCache](super::shared::ExhaustivenessCache), which owns the
//! static type arena and the client operations (Dart `_typeOperations`,
//! `_fieldLookup`, `_cache`). The Dart `part` files are the submodules in
//! `types/`.

use std::any::Any;
use std::cell::OnceCell;
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};

use indexmap::{IndexMap, IndexSet};

use super::dart_template_buffer::DartTemplateBuffer;
use super::key::{Identity, Key, MapKey};
use super::shared::{ExhaustivenessCache, TypeOperations};
use super::space::Space;
use super::static_type::{self as st, StaticType};
use super::witness::PropertyWitness;

#[path = "types/bool.rs"]
mod bool_type;
#[path = "types/enum.rs"]
mod enum_type;
#[path = "types/future_or.rs"]
mod future_or;
#[path = "types/list.rs"]
mod list;
#[path = "types/map.rs"]
mod map;
#[path = "types/record.rs"]
mod record;
#[path = "types/sealed.rs"]
mod sealed;

pub use bool_type::BoolStaticType;
pub use enum_type::{EnumInfo, EnumOperations, EnumStaticType};
pub use future_or::FutureOrStaticType;
pub use list::ListTypeRestriction;
pub use map::MapTypeRestriction;
pub use sealed::{SealedClassInfo, SealedClassOperations, SealedClassStaticType};

/// [StaticType] based on a non-nullable `Type`.
///
/// All [StaticType] implementation in this library are based on `Type` through
/// this class. Additionally, the `static_type.dart` library has fixed
/// [StaticType] implementations for `Object`, `Null`, `Never` and nullable
/// types.
///
/// The fields of the Dart subclasses are in [kind]. The Dart class
/// `RestrictedStaticType` is the case where [restricted_name] is set.
pub struct TypeBasedStaticType<T, E, S> {
    pub(crate) type_: T,
    pub(crate) is_implicitly_nullable: bool,

    /// Returns a [Restriction] value for static types the determines subtypes
    /// of the [type_]. For instance individual elements of an enum.
    ///
    /// `const Unrestricted()` for the classes that are not a
    /// `RestrictedStaticType`.
    pub(crate) restriction: Restriction<T>,

    /// `RestrictedStaticType.name`.
    pub(crate) restricted_name: Option<String>,

    pub(crate) kind: TypeBasedKind<E, S>,

    /// `NonNullableStaticType.nullable`.
    pub(crate) nullable: OnceCell<StaticType>,
}

impl<T, E, S> TypeBasedStaticType<T, E, S> {
    pub(crate) fn new(
        type_: T,
        is_implicitly_nullable: bool,
        restriction: Restriction<T>,
        restricted_name: Option<String>,
        kind: TypeBasedKind<E, S>,
    ) -> Self {
        TypeBasedStaticType {
            type_,
            is_implicitly_nullable,
            restriction,
            restricted_name,
            kind,
            nullable: OnceCell::new(),
        }
    }

    pub fn type_for_testing(&self) -> &T {
        &self.type_
    }

    pub fn restriction(&self) -> &Restriction<T> {
        &self.restriction
    }

    pub fn kind(&self) -> &TypeBasedKind<E, S> {
        &self.kind
    }
}

/// The Dart subclasses of `TypeBasedStaticType`.
pub enum TypeBasedKind<E, S> {
    /// `TypeBasedStaticType` itself.
    TypeBased,

    /// `BoolStaticType`.
    Bool(BoolStaticType),

    /// `_BoolValueStaticType` with its `_value`.
    BoolValue(bool),

    /// `GeneralValueStaticType`. The `_value` is the identity of the
    /// [IdentityRestriction].
    GeneralValue,

    /// `EnumElementStaticType`. The `_value` (the enum element) is the
    /// identity of the [IdentityRestriction].
    EnumElement,

    /// `EnumStaticType`.
    Enum(EnumStaticType<E>),

    /// `FutureOrStaticType`.
    FutureOr(FutureOrStaticType),

    /// `ListTypeStaticType`.
    ListType,

    /// `ListPatternStaticType`. The restriction is a
    /// [Restriction::List].
    ListPattern,

    /// `MapPatternStaticType`. The restriction is a [Restriction::Map].
    MapPattern,

    /// `RecordStaticType`.
    Record,

    /// `SealedClassStaticType`.
    SealedClass(SealedClassStaticType<S>),
}

impl<E, S> TypeBasedKind<E, S> {
    /// Dart `is ValueStaticType`.
    fn is_value_static_type(&self) -> bool {
        matches!(
            self,
            TypeBasedKind::BoolValue(_) | TypeBasedKind::GeneralValue | TypeBasedKind::EnumElement
        )
    }
}

/// Interface for a restriction within a subtype relation.
///
/// This is used for instance to model enum values within an enum type and
/// map patterns within a map type.
///
/// The variants are the Dart implementations of `Restriction`.
#[derive(Clone, Debug)]
pub enum Restriction<T> {
    /// The unrestricted [Restriction] that covers all values of a type.
    Unrestricted,

    /// [Restriction] based a unique identity value.
    Identity(IdentityRestriction),

    /// See [ListTypeRestriction].
    List(ListTypeRestriction<T>),

    /// See [MapTypeRestriction].
    Map(MapTypeRestriction<T>),
}

impl<T: Clone + Eq + Hash + std::fmt::Debug + 'static> Restriction<T> {
    /// Returns `true` if this [Restriction] covers the whole type.
    pub fn is_unrestricted(&self) -> bool {
        match self {
            Restriction::Unrestricted => true,
            Restriction::Identity(_) => false,
            Restriction::List(restriction) => restriction.is_unrestricted(),
            Restriction::Map(restriction) => restriction.is_unrestricted(),
        }
    }

    /// Returns `true` if this restriction is a subtype of [other].
    pub fn is_subtype_of(
        &self,
        type_operations: &dyn TypeOperations<Type = T>,
        other: &Restriction<T>,
    ) -> bool {
        match self {
            Restriction::Unrestricted => other.is_unrestricted(),
            Restriction::Identity(restriction) => {
                other.is_unrestricted()
                    || matches!(other, Restriction::Identity(other)
                        if restriction.identity == other.identity)
            }
            Restriction::List(restriction) => restriction.is_subtype_of(type_operations, other),
            Restriction::Map(restriction) => restriction.is_subtype_of(type_operations, other),
        }
    }

    /// The key of Dart `==` / `hashCode` of this restriction.
    pub(crate) fn eq_key(&self) -> RestrictionEqKey<T> {
        match self {
            Restriction::Unrestricted => RestrictionEqKey::Unrestricted,
            Restriction::Identity(restriction) => RestrictionEqKey::Identity(restriction.object_id),
            Restriction::List(restriction) => RestrictionEqKey::List {
                element_type: restriction.element_type.clone(),
                size: restriction.size,
                has_rest: restriction.has_rest,
            },
            Restriction::Map(restriction) => RestrictionEqKey::Map {
                key_type: restriction.key_type.clone(),
                value_type: restriction.value_type.clone(),
                keys: restriction.keys.clone(),
            },
        }
    }
}

/// The Dart `==` of a [Restriction]: `Unrestricted` is a const singleton,
/// `IdentityRestriction` has object identity, and the list and map
/// restrictions are structural.
#[derive(Clone, Debug)]
pub(crate) enum RestrictionEqKey<T> {
    Unrestricted,
    Identity(u64),
    List {
        element_type: T,
        size: usize,
        has_rest: bool,
    },
    Map {
        key_type: T,
        value_type: T,
        keys: IndexSet<MapKey>,
    },
}

impl<T: Eq> PartialEq for RestrictionEqKey<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (RestrictionEqKey::Unrestricted, RestrictionEqKey::Unrestricted) => true,
            (RestrictionEqKey::Identity(a), RestrictionEqKey::Identity(b)) => a == b,
            (
                RestrictionEqKey::List {
                    element_type: a_element_type,
                    size: a_size,
                    has_rest: a_has_rest,
                },
                RestrictionEqKey::List {
                    element_type: b_element_type,
                    size: b_size,
                    has_rest: b_has_rest,
                },
            ) => a_element_type == b_element_type && a_size == b_size && a_has_rest == b_has_rest,
            (
                RestrictionEqKey::Map {
                    key_type: a_key_type,
                    value_type: a_value_type,
                    keys: a_keys,
                },
                RestrictionEqKey::Map {
                    key_type: b_key_type,
                    value_type: b_value_type,
                    keys: b_keys,
                },
            ) => {
                // `IndexSet ==` is order-insensitive, as Dart
                // `keys.length == other.keys.length && keys.containsAll(...)`.
                a_key_type == b_key_type && a_value_type == b_value_type && a_keys == b_keys
            }
            _ => false,
        }
    }
}

impl<T: Eq> Eq for RestrictionEqKey<T> {}

impl<T: Hash> Hash for RestrictionEqKey<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            RestrictionEqKey::Unrestricted => {}
            RestrictionEqKey::Identity(id) => id.hash(state),
            RestrictionEqKey::List {
                element_type,
                size,
                has_rest,
            } => {
                element_type.hash(state);
                size.hash(state);
                has_rest.hash(state);
            }
            RestrictionEqKey::Map {
                key_type,
                value_type,
                keys,
            } => {
                key_type.hash(state);
                value_type.hash(state);
                // Unordered: the number of keys is consistent with `==`.
                keys.len().hash(state);
            }
        }
    }
}

/// [Restriction] based a unique [identity] value.
///
/// Dart `IdentityRestriction` does not override `==`, so two restrictions are
/// equal only if they are the same object. [object_id] models this object
/// identity.
#[derive(Clone, Debug)]
pub struct IdentityRestriction {
    pub identity: Identity,
    object_id: u64,
}

static NEXT_IDENTITY_RESTRICTION_ID: AtomicU64 = AtomicU64::new(0);

impl IdentityRestriction {
    pub fn new(identity: Identity) -> IdentityRestriction {
        IdentityRestriction {
            identity,
            object_id: NEXT_IDENTITY_RESTRICTION_ID.fetch_add(1, Ordering::Relaxed),
        }
    }
}

/// Shorthand for the type based node of the arena of an
/// [ExhaustivenessCache].
pub(crate) type CacheTypeBased<TO, EO, SO> =
    TypeBasedStaticType<<TO as TypeOperations>::Type, EnumInfo<EO>, SealedClassInfo<SO>>;

impl<TO, EO, SO> ExhaustivenessCache<TO, EO, SO>
where
    TO: TypeOperations,
    EO: EnumOperations<Type = TO::Type>,
    SO: SealedClassOperations<Type = TO::Type>,
{
    // -----------------------------------------------------------------------
    // TypeBasedStaticType

    /// `TypeBasedStaticType.fields`.
    pub(crate) fn type_based_fields(
        &self,
        this: &CacheTypeBased<TO, EO, SO>,
    ) -> std::rc::Rc<IndexMap<Key, StaticType>> {
        self.get_field_types(&this.type_)
    }

    /// `TypeBasedStaticType.getAdditionalPropertyType`.
    pub(crate) fn type_based_get_additional_property_type(
        &self,
        this: &CacheTypeBased<TO, EO, SO>,
        key: &Key,
    ) -> Option<StaticType> {
        self.get_additional_field_type(&this.type_, key)
    }

    /// `TypeBasedStaticType.isSubtypeOfInternal`.
    pub(crate) fn type_based_is_subtype_of_internal(
        &self,
        this: &CacheTypeBased<TO, EO, SO>,
        other: StaticType,
    ) -> bool {
        let other_node = self.node(other);
        match other_node.non_nullable_data() {
            Some(other) => {
                self.type_operations
                    .is_subtype_of(&this.type_, &other.type_)
                    && this
                        .restriction
                        .is_subtype_of(&self.type_operations, &other.restriction)
            }
            None => false,
        }
    }

    /// `TypeBasedStaticType.isSealed` and its overrides.
    pub(crate) fn type_based_is_sealed(&self, this: &CacheTypeBased<TO, EO, SO>) -> bool {
        match &this.kind {
            TypeBasedKind::Bool(_)
            | TypeBasedKind::Enum(_)
            | TypeBasedKind::FutureOr(_)
            | TypeBasedKind::ListType
            | TypeBasedKind::SealedClass(_) => true,
            TypeBasedKind::TypeBased
            | TypeBasedKind::BoolValue(_)
            | TypeBasedKind::GeneralValue
            | TypeBasedKind::EnumElement
            | TypeBasedKind::ListPattern
            | TypeBasedKind::MapPattern
            | TypeBasedKind::Record => false,
        }
    }

    /// `TypeBasedStaticType.name` and `RestrictedStaticType.name`.
    pub(crate) fn type_based_name(&self, this: &CacheTypeBased<TO, EO, SO>) -> String {
        match &this.restricted_name {
            Some(name) => name.clone(),
            None => self.type_operations.type_to_string(&this.type_),
        }
    }

    /// `TypeBasedStaticType.getSubtypes` and its overrides.
    pub(crate) fn type_based_get_subtypes(
        &self,
        this_type: StaticType,
        this: &CacheTypeBased<TO, EO, SO>,
        keys_of_interest: &IndexSet<Key>,
    ) -> Vec<StaticType> {
        match &this.kind {
            TypeBasedKind::Bool(bool_type) => self.bool_static_type_get_subtypes(this, bool_type),
            TypeBasedKind::Enum(enum_type) => {
                self.enum_static_type_enum_elements(this_type, this, enum_type)
            }
            TypeBasedKind::FutureOr(future_or) => future_or.get_subtypes(),
            TypeBasedKind::ListType => {
                self.list_type_static_type_get_subtypes(this, keys_of_interest)
            }
            TypeBasedKind::SealedClass(sealed) => {
                self.sealed_class_static_type_get_subtypes(this_type, this, sealed)
            }
            TypeBasedKind::TypeBased
            | TypeBasedKind::BoolValue(_)
            | TypeBasedKind::GeneralValue
            | TypeBasedKind::EnumElement
            | TypeBasedKind::ListPattern
            | TypeBasedKind::MapPattern
            | TypeBasedKind::Record => vec![],
        }
    }

    /// `spaceToText` of the `TypeBasedStaticType` family.
    pub(crate) fn type_based_space_to_text(
        &self,
        this_type: StaticType,
        this: &CacheTypeBased<TO, EO, SO>,
        space_properties: &IndexMap<Key, Space>,
        additional_space_properties: &IndexMap<Key, Space>,
    ) -> String {
        match &this.kind {
            TypeBasedKind::ListPattern => {
                self.list_pattern_static_type_space_to_text(this, additional_space_properties)
            }
            TypeBasedKind::MapPattern => {
                self.map_pattern_static_type_space_to_text(this, additional_space_properties)
            }
            TypeBasedKind::Record => self.record_static_type_space_to_text(
                this,
                space_properties,
                additional_space_properties,
            ),
            _ => st::base_space_to_text(
                self,
                this_type,
                space_properties,
                additional_space_properties,
            ),
        }
    }

    /// `witnessToDart` of the `TypeBasedStaticType` family.
    pub(crate) fn type_based_witness_to_dart_dispatch(
        &self,
        this_type: StaticType,
        this: &CacheTypeBased<TO, EO, SO>,
        buffer: &mut dyn DartTemplateBuffer,
        witness: &PropertyWitness,
        witness_fields: &IndexMap<Key, &PropertyWitness>,
        for_correction: bool,
    ) {
        match &this.kind {
            TypeBasedKind::ListPattern => self.list_pattern_static_type_witness_to_dart(
                this,
                buffer,
                witness_fields,
                for_correction,
            ),
            TypeBasedKind::MapPattern => self.map_pattern_static_type_witness_to_dart(
                this,
                buffer,
                witness_fields,
                for_correction,
            ),
            TypeBasedKind::Record => self.record_static_type_witness_to_dart(
                this,
                buffer,
                witness_fields,
                for_correction,
            ),
            kind if kind.is_value_static_type() => {
                self.value_static_type_witness_to_dart(this, buffer, witness_fields, for_correction)
            }
            _ => self.type_based_witness_to_dart(
                this_type,
                this,
                buffer,
                witness,
                witness_fields,
                for_correction,
            ),
        }
    }

    /// `TypeBasedStaticType.witnessToDart`.
    fn type_based_witness_to_dart(
        &self,
        this_type: StaticType,
        this: &CacheTypeBased<TO, EO, SO>,
        buffer: &mut dyn DartTemplateBuffer,
        witness: &PropertyWitness,
        witness_fields: &IndexMap<Key, &PropertyWitness>,
        for_correction: bool,
    ) {
        if !self.type_operations.has_simple_name(&this.type_) {
            buffer.write(&self.type_based_name(this));
            buffer.write(" _");

            // If we have restrictions on the record type we create an and
            // pattern.
            let mut additional_start = " && Object(";
            let mut additional_end = "";
            let mut comma = "";
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
        } else {
            st::base_witness_to_dart(
                self,
                this_type,
                buffer,
                witness,
                witness_fields,
                for_correction,
            );
        }
    }

    /// `TypeBasedStaticType.typeToDart`.
    pub(crate) fn type_based_type_to_dart(
        &self,
        this: &CacheTypeBased<TO, EO, SO>,
        buffer: &mut dyn DartTemplateBuffer,
    ) {
        buffer.write_general_type(&this.type_ as &dyn Any, &self.type_based_name(this));
    }

    /// `TypeBasedStaticType.isEnumSubtype`.
    pub(crate) fn type_based_is_enum_subtype(&self, this: &CacheTypeBased<TO, EO, SO>) -> bool {
        self.type_operations.is_enum(&this.type_)
    }

    /// `TypeBasedStaticType.libraryUri`.
    pub(crate) fn type_based_library_uri(
        &self,
        this: &CacheTypeBased<TO, EO, SO>,
    ) -> Option<String> {
        self.type_operations.library_uri(&this.type_)
    }

    /// `TypeBasedStaticType.isPrivate`.
    pub(crate) fn type_based_is_private(&self, this: &CacheTypeBased<TO, EO, SO>) -> bool {
        self.type_based_name(this).starts_with('_')
    }

    // -----------------------------------------------------------------------
    // GeneralValueStaticType / ValueStaticType

    /// `ValueStaticType.valueToDart` and its implementations.
    fn value_static_type_value_to_dart(
        &self,
        this: &CacheTypeBased<TO, EO, SO>,
        buffer: &mut dyn DartTemplateBuffer,
    ) {
        let name = self.type_based_name(this);
        match (&this.kind, &this.restriction) {
            (TypeBasedKind::BoolValue(value), _) => buffer.write_bool_value(*value),
            (TypeBasedKind::GeneralValue, Restriction::Identity(restriction)) => {
                buffer.write_general_constant_value(restriction.identity.as_any(), &name)
            }
            (TypeBasedKind::EnumElement, Restriction::Identity(restriction)) => {
                buffer.write_enum_value(restriction.identity.as_any(), &name)
            }
            _ => unreachable!("Not a ValueStaticType"),
        }
    }

    /// `ValueStaticType.witnessToDart`.
    fn value_static_type_witness_to_dart(
        &self,
        this: &CacheTypeBased<TO, EO, SO>,
        buffer: &mut dyn DartTemplateBuffer,
        witness_fields: &IndexMap<Key, &PropertyWitness>,
        for_correction: bool,
    ) {
        self.value_static_type_value_to_dart(this, buffer);

        // If we have restrictions on the value we create an and pattern.
        let mut additional_start = " && Object(";
        let mut additional_end = "";
        let mut comma = "";
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

    /// Dart `new GeneralValueStaticType<Type, T>(typeOperations, fieldLookup,
    /// type, restriction, name, value)`. The value is the identity of
    /// [restriction].
    pub(crate) fn new_general_value_static_type(
        &self,
        type_: TO::Type,
        restriction: IdentityRestriction,
        name: String,
    ) -> StaticType {
        self.new_type_based_static_type(TypeBasedStaticType::new(
            type_,
            false,
            Restriction::Identity(restriction),
            Some(name),
            TypeBasedKind::GeneralValue,
        ))
    }
}
