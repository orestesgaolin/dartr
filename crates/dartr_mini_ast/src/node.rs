// Dart source: pkg/_fe_analyzer_shared/test/mini_ast.dart (the AST classes,
// the top-level builder functions and the Proto* mixins)

//! The AST of the pseudo-Dart language used by the type analyzer tests.
//!
//! # Representation
//!
//! Dart allocates an object per AST node; the classes form a hierarchy
//! (`Node` > `Expression`, `Statement`, `Pattern`, `CollectionElement`, ...).
//! Here every node is a [`Node`]: a `Copy` handle into a `thread_local!`
//! arena of [`NodeData`], whose [`NodeKind`] says which Dart class it is.
//! Statements, expressions and patterns are all [`Node`]s, so the type
//! analyzer uses `Node` for its `Node`, `Statement`, `Expression` and
//! `Pattern` types. Variables ([`Var`], Dart class `Var` and
//! `PatternVariableJoin`) are handles into a second table.
//!
//! Rust runs each test on its own thread, so each test has its own arena.
//!
//! # Builders
//!
//! The Dart top-level builder functions (`expr`, `if_`, `switch_`, ...) are
//! free functions with the same names in snake case. Dart named parameters
//! with defaults become `with_...` methods on the result (for example
//! `declare(x).with_type("int").with_initializer(expr("int"))` for Dart
//! `declare(x, type: 'int', initializer: expr('int'))`). Dart optional
//! positional parameters become `Option` parameters, or a second function
//! (`if_else` for `if_` with an `else` branch).
//!
//! The Dart `ProtoExpression`/`ProtoStatement`/`ProtoCollectionElement`
//! conversions (`asExpression`, `asStatement`, `asCollectionElement`,
//! `asSwitchHead`) are done by the builders, by looking at the [`NodeKind`]
//! of their arguments. A [`Var`] used as an expression must be converted
//! with [`Var::expr`] (Dart does it implicitly).
//!
//! # Locations
//!
//! Dart computes the source location of each node from the stack trace
//! (`computeLocation`). Here the builders are `#[track_caller]`, and the
//! location is the caller's `file:line:column`.
//!
//! # `toString`
//!
//! Dart node `toString` methods are only used in failure messages, except
//! for `PatternVariableJoin.toString` (used in IR and error strings), which
//! is ported as [`Var::string_to_check_variables`]. `Debug` of a node prints
//! its kind and location.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt;
use std::panic::Location;
use std::rc::Rc;

use dartr_flow::flow_analysis_operations::PropertyNonPromotabilityReason;
use dartr_flow::shared_type::{SharedTypeSchemaView, SharedTypeView};
use dartr_flow::type_analysis_result::ExpressionTypeAnalysisResult;
use dartr_flow::type_analyzer::JoinedPatternVariableInconsistency;

use super::mini_types::{Name, Type, intern};

/// A source location in the test file (Dart `String location`).
pub type Loc = &'static Location<'static>;

/// Formats a location as Dart does (`file:line:column`).
pub fn loc_str(loc: Loc) -> String {
    format!("{}:{}:{}", loc.file(), loc.line(), loc.column())
}

/// The result of analyzing an expression in the mini-AST.
pub type ExprResult = ExpressionTypeAnalysisResult<Type, ExprInfo>;

/// The flow analysis `ExpressionInfo` of the mini AST.
pub type ExprInfo =
    dartr_flow::flow_analysis_impl::model::ExpressionInfo<super::operations::MiniAstTypes>;

/// The result passed to a `checkExpressionTypeAnalysisResult` checker.
///
/// Dart passes the result object, and tests downcast it to
/// `AwaitExpressionResult` or `IntTypeAnalysisResult`. Here the subclass
/// fields are optional fields.
#[derive(Clone, Debug)]
pub struct ExprResultDetail {
    /// The result.
    pub result: ExprResult,
    /// `AwaitExpressionResult.operandType`, for an `await` expression.
    pub operand_type: Option<Type>,
    /// `IntTypeAnalysisResult.convertedToDouble`, for an integer literal.
    pub converted_to_double: Option<bool>,
    /// `PatternAssignmentAnalysisResult.patternSchema`, for a pattern
    /// assignment.
    pub pattern_schema: Option<Type>,
}

/// The result passed to a `checkStatementTypeAnalysisResult` checker.
///
/// Dart passes the result object, and tests downcast it to
/// `YieldStatementResult`. Here the subclass field is optional.
#[derive(Clone, Debug, Default)]
pub struct StmtResultDetail {
    /// `YieldStatementResult.operandType`, for a `yield` statement.
    pub operand_type: Option<Type>,
}

/// A checker of a statement analysis result
/// (`checkStatementTypeAnalysisResult`).
pub type StmtResultChecker = Rc<dyn Fn(&StmtResultDetail)>;

/// A checker of an expression analysis result
/// (`checkExpressionTypeAnalysisResult`).
pub type ExprResultChecker = Rc<dyn Fn(&ExprResultDetail)>;

/// A callback receiving the promoted type of a variable read
/// (`readAndCheckPromotedType`).
pub type PromotedTypeCallback = Rc<dyn Fn(Option<Type>)>;

/// A callback receiving the flow analysis `ExpressionInfo` of an expression
/// (`getExpressionInfo`).
pub type ExpressionInfoCallback = Rc<dyn Fn(Option<ExprInfo>)>;

/// A callback receiving an [`SsaNodeHarness`](crate::flow_analysis_mini_ast::SsaNodeHarness)
/// (`getSsaNodes`).
pub type SsaNodesCallback = Rc<dyn Fn(&crate::flow_analysis_mini_ast::SsaNodeHarness)>;

/// A non-promotion reason of the mini AST flow analysis.
pub type NonPromotionReason = dartr_flow::flow_analysis::NonPromotionReasonOf<
    dartr_flow::flow_analysis_impl::FlowAnalysisImpl<super::operations::MiniAstTypes>,
>;

/// The result of `whyNotPromoted`: (type, reason) pairs in Dart map order.
pub type WhyNotPromotedMap = Vec<(SharedTypeView<Type>, NonPromotionReason)>;

/// A callback receiving the non-promotion reasons (`whyNotPromoted`,
/// `implicitThis_whyNotPromoted`).
pub type WhyNotPromotedCallback = Rc<dyn Fn(WhyNotPromotedMap)>;

// ===================================================================== arena

thread_local! {
    static NODES: RefCell<Vec<NodeData>> = const { RefCell::new(Vec::new()) };
    static VARS: RefCell<Vec<VarData>> = const { RefCell::new(Vec::new()) };
    /// Dart `Node._nodesWithUnusedErrorIds`.
    static UNUSED_ERROR_IDS: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
}

/// A node of the mini-AST (Dart `Node` and its subclasses).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Node(u32);

/// The data of a [`Node`].
#[derive(Clone)]
pub struct NodeData {
    /// Where the node was created.
    pub location: Loc,
    /// The Dart class and fields of the node.
    pub kind: NodeKind,
    /// `_errorId`.
    pub error_id: Option<String>,
    /// `_expectedIR`.
    pub expected_ir: Option<String>,
    /// `_expectedSchema` (expressions).
    pub expected_schema: Option<String>,
    /// `_expectedType` (expressions).
    pub expected_type: Option<String>,
    /// `_checkExpressionTypeAnalysisResult`.
    pub check_expression_result: Option<ExprResultChecker>,
    /// `_checkStatementTypeAnalysisResult`.
    pub check_statement_result: Option<StmtResultChecker>,
}

/// A label (Dart `BoundLabel` / `UnboundLabel`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Label(Node);

/// A collection element context (Dart `CollectionElementContext`).
#[derive(Clone, Copy, Debug)]
pub enum CollectionElementContext {
    /// `CollectionElementContextMapEntry`.
    MapEntry {
        /// The key type.
        key_type: Type,
        /// The value type.
        value_type: Type,
    },
    /// `CollectionElementContextType`.
    Type {
        /// The element type schema.
        element_type_schema: SharedTypeSchemaView<Type>,
    },
}

/// A catch clause of a try statement (Dart `CatchClause`).
#[derive(Clone, Debug)]
pub struct CatchClause {
    /// The body.
    pub body: Node,
    /// `on` type.
    pub exception_type: Option<Type>,
    /// The exception variable.
    pub exception: Option<Var>,
    /// The stack trace variable.
    pub stack_trace: Option<Var>,
}

/// Something that might be promoted (Dart `Promotable`).
#[derive(Clone, Copy, Debug)]
pub enum Promotable {
    /// A variable.
    Var(Var),
    /// `this`, a property or `this`/`super` property (a node).
    Node(Node),
}

impl From<Var> for Promotable {
    fn from(v: Var) -> Self {
        Promotable::Var(v)
    }
}

impl From<Node> for Promotable {
    fn from(n: Node) -> Self {
        Promotable::Node(n)
    }
}

/// The Dart class of a node and its fields.
#[derive(Clone)]
#[allow(missing_docs)]
pub enum NodeKind {
    // ----------------------------------------------------------- expressions
    As {
        target: Node,
        ty: Type,
    },
    Await {
        operand: Node,
    },
    BooleanLiteral {
        value: bool,
    },
    Cascade {
        target: Node,
        sections: Vec<Node>,
        is_null_aware: bool,
    },
    CascadePlaceholder,
    CheckAssigned {
        variable: Var,
        expected: bool,
    },
    CheckPromoted {
        promotable: Promotable,
        expected: Option<String>,
    },
    CheckPromotionChain {
        promotable: Promotable,
        expected: Vec<String>,
    },
    CheckReachable {
        expected: bool,
    },
    CheckUnassigned {
        variable: Var,
        expected: bool,
    },
    Conditional {
        condition: Node,
        if_true: Node,
        if_false: Node,
    },
    DotShorthand {
        expr: Node,
    },
    DotShorthandHead {
        name: String,
    },
    Equal {
        lhs: Node,
        rhs: Node,
        is_inverted: bool,
    },
    IfNull {
        lhs: Node,
        rhs: Node,
    },
    IntLiteral {
        value: i64,
    },
    InvokeAnonymousMethod {
        target: Node,
        body: Node,
        return_type: Type,
        is_null_aware: bool,
        is_parameterless: bool,
        parameter: Option<Var>,
    },
    InvokeMethod {
        target: Node,
        method_name: String,
        arguments: Vec<Node>,
        is_null_aware: bool,
    },
    Is {
        target: Node,
        ty: Type,
        is_inverted: bool,
    },
    ListLiteral {
        elements: Vec<Node>,
        element_type: Type,
    },
    LocalFunction {
        body: Node,
        ty: Type,
    },
    Logical {
        lhs: Node,
        rhs: Node,
        is_and: bool,
    },
    MapLiteral {
        elements: Vec<Node>,
        key_type: Type,
        value_type: Type,
    },
    NonNullAssert {
        operand: Node,
    },
    Not {
        operand: Node,
    },
    NullLiteral,
    ParenthesizedExpression {
        expr: Node,
    },
    PatternAssignment {
        lhs: Node,
        rhs: Node,
    },
    PlaceholderExpression {
        ty: Type,
    },
    PostIncDec {
        lhs: Node,
    },
    PreIncDec {
        lhs: Node,
    },
    Property {
        target: Node,
        property_name: String,
        is_null_aware: bool,
    },
    Second {
        first: Node,
        second: Node,
    },
    SwitchExpression {
        scrutinee: Node,
        cases: Vec<Node>,
    },
    This,
    ThisOrSuperProperty {
        property_name: String,
        is_super_access: bool,
    },
    Throw {
        operand: Node,
    },
    VariableReference {
        variable: Var,
        callback: Option<PromotedTypeCallback>,
    },
    WrappedExpression {
        before: Option<Node>,
        expr: Node,
        after: Option<Node>,
    },
    Write {
        lhs: Node,
        rhs: Node,
    },
    // flow_analysis_mini_ast.dart
    /// `_GetExpressionInfo`.
    GetExpressionInfo {
        target: Node,
        callback: ExpressionInfoCallback,
    },
    /// `_GetSsaNodes`.
    GetSsaNodes {
        callback: SsaNodesCallback,
    },
    /// `_WhyNotPromoted`.
    WhyNotPromoted {
        target: Node,
        callback: WhyNotPromotedCallback,
    },
    /// `_WhyNotPromoted_ImplicitThis`.
    WhyNotPromotedImplicitThis {
        static_type: Type,
        callback: WhyNotPromotedCallback,
    },

