// Dart source: pkg/analyzer/lib/src/dart/resolver/resolution_visitor.dart

//! `ResolutionVisitor`: the pass before the `ResolverVisitor`. It walks the
//! unit with the lexical scopes ([`crate::scope_context`]), resolves type
//! annotations ([`crate::named_type_resolver`]), sets the types of
//! declarations with explicit types (local variables, formal parameters),
//! rewrites nodes whose meaning depends on the scope
//! ([`crate::ast_rewrite`]), and records the scope lookup result of each
//! `SimpleIdentifier` (`ResolverTables.scope_lookup_result`) and the element
//! of each identifier that refers to a promotable local
//! (`ResolutionTables.element`).
//!
//! STUB (unit C1, resolution visitor): [`resolve_unit`] does nothing yet.

use dartr_ast::{Ast, CompilationUnit, Id};
use dartr_diagnostics::Diagnostic;
use dartr_element::{Ctx, ResolutionTables};

use crate::resolver::UnitContext;
use crate::tables::ResolverTables;

/// Dart `unit.accept(ResolutionVisitor(libraryFragment: ..., nameScope:
/// libraryFragment.scope, ...))`.
pub fn resolve_unit(
    ctx: &Ctx<'_>,
    unit_ctx: UnitContext<'_>,
    ast: &mut Ast,
    unit: Id<CompilationUnit>,
    tables: &mut ResolutionTables,
    rt: &mut ResolverTables,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let _ = (ctx, unit_ctx, ast, unit, tables, rt, diagnostics);
}
