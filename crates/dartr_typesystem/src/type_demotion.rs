// Dart source: pkg/analyzer/lib/src/dart/element/type_demotion.dart

//! `DemotionVisitor`: replaces all promoted type variables with the type
//! variable itself.

use dartr_element::{Ctx, TypeId, TypeKind};

use crate::replacement_visitor::ReplacementVisitor;

/// `DemotionVisitor`. The visitor returns `None` if the type wasn't changed.
pub struct DemotionVisitor<'a> {
    pub ctx: Ctx<'a>,
}

impl<'a> ReplacementVisitor<'a> for DemotionVisitor<'a> {
    fn ctx(&self) -> Ctx<'a> {
        self.ctx
    }

    fn visit_type_parameter_type(&mut self, t: TypeId) -> Option<TypeId> {
        let TypeKind::TypeParameter {
            param,
            nullability,
            promoted_bound,
            alias,
        } = *self.ctx.ty(t)
        else {
            unreachable!()
        };

        promoted_bound?;

        Some(self.ctx.intern(TypeKind::TypeParameter {
            param,
            nullability,
            promoted_bound: None,
            alias,
        }))
    }
}
