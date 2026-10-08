// Dart source: pkg/_fe_analyzer_shared/lib/src/exhaustiveness/types/bool.dart

use std::cell::OnceCell;

use super::super::key::Identity;
use super::super::shared::{ExhaustivenessCache, TypeOperations};
use super::super::static_type::StaticType;
use super::{
    CacheTypeBased, EnumOperations, IdentityRestriction, Restriction, SealedClassOperations,
    TypeBasedKind, TypeBasedStaticType,
};

/// [StaticType] for the `bool` type.
///
/// The fields of the Dart class `BoolStaticType`; see
/// [TypeBasedKind::Bool].
#[derive(Default)]
pub struct BoolStaticType {
    true_type: OnceCell<StaticType>,
    false_type: OnceCell<StaticType>,
}

impl BoolStaticType {
    pub fn new() -> BoolStaticType {
        BoolStaticType::default()
    }
}

impl<TO, EO, SO> ExhaustivenessCache<TO, EO, SO>
where
    TO: TypeOperations,
    EO: EnumOperations<Type = TO::Type>,
    SO: SealedClassOperations<Type = TO::Type>,
{
    /// Dart `new BoolStaticType(typeOperations, fieldLookup, type)`.
    pub(crate) fn new_bool_static_type(&self, type_: TO::Type) -> StaticType {
        self.new_type_based_static_type(TypeBasedStaticType::new(
            type_,
            false,
            Restriction::Unrestricted,
            None,
            TypeBasedKind::Bool(BoolStaticType::new()),
        ))
    }

    /// `BoolStaticType.trueType`.
    pub(crate) fn bool_static_type_true_type(
        &self,
        this: &CacheTypeBased<TO, EO, SO>,
        bool_type: &BoolStaticType,
    ) -> StaticType {
        if let Some(true_type) = bool_type.true_type.get() {
            return *true_type;
        }
        let true_type = self.new_bool_value_static_type(this.type_.clone(), true);
        *bool_type.true_type.get_or_init(|| true_type)
    }

    /// `BoolStaticType.falseType`.
    pub(crate) fn bool_static_type_false_type(
        &self,
        this: &CacheTypeBased<TO, EO, SO>,
        bool_type: &BoolStaticType,
    ) -> StaticType {
        if let Some(false_type) = bool_type.false_type.get() {
            return *false_type;
        }
        let false_type = self.new_bool_value_static_type(this.type_.clone(), false);
        *bool_type.false_type.get_or_init(|| false_type)
    }

    /// `BoolStaticType.getSubtypes`.
    pub(crate) fn bool_static_type_get_subtypes(
        &self,
        this: &CacheTypeBased<TO, EO, SO>,
        bool_type: &BoolStaticType,
    ) -> Vec<StaticType> {
        vec![
            self.bool_static_type_true_type(this, bool_type),
            self.bool_static_type_false_type(this, bool_type),
        ]
    }

    /// Dart `new _BoolValueStaticType(typeOperations, fieldLookup, type,
    /// value)`: [StaticType] for an object restricted to a single boolean
    /// value (either `true` or `false`).
    fn new_bool_value_static_type(&self, type_: TO::Type, value: bool) -> StaticType {
        self.new_type_based_static_type(TypeBasedStaticType::new(
            type_,
            false,
            Restriction::Identity(IdentityRestriction::new(Identity::new(value))),
            Some(format!("{value}")),
            TypeBasedKind::BoolValue(value),
        ))
    }
}
