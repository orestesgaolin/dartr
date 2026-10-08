// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/parser_impl.dart

//! The parser (Dart `class Parser`).
//!
//! The Dart class is split over several files; each file holds the methods
//! of a range of lines of `parser_impl.dart`, in the same order:
//!
//! | file | Dart lines | content |
//! |------|------------|---------|
//! | `mod.rs` | 274-420, 11353-11548 | fields, constructor, token helpers, error reporting |
//! | `part1.rs` | 419-2524 | compilation unit, directives, typedef, formal parameters |
//! | `part2.rs` | 2525-4392 | enum, class, mixin, extension, primary constructors, identifiers |
//! | `part3.rs` | 4393-6686 | fields, methods, initializers, `ensure*`, class members, function bodies |
//! | `part4.rs` | 6687-8450 | statements, expressions, precedence, cascades, primaries |
//! | `part5.rs` | 8451-10489 | literals, sends, arguments, `is`/`as`, declarations, `if`/`for`/`while` |
//! | `part6.rs` | 10490-12597 | blocks, `try`, `switch`, recovery, patterns, switch expressions |

use dartr_diagnostics::cfe::CfeMessage;
use dartr_syntax::{Keyword, KeywordStyle, Token, TokenId, TokenType, Tokens};

use crate::async_modifier::AsyncModifier;
use crate::experimental_features::{ExperimentalFeatures, ExperimentalFlag};
use crate::listener::Listener;
use crate::listener_stack::{Layer, ListenerStack};
use crate::loop_state::LoopState;
use crate::token_stream_rewriter::{TokenStreamChange, TokenStreamRewriter};
use crate::util::find_non_zero_length_token;

mod part1;
mod part2;
mod part3;
mod part4;
mod part5;
mod part6;

/// Dart `AwaitOrYieldContext`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AwaitOrYieldContext {
    Statement,
    UnaryExpression,
}

/// Data structure tracking additional information when parsing the
/// `forLoopParts` grammar production (Dart `ForPartsContext`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ForPartsContext {
    /// If `forLoopParts` began with `( 'final' | 'var' ) outerPattern`,
    /// followed by either `=` or `in`, the `final` or `var` keyword.
    /// Otherwise `None`.
    pub pattern_keyword: Option<TokenId>,
}

/// Enum describing the different contexts in which a pattern can occur
/// (Dart `PatternContext`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PatternContext {
    /// The pattern is part of a localVariableDeclaration or forLoopParts,
    /// meaning bare identifiers refer to freshly declared variables.
    Declaration,
    /// The pattern is part of a guardedPattern inside an if-case, switch
    /// expression, or switch statement, meaning bare identifiers refer to
    /// constants.
    Matching,
    /// The pattern is part of a pattern assignment, meaning bare identifiers
    /// refer to previously declared variables.
    Assignment,
}

impl PatternContext {
    pub fn is_refutable(self) -> bool {
        self == PatternContext::Matching
    }
}

/// Enum describing the different contexts in which a constant pattern is
/// parsed (Dart `ConstantPatternContext`). This restricts what expressions
/// are allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConstantPatternContext {
    /// A constant pattern is not currently being parsed.
    None,
    /// A constant pattern without a preceding `const` is being parsed.
    Implicit,
    /// A constant pattern with a preceding `const` is being parsed.
    Explicit,
    /// A constant pattern started with a `-` is being parsed.
    NumericLiteralOnly,
}

/// Dart `Parser._tokenRecoveryReplacements`.
pub(crate) const TOKEN_RECOVERY_REPLACEMENTS: &[(&str, &[TokenType])] = &[
    // E.g. in Kotlin binary operators are written out, see.
    // https://kotlinlang.org/api/latest/jvm/stdlib/kotlin/-int/.
    ("xor", &[TokenType::CARET]),
    ("and", &[TokenType::AMPERSAND, TokenType::AMPERSAND_AMPERSAND]),
    ("or", &[TokenType::BAR, TokenType::BAR_BAR]),
    ("shl", &[TokenType::LT_LT]),
    ("shr", &[TokenType::GT_GT]),
];

