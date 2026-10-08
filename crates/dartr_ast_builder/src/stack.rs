// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/stack_listener.dart

//! The value stack of the AST builder (Dart `StackListener`, `Stack`,
//! `NullValues`) and the events that `StackListener` implements and the
//! `AstBuilder` does not override.
//!
//! A Dart stack entry is any object; here it is a [`Value`]: a node, a
//! token, a list, one of the helper objects of the AST builder, or a typed
//! `null` ([`NullValue`]). The pop helpers do the casts of the Dart code:
//! `pop() as T` is [`AstBuilder::pop_node`] (panics like a failed cast),
//! `pop() as T?` is [`AstBuilder::pop_node_opt`], and so on.

use dartr_ast::{Id, NodeId, NodeType};
use dartr_syntax::TokenId;

use crate::ast_builder::{
    AstBuilder, ConstructorNameWithInvalidTypeArgs, FunctionTypedFormalParameterData, Modifiers,
    ObjectPatternFields, OperatorName, OptionalFormalParameters, ParenthesizedCondition,
    PrimaryConstructorBuilder, RedirectingFactoryBody,
};

/// Dart `NullValues`: sentinel values used for typed `null` values on the
/// stack.
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NullValue {
    Arguments,
    As,
    AwaitToken,
    Block,
    BreakTarget,
    CascadeReceiver,
    Combinators,
    Comments,
    ConditionalUris,
    ConditionallySelectedImport,
    ConstructorInitializerSeparator,
    ConstructorInitializers,
    ConstructorReference,
    ConstructorReferenceContinuationAfterTypeArguments,
    ContinueTarget,
    Deferred,
    DocumentationComment,
    EnumConstantInfo,
    Expression,
    ExtendsClause,
    FieldInitializer,
    FormalParameters,
    FunctionBody,
    FunctionBodyAsyncToken,
    FunctionBodyStarToken,
    HideClause,
    Identifier,
    IdentifierList,
    Initializers,
    Labels,
    Metadata,
    Modifiers,
    Name,
    NominalVariable,
    NominalParameters,
    OperatorList,
    ParameterDefaultValue,
    Pattern,
    PatternList,
    Prefix,
    PrimaryConstructor,
    RecordTypeFieldList,
    ShowClause,
    StringLiteral,
    StructuralParameters,
    Token,
    Type,
    TypeArguments,
    TypeBuilder,
    TypeBuilderList,
    TypeList,
    VarFinalOrConstToken,
    VariableDeclarationList,
    WithClause,
}

/// A stack entry (a Dart `Object` on the `StackListener` stack).
#[derive(Debug)]
pub enum Value {
    /// A typed `null` (Dart `NullValue`).
    Null(NullValue),
    Token(TokenId),
    /// An AST node.
    Node(NodeId),
    /// A Dart `List` of nodes (metadata, combinators, type lists, ...).
    Nodes(Vec<NodeId>),
    /// A Dart `List<Token>` (library names, `handleQualified` in them).
    Tokens(Vec<TokenId>),
    Modifiers(Box<Modifiers>),
    OperatorName(OperatorName),
    ParenthesizedCondition(ParenthesizedCondition),
    ConstructorNameWithInvalidTypeArgs(ConstructorNameWithInvalidTypeArgs),
    FunctionTypedFormalParameterData(FunctionTypedFormalParameterData),
    ObjectPatternFields(Box<ObjectPatternFields>),
    OptionalFormalParameters(Box<OptionalFormalParameters>),
    PrimaryConstructorBuilder(Box<PrimaryConstructorBuilder>),
    RedirectingFactoryBody(Box<RedirectingFactoryBody>),
}

impl Value {
    /// Dart `value is NullValue`.
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null(_))
    }
}

impl From<NullValue> for Value {
    fn from(v: NullValue) -> Value {
        Value::Null(v)
    }
}

impl From<TokenId> for Value {
    fn from(v: TokenId) -> Value {
        Value::Token(v)
    }
}

impl<T: ?Sized> From<Id<T>> for Value {
    fn from(v: Id<T>) -> Value {
        Value::Node(v.raw())
    }
}

impl From<NodeId> for Value {
    fn from(v: NodeId) -> Value {
        Value::Node(v)
    }
}

impl<T: ?Sized> From<Vec<Id<T>>> for Value {
    fn from(v: Vec<Id<T>>) -> Value {
        Value::Nodes(v.into_iter().map(|i| i.raw()).collect())
    }
}

