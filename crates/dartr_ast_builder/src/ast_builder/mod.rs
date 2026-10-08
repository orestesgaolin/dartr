// Dart source: pkg/analyzer/lib/src/fasta/ast_builder.dart

//! The AST builder (Dart `AstBuilder`): a parser listener that builds the
//! analyzer AST ([`dartr_ast::Ast`]).
//!
//! The Dart class is split over several files; each file holds the methods
//! of a range of lines of `ast_builder.dart`, in the same order:
//!
//! | file | Dart lines | content |
//! |------|------------|---------|
//! | `mod.rs` | 53-200, 639-905, 4168, 5341-5363, 5719-6863 | fields, constructor, error reporting, initializers, `doDotExpression`, `doInvocation`, private helpers, helper classes |
//! | `part1.rs` | 204-1693 | `beginAsOperatorType` .. `endFields` |
//! | `part2.rs` | 1695-3098 | `endForControlFlow` .. `endSwitchCaseWhenClause` |
//! | `part3.rs` | 3100-4397 | `endSwitchExpression` .. `handleFormalParameterWithoutValue` |
//! | `part4.rs` | 4400-5717 | `handleIdentifier` .. `handleWildcardPattern` |
//! | `listener_impl.rs` | | the `Listener` implementation (generated) |
//!
//! # Port conventions
//!
//! - Every event is an inherent method with the Dart name in snake_case and
//!   the arguments of the `Listener` method without the token arena. The
//!   generated `Listener` implementation (`listener_impl.rs`,
//!   `tools/codegen/gen_builder_listener.py`) moves the parser's token arena
//!   into `self.ast.tokens` for the duration of each event, so the builder
//!   reads tokens with `self.ast.tokens` and rewrites them with
//!   `TokenStreamRewriter::new(&mut self.ast.tokens)` (Dart
//!   `parser.rewriter`).
//! - Dart private members `_foo` are `foo`, or `foo_impl` where that name
//!   is taken by an event (`_beginMethod` is `begin_method_impl`).
//! - The stack holds [`Value`]s; the casts of the Dart code are the typed
//!   pops of `crate::stack` (`pop() as T` is `pop_node::<T>()`).
//! - Nodes are made with `self.ast.add(NodeStruct { .. })`; Dart lists of
//!   nodes are `Vec<Id<T>>` until they become a `NodeList` with
//!   `self.ast.new_list(..)` (make the list before the `add` call). A
//!   `List<AnnotationImpl>?` is `Option<Vec<Id<Annotation>>>` (`None` is an
//!   empty `NodeList`, see [`AstBuilder::node_list`]).
//! - Dart `throw`/`internalProblem` are panics.

use dartr_ast::{
    Annotation, Argument, ArgumentList, AssignmentExpression, Ast, BlockClassBody, BlockEnumBody,
    CascadeExpression, ClassBody, ClassDeclaration, ClassMember, ClassNamePart, Comment,
    CompilationUnitMember, ConstructorDeclaration, ConstructorFieldInitializer,
    ConstructorInitializer, ConstructorName, Directive, EmptyClassBody, EmptyEnumBody,
    EmptyFunctionBody, EnumBody, EnumConstantDeclaration, EnumDeclaration, Expression,
    ExtendsClause, ExtensionDeclaration, ExtensionOnClause, ExtensionTypeDeclaration,
    FieldDeclaration, FieldFormalParameter, FormalParameter, FormalParameterList, FunctionBody,
    FunctionExpressionInvocation, Id, Identifier, ImplementsClause, IndexExpression,
    InstanceCreationExpression, MethodDeclaration, MethodInvocation, MixinDeclaration,
    MixinOnClause, NameWithTypeParameters, NamedType, NativeClause, NodeId, NodeList,
    ParameterKind, PatternField, PrefixedIdentifier, PrimaryConstructorDeclaration,
    PrimaryConstructorName, PropertyAccess, RedirectingConstructorInvocation,
    RegularFormalParameter, ScriptTag, SimpleIdentifier, StringLiteral, SuperConstructorInvocation,
    SuperExpression, ThisExpression, TypeAnnotation, TypeArgumentList, TypeParameterList,
    VariableDeclaration, VariableDeclarationList, WithClause,
};
use dartr_diagnostics::cfe::{CfeMessage, PseudoSharedCode};
use dartr_diagnostics::{LocatableDiagnostic, cfe_codes, diag};
use dartr_parser::experimental_features::{ExperimentalFeatures, ExperimentalFlag};
use dartr_parser::formal_parameter_kind::FormalParameterKind;
use dartr_parser::parser_impl::find_dart_doc;
use dartr_parser::token_stream_rewriter::TokenStreamRewriter;
use dartr_syntax::analyzer_scanner::{CURRENT_LANGUAGE_VERSION, translate_error_token};
use dartr_syntax::{Keyword, Token, TokenId, TokenType, Tokens};

use crate::error_converter::FastaErrorReporter;
use crate::parse::LibraryLanguageVersion;
use crate::stack::Value;

mod listener_impl;
mod part1;
mod part2;
mod part3;
mod part4;

/// A parser listener that builds the analyzer's AST structure (Dart
/// `AstBuilder`).
pub struct AstBuilder {
    /// The AST being built. Between events `ast.tokens` is empty: the
    /// parser owns the token arena and moves it in for each event (see the
    /// module documentation).
    pub ast: Ast,
    /// Dart `diagnosticReporter` (a `FastaErrorReporter`).
    pub diagnostic_reporter: FastaErrorReporter,
    /// Dart `fileUri`.
    pub file_uri: String,
    /// Dart `uri`.
    pub uri: String,
    pub script_tag: Option<Id<ScriptTag>>,
    pub directives: Vec<Id<Directive>>,
    pub declarations: Vec<Id<CompilationUnitMember>>,
    pub invalid_nodes: Vec<NodeId>,
    /// The `StackListener` stack.
    pub(crate) stack: Vec<Value>,
    /// The class like declaration being parsed.
    pub(crate) class_like_builder: Option<Box<ClassLikeDeclarationBuilder>>,
    /// If true, this is building a full AST. Otherwise, only create method
    /// bodies.
    pub is_full_ast: bool,
    /// `true` if the `native` clause is allowed in class, method, and
    /// function declarations.
    pub allow_native_clause: bool,
    pub(crate) native_name: Option<Id<StringLiteral>>,
    pub parse_function_bodies: bool,
    /// Whether the 'augmentations' feature is enabled.
    pub enable_augmentations: bool,
    /// `true` if triple-shift behavior is enabled
    pub enable_triple_shift: bool,
    /// `true` if nonfunction-type-aliases behavior is enabled
    pub enable_non_function_type_aliases: bool,
    /// `true` if variance behavior is enabled
    pub enable_variance: bool,
    /// `true` if constructor tearoffs are enabled
    pub enable_constructor_tearoffs: bool,
    /// `true` if named arguments anywhere are enabled
    pub enable_named_arguments_anywhere: bool,
    /// `true` if super parameters are enabled
    pub enable_super_parameters: bool,
    /// `true` if enhanced enums are enabled
    pub enable_enhanced_enums: bool,
    /// Whether the 'enhanced_parts' feature is enabled.
    pub enable_enhanced_parts: bool,
    /// `true` if records are enabled
    pub enable_records: bool,
    /// `true` if unnamed-library behavior is enabled
    pub enable_unnamed_libraries: bool,
    /// `true` if inline-class is enabled
    pub enable_inline_class: bool,
    /// `true` if sealed-class is enabled
    pub enable_sealed_class: bool,
    /// `true` if class-modifiers is enabled
    pub enable_class_modifiers: bool,
    /// `true` if null-aware elements is enabled
    pub enable_null_aware_elements: bool,
    /// `true` if dot-shorthands is enabled
    pub enabled_dot_shorthands: bool,
    /// `true` if digit-separators is enabled.
    pub(crate) enable_digit_separators: bool,
    /// Dart `_featureSet`.
    pub feature_set: ExperimentalFeatures,
    /// Dart `_languageVersion`.
    pub language_version: LibraryLanguageVersion,
    /// The expressions with `DotShorthandMixin.isDotShorthand = true` (set
    /// by `handleDotShorthandContext`; the node structs have no field for
    /// this resolution flag).
    pub dot_shorthands: Vec<NodeId>,
}