    // ------------------------------------------------------------ statements
    Assert {
        condition: Node,
        message: Option<Node>,
    },
    Block {
        statements: Vec<Node>,
    },
    Break {
        target: Option<Label>,
    },
    Continue {
        target: Option<Label>,
    },
    Do {
        body: Node,
        condition: Node,
    },
    ExpressionInTypeSchema {
        expr: Node,
        type_schema: SharedTypeSchemaView<Type>,
    },
    ExpressionStatement {
        expr: Node,
    },
    For {
        initializer: Option<Node>,
        condition: Option<Node>,
        updater: Option<Node>,
        body: Node,
        for_collection: bool,
    },
    ForEach {
        variable: Option<Var>,
        iterable: Node,
        body: Node,
        declares_variable: bool,
    },
    If {
        condition: Node,
        if_true: Node,
        if_false: Option<Node>,
    },
    IfCase {
        expression: Node,
        pattern: Node,
        guard: Option<Node>,
        if_true: Node,
        if_false: Option<Node>,
        candidate_variables: Vec<(Name, Var)>,
    },
    LabeledStatement {
        labels: Vec<Label>,
        body: Node,
    },
    PatternForIn {
        pattern: Node,
        expression: Node,
        body: Node,
        has_await: bool,
    },
    PatternVariableDeclaration {
        pattern: Node,
        initializer: Node,
        is_final: bool,
    },
    Return,
    SwitchStatement {
        scrutinee: Node,
        cases: Vec<Node>,
        is_legacy_exhaustive: Option<bool>,
        expect_has_default: Option<bool>,
        expect_is_exhaustive: Option<bool>,
        expect_last_case_terminates: Option<bool>,
        expect_requires_exhaustiveness_validation: Option<bool>,
        expect_scrutinee_type: Option<String>,
    },
    TryStatement {
        body: Node,
        catches: Vec<CatchClause>,
        finally_statement: Option<Node>,
    },
    VariableDeclaration {
        variable: Var,
        is_late: bool,
        is_final: bool,
        declared_type: Option<Type>,
        initializer: Option<Node>,
        expect_inferred_type: Option<String>,
    },
    While {
        condition: Node,
        body: Node,
    },
    YieldStatement {
        operand: Node,
        is_yield_star: bool,
    },

    // --------------------------------------------------- collection elements
    ExpressionCollectionElement {
        expression: Node,
    },
    IfCaseElement {
        expression: Node,
        pattern: Node,
        guard: Option<Node>,
        if_true: Node,
        if_false: Option<Node>,
        variables: Vec<(Name, Var)>,
    },
    IfElement {
        condition: Node,
        if_true: Node,
        if_false: Option<Node>,
    },
    MapEntry {
        key: Node,
        value: Node,
        is_key_null_aware: bool,
    },
    PatternForInElement {
        pattern: Node,
        expression: Node,
        body: Node,
        has_await: bool,
    },

    // -------------------------------------------------------------- patterns
    CastPattern {
        inner: Node,
        ty: Type,
    },
    ConstantPattern {
        constant: Node,
    },
    ListPattern {
        element_type: Option<Type>,
        elements: Vec<Node>,
    },
    LogicalAndPattern {
        lhs: Node,
        rhs: Node,
    },
    LogicalOrPattern {
        lhs: Node,
        rhs: Node,
    },
    MapPattern {
        type_arguments: Option<(Type, Type)>,
        elements: Vec<Node>,
    },
    NullCheckOrAssertPattern {
        inner: Node,
        is_assert: bool,
    },
    ObjectPattern {
        required_type: Type,
        fields: Vec<Node>,
    },
    ParenthesizedPattern {
        inner: Node,
    },
    RecordPattern {
        fields: Vec<Node>,
    },
    RelationalPattern {
        operator: String,
        operand: Node,
    },
    VariablePattern {
        declared_type: Option<Type>,
        variable: Var,
        expect_inferred_type: Option<String>,
        is_assigned_variable: Option<bool>,
    },
    WildcardPattern {
        declared_type: Option<Type>,
        expect_inferred_type: Option<String>,
    },

    // ----------------------------------------------------------------- other
    /// `Node.placeholder()`.
    Placeholder,
    ExpressionCase {
        guarded_pattern: Option<Node>,
        expression: Node,
    },
    GuardedPattern {
        pattern: Node,
        guard: Option<Node>,
        variables: Option<Vec<(Name, Var)>>,
    },
    MapPatternEntry {
        key: Node,
        value: Node,
    },
    RecordPatternField {
        name: Option<Name>,
        pattern: Node,
    },
    RestPattern {
        sub_pattern: Option<Node>,
    },
    SwitchHeadCase {
        guarded_pattern: Node,
    },
    SwitchHeadDefault,
    SwitchStatementMember {
        elements: Vec<Node>,
        body: Node,
        has_labels: bool,
        candidate_variables: Option<Vec<(Name, Var)>>,
    },
    BoundLabel {
        name: String,
        binding: Option<Node>,
    },
    UnboundLabel,
}

/// The Dart superclass of a node kind.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    /// `Expression`.
    Expression,
    /// `Statement`.
    Statement,
    /// `CollectionElement`.
    CollectionElement,
    /// `Pattern`.
    Pattern,
    /// Any other node (`GuardedPattern`, `SwitchHead`, `RestPattern`, ...).
    Other,
}

impl NodeKind {
    /// The Dart superclass of this kind.
    pub fn category(&self) -> Category {
        use NodeKind::*;
        match self {
            As { .. }
            | Await { .. }
            | BooleanLiteral { .. }
            | Cascade { .. }
            | CascadePlaceholder
            | CheckAssigned { .. }
            | CheckPromoted { .. }
            | CheckPromotionChain { .. }
            | CheckReachable { .. }
            | CheckUnassigned { .. }
            | Conditional { .. }
            | DotShorthand { .. }
            | DotShorthandHead { .. }
            | Equal { .. }
            | IfNull { .. }
            | IntLiteral { .. }
            | InvokeAnonymousMethod { .. }
            | InvokeMethod { .. }
            | Is { .. }
            | ListLiteral { .. }
            | LocalFunction { .. }
            | Logical { .. }
            | MapLiteral { .. }
            | NonNullAssert { .. }
            | Not { .. }
            | NullLiteral
            | ParenthesizedExpression { .. }
            | PatternAssignment { .. }
            | PlaceholderExpression { .. }
            | PostIncDec { .. }
            | PreIncDec { .. }
            | Property { .. }
            | Second { .. }
            | SwitchExpression { .. }
            | This
            | ThisOrSuperProperty { .. }
            | Throw { .. }
            | VariableReference { .. }
            | WrappedExpression { .. }
            | Write { .. }
            | GetExpressionInfo { .. }
            | GetSsaNodes { .. }
            | WhyNotPromoted { .. }
            | WhyNotPromotedImplicitThis { .. } => Category::Expression,
            Assert { .. }
            | Block { .. }
            | Break { .. }
            | Continue { .. }
            | Do { .. }
            | ExpressionInTypeSchema { .. }
            | ExpressionStatement { .. }
            | For { .. }
            | ForEach { .. }
            | If { .. }
            | IfCase { .. }
            | LabeledStatement { .. }
            | PatternForIn { .. }
            | PatternVariableDeclaration { .. }
            | Return
            | SwitchStatement { .. }
            | TryStatement { .. }
            | VariableDeclaration { .. }
            | While { .. }
            | YieldStatement { .. } => Category::Statement,
            ExpressionCollectionElement { .. }
            | IfCaseElement { .. }
            | IfElement { .. }
            | MapEntry { .. }
            | PatternForInElement { .. } => Category::CollectionElement,
            CastPattern { .. }
            | ConstantPattern { .. }
            | ListPattern { .. }
            | LogicalAndPattern { .. }
            | LogicalOrPattern { .. }
            | MapPattern { .. }
            | NullCheckOrAssertPattern { .. }
            | ObjectPattern { .. }
            | ParenthesizedPattern { .. }
            | RecordPattern { .. }
            | RelationalPattern { .. }
            | VariablePattern { .. }
            | WildcardPattern { .. } => Category::Pattern,
            _ => Category::Other,
        }
    }

