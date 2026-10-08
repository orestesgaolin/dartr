// Dart source: pkg/_fe_analyzer_shared/lib/src/exhaustiveness/types/future_or.dart

use super::super::shared::{ExhaustivenessCache, TypeOperations};
use super::super::static_type::StaticType;
use super::{
    EnumOperations, Restriction, SealedClassOperations, TypeBasedKind, TypeBasedStaticType,
};

/// [StaticType] for a `FutureOr<T>` type for some type `T`.
///
/// This is a sealed type where the subtypes for are `T` and `Future<T>`.
///
/// The fields of the Dart class `FutureOrStaticType`; see
/// [TypeBasedKind::FutureOr].
pub struct FutureOrStaticType {
    /// The type for `T`.
    type_argument: StaticType,

    /// The type for `Future<T>`.
    future_type: StaticType,
}

impl FutureOrStaticType {
    /// `FutureOrStaticType.getSubtypes`.
    pub(crate) fn get_subtypes(&self) -> Vec<StaticType> {
        vec![self.type_argument, self.future_type]
    }
}

impl<TO, EO, SO> ExhaustivenessCache<TO, EO, SO>
where
    TO: TypeOperations,
    EO: EnumOperations<Type = TO::Type>,
    SO: SealedClassOperations<Type = TO::Type>,
{
    /// Dart `new FutureOrStaticType(typeOperations, fieldLookup, type,
    /// typeArgument, futureType, isImplicitlyNullable: ...)`.
    pub(crate) fn new_future_or_static_type(
        &self,
        type_: TO::Type,
        type_argument: StaticType,
        future_type: StaticType,
        is_implicitly_nullable: bool,
    ) -> StaticType {
        self.new_type_based_static_type(TypeBasedStaticType::new(
            type_,
            is_implicitly_nullable,
            Restriction::Unrestricted,
            None,
            TypeBasedKind::FutureOr(FutureOrStaticType {
                type_argument,
                future_type,
            }),
        ))
    }
}
