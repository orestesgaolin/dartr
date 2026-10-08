// Dart source: pkg/analyzer/lib/src/dart/resolver/element_binding_visitor.dart

//! `ElementBindingVisitor`: binds the linked fragments of the declarations
//! of a unit to their nodes (`ResolutionTables.declared_fragment`, walking
//! the fragments with [`crate::element_walker`] in declaration order), and
//! creates the fragments and elements of local declarations (local
//! variables, local functions, function expressions, formal parameters of
//! local functions, labels, catch parameters, ...) in the unit's local
//! arena (`ctx.local`).
//!
//! STUB (unit C1, element binding): [`bind_unit`] does nothing yet.

use dartr_ast::{Ast, CompilationUnit, Id};
use dartr_element::{Ctx, FId, LibraryFragment, ResolutionTables};

use crate::tables::ResolverTables;

/// Dart `unit.accept(ElementBindingVisitor.forAnalysis(fragment: fragment,
/// walker: ElementWalker.forCompilationUnit(fragment)))`.
pub fn bind_unit(
    ctx: &Ctx<'_>,
    ast: &Ast,
    unit: Id<CompilationUnit>,
    fragment: FId<LibraryFragment>,
    tables: &mut ResolutionTables,
    rt: &mut ResolverTables,
) {
    let _ = (ctx, ast, unit, fragment, tables, rt);
}