impl AstBuilder {
    /// Dart `AstBuilder(errorReporter, fileUri, isFullAst, featureSet,
    /// languageVersion, lineInfo, [uri])`. The token arena is given to the
    /// parser; `self.ast.tokens` is filled in after parsing.
    pub fn new(
        file_uri: String,
        is_full_ast: bool,
        feature_set: ExperimentalFeatures,
        language_version: LibraryLanguageVersion,
        uri: Option<String>,
    ) -> AstBuilder {
        let on = |flag| feature_set.is_experiment_enabled(flag);
        AstBuilder {
            ast: Ast::default(),
            diagnostic_reporter: FastaErrorReporter::new(),
            uri: uri.unwrap_or_else(|| file_uri.clone()),
            file_uri,
            script_tag: None,
            directives: Vec::new(),
            declarations: Vec::new(),
            invalid_nodes: Vec::new(),
            stack: Vec::with_capacity(64),
            class_like_builder: None,
            is_full_ast,
            allow_native_clause: false,
            native_name: None,
            parse_function_bodies: true,
            enable_augmentations: on(ExperimentalFlag::Augmentations),
            enable_triple_shift: on(ExperimentalFlag::TripleShift),
            enable_non_function_type_aliases: on(ExperimentalFlag::NonfunctionTypeAliases),
            enable_variance: on(ExperimentalFlag::Variance),
            enable_constructor_tearoffs: on(ExperimentalFlag::ConstructorTearoffs),
            enable_named_arguments_anywhere: on(ExperimentalFlag::NamedArgumentsAnywhere),
            enable_super_parameters: on(ExperimentalFlag::SuperParameters),
            enable_enhanced_enums: on(ExperimentalFlag::EnhancedEnums),
            enable_enhanced_parts: on(ExperimentalFlag::EnhancedParts),
            enable_records: on(ExperimentalFlag::Records),
            enable_unnamed_libraries: on(ExperimentalFlag::UnnamedLibraries),
            enable_inline_class: on(ExperimentalFlag::InlineClass),
            enable_sealed_class: on(ExperimentalFlag::SealedClass),
            enable_class_modifiers: on(ExperimentalFlag::ClassModifiers),
            enable_null_aware_elements: on(ExperimentalFlag::NullAwareElements),
            enabled_dot_shorthands: on(ExperimentalFlag::DotShorthands),
            enable_digit_separators: on(ExperimentalFlag::DigitSeparators),
            feature_set,
            language_version,
            dot_shorthands: Vec::new(),
        }
    }

    /// Dart `_featureSet.isEnabled(feature)`.
    #[inline]
    pub(crate) fn is_enabled(&self, flag: ExperimentalFlag) -> bool {
        self.feature_set.is_experiment_enabled(flag)
    }

