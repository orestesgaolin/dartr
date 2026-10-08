// Dart source: pkg/analyzer/lib/src/dart/element/runtime_type_equality.dart

//! `RuntimeTypeEqualityHelper`.

use dartr_element::TypeId;

use crate::type_system::TypeSystem;

/// `RuntimeTypeEqualityHelper`.
pub struct RuntimeTypeEqualityHelper<'a> {
    pub type_system: TypeSystem<'a>,
}

impl<'a> RuntimeTypeEqualityHelper<'a> {
    pub fn new(type_system: TypeSystem<'a>) -> Self {
        RuntimeTypeEqualityHelper { type_system }
    }

    /// `equal(T1, T2)`.
    pub fn equal(&self, _t1: TypeId, _t2: TypeId) -> bool {
        todo!("RuntimeTypeEqualityHelper.equal")
    }
}