    /// The name of the Dart class (for failure messages).
    pub fn class_name(&self) -> &'static str {
        use NodeKind::*;
        match self {
            As { .. } => "As",
            Await { .. } => "AwaitExpression",
            BooleanLiteral { .. } => "BooleanLiteral",
            Cascade { .. } => "Cascade",
            CascadePlaceholder => "CascadePlaceholder",
            CheckAssigned { .. } => "CheckAssigned",
            CheckPromoted { .. } => "CheckPromoted",
            CheckPromotionChain { .. } => "CheckPromotionChain",
            CheckReachable { .. } => "CheckReachable",
            CheckUnassigned { .. } => "CheckUnassigned",
            Conditional { .. } => "Conditional",
            DotShorthand { .. } => "DotShorthand",
            DotShorthandHead { .. } => "DotShorthandHead",
            Equal { .. } => "Equal",
            IfNull { .. } => "IfNull",
            IntLiteral { .. } => "IntLiteral",
            InvokeAnonymousMethod { .. } => "InvokeAnonymousMethod",
            InvokeMethod { .. } => "InvokeMethod",
            Is { .. } => "Is",
            ListLiteral { .. } => "ListLiteral",
            LocalFunction { .. } => "LocalFunction",
            Logical { .. } => "Logical",
            MapLiteral { .. } => "MapLiteral",
            NonNullAssert { .. } => "NonNullAssert",
            Not { .. } => "Not",
            NullLiteral => "NullLiteral",
            ParenthesizedExpression { .. } => "ParenthesizedExpression",
            PatternAssignment { .. } => "PatternAssignment",
            PlaceholderExpression { .. } => "PlaceholderExpression",
            PostIncDec { .. } => "PostIncDec",
            GetExpressionInfo { .. } => "_GetExpressionInfo",
            GetSsaNodes { .. } => "_GetSsaNodes",
            WhyNotPromoted { .. } => "_WhyNotPromoted",
            WhyNotPromotedImplicitThis { .. } => "_WhyNotPromoted_ImplicitThis",
            PreIncDec { .. } => "PreIncDec",
            Property { .. } => "Property",
            Second { .. } => "Second",
            SwitchExpression { .. } => "SwitchExpression",
            This => "This",
            ThisOrSuperProperty { .. } => "ThisOrSuperProperty",
            Throw { .. } => "Throw",
            VariableReference { .. } => "VariableReference",
            WrappedExpression { .. } => "WrappedExpression",
            Write { .. } => "Write",
            Assert { .. } => "Assert",
            Block { .. } => "Block",
            Break { .. } => "Break",
            Continue { .. } => "Continue",
            Do { .. } => "Do",
            ExpressionInTypeSchema { .. } => "ExpressionInTypeSchema",
            ExpressionStatement { .. } => "ExpressionStatement",
            For { .. } => "For",
            ForEach { .. } => "ForEach",
            If { .. } => "If",
            IfCase { .. } => "IfCase",
            LabeledStatement { .. } => "LabeledStatement",
            PatternForIn { .. } => "PatternForIn",
            PatternVariableDeclaration { .. } => "PatternVariableDeclaration",
            Return => "Return",
            SwitchStatement { .. } => "SwitchStatement",
            TryStatement { .. } => "TryStatementImpl",
            VariableDeclaration { .. } => "VariableDeclaration",
            While { .. } => "While",
            YieldStatement { .. } => "YieldStatement",
            ExpressionCollectionElement { .. } => "ExpressionCollectionElement",
            IfCaseElement { .. } => "IfCaseElement",
            IfElement { .. } => "IfElement",
            MapEntry { .. } => "MapEntry",
            PatternForInElement { .. } => "PatternForInElement",
            CastPattern { .. } => "CastPattern",
            ConstantPattern { .. } => "ConstantPattern",
            ListPattern { .. } => "ListPattern",
            LogicalAndPattern { .. } => "LogicalAndPattern",
            LogicalOrPattern { .. } => "LogicalOrPattern",
            MapPattern { .. } => "MapPattern",
            NullCheckOrAssertPattern { .. } => "NullCheckOrAssertPattern",
            ObjectPattern { .. } => "ObjectPattern",
            ParenthesizedPattern { .. } => "ParenthesizedPattern",
            RecordPattern { .. } => "RecordPattern",
            RelationalPattern { .. } => "RelationalPattern",
            VariablePattern { .. } => "VariablePattern",
            WildcardPattern { .. } => "WildcardPattern",
            Placeholder => "Node",
            ExpressionCase { .. } => "ExpressionCase",
            GuardedPattern { .. } => "GuardedPattern",
            MapPatternEntry { .. } => "MapPatternEntry",
            RecordPatternField { .. } => "RecordPatternField",
            RestPattern { .. } => "RestPattern",
            SwitchHeadCase { .. } => "SwitchHeadCase",
            SwitchHeadDefault => "SwitchHeadDefault",
            SwitchStatementMember { .. } => "SwitchStatementMember",
            BoundLabel { .. } => "BoundLabel",
            UnboundLabel => "UnboundLabel",
        }
    }
}

impl Node {
    /// Allocates a node (Dart `Node._`).
    pub(crate) fn alloc(kind: NodeKind, location: Loc) -> Node {
        NODES.with(|nodes| {
            let mut nodes = nodes.borrow_mut();
            let id = nodes.len() as u32;
            nodes.push(NodeData {
                location,
                kind,
                error_id: None,
                expected_ir: None,
                expected_schema: None,
                expected_type: None,
                check_expression_result: None,
                check_statement_result: None,
            });
            Node(id)
        })
    }

    /// `Node.placeholder()`.
    #[track_caller]
    pub fn placeholder() -> Node {
        Node::alloc(NodeKind::Placeholder, Location::caller())
    }

    /// A copy of the node's data.
    pub fn data(self) -> NodeData {
        NODES.with(|nodes| nodes.borrow()[self.0 as usize].clone())
    }

    /// The node's kind.
    pub fn kind(self) -> NodeKind {
        self.data().kind
    }

    /// Updates the node's data.
    pub fn update(self, f: impl FnOnce(&mut NodeData)) {
        NODES.with(|nodes| f(&mut nodes.borrow_mut()[self.0 as usize]));
    }

    /// Updates the node's kind.
    pub fn update_kind(self, f: impl FnOnce(&mut NodeKind)) {
        self.update(|data| f(&mut data.kind));
    }

    /// `location`.
    pub fn location(self) -> String {
        loc_str(self.data().location)
    }

    /// The Dart superclass of the node.
    pub fn category(self) -> Category {
        self.kind().category()
    }

    /// `node.errorId` (getter): fails if no error ID was assigned.
    pub fn error_id_value(self) -> String {
        let data = self.data();
        UNUSED_ERROR_IDS.with(|ids| ids.borrow_mut().remove(&format!("N{}", self.0)));
        match data.error_id {
            Some(error_id) => error_id,
            None => panic!(
                "No error ID assigned for {} at {}",
                data.kind.class_name(),
                loc_str(data.location)
            ),
        }
    }

    /// `node.errorId = value` (Dart cascade `..errorId = 'X'`).
    pub fn error_id(self, value: &str) -> Node {
        self.update(|data| data.error_id = Some(value.to_string()));
        UNUSED_ERROR_IDS.with(|ids| ids.borrow_mut().insert(format!("N{}", self.0)));
        self
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let data = self.data();
        write!(
            f,
            "{}#{} at {}",
            data.kind.class_name(),
            self.0,
            loc_str(data.location)
        )
    }
}

/// Error IDs that were assigned but never used (Dart
/// `Node._nodesWithUnusedErrorIds`), and clears the set.
pub fn take_unused_error_ids() -> Vec<String> {
    let keys: Vec<String> = UNUSED_ERROR_IDS
        .with(|ids| std::mem::take(&mut *ids.borrow_mut()))
        .into_iter()
        .collect();
    keys.into_iter()
        .map(|key| {
            let id: u32 = key[1..].parse().unwrap();
            if key.starts_with('N') {
                Node(id).data().error_id.unwrap()
            } else {
                Var(id).data().error_id.unwrap()
            }
        })
        .collect()
}

// ================================================================ variables

/// A local variable (Dart `Var`), or a join of pattern variables (Dart
/// `PatternVariableJoin`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Var(u32);

/// The data of a [`Var`].
#[derive(Clone)]
pub struct VarData {
    /// `name`.
    pub name: Name,
    /// `isFinal`.
    pub is_final: bool,
    /// `_type`.
    pub ty: Option<Type>,
    /// `identity`.
    pub identity: String,
    /// `_joinedVar`.
    pub joined_var: Option<Var>,
    /// Where the variable was created.
    pub location: Loc,
    /// `_errorId`.
    pub error_id: Option<String>,
    /// The `PatternVariableJoin` data, if this is a join.
    pub join: Option<JoinData>,
}

/// The fields of a Dart `PatternVariableJoin`.
#[derive(Clone)]
pub struct JoinData {
    /// `expectedComponents`.
    pub expected_components: Vec<Var>,
    /// `inconsistency`.
    pub inconsistency: JoinedPatternVariableInconsistency,
    /// `isJoined`.
    pub is_joined: bool,
}

impl Var {
    fn alloc(name: &str, location: Loc, join: Option<JoinData>) -> Var {
        VARS.with(|vars| {
            let mut vars = vars.borrow_mut();
            let id = vars.len() as u32;
            vars.push(VarData {
                name: intern(name),
                is_final: false,
                ty: None,
                identity: name.to_string(),
                joined_var: None,
                location,
                error_id: None,
                join,
            });
            Var(id)
        })
    }

    /// `Var(name)`.
    #[track_caller]
    pub fn new(name: &str) -> Var {
        Var::alloc(name, Location::caller(), None)
    }

    /// `PatternVariableJoin(name, expectedComponents: ...)`.
    #[track_caller]
    pub fn join(name: &str, expected_components: Vec<Var>) -> Var {
        let joined = Var::alloc(
            name,
            Location::caller(),
            Some(JoinData {
                expected_components: expected_components.clone(),
                inconsistency: JoinedPatternVariableInconsistency::None,
                is_joined: false,
            }),
        );
        for component in expected_components {
            assert!(component.data().joined_var.is_none());
            component.update(|d| d.joined_var = Some(joined));
        }
        joined
    }

    /// A copy of the variable's data.
    pub fn data(self) -> VarData {
        VARS.with(|vars| vars.borrow()[self.0 as usize].clone())
    }

    /// Updates the variable's data.
    pub fn update(self, f: impl FnOnce(&mut VarData)) {
        VARS.with(|vars| f(&mut vars.borrow_mut()[self.0 as usize]));
    }

    /// Dart named parameter `isFinal: true`.
    pub fn with_final(self, is_final: bool) -> Var {
        self.update(|d| d.is_final = is_final);
        self
    }

    /// Dart named parameter `identity:`.
    pub fn with_identity(self, identity: &str) -> Var {
        self.update(|d| d.identity = identity.to_string());
        self
    }

    /// Sets the type before analysis (Dart `x.type = ...` / `x._type = ...`
    /// in a test, e.g. for a parameter).
    pub fn with_type(self, ty: &str) -> Var {
        let ty = Type::parse(ty);
        self.update(|d| d.ty = Some(ty));
        self
    }

    /// `node.errorId = value`.
    pub fn error_id(self, value: &str) -> Var {
        self.update(|d| d.error_id = Some(value.to_string()));
        UNUSED_ERROR_IDS.with(|ids| ids.borrow_mut().insert(format!("V{}", self.0)));
        self
    }

    /// `errorId` (getter).
    pub fn error_id_value(self) -> String {
        UNUSED_ERROR_IDS.with(|ids| ids.borrow_mut().remove(&format!("V{}", self.0)));
        let data = self.data();
        match data.error_id {
            Some(error_id) => error_id,
            None => panic!(
                "No error ID assigned for Var {} at {}",
                data.name,
                loc_str(data.location)
            ),
        }
    }

    /// `name`.
    pub fn name(self) -> Name {
        self.data().name
    }

    /// `isFinal`.
    pub fn is_final(self) -> bool {
        self.data().is_final
    }

    /// `type` (getter): fails if the type is not yet known.
    pub fn type_(self) -> Type {
        match self.data().ty {
            Some(ty) => ty,
            None => panic!("Type not yet known"),
        }
    }

    /// `type` (getter), the name used by [`MiniAstOperations`](super::operations::MiniAstOperations).
    pub fn ty(self) -> Type {
        self.type_()
    }

    /// `_type`.
    pub fn type_if_known(self) -> Option<Type> {
        self.data().ty
    }

    /// `type = value` (setter): fails if the type is already set.
    pub fn set_type(self, ty: Type) {
        self.update(|d| {
            if d.ty.is_some() {
                panic!("Type already set");
            }
            d.ty = Some(ty);
        });
    }

    /// Sets `_type` without the check (Dart `catch_.exception?._type = ...`
    /// and `PatternVariableJoin.type = ...` in `finishJoinedPatternVariable`
    /// assigns `type` which checks; this is the unchecked field write).
    pub fn set_type_unchecked(self, ty: Type) {
        self.update(|d| d.ty = Some(ty));
    }

