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
use dartr_element::{ElementId, FragmentId, Name};
use indexmap::{IndexMap, IndexSet};

use crate::scope::ScopeLookupResult;

/// Per-node resolver data of one unit.
#[derive(Debug, Default)]
pub struct ResolverTables {
    /// The lookup of the name of the property or method of `this.name` in
    /// the scope of each `ThisExpression` target (Dart
    /// `resolveNameInScope` of the linter, read by `unnecessary_this`):
    /// the getter and the setter.
    pub this_scope_lookup: NodeMap<(
        Option<dartr_element::ElementId>,
        Option<dartr_element::ElementId>,
    )>,
    /// `SimpleIdentifierImpl.scopeLookupResult`: the lexical lookup of the
    /// identifier, set by the resolution visitor
    /// (`ResolutionVisitor.visitSimpleIdentifier`), read by the identifier
    /// resolvers of the `ResolverVisitor`.
    pub scope_lookup_result: NodeMap<ScopeLookupResult>,
    /// `AnnotationImpl.elementAnnotation`: the fragment that owns the
    /// metadata of an annotation node (the library fragment of the unit for
    /// the annotations of directives and annotations without a fragment).
    /// Set by the element binding visitor.
    pub element_annotation: NodeMap<FragmentId>,
    /// `BreakStatementImpl.target` / `ContinueStatementImpl.target`: the
    /// statement (or switch member) that the jump goes to. Set by the
    /// resolution visitor (`_lookupBreakOrContinueTarget`).
    pub break_continue_target: NodeMap<NodeId>,
    /// `LocalVariableInfo.potentiallyMutatedInScope` (one
    /// `LocalVariableInfo` per unit, shared by the function bodies): the
    /// promotable elements that are assigned somewhere. Set by the
    /// resolution visitor.
    pub potentially_mutated_in_scope: IndexSet<ElementId>,
    /// `GuardedPatternImpl.variables`: the pattern variables of a guarded
    /// pattern by name (Dart `Map<String, PatternVariableElementImpl>`).
    pub guarded_pattern_variables: NodeMap<IndexMap<Name, ElementId>>,
    /// `SwitchStatementCaseGroup.variables`: the joined variables of a
    /// group of switch members that share their statements, keyed by the
    /// last member of the group (the member with the statements).
    pub switch_case_group_variables: NodeMap<IndexMap<Name, ElementId>>,
    /// `PatternVariableDeclarationImpl.elements`: the bind pattern variables
    /// of a pattern variable declaration.
    pub pattern_variable_declaration_elements: NodeMap<Vec<ElementId>>,
    /// `ForEachPartsWithPatternImpl.variables`: the bind pattern variables
    /// of a for-each pattern.
    pub for_each_pattern_variables: NodeMap<Vec<ElementId>>,
    /// `DotShorthandMixin.isDotShorthand` (from the parser:
    /// `ParsedUnit.dot_shorthands`).
    pub dot_shorthand: NodeMap<()>,
    /// `FunctionBodyImpl.bodyContext`: the body inference context of a
    /// block or expression function body, kept after the body is resolved
    /// (read by `checkForBodyMayCompleteNormally`).
    pub body_context: NodeMap<crate::body_inference_context::BodyInferenceContext>,
    /// The type of `ExpressionImpl.correspondingParameter` of an argument
    /// expression (keyed like `ResolutionTables::param_element`): the
    /// parameter type of the invoked (instantiated) function type. Dart
    /// reads it from the parameter member; `param_element` keeps the
    /// declaring element.
    pub corresponding_parameter_type: NodeMap<dartr_element::TypeId>,
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
