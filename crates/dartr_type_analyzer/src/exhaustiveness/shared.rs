// Dart source: pkg/_fe_analyzer_shared/lib/src/exhaustiveness/shared.dart

use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use indexmap::{IndexMap, IndexSet};

use super::dart_template_buffer::DartTemplateBuffer;
use super::exhaustive::expand_sealed_subtypes;
use super::key::{ExtensionKey, Identity, Key, MapKey};
use super::path::Path;
use super::space::{SingleSpace, Space};
use super::static_type::{
    self as st, NullableStaticType, ObjectPropertyLookup, StaticType, StaticTypeArena,
    StaticTypeId, StaticTypeNode, WrappedStaticType,
};
use super::types::{
    CacheTypeBased, EnumInfo, EnumOperations, IdentityRestriction, ListTypeRestriction,
    MapTypeRestriction, RestrictionEqKey, SealedClassInfo, SealedClassOperations, TypeBasedKind,
    TypeBasedStaticType,
};
use super::witness::PropertyWitness;

/// Interface implemented by analyze/CFE to support type operations need for
/// the shared [StaticType]s.
pub trait TypeOperations {
    type Type: Clone + Eq + Hash + Debug + 'static;

    /// Returns the type for `Object?`.
    fn nullable_object_type(&self) -> Self::Type;

    /// Returns the type for the non-nullable `Object`.
    fn non_nullable_object_type(&self) -> Self::Type;

    /// Returns `true` if [s] is a subtype of [t].
    fn is_subtype_of(&self, s: &Self::Type, t: &Self::Type) -> bool;

    /// Returns a type that overapproximates the possible values of [type_] by
    /// replacing all type variables with the default types.
    fn overapproximate(&self, type_: &Self::Type) -> Self::Type;

    /// Returns `true` if [type_] is a nullable type.
    fn is_nullable(&self, type_: &Self::Type) -> bool;

    /// Returns `true` if [type_] is a potentially nullable type.
    fn is_potentially_nullable(&self, type_: &Self::Type) -> bool;

    /// Returns the non-nullable type corresponding to [type_]. For instance
    /// `Foo` for `Foo?`. If [type_] is already non-nullable, it itself is
    /// returned.
    fn get_non_nullable(&self, type_: &Self::Type) -> Self::Type;

    /// Returns `true` if [type_] is the `Null` type.
    fn is_null_type(&self, type_: &Self::Type) -> bool;

    /// Returns `true` if [type_] is the `Never` type.
    fn is_never_type(&self, type_: &Self::Type) -> bool;

    /// Returns `true` if [type_] is the `Object?` type.
    fn is_nullable_object(&self, type_: &Self::Type) -> bool;

    /// Returns `true` if [type_] is the `Object` type.
    fn is_non_nullable_object(&self, type_: &Self::Type) -> bool;

    /// Returns `true` if [type_] is the `dynamic` type.
    fn is_dynamic(&self, type_: &Self::Type) -> bool;

    /// Returns `true` if [type_] is the `bool` type.
    fn is_bool_type(&self, type_: &Self::Type) -> bool;

    /// Returns the `bool` type.
    fn bool_type(&self) -> Self::Type;

    /// Returns `true` if [type_] is a record type.
    fn is_record_type(&self, type_: &Self::Type) -> bool;

    /// Returns `true` if [type_] is a generic interface type.
    fn is_generic(&self, type_: &Self::Type) -> bool;

    /// Returns the type `T` if [type_] is `FutureOr<T>`. Returns `None`
    /// otherwise.
    fn get_future_or_type_argument(&self, type_: &Self::Type) -> Option<Self::Type>;

    /// Returns the non-nullable type `Future<T>` for [type_] `T`.
    fn instantiate_future(&self, type_: &Self::Type) -> Self::Type;

    /// Returns a map of the field names and corresponding types available on
    /// [type_]. For an interface type, these are the fields and getters, and
    /// for record types these are the record fields.
    fn get_field_types(&self, type_: &Self::Type) -> IndexMap<Key, Self::Type>;

    /// Returns the value type `V` if [type_] implements `Map<K, V>` or `None`
    /// otherwise.
    fn get_map_value_type(&self, type_: &Self::Type) -> Option<Self::Type>;

    /// Returns the element type `E` if [type_] implements `List<E>` or `None`
    /// otherwise.
    fn get_list_element_type(&self, type_: &Self::Type) -> Option<Self::Type>;

    /// Returns the list type `List<E>` if [type_] implements `List<E>` or
    /// `None` otherwise.
    fn get_list_type(&self, type_: &Self::Type) -> Option<Self::Type>;

    /// Returns the extension type erasure of [type_].
    ///
    /// This is [type_] in which all occurrences of extension types have been
    /// replaced with their representation type.
    fn get_extension_type_erasure(&self, type_: &Self::Type) -> Self::Type;

    /// Returns a human-readable representation of the [type_].
    fn type_to_string(&self, type_: &Self::Type) -> String;

    /// Returns `true` if [type_] has a simple name that can be used as the type
    /// of an object pattern.
    fn has_simple_name(&self, type_: &Self::Type) -> bool;

    /// Returns the bound of [type_] if is a type variable or a promoted type
    /// variable. Otherwise returns `None`.
    fn get_type_variable_bound(&self, type_: &Self::Type) -> Option<Self::Type>;

    /// Returns `true` if [type_] is an enum type.
    fn is_enum(&self, type_: &Self::Type) -> bool;

    /// Returns the library URI for [type_].
    fn library_uri(&self, type_: &Self::Type) -> Option<String>;
}

/// Interface for looking up fields and their corresponding [StaticType]s of
/// a given type.
pub trait FieldLookup {
    type Type;

    /// Returns a map of the field names and corresponding [StaticType]s
    /// available on [type_]. For an interface type, these are the fields and
    /// getters, and for record types these are the record fields.
    fn get_field_types(&self, type_: &Self::Type) -> Rc<IndexMap<Key, StaticType>>;

    fn get_additional_field_type(&self, type_: &Self::Type, key: &Key) -> Option<StaticType>;
}

/// The arena node of an [ExhaustivenessCache].
pub(crate) type CacheNode<TO, EO, SO> = StaticTypeNode<CacheTypeBased<TO, EO, SO>>;

/// The Dart `==` of a static type node, used to compute the `eq_id` of a
/// [StaticType]. `_NonNullableObject` and `_NeverType` have object identity
/// and need no key.
#[derive(PartialEq, Eq, Hash)]
enum StaticTypeEqKey<T: Eq + Hash> {
    /// `NullableStaticType.==` (also `_NullType`).
    Nullable(StaticTypeId),

    /// `WrappedStaticType.==`.
    Wrapped(StaticTypeId, StaticTypeId),

    /// `TypeBasedStaticType.==`.
    TypeBased(T, RestrictionEqKey<T>),
}

/// The Dart `new Object()` used as unique value by
/// [ExhaustivenessCache::get_unknown_static_type].
#[derive(PartialEq, Eq, Hash, Debug)]
struct UnknownIdentity(u64);

static NEXT_UNKNOWN_IDENTITY: AtomicU64 = AtomicU64::new(0);

