// Dart source: pkg/analyzer/lib/src/dart/element/least_greatest_closure.dart

//! `LeastGreatestClosureHelper`.

use dartr_element::{EId, TypeId, TypeParameterElement};

use crate::type_system::TypeSystem;

/// `LeastGreatestClosureHelper`.
pub struct LeastGreatestClosureHelper<'a> {
    pub type_system: TypeSystem<'a>,
    pub top_type: TypeId,
    pub top_function_type: TypeId,
    pub bottom_type: TypeId,
    pub elimination_targets: Vec<EId<TypeParameterElement>>,
}

impl<'a> LeastGreatestClosureHelper<'a> {
    pub fn new(
        type_system: TypeSystem<'a>,
        top_type: TypeId,
        top_function_type: TypeId,
        bottom_type: TypeId,
        elimination_targets: Vec<EId<TypeParameterElement>>,
    ) -> Self {
        LeastGreatestClosureHelper {
            type_system,
            top_type,
            top_function_type,
            bottom_type,
            elimination_targets,
        }
    }

    /// `eliminateToGreatest(type)`.
    pub fn eliminate_to_greatest(&mut self, _t: TypeId) -> TypeId {
        todo!("LeastGreatestClosureHelper.eliminateToGreatest")
    }

    /// `eliminateToLeast(type)`.
    pub fn eliminate_to_least(&mut self, _t: TypeId) -> TypeId {
        todo!("LeastGreatestClosureHelper.eliminateToLeast")
    }
}
