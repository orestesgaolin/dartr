// Dart source: pkg/analyzer/lib/src/dart/ast/ast.dart (the resolution fields
// of the *Impl node classes: staticType, element, declaredFragment,
// staticInvokeType, typeArgumentTypes, readElement/writeElement, ...)

//! [`ResolutionTables`]: what the Dart resolver writes into AST nodes, as
//! side tables indexed by `NodeId` (design §3), so that the parsed AST stays
//! syntax only and shareable.

use dartr_ast::NodeMap;

use crate::data::DirectiveUri;
use crate::ids::FragmentId;
use crate::types::{ElemRef, TypeId, TypeList};

/// Pattern data (`DartPatternImpl.matchedValueType`,
/// `ListPatternImpl.requiredType`, `MapPatternImpl.requiredType`,
/// `PatternAssignmentImpl.patternTypeSchema`,
/// `PatternVariableDeclarationImpl.patternTypeSchema`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PatternInfo {
    pub matched_value_type: Option<TypeId>,
    pub required_type: Option<TypeId>,
    pub pattern_type_schema: Option<TypeId>,
}

/// Boolean resolution facts of a node. Units add named bits as they port
/// the Dart code that computes them (adding a bit does not change other
/// users).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct NodeFlags(pub u32);

impl NodeFlags {
    pub fn contains(self, flag: NodeFlags) -> bool {
        self.0 & flag.0 == flag.0
    }

    pub fn insert(&mut self, flag: NodeFlags) {
        self.0 |= flag.0;
    }
}

/// The resolution of one unit (or of the `ConstExprs` of one cycle).
/// Mirrors the fields that the Dart resolver sets on AST nodes. Every table
/// is keyed by the node the Dart field is on.
#[derive(Debug, Default)]
pub struct ResolutionTables {
    /// `Expression.staticType` (`ExpressionImpl._staticType`).
    pub static_type: NodeMap<TypeId>,
    /// `.element` of `SimpleIdentifier`, `NamedType`, `Annotation`,
    /// `ConstructorName`, `BinaryExpression`, `IndexExpression`,
    /// `PrefixExpression`, `PostfixExpression`, `PatternField`,
    /// `RelationalPattern`, `AssignedVariablePattern`,
    /// `ImportPrefixReference`, `SuperConstructorInvocation`,
    /// `RedirectingConstructorInvocation`, `DotShorthandConstructorInvocation`,
    /// `FunctionExpressionInvocation`, `LibraryDirective`, and
    /// `EnumConstantDeclaration.constructorElement`.
    pub element: NodeMap<ElemRef>,
    /// `Declaration.declaredFragment` (and `CompilationUnit`,
    /// `FormalParameter`, `DeclaredIdentifier`, `CatchClauseParameter`,
    /// `Label`, `GenericFunctionType`, `DeclaredVariablePattern`,
    /// `FunctionExpression`, `AnonymousMethodInvocation`).
    pub declared_fragment: NodeMap<FragmentId>,
    /// `InvocationExpression.staticInvokeType`,
    /// `BinaryExpression.staticInvokeType`.
    pub invoke_type: NodeMap<TypeId>,
    /// Inferred type arguments: `InvocationExpression.typeArgumentTypes`,
    /// `FunctionReference.typeArgumentTypes`,
    /// `ExtensionOverride.typeArgumentTypes`,
    /// `SimpleIdentifier.tearOffTypeArgumentTypes`.
    pub type_arg_types: NodeMap<TypeList>,
    /// Argument expression → corresponding parameter
    /// (`ArgumentList.correspondingStaticParameters`).
    pub param_element: NodeMap<ElemRef>,
    /// `TypeAnnotation.type` (`NamedType`, `GenericFunctionType`,
    /// `RecordTypeAnnotation`) and `FormalParameter.explicitFragmentType`.
    pub annotation_type: NodeMap<TypeId>,
    /// `CompoundAssignmentExpression.readElement`.
    pub read_element: NodeMap<ElemRef>,
    /// `CompoundAssignmentExpression.writeElement`.
    pub write_element: NodeMap<ElemRef>,
    /// `CompoundAssignmentExpression.readType`.
    pub read_type: NodeMap<TypeId>,
    /// `CompoundAssignmentExpression.writeType`.
    pub write_type: NodeMap<TypeId>,
    /// `ExtensionOverride.extendedType`.
    pub extended_type: NodeMap<TypeId>,
    /// `MethodInvocation._methodNameType`.
    pub method_name_type: NodeMap<TypeId>,
    /// `Configuration.resolvedUri`.
    pub resolved_uri: NodeMap<DirectiveUri>,
    /// Pattern data, see [`PatternInfo`].
    pub pattern_info: NodeMap<PatternInfo>,
    pub flags: NodeMap<NodeFlags>,
}

impl ResolutionTables {
    pub fn new() -> Self {
        Self::default()
    }
}