/// Cache used for computing [StaticType]s used for exhaustiveness checking.
///
/// This implementation is shared between analyzer and CFE, and implemented
/// using the analyzer/CFE implementations of [TypeOperations],
/// [EnumOperations], and [SealedClassOperations].
///
/// The Dart type parameters `Type`, `Class`, `EnumClass`, `EnumElement` and
/// `EnumElementValue` are associated types of the operations.
///
/// The cache is also the arena that owns the [StaticType]s it creates (see
/// [StaticTypeArena]). It uses interior mutability, so all methods take
/// `&self`, as the Dart objects are shared and lazily initialized.
pub struct ExhaustivenessCache<TO, EO, SO>
where
    TO: TypeOperations,
    EO: EnumOperations<Type = TO::Type>,
    SO: SealedClassOperations<Type = TO::Type>,
{
    pub type_operations: TO,
    pub enum_operations: EO,
    pub(crate) sealed_class_operations: SO,

    /// Cache for [EnumInfo] for enum classes.
    enum_info: RefCell<IndexMap<EO::EnumClass, Rc<EnumInfo<EO>>>>,

    /// Cache for [SealedClassInfo] for sealed classes.
    sealed_class_info: RefCell<IndexMap<SO::Class, Rc<SealedClassInfo<SO>>>>,

    /// Cache for unique [StaticType]s.
    unique_type_map: RefCell<IndexMap<Identity, StaticType>>,

    /// Cache for the [StaticType] for `bool`.
    bool_static_type: OnceCell<StaticType>,

    /// Cache for [StaticType]s for fields available on a `Type`.
    field_cache: RefCell<HashMap<TO::Type, Rc<IndexMap<Key, StaticType>>>>,

    /// The arena of static types, indexed by [StaticTypeId].
    nodes: RefCell<Vec<Rc<CacheNode<TO, EO, SO>>>>,

    /// The first node for each Dart `==` class of static types.
    eq_ids: RefCell<HashMap<StaticTypeEqKey<TO::Type>, StaticTypeId>>,
}