impl From<Vec<TokenId>> for Value {
    fn from(v: Vec<TokenId>) -> Value {
        Value::Tokens(v)
    }
}

impl From<Modifiers> for Value {
    fn from(v: Modifiers) -> Value {
        Value::Modifiers(Box::new(v))
    }
}

impl From<OperatorName> for Value {
    fn from(v: OperatorName) -> Value {
        Value::OperatorName(v)
    }
}

impl From<ParenthesizedCondition> for Value {
    fn from(v: ParenthesizedCondition) -> Value {
        Value::ParenthesizedCondition(v)
    }
}

impl From<ConstructorNameWithInvalidTypeArgs> for Value {
    fn from(v: ConstructorNameWithInvalidTypeArgs) -> Value {
        Value::ConstructorNameWithInvalidTypeArgs(v)
    }
}

impl From<FunctionTypedFormalParameterData> for Value {
    fn from(v: FunctionTypedFormalParameterData) -> Value {
        Value::FunctionTypedFormalParameterData(v)
    }
}

impl From<ObjectPatternFields> for Value {
    fn from(v: ObjectPatternFields) -> Value {
        Value::ObjectPatternFields(Box::new(v))
    }
}

impl From<OptionalFormalParameters> for Value {
    fn from(v: OptionalFormalParameters) -> Value {
        Value::OptionalFormalParameters(Box::new(v))
    }
}

impl From<PrimaryConstructorBuilder> for Value {
    fn from(v: PrimaryConstructorBuilder) -> Value {
        Value::PrimaryConstructorBuilder(Box::new(v))
    }
}

impl From<RedirectingFactoryBody> for Value {
    fn from(v: RedirectingFactoryBody) -> Value {
        Value::RedirectingFactoryBody(Box::new(v))
    }
}

/// Stack operations (Dart `StackListener.push`, `pop`, `peek`, `popList`,
/// ...) and the typed pops that replace the casts of the Dart code.
impl AstBuilder {
    /// Dart `push`.
    #[inline]
    pub(crate) fn push(&mut self, value: impl Into<Value>) {
        self.stack.push(value.into());
    }

    /// Dart `push(token ?? nullValue)`.
    #[inline]
    pub(crate) fn push_token_or(&mut self, token: Option<TokenId>, null_value: NullValue) {
        match token {
            Some(t) => self.stack.push(Value::Token(t)),
            None => self.stack.push(Value::Null(null_value)),
        }
    }

    /// Dart `push(list ?? nullValue)` for a node list.
    #[inline]
    pub(crate) fn push_nodes_or<T: ?Sized>(
        &mut self,
        nodes: Option<Vec<Id<T>>>,
        null_value: NullValue,
    ) {
        match nodes {
            Some(n) => self.push(n),
            None => self.stack.push(Value::Null(null_value)),
        }
    }

    /// Dart `pop()`: the raw value (a [`Value::Null`] for a typed `null`).
    #[inline]
    pub(crate) fn pop(&mut self) -> Value {
        self.stack.pop().expect("pop on an empty stack")
    }

    /// Dart `peek()`: the top value, `None` when the stack is empty.
    #[inline]
    pub(crate) fn peek(&self) -> Option<&Value> {
        self.stack.last()
    }

    /// The node in [value] as a `T` (Dart `value as T?`): `None` for a
    /// typed `null`; panics for another value.
    pub(crate) fn value_as_node_opt<T: ?Sized + NodeType>(&self, value: Value) -> Option<Id<T>> {
        match value {
            Value::Null(_) => None,
            Value::Node(n) => match self.ast.cast::<T>(n) {
                Some(id) => Some(id),
                None => panic!(
                    "type '{}Impl' is not a subtype of type '{}'",
                    self.ast.kind(n).name(),
                    T::NAME
                ),
            },
            other => panic!("{other:?} is not a subtype of type '{}'", T::NAME),
        }
    }

    /// Dart `pop() as T`.
    #[inline]
    pub(crate) fn pop_node<T: ?Sized + NodeType>(&mut self) -> Id<T> {
        let v = self.pop();
        match self.value_as_node_opt::<T>(v) {
            Some(id) => id,
            None => panic!("Null check operator used on a null value ({})", T::NAME),
        }
    }

    /// Dart `pop() as T?`.
    #[inline]
    pub(crate) fn pop_node_opt<T: ?Sized + NodeType>(&mut self) -> Option<Id<T>> {
        let v = self.pop();
        self.value_as_node_opt::<T>(v)
    }