    /// Dart `parser.rewriter`.
    #[inline]
    pub(crate) fn rewriter(&mut self) -> TokenStreamRewriter<'_> {
        TokenStreamRewriter::new(&mut self.ast.tokens)
    }

    /// Dart `optional(value, token)`.
    #[inline]
    pub(crate) fn optional(&self, value: &str, token: TokenId) -> bool {
        dartr_parser::util::optional(&self.ast.tokens, value, token)
    }

    /// Dart `token.lexeme`.
    #[inline]
    pub(crate) fn lexeme(&self, token: TokenId) -> &str {
        self.ast.tokens.lexeme(token)
    }

    /// Dart `token.next!`.
    #[inline]
    pub(crate) fn next(&self, token: TokenId) -> TokenId {
        self.ast.tokens.next(token)
    }

    /// Dart `token.endGroup!`.
    #[inline]
    pub(crate) fn end_group(&self, token: TokenId) -> TokenId {
        let end = self.ast.tokens.get(token).end_group;
        assert!(
            end.is_some(),
            "Null check operator used on a null value (endGroup)"
        );
        end
    }

    /// Dart `node.beginToken`.
    #[inline]
    pub(crate) fn begin_token(&self, node: impl Into<NodeId>) -> TokenId {
        self.ast.begin_token(node)
    }

    /// Dart `node.endToken`.
    #[inline]
    pub(crate) fn end_token(&self, node: impl Into<NodeId>) -> TokenId {
        self.ast.end_token(node)
    }

    /// A Dart `List<T>?` of nodes as a `NodeList` (`null` is empty).
    #[inline]
    pub(crate) fn node_list<T: ?Sized>(&mut self, nodes: Option<Vec<Id<T>>>) -> NodeList<T> {
        match nodes {
            Some(n) => self.ast.new_list(n),
            None => NodeList::EMPTY,
        }
    }

    /// Dart `TokenFactory.tokenFromType(type)` (`Tokens.semicolon()`,
    /// `Tokens.openCurlyBracket()`, ...): a token at offset 0 that is not in
    /// the token stream.
    pub(crate) fn detached_token(&mut self, ty: TokenType) -> TokenId {
        self.ast.tokens.push(Token::fixed(ty, 0, 0, false))
    }

    /// Dart `isDartLibrary`.
    pub fn is_dart_library(&self) -> bool {
        self.uri.starts_with("dart:") || self.uri.starts_with("org-dartlang-sdk:")
    }

    /// Dart `addProblem`.
    pub(crate) fn add_problem(&mut self, message: &CfeMessage, char_offset: usize, length: usize) {
        if self.directives.is_empty()
            && message.code.pseudo_shared_code == Some(PseudoSharedCode::NonPartOfDirectiveInPart)
        {
            let message = cfe_codes::directive_after_declaration();
            self.diagnostic_reporter
                .report_message(&message, char_offset, length);
            return;
        }
        self.diagnostic_reporter
            .report_message(message, char_offset, length);
    }

    /// Dart `diagnosticReporter.diagnosticReporter?.report(diagnostic.at(node))`.
    pub(crate) fn report_at_node(
        &mut self,
        diagnostic: LocatableDiagnostic,
        node: impl Into<NodeId>,
    ) {
        let node = node.into();
        let offset = self.ast.offset(node) as usize;
        let length = self.ast.length(node) as usize;
        self.diagnostic_reporter
            .report(diagnostic.at_offset(offset, length));
    }

    /// Dart `diagnosticReporter.diagnosticReporter?.report(diagnostic.at(token))`.
    pub(crate) fn report_at_token(&mut self, diagnostic: LocatableDiagnostic, token: TokenId) {
        let t = self.ast.tokens.get(token);
        let (offset, length) = (t.offset as usize, t.length as usize);
        self.diagnostic_reporter
            .report(diagnostic.at_offset(offset, length));
    }

    // ---------------------------------------------------------------------
    // Dart lines 639-905.

    pub(crate) fn build_initializer(
        &mut self,
        initializer_object: &Value,
    ) -> Option<Id<ConstructorInitializer>> {
        let Value::Node(node) = *initializer_object else {
            return None;
        };
        if let Some(invocation) = self.ast.cast::<FunctionExpressionInvocation>(node) {
            let n = self.ast[invocation].clone();
            let function = n.function;
            if let Some(s) = self.ast.cast::<SuperExpression>(function) {
                let super_keyword = self.ast[s].super_keyword;
                return Some(
                    self.ast
                        .add(SuperConstructorInvocation {
                            super_keyword,
                            period: None,
                            constructor_name: None,
                            argument_list: n.argument_list,
                        })
                        .upcast(),
                );
            }
            if let Some(t) = self.ast.cast::<ThisExpression>(function) {
                let this_keyword = self.ast[t].this_keyword;
                return Some(
                    self.ast
                        .add(RedirectingConstructorInvocation {
                            this_keyword,
                            period: None,
                            constructor_name: None,
                            argument_list: n.argument_list,
                        })
                        .upcast(),
                );
            }
            return None;
        }

        if let Some(invocation) = self.ast.cast::<MethodInvocation>(node) {
            let n = self.ast[invocation].clone();
            let target = n.target;
            if let Some(s) = target.and_then(|t| self.ast.cast::<SuperExpression>(t)) {
                let super_keyword = self.ast[s].super_keyword;
                return Some(
                    self.ast
                        .add(SuperConstructorInvocation {
                            super_keyword,
                            period: n.operator,
                            constructor_name: Some(n.method_name),
                            argument_list: n.argument_list,
                        })
                        .upcast(),
                );
            }
            if let Some(t) = target.and_then(|t| self.ast.cast::<ThisExpression>(t)) {
                let this_keyword = self.ast[t].this_keyword;
                return Some(
                    self.ast
                        .add(RedirectingConstructorInvocation {
                            this_keyword,
                            period: n.operator,
                            constructor_name: Some(n.method_name),
                            argument_list: n.argument_list,
                        })
                        .upcast(),
                );
            }
            return self.build_initializer_target_expression_recovery(target);
        }

        if let Some(access) = self.ast.cast::<PropertyAccess>(node) {
            let target = self.ast[access].target;
            return self.build_initializer_target_expression_recovery(target);
        }

        if let Some(assignment) = self.ast.cast::<AssignmentExpression>(node) {
            let a = self.ast[assignment].clone();
            let mut this_keyword = None;
            let mut period = None;
            let field_name;
            let left = a.left_hand_side;
            if let Some(access) = self.ast.cast::<PropertyAccess>(left) {
                let p = self.ast[access].clone();
                if let Some(t) = p.target.and_then(|t| self.ast.cast::<ThisExpression>(t)) {
                    this_keyword = Some(self.ast[t].this_keyword);
                    period = Some(p.operator);
                } else {
                    // Recovery:
                    // Parser has reported FieldInitializedOutsideDeclaringClass.
                }
                field_name = p.property_name;
            } else if let Some(identifier) = self.ast.cast::<SimpleIdentifier>(left) {
                field_name = identifier;
            } else {
                return None;
            }
            return Some(
                self.ast
                    .add(ConstructorFieldInitializer {
                        this_keyword,
                        period,
                        field_name,
                        equals: a.operator,
                        expression: a.right_hand_side,
                    })
                    .upcast(),
            );
        }

        if let Some(assert) = self.ast.cast::<dartr_ast::AssertInitializer>(node) {
            return Some(assert.upcast());
        }

        if let Some(index) = self.ast.cast::<IndexExpression>(node) {
            let target = self.ast[index].target;
            return self.build_initializer_target_expression_recovery(target);
        }

        if let Some(cascade) = self.ast.cast::<CascadeExpression>(node) {
            let target = self.ast[cascade].target;
            return self.build_initializer_target_expression_recovery(Some(target));
        }

        None
    }

    pub(crate) fn build_initializer_target_expression_recovery(
        &mut self,
        mut target: Option<Id<Expression>>,
    ) -> Option<Id<ConstructorInitializer>> {
        let mut argument_list: Option<Id<ArgumentList>> = None;
        while let Some(t) = target {
            if let Some(f) = self.ast.cast::<FunctionExpressionInvocation>(t) {
                argument_list = Some(self.ast[f].argument_list);
                target = Some(self.ast[f].function);
            } else if let Some(m) = self.ast.cast::<MethodInvocation>(t) {
                argument_list = Some(self.ast[m].argument_list);
                target = self.ast[m].target;
            } else if let Some(p) = self.ast.cast::<PropertyAccess>(t) {
                argument_list = None;
                target = self.ast[p].target;
            } else {
                break;
            }
        }
        let target = target?;
        if let Some(s) = self.ast.cast::<SuperExpression>(target) {
            let super_keyword = self.ast[s].super_keyword;
            // This error is also reported in the body builder
            self.handle_recoverable_error(
                cfe_codes::invalid_super_in_initializer(),
                super_keyword,
                super_keyword,
            );
            let argument_list = match argument_list {
                Some(a) => a,
                None => self.synthetic_argument_list(super_keyword),
            };
            return Some(
                self.ast
                    .add(SuperConstructorInvocation {
                        super_keyword,
                        period: None,
                        constructor_name: None,
                        argument_list,
                    })
                    .upcast(),
            );
        } else if let Some(t) = self.ast.cast::<ThisExpression>(target) {
            let this_keyword = self.ast[t].this_keyword;
            // This error is also reported in the body builder
            self.handle_recoverable_error(
                cfe_codes::invalid_this_in_initializer(),
                this_keyword,
                this_keyword,
            );
            let argument_list = match argument_list {
                Some(a) => a,
                None => self.synthetic_argument_list(this_keyword),
            };
            return Some(
                self.ast
                    .add(RedirectingConstructorInvocation {
                        this_keyword,
                        period: None,
                        constructor_name: None,
                        argument_list,
                    })
                    .upcast(),
            );
        }
        None
    }

    pub(crate) fn do_dot_expression(&mut self, dot: TokenId) {
        let identifier_or_invoke = self.pop_node::<Expression>();
        let receiver = self.pop_node_opt::<Expression>();
        if let Some(identifier) = self.ast.cast::<SimpleIdentifier>(identifier_or_invoke) {
            let receiver_identifier = receiver.and_then(|r| self.ast.cast::<SimpleIdentifier>(r));
            if let (Some(prefix), true) = (
                receiver_identifier,
                self.ast.tokens.ty(dot) == TokenType::PERIOD,
            ) {
                let node = self.ast.add(PrefixedIdentifier {
                    prefix,
                    period: dot,
                    identifier,
                });
                self.push(node);
            } else {
                let node = self.ast.add(PropertyAccess {
                    target: receiver,
                    operator: dot,
                    property_name: identifier,
                });
                self.push(node);
            }
        } else if let Some(invocation) = self.ast.cast::<MethodInvocation>(identifier_or_invoke) {
            debug_assert!(self.ast[invocation].target.is_none());
            self.ast.modify(invocation, |m| {
                m.target = receiver;
                m.operator = Some(dot);
            });
            self.push(invocation);
        } else {
            // This same error is reported in BodyBuilder.doDotOrCascadeExpression
            let token = self.begin_token(identifier_or_invoke);
            let lexeme = self.lexeme(token).to_string();
            self.handle_recoverable_error(cfe_codes::expected_identifier(&lexeme), token, token);
            let identifier = self.ast.add(SimpleIdentifier { token });
            let node = self.ast.add(PropertyAccess {
                target: receiver,
                operator: dot,
                property_name: identifier,
            });
            self.push(node);
        }
    }

    pub(crate) fn do_invocation(
        &mut self,
        type_arguments: Option<Id<TypeArgumentList>>,
        argument_list: Id<ArgumentList>,
    ) {
        let receiver = self.pop_node::<Expression>();
        if let Some(method_name) = self.ast.cast::<SimpleIdentifier>(receiver) {
            let node = self.ast.add(MethodInvocation {
                target: None,
                operator: None,
                method_name,
                type_arguments,
                argument_list,
            });
            self.push(node);
        } else {
            let node = self.ast.add(FunctionExpressionInvocation {
                function: receiver,
                type_arguments,
                argument_list,
            });
            self.push(node);
        }
    }

    pub(crate) fn do_property_get(&mut self) {}

    // ---------------------------------------------------------------------
    // Dart line 4168.

    pub(crate) fn handle_error_token(&mut self, token: TokenId) {
        let reporter = &mut self.diagnostic_reporter;
        translate_error_token(&self.ast.tokens, token, &mut |d| {
            reporter.report_scanner_error(d)
        });
    }

    // ---------------------------------------------------------------------
    // Dart lines 5341-5363.

    pub(crate) fn handle_recoverable_error(
        &mut self,
        message: CfeMessage,
        start_token: TokenId,
        end_token: TokenId,
    ) {
        // Ignore this error until we deprecate `native` support.
        if message.code == &cfe_codes::NATIVE_CLAUSE_SHOULD_BE_ANNOTATION
            && self.allow_native_clause
        {
            return;
        } else if message.code == &cfe_codes::BUILT_IN_IDENTIFIER_IN_DECLARATION {
            // Allow e.g. 'class Function' in sdk.
            if self.is_dart_library() {
                return;
            }
        }
        if message.code.pseudo_shared_code.is_none() && self.ast.tokens.get(start_token).is_error()
        {
            let reporter = &mut self.diagnostic_reporter;
            translate_error_token(&self.ast.tokens, start_token, &mut |d| {
                reporter.report_scanner_error(d)
            });
        } else {
            let offset = self.ast.tokens.offset(start_token);
            let length = self.ast.tokens.get(end_token).end().wrapping_sub(offset);
            self.add_problem(&message, offset as usize, length as usize);
        }
    }

    // ---------------------------------------------------------------------
    // Dart lines 5719-6385.

    /// Parses the comment references in a sequence of comment tokens where
    /// [dartdoc] is the first token in the sequence (Dart
    /// `parseDocComment`).
    pub(crate) fn parse_doc_comment(&mut self, dartdoc: TokenId) -> Id<Comment> {
        crate::doc_comment_builder::DocCommentBuilder::new(
            &mut self.ast,
            &mut self.diagnostic_reporter,
            &self.uri,
            self.feature_set,
            self.language_version,
            dartdoc,
        )
        .build()
    }

    /// Dart `popCollectionElements`.
    pub(crate) fn pop_collection_elements(
        &mut self,
        count: usize,
    ) -> Vec<Id<dartr_ast::CollectionElement>> {
        let mut elements = Vec::with_capacity(count);
        for _ in 0..count {
            elements.push(self.pop_node::<dartr_ast::CollectionElement>());
        }
        elements.reverse();
        elements
    }

    /// Dart `reportErrorIfNullableType` (no caller in the AST builder).
    #[allow(dead_code)]
    pub(crate) fn report_error_if_nullable_type(&mut self, question_mark: Option<TokenId>) {
        if let Some(question_mark) = question_mark {
            self.report_feature_not_enabled(ExperimentalFlag::NonNullable, question_mark, None);
        }
    }

    /// Dart `Listener.reportVarianceModifierNotEnabled` (the default of the
    /// listener: `handleExperimentNotEnabled`, which reports through
    /// `handleRecoverableError`).
    pub(crate) fn report_variance_modifier_not_enabled(&mut self, variance: Option<TokenId>) {
        if let Some(variance) = variance {
            self.handle_recoverable_error(
                dartr_parser::experimental_features::get_experiment_not_enabled_message(
                    ExperimentalFlag::Variance,
                ),
                variance,
                variance,
            );
        }
    }

    /// Dart `reportErrorIfSuper`.
    pub(crate) fn report_error_if_super(&mut self, expression: impl Into<NodeId>) {
        let expression = expression.into();
        if self.ast.is::<SuperExpression>(expression) {
            // This error is also reported by the body builder.
            let begin = self.begin_token(expression);
            let end = self.end_token(expression);
            self.handle_recoverable_error(cfe_codes::missing_assignable_selector(), begin, end);
        }
    }

    /// Dart `_beginMethod`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn begin_method_impl(
        &mut self,
        _declaration_kind: dartr_parser::declaration_kind::DeclarationKind,
        augment_token: Option<TokenId>,
        external_token: Option<TokenId>,
        static_token: Option<TokenId>,
        covariant_token: Option<TokenId>,
        var_final_or_const: Option<TokenId>,
        new_keyword: Option<TokenId>,
        get_or_set: Option<TokenId>,
        name: TokenId,
        _enclosing_declaration_name: Option<String>,
    ) {
        let mut modifiers = Modifiers::default();
        if let Some(augment_token) = augment_token {
            modifiers.augment_keyword = Some(augment_token);
        }
        if let Some(external_token) = external_token {
            modifiers.external_keyword = Some(external_token);
        }
        if let Some(static_token) = static_token {
            let is_class_with_name = match &self.class_like_builder {
                Some(builder) => match &builder.kind {
                    ClassLikeKind::Class(c) => self.lexeme(c.name) == self.lexeme(name),
                    _ => false,
                },
                None => false,
            };
            if !is_class_with_name || get_or_set.is_some() {
                modifiers.static_keyword = Some(static_token);
            }
        }
        if let Some(covariant_token) = covariant_token {
            modifiers.covariant_keyword = Some(covariant_token);
        }
        if let Some(var_final_or_const) = var_final_or_const {
            modifiers.final_const_or_var_keyword = Some(var_final_or_const);
        }
        if let Some(new_keyword) = new_keyword {
            modifiers.new_keyword = Some(new_keyword);
        }
        self.push(modifiers);
    }

    /// Dart `_buildConstructorDeclaration`.
    pub(crate) fn build_constructor_declaration(
        &mut self,
        begin_token: TokenId,
        end_token: TokenId,
    ) -> Id<ConstructorDeclaration> {
        let body_object = self.pop();
        let initializers = self
            .pop_nodes_opt::<ConstructorInitializer>()
            .unwrap_or_default();
        let mut separator = self.pop_token_opt();
        let parameters = self.pop_node::<FormalParameterList>();
        let type_parameters = self.pop_node_opt::<TypeParameterList>();
        let preliminary_name = self.pop();
        self.pop(); // return type
        let modifiers = self.pop_modifiers();
        let metadata = self.pop_metadata();
        let comment = self.find_comment(metadata.as_deref(), begin_token);

        let mut redirected_constructor = None;
        let body: Id<FunctionBody>;
        match body_object {
            Value::Node(n) if self.ast.is::<FunctionBody>(n) => {
                body = Id::from_raw(n);
            }
            Value::RedirectingFactoryBody(b) => {
                separator = Some(b.equal_token);
                redirected_constructor = Some(b.constructor_name);
                body = self
                    .ast
                    .add(EmptyFunctionBody {
                        semicolon: end_token,
                    })
                    .upcast();
            }
            other => self.internal_problem(&format!("Unhandled {other:?} in bodyObject.")),
        }

        let mut type_name_identifier: Option<Id<SimpleIdentifier>> = None;
        let mut period = None;
        let mut constructor_name_token = None;

        let new_keyword = modifiers.as_ref().and_then(|m| m.new_keyword);
        match preliminary_name {
            Value::Null(_) => {}
            Value::Node(n) if self.ast.is::<SimpleIdentifier>(n) => {
                let identifier = Id::<SimpleIdentifier>::from_raw(n);
                if new_keyword.is_some() {
                    constructor_name_token = Some(self.ast[identifier].token);
                } else {
                    type_name_identifier = Some(identifier);
                }
            }
            Value::Node(n) if self.ast.is::<PrefixedIdentifier>(n) => {
                let p = self.ast[Id::<PrefixedIdentifier>::from_raw(n)].clone();
                type_name_identifier = Some(p.prefix);
                period = Some(p.period);
                constructor_name_token = Some(self.ast[p.identifier].token);
            }
            Value::OperatorName(o) => {
                type_name_identifier = Some(o.name);
            }
            other => panic!(
                "UnimplementedError: name is an instance of {other:?} in endClassConstructor"
            ),
        }

        if let Some(type_parameters) = type_parameters {
            // Outline builder also reports this error message.
            let begin = self.begin_token(type_parameters);
            let end = self.end_token(type_parameters);
            self.handle_recoverable_error(
                cfe_codes::constructor_with_type_parameters(),
                begin,
                end,
            );
        }

        if modifiers
            .as_ref()
            .and_then(|m| m.external_keyword)
            .is_some()
        {
            let list = self.ast[parameters].parameters;
            for formal_parameter in self.ast.list(list).to_vec() {
                if let Some(field) = self.ast.cast::<FieldFormalParameter>(formal_parameter) {
                    let this_keyword = self.ast[field].this_keyword;
                    self.report_at_token(
                        diag::external_constructor_with_field_initializers(),
                        this_keyword,
                    );
                }
            }
        }

        let metadata = self.node_list(metadata);
        let initializers = self.ast.new_list(initializers);
        let m = modifiers.as_deref();
        self.ast.add(ConstructorDeclaration {
            documentation_comment: comment,
            metadata,
            augment_keyword: m.and_then(|m| m.augment_keyword),
            external_keyword: m.and_then(|m| m.external_keyword),
            const_keyword: m.and_then(|m| m.final_const_or_var_keyword),
            factory_keyword: None,
            new_keyword: m.and_then(|m| m.new_keyword),
            type_name: type_name_identifier,
            period,
            name: constructor_name_token,
            parameters,
            separator,
            initializers,
            redirected_constructor,
            body,
        })
    }

    /// Dart `_buildFactoryConstructorDeclaration`.
    pub(crate) fn build_factory_constructor_declaration(
        &mut self,
        begin_token: TokenId,
        factory_keyword: TokenId,
        end_token: TokenId,
    ) -> Id<ConstructorDeclaration> {
        let body: Id<FunctionBody>;
        let mut separator = None;
        let mut redirected_constructor = None;
        let body_object = self.pop();
        match body_object {
            Value::Node(n) if self.ast.is::<FunctionBody>(n) => {
                body = Id::from_raw(n);
            }
            Value::RedirectingFactoryBody(b) => {
                separator = Some(b.equal_token);
                redirected_constructor = Some(b.constructor_name);
                body = self
                    .ast
                    .add(EmptyFunctionBody {
                        semicolon: end_token,
                    })
                    .upcast();
            }
            other => self.internal_problem(&format!("Unhandled {other:?} in bodyObject.")),
        }

        let parameters = self.pop_node::<FormalParameterList>();
        let type_parameters = self.pop_node_opt::<TypeParameterList>();
        let preliminary_name = self.pop_node_opt::<Identifier>();
        let modifiers = self.pop_modifiers();
        let metadata = self.pop_metadata();
        let comment = self.find_comment(metadata.as_deref(), begin_token);

        if let Some(type_parameters) = type_parameters {
            let begin = self.begin_token(type_parameters);
            let end = self.end_token(type_parameters);
            self.handle_recoverable_error(
                cfe_codes::constructor_with_type_parameters(),
                begin,
                end,
            );
        }

        let mut type_name_identifier: Option<Id<SimpleIdentifier>> = None;
        let mut period = None;
        let mut constructor_name_token = None;
        if let Some(preliminary_name) = preliminary_name {
            if let Some(identifier) = self.ast.cast::<SimpleIdentifier>(preliminary_name) {
                if self.is_enabled(ExperimentalFlag::PrimaryConstructors) {
                    // Consider a factory constructor declaration of the form
                    // `factory C(...` optionally starting with zero or more
                    // of the modifiers `const`, `augment`, or `external`.
                    // Assume that `C` is the name of the enclosing class,
                    // mixin class, enum, or extension type. In this
                    // situation, the declaration declares a constructor
                    // whose name is `C`.
                    let enclosing_class_name =
                        self.class_like_builder.as_ref().and_then(|b| b.name());
                    let token = self.ast[identifier].token;
                    if enclosing_class_name.map(|n| self.lexeme(n)) == Some(self.lexeme(token)) {
                        type_name_identifier = Some(identifier);
                    } else {
                        constructor_name_token = Some(token);
                    }
                } else {
                    type_name_identifier = Some(identifier);
                }
            } else if let Some(prefixed) = self.ast.cast::<PrefixedIdentifier>(preliminary_name) {
                let p = self.ast[prefixed].clone();
                type_name_identifier = Some(p.prefix);
                period = Some(p.period);
                constructor_name_token = Some(self.ast[p.identifier].token);
            }
        }

        let metadata = self.node_list(metadata);
        let m = modifiers.as_deref();
        self.ast.add(ConstructorDeclaration {
            documentation_comment: comment,
            metadata,
            augment_keyword: m.and_then(|m| m.augment_keyword),
            external_keyword: m.and_then(|m| m.external_keyword),
            const_keyword: m.and_then(|m| m.final_const_or_var_keyword),
            factory_keyword: Some(factory_keyword),
            new_keyword: None,
            type_name: type_name_identifier,
            period,
            name: constructor_name_token,
            parameters,
            separator,
            initializers: NodeList::EMPTY,
            redirected_constructor,
            body,
        })
    }

    /// Dart `_endClassConstructor`.
    pub(crate) fn end_class_constructor(
        &mut self,
        begin_token: TokenId,
        _begin_param: TokenId,
        _begin_initializers: Option<TokenId>,
        end_token: TokenId,
    ) {
        let constructor = self.build_constructor_declaration(begin_token, end_token);
        if let Some(builder) = self.class_like_builder.as_mut() {
            builder.members.push(constructor.upcast());
        }
    }

    /// Dart `_endClassFields`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn end_class_fields(
        &mut self,
        abstract_token: Option<TokenId>,
        augment_token: Option<TokenId>,
        external_token: Option<TokenId>,
        static_token: Option<TokenId>,
        covariant_token: Option<TokenId>,
        late_token: Option<TokenId>,
        var_final_or_const: Option<TokenId>,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        if let Some(abstract_token) = abstract_token {
            if static_token.is_some() && !self.enable_augmentations {
                self.handle_recoverable_error(
                    cfe_codes::abstract_static_field(),
                    abstract_token,
                    abstract_token,
                );
            }
            if late_token.is_some() {
                self.handle_recoverable_error(
                    cfe_codes::abstract_late_field(),
                    abstract_token,
                    abstract_token,
                );
            }
        }
        if let Some(external_token) = external_token {
            if late_token.is_some() {
                self.handle_recoverable_error(
                    cfe_codes::external_late_field(),
                    external_token,
                    external_token,
                );
            }
        }

        let variables = self.pop_typed_list2::<VariableDeclaration>(count as usize);
        let type_ = self.pop_node_opt::<TypeAnnotation>();
        let variables = self.ast.new_list(variables);
        let variable_list = self.ast.add(VariableDeclarationList {
            documentation_comment: None,
            metadata: NodeList::EMPTY,
            late_keyword: late_token,
            keyword: var_final_or_const,
            type_,
            variables,
        });
        let metadata = self.pop_metadata();
        let comment = self.find_comment(metadata.as_deref(), begin_token);
        let metadata = self.node_list(metadata);
        let field = self.ast.add(FieldDeclaration {
            documentation_comment: comment,
            metadata,
            augment_keyword: augment_token,
            external_keyword: external_token,
            static_keyword: static_token,
            abstract_keyword: abstract_token,
            covariant_keyword: covariant_token,
            fields: variable_list,
            semicolon: end_token,
        });
        if let Some(builder) = self.class_like_builder.as_mut() {
            builder.members.push(field.upcast());
        }
    }

    /// Dart `_endClassMethod`.
    pub(crate) fn end_class_method(
        &mut self,
        get_or_set: Option<TokenId>,
        begin_token: TokenId,
        _begin_param: TokenId,
        _begin_initializers: Option<TokenId>,
        end_token: TokenId,
    ) {
        let body_object = self.pop();
        self.pop(); // initializers
        self.pop(); // separator
        let mut formal_parameters = self.pop_node_opt::<FormalParameterList>();
        let type_parameters = self.pop_node_opt::<TypeParameterList>();
        let name = self.pop();
        let return_type = self.pop_node_opt::<TypeAnnotation>();
        let modifiers = self.pop_modifiers();
        let metadata = self.pop_metadata();
        let comment = self.find_comment(metadata.as_deref(), begin_token);

        let mut operator_keyword = None;
        let name_id: Id<SimpleIdentifier>;
        match name {
            Value::Node(n) if self.ast.is::<SimpleIdentifier>(n) => {
                name_id = Id::from_raw(n);
            }
            Value::OperatorName(o) => {
                operator_keyword = Some(o.operator_keyword);
                name_id = o.name;
                if let Some(type_parameters) = type_parameters {
                    let begin = self.begin_token(type_parameters);
                    let end = self.end_token(type_parameters);
                    self.handle_recoverable_error(
                        cfe_codes::operator_with_type_parameters(),
                        begin,
                        end,
                    );
                }
            }
            other => {
                panic!("UnimplementedError: name is an instance of {other:?} in endClassMethod")
            }
        }

        if get_or_set.is_some_and(|t| self.ast.tokens.ty(t) == Keyword::SET) {
            formal_parameters = self.ensure_setter_formal_parameter(name_id, formal_parameters);
        }

        let body: Id<FunctionBody> = match body_object {
            Value::Node(n) if self.ast.is::<FunctionBody>(n) => Id::from_raw(n),
            Value::RedirectingFactoryBody(_) => self
                .ast
                .add(EmptyFunctionBody {
                    semicolon: end_token,
                })
                .upcast(),
            other => self.internal_problem(&format!("Unhandled {other:?} in bodyObject.")),
        };

        let metadata = self.node_list(metadata);
        let m = modifiers.as_deref();
        let name = self.ast[name_id].token;
        let method = self.ast.add(MethodDeclaration {
            documentation_comment: comment,
            metadata,
            augment_keyword: m.and_then(|m| m.augment_keyword),
            external_keyword: m.and_then(|m| m.external_keyword),
            modifier_keyword: m.and_then(|m| m.abstract_keyword.or(m.static_keyword)),
            return_type,
            property_keyword: get_or_set,
            operator_keyword,
            name,
            type_parameters,
            parameters: formal_parameters,
            body,
        });
        if let Some(builder) = self.class_like_builder.as_mut() {
            builder.members.push(method.upcast());
        }
    }

    /// Dart `_endFactoryMethod`.
    pub(crate) fn end_factory_method(
        &mut self,
        begin_token: TokenId,
        factory_keyword: TokenId,
        end_token: TokenId,
    ) {
        let constructor =
            self.build_factory_constructor_declaration(begin_token, factory_keyword, end_token);
        if let Some(builder) = self.class_like_builder.as_mut() {
            builder.members.push(constructor.upcast());
        }
    }

    /// Dart `_ensureSetterFormalParameter`.
    pub(crate) fn ensure_setter_formal_parameter(
        &mut self,
        setter_name: Id<SimpleIdentifier>,
        formal_parameters: Option<Id<FormalParameterList>>,
    ) -> Option<Id<FormalParameterList>> {
        let formal_parameters =
            formal_parameters.expect("Bad state: Parser has recovery, this never happens.");
        let list = self.ast[formal_parameters].clone();
        let setter_name_token = self.ast[setter_name].token;

        let value_formal_parameter = self.ast.list(list.parameters).first().copied();
        let Some(value_formal_parameter) = value_formal_parameter else {
            if !self.ast.tokens.get(list.left_parenthesis).is_synthetic() {
                self.report_at_token(
                    diag::wrong_number_of_parameters_for_setter(),
                    setter_name_token,
                );
            }
            let value_name_token = self
                .rewriter()
                .insert_synthetic_identifier(list.left_parenthesis, "");
            let parameter = self.ast.add(RegularFormalParameter {
                documentation_comment: None,
                metadata: NodeList::EMPTY,
                kind: ParameterKind::Required,
                required_keyword: None,
                covariant_keyword: None,
                const_final_or_var_keyword: None,
                type_: None,
                name: Some(value_name_token),
                function_typed_suffix: None,
                default_clause: None,
            });
            let parameters = self.ast.new_list([parameter.upcast::<FormalParameter>()]);
            return Some(self.ast.add(FormalParameterList {
                left_parenthesis: list.left_parenthesis,
                parameters,
                left_delimiter: None,
                right_delimiter: None,
                right_parenthesis: list.right_parenthesis,
            }));
        };

        if self.parameter_kind(value_formal_parameter) == ParameterKind::Required
            && list.parameters.len() == 1
        {
            return Some(formal_parameters);
        }

        self.report_at_token(
            diag::wrong_number_of_parameters_for_setter(),
            setter_name_token,
        );
        let parameters = self.ast.new_list([value_formal_parameter]);
        Some(self.ast.add(FormalParameterList {
            left_parenthesis: list.left_parenthesis,
            parameters,
            left_delimiter: None,
            right_delimiter: None,
            right_parenthesis: list.right_parenthesis,
        }))
    }

    /// Dart `FormalParameter.kind` (`isNamed`, `isOptional`, ...).
    pub(crate) fn parameter_kind(&self, parameter: Id<FormalParameter>) -> ParameterKind {
        let n = parameter.raw();
        if let Some(p) = self.ast.cast::<RegularFormalParameter>(n) {
            self.ast[p].kind
        } else if let Some(p) = self.ast.cast::<FieldFormalParameter>(n) {
            self.ast[p].kind
        } else if let Some(p) = self.ast.cast::<dartr_ast::SuperFormalParameter>(n) {
            self.ast[p].kind
        } else {
            unreachable!("FormalParameter {:?}", self.ast.kind(n))
        }
    }

    /// Dart `FormalParameter.name`.
    pub(crate) fn parameter_name(&self, parameter: Id<FormalParameter>) -> Option<TokenId> {
        let n = parameter.raw();
        if let Some(p) = self.ast.cast::<RegularFormalParameter>(n) {
            self.ast[p].name
        } else if let Some(p) = self.ast.cast::<FieldFormalParameter>(n) {
            Some(self.ast[p].name)
        } else if let Some(p) = self.ast.cast::<dartr_ast::SuperFormalParameter>(n) {
            Some(self.ast[p].name)
        } else {
            unreachable!("FormalParameter {:?}", self.ast.kind(n))
        }
    }

    /// Dart `_findComment`.
    pub(crate) fn find_comment(
        &mut self,
        metadata: Option<&[Id<Annotation>]>,
        token_after_metadata: TokenId,
    ) -> Option<Id<Comment>> {
        // Find the dartdoc tokens.
        let mut dartdoc = find_dart_doc(&self.ast.tokens, token_after_metadata);
        if dartdoc.is_none() {
            let metadata = metadata?;
            let mut index = metadata.len();
            loop {
                if index == 0 {
                    return None;
                }
                index -= 1;
                let begin = self.begin_token(metadata[index]);
                dartdoc = find_dart_doc(&self.ast.tokens, begin);
                if dartdoc.is_some() {
                    break;
                }
            }
        }
        Some(self.parse_doc_comment(dartdoc.unwrap()))
    }

    /// Dart `_handleInstanceCreation`.
    pub(crate) fn handle_instance_creation(&mut self, token: Option<TokenId>) {
        let argument_list = self.pop_node::<ArgumentList>();
        let constructor_name;
        let mut type_arguments = None;
        match self.pop() {
            Value::ConstructorNameWithInvalidTypeArgs(o) => {
                constructor_name = o.name;
                type_arguments = Some(o.invalid_type_args);
            }
            other => {
                constructor_name = self
                    .value_as_node_opt::<ConstructorName>(other)
                    .expect("ConstructorName");
            }
        }
        let node = self.ast.add(InstanceCreationExpression {
            keyword: token,
            constructor_name,
            type_arguments,
            argument_list,
        });
        self.push(node);
    }

    /// Dart `_popNamedTypeList`.
    pub(crate) fn pop_named_type_list(
        &mut self,
        locatable_diagnostic: fn() -> LocatableDiagnostic,
    ) -> Vec<Id<NamedType>> {
        let types = self.pop_nodes::<TypeAnnotation>();
        let mut named_types = Vec::with_capacity(types.len());
        for type_ in types {
            if let Some(named) = self.ast.cast::<NamedType>(type_) {
                named_types.push(named);
            } else {
                self.report_at_node(locatable_diagnostic(), type_);
            }
        }
        named_types
    }

    /// Dart `_reportFeatureNotEnabled`.
    pub(crate) fn report_feature_not_enabled(
        &mut self,
        feature: ExperimentalFlag,
        start_token: TokenId,
        end_token: Option<TokenId>,
    ) {
        let (major, minor) = analyzer_release_version(feature);
        self.handle_recoverable_error(
            cfe_codes::experiment_not_enabled(feature.name(), &version_as_string(major, minor)),
            start_token,
            end_token.unwrap_or(start_token),
        );
    }

    /// Dart `_syntheticArgumentList`.
    pub(crate) fn synthetic_argument_list(&mut self, preceding_token: TokenId) -> Id<ArgumentList> {
        let left = self.rewriter().insert_parens(preceding_token, false);
        let right = self.end_group(left);
        self.ast.add(ArgumentList {
            left_parenthesis: left,
            arguments: NodeList::<Argument>::EMPTY,
            right_parenthesis: right,
        })
    }

    /// Dart `_syntheticFormalParameterList`.
    pub(crate) fn synthetic_formal_parameter_list(
        &mut self,
        preceding_token: TokenId,
    ) -> Id<FormalParameterList> {
        let left = self.rewriter().insert_parens(preceding_token, false);
        let right = self.end_group(left);
        self.ast.add(FormalParameterList {
            left_parenthesis: left,
            parameters: NodeList::EMPTY,
            left_delimiter: None,
            right_delimiter: None,
            right_parenthesis: right,
        })
    }

    /// Dart `_toAnalyzerParameterKind`.
    pub(crate) fn to_analyzer_parameter_kind(&self, ty: FormalParameterKind) -> ParameterKind {
        match ty {
            FormalParameterKind::RequiredPositional => ParameterKind::Required,
            FormalParameterKind::RequiredNamed => ParameterKind::NamedRequired,
            FormalParameterKind::OptionalNamed => ParameterKind::Named,
            FormalParameterKind::OptionalPositional => ParameterKind::Positional,
        }
    }

    /// Builds the declaration of the class like builder and clears it
    /// (the `build()` methods of the Dart builder classes).
    pub(crate) fn take_class_like_builder(&mut self) -> Box<ClassLikeDeclarationBuilder> {
        self.class_like_builder
            .take()
            .expect("Null check operator used on a null value (_classLikeBuilder)")
    }
}