impl<TO, EO, SO> ExhaustivenessCache<TO, EO, SO>
where
    TO: TypeOperations,
    EO: EnumOperations<Type = TO::Type>,
    SO: SealedClassOperations<Type = TO::Type>,
{
    pub fn new(type_operations: TO, enum_operations: EO, sealed_class_operations: SO) -> Self {
        let cache = ExhaustivenessCache {
            type_operations,
            enum_operations,
            sealed_class_operations,
            enum_info: RefCell::new(IndexMap::new()),
            sealed_class_info: RefCell::new(IndexMap::new()),
            unique_type_map: RefCell::new(IndexMap::new()),
            bool_static_type: OnceCell::new(),
            field_cache: RefCell::new(HashMap::new()),
            nodes: RefCell::new(vec![]),
            eq_ids: RefCell::new(HashMap::new()),
        };
        // The built-in static types, in the order of their ids.
        let non_nullable_object = cache.alloc(StaticTypeNode::NonNullableObject, None);
        debug_assert!(non_nullable_object.is_identical(StaticType::NON_NULLABLE_OBJECT));
        let never_type = cache.alloc(StaticTypeNode::Never, None);
        debug_assert!(never_type.is_identical(StaticType::NEVER_TYPE));
        let nullable_object = cache.new_nullable_static_type(StaticType::NON_NULLABLE_OBJECT);
        debug_assert!(nullable_object.is_identical(StaticType::NULLABLE_OBJECT));
        let null_type = cache.alloc(
            StaticTypeNode::Null,
            Some(StaticTypeEqKey::Nullable(StaticType::NEVER_TYPE.eq_id())),
        );
        debug_assert!(null_type.is_identical(StaticType::NULL_TYPE));
        cache
    }

    // -----------------------------------------------------------------------
    // Arena

    /// Allocates a new node. [eq_key] is the Dart `==` of the node, or `None`
    /// for object identity.
    fn alloc(
        &self,
        node: CacheNode<TO, EO, SO>,
        eq_key: Option<StaticTypeEqKey<TO::Type>>,
    ) -> StaticType {
        let id = {
            let mut nodes = self.nodes.borrow_mut();
            let id = StaticTypeId(u32::try_from(nodes.len()).expect("Too many static types"));
            nodes.push(Rc::new(node));
            id
        };
        let eq_id = match eq_key {
            None => id,
            Some(eq_key) => *self.eq_ids.borrow_mut().entry(eq_key).or_insert(id),
        };
        StaticType::new(id, eq_id)
    }

    /// Returns the arena node of [type_].
    pub(crate) fn node(&self, type_: StaticType) -> Rc<CacheNode<TO, EO, SO>> {
        self.nodes.borrow()[type_.id().0 as usize].clone()
    }

    /// The number of static types created by this cache.
    pub fn static_type_count(&self) -> usize {
        self.nodes.borrow().len()
    }

    /// Dart `new NullableStaticType(underlying)`.
    fn new_nullable_static_type(&self, underlying: StaticType) -> StaticType {
        self.alloc(
            StaticTypeNode::Nullable(NullableStaticType { underlying }),
            Some(StaticTypeEqKey::Nullable(underlying.eq_id())),
        )
    }

    /// Allocates a node of the `TypeBasedStaticType` family.
    pub(crate) fn new_type_based_static_type(
        &self,
        node: CacheTypeBased<TO, EO, SO>,
    ) -> StaticType {
        let eq_key = StaticTypeEqKey::TypeBased(node.type_.clone(), node.restriction.eq_key());
        self.alloc(StaticTypeNode::NonNullable(node), Some(eq_key))
    }

    /// Dart `new TypeBasedStaticType(typeOperations, fieldLookup, type,
    /// isImplicitlyNullable: ...)`.
    fn new_plain_type_based_static_type(
        &self,
        type_: TO::Type,
        is_implicitly_nullable: bool,
    ) -> StaticType {
        self.new_type_based_static_type(TypeBasedStaticType::new(
            type_,
            is_implicitly_nullable,
            super::types::Restriction::Unrestricted,
            None,
            TypeBasedKind::TypeBased,
        ))
    }

    /// Dart `type.typeForTesting` if [type_] is a `TypeBasedStaticType`.
    pub fn type_for_testing(&self, type_: StaticType) -> Option<TO::Type> {
        self.node(type_)
            .non_nullable_data()
            .map(|data| data.type_for_testing().clone())
    }

    // -----------------------------------------------------------------------
    // ExhaustivenessCache

    /// Returns the [EnumInfo] for [enum_class].
    fn get_enum_info(&self, enum_class: &EO::EnumClass) -> Rc<EnumInfo<EO>> {
        self.enum_info
            .borrow_mut()
            .entry(enum_class.clone())
            .or_insert_with(|| Rc::new(EnumInfo::new(enum_class.clone())))
            .clone()
    }

    /// Returns the [SealedClassInfo] for [sealed_class].
    fn get_sealed_class_info(&self, sealed_class: &SO::Class) -> Rc<SealedClassInfo<SO>> {
        self.sealed_class_info
            .borrow_mut()
            .entry(sealed_class.clone())
            .or_insert_with(|| Rc::new(SealedClassInfo::new(sealed_class.clone())))
            .clone()
    }

    /// `_boolStaticType`.
    fn bool_static_type(&self) -> StaticType {
        if let Some(bool_static_type) = self.bool_static_type.get() {
            return *bool_static_type;
        }
        let bool_static_type = self.new_bool_static_type(self.type_operations.bool_type());
        *self.bool_static_type.get_or_init(|| bool_static_type)
    }

    /// Returns the [StaticType] for the boolean [value].
    pub fn get_bool_value_static_type(&self, value: bool) -> StaticType {
        let bool_static_type = self.bool_static_type();
        let node = self.node(bool_static_type);
        let data = node.non_nullable_data().expect("BoolStaticType");
        let TypeBasedKind::Bool(bool_type) = &data.kind else {
            unreachable!("BoolStaticType");
        };
        if value {
            self.bool_static_type_true_type(data, bool_type)
        } else {
            self.bool_static_type_false_type(data, bool_type)
        }
    }

    /// Returns the [StaticType] for [type_].
    pub fn get_static_type(&self, type_: &TO::Type) -> StaticType {
        let type_ = self.type_operations.get_extension_type_erasure(type_);
        if self.type_operations.is_never_type(&type_) {
            return StaticType::NEVER_TYPE;
        } else if self.type_operations.is_null_type(&type_) {
            return StaticType::NULL_TYPE;
        } else if self.type_operations.is_non_nullable_object(&type_) {
            return StaticType::NON_NULLABLE_OBJECT;
        } else if self.type_operations.is_nullable_object(&type_)
            || self.type_operations.is_dynamic(&type_)
        {
            return StaticType::NULLABLE_OBJECT;
        }

        let mut static_type: StaticType;
        let extract_null = self.type_operations.is_nullable(&type_);
        let mut type_without_null = type_.clone();
        if extract_null {
            // If [type] is nullable, we model the static type by creating the
            // non-nullable equivalent and then add `Null` afterwards.
            //
            // For instance we model `int?` as `int|Null`.
            type_without_null = self.type_operations.get_non_nullable(&type_);
        }
        if self.type_operations.is_bool_type(&type_without_null) {
            static_type = self.bool_static_type();
        } else if self.type_operations.is_record_type(&type_without_null) {
            static_type = self.new_record_static_type(type_without_null);
        } else {
            let future_or_type_argument = self
                .type_operations
                .get_future_or_type_argument(&type_without_null);
            if let Some(future_or_type_argument) = future_or_type_argument {
                let type_argument = self.get_static_type(&future_or_type_argument);
                let future_type = self.get_static_type(
                    &self
                        .type_operations
                        .instantiate_future(&future_or_type_argument),
                );
                let is_implicitly_nullable =
                    self.type_operations.is_nullable(&future_or_type_argument);
                static_type = self.new_future_or_static_type(
                    type_without_null,
                    type_argument,
                    future_type,
                    is_implicitly_nullable,
                );
            } else {
                let enum_class = self.enum_operations.get_enum_class(&type_without_null);
                if let Some(enum_class) = enum_class {
                    static_type = self
                        .new_enum_static_type(type_without_null, self.get_enum_info(&enum_class));
                } else {
                    let sealed_class = self
                        .sealed_class_operations
                        .get_sealed_class(&type_without_null);
                    if let Some(sealed_class) = sealed_class {
                        static_type = self.new_sealed_class_static_type(
                            type_without_null,
                            self.get_sealed_class_info(&sealed_class),
                        );
                    } else {
                        let list_type = self.type_operations.get_list_type(&type_without_null);
                        if list_type.is_some() {
                            static_type = self.new_list_type_static_type(type_without_null);
                        } else {
                            let is_implicitly_nullable =
                                self.type_operations.is_nullable(&type_without_null);
                            static_type = self.new_plain_type_based_static_type(
                                type_without_null,
                                is_implicitly_nullable,
                            );
                            let bound = self.type_operations.get_type_variable_bound(&type_);
                            if let Some(bound) = bound {
                                static_type = self.new_wrapped_static_type(
                                    self.get_static_type(&bound),
                                    static_type,
                                );
                            }
                        }
                    }
                }
            }
        }
        if extract_null {
            // Include the `Null` which extracted from [type] into
            // [typeWithoutNull`.
            static_type = self.nullable(static_type);
        }
        static_type
    }

    /// Returns the [StaticType] for the [enum_element_value] declared by
    /// [enum_class].
    pub fn get_enum_element_static_type(
        &self,
        enum_class: &EO::EnumClass,
        enum_element_value: &EO::EnumElementValue,
    ) -> StaticType {
        let enum_info = self.get_enum_info(enum_class);
        self.enum_info_get_enum_element(&enum_info, enum_element_value)
    }

    /// Creates a new unique [StaticType].
    pub fn get_unknown_static_type(&self) -> StaticType {
        // The unknown static type should be based on the nullable `Object`,
        // since even though it _might_ be `null`, using the nullable `Object`
        // here would mean that it _does_ include `null`, and we need this type
        // to only cover itself.
        self.get_unique_static_type(
            &self.type_operations.non_nullable_object_type(),
            Identity::new(UnknownIdentity(
                NEXT_UNKNOWN_IDENTITY.fetch_add(1, Ordering::Relaxed),
            )),
            "?",
        )
    }

    /// Returns a [StaticType] of the given [type_] with the given
    /// [textual_representation] that unique identifies the [unique_value].
    ///
    /// This is used for constants that are neither bool nor enum values.
    pub fn get_unique_static_type(
        &self,
        type_: &TO::Type,
        unique_value: Identity,
        textual_representation: &str,
    ) -> StaticType {
        let non_nullable = self.type_operations.get_non_nullable(type_);
        let existing = self.unique_type_map.borrow().get(&unique_value).copied();
        let mut static_type = match existing {
            Some(static_type) => static_type,
            None => {
                let static_type = self.new_general_value_static_type(
                    non_nullable,
                    IdentityRestriction::new(unique_value.clone()),
                    textual_representation.to_string(),
                );
                self.unique_type_map
                    .borrow_mut()
                    .insert(unique_value, static_type);
                static_type
            }
        };
        if self.type_operations.is_nullable(type_) {
            static_type = self.nullable(static_type);
        }
        static_type
    }

    /// Returns a [StaticType] of the list [type_] with the given
    /// [restriction] .
    pub fn get_list_static_type(
        &self,
        type_: &TO::Type,
        restriction: ListTypeRestriction<TO::Type>,
    ) -> StaticType {
        let non_nullable = self.type_operations.get_non_nullable(type_);
        let name = restriction.to_string();
        let mut static_type = self.new_list_pattern_static_type(non_nullable, restriction, name);
        if self.type_operations.is_nullable(type_) {
            static_type = self.nullable(static_type);
        }
        static_type
    }

    /// Returns a [StaticType] of the map [type_] with the given
    /// [restriction] .
    pub fn get_map_static_type(
        &self,
        type_: &TO::Type,
        restriction: MapTypeRestriction<TO::Type>,
    ) -> StaticType {
        let non_nullable = self.type_operations.get_non_nullable(type_);
        let name = restriction.to_string();
        let mut static_type = self.new_map_pattern_static_type(non_nullable, restriction, name);
        if self.type_operations.is_nullable(type_) {
            static_type = self.nullable(static_type);
        }
        static_type
    }

    /// `ExhaustivenessCache.getFieldTypes` (the [FieldLookup] method).
    pub fn get_field_types(&self, type_: &TO::Type) -> Rc<IndexMap<Key, StaticType>> {
        if let Some(fields) = self.field_cache.borrow().get(type_) {
            return fields.clone();
        }
        // Dart stores the (still empty) map before it is filled. A recursive
        // request for the same type sees the empty map here.
        self.field_cache
            .borrow_mut()
            .insert(type_.clone(), Rc::new(IndexMap::new()));
        let mut fields: IndexMap<Key, StaticType> = IndexMap::new();
        for (key, field_type) in self.type_operations.get_field_types(type_) {
            fields.insert(key, self.get_static_type(&field_type));
        }
        let fields = Rc::new(fields);
        self.field_cache
            .borrow_mut()
            .insert(type_.clone(), fields.clone());
        fields
    }

    /// `ExhaustivenessCache.getAdditionalFieldType` (the [FieldLookup]
    /// method).
    pub fn get_additional_field_type(&self, type_: &TO::Type, key: &Key) -> Option<StaticType> {
        match key {
            Key::Map(_) => {
                let value_type = self.type_operations.get_map_value_type(type_);
                if let Some(value_type) = value_type {
                    return Some(self.get_static_type(&value_type));
                }
            }
            Key::Head(_) | Key::Tail(_) => {
                let element_type = self.type_operations.get_list_element_type(type_);
                if let Some(element_type) = element_type {
                    return Some(self.get_static_type(&element_type));
                }
            }
            Key::Rest { .. } => {
                let list_type = self.type_operations.get_list_type(type_);
                if let Some(list_type) = list_type {
                    return Some(self.get_static_type(&list_type));
                }
            }
            _ => {
                return self.get_object_field_type(key);
            }
        }
        None
    }
}