    /// Dart `pop() as Token`.
    #[inline]
    pub(crate) fn pop_token(&mut self) -> TokenId {
        match self.pop() {
            Value::Token(t) => t,
            other => panic!("{other:?} is not a subtype of type 'Token'"),
        }
    }

    /// Dart `pop() as Token?`.
    #[inline]
    pub(crate) fn pop_token_opt(&mut self) -> Option<TokenId> {
        match self.pop() {
            Value::Token(t) => Some(t),
            Value::Null(_) => None,
            other => panic!("{other:?} is not a subtype of type 'Token?'"),
        }
    }

    /// Dart `pop() as List<T>?` for a list of nodes.
    pub(crate) fn pop_nodes_opt<T: ?Sized + NodeType>(&mut self) -> Option<Vec<Id<T>>> {
        match self.pop() {
            Value::Nodes(list) => Some(self.cast_list(list)),
            Value::Null(_) => None,
            other => panic!("{other:?} is not a subtype of type 'List<{}>?'", T::NAME),
        }
    }

    /// Dart `pop() as List<T>` for a list of nodes.
    pub(crate) fn pop_nodes<T: ?Sized + NodeType>(&mut self) -> Vec<Id<T>> {
        match self.pop_nodes_opt::<T>() {
            Some(list) => list,
            None => panic!("type 'Null' is not a subtype of type 'List<{}>'", T::NAME),
        }
    }

    /// Dart `pop() as List<AnnotationImpl>?` (the metadata).
    #[inline]
    pub(crate) fn pop_metadata(&mut self) -> Option<Vec<Id<dartr_ast::Annotation>>> {
        self.pop_nodes_opt::<dartr_ast::Annotation>()
    }

    /// Dart `pop() as List<Token>?`.
    pub(crate) fn pop_tokens_opt(&mut self) -> Option<Vec<TokenId>> {
        match self.pop() {
            Value::Tokens(list) => Some(list),
            Value::Null(_) => None,
            other => panic!("{other:?} is not a subtype of type 'List<Token>?'"),
        }
    }

    /// Dart `pop() as _Modifiers?`.
    pub(crate) fn pop_modifiers(&mut self) -> Option<Box<Modifiers>> {
        match self.pop() {
            Value::Modifiers(m) => Some(m),
            Value::Null(_) => None,
            other => panic!("{other:?} is not a subtype of type '_Modifiers?'"),
        }
    }

    /// Dart `popIfNotNull(value)`: pops when [value] is not `null`.
    #[inline]
    pub(crate) fn pop_if_not_null<V>(&mut self, value: Option<V>) -> Option<Value> {
        value.map(|_| self.pop())
    }

    /// Checks that every node of [list] is a `T` (the cast of a typed
    /// list).
    fn cast_list<T: ?Sized + NodeType>(&self, list: Vec<NodeId>) -> Vec<Id<T>> {
        list.into_iter()
            .map(|n| match self.ast.cast::<T>(n) {
                Some(id) => id,
                None => panic!(
                    "type '{}Impl' is not a subtype of type '{}'",
                    self.ast.kind(n).name(),
                    T::NAME
                ),
            })
            .collect()
    }

    /// Dart `stack.popList(count, list, null)`: the top [count] values in
    /// stack order (bottom first); typed `null`s become `None`.
    pub(crate) fn pop_list_values(&mut self, count: usize) -> Vec<Option<Value>> {
        assert!(self.stack.len() >= count);
        let start = self.stack.len() - count;
        self.stack
            .drain(start..)
            .map(|v| if v.is_null() { None } else { Some(v) })
            .collect()
    }

    /// Dart `popTypedList<Object>(count)`: `None` when [count] is 0; the
    /// values without the typed `null`s otherwise.
    pub(crate) fn pop_typed_list_values(&mut self, count: usize) -> Option<Vec<Value>> {
        if count == 0 {
            return None;
        }
        Some(self.pop_list_values(count).into_iter().flatten().collect())
    }

    /// Dart `popTypedList<T>(count)` for nodes: `None` when [count] is 0;
    /// the nodes without the typed `null`s otherwise.
    pub(crate) fn pop_typed_list<T: ?Sized + NodeType>(
        &mut self,
        count: usize,
    ) -> Option<Vec<Id<T>>> {
        let values = self.pop_typed_list_values(count)?;
        Some(
            values
                .into_iter()
                .map(|v| match self.value_as_node_opt::<T>(v) {
                    Some(id) => id,
                    None => unreachable!(),
                })
                .collect(),
        )
    }