/// An event generating parser of Dart programs (Dart `Parser`). This
/// parser expects all tokens in a linked list (aka a token stream).
///
/// The listener `L` gets the events; the parser owns it and the token
/// arena (both in [`Parser::listener`], a [`ListenerStack`]). Use
/// [`Parser::new`], a parse method (for example [`Parser::parse_unit`]),
/// then [`Parser::into_parts`].
pub struct Parser<L: Listener> {
    /// Dart `listener`: the primary listener, the replacement listeners
    /// (layers) and the token arena.
    pub listener: ListenerStack<L>,

    pub may_parse_function_expressions: bool,

    /// Represents parser state: what asynchronous syntax is allowed in the
    /// function being currently parsed. In rare situations, this can be set
    /// by external clients, for example, to parse an expression outside a
    /// function.
    pub async_state: AsyncModifier,

    /// Represents parser state: whether parsing outside a loop, inside a
    /// loop, or inside a switch. This is used to determine whether break and
    /// continue statements are allowed.
    pub loop_state: LoopState,

    /// Dart `cachedRewriter` when it is an `UndoableTokenStreamRewriter`:
    /// its change log. `None`: the rewriter is a `TokenStreamRewriterImpl`.
    pub undo_log: Option<Vec<TokenStreamChange>>,

    /// If `true`, syntax like `foo<bar>.baz()` is parsed like an implicit
    /// creation expression. Otherwise it is parsed as a explicit
    /// instantiation followed by an invocation.
    pub use_implicit_creation_expression: bool,

    /// The experiments currently enabled.
    pub experimental_features: ExperimentalFeatures,

    /// `true` if the 'patterns' feature is enabled.
    pub is_patterns_feature_enabled: bool,

    /// `true` if the 'enhanced-parts' feature is enabled (Dart
    /// `_isEnhancedPartsFeatureEnabled`).
    pub(crate) is_enhanced_parts_feature_enabled: bool,

    /// `true` if the 'primary-constructors' feature is enabled.
    pub is_primary_constructors_feature_enabled: bool,

    /// `true` if the 'anonymous-methods' feature is enabled (Dart
    /// `_isAnonymousMethodsFeatureEnabled`).
    pub(crate) is_anonymous_methods_feature_enabled: bool,

    /// Indicates whether the last pattern parsed is allowed inside unary
    /// patterns. This is set by `parse_primary_pattern` and `parse_pattern`.
    pub is_last_pattern_allowed_inside_unary_pattern: bool,

    /// `true` if the 'augmentations' feature is enabled.
    pub is_augmentations_feature_enabled: bool,

    pub statement_depth: i32,
    pub expression_depth: i32,

    /// Dart `_recoverAtPrecedenceLevel`.
    pub(crate) recover_at_precedence_level: bool,
    /// Dart `_currentlyRecovering`.
    pub(crate) currently_recovering: bool,
}

impl<L: Listener> Parser<L> {
    /// Dart `Parser(listener, useImplicitCreationExpression:,
    /// experimentalFeatures:)`. [tokens] is the token arena that the parse
    /// methods read and change.
    pub fn new(
        listener: L,
        tokens: Tokens,
        use_implicit_creation_expression: bool,
        experimental_features: ExperimentalFeatures,
    ) -> Self {
        Parser {
            listener: ListenerStack::new(tokens, listener),
            may_parse_function_expressions: true,
            async_state: AsyncModifier::Sync,
            loop_state: LoopState::OutsideLoop,
            undo_log: None,
            use_implicit_creation_expression,
            experimental_features,
            is_patterns_feature_enabled: experimental_features
                .is_experiment_enabled(ExperimentalFlag::Patterns),
            is_enhanced_parts_feature_enabled: experimental_features
                .is_experiment_enabled(ExperimentalFlag::EnhancedParts),
            is_primary_constructors_feature_enabled: experimental_features
                .is_experiment_enabled(ExperimentalFlag::PrimaryConstructors),
            is_anonymous_methods_feature_enabled: experimental_features
                .is_experiment_enabled(ExperimentalFlag::AnonymousMethods),
            is_last_pattern_allowed_inside_unary_pattern: false,
            is_augmentations_feature_enabled: experimental_features
                .is_experiment_enabled(ExperimentalFlag::Augmentations),
            statement_depth: 0,
            expression_depth: 0,
            recover_at_precedence_level: false,
            currently_recovering: false,
        }
    }