/// The analyzer `ExperimentalFeature.releaseVersion ??
/// ExperimentStatus.currentVersion` of [feature].
fn analyzer_release_version(feature: ExperimentalFlag) -> (u32, u32) {
    if feature.is_enabled_by_default() {
        feature.experiment_enabled_version()
    } else {
        (
            CURRENT_LANGUAGE_VERSION.0 as u32,
            CURRENT_LANGUAGE_VERSION.1 as u32,
        )
    }
}

/// Dart `_versionAsString`.
fn version_as_string(major: u32, minor: u32) -> String {
    format!("{major}.{minor}.0")
}

// -------------------------------------------------------------------------
// Dart lines 6387-6863: the helper classes.

/// Dart `_ClassLikeDeclarationBuilder` and its subclasses: the common
/// fields, and the subclass in [`ClassLikeDeclarationBuilder::kind`].
#[derive(Debug)]
pub struct ClassLikeDeclarationBuilder {
    pub comment: Option<Id<Comment>>,
    pub metadata: Option<Vec<Id<Annotation>>>,
    pub type_parameters: Option<Id<TypeParameterList>>,
    pub empty_class_body_semicolon: Option<TokenId>,
    pub left_bracket: TokenId,
    pub members: Vec<Id<ClassMember>>,
    pub right_bracket: TokenId,
    pub kind: ClassLikeKind,
}