    /// Dart `popTypedList2<T>(count)` (`popNonNullableNewList`): the top
    /// [count] nodes; panics on a typed `null`.
    pub(crate) fn pop_typed_list2<T: ?Sized + NodeType>(&mut self, count: usize) -> Vec<Id<T>> {
        assert!(self.stack.len() >= count);
        let start = self.stack.len() - count;
        let values: Vec<Value> = self.stack.drain(start..).collect();
        values
            .into_iter()
            .map(|v| match self.value_as_node_opt::<T>(v) {
                Some(id) => id,
                None => panic!("type 'Null' is not a subtype of type '{}'", T::NAME),
            })
            .collect()
    }

    /// Dart `popTypedList2<Token>(count)`.
    pub(crate) fn pop_typed_list2_tokens(&mut self, count: usize) -> Vec<TokenId> {
        assert!(self.stack.len() >= count);
        let start = self.stack.len() - count;
        self.stack
            .drain(start..)
            .map(|v| match v {
                Value::Token(t) => t,
                other => panic!("{other:?} is not a subtype of type 'Token'"),
            })
            .collect()
    }

    /// Dart `checkEmpty`: the stack must be empty at the end of a top level
    /// declaration (Dart throws an internal problem).
    pub(crate) fn check_empty(&self, _char_offset: u32) {
        if !self.stack.is_empty() {
            panic!(
                "Internal problem: AstBuilder stack not empty: {:?}",
                self.stack
            );
        }
    }

    /// Dart `internalProblem`: throws `UnsupportedError`.
    pub(crate) fn internal_problem(&self, message: &str) -> ! {
        panic!("Unsupported operation: {message}");
    }

    /// Dart `logEvent`: an unhandled event is an internal problem.
    pub(crate) fn log_event(&self, name: &str) -> ! {
        self.internal_problem(&format!("Unhandled {name} in AstBuilder."))
    }
}

/// The events that `StackListener` implements and `AstBuilder` does not
/// override.
impl AstBuilder {
    pub(crate) fn handle_no_name(&mut self, _token: TokenId) {
        self.push(NullValue::Identifier);
    }

    pub(crate) fn end_initializer(&mut self, _end_token: TokenId) {}

    pub(crate) fn handle_implicit_formal_parameters(&mut self, _token: TokenId) {
        self.push(NullValue::FormalParameters);
    }

    pub(crate) fn handle_no_type_arguments(&mut self, _token: TokenId) {
        self.push(NullValue::TypeArguments);
    }

    pub(crate) fn handle_no_type_variables(&mut self, _token: TokenId) {
        self.push(NullValue::NominalParameters);
    }

    pub(crate) fn handle_no_type(&mut self, _last_consumed: TokenId) {
        self.push(NullValue::TypeBuilder);
    }

    pub(crate) fn handle_no_formal_parameters(
        &mut self,
        _token: TokenId,
        _kind: dartr_parser::member_kind::MemberKind,
    ) {
        self.push(NullValue::FormalParameters);
    }

    pub(crate) fn handle_no_arguments(&mut self, _token: TokenId) {
        self.push(NullValue::Arguments);
    }

    pub(crate) fn handle_native_function_body_ignored(
        &mut self,
        _native_token: TokenId,
        _semicolon: TokenId,
    ) {
    }

    pub(crate) fn handle_native_function_body_skipped(
        &mut self,
        _native_token: TokenId,
        _semicolon: TokenId,
    ) {
    }

    pub(crate) fn handle_no_function_body(&mut self, _token: TokenId) {
        self.push(NullValue::FunctionBody);
    }

    pub(crate) fn handle_directives_only(&mut self) {
        // Discard the metadata.
        self.pop();
    }

    pub(crate) fn handle_extraneous_expression(
        &mut self,
        _token: TokenId,
        _message: dartr_diagnostics::cfe::CfeMessage,
    ) {
        // Discard the extraneous expression.
        self.pop();
    }

    pub(crate) fn end_catch_clause(&mut self, _token: TokenId) {}

    pub(crate) fn handle_unescape_error(
        &mut self,
        message: dartr_diagnostics::cfe::CfeMessage,
        token: TokenId,
        string_offset: i32,
        length: i32,
    ) {
        let offset = self.ast.tokens.offset(token) as i64 + string_offset as i64;
        self.add_problem(&message, offset as usize, length as usize);
    }
}