    /// Returns the token arena and the listener.
    pub fn into_parts(self) -> (Tokens, L) {
        (self.listener.tokens, self.listener.primary)
    }

    // ---------------------------------------------------------------------
    // Getters (Dart lines 299-397).

    /// Dart `allowedToShortcutParseExpression`.
    #[inline]
    pub fn allowed_to_shortcut_parse_expression(&self) -> bool {
        true
    }

    #[inline]
    pub fn in_generator(&self) -> bool {
        self.async_state == AsyncModifier::AsyncStar || self.async_state == AsyncModifier::SyncStar
    }

    #[inline]
    pub fn in_async(&self) -> bool {
        self.async_state == AsyncModifier::Async || self.async_state == AsyncModifier::AsyncStar
    }

    #[inline]
    pub fn in_plain_sync(&self) -> bool {
        self.async_state == AsyncModifier::Sync
    }

    #[inline]
    pub fn is_break_allowed(&self) -> bool {
        self.loop_state != LoopState::OutsideLoop
    }

    #[inline]
    pub fn is_continue_allowed(&self) -> bool {
        self.loop_state == LoopState::InsideLoop
    }

    #[inline]
    pub fn is_continue_with_label_allowed(&self) -> bool {
        self.loop_state != LoopState::OutsideLoop
    }