    /// `identity`.
    pub fn identity(self) -> String {
        self.data().identity
    }

    /// `location`.
    pub fn location(self) -> String {
        loc_str(self.data().location)
    }

    /// `inconsistency`.
    pub fn inconsistency(self) -> JoinedPatternVariableInconsistency {
        match self.data().join {
            Some(join) => join.inconsistency,
            None => JoinedPatternVariableInconsistency::None,
        }
    }

    /// Whether this is a `PatternVariableJoin`.
    pub fn is_join(self) -> bool {
        self.data().join.is_some()
    }

    /// `stringToCheckVariables`: the identity, or for a join its
    /// `toString()`.
    pub fn string_to_check_variables(self) -> String {
        let data = self.data();
        match &data.join {
            None => data.identity.clone(),
            Some(join) => {
                let mut declaration: Vec<String> = Vec::new();
                if let Some(ty) = data.ty {
                    if join.inconsistency != JoinedPatternVariableInconsistency::None {
                        declaration.push(format!(
                            "notConsistent:{}",
                            inconsistency_name(join.inconsistency)
                        ));
                    }
                    if data.is_final {
                        declaration.push("final".to_string());
                    }
                    declaration.push(ty.type_string());
                }
                declaration.push(data.name.to_string());
                let components: Vec<String> = join
                    .expected_components
                    .iter()
                    .map(|v| v.string_to_check_variables())
                    .collect();
                format!("{} = [{}]", declaration.join(" "), components.join(", "))
            }
        }
    }

    // ------------------------------------------------------------ builders

    /// Dart: using the variable where an expression is expected
    /// (`asExpression`, a `VariableReference`).
    #[track_caller]
    pub fn expr(self) -> Node {
        Node::alloc(
            NodeKind::VariableReference {
                variable: self,
                callback: None,
            },
            Location::caller(),
        )
    }

    /// `pattern()`: a variable pattern. Use
    /// [`Node::with_declared_type`] and [`Node::with_expect_inferred_type`]
    /// for the Dart named parameters `type` and `expectInferredType`.
    #[track_caller]
    pub fn pattern(self) -> Node {
        Node::alloc(
            NodeKind::VariablePattern {
                declared_type: None,
                variable: self,
                expect_inferred_type: None,
                is_assigned_variable: None,
            },
            Location::caller(),
        )
    }

    /// `postIncDec()`.
    #[track_caller]
    pub fn post_inc_dec(self) -> Node {
        let lhs = self.expr();
        Node::alloc(NodeKind::PostIncDec { lhs }, Location::caller())
    }

    /// `preIncDec()`.
    #[track_caller]
    pub fn pre_inc_dec(self) -> Node {
        let lhs = self.expr();
        Node::alloc(NodeKind::PreIncDec { lhs }, Location::caller())
    }

    /// `readAndCheckPromotedType(callback)`.
    #[track_caller]
    pub fn read_and_check_promoted_type(self, callback: impl Fn(Option<Type>) + 'static) -> Node {
        Node::alloc(
            NodeKind::VariableReference {
                variable: self,
                callback: Some(Rc::new(callback)),
            },
            Location::caller(),
        )
    }

    /// `write(value)`.
    #[track_caller]
    pub fn write(self, value: impl IntoNode) -> Node {
        let value = value.into_node();
        let lhs = self.expr();
        Node::alloc(
            NodeKind::Write {
                lhs,
                rhs: as_expression(value),
            },
            Location::caller(),
        )
    }

    // ------------------------------------- ProtoExpression methods of Var
    //
    // Dart `Var` mixes in `ProtoExpression`: these methods read the
    // variable (`asExpression`) and call the [`Node`] method.

    /// `x!` (Dart getter `nonNullAssert`).
    #[track_caller]
    pub fn non_null_assert(self) -> Node {
        self.expr().non_null_assert()
    }

    /// `!x` (Dart getter `not`).
    #[track_caller]
    pub fn not(self) -> Node {
        self.expr().not()
    }

    /// `(x)` (Dart getter `parenthesized`).
    #[track_caller]
    pub fn parenthesized(self) -> Node {
        self.expr().parenthesized()
    }

    /// `x && other`.
    #[track_caller]
    pub fn and(self, other: impl IntoNode) -> Node {
        self.expr().and(other)
    }

    /// `x || other`.
    #[track_caller]
    pub fn or(self, other: impl IntoNode) -> Node {
        self.expr().or(other)
    }

    /// `x as type`.
    #[track_caller]
    pub fn as_(self, type_str: &str) -> Node {
        self.expr().as_(type_str)
    }

    /// `x.cascade(sections, isNullAware: ...)`.
    #[track_caller]
    pub fn cascade(self, sections: Vec<Box<dyn Fn(Node) -> Node>>, is_null_aware: bool) -> Node {
        self.expr().cascade(sections, is_null_aware)
    }

    /// `x ? ifTrue : ifFalse`.
    #[track_caller]
    pub fn conditional(self, if_true: impl IntoNode, if_false: impl IntoNode) -> Node {
        self.expr().conditional(if_true, if_false)
    }

    /// `x == other`.
    #[track_caller]
    pub fn eq(self, other: impl IntoNode) -> Node {
        self.expr().eq(other)
    }

    /// `x ?? other`.
    #[track_caller]
    pub fn if_null(self, other: impl IntoNode) -> Node {
        self.expr().if_null(other)
    }

    /// `x.invokeMethod(name, arguments, isNullAware: ...)`.
    #[track_caller]
    pub fn invoke_method(self, name: &str, arguments: Vec<Node>, is_null_aware: bool) -> Node {
        self.expr().invoke_method(name, arguments, is_null_aware)
    }

    /// `x.invokeAnonymousMethod(body, returnType:, isNullAware:,
    /// isParameterless:, parameter:)`.
    #[track_caller]
    pub fn invoke_anonymous_method(
        self,
        body: Vec<Node>,
        return_type: &str,
        is_null_aware: bool,
        is_parameterless: bool,
        parameter: Option<Var>,
    ) -> Node {
        self.expr().invoke_anonymous_method(
            body,
            return_type,
            is_null_aware,
            is_parameterless,
            parameter,
        )
    }

    /// `x is type`.
    #[track_caller]
    pub fn is_(self, type_str: &str) -> Node {
        self.expr().is_(type_str)
    }

    /// `x is! type`.
    #[track_caller]
    pub fn is_not(self, type_str: &str) -> Node {
        self.expr().is_not(type_str)
    }

    /// `x != other`.
    #[track_caller]
    pub fn not_eq(self, other: impl IntoNode) -> Node {
        self.expr().not_eq(other)
    }

    /// `x.property(name, isNullAware: ...)`.
    #[track_caller]
    pub fn property(self, name: &str, is_null_aware: bool) -> Node {
        self.expr().property(name, is_null_aware)
    }

    /// `x.thenStmt(stmt)`.
    #[track_caller]
    pub fn then_stmt(self, stmt: impl IntoNode) -> Node {
        self.expr().then_stmt(stmt)
    }

    /// `x.checkType(expectedType)`.
    #[track_caller]
    pub fn check_type(self, expected_type: &str) -> Node {
        self.expr().check_type(expected_type)
    }

    /// `x.checkSchema(expectedSchema)`.
    #[track_caller]
    pub fn check_schema(self, expected_schema: &str) -> Node {
        self.expr().check_schema(expected_schema)
    }

    /// `x.checkIR(expectedIR)`.
    #[track_caller]
    pub fn check_ir(self, expected_ir: &str) -> Node {
        self.expr().check_ir(expected_ir)
    }

    /// `x.getExpressionInfo(callback)`.
    #[track_caller]
    pub fn get_expression_info(self, callback: impl Fn(Option<ExprInfo>) + 'static) -> Node {
        self.expr().get_expression_info(callback)
    }

    /// `x.whyNotPromoted(callback)`.
    #[track_caller]
    pub fn why_not_promoted(self, callback: impl Fn(WhyNotPromotedMap) + 'static) -> Node {
        self.expr().why_not_promoted(callback)
    }
}

impl fmt::Debug for Var {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let data = self.data();
        if data.join.is_some() {
            write!(f, "{}", self.string_to_check_variables())
        } else {
            write!(f, "var {}", data.name)
        }
    }
}

/// The Dart `name` of a [`JoinedPatternVariableInconsistency`] value.
pub fn inconsistency_name(inconsistency: JoinedPatternVariableInconsistency) -> &'static str {
    match inconsistency {
        JoinedPatternVariableInconsistency::None => "none",
        JoinedPatternVariableInconsistency::LogicalOr => "logicalOr",
        JoinedPatternVariableInconsistency::SharedCaseAbsent => "sharedCaseAbsent",
        JoinedPatternVariableInconsistency::SharedCaseHasLabel => "sharedCaseHasLabel",
        JoinedPatternVariableInconsistency::DifferentFinalityOrType => "differentFinalityOrType",
    }
}

// ========================================================= proto conversions

/// Something that can be used where an expression (or a statement, or a
/// collection element) is expected: a [`Node`], or a [`Var`] (which
/// becomes a read of the variable, Dart `Var.asExpression`). All builder
/// parameters of type `impl IntoNode` accept both; lists use
/// [`nodes!`](crate::nodes).
pub trait IntoNode {
    /// The node (a [`Var`] becomes a new variable reference).
    #[track_caller]
    fn into_node(self) -> Node;
}

impl IntoNode for Node {
    fn into_node(self) -> Node {
        self
    }
}

impl IntoNode for Var {
    #[track_caller]
    fn into_node(self) -> Node {
        self.expr()
    }
}

/// An optional [`IntoNode`] argument (Dart `ProtoExpression?`): `None`, or
/// anything that implements [`IntoNode`].
pub trait IntoOptNode {
    /// The node, if any.
    #[track_caller]
    fn into_opt_node(self) -> Option<Node>;
}

impl<T: IntoNode> IntoOptNode for T {
    #[track_caller]
    fn into_opt_node(self) -> Option<Node> {
        Some(self.into_node())
    }
}

impl IntoOptNode for Option<Node> {
    fn into_opt_node(self) -> Option<Node> {
        self
    }
}

/// A `Vec<Node>` from nodes and variables (Dart lists of
/// `ProtoStatement`/`ProtoExpression`/`ProtoCollectionElement`): a [`Var`]
/// becomes a read of the variable.
#[macro_export]
macro_rules! nodes {
    ($($e:expr),* $(,)?) => {
        vec![$($crate::node::IntoNode::into_node($e)),*]
    };
}

/// Dart `asExpression`: `node` must be an expression.
pub fn as_expression(node: Node) -> Node {
    match node.category() {
        Category::Expression => node,
        other => panic!("{node:?} ({other:?}) used where an expression is expected"),
    }
}

/// Dart `asStatement`: an expression becomes an expression statement.
fn as_statement(node: Node, location: Loc) -> Node {
    match node.category() {
        Category::Statement => node,
        Category::Expression => Node::alloc(NodeKind::ExpressionStatement { expr: node }, location),
        other => panic!("{node:?} ({other:?}) used where a statement is expected"),
    }
}