impl<TO, EO, SO> FieldLookup for ExhaustivenessCache<TO, EO, SO>
where
    TO: TypeOperations,
    EO: EnumOperations<Type = TO::Type>,
    SO: SealedClassOperations<Type = TO::Type>,
{
    type Type = TO::Type;

    fn get_field_types(&self, type_: &TO::Type) -> Rc<IndexMap<Key, StaticType>> {
        ExhaustivenessCache::get_field_types(self, type_)
    }

    fn get_additional_field_type(&self, type_: &TO::Type, key: &Key) -> Option<StaticType> {
        ExhaustivenessCache::get_additional_field_type(self, type_, key)
    }
}

impl<TO, EO, SO> ObjectPropertyLookup for ExhaustivenessCache<TO, EO, SO>
where
    TO: TypeOperations,
    EO: EnumOperations<Type = TO::Type>,
    SO: SealedClassOperations<Type = TO::Type>,
{
    fn get_object_field_type(&self, key: &Key) -> Option<StaticType> {
        self.get_field_types(&self.type_operations.non_nullable_object_type())
            .get(key)
            .copied()
    }

    fn static_types(&self) -> &dyn StaticTypeArena {
        self
    }
}

/// The dispatch of the Dart virtual `StaticType` members on the node kinds.
impl<TO, EO, SO> StaticTypeArena for ExhaustivenessCache<TO, EO, SO>
where
    TO: TypeOperations,
    EO: EnumOperations<Type = TO::Type>,
    SO: SealedClassOperations<Type = TO::Type>,
{
    fn fields(&self, this: StaticType) -> Rc<IndexMap<Key, StaticType>> {
        match &*self.node(this) {
            StaticTypeNode::Wrapped(wrapped) => self.fields(wrapped.wrapped_type),
            StaticTypeNode::NonNullable(data) => self.type_based_fields(data),
            StaticTypeNode::NonNullableObject
            | StaticTypeNode::Never
            | StaticTypeNode::Null
            | StaticTypeNode::Nullable(_) => Rc::new(IndexMap::new()),
        }
    }

    fn get_property_type(
        &self,
        this: StaticType,
        field_lookup: &dyn ObjectPropertyLookup,
        key: &Key,
    ) -> Option<StaticType> {
        match &*self.node(this) {
            StaticTypeNode::NonNullableObject
            | StaticTypeNode::Never
            | StaticTypeNode::Null
            | StaticTypeNode::Nullable(_) => {
                st::object_field_mixin_get_property_type(self, this, field_lookup, key)
            }
            StaticTypeNode::Wrapped(wrapped) => {
                self.get_property_type(wrapped.wrapped_type, field_lookup, key)
            }
            StaticTypeNode::NonNullable(_) => st::base_get_property_type(self, this, key),
        }
    }

    fn get_additional_property_type(&self, this: StaticType, key: &Key) -> Option<StaticType> {
        match &*self.node(this) {
            StaticTypeNode::NonNullable(data) => {
                self.type_based_get_additional_property_type(data, key)
            }
            _ => None,
        }
    }

    fn is_subtype_of(&self, this: StaticType, other: StaticType) -> bool {
        match &*self.node(this) {
            StaticTypeNode::NonNullableObject => {
                st::non_nullable_object_is_subtype_of(self, this, other)
            }
            StaticTypeNode::Never => st::never_type_is_subtype_of(other),
            StaticTypeNode::Null => {
                st::nullable_is_subtype_of(self, this, StaticType::NEVER_TYPE, other)
            }
            StaticTypeNode::Nullable(nullable) => {
                st::nullable_is_subtype_of(self, this, nullable.underlying, other)
            }
            StaticTypeNode::Wrapped(wrapped) => st::wrapped_is_subtype_of(
                self,
                this,
                wrapped.wrapped_type,
                wrapped.implied_type,
                other,
            ),
            StaticTypeNode::NonNullable(data) => {
                st::non_nullable_is_subtype_of(self, this, other, |other| {
                    self.type_based_is_subtype_of_internal(data, other)
                })
            }
        }
    }

    fn is_sealed(&self, this: StaticType) -> bool {
        match &*self.node(this) {
            StaticTypeNode::NonNullableObject | StaticTypeNode::Never => false,
            // Avoid splitting into [nullType] and [neverType].
            StaticTypeNode::Null => false,
            StaticTypeNode::Nullable(_) => true,
            StaticTypeNode::Wrapped(wrapped) => self.is_sealed(wrapped.wrapped_type),
            StaticTypeNode::NonNullable(data) => self.type_based_is_sealed(data),
        }
    }

    fn is_private(&self, this: StaticType) -> bool {
        match &*self.node(this) {
            StaticTypeNode::NonNullableObject | StaticTypeNode::Never => false,
            StaticTypeNode::Null | StaticTypeNode::Nullable(_) => self.name(this).starts_with('_'),
            StaticTypeNode::Wrapped(wrapped) => self.is_private(wrapped.wrapped_type),
            StaticTypeNode::NonNullable(data) => self.type_based_is_private(data),
        }
    }

    fn is_enum_subtype(&self, this: StaticType) -> bool {
        match &*self.node(this) {
            StaticTypeNode::NonNullableObject | StaticTypeNode::Never => false,
            StaticTypeNode::Null => self.is_enum_subtype(StaticType::NEVER_TYPE),
            StaticTypeNode::Nullable(nullable) => self.is_enum_subtype(nullable.underlying),
            StaticTypeNode::Wrapped(wrapped) => self.is_enum_subtype(wrapped.wrapped_type),
            StaticTypeNode::NonNullable(data) => self.type_based_is_enum_subtype(data),
        }
    }

    fn library_uri(&self, this: StaticType) -> Option<String> {
        match &*self.node(this) {
            StaticTypeNode::NonNullableObject | StaticTypeNode::Never => None,
            StaticTypeNode::Null => self.library_uri(StaticType::NEVER_TYPE),
            StaticTypeNode::Nullable(nullable) => self.library_uri(nullable.underlying),
            StaticTypeNode::Wrapped(wrapped) => self.library_uri(wrapped.wrapped_type),
            StaticTypeNode::NonNullable(data) => self.type_based_library_uri(data),
        }
    }

    fn is_record(&self, this: StaticType) -> bool {
        match &*self.node(this) {
            StaticTypeNode::Wrapped(wrapped) => self.is_record(wrapped.wrapped_type),
            StaticTypeNode::NonNullable(data) => matches!(data.kind, TypeBasedKind::Record),
            _ => false,
        }
    }

    fn is_implicitly_nullable(&self, this: StaticType) -> bool {
        match &*self.node(this) {
            StaticTypeNode::NonNullableObject | StaticTypeNode::Never => false,
            StaticTypeNode::Null | StaticTypeNode::Nullable(_) => true,
            StaticTypeNode::Wrapped(wrapped) => self.is_implicitly_nullable(wrapped.wrapped_type),
            StaticTypeNode::NonNullable(data) => data.is_implicitly_nullable,
        }
    }

    fn name(&self, this: StaticType) -> String {
        match &*self.node(this) {
            StaticTypeNode::NonNullableObject => st::NON_NULLABLE_OBJECT_NAME.to_string(),
            StaticTypeNode::Never => st::NEVER_TYPE_NAME.to_string(),
            StaticTypeNode::Null => st::NULL_TYPE_NAME.to_string(),
            StaticTypeNode::Nullable(nullable) => st::nullable_name(self, nullable.underlying),
            StaticTypeNode::Wrapped(wrapped) => self.name(wrapped.wrapped_type),
            StaticTypeNode::NonNullable(data) => self.type_based_name(data),
        }
    }

    fn type_to_dart(&self, this: StaticType, buffer: &mut dyn DartTemplateBuffer) {
        match &*self.node(this) {
            StaticTypeNode::NonNullableObject | StaticTypeNode::Never | StaticTypeNode::Null => {
                buffer.write_core_type(&self.name(this))
            }
            StaticTypeNode::Nullable(nullable) => {
                st::nullable_type_to_dart(self, nullable.underlying, buffer)
            }
            StaticTypeNode::Wrapped(wrapped) => self.type_to_dart(wrapped.wrapped_type, buffer),
            StaticTypeNode::NonNullable(data) => self.type_based_type_to_dart(data, buffer),
        }
    }

    fn nullable(&self, this: StaticType) -> StaticType {
        match &*self.node(this) {
            StaticTypeNode::NonNullableObject => StaticType::NULLABLE_OBJECT,
            StaticTypeNode::Never => StaticType::NULL_TYPE,
            StaticTypeNode::Null | StaticTypeNode::Nullable(_) => this,
            StaticTypeNode::Wrapped(wrapped) => st::wrapped_nullable(self, this, wrapped),
            StaticTypeNode::NonNullable(data) => {
                if let Some(nullable) = data.nullable.get() {
                    return *nullable;
                }
                let nullable = self.new_nullable_static_type(this);
                *data.nullable.get_or_init(|| nullable)
            }
        }
    }

    fn non_nullable(&self, this: StaticType) -> StaticType {
        match &*self.node(this) {
            StaticTypeNode::NonNullableObject | StaticTypeNode::Never => this,
            StaticTypeNode::Null => StaticType::NEVER_TYPE,
            StaticTypeNode::Nullable(nullable) => nullable.underlying,
            StaticTypeNode::Wrapped(wrapped) => st::wrapped_non_nullable(self, this, wrapped),
            StaticTypeNode::NonNullable(_) => this,
        }
    }

    fn get_subtypes(&self, this: StaticType, keys_of_interest: &IndexSet<Key>) -> Vec<StaticType> {
        match &*self.node(this) {
            StaticTypeNode::NonNullableObject | StaticTypeNode::Never => vec![],
            // Avoid splitting into [nullType] and [neverType].
            StaticTypeNode::Null => vec![],
            StaticTypeNode::Nullable(nullable) => st::nullable_get_subtypes(nullable.underlying),
            StaticTypeNode::Wrapped(wrapped) => st::wrapped_get_subtypes(
                self,
                wrapped.wrapped_type,
                wrapped.implied_type,
                keys_of_interest,
            ),
            StaticTypeNode::NonNullable(data) => {
                self.type_based_get_subtypes(this, data, keys_of_interest)
            }
        }
    }

    fn space_to_text(
        &self,
        this: StaticType,
        space_properties: &IndexMap<Key, Space>,
        additional_space_properties: &IndexMap<Key, Space>,
    ) -> String {
        match &*self.node(this) {
            StaticTypeNode::NonNullable(data) => self.type_based_space_to_text(
                this,
                data,
                space_properties,
                additional_space_properties,
            ),
            _ => st::base_space_to_text(self, this, space_properties, additional_space_properties),
        }
    }

    fn witness_to_dart(
        &self,
        this: StaticType,
        buffer: &mut dyn DartTemplateBuffer,
        witness: &PropertyWitness,
        witness_fields: &IndexMap<Key, &PropertyWitness>,
        for_correction: bool,
    ) {
        match &*self.node(this) {
            StaticTypeNode::Wrapped(wrapped) => self.witness_to_dart(
                wrapped.wrapped_type,
                buffer,
                witness,
                witness_fields,
                for_correction,
            ),
            StaticTypeNode::NonNullable(data) => self.type_based_witness_to_dart_dispatch(
                this,
                data,
                buffer,
                witness,
                witness_fields,
                for_correction,
            ),
            _ => st::base_witness_to_dart(
                self,
                this,
                buffer,
                witness,
                witness_fields,
                for_correction,
            ),
        }
    }

    fn as_nullable_static_type(&self, this: StaticType) -> Option<StaticType> {
        match &*self.node(this) {
            StaticTypeNode::Null => Some(StaticType::NEVER_TYPE),
            StaticTypeNode::Nullable(nullable) => Some(nullable.underlying),
            _ => None,
        }
    }

    fn as_wrapped_static_type(&self, this: StaticType) -> Option<(StaticType, StaticType)> {
        match &*self.node(this) {
            StaticTypeNode::Wrapped(wrapped) => Some((wrapped.wrapped_type, wrapped.implied_type)),
            _ => None,
        }
    }

    fn new_wrapped_static_type(
        &self,
        wrapped_type: StaticType,
        implied_type: StaticType,
    ) -> StaticType {
        self.alloc(
            StaticTypeNode::Wrapped(WrappedStaticType::new(wrapped_type, implied_type)),
            Some(StaticTypeEqKey::Wrapped(
                wrapped_type.eq_id(),
                implied_type.eq_id(),
            )),
        )
    }
}