    /// Dart `rewriter`: the current token stream rewriter (undoable while a
    /// look-ahead parse is running).
    #[inline]
    pub fn rewriter(&mut self) -> TokenStreamRewriter<'_> {
        TokenStreamRewriter {
            tokens: &mut self.listener.tokens,
            changes: self.undo_log.as_mut(),
        }
    }

    // ---------------------------------------------------------------------
    // Token helpers. Dart `token.next!` is `self.next(token)`, `token.isA(T)`
    // is `self.is_a(token, T)`, and so on.

    #[inline(always)]
    pub fn tokens(&self) -> &Tokens {
        &self.listener.tokens
    }

    #[inline(always)]
    pub fn tokens_mut(&mut self) -> &mut Tokens {
        &mut self.listener.tokens
    }

    #[inline(always)]
    pub fn token(&self, token: TokenId) -> &Token {
        self.listener.tokens.get(token)
    }

    /// Dart `token.next!`.
    #[inline(always)]
    pub fn next(&self, token: TokenId) -> TokenId {
        self.listener.tokens.next(token)
    }

    /// Dart `token.previous` (`None` for Dart `null`).
    #[inline(always)]
    pub fn previous(&self, token: TokenId) -> Option<TokenId> {
        self.listener.tokens.previous(token).get()
    }

    /// Dart `token.type`.
    #[inline(always)]
    pub fn ty(&self, token: TokenId) -> TokenType {
        self.listener.tokens.ty(token)
    }

    /// Dart `token.isA(type)`.
    #[inline(always)]
    pub fn is_a(&self, token: TokenId, ty: TokenType) -> bool {
        self.listener.tokens.ty(token) == ty
    }

    /// Dart `token.kind`.
    #[inline(always)]
    pub fn kind(&self, token: TokenId) -> i32 {
        self.listener.tokens.ty(token).kind()
    }

    /// Dart `token.lexeme`.
    #[inline(always)]
    pub fn lexeme(&self, token: TokenId) -> &str {
        self.listener.tokens.lexeme(token)
    }

    /// Dart `token.stringValue` (the lexeme of operators and keywords,
    /// `None` for identifiers, literals, ...).
    #[inline(always)]
    pub fn string_value(&self, token: TokenId) -> Option<&'static str> {
        self.listener.tokens.ty(token).string_value()
    }

    /// Dart `optional(value, token)` (util.dart).
    #[inline(always)]
    pub fn optional(&self, value: &str, token: TokenId) -> bool {
        self.string_value(token) == Some(value)
    }

    /// Dart `token.offset` / `token.charOffset`.
    #[inline(always)]
    pub fn offset(&self, token: TokenId) -> u32 {
        self.listener.tokens.get(token).offset
    }

    /// Dart `token.charOffset` (same as [`Self::offset`]).
    #[inline(always)]
    pub fn char_offset(&self, token: TokenId) -> u32 {
        self.listener.tokens.get(token).offset
    }

    /// Dart `token.length` / `token.charCount`.
    #[inline(always)]
    pub fn length(&self, token: TokenId) -> u32 {
        self.listener.tokens.get(token).length
    }

    /// Dart `token.end` / `token.charEnd`.
    #[inline(always)]
    pub fn end(&self, token: TokenId) -> u32 {
        self.listener.tokens.get(token).end()
    }

    /// Dart `token.isEof`.
    #[inline(always)]
    pub fn is_eof(&self, token: TokenId) -> bool {
        self.listener.tokens.ty(token) == TokenType::EOF
    }

    /// Dart `token.isSynthetic`.
    #[inline(always)]
    pub fn is_synthetic(&self, token: TokenId) -> bool {
        self.listener.tokens.get(token).is_synthetic()
    }

    /// Dart `token.isIdentifier`.
    #[inline(always)]
    pub fn is_identifier(&self, token: TokenId) -> bool {
        self.listener.tokens.get(token).is_identifier()
    }

    /// Dart `token.isKeyword`.
    #[inline(always)]
    pub fn is_keyword(&self, token: TokenId) -> bool {
        self.listener.tokens.ty(token).is_keyword()
    }

    /// Dart `token.isKeywordOrIdentifier`.
    #[inline(always)]
    pub fn is_keyword_or_identifier(&self, token: TokenId) -> bool {
        self.listener.tokens.get(token).is_keyword_or_identifier()
    }

    /// Dart `token.isOperator`.
    #[inline(always)]
    pub fn is_operator(&self, token: TokenId) -> bool {
        self.listener.tokens.ty(token).is_operator()
    }

    /// Dart `token.isUserDefinableOperator`.
    #[inline(always)]
    pub fn is_user_definable_operator(&self, token: TokenId) -> bool {
        self.listener.tokens.ty(token).is_user_definable_operator()
    }

    /// Dart `token.isModifier`.
    #[inline(always)]
    pub fn is_modifier(&self, token: TokenId) -> bool {
        self.listener.tokens.ty(token).is_modifier()
    }

    /// Dart `token.isTopLevelKeyword`.
    #[inline(always)]
    pub fn is_top_level_keyword(&self, token: TokenId) -> bool {
        self.listener.tokens.ty(token).is_top_level_keyword()
    }

    /// Dart `token.type.isBuiltIn`.
    #[inline(always)]
    pub fn is_built_in(&self, token: TokenId) -> bool {
        self.listener.tokens.ty(token).keyword_style() == Some(KeywordStyle::BuiltIn)
    }

    /// Dart `token.type.isPseudo`.
    #[inline(always)]
    pub fn is_pseudo(&self, token: TokenId) -> bool {
        self.listener.tokens.ty(token).keyword_style() == Some(KeywordStyle::Pseudo)
    }

    /// Dart `token.type.isReservedWord`.
    #[inline(always)]
    pub fn is_reserved_word(&self, token: TokenId) -> bool {
        self.listener.tokens.ty(token).keyword_style() == Some(KeywordStyle::Reserved)
    }

    /// Dart `token.endGroup` (`None` for Dart `null`).
    #[inline(always)]
    pub fn end_group(&self, token: TokenId) -> Option<TokenId> {
        self.listener.tokens.get(token).end_group.get()
    }

    /// Dart `token.precedingComments`.
    #[inline(always)]
    pub fn preceding_comments(&self, token: TokenId) -> Option<TokenId> {
        self.listener.tokens.get(token).preceding_comments.get()
    }

    /// Dart `token is ErrorToken`.
    #[inline(always)]
    pub fn is_error_token(&self, token: TokenId) -> bool {
        self.listener.tokens.get(token).is_error()
    }

    /// Dart `token.isA(Keyword.X)` for a keyword written as text, for code
    /// that compares `token.stringValue` with a keyword lexeme.
    #[inline(always)]
    pub fn is_keyword_type(&self, token: TokenId, keyword: TokenType) -> bool {
        debug_assert!(keyword.is_keyword() || keyword == Keyword::AS);
        self.is_a(token, keyword)
    }

    // ---------------------------------------------------------------------
    // Listener layers: the Dart code that replaces `listener` for a while.

    /// Dart `listener = new NullListener()`; restore with
    /// [`Self::pop_null_listener`].
    #[inline]
    pub fn push_null_listener(&mut self) {
        self.listener.push_layer(Layer::Null { has_errors: false });
    }

    /// Restores the listener replaced by [`Self::push_null_listener`] and
    /// returns `nullListener.hasErrors`.
    #[inline]
    pub fn pop_null_listener(&mut self) -> bool {
        match self.listener.pop_layer() {
            Layer::Null { has_errors } => has_errors,
            layer => panic!("expected a NullListener layer, found {layer:?}"),
        }
    }

    /// Dart `listener = new ForwardingListener()` (no target: drops all
    /// events); restore with [`Self::pop_layer`].
    #[inline]
    pub fn push_silent_listener(&mut self) {
        self.listener.push_layer(Layer::Silent);
    }

    /// Dart `listener = new ForwardingListener(listener)..forwardErrors =
    /// false`; restore with [`Self::pop_layer`].
    #[inline]
    pub fn push_no_errors_listener(&mut self) {
        self.listener.push_layer(Layer::NoErrors);
    }

    /// Restores the listener that was current before the last push.
    #[inline]
    pub fn pop_layer(&mut self) -> Layer {
        self.listener.pop_layer()
    }

    /// Starts an undoable look-ahead: Dart `cachedRewriter = new
    /// UndoableTokenStreamRewriter()`. Returns the previous rewriter state
    /// for [`Self::end_undoable_rewriter`].
    #[inline]
    pub fn begin_undoable_rewriter(&mut self) -> Option<Vec<TokenStreamChange>> {
        self.undo_log.replace(Vec::new())
    }

    /// Dart `undoableTokenStreamRewriter.undo(); cachedRewriter =
    /// originalRewriter`.
    #[inline]
    pub fn end_undoable_rewriter(&mut self, original: Option<Vec<TokenStreamChange>>) {
        let mut log = std::mem::replace(&mut self.undo_log, original).expect("no undoable rewriter");
        crate::token_stream_rewriter::undo(&mut self.listener.tokens, &mut log);
    }

    // ---------------------------------------------------------------------
    // Dart lines 11353-11548: error reporting and token helpers.

    pub fn report_recoverable_error(&mut self, token: TokenId, message: CfeMessage) {
        // Find a non-synthetic token on which to report the error.
        let token = find_non_zero_length_token(self.tokens(), token);
        self.listener.handle_recoverable_error(message, token, token);
    }

    pub fn report_recoverable_error_with_end(
        &mut self,
        start_token: TokenId,
        end_token: TokenId,
        message: CfeMessage,
    ) {
        self.listener
            .handle_recoverable_error(message, start_token, end_token);
    }

    pub fn report_experiment_not_enabled(
        &mut self,
        experimental_flag: ExperimentalFlag,
        start_token: TokenId,
        end_token: TokenId,
    ) {
        self.listener
            .handle_experiment_not_enabled(experimental_flag, start_token, end_token);
    }

    /// Dart `reportRecoverableErrorWithToken(token, template)`; the template
    /// is a `cfe_codes` function that takes the lexeme of the token.
    pub fn report_recoverable_error_with_token(
        &mut self,
        token: TokenId,
        template: fn(&str) -> CfeMessage,
    ) {
        // Find a non-synthetic token on which to report the error.
        let token = find_non_zero_length_token(self.tokens(), token);
        let message = template(self.lexeme(token));
        self.listener.handle_recoverable_error(message, token, token);
    }

    pub fn report_all_error_tokens(&mut self, mut token: TokenId) -> TokenId {
        while self.is_error_token(token) {
            self.listener.handle_error_token(token);
            token = self.next(token);
        }
        token
    }

    pub fn skip_error_tokens(&self, mut token: TokenId) -> TokenId {
        while self.is_error_token(token) {
            token = self.next(token);
        }
        token
    }

    /// Create a short token chain from the [begin_token] and [end_token] and
    /// return the [begin_token].
    pub fn link(&mut self, begin_token: TokenId, end_token: TokenId) -> TokenId {
        let tokens = self.tokens_mut();
        tokens.set_next(begin_token, end_token);
        tokens.get_mut(begin_token).end_group = end_token;
        begin_token
    }

    /// Create and return a token whose next token is the given [token].
    pub fn synthetic_previous_token(&mut self, token: TokenId) -> TokenId {
        synthetic_previous_token(self.tokens_mut(), token)
    }

    /// Return the first dartdoc comment token preceding the given token or
    /// `None` if no dartdoc token is found.
    pub fn find_dart_doc(&self, token: TokenId) -> Option<TokenId> {
        find_dart_doc(self.tokens(), token)
    }

    pub fn previous_token(&self, mut before_token: TokenId, token: TokenId) -> TokenId {
        let mut next = self.next(before_token);
        while next != token && next != before_token {
            before_token = next;
            next = self.next(before_token);
        }
        before_token
    }

    pub fn not_eof_or_type(&self, ty: TokenType, token: TokenId) -> bool {
        !self.is_a(token, TokenType::EOF) && !self.is_a(token, ty)
    }
}

