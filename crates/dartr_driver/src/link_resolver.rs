// Dart source: pkg/analyzer/lib/src/summary2/ast_resolver.dart
// (the `AstResolver` that the linker creates)

//! [`ResolverForLinking`]: the [`LinkResolver`] that the driver passes to
//! [`dartr_link::link::link_cycle`]. It resolves with
//! `dartr_resolver::ast_resolver` (Dart `AstResolver`). The two crates do
//! not depend on each other; this adapter connects them.
//!
//! The analysis options of the libraries are not known while linking
//! (Dart reads `builder.kind.file.analysisOptions`): the default options
//! are used. They only change diagnostics, which linking drops.

use dartr_element::{Ctx, TypeId};
use dartr_link::link::{ExpressionRequest, LinkResolver, LinkResolverSession};
use dartr_resolver::ast_resolver::{self, LinkResolution};
use dartr_resolver::options::AnalysisOptions;

/// The resolver of linking (see the module documentation).
pub struct ResolverForLinking;

impl LinkResolver for ResolverForLinking {
    fn session(&self) -> Box<dyn LinkResolverSession + '_> {
        Box::new(Session(LinkResolution::new()))
    }
}

struct Session(LinkResolution);

impl LinkResolverSession for Session {
    fn resolve_expression(&self, ctx: &Ctx<'_>, request: &ExpressionRequest<'_>) -> Option<TypeId> {
        self.0.resolve_expression(
            ctx,
            &ast_resolver::ExpressionRequest {
                library: request.library,
                fragment: request.fragment,
                parsed: request.parsed,
                declared_fragments: request.declared_fragments,
                options: AnalysisOptions::default(),
                owner: request.owner,
                enclosing_instance: request.enclosing_instance,
                enclosing_class: request.enclosing_class,
                context_type: request.context_type,
                in_scope_primary_constructor_parameters: request
                    .in_scope_primary_constructor_parameters,
            },
        )
    }
}
