// Dart source: pkg/analyzer/lib/src/dart/element/least_upper_bound.dart

//! `LeastUpperBoundHelper` (and `InterfaceLeastUpperBoundHelper`).

use dartr_element::TypeId;

use crate::type_system::TypeSystem;

/// `LeastUpperBoundHelper`.
pub struct LeastUpperBoundHelper<'a> {
    pub type_system: TypeSystem<'a>,
}

impl<'a> LeastUpperBoundHelper<'a> {
    pub fn new(type_system: TypeSystem<'a>) -> Self {
        LeastUpperBoundHelper { type_system }
    }

    /// `getLeastUpperBound(T1, T2)`.
    pub fn get_least_upper_bound(&self, _t1: TypeId, _t2: TypeId) -> TypeId {
        todo!("LeastUpperBoundHelper.getLeastUpperBound")
    }
}