/// Dart `Parser.syntheticPreviousToken`: the previous token of [token], or
/// a new EOF token with offset -1 whose `next` is [token].
pub fn synthetic_previous_token(tokens: &mut Tokens, token: TokenId) -> TokenId {
    // Return the previous token if there is one so that any token inserted
    // before `token` will be properly inserted into the token stream.
    if let Some(previous) = tokens.previous(token).get() {
        return previous;
    }
    let byte = tokens.byte_offset(token);
    let before = tokens.push_synthetic(TokenType::EOF, u32::MAX, byte);
    tokens.get_mut(before).next = token;
    before
}

/// Dart `Parser.findDartDoc`: the first dartdoc comment token preceding the
/// given token or `None` if no dartdoc token is found.
pub fn find_dart_doc(tokens: &Tokens, token: TokenId) -> Option<TokenId> {
    let mut comments = tokens.get(token).preceding_comments;
    let mut dartdoc = None;
    let mut is_multiline = false;
    while comments.is_some() {
        let lexeme = tokens.lexeme(comments);
        if lexeme.starts_with("///") {
            if !is_multiline {
                dartdoc = Some(comments);
                is_multiline = true;
            }
        } else if lexeme.starts_with("/**") {
            dartdoc = Some(comments);
            is_multiline = false;
        }
        comments = tokens.get(comments).next;
    }
    dartdoc
}
