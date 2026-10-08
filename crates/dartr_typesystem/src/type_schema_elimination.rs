// Dart source: pkg/analyzer/lib/src/dart/element/type_schema_elimination.dart

//! `TypeSchemaEliminationVisitor`: computes least and greatest closures of
//! a type schema.
//!
//! Each visitor method returns `None` if there are no `_`s contained in the
//! type, otherwise it returns the result of substituting `_` with
//! `bottom_type` or `top_type`, as appropriate.

use dartr_element::{Ctx, TypeId};

use crate::replacement_visitor::ReplacementVisitor;

/// `TypeSchemaEliminationVisitor`.
pub struct TypeSchemaEliminationVisitor<'a> {
    ctx: Ctx<'a>,
    top_type: TypeId,
    bottom_type: TypeId,
    is_least_closure: bool,
}

impl<'a> ReplacementVisitor<'a> for TypeSchemaEliminationVisitor<'a> {
    fn ctx(&self) -> Ctx<'a> {
        self.ctx
    }

    fn change_variance(&mut self) {
        self.is_least_closure = !self.is_least_closure;
    }

    fn visit_unknown_inferred_type(&mut self, _t: TypeId) -> Option<TypeId> {
        Some(if self.is_least_closure {
            self.bottom_type
        } else {
            self.top_type
        })
    }
}

/// `TypeSchemaEliminationVisitor.run(topType, bottomType, isLeastClosure,
/// schema)`.
///
/// Runs an instance of the visitor on the given [schema] and returns the
/// resulting type. If the schema contains no instances of `_`, the original
/// schema object is returned to avoid unnecessary allocation.
pub fn run(
    ctx: &Ctx<'_>,
    top_type: TypeId,
    bottom_type: TypeId,
    is_least_closure: bool,
    schema: TypeId,
) -> TypeId {
    let mut visitor = TypeSchemaEliminationVisitor {
        ctx: *ctx,
        top_type,
        bottom_type,
        is_least_closure,
    };
    let result = visitor.visit(schema);
    debug_assert_eq!(visitor.is_least_closure, is_least_closure);
    result.unwrap_or(schema)
}