/// Mixin for creating [Space]s from `Pattern`s.
///
/// The Dart type parameters `Pattern` and `Type` are associated types. The
/// abstract members are required methods; the mixin methods are provided
/// methods. The private Dart methods are the private free functions below.
pub trait SpaceCreator {
    type Pattern;
    type Type: Clone + Eq + Hash + Debug + 'static;

    fn type_operations(&self) -> &dyn TypeOperations<Type = Self::Type>;

    fn object_field_lookup(&self) -> &dyn ObjectPropertyLookup;

    /// Returns `true` if the current library has version greater than or
    /// equal to `$major.$minor`.
    fn has_language_version(&self, major: u32, minor: u32) -> bool;

    /// Creates a [StaticType] for an unknown type.
    ///
    /// This is used when the type of the pattern is unknown or can't be
    /// represented as a [StaticType]. This type is unique and ensures that it
    /// is neither matches anything nor is matched by anything.
    fn create_unknown_static_type(&mut self) -> StaticType;

    /// Creates the [StaticType] for [type_].
    fn create_static_type(&mut self, type_: &Self::Type) -> StaticType;

    /// Creates the [StaticType] for the list [type_] with the given
    /// [restriction].
    fn create_list_type(
        &mut self,
        type_: &Self::Type,
        restriction: ListTypeRestriction<Self::Type>,
    ) -> StaticType;