/// Dart `asCollectionElement`: an expression becomes an expression
/// collection element.
fn as_collection_element(node: Node, location: Loc) -> Node {
    match node.category() {
        Category::CollectionElement => node,
        Category::Expression => Node::alloc(
            NodeKind::ExpressionCollectionElement { expression: node },
            location,
        ),
        other => panic!("{node:?} ({other:?}) used where a collection element is expected"),
    }
}

/// Dart `_asGuardedPattern` (of `PossiblyGuardedPattern`).
fn as_guarded_pattern(node: Node) -> Node {
    match node.kind() {
        NodeKind::GuardedPattern { .. } => node,
        _ if node.category() == Category::Pattern => Node::alloc(
            NodeKind::GuardedPattern {
                pattern: node,
                guard: None,
                variables: None,
            },
            node.data().location,
        ),
        _ => panic!("{node:?} used where a pattern is expected"),
    }
}

/// Dart `asSwitchHead`.
fn as_switch_head(node: Node) -> Node {
    match node.kind() {
        NodeKind::SwitchHeadCase { .. } | NodeKind::SwitchHeadDefault => node,
        _ => {
            let guarded = as_guarded_pattern(node);
            Node::alloc(
                NodeKind::SwitchHeadCase {
                    guarded_pattern: guarded,
                },
                node.data().location,
            )
        }
    }
}

/// Dart `Block._`.
fn block_at(statements: Vec<Node>, location: Loc) -> Node {
    let statements = statements
        .into_iter()
        .map(|s| as_statement(s, location))
        .collect();
    Node::alloc(NodeKind::Block { statements }, location)
}

fn opt_type(ty: Option<&str>) -> Option<Type> {
    ty.map(Type::parse)
}

// ================================================================= builders

/// `default_`.
#[track_caller]
pub fn default_() -> Node {
    Node::alloc(NodeKind::SwitchHeadDefault, Location::caller())
}

/// `nullLiteral`.
#[track_caller]
pub fn null_literal() -> Node {
    Node::alloc(NodeKind::NullLiteral, Location::caller())
}

/// `this_`.
#[track_caller]
pub fn this_() -> Node {
    Node::alloc(NodeKind::This, Location::caller())
}

/// `assert_(condition, [message])`.
#[track_caller]
pub fn assert_(condition: impl IntoNode, message: impl IntoOptNode) -> Node {
    let condition = condition.into_node();
    let message = message.into_opt_node();
    Node::alloc(
        NodeKind::Assert {
            condition: as_expression(condition),
            message: message.map(as_expression),
        },
        Location::caller(),
    )
}

/// `await_(operand)`.
#[track_caller]
pub fn await_(operand: impl IntoNode) -> Node {
    let operand = operand.into_node();
    Node::alloc(
        NodeKind::Await {
            operand: as_expression(operand),
        },
        Location::caller(),
    )
}

/// `block(statements)`.
#[track_caller]
pub fn block(statements: Vec<Node>) -> Node {
    block_at(statements, Location::caller())
}

/// `booleanLiteral(value)`.
#[track_caller]
pub fn boolean_literal(value: bool) -> Node {
    Node::alloc(NodeKind::BooleanLiteral { value }, Location::caller())
}

/// `break_([target])`.
#[track_caller]
pub fn break_(target: Option<Label>) -> Node {
    Node::alloc(NodeKind::Break { target }, Location::caller())
}

/// `checkAssigned(variable, expectedAssignedState)`.
#[track_caller]
pub fn check_assigned(variable: Var, expected: bool) -> Node {
    Node::alloc(
        NodeKind::CheckAssigned { variable, expected },
        Location::caller(),
    )
}

/// `checkNotPromoted(promotable)`.
#[track_caller]
pub fn check_not_promoted(promotable: impl Into<Promotable>) -> Node {
    Node::alloc(
        NodeKind::CheckPromoted {
            promotable: promotable.into(),
            expected: None,
        },
        Location::caller(),
    )
}

/// `checkPromoted(promotable, expectedTypeStr)`.
#[track_caller]
pub fn check_promoted<'a>(
    promotable: impl Into<Promotable>,
    expected: impl Into<Option<&'a str>>,
) -> Node {
    Node::alloc(
        NodeKind::CheckPromoted {
            promotable: promotable.into(),
            expected: expected.into().map(str::to_string),
        },
        Location::caller(),
    )
}

/// `checkPromotionChain(promotable, expectedPromotionChain)`.
#[track_caller]
pub fn check_promotion_chain(promotable: impl Into<Promotable>, expected: &[&str]) -> Node {
    Node::alloc(
        NodeKind::CheckPromotionChain {
            promotable: promotable.into(),
            expected: expected.iter().map(|s| s.to_string()).collect(),
        },
        Location::caller(),
    )
}

/// `checkReachable(expectedReachable)`.
#[track_caller]
pub fn check_reachable(expected: bool) -> Node {
    Node::alloc(NodeKind::CheckReachable { expected }, Location::caller())
}

/// `checkUnassigned(variable, expectedUnassignedState)`.
#[track_caller]
pub fn check_unassigned(variable: Var, expected: bool) -> Node {
    Node::alloc(
        NodeKind::CheckUnassigned { variable, expected },
        Location::caller(),
    )
}

/// `continue_([target])`.
#[track_caller]
pub fn continue_(target: Option<Label>) -> Node {
    Node::alloc(NodeKind::Continue { target }, Location::caller())
}

/// `declare(variable)`; use `with_late`, `with_final`, `with_declared_type`,
/// `with_initializer` and `with_expect_inferred_type` for the Dart named
/// parameters.
#[track_caller]
pub fn declare(variable: Var) -> Node {
    Node::alloc(
        NodeKind::VariableDeclaration {
            variable,
            is_late: false,
            is_final: false,
            declared_type: None,
            initializer: None,
            expect_inferred_type: None,
        },
        Location::caller(),
    )
}

/// `do_(body, condition)`.
#[track_caller]
pub fn do_(body: Vec<Node>, condition: impl IntoNode) -> Node {
    let condition = condition.into_node();
    let location = Location::caller();
    Node::alloc(
        NodeKind::Do {
            body: block_at(body, location),
            condition: as_expression(condition),
        },
        location,
    )
}

/// `dotShorthandHead(name)`.
#[track_caller]
pub fn dot_shorthand_head(name: &str) -> Node {
    Node::alloc(
        NodeKind::DotShorthandHead {
            name: name.to_string(),
        },
        Location::caller(),
    )
}

/// `expr(typeStr)`: a placeholder expression of the given type.
#[track_caller]
pub fn expr(type_str: &str) -> Node {
    Node::alloc(
        NodeKind::PlaceholderExpression {
            ty: Type::parse(type_str),
        },
        Location::caller(),
    )
}

/// `for_(initializer, condition, updater, body, {forCollection})`.
#[track_caller]
pub fn for_(
    initializer: impl IntoOptNode,
    condition: impl IntoOptNode,
    updater: impl IntoOptNode,
    body: Vec<Node>,
    for_collection: bool,
) -> Node {
    let initializer = initializer.into_opt_node();
    let condition = condition.into_opt_node();
    let updater = updater.into_opt_node();
    let location = Location::caller();
    Node::alloc(
        NodeKind::For {
            initializer: initializer.map(|s| as_statement(s, location)),
            condition: condition.map(as_expression),
            updater: updater.map(as_expression),
            body: block_at(body, location),
            for_collection,
        },
        location,
    )
}

/// `forEachWithNonVariable(iterable, body)`.
#[track_caller]
pub fn for_each_with_non_variable(iterable: impl IntoNode, body: Vec<Node>) -> Node {
    let iterable = iterable.into_node();
    let location = Location::caller();
    Node::alloc(
        NodeKind::ForEach {
            variable: None,
            iterable: as_expression(iterable),
            body: block_at(body, location),
            declares_variable: false,
        },
        location,
    )
}

/// `forEachWithVariableDecl(variable, iterable, body)`.
#[track_caller]
pub fn for_each_with_variable_decl(
    variable: Var,
    iterable: impl IntoNode,
    body: Vec<Node>,
) -> Node {
    let iterable = iterable.into_node();
    let location = Location::caller();
    Node::alloc(
        NodeKind::ForEach {
            variable: Some(variable),
            iterable: as_expression(iterable),
            body: block_at(body, location),
            declares_variable: true,
        },
        location,
    )
}

/// `forEachWithVariableSet(variable, iterable, body)`.
#[track_caller]
pub fn for_each_with_variable_set(variable: Var, iterable: impl IntoNode, body: Vec<Node>) -> Node {
    let iterable = iterable.into_node();
    let location = Location::caller();
    Node::alloc(
        NodeKind::ForEach {
            variable: Some(variable),
            iterable: as_expression(iterable),
            body: block_at(body, location),
            declares_variable: false,
        },
        location,
    )
}

/// `if_(condition, ifTrue)`.
#[track_caller]
pub fn if_(condition: impl IntoNode, if_true: Vec<Node>) -> Node {
    let condition = condition.into_node();
    let location = Location::caller();
    Node::alloc(
        NodeKind::If {
            condition: as_expression(condition),
            if_true: block_at(if_true, location),
            if_false: None,
        },
        location,
    )
}

/// `if_(condition, ifTrue, ifFalse)`.
#[track_caller]
pub fn if_else(condition: impl IntoNode, if_true: Vec<Node>, if_false: Vec<Node>) -> Node {
    let condition = condition.into_node();
    let location = Location::caller();
    Node::alloc(
        NodeKind::If {
            condition: as_expression(condition),
            if_true: block_at(if_true, location),
            if_false: Some(block_at(if_false, location)),
        },
        location,
    )
}

/// `ifCase(expression, pattern, ifTrue, [ifFalse])`.
#[track_caller]
pub fn if_case(
    expression: impl IntoNode,
    pattern: impl IntoNode,
    if_true: Vec<Node>,
    if_false: Option<Vec<Node>>,
) -> Node {
    let expression = expression.into_node();
    let pattern = pattern.into_node();
    let location = Location::caller();
    let guarded = as_guarded_pattern(pattern);
    let NodeKind::GuardedPattern { pattern, guard, .. } = guarded.kind() else {
        unreachable!()
    };
    Node::alloc(
        NodeKind::IfCase {
            expression: as_expression(expression),
            pattern,
            guard,
            if_true: block_at(if_true, location),
            if_false: if_false.map(|f| block_at(f, location)),
            candidate_variables: Vec::new(),
        },
        location,
    )
}

/// `ifCaseElement(expression, pattern, ifTrue, [ifFalse])`.
#[track_caller]
pub fn if_case_element(
    expression: impl IntoNode,
    pattern: impl IntoNode,
    if_true: impl IntoNode,
    if_false: impl IntoOptNode,
) -> Node {
    let expression = expression.into_node();
    let pattern = pattern.into_node();
    let if_true = if_true.into_node();
    let if_false = if_false.into_opt_node();
    let location = Location::caller();
    let guarded = as_guarded_pattern(pattern);
    let NodeKind::GuardedPattern { pattern, guard, .. } = guarded.kind() else {
        unreachable!()
    };
    Node::alloc(
        NodeKind::IfCaseElement {
            expression: as_expression(expression),
            pattern,
            guard,
            if_true: as_collection_element(if_true, location),
            if_false: if_false.map(|f| as_collection_element(f, location)),
            variables: Vec::new(),
        },
        location,
    )
}

