// Dart source: pkg/_fe_analyzer_shared/lib/src/exhaustiveness/static_type.dart

//! A static type in the type system of the exhaustiveness checker.
//!
//! # Identity
//!
//! A Dart `StaticType` is an object. Here it is the `Copy` handle
//! [`StaticType`], an id into the arena that owns the type data (the
//! `ExhaustivenessCache`, through the trait [`StaticTypeArena`]). Every Dart
//! `new` of a static type allocates a new arena node with a new
//! [`StaticTypeId`].
//!
//! Dart `==` on static types is structural for some classes
//! (`NullableStaticType`, `WrappedStaticType`, `TypeBasedStaticType`) and
//! identity for others. The arena hash-conses an "equality key" for every node
//! and records the id of the first node with an equal key as the node's
//! `eq_id`. [`StaticType`] implements [`PartialEq`] and [`Hash`] through
//! `eq_id`, so `==`, `Set<StaticType>` and `Map<StaticType, ...>` behave as in
//! Dart, while each node keeps its own data (for example the name of a list
//! pattern type, which is not part of Dart `==`).
//!
//! # Virtual methods
//!
//! The abstract members of the Dart class `StaticType` are the methods of
//! [`StaticTypeArena`]; the arena dispatches on the node kind. The bodies of
//! the Dart classes in this file (`_BaseStaticType`, `_NonNullableObject`,
//! `_NeverType`, `_NullType`, `NullableStaticType`, `NonNullableStaticType`,
//! `WrappedStaticType`) are the free functions in this module.

use std::cell::OnceCell;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};

use super::dart_template_buffer::DartTemplateBuffer;
use super::key::Key;
use super::space::Space;
use super::witness::PropertyWitness;

/// The index of a static type node in the arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StaticTypeId(pub u32);

/// A static type in the type system.
///
/// A handle into a [`StaticTypeArena`]. Equality and hashing follow Dart `==`
/// (see the module documentation).
#[derive(Clone, Copy)]
pub struct StaticType {
    id: StaticTypeId,
    eq_id: StaticTypeId,
}

impl StaticType {
    /// Built-in top type that all types are a subtype of.
    pub const NON_NULLABLE_OBJECT: StaticType = StaticType::from_ids(0, 0);

    /// Built-in `Never` type.
    pub const NEVER_TYPE: StaticType = StaticType::from_ids(1, 1);

    /// Built-in top type that all types are a subtype of.
    pub const NULLABLE_OBJECT: StaticType = StaticType::from_ids(2, 2);

    /// Built-in `Null` type.
    pub const NULL_TYPE: StaticType = StaticType::from_ids(3, 3);

    /// The number of the built-in static types, which every arena allocates
    /// first, in the order of the ids above.
    pub const BUILT_IN_COUNT: u32 = 4;

    const fn from_ids(id: u32, eq_id: u32) -> StaticType {
        StaticType {
            id: StaticTypeId(id),
            eq_id: StaticTypeId(eq_id),
        }
    }

    /// Creates the handle of the arena node [id] whose Dart `==` class is
    /// represented by the node [eq_id].
    pub fn new(id: StaticTypeId, eq_id: StaticTypeId) -> StaticType {
        StaticType { id, eq_id }
    }

    /// The arena node of this static type (Dart object identity).
    pub fn id(self) -> StaticTypeId {
        self.id
    }

    /// The first arena node that is `==` to this static type.
    pub fn eq_id(self) -> StaticTypeId {
        self.eq_id
    }

    /// Dart `identical`.
    pub fn is_identical(self, other: StaticType) -> bool {
        self.id == other.id
    }

    // The members below forward to the arena, so that client code reads like
    // the Dart code (`type.isSubtypeOf(other)` is
    // `type.is_subtype_of(types, other)`).

    pub fn fields(self, types: &dyn StaticTypeArena) -> Rc<IndexMap<Key, StaticType>> {
        types.fields(self)
    }

