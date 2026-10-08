// Dart source: pkg/analyzer/lib/src/dart/element/normalize.dart

//! `NormalizeHelper` (`resources/type-system/normalization.md`).

use dartr_element::TypeId;

use crate::type_system::TypeSystem;

/// `NormalizeHelper`.
pub struct NormalizeHelper<'a> {
    pub type_system: TypeSystem<'a>,
}

impl<'a> NormalizeHelper<'a> {
    pub fn new(type_system: TypeSystem<'a>) -> Self {
        NormalizeHelper { type_system }
    }

    /// `normalize(T)`.
    pub fn normalize(&mut self, _t: TypeId) -> TypeId {
        todo!("NormalizeHelper.normalize")
    }

    /// `normalizeFunctionType(T)`.
    pub fn normalize_function_type(&mut self, _t: TypeId) -> TypeId {
        todo!("NormalizeHelper.normalizeFunctionType")
    }

    /// `normalizeInterfaceType(T)`.
    pub fn normalize_interface_type(&mut self, _t: TypeId) -> TypeId {
        todo!("NormalizeHelper.normalizeInterfaceType")
    }
}
