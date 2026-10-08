// Dart source: pkg/_fe_analyzer_shared/lib/src/exhaustiveness/types/enum.dart

use std::cell::OnceCell;
use std::fmt::Debug;
use std::hash::Hash;
use std::rc::Rc;

use indexmap::IndexMap;

use super::super::key::Identity;
use super::super::shared::{ExhaustivenessCache, TypeOperations};
use super::super::static_type::{StaticType, StaticTypeArena};
use super::{
    CacheTypeBased, IdentityRestriction, Restriction, SealedClassOperations, TypeBasedKind,
    TypeBasedStaticType,
};

/// Interface implemented by analyzer/CFE to support [StaticType]s for enums.
pub trait EnumOperations {
    type Type;
    type EnumClass: Clone + Eq + Hash;
    type EnumElement: Clone + Eq + Hash + Debug + 'static;
    type EnumElementValue: Clone + Eq + Hash;

    /// Returns the enum class declaration for the [type_] or `None` if [type_]
    /// is not an enum type.
    fn get_enum_class(&self, type_: &Self::Type) -> Option<Self::EnumClass>;

    /// Returns the enum elements defined by [enum_class].
    fn get_enum_elements(&self, enum_class: &Self::EnumClass) -> Vec<Self::EnumElement>;

    /// Returns the value defined by the [enum_element]. The encoding is
    /// specific the implementation of this interface but must ensure constant
    /// value identity.
    fn get_enum_element_value(
        &self,
        enum_element: &Self::EnumElement,
    ) -> Option<Self::EnumElementValue>;

    /// Returns the declared name of the [enum_element].
    fn get_enum_element_name(&self, enum_element: &Self::EnumElement) -> String;

    /// Returns the static type of the [enum_element].
    fn get_enum_element_type(&self, enum_element: &Self::EnumElement) -> Self::Type;
}

/// [EnumInfo] stores information to compute the static type for and the type
/// of and enum class and its enum elements.
pub struct EnumInfo<EO: EnumOperations> {
    enum_class: EO::EnumClass,
    enum_elements: OnceCell<IndexMap<EO::EnumElementValue, StaticType>>,
}

impl<EO: EnumOperations> EnumInfo<EO> {
    pub fn new(enum_class: EO::EnumClass) -> EnumInfo<EO> {
        EnumInfo {
            enum_class,
            enum_elements: OnceCell::new(),
        }
    }
}

/// [StaticType] for an instantiation of an enum that support access to the
/// enum values that populate its type through the `getSubtypes` getter.
///
/// The fields of the Dart class `EnumStaticType`; see
/// [TypeBasedKind::Enum].
pub struct EnumStaticType<E> {
    enum_info: Rc<E>,
    enum_elements: OnceCell<Vec<StaticType>>,
}