/// The subclass of a [`ClassLikeDeclarationBuilder`].
#[derive(Debug)]
pub enum ClassLikeKind {
    Class(ClassDeclarationBuilder),
    Enum(EnumDeclarationBuilder),
    Extension(ExtensionDeclarationBuilder),
    ExtensionType(ExtensionTypeDeclarationBuilder),
    Mixin(MixinDeclarationBuilder),
}

/// Dart `_ClassDeclarationBuilder` (the fields of the subclass).
#[derive(Debug)]
pub struct ClassDeclarationBuilder {
    pub augment_keyword: Option<TokenId>,
    pub abstract_keyword: Option<TokenId>,
    pub sealed_keyword: Option<TokenId>,
    pub base_keyword: Option<TokenId>,
    pub interface_keyword: Option<TokenId>,
    pub final_keyword: Option<TokenId>,
    pub mixin_keyword: Option<TokenId>,
    pub class_keyword: TokenId,
    pub primary_constructor_builder: Option<Box<PrimaryConstructorBuilder>>,
    pub name: TokenId,
    pub extends_clause: Option<Id<ExtendsClause>>,
    pub with_clause: Option<Id<WithClause>>,
    pub implements_clause: Option<Id<ImplementsClause>>,
    pub native_clause: Option<Id<NativeClause>>,
}

