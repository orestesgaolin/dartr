// Dart source: pkg/analyzer/lib/src/dart/element/top_merge.dart

//! `TopMergeHelper` (NNBD_TOP_MERGE).

use dartr_element::TypeId;

use crate::type_system::TypeSystem;

/// `TopMergeHelper`.
pub struct TopMergeHelper<'a> {
    pub type_system: TypeSystem<'a>,
}

impl<'a> TopMergeHelper<'a> {
    pub fn new(type_system: TypeSystem<'a>) -> Self {
        TopMergeHelper { type_system }
    }

    /// `topMerge(T, S)`; `None` where Dart throws.
    pub fn top_merge(&self, _t: TypeId, _s: TypeId) -> Option<TypeId> {
        todo!("TopMergeHelper.topMerge")
    }
}