impl<TO, EO, SO> ExhaustivenessCache<TO, EO, SO>
where
    TO: TypeOperations,
    EO: EnumOperations<Type = TO::Type>,
    SO: SealedClassOperations<Type = TO::Type>,
{
    /// `EnumInfo.enumElements`: returns a map of the enum elements and their
    /// corresponding [StaticType]s declared by the enum class.
    pub(crate) fn enum_info_enum_elements<'a>(
        &self,
        enum_info: &'a EnumInfo<EO>,
    ) -> &'a IndexMap<EO::EnumElementValue, StaticType> {
        if enum_info.enum_elements.get().is_none() {
            let elements = self.enum_info_create_enum_elements(enum_info);
            let _ = enum_info.enum_elements.set(elements);
        }
        enum_info.enum_elements.get().unwrap()
    }

    /// `EnumInfo.getEnumElement`: returns the [StaticType] corresponding to
    /// [enum_element_value].
    pub(crate) fn enum_info_get_enum_element(
        &self,
        enum_info: &EnumInfo<EO>,
        enum_element_value: &EO::EnumElementValue,
    ) -> StaticType {
        *self
            .enum_info_enum_elements(enum_info)
            .get(enum_element_value)
            .expect("Unknown enum element value")
    }

    /// `EnumInfo._createEnumElements`.
    fn enum_info_create_enum_elements(
        &self,
        enum_info: &EnumInfo<EO>,
    ) -> IndexMap<EO::EnumElementValue, StaticType> {
        let mut elements: IndexMap<EO::EnumElementValue, StaticType> = IndexMap::new();
        for element in self
            .enum_operations
            .get_enum_elements(&enum_info.enum_class)
        {
            if let Some(value) = self.enum_operations.get_enum_element_value(&element) {
                let static_type = self.new_enum_element_static_type(
                    self.enum_operations.get_enum_element_type(&element),
                    IdentityRestriction::new(Identity::new(element.clone())),
                    self.enum_operations.get_enum_element_name(&element),
                );
                elements.insert(value, static_type);
            }
        }
        elements
    }

    /// Dart `new EnumStaticType(typeOperations, fieldLookup, type, enumInfo)`.
    pub(crate) fn new_enum_static_type(
        &self,
        type_: TO::Type,
        enum_info: Rc<EnumInfo<EO>>,
    ) -> StaticType {
        self.new_type_based_static_type(TypeBasedStaticType::new(
            type_,
            false,
            Restriction::Unrestricted,
            None,
            TypeBasedKind::Enum(EnumStaticType {
                enum_info,
                enum_elements: OnceCell::new(),
            }),
        ))
    }

    /// `EnumStaticType.enumElements` (and `getSubtypes`).
    pub(crate) fn enum_static_type_enum_elements(
        &self,
        this_type: StaticType,
        this: &CacheTypeBased<TO, EO, SO>,
        enum_type: &EnumStaticType<EnumInfo<EO>>,
    ) -> Vec<StaticType> {
        if let Some(elements) = enum_type.enum_elements.get() {
            return elements.clone();
        }
        let elements = self.enum_static_type_create_enum_elements(this_type, this, enum_type);
        enum_type.enum_elements.get_or_init(|| elements).clone()
    }

    /// `EnumStaticType._createEnumElements`.
    fn enum_static_type_create_enum_elements(
        &self,
        this_type: StaticType,
        this: &CacheTypeBased<TO, EO, SO>,
        enum_type: &EnumStaticType<EnumInfo<EO>>,
    ) -> Vec<StaticType> {
        let mut elements: Vec<StaticType> = vec![];
        let enum_elements: Vec<StaticType> = self
            .enum_info_enum_elements(&enum_type.enum_info)
            .values()
            .copied()
            .collect();
        for enum_element in enum_elements {
            // For generic enums, the individual enum elements might not be
            // subtypes of the concrete enum type. For instance
            //
            //    enum E<T> {
            //      a<int>(),
            //      b<String>(),
            //      c<bool>(),
            //    }
            //
            //    method<T extends num>(E<T> e) {
            //      switch (e) { ... }
            //    }
            //
            // Here the enum elements `E.b` and `E.c` cannot be actual values of
            // `e` because of the bound `num` on `T`.
            //
            // We detect this by checking whether the enum element type is a
            // subtype of the overapproximation of [_type], in this case whether
            // the element types are subtypes of `E<num>`.
            //
            // Since all type arguments on enum values are fixed, we don't have
            // to avoid the trivial subtype instantiation `E<Never>`.
            let enum_element_node = self.node(enum_element);
            let enum_element_type = &enum_element_node
                .non_nullable_data()
                .expect("EnumElementStaticType")
                .type_;
            if self.type_operations.is_subtype_of(
                enum_element_type,
                &self.type_operations.overapproximate(&this.type_),
            ) {
                // Since the type of the enum element might not itself be a
                // subtype of [_type], for instance in the example above the
                // type of `Enum.a`, `Enum<int>`, is not a subtype of `Enum<T>`,
                // we wrap the static type to establish the subtype relation
                // between the [StaticType] for the enum element and this
                // [StaticType].
                elements.push(self.new_wrapped_static_type(enum_element, this_type));
            }
        }
        elements
    }

    /// Dart `new EnumElementStaticType<Type, EnumElement>(typeOperations,
    /// fieldLookup, type, restriction, name, value)`: [StaticType] for a single
    /// enum element.
    ///
    /// In the [StaticType] model, individual enum elements are represented as
    /// unique subtypes of the enum type, modelled using `EnumStaticType`. The
    /// `_value` is the identity of the [restriction].
    fn new_enum_element_static_type(
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
            TypeBasedKind::EnumElement,
        ))
    }
}