/// `ifElement(condition, ifTrue, [ifFalse])`.
#[track_caller]
pub fn if_element(
    condition: impl IntoNode,
    if_true: impl IntoNode,
    if_false: impl IntoOptNode,
) -> Node {
    let condition = condition.into_node();
    let if_true = if_true.into_node();
    let if_false = if_false.into_opt_node();
    let location = Location::caller();
    Node::alloc(
        NodeKind::IfElement {
            condition: as_expression(condition),
            if_true: as_collection_element(if_true, location),
            if_false: if_false.map(|f| as_collection_element(f, location)),
        },
        location,
    )
}

/// `intLiteral(value)`.
#[track_caller]
pub fn int_literal(value: i64) -> Node {
    Node::alloc(NodeKind::IntLiteral { value }, Location::caller())
}

/// `listLiteral(elements, elementType: ...)`.
#[track_caller]
pub fn list_literal(elements: Vec<Node>, element_type: &str) -> Node {
    let location = Location::caller();
    Node::alloc(
        NodeKind::ListLiteral {
            elements: elements
                .into_iter()
                .map(|e| as_collection_element(e, location))
                .collect(),
            element_type: Type::parse(element_type),
        },
        location,
    )
}

/// `listPattern(elements, {elementType})`.
#[track_caller]
pub fn list_pattern(elements: Vec<Node>, element_type: Option<&str>) -> Node {
    Node::alloc(
        NodeKind::ListPattern {
            element_type: opt_type(element_type),
            elements,
        },
        Location::caller(),
    )
}

/// `localFunction(body)`.
#[track_caller]
pub fn local_function(body: Vec<Node>) -> Node {
    let location = Location::caller();
    Node::alloc(
        NodeKind::LocalFunction {
            body: block_at(body, location),
            ty: Type::parse("void Function()"),
        },
        location,
    )
}

/// `mapEntry(key, value, {isKeyNullAware})`.
#[track_caller]
pub fn map_entry(key: impl IntoNode, value: impl IntoNode, is_key_null_aware: bool) -> Node {
    let key = key.into_node();
    let value = value.into_node();
    Node::alloc(
        NodeKind::MapEntry {
            key: as_expression(key),
            value: as_expression(value),
            is_key_null_aware,
        },
        Location::caller(),
    )
}

/// `mapLiteral(elements, keyType: ..., valueType: ...)`.
#[track_caller]
pub fn map_literal(elements: Vec<Node>, key_type: &str, value_type: &str) -> Node {
    let location = Location::caller();
    Node::alloc(
        NodeKind::MapLiteral {
            elements: elements
                .into_iter()
                .map(|e| as_collection_element(e, location))
                .collect(),
            key_type: Type::parse(key_type),
            value_type: Type::parse(value_type),
        },
        location,
    )
}

/// `mapPattern(elements, {keyType, valueType})`.
#[track_caller]
pub fn map_pattern(elements: Vec<Node>, key_type: Option<&str>, value_type: Option<&str>) -> Node {
    let type_arguments = if key_type.is_none() && value_type.is_none() {
        None
    } else {
        Some((
            Type::parse(key_type.unwrap()),
            Type::parse(value_type.unwrap()),
        ))
    };
    Node::alloc(
        NodeKind::MapPattern {
            type_arguments,
            elements,
        },
        Location::caller(),
    )
}

/// `mapPatternEntry(key, value)`.
#[track_caller]
pub fn map_pattern_entry(key: impl IntoNode, value: impl IntoNode) -> Node {
    let key = key.into_node();
    let value = value.into_node();
    Node::alloc(
        NodeKind::MapPatternEntry {
            key: as_expression(key),
            value,
        },
        Location::caller(),
    )
}

/// `mapPatternWithTypeArguments(keyType:, valueType:, elements:)`.
#[track_caller]
pub fn map_pattern_with_type_arguments(
    key_type: &str,
    value_type: &str,
    elements: Vec<Node>,
) -> Node {
    Node::alloc(
        NodeKind::MapPattern {
            type_arguments: Some((Type::parse(key_type), Type::parse(value_type))),
            elements,
        },
        Location::caller(),
    )
}

/// `objectPattern(requiredType:, fields:)`.
#[track_caller]
pub fn object_pattern(required_type: &str, fields: Vec<Node>) -> Node {
    let parsed_type = Type::parse(required_type);
    if parsed_type.as_primary_type().is_none() || parsed_type.is_question_type() {
        panic!("Expected a primary type, got {parsed_type}");
    }
    Node::alloc(
        NodeKind::ObjectPattern {
            required_type: parsed_type,
            fields,
        },
        Location::caller(),
    )
}

/// `patternForIn(pattern, expression, body, {hasAwait})`.
#[track_caller]
pub fn pattern_for_in(
    pattern: impl IntoNode,
    expression: impl IntoNode,
    body: Vec<Node>,
    has_await: bool,
) -> Node {
    let pattern = pattern.into_node();
    let expression = expression.into_node();
    let location = Location::caller();
    Node::alloc(
        NodeKind::PatternForIn {
            pattern,
            expression: as_expression(expression),
            body: block_at(body, location),
            has_await,
        },
        location,
    )
}

/// `patternForInElement(pattern, expression, body, {hasAwait})`.
#[track_caller]
pub fn pattern_for_in_element(
    pattern: impl IntoNode,
    expression: impl IntoNode,
    body: impl IntoNode,
    has_await: bool,
) -> Node {
    let pattern = pattern.into_node();
    let expression = expression.into_node();
    let body = body.into_node();
    let location = Location::caller();
    Node::alloc(
        NodeKind::PatternForInElement {
            pattern,
            expression: as_expression(expression),
            body: as_collection_element(body, location),
            has_await,
        },
        location,
    )
}

/// `patternVariableDeclaration(pattern, initializer, {isFinal})`.
#[track_caller]
pub fn pattern_variable_declaration(
    pattern: impl IntoNode,
    initializer: impl IntoNode,
    is_final: bool,
) -> Node {
    let pattern = pattern.into_node();
    let initializer = initializer.into_node();
    Node::alloc(
        NodeKind::PatternVariableDeclaration {
            pattern,
            initializer: as_expression(initializer),
            is_final,
        },
        Location::caller(),
    )
}

/// `recordPattern(fields)`.
#[track_caller]
pub fn record_pattern(fields: Vec<Node>) -> Node {
    Node::alloc(NodeKind::RecordPattern { fields }, Location::caller())
}

/// `relationalPattern(operator, operand)`; use [`Node::error_id`] for the
/// Dart named parameter `errorId`.
#[track_caller]
pub fn relational_pattern(operator: &str, operand: impl IntoNode) -> Node {
    let operand = operand.into_node();
    Node::alloc(
        NodeKind::RelationalPattern {
            operator: operator.to_string(),
            operand: as_expression(operand),
        },
        Location::caller(),
    )
}

/// `restPattern([subPattern])`.
#[track_caller]
pub fn rest_pattern(sub_pattern: impl IntoOptNode) -> Node {
    let sub_pattern = sub_pattern.into_opt_node();
    Node::alloc(NodeKind::RestPattern { sub_pattern }, Location::caller())
}

/// `return_()`.
#[track_caller]
pub fn return_() -> Node {
    Node::alloc(NodeKind::Return, Location::caller())
}

/// `second(first, second)`.
#[track_caller]
pub fn second(first: impl IntoNode, second: impl IntoNode) -> Node {
    let first = first.into_node();
    let second = second.into_node();
    Node::alloc(
        NodeKind::Second {
            first: as_expression(first),
            second: as_expression(second),
        },
        Location::caller(),
    )
}

/// `superProperty(name)`.
#[track_caller]
pub fn super_property(name: &str) -> Node {
    Node::alloc(
        NodeKind::ThisOrSuperProperty {
            property_name: name.to_string(),
            is_super_access: true,
        },
        Location::caller(),
    )
}

/// `switch_(expression, cases)`; use `with_legacy_exhaustive` and the
/// `expect_...` methods for the Dart named parameters.
#[track_caller]
pub fn switch_(expression: impl IntoNode, cases: Vec<Node>) -> Node {
    let expression = expression.into_node();
    Node::alloc(
        NodeKind::SwitchStatement {
            scrutinee: as_expression(expression),
            cases,
            is_legacy_exhaustive: None,
            expect_has_default: None,
            expect_is_exhaustive: None,
            expect_last_case_terminates: None,
            expect_requires_exhaustiveness_validation: None,
            expect_scrutinee_type: None,
        },
        Location::caller(),
    )
}

/// `switchExpr(expression, cases)`.
#[track_caller]
pub fn switch_expr(expression: impl IntoNode, cases: Vec<Node>) -> Node {
    let expression = expression.into_node();
    Node::alloc(
        NodeKind::SwitchExpression {
            scrutinee: as_expression(expression),
            cases,
        },
        Location::caller(),
    )
}

/// `switchStatementMember(cases, body, {hasLabels})`.
#[track_caller]
pub fn switch_statement_member(cases: Vec<Node>, body: Vec<Node>, has_labels: bool) -> Node {
    let location = Location::caller();
    Node::alloc(
        NodeKind::SwitchStatementMember {
            elements: cases.into_iter().map(as_switch_head).collect(),
            body: block_at(body, location),
            has_labels,
            candidate_variables: None,
        },
        location,
    )
}

/// `thisProperty(name)`.
#[track_caller]
pub fn this_property(name: &str) -> Node {
    Node::alloc(
        NodeKind::ThisOrSuperProperty {
            property_name: name.to_string(),
            is_super_access: false,
        },
        Location::caller(),
    )
}

/// `throw_(operand)`.
#[track_caller]
pub fn throw_(operand: impl IntoNode) -> Node {
    let operand = operand.into_node();
    Node::alloc(
        NodeKind::Throw {
            operand: as_expression(operand),
        },
        Location::caller(),
    )
}

/// `try_(body)`: use [`Node::catch_`] and [`Node::finally_`] to add clauses.
#[track_caller]
pub fn try_(body: Vec<Node>) -> Node {
    let location = Location::caller();
    Node::alloc(
        NodeKind::TryStatement {
            body: block_at(body, location),
            catches: Vec::new(),
            finally_statement: None,
        },
        location,
    )
}

/// `while_(condition, body)`.
#[track_caller]
pub fn while_(condition: impl IntoNode, body: Vec<Node>) -> Node {
    let condition = condition.into_node();
    let location = Location::caller();
    Node::alloc(
        NodeKind::While {
            condition: as_expression(condition),
            body: block_at(body, location),
        },
        location,
    )
}

/// `wildcard({type, expectInferredType})`; use
/// [`Node::with_declared_type`] and [`Node::with_expect_inferred_type`].
#[track_caller]
pub fn wildcard() -> Node {
    Node::alloc(
        NodeKind::WildcardPattern {
            declared_type: None,
            expect_inferred_type: None,
        },
        Location::caller(),
    )
}

/// `yield_(operand, {isYieldStar})`.
#[track_caller]
pub fn yield_(operand: impl IntoNode, is_yield_star: bool) -> Node {
    let operand = operand.into_node();
    Node::alloc(
        NodeKind::YieldStatement {
            operand: as_expression(operand),
            is_yield_star,
        },
        Location::caller(),
    )
}