/// Dart `_EnumDeclarationBuilder`.
#[derive(Debug)]
pub struct EnumDeclarationBuilder {
    pub augment_keyword: Option<TokenId>,
    pub enum_keyword: TokenId,
    pub primary_constructor_builder: Option<Box<PrimaryConstructorBuilder>>,
    pub name: TokenId,
    pub with_clause: Option<Id<WithClause>>,
    pub implements_clause: Option<Id<ImplementsClause>>,
    pub constants: Vec<Id<EnumConstantDeclaration>>,
    pub semicolon: Option<TokenId>,
}

/// Dart `_ExtensionDeclarationBuilder`.
#[derive(Debug)]
pub struct ExtensionDeclarationBuilder {
    pub augment_keyword: Option<TokenId>,
    pub extension_keyword: TokenId,
    pub name: Option<TokenId>,
}

/// Dart `_ExtensionTypeDeclarationBuilder`.
#[derive(Debug)]
pub struct ExtensionTypeDeclarationBuilder {
    pub augment_keyword: Option<TokenId>,
    pub extension_keyword: TokenId,
    pub name: TokenId,
}

/// Dart `_MixinDeclarationBuilder`.
#[derive(Debug)]
pub struct MixinDeclarationBuilder {
    pub augment_keyword: Option<TokenId>,
    pub base_keyword: Option<TokenId>,
    pub mixin_keyword: TokenId,
    pub name: TokenId,
    pub on_clause: Option<Id<MixinOnClause>>,
    pub implements_clause: Option<Id<ImplementsClause>>,
}