    /// Creates the [StaticType] for the map [type_] with the given
    /// [restriction].
    fn create_map_type(
        &mut self,
        type_: &Self::Type,
        restriction: MapTypeRestriction<Self::Type>,
    ) -> StaticType;

    /// Creates the [Space] for [pattern] at the given [path].
    ///
    /// The [context_type] is the [StaticType] in which the pattern match is
    /// performed. This is used to the restrict type of the created [Space] to
    /// the types allowed by the context. For instance `Object(:var hashCode)`
    /// is in itself unrestricted and would yield the top space for matching
    /// `var hashCode`. Using the [context_type] `int`, as given by the type of
    /// the `Object.hashCode`, the created space is all `int` values rather
    /// than all values.
    ///
    /// If [non_null] is `true`, the space is implicitly non-nullable.
    fn dispatch_pattern(
        &mut self,
        path: &Path,
        context_type: StaticType,
        pattern: &Self::Pattern,
        non_null: bool,
    ) -> Space;

    /// Creates the root space for [pattern].
    fn create_root_space(&mut self, context_type: StaticType, pattern: &Self::Pattern) -> Space {
        self.dispatch_pattern(&Path::root(), context_type, pattern, false)
    }

    /// Creates the [Space] at [path] for a variable pattern of the declared
    /// [type_].
    ///
    /// If [non_null] is `true`, the space is implicitly non-nullable.
    fn create_variable_space(
        &mut self,
        path: &Path,
        context_type: StaticType,
        type_: &Self::Type,
        non_null: bool,
    ) -> Space {
        let static_type = create_static_type_with_context(self, context_type, type_, non_null);
        Space::new(path.clone(), static_type)
    }

    /// Creates the [Space] at [path] for an object pattern of the required
    /// [type_] and [field_patterns].
    ///
    /// If [non_null] is `true`, the space is implicitly non-nullable.
    fn create_object_space(
        &mut self,
        path: &Path,
        context_type: StaticType,
        type_: &Self::Type,
        field_patterns: &IndexMap<String, Self::Pattern>,
        extension_property_types: &IndexMap<String, Self::Type>,
        non_null: bool,
    ) -> Space {
        let static_type = create_static_type_with_context(self, context_type, type_, non_null);
        let mut properties: IndexMap<Key, Space> = IndexMap::new();
        for (name, pattern) in field_patterns {
            let property_type: StaticType;
            let extension_property_type = extension_property_types.get(name);
            let key: Key;
            if let Some(extension_property_type) = extension_property_type {
                property_type = self.create_static_type(extension_property_type);
                let receiver_type = self.create_static_type(type_);
                key = Key::Extension(ExtensionKey::new(
                    receiver_type,
                    name.clone(),
                    property_type,
                ));
            } else {
                key = Key::Name(name.clone());
                let object_field_lookup = self.object_field_lookup();
                property_type = object_field_lookup
                    .static_types()
                    .get_property_type(static_type, object_field_lookup, &key)
                    .unwrap_or(StaticType::NULLABLE_OBJECT);
            }
            let space =
                self.dispatch_pattern(&path.add(key.clone()), property_type, pattern, false);
            properties.insert(key, space);
        }
        Space::with_properties(path.clone(), static_type, properties, IndexMap::new())
    }

    /// Creates the [Space] at [path] for a record pattern of the required
    /// [record_type], [positional_fields], and [named_fields].
    fn create_record_space(
        &mut self,
        path: &Path,
        context_type: StaticType,
        record_type: &Self::Type,
        positional_fields: &[Self::Pattern],
        named_fields: &IndexMap<String, Self::Pattern>,
    ) -> Space {
        let static_type = create_static_type_with_context(self, context_type, record_type, true);
        let mut properties: IndexMap<Key, Space> = IndexMap::new();
        for (index, positional_field) in positional_fields.iter().enumerate() {
            let key = Key::RecordIndex(index);
            let object_field_lookup = self.object_field_lookup();
            let property_type = object_field_lookup
                .static_types()
                .get_property_type(static_type, object_field_lookup, &key)
                .unwrap_or(StaticType::NULLABLE_OBJECT);
            let space = self.dispatch_pattern(
                &path.add(key.clone()),
                property_type,
                positional_field,
                false,
            );
            properties.insert(key, space);
        }
        for (name, pattern) in named_fields {
            let key = Key::RecordName(name.clone());
            let object_field_lookup = self.object_field_lookup();
            let property_type = object_field_lookup
                .static_types()
                .get_property_type(static_type, object_field_lookup, &key)
                .unwrap_or(StaticType::NULLABLE_OBJECT);
            let space =
                self.dispatch_pattern(&path.add(key.clone()), property_type, pattern, false);
            properties.insert(key, space);
        }
        Space::with_properties(path.clone(), static_type, properties, IndexMap::new())
    }

    /// Creates the [Space] at [path] for a wildcard pattern with the declared
    /// [type_].
    ///
    /// If [non_null] is `true`, the space is implicitly non-nullable.
    fn create_wildcard_space(
        &mut self,
        path: &Path,
        context_type: StaticType,
        type_: Option<&Self::Type>,
        non_null: bool,
    ) -> Space {
        match type_ {
            None => {
                let mut static_type = context_type;
                if non_null {
                    static_type = self
                        .object_field_lookup()
                        .static_types()
                        .non_nullable(static_type);
                }
                Space::new(path.clone(), static_type)
            }
            Some(type_) => {
                let static_type =
                    create_static_type_with_context(self, context_type, type_, non_null);
                Space::new(path.clone(), static_type)
            }
        }
    }

    /// Creates the [Space] at [path] for a relational pattern.
    fn create_relational_space(&mut self, path: &Path) -> Space {
        // This pattern do not add to the exhaustiveness coverage.
        self.create_unknown_space(path)
    }