    pub fn get_property_type(
        self,
        types: &dyn StaticTypeArena,
        field_lookup: &dyn ObjectPropertyLookup,
        key: &Key,
    ) -> Option<StaticType> {
        types.get_property_type(self, field_lookup, key)
    }

    pub fn get_additional_property_type(
        self,
        types: &dyn StaticTypeArena,
        key: &Key,
    ) -> Option<StaticType> {
        types.get_additional_property_type(self, key)
    }

    pub fn is_subtype_of(self, types: &dyn StaticTypeArena, other: StaticType) -> bool {
        types.is_subtype_of(self, other)
    }

    pub fn is_sealed(self, types: &dyn StaticTypeArena) -> bool {
        types.is_sealed(self)
    }

    pub fn is_record(self, types: &dyn StaticTypeArena) -> bool {
        types.is_record(self)
    }

    pub fn name(self, types: &dyn StaticTypeArena) -> String {
        types.name(self)
    }

    pub fn nullable(self, types: &dyn StaticTypeArena) -> StaticType {
        types.nullable(self)
    }

    pub fn non_nullable(self, types: &dyn StaticTypeArena) -> StaticType {
        types.non_nullable(self)
    }

    pub fn get_subtypes(
        self,
        types: &dyn StaticTypeArena,
        keys_of_interest: &IndexSet<Key>,
    ) -> Vec<StaticType> {
        types.get_subtypes(self, keys_of_interest)
    }
}

impl PartialEq for StaticType {
    fn eq(&self, other: &StaticType) -> bool {
        self.eq_id == other.eq_id
    }
}

impl Eq for StaticType {}

impl Hash for StaticType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.eq_id.hash(state);
    }
}

impl fmt::Debug for StaticType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.id == self.eq_id {
            write!(f, "StaticType#{}", self.id.0)
        } else {
            write!(f, "StaticType#{}(=={})", self.id.0, self.eq_id.0)
        }
    }
}

/// The interface of the Dart class `StaticType`, implemented by the arena that
/// owns the static types.
///
/// Every method takes the receiver static type as its first argument.
pub trait StaticTypeArena {
    /// The static types of the fields this type exposes for record
    /// destructuring.
    ///
    /// Includes inherited fields.
    fn fields(&self, this: StaticType) -> Rc<IndexMap<Key, StaticType>>;

    /// Returns the static type for the [key] in this static type, or `None` if
    /// no such key exists.
    ///
    /// This is used to support implicit on the constant [StaticType]s
    /// `nullableObject`, `nonNullableObject`, `nullType` and `neverType`.
    fn get_property_type(
        &self,
        this: StaticType,
        field_lookup: &dyn ObjectPropertyLookup,
        key: &Key,
    ) -> Option<StaticType>;

    /// Returns the static type for the [key] in this static type, or `None` if
    /// no such key exists.
    ///
    /// This is used to model keys in map patterns, and indices and ranges in
    /// list patterns.
    fn get_additional_property_type(&self, this: StaticType, key: &Key) -> Option<StaticType>;

    /// Returns `true` if this static type is a subtype of [other], taking the
    /// nullability and subtyping relation into account.
    fn is_subtype_of(&self, this: StaticType, other: StaticType) -> bool;

    /// Whether this type is sealed. A sealed type is implicitly abstract and
    /// has a closed set of known subtypes. This means that every instance of
    /// the type must be an instance of one of those subtypes. Conversely, if an
    /// instance is *not* an instance of one of those subtypes, that it must not
    /// be an instance of this type.
    ///
    /// Note that subtypes of a sealed type do not themselves have to be sealed.
    /// Consider:
    ///
    /// ```text
    ///      (A)
    ///      / \
    ///     B   C
    /// ```
    ///
    /// Here, A is sealed and B and C are not. There may be many unknown
    /// subclasses of B and C, or classes implementing their interfaces. That
    /// doesn't interfere with exhaustiveness checking because it's still the
    /// case that any instance of A must be either a B or C *or some subtype of
    /// one of those two types*.
    fn is_sealed(&self, this: StaticType) -> bool;

    /// Returns `true` if this is a private type.
    ///
    /// This is used to determine whether a missing pattern part is not
    /// accessible due to being private.
    fn is_private(&self, this: StaticType) -> bool;