impl Label {
    /// `Label(name)`.
    #[track_caller]
    pub fn new(name: &str) -> Label {
        Label(Node::alloc(
            NodeKind::BoundLabel {
                name: name.to_string(),
                binding: None,
            },
            Location::caller(),
        ))
    }

    /// `Label.unbound()`.
    #[track_caller]
    pub fn unbound() -> Label {
        Label(Node::alloc(NodeKind::UnboundLabel, Location::caller()))
    }

    /// `thenStmt(statement)`: binds the label to `statement`.
    #[track_caller]
    pub fn then_stmt(self, statement: impl IntoNode) -> Node {
        let statement = statement.into_node();
        match self.0.kind() {
            NodeKind::UnboundLabel => panic!("Unbound labels can't be bound"),
            NodeKind::BoundLabel { .. } => {
                let statement = as_statement(statement, Location::caller());
                let statement = match statement.kind() {
                    NodeKind::LabeledStatement { .. } => statement,
                    _ => Node::alloc(
                        NodeKind::LabeledStatement {
                            labels: Vec::new(),
                            body: statement,
                        },
                        Location::caller(),
                    ),
                };
                statement.update_kind(|k| {
                    if let NodeKind::LabeledStatement { labels, .. } = k {
                        labels.insert(0, self);
                    }
                });
                self.0.update_kind(|k| {
                    if let NodeKind::BoundLabel { binding, .. } = k {
                        *binding = Some(statement);
                    }
                });
                statement
            }
            _ => unreachable!(),
        }
    }

    /// `_getBinding()`.
    pub fn get_binding(self) -> Option<Node> {
        match self.0.kind() {
            NodeKind::BoundLabel { name, binding } => match binding {
                Some(binding) => Some(binding),
                None => panic!("Unbound label {name}"),
            },
            _ => None,
        }
    }
}

// ============================================================ node methods

impl Node {
    fn expect_category(self, category: Category, what: &str) {
        assert_eq!(self.category(), category, "{self:?}: {what}");
    }

    // ------------------------------------------------------ check methods

    /// `checkIR(expectedIR)`.
    pub fn check_ir(self, expected_ir: &str) -> Node {
        self.update(|d| d.expected_ir = Some(expected_ir.to_string()));
        self
    }

    /// `checkSchema(expectedSchema)`.
    pub fn check_schema(self, expected_schema: &str) -> Node {
        self.expect_category(Category::Expression, "checkSchema");
        self.update(|d| d.expected_schema = Some(expected_schema.to_string()));
        self
    }

    /// `checkType(expectedType)`.
    pub fn check_type(self, expected_type: &str) -> Node {
        self.expect_category(Category::Expression, "checkType");
        self.update(|d| d.expected_type = Some(expected_type.to_string()));
        self
    }

    /// `checkExpressionTypeAnalysisResult(checker)`.
    pub fn check_expression_type_analysis_result(
        self,
        checker: impl Fn(&ExprResultDetail) + 'static,
    ) -> Node {
        self.expect_category(Category::Expression, "checkExpressionTypeAnalysisResult");
        self.update(|d| d.check_expression_result = Some(Rc::new(checker)));
        self
    }

    /// `checkStatementTypeAnalysisResult(checker)`.
    #[track_caller]
    pub fn check_statement_type_analysis_result(
        self,
        checker: impl Fn(&StmtResultDetail) + 'static,
    ) -> Node {
        let statement = as_statement(self, Location::caller());
        statement.update(|d| d.check_statement_result = Some(Rc::new(checker)));
        statement
    }

    // ------------------------------------------- named-parameter setters

    /// Dart named parameter `type:` of `declare`, `Var.pattern` and
    /// `wildcard`.
    pub fn with_declared_type(self, ty: &str) -> Node {
        let parsed = Type::parse(ty);
        self.update_kind(|k| match k {
            NodeKind::VariableDeclaration { declared_type, .. }
            | NodeKind::VariablePattern { declared_type, .. }
            | NodeKind::WildcardPattern { declared_type, .. } => *declared_type = Some(parsed),
            _ => panic!("with_declared_type on {}", k.class_name()),
        });
        self
    }

    /// Dart named parameter `expectInferredType:` of `declare`,
    /// `Var.pattern` and `wildcard`.
    pub fn with_expect_inferred_type(self, ty: &str) -> Node {
        self.update_kind(|k| match k {
            NodeKind::VariableDeclaration {
                expect_inferred_type,
                ..
            }
            | NodeKind::VariablePattern {
                expect_inferred_type,
                ..
            }
            | NodeKind::WildcardPattern {
                expect_inferred_type,
                ..
            } => *expect_inferred_type = Some(ty.to_string()),
            _ => panic!("with_expect_inferred_type on {}", k.class_name()),
        });
        self
    }

    /// Dart named parameter `initializer:` of `declare`.
    pub fn with_initializer(self, value: impl IntoNode) -> Node {
        let value = value.into_node();
        let value = as_expression(value);
        self.update_kind(|k| match k {
            NodeKind::VariableDeclaration { initializer, .. } => *initializer = Some(value),
            _ => panic!("with_initializer on {}", k.class_name()),
        });
        self
    }

    /// Dart named parameter `isLate: true` of `declare`.
    pub fn with_late(self) -> Node {
        self.update_kind(|k| match k {
            NodeKind::VariableDeclaration { is_late, .. } => *is_late = true,
            _ => panic!("with_late on {}", k.class_name()),
        });
        self
    }

    /// Dart named parameter `isFinal: true` of `declare`.
    pub fn with_final(self) -> Node {
        self.update_kind(|k| match k {
            NodeKind::VariableDeclaration { is_final, .. } => *is_final = true,
            _ => panic!("with_final on {}", k.class_name()),
        });
        self
    }

    /// Dart named parameter `isLegacyExhaustive:` of `switch_`.
    pub fn with_legacy_exhaustive(self, value: bool) -> Node {
        self.update_kind(|k| match k {
            NodeKind::SwitchStatement {
                is_legacy_exhaustive,
                ..
            } => *is_legacy_exhaustive = Some(value),
            _ => panic!("with_legacy_exhaustive on {}", k.class_name()),
        });
        self
    }

    /// Dart named parameter `expectHasDefault:` of `switch_`.
    pub fn expect_has_default(self, value: bool) -> Node {
        self.update_kind(|k| {
            if let NodeKind::SwitchStatement {
                expect_has_default, ..
            } = k
            {
                *expect_has_default = Some(value)
            }
        });
        self
    }

    /// Dart named parameter `expectIsExhaustive:` of `switch_`.
    pub fn expect_is_exhaustive(self, value: bool) -> Node {
        self.update_kind(|k| {
            if let NodeKind::SwitchStatement {
                expect_is_exhaustive,
                ..
            } = k
            {
                *expect_is_exhaustive = Some(value)
            }
        });
        self
    }

    /// Dart named parameter `expectLastCaseTerminates:` of `switch_`.
    pub fn expect_last_case_terminates(self, value: bool) -> Node {
        self.update_kind(|k| {
            if let NodeKind::SwitchStatement {
                expect_last_case_terminates,
                ..
            } = k
            {
                *expect_last_case_terminates = Some(value)
            }
        });
        self
    }

    /// Dart named parameter `expectRequiresExhaustivenessValidation:` of
    /// `switch_`.
    pub fn expect_requires_exhaustiveness_validation(self, value: bool) -> Node {
        self.update_kind(|k| {
            if let NodeKind::SwitchStatement {
                expect_requires_exhaustiveness_validation,
                ..
            } = k
            {
                *expect_requires_exhaustiveness_validation = Some(value)
            }
        });
        self
    }

    /// Dart named parameter `expectScrutineeType:` of `switch_`.
    pub fn expect_scrutinee_type(self, value: &str) -> Node {
        self.update_kind(|k| {
            if let NodeKind::SwitchStatement {
                expect_scrutinee_type,
                ..
            } = k
            {
                *expect_scrutinee_type = Some(value.to_string())
            }
        });
        self
    }

    // -------------------------------------------- ProtoExpression methods

    /// `dotShorthand` (getter).
    #[track_caller]
    pub fn dot_shorthand(self) -> Node {
        Node::alloc(
            NodeKind::DotShorthand {
                expr: as_expression(self),
            },
            Location::caller(),
        )
    }

    /// `nonNullAssert` (getter).
    #[track_caller]
    pub fn non_null_assert(self) -> Node {
        Node::alloc(
            NodeKind::NonNullAssert {
                operand: as_expression(self),
            },
            Location::caller(),
        )
    }

    /// `not` (getter).
    #[track_caller]
    pub fn not(self) -> Node {
        Node::alloc(
            NodeKind::Not {
                operand: as_expression(self),
            },
            Location::caller(),
        )
    }

    /// `parenthesized` (getter), of an expression or of a pattern.
    #[track_caller]
    pub fn parenthesized(self) -> Node {
        if self.category() == Category::Pattern {
            Node::alloc(
                NodeKind::ParenthesizedPattern { inner: self },
                Location::caller(),
            )
        } else {
            Node::alloc(
                NodeKind::ParenthesizedExpression {
                    expr: as_expression(self),
                },
                Location::caller(),
            )
        }
    }

    /// `and(other)`: `x && other` for expressions, a logical-and pattern for
    /// patterns.
    #[track_caller]
    pub fn and(self, other: impl IntoNode) -> Node {
        let other = other.into_node();
        if self.category() == Category::Pattern {
            Node::alloc(
                NodeKind::LogicalAndPattern {
                    lhs: self,
                    rhs: other,
                },
                Location::caller(),
            )
        } else {
            Node::alloc(
                NodeKind::Logical {
                    lhs: as_expression(self),
                    rhs: as_expression(other),
                    is_and: true,
                },
                Location::caller(),
            )
        }
    }

    /// `or(other)`: `x || other` for expressions, a logical-or pattern for
    /// patterns.
    #[track_caller]
    pub fn or(self, other: impl IntoNode) -> Node {
        let other = other.into_node();
        if self.category() == Category::Pattern {
            Node::alloc(
                NodeKind::LogicalOrPattern {
                    lhs: self,
                    rhs: other,
                },
                Location::caller(),
            )
        } else {
            Node::alloc(
                NodeKind::Logical {
                    lhs: as_expression(self),
                    rhs: as_expression(other),
                    is_and: false,
                },
                Location::caller(),
            )
        }
    }

    /// `as_(type)`: a cast expression, or a cast pattern for patterns.
    #[track_caller]
    pub fn as_(self, type_str: &str) -> Node {
        let ty = Type::parse(type_str);
        if self.category() == Category::Pattern {
            Node::alloc(
                NodeKind::CastPattern { inner: self, ty },
                Location::caller(),
            )
        } else {
            Node::alloc(
                NodeKind::As {
                    target: as_expression(self),
                    ty,
                },
                Location::caller(),
            )
        }
    }