    /// Creates the [Space] at [path] for a cast pattern with the given
    /// [sub_pattern].
    ///
    /// If [non_null] is `true`, the space is implicitly non-nullable.
    fn create_cast_space(
        &mut self,
        path: &Path,
        context_type: StaticType,
        type_: &Self::Type,
        sub_pattern: &Self::Pattern,
        non_null: bool,
    ) -> Space {
        let mut space = self.dispatch_pattern(path, context_type, sub_pattern, non_null);
        let cast_type = self.create_static_type(type_);
        if self.has_language_version(3, 3) {
            if is_contained_in(self.object_field_lookup().static_types(), cast_type, &space) {
                // If all values in [castType] are also in [space] then the
                // complete [contextType] is matched. For instance
                //
                //    method(var d) => switch (d) {
                //        final String value => value,
                //        // This covers everything either by matching as int or
                //        // by throwing.
                //        final value as int => value,
                //      };
                space = Space::new(path.clone(), context_type);
            }
            if !self.type_operations().is_potentially_nullable(type_) {
                // If `null` is _not_ included in [castType], we can include in
                // the generated [space].
                let types = self.object_field_lookup().static_types();
                space = space.union(types, &Space::new(path.clone(), StaticType::NULL_TYPE));
            }
        } else {
            let types = self.object_field_lookup().static_types();
            // The following check assumes that the subpattern space is
            // unrestrictive.
            if types.is_subtype_of(cast_type, context_type) && types.is_sealed(context_type) {
                for subtype in expand_sealed_subtypes(types, context_type, &IndexSet::new()) {
                    // If [subtype] is a subtype of [castType] it will not throw
                    // and must be handled by [subPattern]. For instance
                    //
                    //    sealed class S {}
                    //    sealed class X extends S {}
                    //    class A extends X {}
                    //    method(S s) => switch (s) {
                    //      A() as X => 0,
                    //    }
                    //
                    // If [castType] is a subtype of [subtype] it might not throw
                    // but still not handle all values of the [subtype].
                    //
                    //    sealed class S {}
                    //    class A extends S {}
                    //    class X extends A {
                    //      int field;
                    //      X(this.field);
                    //    }
                    //    method(S s) => switch (s) {
                    //      X(field: 42) as X => 0,
                    //    }
                    //
                    if !types.is_subtype_of(cast_type, subtype)
                        && !types.is_subtype_of(subtype, cast_type)
                    {
                        // Otherwise the cast implicitly handles [subtype].
                        space = space.union(types, &Space::new(path.clone(), subtype));
                    }
                }
            }
        }
        space
    }

    /// Creates the [Space] at [path] for a null check pattern with the given
    /// [sub_pattern].
    fn create_null_check_space(
        &mut self,
        path: &Path,
        context_type: StaticType,
        sub_pattern: &Self::Pattern,
    ) -> Space {
        self.dispatch_pattern(path, context_type, sub_pattern, true)
    }

    /// Creates the [Space] at [path] for a null assert pattern with the given
    /// [sub_pattern].
    fn create_null_assert_space(
        &mut self,
        path: &Path,
        context_type: StaticType,
        sub_pattern: &Self::Pattern,
    ) -> Space {
        let space = self.dispatch_pattern(path, context_type, sub_pattern, true);
        let types = self.object_field_lookup().static_types();
        space.union(types, &Space::new(path.clone(), StaticType::NULL_TYPE))
    }

    /// Creates the [Space] at [path] for a logical or pattern with the given
    /// [left] and [right] subpatterns.
    ///
    /// If [non_null] is `true`, the space is implicitly non-nullable.
    fn create_logical_or_space(
        &mut self,
        path: &Path,
        context_type: StaticType,
        left: &Self::Pattern,
        right: &Self::Pattern,
        non_null: bool,
    ) -> Space {
        let a_space = self.dispatch_pattern(path, context_type, left, non_null);
        let b_space = self.dispatch_pattern(path, context_type, right, non_null);
        let types = self.object_field_lookup().static_types();
        a_space.union(types, &b_space)
    }

    /// Creates the [Space] at [path] for a logical and pattern with the given
    /// [left] and [right] subpatterns.
    ///
    /// If [non_null] is `true`, the space is implicitly non-nullable.
    fn create_logical_and_space(
        &mut self,
        path: &Path,
        context_type: StaticType,
        left: &Self::Pattern,
        right: &Self::Pattern,
        non_null: bool,
    ) -> Space {
        let a_space = self.dispatch_pattern(path, context_type, left, non_null);
        let b_space = self.dispatch_pattern(path, context_type, right, non_null);
        create_space_intersection(self, path, &a_space, &b_space)
    }

    /// Creates the [Space] at [path] for a list pattern.
    fn create_list_space(
        &mut self,
        path: &Path,
        type_: &Self::Type,
        element_type: &Self::Type,
        head_elements: &[Self::Pattern],
        rest_element: Option<&Self::Pattern>,
        tail_elements: &[Self::Pattern],
        has_rest: bool,
        has_explicit_type_argument: bool,
    ) -> Space {
        let head_size = head_elements.len();
        let tail_size = tail_elements.len();

        let type_argument_text = if has_explicit_type_argument {
            let mut sb = String::new();
            sb.push('<');
            sb.push_str(&self.type_operations().type_to_string(element_type));
            sb.push('>');
            sb
        } else {
            String::new()
        };

        let identity = ListTypeRestriction::new(
            element_type.clone(),
            type_argument_text,
            head_size + tail_size,
            has_rest,
        );

        let static_type = self.create_list_type(type_, identity);

        let mut additional_properties: IndexMap<Key, Space> = IndexMap::new();
        for (index, head_element) in head_elements.iter().enumerate() {
            let key = Key::Head(index);
            let property_type = self
                .object_field_lookup()
                .static_types()
                .get_additional_property_type(static_type, &key)
                .unwrap_or(StaticType::NULLABLE_OBJECT);
            let space =
                self.dispatch_pattern(&path.add(key.clone()), property_type, head_element, false);
            additional_properties.insert(key, space);
        }
        if has_rest {
            let key = Key::Rest {
                head_size,
                tail_size,
            };
            let property_type = self
                .object_field_lookup()
                .static_types()
                .get_additional_property_type(static_type, &key)
                .unwrap_or(StaticType::NULLABLE_OBJECT);
            let space = if let Some(rest_element) = rest_element {
                self.dispatch_pattern(&path.add(key.clone()), property_type, rest_element, false)
            } else {
                Space::new(path.add(key.clone()), property_type)
            };
            additional_properties.insert(key, space);
        }
        for index in 0..tail_size {
            let key = Key::Tail(index);
            let property_type = self
                .object_field_lookup()
                .static_types()
                .get_additional_property_type(static_type, &key)
                .unwrap_or(StaticType::NULLABLE_OBJECT);
            let space = self.dispatch_pattern(
                &path.add(key.clone()),
                property_type,
                &tail_elements[tail_elements.len() - index - 1],
                false,
            );
            additional_properties.insert(key, space);
        }
        Space::with_properties(
            path.clone(),
            static_type,
            IndexMap::new(),
            additional_properties,
        )
    }

