// Dart source: pkg/analyzer/lib/src/dart/element/greatest_lower_bound.dart

//! `GreatestLowerBoundHelper`.

use dartr_element::TypeId;

use crate::type_system::TypeSystem;

/// `GreatestLowerBoundHelper`.
pub struct GreatestLowerBoundHelper<'a> {
    pub type_system: TypeSystem<'a>,
}

impl<'a> GreatestLowerBoundHelper<'a> {
    pub fn new(type_system: TypeSystem<'a>) -> Self {
        GreatestLowerBoundHelper { type_system }
    }

    /// `getGreatestLowerBound(T1, T2)`.
    pub fn get_greatest_lower_bound(&self, _t1: TypeId, _t2: TypeId) -> TypeId {
        todo!("GreatestLowerBoundHelper.getGreatestLowerBound")
    }
}