    /// Returns `true` if this is a subtype of `Enum`.
    ///
    /// This is used along with [is_private] to determine whether we should
    /// suggest enum values as missing pattern parts or only a default case.
    fn is_enum_subtype(&self, this: StaticType) -> bool;

    /// Returns the library URI for this type, or `None`.
    fn library_uri(&self, this: StaticType) -> Option<String>;

    /// Returns `true` if this is a record type.
    ///
    /// This is only used for print the type as part of a `Witness`.
    fn is_record(&self, this: StaticType) -> bool;

    /// Return `true` if this type is implicitly nullable.
    ///
    /// This is used to omit the '?' for the [name] in the
    /// `NullableStaticType`.
    fn is_implicitly_nullable(&self, this: StaticType) -> bool;

    /// Returns the name of this static type.
    ///
    /// This is used for printing `Space`s. It is also the Dart `toString`.
    fn name(&self, this: StaticType) -> String;

    /// Writes the name of this static type to [buffer].
    fn type_to_dart(&self, this: StaticType, buffer: &mut dyn DartTemplateBuffer);

    /// Returns the nullable static type corresponding to this type.
    fn nullable(&self, this: StaticType) -> StaticType;

    /// Returns the non-nullable static type corresponding to this type.
    fn non_nullable(&self, this: StaticType) -> StaticType;

    /// The immediate subtypes of this type.
    ///
    /// The [keys_of_interest] of interest are the keys used in one of the case
    /// rows. This is used to select how a `List` type should be divided into
    /// subtypes that should be used for testing the exhaustiveness of a list.
    fn get_subtypes(&self, this: StaticType, keys_of_interest: &IndexSet<Key>) -> Vec<StaticType>;

    /// Returns a textual representation of a single space consisting of this
    /// type and the provided [space_properties] and
    /// [additional_space_properties].
    fn space_to_text(
        &self,
        this: StaticType,
        space_properties: &IndexMap<Key, Space>,
        additional_space_properties: &IndexMap<Key, Space>,
    ) -> String;

    /// Write this [witness] with the [witness_fields] as a pattern into
    /// [buffer] using this [StaticType] to determine the syntax.
    ///
    /// If [for_correction] is true, [witness_fields] that fully cover their
    /// static type are omitted if possible.
    fn witness_to_dart(
        &self,
        this: StaticType,
        buffer: &mut dyn DartTemplateBuffer,
        witness: &PropertyWitness,
        witness_fields: &IndexMap<Key, &PropertyWitness>,
        for_correction: bool,
    );

    /// Dart `this is NullableStaticType`: returns the `underlying` type if
    /// [this] is a `NullableStaticType` (this includes `Object?` and `Null`).
    fn as_nullable_static_type(&self, this: StaticType) -> Option<StaticType>;

    /// Dart `this is WrappedStaticType`: returns the `wrappedType` and the
    /// `impliedType` if [this] is a `WrappedStaticType`.
    fn as_wrapped_static_type(&self, this: StaticType) -> Option<(StaticType, StaticType)>;

    /// Dart `new WrappedStaticType(wrappedType, impliedType)`.
    fn new_wrapped_static_type(
        &self,
        wrapped_type: StaticType,
        implied_type: StaticType,
    ) -> StaticType;
}

/// Interface for accessing the members defined on `Object`.
pub trait ObjectPropertyLookup {
    /// Returns the [StaticType] for the member with the given [key] defined on
    /// `Object`, or `None` none exists.
    fn get_object_field_type(&self, key: &Key) -> Option<StaticType>;

    /// The arena that owns the static types. (dartr: in Dart the static types
    /// are objects and need no arena.)
    fn static_types(&self) -> &dyn StaticTypeArena;
}

/// The node of a static type in an arena. The type parameter [X] is the data
/// of the `NonNullableStaticType` subclasses (the `TypeBasedStaticType`
/// family in `types.dart`).
pub enum StaticTypeNode<X> {
    /// `_NonNullableObject`.
    NonNullableObject,