    /// Creates the [Space] at [path] for a map pattern.
    fn create_map_space(
        &mut self,
        path: &Path,
        type_: &Self::Type,
        key_type: &Self::Type,
        value_type: &Self::Type,
        entries: &IndexMap<MapKey, Self::Pattern>,
        has_explicit_type_arguments: bool,
    ) -> Space {
        let type_arguments_text = if has_explicit_type_arguments {
            let mut sb = String::new();
            sb.push('<');
            sb.push_str(&self.type_operations().type_to_string(key_type));
            sb.push_str(", ");
            sb.push_str(&self.type_operations().type_to_string(value_type));
            sb.push('>');
            sb
        } else {
            String::new()
        };

        let identity = MapTypeRestriction::new(
            key_type.clone(),
            value_type.clone(),
            entries.keys().cloned().collect(),
            type_arguments_text,
        );
        let static_type = self.create_map_type(type_, identity);

        let mut additional_properties: IndexMap<Key, Space> = IndexMap::new();
        for (map_key, pattern) in entries {
            let key = Key::Map(map_key.clone());
            let property_type = self
                .object_field_lookup()
                .static_types()
                .get_additional_property_type(static_type, &key)
                .unwrap_or(StaticType::NULLABLE_OBJECT);
            let space =
                self.dispatch_pattern(&path.add(key.clone()), property_type, pattern, false);
            additional_properties.insert(key, space);
        }
        Space::with_properties(
            path.clone(),
            static_type,
            IndexMap::new(),
            additional_properties,
        )
    }

    /// Creates the [Space] at [path] for a pattern with unknown space.
    ///
    /// This is used when the space of the pattern is unknown or can't be
    /// represented precisely as a union of [SingleSpace]s. This space is
    /// unique and ensures that it is neither matches anything nor is matched
    /// by anything.
    fn create_unknown_space(&mut self, path: &Path) -> Space {
        let static_type = self.create_unknown_static_type();
        Space::new(path.clone(), static_type)
    }
}

/// `SpaceCreator._createStaticTypeWithContext`: creates the [StaticType] for
/// [type_] restricted by the [context_type]. If [non_null] is `true`, the
/// created type is non-nullable.
fn create_static_type_with_context<S: SpaceCreator + ?Sized>(
    creator: &mut S,
    context_type: StaticType,
    type_: &S::Type,
    non_null: bool,
) -> StaticType {
    let mut static_type = creator.create_static_type(type_);
    let types = creator.object_field_lookup().static_types();
    if types.is_subtype_of(context_type, static_type) {
        static_type = context_type;
    }
    if non_null {
        static_type = types.non_nullable(static_type);
    }
    static_type
}

/// `SpaceCreator._isUnrestricted`: returns `true` if [single_space] does not
/// restrict the spaces of any of its properties. For instance the pattern
/// `Object(:int hashCode)` is unrestricted but the pattern
/// `Object(hashCode: 5)` is restricted.
fn is_unrestricted(types: &dyn StaticTypeArena, single_space: &SingleSpace) -> bool {
    if !single_space.properties.is_empty() {
        let field_types = types.fields(single_space.type_);
        for (key, space) in &single_space.properties {
            let field_type = field_types
                .get(key)
                .copied()
                .unwrap_or(StaticType::NEVER_TYPE);
            if !is_contained_in(types, field_type, space) {
                return false;
            }
        }
    }
    if !single_space.additional_properties.is_empty() {
        for (key, space) in &single_space.additional_properties {
            let field_type = types
                .get_additional_property_type(single_space.type_, key)
                .unwrap_or(StaticType::NEVER_TYPE);
            if !is_contained_in(types, field_type, space) {
                return false;
            }
        }
    }
    true
}

/// `SpaceCreator._isContainedIn`: returns `true` if the space implied by
/// [type_], i.e. all values of [type_] regardless of properties, is contained
/// in [space].
fn is_contained_in(types: &dyn StaticTypeArena, type_: StaticType, space: &Space) -> bool {
    let mut unrestricted_cache: HashMap<Rc<SingleSpace>, bool> = HashMap::new();
    let mut is_unrestricted_cached = |single_space: &Rc<SingleSpace>| -> bool {
        if let Some(result) = unrestricted_cache.get(single_space) {
            return *result;
        }
        let result = is_unrestricted(types, single_space);
        unrestricted_cache.insert(single_space.clone(), result);
        result
    };
    if space.single_spaces().len() == 1 {
        // Optimize for simple spaces to avoid unnecessary expansion of
        // subtypes.
        let single_space = &space.single_spaces()[0];
        let is_unrestricted = is_unrestricted_cached(single_space);
        if is_unrestricted && types.is_subtype_of(type_, single_space.type_) {
            return true;
        }
    }
    // To handle case like:
    //
    //    sealed class M {}
    //    class A extends M {}
    //    class B extends M {}
    //    method(o) => switch (o) {
    //        (A() || B()) as M => 0,
    //      };
    //
    // we expand [type] into subtypes before determining for containment.
    let subtypes = expand_sealed_subtypes(types, type_, &IndexSet::new());
    for subtype in subtypes {
        let mut found = false;
        for single_space in space.single_spaces() {
            let is_unrestricted = is_unrestricted_cached(single_space);
            if is_unrestricted && types.is_subtype_of(subtype, single_space.type_) {
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
    }
    true
}

/// `SpaceCreator._createSingleSpaceIntersection`: creates an approximation of
/// the intersection of the single spaces [a] and [b].
fn create_single_space_intersection<S: SpaceCreator + ?Sized>(
    creator: &mut S,
    path: &Path,
    a: &SingleSpace,
    b: &SingleSpace,
) -> Option<SingleSpace> {
    let types = creator.object_field_lookup().static_types();
    let type_ = if types.is_subtype_of(a.type_, b.type_) {
        a.type_
    } else if types.is_subtype_of(b.type_, a.type_) {
        b.type_
    } else {
        return None;
    };
    let mut properties: IndexMap<Key, Space> = IndexMap::new();
    for (key, a_space) in &a.properties {
        if let Some(b_space) = b.properties.get(key) {
            let space =
                create_space_intersection(creator, &path.add(key.clone()), a_space, b_space);
            properties.insert(key.clone(), space);
        } else {
            properties.insert(key.clone(), a_space.clone());
        }
    }
    for (key, space) in &b.properties {
        properties
            .entry(key.clone())
            .or_insert_with(|| space.clone());
    }
    Some(SingleSpace::with_properties(
        type_,
        properties,
        IndexMap::new(),
    ))
}

/// `SpaceCreator._createSpaceIntersection`: creates an approximation of the
/// intersection of spaces [a] and [b].
fn create_space_intersection<S: SpaceCreator + ?Sized>(
    creator: &mut S,
    path: &Path,
    a: &Space,
    b: &Space,
) -> Space {
    debug_assert!(
        path == a.path(),
        "Unexpected path. Expected {path}, actual {}.",
        a.path()
    );
    debug_assert!(
        path == b.path(),
        "Unexpected path. Expected {path}, actual {}.",
        b.path()
    );
    let mut single_spaces: Vec<Rc<SingleSpace>> = vec![];
    let mut has_unknown_space = false;
    for a_single_space in a.single_spaces() {
        for b_single_space in b.single_spaces() {
            let space =
                create_single_space_intersection(creator, path, a_single_space, b_single_space);
            if let Some(space) = space {
                single_spaces.push(Rc::new(space));
            } else {
                has_unknown_space = true;
            }
        }
    }
    if has_unknown_space {
        let unknown = creator.create_unknown_static_type();
        single_spaces.push(Rc::new(SingleSpace::new(unknown)));
    }
    Space::from_single_spaces(
        creator.object_field_lookup().static_types(),
        path.clone(),
        single_spaces,
    )
}
