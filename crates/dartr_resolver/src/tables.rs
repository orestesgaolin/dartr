// Dart source: pkg/analyzer/lib/src/dart/ast/ast.dart (resolution fields of
// the *Impl node classes that are not in dartr_element::ResolutionTables)

//! [`ResolverTables`]: per-node data that the resolver passes write into the
//! AST in Dart and that only the resolver reads (the public resolution
//! results are in [`dartr_element::ResolutionTables`]).
//!
//! Add a table here when a ported resolver file needs a Dart node field that
//! is neither a public resolution result nor computable from the syntax.
//! Name it after the Dart field and document the Dart field.

use dartr_ast::{NodeId, NodeMap};
use dartr_element::{EId, FragmentId, Name, PromotableElement};

use crate::scope::ScopeLookupResult;

/// Per-node resolver data of one unit.
#[derive(Debug, Default)]
pub struct ResolverTables {
    /// `SimpleIdentifierImpl.scopeLookupResult`: the lexical lookup of the
    /// identifier, set by the resolution visitor
    /// (`ResolutionVisitor.visitSimpleIdentifier`), read by the identifier
    /// resolvers of the `ResolverVisitor`.
    pub scope_lookup_result: NodeMap<ScopeLookupResult>,
    /// `AnnotationImpl.elementAnnotation`: the fragment that owns the
    /// metadata of an annotation node.
    pub element_annotation: NodeMap<FragmentId>,
    /// `DotShorthandMixin.isDotShorthand` (from the parser:
    /// `ParsedUnit.dot_shorthands`).
    pub dot_shorthand: NodeMap<()>,
    /// `GuardedPatternImpl.variables`: the pattern variables of the guarded
    /// pattern (bind variables and logical-or join variables), by name, in
    /// Dart map order. Keyed by the `GuardedPattern` node. Written by the
    /// resolution visitor (`ResolutionVisitor._resolveGuardedPattern`);
    /// when it is not written, the pattern resolver computes the list from
    /// `declared_fragment` of the `DeclaredVariablePattern`s and the `join`
    /// of their fragments (`pattern_resolver::guarded_pattern_variables`).
    pub guarded_pattern_variables: NodeMap<Vec<(Name, EId<PromotableElement>)>>,
    /// `SwitchStatementCaseGroup.variables`: the joined pattern variables of
    /// a group of switch members that share a body, by name, in Dart map
    /// order. Keyed by the last `SwitchMember` of the group. Written by the
    /// resolution visitor (`ResolutionVisitor.visitSwitchStatement`, the
    /// result of `switchStatementSharedCaseScopeFinish`); when it is not
    /// written, the pattern resolver computes it
    /// (`pattern_resolver::switch_group_variables`).
    pub switch_group_variables: NodeMap<Vec<(Name, EId<PromotableElement>)>>,
}

impl ResolverTables {
    pub fn new() -> Self {
        Self::default()
    }

    /// Dart `DotShorthandMixin.isDotShorthand` of [node].
    pub fn is_dot_shorthand(&self, node: NodeId) -> bool {
        self.dot_shorthand.get(node).is_some()
    }
}