    /// `cascade(sections, {isNullAware})`: each section is built from the
    /// cascade placeholder.
    #[track_caller]
    pub fn cascade(self, sections: Vec<Box<dyn Fn(Node) -> Node>>, is_null_aware: bool) -> Node {
        let location = Location::caller();
        let sections = sections
            .into_iter()
            .map(|section| {
                let placeholder = Node::alloc(NodeKind::CascadePlaceholder, location);
                as_expression(section(placeholder))
            })
            .collect();
        Node::alloc(
            NodeKind::Cascade {
                target: as_expression(self),
                sections,
                is_null_aware,
            },
            location,
        )
    }

    /// `conditional(ifTrue, ifFalse)`.
    #[track_caller]
    pub fn conditional(self, if_true: impl IntoNode, if_false: impl IntoNode) -> Node {
        let if_true = if_true.into_node();
        let if_false = if_false.into_node();
        Node::alloc(
            NodeKind::Conditional {
                condition: as_expression(self),
                if_true: as_expression(if_true),
                if_false: as_expression(if_false),
            },
            Location::caller(),
        )
    }

    /// `eq(other)`.
    #[track_caller]
    pub fn eq(self, other: impl IntoNode) -> Node {
        let other = other.into_node();
        Node::alloc(
            NodeKind::Equal {
                lhs: as_expression(self),
                rhs: as_expression(other),
                is_inverted: false,
            },
            Location::caller(),
        )
    }

    /// `ifNull(other)`.
    #[track_caller]
    pub fn if_null(self, other: impl IntoNode) -> Node {
        let other = other.into_node();
        Node::alloc(
            NodeKind::IfNull {
                lhs: as_expression(self),
                rhs: as_expression(other),
            },
            Location::caller(),
        )
    }

    /// `inTypeSchema(typeSchema)`.
    #[track_caller]
    pub fn in_type_schema(self, type_schema: &str) -> Node {
        Node::alloc(
            NodeKind::ExpressionInTypeSchema {
                expr: as_expression(self),
                type_schema: SharedTypeSchemaView::new(Type::parse(type_schema)),
            },
            Location::caller(),
        )
    }

    /// `invokeAnonymousMethod(body, {returnType, isNullAware,
    /// isParameterless, parameter})`.
    #[track_caller]
    pub fn invoke_anonymous_method(
        self,
        body: Vec<Node>,
        return_type: &str,
        is_null_aware: bool,
        is_parameterless: bool,
        parameter: Option<Var>,
    ) -> Node {
        let location = Location::caller();
        Node::alloc(
            NodeKind::InvokeAnonymousMethod {
                target: as_expression(self),
                body: block_at(body, location),
                return_type: Type::parse(return_type),
                is_null_aware,
                is_parameterless,
                parameter,
            },
            location,
        )
    }

    /// `invokeMethod(name, arguments, {isNullAware})`.
    #[track_caller]
    pub fn invoke_method(self, name: &str, arguments: Vec<Node>, is_null_aware: bool) -> Node {
        Node::alloc(
            NodeKind::InvokeMethod {
                target: as_expression(self),
                method_name: name.to_string(),
                arguments: arguments.into_iter().map(as_expression).collect(),
                is_null_aware,
            },
            Location::caller(),
        )
    }

    /// `is_(typeStr, {isInverted})`.
    #[track_caller]
    pub fn is_(self, type_str: &str) -> Node {
        Node::alloc(
            NodeKind::Is {
                target: as_expression(self),
                ty: Type::parse(type_str),
                is_inverted: false,
            },
            Location::caller(),
        )
    }

    /// `isNot(typeStr)`.
    #[track_caller]
    pub fn is_not(self, type_str: &str) -> Node {
        Node::alloc(
            NodeKind::Is {
                target: as_expression(self),
                ty: Type::parse(type_str),
                is_inverted: true,
            },
            Location::caller(),
        )
    }

    /// `notEq(other)`.
    #[track_caller]
    pub fn not_eq(self, other: impl IntoNode) -> Node {
        let other = other.into_node();
        Node::alloc(
            NodeKind::Equal {
                lhs: as_expression(self),
                rhs: as_expression(other),
                is_inverted: true,
            },
            Location::caller(),
        )
    }

    /// `property(name, {isNullAware})`.
    #[track_caller]
    pub fn property(self, name: &str, is_null_aware: bool) -> Node {
        Node::alloc(
            NodeKind::Property {
                target: as_expression(self),
                property_name: name.to_string(),
                is_null_aware,
            },
            Location::caller(),
        )
    }

    /// `thenStmt(stmt)`: evaluation of `this` followed by `stmt`.
    #[track_caller]
    pub fn then_stmt(self, stmt: impl IntoNode) -> Node {
        let stmt = stmt.into_node();
        let location = Location::caller();
        Node::alloc(
            NodeKind::WrappedExpression {
                before: None,
                expr: as_expression(self),
                after: Some(as_statement(stmt, location)),
            },
            location,
        )
    }

    /// `LValue.write(value)` (for properties).
    #[track_caller]
    pub fn write(self, value: impl IntoNode) -> Node {
        let value = value.into_node();
        Node::alloc(
            NodeKind::Write {
                lhs: as_expression(self),
                rhs: as_expression(value),
            },
            Location::caller(),
        )
    }

    // ----------------------------------------------------- pattern methods

    /// `ConstExpression.pattern` (getter): a constant pattern.
    #[track_caller]
    pub fn pattern(self) -> Node {
        Node::alloc(
            NodeKind::ConstantPattern {
                constant: as_expression(self),
            },
            Location::caller(),
        )
    }

    /// `nullAssert` (getter).
    #[track_caller]
    pub fn null_assert(self) -> Node {
        self.expect_category(Category::Pattern, "nullAssert");
        Node::alloc(
            NodeKind::NullCheckOrAssertPattern {
                inner: self,
                is_assert: true,
            },
            Location::caller(),
        )
    }

    /// `nullCheck` (getter).
    #[track_caller]
    pub fn null_check(self) -> Node {
        self.expect_category(Category::Pattern, "nullCheck");
        Node::alloc(
            NodeKind::NullCheckOrAssertPattern {
                inner: self,
                is_assert: false,
            },
            Location::caller(),
        )
    }

    /// `assign(rhs)`: a pattern assignment.
    #[track_caller]
    pub fn assign(self, rhs: impl IntoNode) -> Node {
        let rhs = rhs.into_node();
        self.expect_category(Category::Pattern, "assign");
        Node::alloc(
            NodeKind::PatternAssignment {
                lhs: self,
                rhs: as_expression(rhs),
            },
            Location::caller(),
        )
    }

    /// `recordField([name])`.
    #[track_caller]
    pub fn record_field(self, name: Option<&str>) -> Node {
        self.expect_category(Category::Pattern, "recordField");
        Node::alloc(
            NodeKind::RecordPatternField {
                name: name.map(intern),
                pattern: self,
            },
            Location::caller(),
        )
    }

    /// `when(guard)`.
    pub fn when(self, guard: impl IntoOptNode) -> Node {
        let guard = guard.into_opt_node();
        self.expect_category(Category::Pattern, "when");
        Node::alloc(
            NodeKind::GuardedPattern {
                pattern: self,
                guard: guard.map(as_expression),
                variables: None,
            },
            self.data().location,
        )
    }

    /// `then(body)` (of `PossiblyGuardedPattern` or `SwitchHead`): a switch
    /// statement member with this head.
    pub fn then(self, body: Vec<Node>) -> Node {
        let location = self.data().location;
        let head = as_switch_head(self);
        Node::alloc(
            NodeKind::SwitchStatementMember {
                elements: vec![head],
                body: block_at(body, location),
                has_labels: false,
                candidate_variables: None,
            },
            location,
        )
    }

    /// `thenExpr(body)`: a switch expression case.
    #[track_caller]
    pub fn then_expr(self, body: impl IntoNode) -> Node {
        let body = body.into_node();
        let guarded_pattern = match self.kind() {
            NodeKind::SwitchHeadDefault => None,
            NodeKind::SwitchHeadCase { guarded_pattern } => Some(guarded_pattern),
            _ => Some(as_guarded_pattern(self)),
        };
        Node::alloc(
            NodeKind::ExpressionCase {
                guarded_pattern,
                expression: as_expression(body),
            },
            Location::caller(),
        )
    }

    // ------------------------------------------------- try statement

    /// `catch_({type, exception, stackTrace, body})`.
    #[track_caller]
    pub fn catch_(
        self,
        ty: Option<&str>,
        exception: Option<Var>,
        stack_trace: Option<Var>,
        body: Vec<Node>,
    ) -> Node {
        let NodeKind::TryStatement {
            body: try_body,
            mut catches,
            finally_statement,
        } = self.kind()
        else {
            panic!("catch_ on {self:?}");
        };
        assert!(finally_statement.is_none(), "catch after finally");
        if exception.is_none() && stack_trace.is_some() {
            panic!(
                "If a stack trace variable is provided, an exception variable must be provided too"
            );
        }
        if exception.is_none() && ty.is_none() {
            panic!("If no exception variable is provided, an exception type must be provided");
        }
        catches.push(CatchClause {
            body: block_at(body, Location::caller()),
            exception_type: opt_type(ty),
            exception,
            stack_trace,
        });
        Node::alloc(
            NodeKind::TryStatement {
                body: try_body,
                catches,
                finally_statement: None,
            },
            self.data().location,
        )
    }

    /// `finally_(statements)`.
    #[track_caller]
    pub fn finally_(self, statements: Vec<Node>) -> Node {
        let NodeKind::TryStatement {
            body,
            catches,
            finally_statement,
        } = self.kind()
        else {
            panic!("finally_ on {self:?}");
        };
        assert!(finally_statement.is_none(), "multiple finally clauses");
        Node::alloc(
            NodeKind::TryStatement {
                body,
                catches,
                finally_statement: Some(block_at(statements, Location::caller())),
            },
            self.data().location,
        )
    }
}

/// Converts a [`SharedTypeView`] of a mini type to its Dart `type` string.
pub fn type_view_string(ty: SharedTypeView<Type>) -> String {
    ty.unwrap_type_view().type_string()
}

// ======================================================= _PropertyElement

/// A property of the mini-AST (Dart class `_PropertyElement`), used as the
/// `PropertyMember` of the operations.
#[derive(Clone, Debug)]
pub struct PropertyElement {
    /// The type of the property.
    pub ty: Type,
    /// The name of the property (used by `toString`).
    pub name: String,
    /// Whether the property is promotable.
    pub is_promotable: bool,
    /// The reason the property is not promotable, if applicable and relevant
    /// to the test.
    ///
    /// If the property is promotable ([`is_promotable`](Self::is_promotable)
    /// is `true`), this value is always `None`.
    ///
    /// Otherwise the value *may* be a reason for the property not being
    /// promotable, but it may also still be `None` if the reason is not
    /// relevant to the test.
    pub why_not_promotable: Option<PropertyNonPromotabilityReason>,
}

impl PropertyElement {
    /// `_PropertyElement(type, name, {isPromotable, whyNotPromotable})`.
    pub fn new(
        ty: Type,
        name: &str,
        is_promotable: bool,
        why_not_promotable: Option<PropertyNonPromotabilityReason>,
    ) -> Self {
        if is_promotable {
            assert!(why_not_promotable.is_none());
        }
        PropertyElement {
            ty,
            name: name.to_string(),
            is_promotable,
            why_not_promotable,
        }
    }
}

impl fmt::Display for PropertyElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.ty, self.name)
    }
}