impl ClassLikeDeclarationBuilder {
    /// Dart `name`.
    pub fn name(&self) -> Option<TokenId> {
        match &self.kind {
            ClassLikeKind::Class(b) => Some(b.name),
            ClassLikeKind::Enum(b) => Some(b.name),
            ClassLikeKind::Extension(b) => b.name,
            ClassLikeKind::ExtensionType(b) => Some(b.name),
            ClassLikeKind::Mixin(b) => Some(b.name),
        }
    }

    /// Dart `buildClassNamePart`.
    fn build_class_name_part(
        &self,
        ast: &mut Ast,
        type_name: TokenId,
        primary_constructor_builder: Option<&PrimaryConstructorBuilder>,
    ) -> Id<ClassNamePart> {
        if let Some(builder) = primary_constructor_builder {
            builder.build(ast, type_name, self.type_parameters).upcast()
        } else {
            ast.add(NameWithTypeParameters {
                type_name,
                type_parameters: self.type_parameters,
            })
            .upcast()
        }
    }

    /// The class body: `EmptyClassBody` or `BlockClassBody`.
    fn build_class_body(&self, ast: &mut Ast) -> Id<ClassBody> {
        if let Some(semicolon) = self.empty_class_body_semicolon {
            ast.add(EmptyClassBody { semicolon }).upcast()
        } else {
            let members = ast.new_list(self.members.iter().copied());
            ast.add(BlockClassBody {
                left_bracket: self.left_bracket,
                members,
                right_bracket: self.right_bracket,
            })
            .upcast()
        }
    }

    fn metadata_list(&self, ast: &mut Ast) -> NodeList<Annotation> {
        match &self.metadata {
            Some(m) => ast.new_list(m.iter().copied()),
            None => NodeList::EMPTY,
        }
    }

    /// Dart `_ClassDeclarationBuilder.build`.
    pub fn build_class(&self, ast: &mut Ast) -> Id<ClassDeclaration> {
        let ClassLikeKind::Class(c) = &self.kind else {
            panic!("not a class builder");
        };
        let body = self.build_class_body(ast);
        let name_part =
            self.build_class_name_part(ast, c.name, c.primary_constructor_builder.as_deref());
        let metadata = self.metadata_list(ast);
        ast.add(ClassDeclaration {
            documentation_comment: self.comment,
            metadata,
            augment_keyword: c.augment_keyword,
            abstract_keyword: c.abstract_keyword,
            sealed_keyword: c.sealed_keyword,
            base_keyword: c.base_keyword,
            interface_keyword: c.interface_keyword,
            final_keyword: c.final_keyword,
            mixin_keyword: c.mixin_keyword,
            class_keyword: c.class_keyword,
            name_part,
            extends_clause: c.extends_clause,
            with_clause: c.with_clause,
            implements_clause: c.implements_clause,
            native_clause: c.native_clause,
            body,
        })
    }

    /// Dart `_EnumDeclarationBuilder.build`.
    pub fn build_enum(&self, ast: &mut Ast) -> Id<EnumDeclaration> {
        let ClassLikeKind::Enum(e) = &self.kind else {
            panic!("not an enum builder");
        };
        let body: Id<EnumBody> = if let Some(semicolon) = self.empty_class_body_semicolon {
            ast.add(EmptyEnumBody { semicolon }).upcast()
        } else {
            let constants = ast.new_list(e.constants.iter().copied());
            let members = ast.new_list(self.members.iter().copied());
            ast.add(BlockEnumBody {
                left_bracket: self.left_bracket,
                constants,
                semicolon: e.semicolon,
                members,
                right_bracket: self.right_bracket,
            })
            .upcast()
        };
        let name_part =
            self.build_class_name_part(ast, e.name, e.primary_constructor_builder.as_deref());
        let metadata = self.metadata_list(ast);
        ast.add(EnumDeclaration {
            documentation_comment: self.comment,
            metadata,
            augment_keyword: e.augment_keyword,
            enum_keyword: e.enum_keyword,
            name_part,
            with_clause: e.with_clause,
            implements_clause: e.implements_clause,
            body,
        })
    }