    /// `_NeverType`.
    Never,

    /// `_NullType` (a `NullableStaticType` with the underlying type `Never`).
    Null,

    /// `NullableStaticType`.
    Nullable(NullableStaticType),

    /// `WrappedStaticType`.
    Wrapped(WrappedStaticType),

    /// A subclass of `NonNullableStaticType`.
    NonNullable(X),
}

impl<X> StaticTypeNode<X> {
    /// Returns the data of a `NonNullableStaticType` subclass.
    pub fn non_nullable_data(&self) -> Option<&X> {
        match self {
            StaticTypeNode::NonNullable(data) => Some(data),
            _ => None,
        }
    }
}

/// The data of the Dart class `NullableStaticType`.
pub struct NullableStaticType {
    pub underlying: StaticType,
}

/// Static type the behaves like [wrapped_type] but is also a subtype of
/// [implied_type].
pub struct WrappedStaticType {
    pub wrapped_type: StaticType,
    pub implied_type: StaticType,
    pub(crate) non_nullable: OnceCell<StaticType>,
    pub(crate) nullable: OnceCell<StaticType>,
}

impl WrappedStaticType {
    pub fn new(wrapped_type: StaticType, implied_type: StaticType) -> WrappedStaticType {
        WrappedStaticType {
            wrapped_type,
            implied_type,
            non_nullable: OnceCell::new(),
            nullable: OnceCell::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// _ObjectFieldMixin

/// `_ObjectFieldMixin.getPropertyType`.
pub(crate) fn object_field_mixin_get_property_type(
    types: &dyn StaticTypeArena,
    this: StaticType,
    field_lookup: &dyn ObjectPropertyLookup,
    key: &Key,
) -> Option<StaticType> {
    types
        .fields(this)
        .get(key)
        .copied()
        .or_else(|| field_lookup.get_object_field_type(key))
}

// ---------------------------------------------------------------------------
// _BaseStaticType

/// `_BaseStaticType.getPropertyType`.
pub(crate) fn base_get_property_type(
    types: &dyn StaticTypeArena,
    this: StaticType,
    key: &Key,
) -> Option<StaticType> {
    types.fields(this).get(key).copied()
}

/// `_BaseStaticType.isSubtypeOf`.
pub(crate) fn base_is_subtype_of(
    types: &dyn StaticTypeArena,
    this: StaticType,
    other: StaticType,
) -> bool {
    if this == other {
        return true;
    }

    // All types are subtypes of Object?.
    if other == StaticType::NULLABLE_OBJECT {
        return true;
    }

    if let Some((wrapped_type, implied_type)) = types.as_wrapped_static_type(other) {
        return types.is_subtype_of(this, wrapped_type) && types.is_subtype_of(this, implied_type);
    }

    false
}

/// `_BaseStaticType.spaceToText`.
pub(crate) fn base_space_to_text(
    types: &dyn StaticTypeArena,
    this: StaticType,
    space_properties: &IndexMap<Key, Space>,
    additional_space_properties: &IndexMap<Key, Space>,
) -> String {
    debug_assert!(
        additional_space_properties.is_empty(),
        "Additional fields not supported in {}.",
        types.name(this)
    );
    if this == StaticType::NULLABLE_OBJECT && space_properties.is_empty() {
        return "()".to_string();
    }
    if this == StaticType::NEVER_TYPE && space_properties.is_empty() {
        return "∅".to_string();
    }

    // If there are no fields, just show the type.
    if space_properties.is_empty() {
        return types.name(this);
    }

    let mut buffer = String::new();
    buffer.push_str(&types.name(this));

    buffer.push('(');
    let mut first = true;

    for (key, space) in space_properties {
        if !first {
            buffer.push_str(", ");
        }
        if let Key::Extension(key) = key {
            buffer.push_str(&format!(
                "{}.{}: {} ({})",
                types.name(key.receiver_type),
                key.name,
                space.to_text(types),
                types.name(key.type_)
            ));
        } else {
            buffer.push_str(&format!("{}: {}", key.name(), space.to_text(types)));
        }
        first = false;
    }

    buffer.push(')');
    buffer
}

/// `_BaseStaticType.witnessToDart`.
pub(crate) fn base_witness_to_dart(
    types: &dyn StaticTypeArena,
    this: StaticType,
    buffer: &mut dyn DartTemplateBuffer,
    _witness: &PropertyWitness,
    witness_fields: &IndexMap<Key, &PropertyWitness>,
    for_correction: bool,
) {
    if this == StaticType::NULLABLE_OBJECT && witness_fields.is_empty() {
        buffer.write("_");
    } else if this == StaticType::NULL_TYPE && witness_fields.is_empty() {
        buffer.write("null");
    } else {
        types.type_to_dart(this, buffer);
        buffer.write("(");
        let mut comma = "";
        for (key, witness) in witness_fields {
            buffer.write(comma);
            comma = ", ";
            buffer.write(&key.name());
            buffer.write(": ");
            witness.witness_to_dart(types, buffer, for_correction);
        }
        buffer.write(")");
    }
}

// ---------------------------------------------------------------------------
// _NonNullableObject

/// `_NonNullableObject.isSubtypeOf`.
pub(crate) fn non_nullable_object_is_subtype_of(
    types: &dyn StaticTypeArena,
    this: StaticType,
    other: StaticType,
) -> bool {
    // Object? is a subtype of itself and Object?.
    base_is_subtype_of(types, this, other) || other == StaticType::NULLABLE_OBJECT
}

/// `_NonNullableObject.name`.
pub(crate) const NON_NULLABLE_OBJECT_NAME: &str = "Object";

// ---------------------------------------------------------------------------
// _NeverType

/// `_NeverType.isSubtypeOf`.
pub(crate) fn never_type_is_subtype_of(_other: StaticType) -> bool {
    // Never is a subtype of all types.
    true
}

/// `_NeverType.name`.
pub(crate) const NEVER_TYPE_NAME: &str = "Never";

// ---------------------------------------------------------------------------
// _NullType

/// `_NullType.name`.
pub(crate) const NULL_TYPE_NAME: &str = "Null";

// ---------------------------------------------------------------------------
// NullableStaticType

/// `NullableStaticType.getSubtypes`.
pub(crate) fn nullable_get_subtypes(underlying: StaticType) -> Vec<StaticType> {
    vec![underlying, StaticType::NULL_TYPE]
}

/// `NullableStaticType.isSubtypeOf`.
pub(crate) fn nullable_is_subtype_of(
    types: &dyn StaticTypeArena,
    this: StaticType,
    underlying: StaticType,
    other: StaticType,
) -> bool {
    if base_is_subtype_of(types, this, other) {
        return true;
    }
    // A nullable type is a subtype if the underlying type and Null both are.
    this == other
        || types
            .as_nullable_static_type(other)
            .is_some_and(|other_underlying| types.is_subtype_of(underlying, other_underlying))
}

/// `NullableStaticType.name`.
pub(crate) fn nullable_name(types: &dyn StaticTypeArena, underlying: StaticType) -> String {
    if types.is_implicitly_nullable(underlying) {
        types.name(underlying)
    } else {
        format!("{}?", types.name(underlying))
    }
}

/// `NullableStaticType.typeToDart`.
pub(crate) fn nullable_type_to_dart(
    types: &dyn StaticTypeArena,
    underlying: StaticType,
    buffer: &mut dyn DartTemplateBuffer,
) {
    types.type_to_dart(underlying, buffer);
    if !types.is_implicitly_nullable(underlying) {
        buffer.write("?");
    }
}

// ---------------------------------------------------------------------------
// NonNullableStaticType

/// `NonNullableStaticType.isSubtypeOf`. [is_subtype_of_internal] is the
/// abstract `isSubtypeOfInternal`.
pub(crate) fn non_nullable_is_subtype_of(
    types: &dyn StaticTypeArena,
    this: StaticType,
    other: StaticType,
    is_subtype_of_internal: impl FnOnce(StaticType) -> bool,
) -> bool {
    if base_is_subtype_of(types, this, other) {
        return true;
    }

    // All non-nullable types are subtypes of Object.
    if other == StaticType::NON_NULLABLE_OBJECT {
        return true;
    }

    // A non-nullable type is a subtype of the underlying type of a nullable
    // type.
    if let Some(underlying) = types.as_nullable_static_type(other) {
        return types.is_subtype_of(this, underlying);
    }

    is_subtype_of_internal(other)
}

// ---------------------------------------------------------------------------
// WrappedStaticType

/// `WrappedStaticType.getSubtypes`.
pub(crate) fn wrapped_get_subtypes(
    types: &dyn StaticTypeArena,
    wrapped_type: StaticType,
    implied_type: StaticType,
    keys_of_interest: &IndexSet<Key>,
) -> Vec<StaticType> {
    if let (Some(wrapped_underlying), Some(implied_underlying)) = (
        types.as_nullable_static_type(wrapped_type),
        types.as_nullable_static_type(implied_type),
    ) {
        // With nullable types we need to avoid carrying the nullable implied
        // type into the non-nullable subtype since it otherwise wouldn't allow
        // for matching the non-nullable aspect of the wrapped type with the
        // non-nullable implied type.
        //
        // For instance
        //
        //     method<O>(O? object) => switch (object) {
        //         O object => 0,
        //         null => 1,
        //       };
        //
        // Here the static type of `O?` is `WrappedStaticType(Object?, O?)`
        // which allows for matching both by the bound `Object?` and the exact
        // type variable type `O?`. If we split this into the subtypes
        // `WrappedStaticType(Object, O?)` and `WrappedStaticType(null, O?)`
        // then we miss that `O object` covers the non-nullable aspect, since
        // `O` is neither a super type of `Object` nor `O?`.
        return vec![
            types.new_wrapped_static_type(wrapped_underlying, implied_underlying),
            StaticType::NULL_TYPE,
        ];
    }
    types
        .get_subtypes(wrapped_type, keys_of_interest)
        .into_iter()
        .map(|e| types.new_wrapped_static_type(e, implied_type))
        .collect()
}

/// `WrappedStaticType.isSubtypeOf`.
pub(crate) fn wrapped_is_subtype_of(
    types: &dyn StaticTypeArena,
    this: StaticType,
    wrapped_type: StaticType,
    implied_type: StaticType,
    other: StaticType,
) -> bool {
    if base_is_subtype_of(types, this, other) {
        return true;
    }

    types.is_subtype_of(wrapped_type, other) || types.is_subtype_of(implied_type, other)
}

/// `WrappedStaticType.nonNullable`.
pub(crate) fn wrapped_non_nullable(
    types: &dyn StaticTypeArena,
    this: StaticType,
    wrapped: &WrappedStaticType,
) -> StaticType {
    if let Some(result) = wrapped.non_nullable.get() {
        return *result;
    }
    let wrapped_type = wrapped.wrapped_type;
    let implied_type = wrapped.implied_type;
    let result = if types.non_nullable(wrapped_type) == wrapped_type
        && types.non_nullable(implied_type) == implied_type
    {
        this
    } else {
        types.new_wrapped_static_type(
            types.non_nullable(wrapped_type),
            types.non_nullable(implied_type),
        )
    };
    *wrapped.non_nullable.get_or_init(|| result)
}

/// `WrappedStaticType.nullable`.
pub(crate) fn wrapped_nullable(
    types: &dyn StaticTypeArena,
    this: StaticType,
    wrapped: &WrappedStaticType,
) -> StaticType {
    if let Some(result) = wrapped.nullable.get() {
        return *result;
    }
    let wrapped_type = wrapped.wrapped_type;
    let implied_type = wrapped.implied_type;
    let result = if types.nullable(wrapped_type) == wrapped_type
        && types.nullable(implied_type) == implied_type
    {
        this
    } else {
        types.new_wrapped_static_type(types.nullable(wrapped_type), types.nullable(implied_type))
    };
    *wrapped.nullable.get_or_init(|| result)
}