    /// Dart `_ExtensionDeclarationBuilder.build`.
    pub fn build_extension(
        &self,
        ast: &mut Ast,
        type_keyword: Option<TokenId>,
        on_clause: Option<Id<ExtensionOnClause>>,
    ) -> Id<ExtensionDeclaration> {
        let ClassLikeKind::Extension(e) = &self.kind else {
            panic!("not an extension builder");
        };
        let body = self.build_class_body(ast);
        let metadata = self.metadata_list(ast);
        ast.add(ExtensionDeclaration {
            documentation_comment: self.comment,
            metadata,
            augment_keyword: e.augment_keyword,
            extension_keyword: e.extension_keyword,
            type_keyword,
            name: e.name,
            type_parameters: self.type_parameters,
            on_clause,
            body,
        })
    }

    /// Dart `_ExtensionTypeDeclarationBuilder.build`.
    pub fn build_extension_type(
        &self,
        ast: &mut Ast,
        type_keyword: TokenId,
        _const_keyword: Option<TokenId>,
        primary_constructor_builder: Option<&PrimaryConstructorBuilder>,
        implements_clause: Option<Id<ImplementsClause>>,
    ) -> Id<ExtensionTypeDeclaration> {
        let ClassLikeKind::ExtensionType(e) = &self.kind else {
            panic!("not an extension type builder");
        };
        let body = self.build_class_body(ast);
        let name_part = self.build_class_name_part(ast, e.name, primary_constructor_builder);
        let metadata = self.metadata_list(ast);
        ast.add(ExtensionTypeDeclaration {
            documentation_comment: self.comment,
            metadata,
            augment_keyword: e.augment_keyword,
            extension_keyword: e.extension_keyword,
            type_keyword,
            name_part,
            implements_clause,
            body,
        })
    }

    /// Dart `_MixinDeclarationBuilder.build`.
    pub fn build_mixin(&self, ast: &mut Ast) -> Id<MixinDeclaration> {
        let ClassLikeKind::Mixin(m) = &self.kind else {
            panic!("not a mixin builder");
        };
        let body = self.build_class_body(ast);
        let metadata = self.metadata_list(ast);
        ast.add(MixinDeclaration {
            documentation_comment: self.comment,
            metadata,
            augment_keyword: m.augment_keyword,
            base_keyword: m.base_keyword,
            mixin_keyword: m.mixin_keyword,
            name: m.name,
            type_parameters: self.type_parameters,
            on_clause: m.on_clause,
            implements_clause: m.implements_clause,
            body,
        })
    }
}

/// Dart `_ConstructorNameWithInvalidTypeArgs`.
#[derive(Clone, Copy, Debug)]
pub struct ConstructorNameWithInvalidTypeArgs {
    pub name: Id<ConstructorName>,
    pub invalid_type_args: Id<TypeArgumentList>,
}

/// Data structure placed on the stack to carry function-typed parameter
/// parts until the concrete formal parameter node is built (Dart
/// `_FunctionTypedFormalParameterData`).
#[derive(Clone, Copy, Debug)]
pub struct FunctionTypedFormalParameterData {
    pub return_type: Option<Id<TypeAnnotation>>,
    pub function_typed_suffix: Id<dartr_ast::FunctionTypedFormalParameterSuffix>,
}

/// Data structure placed on the stack to represent a non-empty sequence of
/// modifiers (Dart `_Modifiers`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Modifiers {
    pub abstract_keyword: Option<TokenId>,
    pub augment_keyword: Option<TokenId>,
    pub external_keyword: Option<TokenId>,
    pub final_const_or_var_keyword: Option<TokenId>,
    pub new_keyword: Option<TokenId>,
    pub static_keyword: Option<TokenId>,
    pub covariant_keyword: Option<TokenId>,
    pub required_token: Option<TokenId>,
    pub late_token: Option<TokenId>,
}

impl Modifiers {
    /// Return the token that is lexically first (Dart `beginToken`).
    pub fn begin_token(&self, tokens: &Tokens) -> Option<TokenId> {
        let mut first_token: Option<TokenId> = None;
        for token in [
            self.abstract_keyword,
            self.external_keyword,
            self.final_const_or_var_keyword,
            self.new_keyword,
            self.static_keyword,
            self.covariant_keyword,
            self.required_token,
            self.late_token,
        ] {
            match first_token {
                None => first_token = token,
                Some(first) => {
                    if let Some(token) = token {
                        if tokens.offset(token) < tokens.offset(first) {
                            first_token = Some(token);
                        }
                    }
                }
            }
        }
        first_token
    }

    /// Return the `const` keyword or `None` (Dart `constKeyword`).
    pub fn const_keyword(&self, tokens: &Tokens) -> Option<TokenId> {
        self.final_const_or_var_keyword
            .filter(|&t| tokens.lexeme(t) == "const")
    }
}

/// Temporary representation of the fields of an extractor used internally
/// by the [`AstBuilder`] (Dart `_ObjectPatternFields`).
#[derive(Clone, Debug)]
pub struct ObjectPatternFields {
    pub left_parenthesis: TokenId,
    pub right_parenthesis: TokenId,
    pub fields: Vec<Id<PatternField>>,
}

/// Data structure placed on the stack to represent the keyword "operator"
/// followed by a token (Dart `_OperatorName`).
#[derive(Clone, Copy, Debug)]
pub struct OperatorName {
    pub operator_keyword: TokenId,
    pub name: Id<SimpleIdentifier>,
}

/// Data structure placed on the stack as a container for optional
/// parameters (Dart `_OptionalFormalParameters`).
#[derive(Clone, Debug)]
pub struct OptionalFormalParameters {
    pub parameters: Option<Vec<Id<FormalParameter>>>,
    pub left_delimiter: TokenId,
    pub right_delimiter: TokenId,
}

/// Data structure placed on the stack to represent the parenthesized
/// condition part of an if-statement, if-control-flow, switch-statement,
/// while-statement, or do-while-statement (Dart `_ParenthesizedCondition`).
#[derive(Clone, Copy, Debug)]
pub struct ParenthesizedCondition {
    pub left_parenthesis: TokenId,
    pub expression: Id<Expression>,
    pub case_clause: Option<Id<dartr_ast::CaseClause>>,
}

impl ParenthesizedCondition {
    /// Dart `rightParenthesis`: `leftParenthesis.endGroup!`.
    pub fn right_parenthesis(&self, tokens: &Tokens) -> TokenId {
        let end = tokens.get(self.left_parenthesis).end_group;
        assert!(
            end.is_some(),
            "Null check operator used on a null value (endGroup)"
        );
        end
    }
}

/// Dart `_PrimaryConstructorBuilder`.
#[derive(Clone, Copy, Debug)]
pub struct PrimaryConstructorBuilder {
    pub const_keyword: Option<TokenId>,
    pub constructor_name: Option<Id<PrimaryConstructorName>>,
    pub formal_parameter_list: Id<FormalParameterList>,
}

impl PrimaryConstructorBuilder {
    /// Dart `build`.
    pub fn build(
        &self,
        ast: &mut Ast,
        type_name: TokenId,
        type_parameters: Option<Id<TypeParameterList>>,
    ) -> Id<PrimaryConstructorDeclaration> {
        ast.add(PrimaryConstructorDeclaration {
            const_keyword: self.const_keyword,
            type_name,
            type_parameters,
            constructor_name: self.constructor_name,
            formal_parameters: self.formal_parameter_list,
        })
    }
}

/// Data structure placed on stack to represent the redirected constructor
/// (Dart `_RedirectingFactoryBody`).
#[derive(Clone, Copy, Debug)]
pub struct RedirectingFactoryBody {
    pub async_keyword: Option<TokenId>,
    pub star_keyword: Option<TokenId>,
    pub equal_token: TokenId,
    pub constructor_name: Id<ConstructorName>,
}
