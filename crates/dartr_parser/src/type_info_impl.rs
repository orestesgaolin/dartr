// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/type_info_impl.dart

//! The implementations of [`TypeInfo`] and [`TypeParamOrArgInfo`] (Dart
//! classes `NoType`, `PrefixedType`, `SimpleType`, ..., `ComplexTypeInfo`,
//! `ComplexTypeParamOrArgInfo`), and the token helpers of the type parser.
//!
//! # Token access
//!
//! The Dart compute and skip methods split `>>`, `>=`, ... tokens into new
//! tokens that are not inserted into the token stream (for example
//! `splitCloser` in `ComplexTypeParamOrArgInfo.compute`). The port writes
//! that code once, generic over [`TokenView`]:
//!
//! - [`Tokens`] is the exact view: split tokens are added to the arena (as
//!   Dart allocates them). The parse methods and the `*_mut` functions use
//!   it.
//! - [`Overlay`] is the view of the functions that take `&Tokens`: split
//!   tokens are kept in a local table. Before a result leaves such a
//!   function, each overlay token in it is replaced by the arena token that
//!   it was made from (see [`Overlay::real`]).

use std::cell::Cell;
use std::rc::Rc;

use dartr_diagnostics::cfe_codes as diag;
use dartr_syntax::token_constants::IDENTIFIER_TOKEN;
use dartr_syntax::{Keyword, Token, TokenId, TokenType, Tokens};

use crate::identifier_context::IdentifierContext;
use crate::listener::Listener;
use crate::member_kind::MemberKind;
use crate::parser_impl::Parser;
use crate::type_info::{
    NO_TYPE, NO_TYPE_PARAM_OR_ARG, TypeInfo, TypeParamOrArgInfo, VOID_TYPE, compute_type_g,
    compute_type_param_or_arg_g, is_possible_record_type_g, is_valid_non_record_type_reference,
    is_valid_non_record_type_reference_g,
};
use crate::util::{
    split_gt_eq, split_gt_from_gt_gt_eq, split_gt_from_gt_gt_gt, split_gt_from_gt_gt_gt_eq,
    split_gt_gt, synthetic_gt,
};

// ---------------------------------------------------------------------------
// Token access (not in Dart, see the module documentation).

/// Read access to tokens (the arena, or the arena plus overlay tokens).
pub(crate) trait TokenRead {
    fn tok(&self, id: TokenId) -> &Token;

    /// Dart `token.next!`.
    #[inline]
    fn next(&self, id: TokenId) -> TokenId {
        self.tok(id).next
    }
    /// Dart `token.next` (nullable).
    #[inline]
    fn next_opt(&self, id: TokenId) -> Option<TokenId> {
        self.tok(id).next.get()
    }
    #[inline]
    fn ty(&self, id: TokenId) -> TokenType {
        self.tok(id).ty
    }
    /// Dart `token.isA(type)`.
    #[inline]
    fn is_a(&self, id: TokenId, ty: TokenType) -> bool {
        self.tok(id).ty == ty
    }
    #[inline]
    fn kind(&self, id: TokenId) -> i32 {
        self.tok(id).kind()
    }
    #[inline]
    fn is_identifier(&self, id: TokenId) -> bool {
        self.tok(id).is_identifier()
    }
    #[inline]
    fn is_keyword_or_identifier(&self, id: TokenId) -> bool {
        self.tok(id).is_keyword_or_identifier()
    }
    #[inline]
    fn is_keyword(&self, id: TokenId) -> bool {
        self.tok(id).ty.is_keyword()
    }
    #[inline]
    fn is_synthetic(&self, id: TokenId) -> bool {
        self.tok(id).is_synthetic()
    }
    #[inline]
    fn is_eof(&self, id: TokenId) -> bool {
        self.tok(id).is_eof()
    }
    /// Dart `token.endGroup`.
    #[inline]
    fn end_group(&self, id: TokenId) -> Option<TokenId> {
        self.tok(id).end_group.get()
    }
    /// Dart `token.stringValue`.
    #[inline]
    fn string_value(&self, id: TokenId) -> Option<&'static str> {
        self.tok(id).ty.string_value()
    }
    #[inline]
    fn is_user_definable_operator(&self, id: TokenId) -> bool {
        self.tok(id).ty.is_user_definable_operator()
    }
}

impl TokenRead for Tokens {
    #[inline]
    fn tok(&self, id: TokenId) -> &Token {
        self.get(id)
    }
}

/// The split functions of `util.dart` that the type parser uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum Split {
    /// Dart `splitGtEq`.
    GtEq,
    /// Dart `splitGtGt`.
    GtGt,
    /// Dart `splitGtFromGtGtGt`.
    GtFromGtGtGt,
    /// Dart `splitGtFromGtGtEq`.
    GtFromGtGtEq,
    /// Dart `splitGtFromGtGtGtEq`.
    GtFromGtGtGtEq,
}

impl Split {
    fn parts(self) -> &'static [TokenType] {
        match self {
            Split::GtEq => &[TokenType::GT, TokenType::EQ],
            Split::GtGt => &[TokenType::GT, TokenType::GT],
            Split::GtFromGtGtGt => &[TokenType::GT, TokenType::GT_GT],
            Split::GtFromGtGtEq => &[TokenType::GT, TokenType::GT_EQ],
            Split::GtFromGtGtGtEq => &[TokenType::GT, TokenType::GT_GT_EQ],
        }
    }
}

/// Token access that can make the tokens of a split (Dart `splitGtGt`,
/// `syntheticGt`, ...: new tokens that are not linked into the stream).
pub(crate) trait TokenView: TokenRead {
    fn split(&mut self, token: TokenId, split: Split) -> TokenId;
    /// Dart `syntheticGt(next)`.
    fn synthetic_gt(&mut self, next: TokenId) -> TokenId;
}

impl TokenView for Tokens {
    fn split(&mut self, token: TokenId, split: Split) -> TokenId {
        match split {
            Split::GtEq => split_gt_eq(self, token),
            Split::GtGt => split_gt_gt(self, token),
            Split::GtFromGtGtGt => split_gt_from_gt_gt_gt(self, token),
            Split::GtFromGtGtEq => split_gt_from_gt_gt_eq(self, token),
            Split::GtFromGtGtGtEq => split_gt_from_gt_gt_gt_eq(self, token),
        }
    }

    fn synthetic_gt(&mut self, next: TokenId) -> TokenId {
        synthetic_gt(self, next)
    }
}

/// A read-only token arena plus a table of split tokens (see the module
/// documentation). Overlay token ids start at the length of the arena.
pub(crate) struct Overlay<'a> {
    tokens: &'a Tokens,
    base: u32,
    extra: Vec<Token>,
    byte_offsets: Vec<u32>,
    /// For each overlay token: the arena token that replaces it in results.
    sources: Vec<TokenId>,
}

impl<'a> Overlay<'a> {
    pub(crate) fn new(tokens: &'a Tokens) -> Self {
        Overlay {
            tokens,
            base: tokens.len() as u32,
            extra: Vec::new(),
            byte_offsets: Vec::new(),
            sources: Vec::new(),
        }
    }

    #[inline]
    fn is_overlay(&self, id: TokenId) -> bool {
        id.is_some() && id.0 >= self.base
    }

    fn byte_offset(&self, id: TokenId) -> u32 {
        if self.is_overlay(id) {
            self.byte_offsets[(id.0 - self.base) as usize]
        } else {
            self.tokens.byte_offset(id)
        }
    }

    fn push(&mut self, token: Token, byte_offset: u32, source: TokenId) -> TokenId {
        let id = TokenId(self.base + self.extra.len() as u32);
        self.extra.push(token);
        self.byte_offsets.push(byte_offset);
        self.sources.push(source);
        id
    }

    #[inline]
    fn extra_mut(&mut self, id: TokenId) -> &mut Token {
        &mut self.extra[(id.0 - self.base) as usize]
    }

    /// The arena token that replaces [id] in a result: [id] itself if it is
    /// an arena token. A part of a split token is replaced by the split
    /// token (the `next` of the last part is the `next` of the split token).
    /// A synthetic `>` is replaced by the arena token before its `next`.
    pub(crate) fn real(&self, id: TokenId) -> TokenId {
        if self.is_overlay(id) {
            self.sources[(id.0 - self.base) as usize]
        } else {
            id
        }
    }

    fn real_opt(&self, id: Option<TokenId>) -> Option<TokenId> {
        id.map(|id| self.real(id))
    }

    /// [info] with all overlay tokens replaced (see [`Self::real`]).
    pub(crate) fn real_type_info(&self, info: TypeInfo) -> TypeInfo {
        match info {
            TypeInfo::Complex(c) => {
                let c = &c.0;
                ComplexTypeInfo {
                    start: Cell::new(self.real(c.start.get())),
                    type_arguments: self.real_type_param_or_arg(c.type_arguments),
                    before_question_mark: self.real_opt(c.before_question_mark),
                    end: Cell::new(self.real_opt(c.end.get())),
                    type_variable_starters: c
                        .type_variable_starters
                        .iter()
                        .map(|&t| self.real(t))
                        .collect(),
                    gft_has_return_type: c.gft_has_return_type,
                    is_record_type: c.is_record_type,
                    gft_return_type_has_record_type: c.gft_return_type_has_record_type,
                    recovered: Cell::new(c.recovered.get()),
                }
                .into_type_info()
            }
            info => info,
        }
    }

    /// [info] with all overlay tokens replaced (see [`Self::real`]).
    pub(crate) fn real_type_param_or_arg(&self, info: TypeParamOrArgInfo) -> TypeParamOrArgInfo {
        match info {
            TypeParamOrArgInfo::Complex(mut c) => {
                c.start = self.real(c.start);
                c.skip_end = self.real_opt(c.skip_end);
                TypeParamOrArgInfo::Complex(c)
            }
            info => info,
        }
    }
}

impl TokenRead for Overlay<'_> {
    #[inline]
    fn tok(&self, id: TokenId) -> &Token {
        if self.is_overlay(id) {
            &self.extra[(id.0 - self.base) as usize]
        } else {
            self.tokens.get(id)
        }
    }
}

impl TokenView for Overlay<'_> {
    /// The same tokens as `split` in `crate::util`.
    fn split(&mut self, token: TokenId, split: Split) -> TokenId {
        let parts = split.parts();
        let t = self.tok(token);
        let offset = t.offset;
        let comments = t.preceding_comments;
        let next = t.next;
        let byte = self.byte_offset(token);
        let source = self.real(token);
        let mut ids = Vec::with_capacity(parts.len());
        let mut delta = 0u32;
        for (i, &ty) in parts.iter().enumerate() {
            let mut part = Token::fixed(ty, offset + delta, byte + delta, false);
            if i == 0 {
                part.preceding_comments = comments;
            }
            ids.push(self.push(part, byte + delta, source));
            delta += ty.lexeme().len() as u32;
        }
        let last = *ids.last().unwrap();
        self.extra_mut(last).next = next;
        for i in (0..ids.len() - 1).rev() {
            let (a, b) = (ids[i], ids[i + 1]);
            self.extra_mut(a).next = b;
            self.extra_mut(b).previous = a;
        }
        ids[0]
    }

    fn synthetic_gt(&mut self, next: TokenId) -> TokenId {
        let offset = self.tok(next).offset;
        let byte = self.byte_offset(next);
        let mut gt = Token::fixed(TokenType::GT, offset, byte, true);
        gt.next = next;
        let real_next = self.real(next);
        let source = self.tokens.previous(real_next).get().unwrap_or(real_next);
        self.push(gt, byte, source)
    }
}

/// Dart `skipMetadata` (util.dart; the same code as
/// [`crate::util::skip_metadata`]), for any [`TokenRead`].
pub(crate) fn skip_metadata_g<R: TokenRead + ?Sized>(tokens: &R, mut token: TokenId) -> TokenId {
    token = tokens.next(token);
    debug_assert!(tokens.ty(token) == TokenType::AT);
    let mut next = tokens.next(token);
    // Corresponds to 'ensureIdentifier' in [parseMetadata].
    if tokens.is_identifier(next) {
        token = next;
        next = tokens.next(token);
        // Corresponds to 'parseQualifiedRestOpt' in [parseMetadata].
        if tokens.ty(next) == TokenType::PERIOD {
            token = next;
            next = tokens.next(token);
            if tokens.is_identifier(next) {
                token = next;
                next = tokens.next(token);
            }
        }
        // Corresponds to 'computeTypeParamOrArg' in [parseMetadata].
        if tokens.ty(next) == TokenType::LT && !tokens.is_synthetic(tokens.tok(next).end_group) {
            token = tokens.tok(next).end_group;
            next = tokens.next(token);
        }

        // The extra .identifier after arguments in [parseMetadata].
        if tokens.ty(next) == TokenType::PERIOD {
            token = next;
            next = tokens.next(token);
            if tokens.is_identifier(next) {
                token = next;
                next = tokens.next(token);
            }
        }

        // Corresponds to 'parseArgumentsOpt' in [parseMetadata].
        if tokens.ty(next) == TokenType::OPEN_PAREN
            && !tokens.is_synthetic(tokens.tok(next).end_group)
        {
            token = tokens.tok(next).end_group;
        }
    }
    token
}

// ---------------------------------------------------------------------------
// Constants.

/// [SimpleType] is a specialized [TypeInfo] returned by [computeType]
/// when there is a single identifier as the type reference.
pub const SIMPLE_TYPE: TypeInfo = TypeInfo::SimpleType;

/// [SimpleNullableType] is a specialized [TypeInfo] returned by [computeType]
/// when there is a single identifier followed by `?` as the type reference.
pub const SIMPLE_NULLABLE_TYPE: TypeInfo = TypeInfo::SimpleNullableType;

/// [PrefixedType] is a specialized [TypeInfo] returned by [computeType]
/// when the type reference is of the form: identifier `.` identifier.
pub const PREFIXED_TYPE: TypeInfo = TypeInfo::PrefixedType;

/// [SimpleTypeWith1Argument] is a specialized [TypeInfo] returned by
/// [computeType] when the type reference is of the form:
/// identifier `<` identifier `>`.
pub const SIMPLE_TYPE_WITH_1_ARGUMENT: TypeInfo =
    TypeInfo::SimpleTypeWith1Argument(SIMPLE_TYPE_ARGUMENT_1);

/// [SimpleTypeWith1Argument] is a specialized [TypeInfo] returned by
/// [computeType] when the type reference is of the form:
/// identifier `<` identifier `>=`.
pub const SIMPLE_TYPE_WITH_1_ARGUMENT_GT_EQ: TypeInfo =
    TypeInfo::SimpleTypeWith1Argument(SIMPLE_TYPE_ARGUMENT_1_GT_EQ);

/// [SimpleTypeWith1Argument] is a specialized [TypeInfo] returned by
/// [computeType] when the type reference is of the form:
/// identifier `<` identifier `>>`.
pub const SIMPLE_TYPE_WITH_1_ARGUMENT_GT_GT: TypeInfo =
    TypeInfo::SimpleTypeWith1Argument(SIMPLE_TYPE_ARGUMENT_1_GT_GT);

/// [SimpleNullableTypeWith1Argument] is a specialized [TypeInfo] returned by
/// [computeType] when the type reference is of the form:
/// identifier `<` identifier `>` `?`.
pub const SIMPLE_NULLABLE_TYPE_WITH_1_ARGUMENT: TypeInfo =
    TypeInfo::SimpleNullableTypeWith1Argument;

/// [SimpleTypeArgument1] is a specialized [TypeParamOrArgInfo] returned by
/// [computeTypeParamOrArg] when the type reference is of the form:
/// `<` identifier `>`.
pub const SIMPLE_TYPE_ARGUMENT_1: TypeParamOrArgInfo = TypeParamOrArgInfo::SimpleTypeArgument1;

/// [SimpleTypeArgument1] is a specialized [TypeParamOrArgInfo] returned by
/// [computeTypeParamOrArg] when the type reference is of the form:
/// `<` identifier `>=`.
pub const SIMPLE_TYPE_ARGUMENT_1_GT_EQ: TypeParamOrArgInfo =
    TypeParamOrArgInfo::SimpleTypeArgument1GtEq;

/// [SimpleTypeArgument1] is a specialized [TypeParamOrArgInfo] returned by
/// [computeTypeParamOrArg] when the type reference is of the form:
/// `<` identifier `>>`.
pub const SIMPLE_TYPE_ARGUMENT_1_GT_GT: TypeParamOrArgInfo =
    TypeParamOrArgInfo::SimpleTypeArgument1GtGt;

// ---------------------------------------------------------------------------
// NoType.

/// See documentation on the [noType] const.
/// Dart (line 95): `class NoType implements TypeInfo`
pub(crate) struct NoType;

impl NoType {
    /// Dart (line 117): `Token ensureTypeNotVoid(Token token, Parser parser)`
    pub(crate) fn ensure_type_not_void<L: Listener>(
        token: TokenId,
        parser: &mut Parser<L>,
    ) -> TokenId {
        let next = parser.next(token);
        parser.report_recoverable_error_with_token(next, diag::expected_type);
        parser.rewriter().insert_synthetic_identifier(token, "");
        SIMPLE_TYPE.parse_type(token, parser)
    }

    /// Dart (line 124): `Token ensureTypeOrVoid(Token token, Parser parser)`
    pub(crate) fn ensure_type_or_void<L: Listener>(
        token: TokenId,
        parser: &mut Parser<L>,
    ) -> TokenId {
        NoType::ensure_type_not_void(token, parser)
    }

    /// Dart (line 128): `Token parseTypeNotVoid(Token token, Parser parser)`
    pub(crate) fn parse_type_not_void<L: Listener>(
        token: TokenId,
        parser: &mut Parser<L>,
    ) -> TokenId {
        NoType::parse_type(token, parser)
    }

    /// Dart (line 132): `Token parseType(Token token, Parser parser)`
    pub(crate) fn parse_type<L: Listener>(token: TokenId, parser: &mut Parser<L>) -> TokenId {
        parser.listener.handle_no_type(token);
        token
    }

    /// Dart (line 138): `Token skipType(Token token)`
    pub(crate) fn skip_type(token: TokenId) -> TokenId {
        token
    }
}

// ---------------------------------------------------------------------------
// PrefixedType.

/// See documentation on the [prefixedType] const.
/// Dart (line 147): `class PrefixedType implements TypeInfo`
pub(crate) struct PrefixedType;

impl PrefixedType {
    /// Dart (line 181): `Token parseType(Token token, Parser parser)`
    ///
    /// (`ensureTypeNotVoid`, `ensureTypeOrVoid` and `parseTypeNotVoid`, Dart
    /// lines 169-178, call this.)
    pub(crate) fn parse_type<L: Listener>(mut token: TokenId, parser: &mut Parser<L>) -> TokenId {
        token = parser.next(token);
        let start = token;
        debug_assert!(parser.is_keyword_or_identifier(token));
        parser
            .listener
            .handle_identifier(token, IdentifierContext::PrefixedTypeReference);

        token = parser.next(token);
        let period = token;
        debug_assert!(parser.is_a(token, TokenType::PERIOD));

        token = parser.next(token);
        debug_assert!(parser.is_keyword_or_identifier(token));
        parser
            .listener
            .handle_identifier(token, IdentifierContext::TypeReferenceContinuation);
        parser.listener.handle_qualified(period);

        let next = parser.next(token);
        parser.listener.handle_no_type_arguments(next);
        parser
            .listener
            .handle_type(start, /* questionMark = */ None);
        token
    }

    /// Dart (line 204): `Token skipType(Token token)`
    pub(crate) fn skip_type<R: TokenRead + ?Sized>(tokens: &R, token: TokenId) -> TokenId {
        tokens.next(tokens.next(tokens.next(token)))
    }
}

// ---------------------------------------------------------------------------
// SimpleNullableTypeWith1Argument / SimpleTypeWith1Argument.

/// See documentation on the [simpleTypeWith1Argument] const.
/// Dart (line 252): `class SimpleTypeWith1Argument implements TypeInfo`
///
/// See documentation on the [simpleNullableTypeWith1Argument] const.
/// Dart (line 215): `class SimpleNullableTypeWith1Argument extends
/// SimpleTypeWith1Argument` (its `typeArg` is [simpleTypeArgument1]).
///
/// The functions take the Dart field `typeArg` as `type_arg`; `nullable`
/// selects the `SimpleNullableTypeWith1Argument` overrides.
pub(crate) struct SimpleTypeWith1Argument;

impl SimpleTypeWith1Argument {
    /// Dart (line 288): `Token parseType(Token token, Parser parser)`
    ///
    /// (`ensureTypeNotVoid`, `ensureTypeOrVoid` and `parseTypeNotVoid`, Dart
    /// lines 276-285, call this.)
    pub(crate) fn parse_type<L: Listener>(
        type_arg: TypeParamOrArgInfo,
        nullable: bool,
        mut token: TokenId,
        parser: &mut Parser<L>,
    ) -> TokenId {
        token = parser.next(token);
        let start = token;
        debug_assert!(parser.is_keyword_or_identifier(token));
        parser
            .listener
            .handle_identifier(token, IdentifierContext::TypeReference);
        token = type_arg.parse_arguments(token, parser);
        SimpleTypeWith1Argument::parse_type_rest(nullable, start, token, parser)
    }

    /// Dart (line 296): `Token parseTypeRest(Token start, Token token, Parser
    /// parser)` and the override in `SimpleNullableTypeWith1Argument` (Dart
    /// line 231).
    fn parse_type_rest<L: Listener>(
        nullable: bool,
        start: TokenId,
        mut token: TokenId,
        parser: &mut Parser<L>,
    ) -> TokenId {
        if nullable {
            token = parser.next(token);
            debug_assert!(parser.is_a(token, TokenType::QUESTION));
            parser.listener.handle_type(start, Some(token));
            return token;
        }
        parser
            .listener
            .handle_type(start, /* questionMark = */ None);
        token
    }

    /// Dart (line 302): `Token skipType(Token token)` and the override in
    /// `SimpleNullableTypeWith1Argument` (Dart line 239).
    pub(crate) fn skip_type<V: TokenView + ?Sized>(
        type_arg: TypeParamOrArgInfo,
        nullable: bool,
        tokens: &mut V,
        mut token: TokenId,
    ) -> TokenId {
        if nullable {
            token = SimpleTypeWith1Argument::skip_type(type_arg, false, tokens, token);
            token = tokens.next(token);
            debug_assert!(tokens.is_a(token, TokenType::QUESTION));
            return token;
        }
        token = tokens.next(token);
        debug_assert!(tokens.is_keyword_or_identifier(token));
        type_arg.skip_g(tokens, token)
    }
}

// ---------------------------------------------------------------------------
// SimpleNullableType / SimpleType.

/// See documentation on the [simpleType] const.
/// Dart (line 350): `class SimpleType implements TypeInfo`
///
/// See documentation on the [simpleNullableType] const.
/// Dart (line 315): `class SimpleNullableType extends SimpleType`
///
/// `nullable` selects the `SimpleNullableType` overrides.
pub(crate) struct SimpleType;

impl SimpleType {
    /// Dart (line 384): `Token parseType(Token token, Parser parser)`
    ///
    /// (`ensureTypeNotVoid`, `ensureTypeOrVoid` and `parseTypeNotVoid`, Dart
    /// lines 372-381, call this.)
    pub(crate) fn parse_type<L: Listener>(
        nullable: bool,
        mut token: TokenId,
        parser: &mut Parser<L>,
    ) -> TokenId {
        token = parser.next(token);
        debug_assert!(is_valid_non_record_type_reference(parser.tokens(), token));
        parser
            .listener
            .handle_identifier(token, IdentifierContext::TypeReference);
        token = NO_TYPE_PARAM_OR_ARG.parse_arguments(token, parser);
        SimpleType::parse_type_rest(nullable, token, parser)
    }

    /// Dart (line 392): `Token parseTypeRest(Token token, Parser parser)` and
    /// the override in `SimpleNullableType` (Dart line 331).
    fn parse_type_rest<L: Listener>(
        nullable: bool,
        token: TokenId,
        parser: &mut Parser<L>,
    ) -> TokenId {
        if nullable {
            let start = token;
            let token = parser.next(start);
            debug_assert!(parser.is_a(token, TokenType::QUESTION));
            parser.listener.handle_type(start, Some(token));
            return token;
        }
        parser
            .listener
            .handle_type(token, /* questionMark = */ None);
        token
    }

    /// Dart (line 398): `Token skipType(Token token)` and the override in
    /// `SimpleNullableType` (Dart line 339).
    pub(crate) fn skip_type<R: TokenRead + ?Sized>(
        nullable: bool,
        tokens: &R,
        token: TokenId,
    ) -> TokenId {
        if nullable {
            return tokens.next(tokens.next(token));
        }
        tokens.next(token)
    }
}

// ---------------------------------------------------------------------------
// VoidType.

/// See documentation on the [voidType] const.
/// Dart (line 409): `class VoidType implements TypeInfo`
pub(crate) struct VoidType;

impl VoidType {
    /// Dart (line 431): `Token ensureTypeNotVoid(Token token, Parser parser)`
    pub(crate) fn ensure_type_not_void<L: Listener>(
        token: TokenId,
        parser: &mut Parser<L>,
    ) -> TokenId {
        // Report an error, then parse `void` as if it were a type name.
        let next = parser.next(token);
        parser.report_recoverable_error(next, diag::invalid_void());
        SIMPLE_TYPE.parse_type_not_void(token, parser)
    }

    /// Dart (line 438): `Token ensureTypeOrVoid(Token token, Parser parser)`
    pub(crate) fn ensure_type_or_void<L: Listener>(
        token: TokenId,
        parser: &mut Parser<L>,
    ) -> TokenId {
        VoidType::parse_type(token, parser)
    }

    /// Dart (line 442): `Token parseTypeNotVoid(Token token, Parser parser)`
    pub(crate) fn parse_type_not_void<L: Listener>(
        token: TokenId,
        parser: &mut Parser<L>,
    ) -> TokenId {
        VoidType::ensure_type_not_void(token, parser)
    }

    /// Dart (line 446): `Token parseType(Token token, Parser parser)`
    pub(crate) fn parse_type<L: Listener>(mut token: TokenId, parser: &mut Parser<L>) -> TokenId {
        token = parser.next(token);
        let void_keyword = token;
        let mut has_type_arguments = false;

        // Recovery: Skip past, but issue problem, if followed by type arguments.
        if parser.is_a(parser.next(token), TokenType::LT) {
            let type_param = compute_type_param_or_arg_g(parser.tokens_mut(), token, false, false);
            if type_param != NO_TYPE_PARAM_OR_ARG {
                has_type_arguments = true;
                let next = parser.next(token);
                parser.report_recoverable_error(next, diag::void_with_type_arguments());
                token = type_param.parse_arguments(token, parser);
            }
        }
        if has_type_arguments {
            parser
                .listener
                .handle_void_keyword_with_type_arguments(void_keyword);
        } else {
            // Normal case.
            parser.listener.handle_void_keyword(void_keyword);
        }
        token
    }

    /// Dart (line 469): `Token skipType(Token token)`
    pub(crate) fn skip_type<V: TokenView + ?Sized>(tokens: &mut V, mut token: TokenId) -> TokenId {
        token = tokens.next(token);
        // Recovery: Skip past if followed by type arguments.
        if tokens.is_a(tokens.next(token), TokenType::LT) {
            let type_param = compute_type_param_or_arg_g(tokens, token, false, false);
            if type_param != NO_TYPE_PARAM_OR_ARG {
                token = type_param.skip_g(tokens, token);
            }
        }
        token
    }
}

// ---------------------------------------------------------------------------
// looksLike*.

/// Dart (line 487): `bool looksLikeName(Token token)`
pub fn looks_like_name(tokens: &Tokens, token: TokenId) -> bool {
    looks_like_name_g(tokens, token)
}

/// [`looks_like_name`] for any [`TokenRead`].
pub(crate) fn looks_like_name_g<R: TokenRead + ?Sized>(tokens: &R, token: TokenId) -> bool {
    tokens.kind(token) == IDENTIFIER_TOKEN
        || tokens.is_a(token, Keyword::THIS)
        || tokens.is_a(token, Keyword::SUPER)
        || (tokens.is_identifier(token)
            // Although `typedef` is a legal identifier,
            // type `typedef` identifier is not legal and in this situation
            // `typedef` is probably a separate declaration.
            && (!tokens.is_a(token, Keyword::TYPEDEF)
                || !tokens.is_identifier(tokens.next(token))))
}

/// Dart (line 498): `bool looksLikeNameOrEndOfBlock(Token token)`
pub fn looks_like_name_or_end_of_block(tokens: &Tokens, token: TokenId) -> bool {
    looks_like_name_or_end_of_block_g(tokens, token)
}

/// [`looks_like_name_or_end_of_block`] for any [`TokenRead`].
pub(crate) fn looks_like_name_or_end_of_block_g<R: TokenRead + ?Sized>(
    tokens: &R,
    token: TokenId,
) -> bool {
    // End-of-file isn't a name, but this is called in a situation where
    // if there had been a name it would have used the type-info it had
    // collected --- this being eof probably mean the user is currently
    // typing and will probably write a name in a moment.
    // The same logic applies to "}" which ends for instance a class. Again,
    // the user is likely typing and will soon write a name.
    looks_like_name_g(tokens, token)
        || tokens.is_eof(token)
        || tokens.is_a(token, TokenType::CLOSE_CURLY_BRACKET)
}

/// When missing a comma, determine if the given token looks like it should
/// be part of a collection of type parameters or arguments.
/// Dart (line 512): `bool looksLikeTypeParamOrArg(bool inDeclaration, Token token)`
pub fn looks_like_type_param_or_arg(tokens: &Tokens, in_declaration: bool, token: TokenId) -> bool {
    looks_like_type_param_or_arg_g(tokens, in_declaration, token)
}

/// [`looks_like_type_param_or_arg`] for any [`TokenRead`].
pub(crate) fn looks_like_type_param_or_arg_g<R: TokenRead + ?Sized>(
    tokens: &R,
    in_declaration: bool,
    token: TokenId,
) -> bool {
    if in_declaration && tokens.kind(token) == IDENTIFIER_TOKEN {
        let next = tokens.next(token);
        if tokens.kind(next) == IDENTIFIER_TOKEN
            || tokens.is_a(next, TokenType::COMMA)
            || is_closer_g(tokens, next)
        {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// ComplexTypeInfo.

/// A shared reference to a [`ComplexTypeInfo`] (a Dart object reference):
/// `==` is Dart `identical`.
#[derive(Clone, Debug)]
pub struct ComplexTypeInfoRef(pub Rc<ComplexTypeInfo>);

impl PartialEq for ComplexTypeInfoRef {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ComplexTypeInfoRef {}

/// Instances of [ComplexTypeInfo] are returned by [computeType] to represent
/// type references that cannot be represented by the constants above.
/// Dart (line 526): `class ComplexTypeInfo implements TypeInfo`
///
/// The fields that Dart changes after the compute methods (`start`, `end`,
/// `recovered`) are `Cell`s.
#[derive(Debug)]
pub struct ComplexTypeInfo {
    /// The first token in the type reference.
    pub start: Cell<TokenId>,

    /// Type arguments were seen during analysis.
    pub type_arguments: TypeParamOrArgInfo,

    /// The token before the trailing question mark or `null` if either
    /// 1) there is no trailing question mark, or
    /// 2) the trailing question mark is not part of the type reference.
    pub before_question_mark: Option<TokenId>,

    /// The last token in the type reference.
    pub end: Cell<Option<TokenId>>,

    /// The `Function` tokens before the start of type variables of function
    /// types as seen during analysis. Dart `Link<Token>`: the last added
    /// token is first.
    pub type_variable_starters: Vec<TokenId>,

    /// If the receiver represents a generalized function type then this
    /// indicates whether it has a return type, otherwise this is `null`.
    pub gft_has_return_type: Option<bool>,

    /// If the type is a record type.
    pub is_record_type: bool,

    /// If this is a generalized function type with a record type included in
    /// the return type. E.g. `(int, int) Function(bool) Function(int)`.
    pub gft_return_type_has_record_type: bool,

    pub recovered: Cell<bool>,
}

impl ComplexTypeInfo {
    /// Dart (line 559): `ComplexTypeInfo(Token beforeStart, this.typeArguments)`
    pub(crate) fn new<R: TokenRead + ?Sized>(
        tokens: &R,
        before_start: TokenId,
        type_arguments: TypeParamOrArgInfo,
    ) -> ComplexTypeInfo {
        ComplexTypeInfo {
            start: Cell::new(tokens.next(before_start)),
            type_arguments,
            before_question_mark: None,
            end: Cell::new(None),
            type_variable_starters: Vec::new(),
            gft_has_return_type: None,
            is_record_type: false,
            gft_return_type_has_record_type: false,
            recovered: Cell::new(type_arguments.recovered()),
        }
    }

    /// Dart `return this;` from a compute method.
    pub(crate) fn into_type_info(self) -> TypeInfo {
        TypeInfo::Complex(ComplexTypeInfoRef(Rc::new(self)))
    }

    /// Dart (line 575): `TypeInfo get asNonNullable`
    pub(crate) fn as_non_nullable(this: &ComplexTypeInfoRef) -> TypeInfo {
        let c = &this.0;
        match c.before_question_mark {
            None => TypeInfo::Complex(this.clone()),
            // Dart (line 563): `ComplexTypeInfo._nonNullable(...)`
            Some(before_question_mark) => ComplexTypeInfo {
                start: Cell::new(c.start.get()),
                type_arguments: c.type_arguments,
                before_question_mark: None,
                end: Cell::new(Some(before_question_mark)),
                type_variable_starters: c.type_variable_starters.clone(),
                gft_has_return_type: c.gft_has_return_type,
                is_record_type: c.is_record_type,
                gft_return_type_has_record_type: c.gft_return_type_has_record_type,
                recovered: Cell::new(c.recovered.get()),
            }
            .into_type_info(),
        }
    }

    /// Dart (line 591): `bool get couldBeExpression`
    pub(crate) fn could_be_expression(&self) -> bool {
        self.type_arguments == NO_TYPE_PARAM_OR_ARG && self.type_variable_starters.is_empty()
    }

    /// Dart (line 595): `bool get hasTypeArguments`
    pub(crate) fn has_type_arguments(&self) -> bool {
        !matches!(self.type_arguments, TypeParamOrArgInfo::NoTypeParamOrArg)
    }

    /// Dart (line 598): `bool get isNullable`
    pub(crate) fn is_nullable(&self) -> bool {
        self.before_question_mark.is_some()
    }

    /// Dart (line 601): `bool get isFunctionType`
    pub(crate) fn is_function_type(&self) -> bool {
        self.gft_has_return_type.is_some()
    }

    /// Dart (line 616): `Token parseType(Token token, Parser parser)`
    ///
    /// (`ensureTypeNotVoid`, `ensureTypeOrVoid` and `parseTypeNotVoid`, Dart
    /// lines 604-613, call this.)
    pub(crate) fn parse_type<L: Listener>(
        &self,
        mut token: TokenId,
        parser: &mut Parser<L>,
    ) -> TokenId {
        debug_assert!(parser.next(token) == self.start.get());

        if parser.is_a(self.start.get(), TokenType::PERIOD) {
            // Recovery: Insert missing identifier without sending events
            let start = parser.insert_synthetic_identifier(
                token,
                IdentifierContext::PrefixedTypeReference,
                None,
                None,
            );
            self.start.set(start);
        }

        let mut type_variable_end_groups: Vec<TokenId> = Vec::new();
        for &t in &self.type_variable_starters {
            parser.listener.begin_function_type(self.start.get());
            let type_param = compute_type_param_or_arg_g(
                parser.tokens_mut(),
                t,
                /* inDeclaration = */ true,
                false,
            );
            type_variable_end_groups.push(type_param.parse_variables(t, parser));
        }

        if self.gft_has_return_type == Some(false) {
            // A function type without return type.
            // Push the non-existing return type first. The loop below will
            // generate the full type.
            NO_TYPE.parse_type(token, parser);
        } else if self.is_record_type {
            token = parser.parse_record_type(
                self.start.get(),
                token,
                /* isQuestionMarkPartOfType = */ self.before_question_mark.is_some(),
            );
        } else if self.gft_return_type_has_record_type {
            token = parser.parse_record_type(
                self.start.get(),
                token,
                /* isQuestionMarkPartOfType = */ true,
            );
        } else {
            let type_ref_or_prefix = parser.next(token);
            if parser.is_a(type_ref_or_prefix, Keyword::VOID) {
                token = VOID_TYPE.parse_type(token, parser);
            } else {
                if !parser.is_a(type_ref_or_prefix, TokenType::PERIOD)
                    && !parser.is_a(parser.next(type_ref_or_prefix), TokenType::PERIOD)
                {
                    token = parser.ensure_identifier(token, IdentifierContext::TypeReference);
                } else {
                    token =
                        parser.ensure_identifier(token, IdentifierContext::PrefixedTypeReference);
                    token = parser
                        .parse_qualified_rest(token, IdentifierContext::TypeReferenceContinuation);
                    if parser.is_synthetic(token)
                        && self.end.get() == parser.tokens().next_opt(type_ref_or_prefix)
                    {
                        // Recovery: Update `end` if a synthetic identifier was inserted.
                        self.end.set(Some(token));
                    }
                }
                token = self.type_arguments.parse_arguments(token, parser);

                // Only consume the `?` if it is part of the complex type
                let mut question_mark = Some(parser.next(token));
                if parser.is_a(question_mark.unwrap(), TokenType::QUESTION)
                    && (!type_variable_end_groups.is_empty() || self.before_question_mark.is_some())
                {
                    token = question_mark.unwrap();
                } else {
                    question_mark = None;
                }

                parser
                    .listener
                    .handle_type(type_ref_or_prefix, question_mark);
            }
        }

        let mut end_group_index = type_variable_end_groups.len() as i32 - 1;
        for _ in &self.type_variable_starters {
            token = parser.next(token);
            debug_assert!(parser.is_a(token, Keyword::FUNCTION));
            let function_token = token;

            if parser.is_a(parser.next(token), TokenType::LT) {
                // Skip type parameters, they were parsed above.
                token = type_variable_end_groups[end_group_index as usize];
                debug_assert!(parser.is_a(token, TokenType::GT));
            }
            token = parser
                .parse_formal_parameters_required_opt(token, MemberKind::GeneralizedFunctionType);

            // Only consume the `?` if it is part of the complex type
            let mut question_mark = Some(parser.next(token));
            if parser.is_a(question_mark.unwrap(), TokenType::QUESTION)
                && (end_group_index > 0 || self.before_question_mark.is_some())
            {
                token = question_mark.unwrap();
            } else {
                question_mark = None;
            }

            end_group_index -= 1;
            parser
                .listener
                .end_function_type(function_token, question_mark);
        }

        // There are two situations in which the [token] != [end]:
        // Valid code:    identifier `<` identifier `<` identifier `>>`
        //    where `>>` is replaced by two tokens.
        // Invalid code:  identifier `<` identifier identifier `>`
        //    where a synthetic `>` is inserted between the identifiers.
        // assert(identical(token, end) || token.isA(TokenType.GT));

        // During recovery, [token] may be a synthetic that was inserted in the
        // middle of the type reference.
        self.end.set(Some(token));
        token
    }

    /// Dart (line 738): `Token skipType(Token token)`
    pub(crate) fn skip_type(&self, _token: TokenId) -> TokenId {
        self.end.get().unwrap()
    }

    /// Given `Function` non-identifier, compute the type
    /// and return the receiver or one of the [TypeInfo] constants.
    /// Dart (line 744): `TypeInfo computeNoTypeGFT(Token beforeStart, bool required)`
    pub(crate) fn compute_no_type_gft<V: TokenView + ?Sized>(
        mut self,
        tokens: &mut V,
        before_start: TokenId,
        required: bool,
    ) -> TypeInfo {
        debug_assert!(tokens.is_a(self.start.get(), Keyword::FUNCTION));
        debug_assert!(tokens.next(before_start) == self.start.get());

        self.compute_rest(tokens, before_start, required);
        if self.gft_has_return_type.is_none() {
            return if required { SIMPLE_TYPE } else { NO_TYPE };
        }
        debug_assert!(self.end.get().is_some());
        self.into_type_info()
    }

    /// Given (a possible) RecordType of the form
    ///
    ///    `(` unchecked content `)`
    ///
    /// compute the type and return the receiver or one of the [TypeInfo]
    /// constants.
    /// Dart (line 762): `TypeInfo computeRecordType(bool required)`
    pub(crate) fn compute_record_type<V: TokenView + ?Sized>(
        mut self,
        tokens: &mut V,
        required: bool,
    ) -> TypeInfo {
        debug_assert!(is_possible_record_type_g(tokens, self.start.get()));
        let mut token = self.start.get();
        let end_group = tokens.end_group(token).unwrap();

        // Verify stuff between parenthesis.
        self.check_if_record_type_parenthesis_are_recovered(tokens, token, end_group);

        token = end_group;
        if !required {
            let mut next = tokens.next(token);
            if tokens.is_a(next, TokenType::QUESTION) {
                next = tokens.next(next);
            }
            let mut get_or_set = false;
            if tokens.is_keyword(next)
                && (tokens.is_a(next, Keyword::GET) || tokens.is_a(next, Keyword::SET))
                && tokens.is_identifier(tokens.next(next))
            {
                get_or_set = true;
                next = tokens.next(next);
            }
            if tokens.is_identifier(next) {
                let after_identifier = tokens.next(next);
                // TODO(jensj): Are there any other instances where it's a valid
                // (optional) record type?
                // * `;` e.g. field definition like `(int, int) x;`-
                // * `=` e.g. field definition like `(int, int) x = (42, 42);`.
                // * `<` e.g. method definition like `(int, int) x<T>(T t) {}`.
                // * `(` e.g. method definition like `(int, int) x() {}`.
                // * `,` e.g. non-last parameter like `void x((int, int) y, int z) {}`.
                // * `)` e.g. last parameter like `void x((int, int) y) {}`.
                // * `in` e.g. `for ((int, int) x in list) {}`.
                // * `}` e.g. `x({(int, int) x}) {}`.
                // * `:` e.g. `x({(int, int) x: (42, 42)}) {}`.
                // * `]` e.g. `x([(int, int) x = (42, 42)]) {}`.
                if !(tokens.is_a(after_identifier, TokenType::SEMICOLON)
                    || tokens.is_a(after_identifier, TokenType::EQ)
                    || tokens.is_a(after_identifier, TokenType::LT)
                    || tokens.is_a(after_identifier, TokenType::OPEN_PAREN)
                    || tokens.is_a(after_identifier, TokenType::COMMA)
                    || tokens.is_a(after_identifier, TokenType::CLOSE_PAREN)
                    || tokens.is_a(after_identifier, Keyword::IN)
                    || tokens.is_a(after_identifier, TokenType::CLOSE_CURLY_BRACKET)
                    || tokens.is_a(after_identifier, TokenType::COLON)
                    || tokens.is_a(after_identifier, TokenType::CLOSE_SQUARE_BRACKET)
                    || tokens.is_a(after_identifier, TokenType::EOF))
                {
                    if get_or_set
                        && (tokens.is_a(after_identifier, TokenType::FUNCTION)
                            || tokens.is_a(after_identifier, TokenType::OPEN_CURLY_BRACKET)
                            || tokens.is_a(after_identifier, Keyword::ASYNC)
                            || tokens.is_a(after_identifier, Keyword::SYNC)
                            || tokens.is_a(after_identifier, TokenType::EOF))
                    {
                        // With a getter/setter in the mix we can accept more stuff, e.g.
                        // these would be "fine":
                        // * `=>`: e.g. `(int, int) get x => (42, 42);`.
                        // * `{`: e.g. `(int, int) get x { }`.
                        // * `async`: e.g. `(int, int) get x async {}`.
                        // * `sync`: e.g. `(int, int) get x sync* {}`.
                        // Not all of this is valid (e.g. a setter can't be async, sync has
                        // to be followed by *, return type of async has to be Future etc),
                        // but for disambiguation we'll assume it's enough, and we'd rather
                        // have an error saying "return time has to be Future" than "I don't
                        // know what these parenthesis mean".
                    } else if tokens.is_a(next, Keyword::OPERATOR)
                        && tokens.is_user_definable_operator(after_identifier)
                    {
                        // E.g.
                        // `(int, int) operator [](int foo) {}`
                    } else {
                        // This could for instance be `(int x, int y) async {`.
                        return NO_TYPE;
                    }
                }
            } else if (tokens.is_a(next, Keyword::THIS) || tokens.is_a(next, Keyword::SUPER))
                && tokens.is_a(tokens.next(next), TokenType::PERIOD)
            {
                // E.g.
                // * C(({int n, String s}) this.x);
                // * C((int, int) super.x);
            } else {
                // Is it e.g.
                // * List<(int, int)>
                // * Map<(int, int), (String, String)>?
                // * List<List<(int, int)>>
                // * List<List<List<(int, int)>>>
                // * typedef F2<T extends List<(int, int)>>= T Function();
                // * typedef F3<T extends List<List<(int, int)>>>= T Function();
                //
                // But don't confuse e.g. `(() => print("hello")) >> 42;` for that.
                if self.recovered.get()
                    || !(tokens.is_a(next, TokenType::COMMA)
                        || tokens.is_a(next, TokenType::GT)
                        || tokens.is_a(next, TokenType::GT_GT)
                        || tokens.is_a(next, TokenType::GT_GT_EQ)
                        || tokens.is_a(next, TokenType::GT_GT_GT)
                        || tokens.is_a(next, TokenType::GT_GT_GT_EQ)
                        || tokens.is_a(next, TokenType::EOF))
                {
                    return NO_TYPE;
                }
            }
        }
        debug_assert!(tokens.is_a(token, TokenType::CLOSE_PAREN));

        self.before_question_mark = None;
        self.end.set(Some(token));
        token = tokens.next(token);

        if tokens.is_a(token, TokenType::QUESTION) {
            self.before_question_mark = self.end.get();
            self.end.set(Some(token));
            // Dart: `token = token.next!;` (the value is not used).
        }

        self.is_record_type = true;

        debug_assert!(self.end.get().is_some());
        self.into_type_info()
    }

    /// Check if the presumed record type has correct syntax between its
    /// parenthesis. If not [recovered] will be set to true.
    /// Keep in sync with [Parser.parseRecordType] et al.
    /// Dart (line 882): `void _checkIfRecordTypeParenthesisAreRecovered(Token token, Token endGroup)`
    fn check_if_record_type_parenthesis_are_recovered<V: TokenView + ?Sized>(
        &mut self,
        tokens: &mut V,
        mut token: TokenId,
        end_group: TokenId,
    ) {
        let mut parameter_count = 0;
        let mut has_named_fields = false;
        let mut has_comma = false;
        loop {
            let mut next = tokens.next(token);
            if tokens.is_a(next, TokenType::CLOSE_PAREN) {
                token = next;
                break;
            } else if has_named_fields
                && tokens.is_a(next, TokenType::CLOSE_CURLY_BRACKET)
                && tokens.is_a(tokens.next(next), TokenType::CLOSE_PAREN)
            {
                token = tokens.next(next);
                break;
            }
            parameter_count += 1;
            let value = tokens.string_value(next);
            if !has_named_fields && value == Some("{") {
                has_named_fields = true;
                token = tokens.next(token);
            }
            if tokens.is_a(tokens.next(token), TokenType::AT) {
                token = skip_metadata_g(tokens, token);
            }
            let ty = compute_type_g(tokens, token, /* required = */ true, false, false);
            if ty.recovered() {
                self.recovered.set(true);
                return;
            }
            token = ty.skip_type_g(tokens, token);
            if tokens.is_identifier(tokens.next(token)) {
                token = tokens.next(token);
            } else if has_named_fields {
                self.recovered.set(true);
                return;
            }

            next = tokens.next(token);
            if !tokens.is_a(next, TokenType::COMMA) {
                let next = tokens.next(token);
                if tokens.is_a(next, TokenType::CLOSE_PAREN) {
                    token = next;
                } else if tokens.is_a(next, TokenType::CLOSE_CURLY_BRACKET)
                    && tokens.is_a(tokens.next(next), TokenType::CLOSE_PAREN)
                {
                    token = tokens.next(next);
                } else {
                    // Recovery.
                    self.recovered.set(true);
                    return;
                }
                break;
            } else {
                has_comma = true;
            }
            token = next;
        }

        if !self.recovered.get()
            && ((parameter_count == 1 && !has_named_fields && !has_comma) || token != end_group)
        {
            self.recovered.set(true);
        }
    }

    /// Given void `Function` non-identifier, compute the type
    /// and return the receiver or one of the [TypeInfo] constants.
    /// Dart (line 949): `TypeInfo computeVoidGFT(bool required)`
    pub(crate) fn compute_void_gft<V: TokenView + ?Sized>(
        mut self,
        tokens: &mut V,
        required: bool,
    ) -> TypeInfo {
        debug_assert!(tokens.is_a(self.start.get(), Keyword::VOID));
        debug_assert!(tokens.is_a(tokens.next(self.start.get()), Keyword::FUNCTION));

        let start = self.start.get();
        self.compute_rest(tokens, start, required);
        if self.gft_has_return_type.is_none() {
            return VOID_TYPE;
        }
        debug_assert!(self.end.get().is_some());
        self.into_type_info()
    }

    /// Given identifier `Function` non-identifier, compute the type
    /// and return the receiver or one of the [TypeInfo] constants.
    /// Dart (line 963): `TypeInfo computeIdentifierGFT(bool required)`
    pub(crate) fn compute_identifier_gft<V: TokenView + ?Sized>(
        mut self,
        tokens: &mut V,
        required: bool,
    ) -> TypeInfo {
        debug_assert!(is_valid_non_record_type_reference_g(
            tokens,
            self.start.get()
        ));
        debug_assert!(tokens.is_a(tokens.next(self.start.get()), Keyword::FUNCTION));

        let start = self.start.get();
        self.compute_rest(tokens, start, required);
        if self.gft_has_return_type.is_none() {
            return SIMPLE_TYPE;
        }
        debug_assert!(self.end.get().is_some());
        self.into_type_info()
    }

    /// Given
    ///
    ///   `(` unchecked content assumed to be RecordType `)`
    ///      `Function` non-identifier
    ///
    /// compute the type and return the receiver or one of the [TypeInfo]
    /// constants.
    /// Dart (line 982): `TypeInfo computeRecordTypeGFT(bool required)`
    pub(crate) fn compute_record_type_gft<V: TokenView + ?Sized>(
        mut self,
        tokens: &mut V,
        required: bool,
    ) -> TypeInfo {
        debug_assert!(is_possible_record_type_g(tokens, self.start.get()));

        // TODO(jensj): Check the record type stuff to set recovered properly.

        let end_group = tokens.end_group(self.start.get()).unwrap();
        self.compute_rest(tokens, end_group, required);
        if self.gft_has_return_type.is_none() {
            return self.compute_record_type(tokens, required);
        }
        debug_assert!(self.end.get().is_some());
        self.gft_return_type_has_record_type = true;
        self.into_type_info()
    }

    /// Given identifier `?` `Function` non-identifier, compute the type
    /// and return the receiver or one of the [TypeInfo] constants.
    /// Dart (line 999): `TypeInfo computeIdentifierQuestionGFT(bool required)`
    pub(crate) fn compute_identifier_question_gft<V: TokenView + ?Sized>(
        mut self,
        tokens: &mut V,
        required: bool,
    ) -> TypeInfo {
        debug_assert!(is_valid_non_record_type_reference_g(
            tokens,
            self.start.get()
        ));
        debug_assert!(tokens.is_a(tokens.next(self.start.get()), TokenType::QUESTION));

        let start = self.start.get();
        self.compute_rest(tokens, start, required);
        if self.gft_has_return_type.is_none() {
            return SIMPLE_NULLABLE_TYPE;
        }
        debug_assert!(self.end.get().is_some());
        self.into_type_info()
    }

    /// Given
    ///
    ///   `(` unchecked content assumed to be RecordType `)` `?`
    ///      `Function` non-identifier
    ///
    /// compute the type and return the receiver or one of the [TypeInfo]
    /// constants.
    /// Dart (line 1019): `TypeInfo computeRecordTypeQuestionGFT(bool required)`
    pub(crate) fn compute_record_type_question_gft<V: TokenView + ?Sized>(
        mut self,
        tokens: &mut V,
        required: bool,
    ) -> TypeInfo {
        debug_assert!(is_possible_record_type_g(tokens, self.start.get()));

        // TODO(jensj): Check the record type stuff to set recovered properly.

        let end_group = tokens.end_group(self.start.get()).unwrap();
        self.compute_rest(tokens, end_group, required);
        if self.gft_has_return_type.is_none() {
            return self.compute_record_type(tokens, required);
        }
        debug_assert!(self.end.get().is_some());
        self.gft_return_type_has_record_type = true;
        self.into_type_info()
    }

    /// Given a builtin, return the receiver so that parseType will report
    /// an error for the builtin used as a type.
    /// Dart (line 1037): `ComplexTypeInfo computeBuiltinOrVarAsType(bool required)`
    pub(crate) fn compute_builtin_or_var_as_type<V: TokenView + ?Sized>(
        mut self,
        tokens: &mut V,
        required: bool,
    ) -> ComplexTypeInfo {
        debug_assert!(
            tokens.ty(self.start.get()).is_built_in()
                || tokens.is_a(self.start.get(), Keyword::VAR)
        );

        let end = self.type_arguments.skip_g(tokens, self.start.get());
        self.end.set(Some(end));
        self.compute_rest(tokens, end, required);
        debug_assert!(self.end.get().is_some());
        self
    }

    /// Given identifier `<` ... `>`, compute the type
    /// and return the receiver or one of the [TypeInfo] constants.
    /// Dart (line 1048): `TypeInfo computeSimpleWithTypeArguments(bool required)`
    pub(crate) fn compute_simple_with_type_arguments<V: TokenView + ?Sized>(
        mut self,
        tokens: &mut V,
        required: bool,
    ) -> TypeInfo {
        debug_assert!(is_valid_non_record_type_reference_g(
            tokens,
            self.start.get()
        ));
        debug_assert!(tokens.is_a(tokens.next(self.start.get()), TokenType::LT));
        debug_assert!(self.type_arguments != NO_TYPE_PARAM_OR_ARG);

        let end = self.type_arguments.skip_g(tokens, self.start.get());
        self.end.set(Some(end));
        self.compute_rest(tokens, end, required);

        if !required
            && !looks_like_name_or_end_of_block_g(tokens, tokens.next(self.end.get().unwrap()))
            && self.gft_has_return_type.is_none()
        {
            return NO_TYPE;
        }
        debug_assert!(self.end.get().is_some());
        self.into_type_info()
    }

    /// Given identifier `.` identifier (or `.` identifier or identifier `.`
    /// for recovery), compute the type and return the receiver or one of the
    /// [TypeInfo] constants.
    /// Dart (line 1068): `TypeInfo computePrefixedType(bool required)`
    pub(crate) fn compute_prefixed_type<V: TokenView + ?Sized>(
        mut self,
        tokens: &mut V,
        required: bool,
    ) -> TypeInfo {
        let mut token = self.start.get();
        if !tokens.is_a(token, TokenType::PERIOD) {
            debug_assert!(tokens.is_keyword_or_identifier(token));
            token = tokens.next(token);
        }
        debug_assert!(tokens.is_a(token, TokenType::PERIOD));
        if tokens.is_keyword_or_identifier(tokens.next(token)) {
            token = tokens.next(token);
        }

        let end = self.type_arguments.skip_g(tokens, token);
        self.end.set(Some(end));
        self.compute_rest(tokens, end, required);
        if !required
            && !looks_like_name_g(tokens, tokens.next(self.end.get().unwrap()))
            && self.gft_has_return_type.is_none()
        {
            return NO_TYPE;
        }
        debug_assert!(self.end.get().is_some());
        self.into_type_info()
    }

    /// Dart (line 1088): `void computeRest(Token token, bool required)`
    pub(crate) fn compute_rest<V: TokenView + ?Sized>(
        &mut self,
        tokens: &mut V,
        mut token: TokenId,
        required: bool,
    ) {
        if tokens.is_a(tokens.next(token), TokenType::QUESTION) {
            self.before_question_mark = Some(token);
            token = tokens.next(token);
            self.end.set(Some(token));
        }
        token = tokens.next(token);
        while tokens.is_a(token, Keyword::FUNCTION) {
            let type_variable_start = token;
            // TODO(danrubel): Consider caching TypeParamOrArgInfo
            token =
                compute_type_param_or_arg_g(tokens, token, /* inDeclaration = */ true, false)
                    .skip_g(tokens, token);
            token = tokens.next(token);
            if !tokens.is_a(token, TokenType::OPEN_PAREN) {
                break; // Not a function type.
            }
            let Some(end_group) = tokens.end_group(token) else {
                break; // Not a function type.
            };
            token = end_group;
            if !required {
                let mut next = tokens.next(token);
                if tokens.is_a(next, TokenType::QUESTION) {
                    next = tokens.next(next);
                }
                if !(tokens.is_identifier(next)
                    || tokens.is_a(next, Keyword::THIS)
                    || tokens.is_a(next, Keyword::SUPER))
                {
                    break; // `Function` used as the name in a function declaration.
                }
            }
            debug_assert!(tokens.is_a(token, TokenType::CLOSE_PAREN));
            if self.gft_has_return_type.is_none() {
                self.gft_has_return_type = Some(type_variable_start != self.start.get());
            }
            // Dart `typeVariableStarters.prepend(typeVariableStart)`.
            self.type_variable_starters.insert(0, type_variable_start);

            self.before_question_mark = None;
            self.end.set(Some(token));
            token = tokens.next(token);

            if tokens.is_a(token, TokenType::QUESTION) {
                self.before_question_mark = self.end.get();
                self.end.set(Some(token));
                token = tokens.next(token);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// NoTypeParamOrArg.

/// See [noTypeParamOrArg].
/// Dart (line 1151): `class NoTypeParamOrArg extends TypeParamOrArgInfo`
pub(crate) struct NoTypeParamOrArg;

impl NoTypeParamOrArg {
    /// Dart (line 1158): `Token parseArguments(Token token, Parser parser)`
    pub(crate) fn parse_arguments<L: Listener>(token: TokenId, parser: &mut Parser<L>) -> TokenId {
        let next = parser.next(token);
        parser.listener.handle_no_type_arguments(next);
        token
    }

    /// Dart (line 1164): `Token parseVariables(Token token, Parser parser)`
    pub(crate) fn parse_variables<L: Listener>(token: TokenId, parser: &mut Parser<L>) -> TokenId {
        let next = parser.next(token);
        parser.listener.handle_no_type_variables(next);
        token
    }

    /// Dart (line 1170): `Token skip(Token token)`
    pub(crate) fn skip(token: TokenId) -> TokenId {
        token
    }
}

// ---------------------------------------------------------------------------
// SimpleTypeArgument1 / SimpleTypeArgument1GtEq / SimpleTypeArgument1GtGt.

/// Dart (line 1178): `class SimpleTypeArgument1 extends TypeParamOrArgInfo`,
/// Dart (line 1253): `class SimpleTypeArgument1GtEq extends SimpleTypeArgument1`,
/// Dart (line 1283): `class SimpleTypeArgument1GtGt extends SimpleTypeArgument1`.
///
/// The functions take `closer` to select the class: `GT`
/// (`SimpleTypeArgument1`), `GT_EQ` or `GT_GT`.
pub(crate) struct SimpleTypeArgument1;

impl SimpleTypeArgument1 {
    /// Dart (line 1191): `Token parseArguments(Token token, Parser parser)`
    pub(crate) fn parse_arguments<L: Listener>(
        closer: TokenType,
        token: TokenId,
        parser: &mut Parser<L>,
    ) -> TokenId {
        let begin_group = parser.next(token);
        debug_assert!(parser.is_a(begin_group, TokenType::LT));
        let before_end_group = parser.next(begin_group);
        let end_group = SimpleTypeArgument1::parse_end_group(
            closer,
            parser.tokens_mut(),
            begin_group,
            before_end_group,
        );
        parser.listener.begin_type_arguments(begin_group);
        SIMPLE_TYPE.parse_type(begin_group, parser);
        parser
            .listener
            .end_type_arguments(/* count = */ 1, begin_group, end_group);
        end_group
    }

    /// Dart (line 1203): `Token parseVariables(Token token, Parser parser)`
    pub(crate) fn parse_variables<L: Listener>(
        closer: TokenType,
        mut token: TokenId,
        parser: &mut Parser<L>,
    ) -> TokenId {
        let begin_group = parser.next(token);
        debug_assert!(parser.is_a(begin_group, TokenType::LT));
        token = parser.next(begin_group);
        let end_group =
            SimpleTypeArgument1::parse_end_group(closer, parser.tokens_mut(), begin_group, token);
        parser.listener.begin_type_variables(begin_group);
        parser.listener.begin_metadata_star(token);
        parser.listener.end_metadata_star(/* count = */ 0);
        parser
            .listener
            .handle_identifier(token, IdentifierContext::TypeVariableDeclaration);
        parser.listener.begin_type_variable(token);
        parser
            .listener
            .handle_type_variables_defined(token, /* count = */ 1);
        parser.listener.handle_no_type(token);
        parser.listener.end_type_variable(
            end_group, /* index = */ 0, /* extendsOrSuper = */ None,
            /* variance = */ None,
        );
        parser.listener.end_type_variables(begin_group, end_group);
        end_group
    }

    /// Dart (line 1227): `Token skip(Token token)`
    pub(crate) fn skip<V: TokenView + ?Sized>(
        closer: TokenType,
        tokens: &mut V,
        mut token: TokenId,
    ) -> TokenId {
        token = tokens.next(token);
        debug_assert!(tokens.is_a(token, TokenType::LT));
        token = tokens.next(token);
        debug_assert!(tokens.is_keyword_or_identifier(token));
        SimpleTypeArgument1::skip_end_group(closer, tokens, token)
    }

    /// Dart (line 1235): `Token skipEndGroup(Token token)` and the overrides
    /// in `SimpleTypeArgument1GtEq` (Dart line 1260) and
    /// `SimpleTypeArgument1GtGt` (Dart line 1290).
    fn skip_end_group<V: TokenView + ?Sized>(
        closer: TokenType,
        tokens: &mut V,
        mut token: TokenId,
    ) -> TokenId {
        token = tokens.next(token);
        if closer == TokenType::GT_EQ {
            debug_assert!(tokens.is_a(token, TokenType::GT_EQ));
            return tokens.split(token, Split::GtEq);
        }
        if closer == TokenType::GT_GT {
            debug_assert!(tokens.is_a(token, TokenType::GT_GT));
            return tokens.split(token, Split::GtGt);
        }
        debug_assert!(tokens.is_a(token, TokenType::GT));
        token
    }

    /// Dart (line 1241): `Token parseEndGroup(Token beginGroup, Token token)`
    /// and the overrides in `SimpleTypeArgument1GtEq` (Dart line 1267) and
    /// `SimpleTypeArgument1GtGt` (Dart line 1297).
    fn parse_end_group(
        closer: TokenType,
        tokens: &mut Tokens,
        _begin_group: TokenId,
        before_end_group: TokenId,
    ) -> TokenId {
        if closer == TokenType::GT_EQ || closer == TokenType::GT_GT {
            let mut end_group = tokens.next(before_end_group);
            if tokens.ty(end_group) != TokenType::GT {
                end_group = if closer == TokenType::GT_EQ {
                    split_gt_eq(tokens, end_group)
                } else {
                    split_gt_gt(tokens, end_group)
                };
                let next = tokens.next(end_group);
                let next_next = tokens.next(next);
                tokens.set_next(next, next_next);
            }
            tokens.set_next(before_end_group, end_group);
            return end_group;
        }
        let token = tokens.next(before_end_group);
        debug_assert!(tokens.ty(token) == TokenType::GT);
        token
    }
}

// ---------------------------------------------------------------------------
// ComplexTypeParamOrArgInfo.

/// Dart (line 1313): `class ComplexTypeParamOrArgInfo extends TypeParamOrArgInfo`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComplexTypeParamOrArgInfo {
    /// The first token in the type var.
    pub start: TokenId,

    /// If [inDeclaration] is `true`, then this will more aggressively recover
    /// given unbalanced `<` `>` and invalid parameters or arguments.
    pub in_declaration: bool,

    // Only support variance parsing if it makes sense.
    // Allows parsing of variance for certain structures.
    // See https://github.com/dart-lang/language/issues/524
    pub allows_variance: bool,

    pub type_argument_count: i32,

    /// The `>` token which ends the type parameter or argument.
    /// This closer may be synthetic, points to the next token in the stream,
    /// is only used when skipping over the type parameters or arguments,
    /// and may not be part of the token stream.
    pub skip_end: Option<TokenId>,

    pub recovered: bool,
}

impl ComplexTypeParamOrArgInfo {
    /// Dart (line 1338): `ComplexTypeParamOrArgInfo(Token token, this.inDeclaration, this.allowsVariance)`
    pub(crate) fn new<R: TokenRead + ?Sized>(
        tokens: &R,
        token: TokenId,
        in_declaration: bool,
        allows_variance: bool,
    ) -> Self {
        debug_assert!(tokens.is_a(tokens.next(token), TokenType::LT));
        ComplexTypeParamOrArgInfo {
            start: tokens.next(token),
            in_declaration,
            allows_variance,
            type_argument_count: 0,
            skip_end: None,
            recovered: false,
        }
    }

    /// Parse the tokens and return the receiver or [noTypeParamOrArg] if there
    /// are no type parameters or arguments. This does not modify the token
    /// stream.
    /// Dart (line 1348): `TypeParamOrArgInfo compute()`
    #[allow(unused_assignments, clippy::assign_op_pattern)]
    pub(crate) fn compute<V: TokenView + ?Sized>(mut self, tokens: &mut V) -> TypeParamOrArgInfo {
        let mut token: TokenId;
        let mut next = self.start;
        loop {
            let mut type_info = compute_type_g(
                tokens,
                next,
                /* required = */ true,
                self.in_declaration,
                false,
            );
            self.recovered = self.recovered | type_info.recovered();
            if type_info == NO_TYPE {
                while type_info == NO_TYPE && tokens.is_a(tokens.next(next), TokenType::AT) {
                    next = skip_metadata_g(tokens, next);
                    type_info = compute_type_g(
                        tokens,
                        next,
                        /* required = */ true,
                        self.in_declaration,
                        false,
                    );
                }
                if type_info == NO_TYPE {
                    if next == self.start
                        && !self.in_declaration
                        && !is_closer_g(tokens, tokens.next(next))
                        && !tokens.is_a(tokens.next(next), TokenType::COMMA)
                    {
                        return NO_TYPE_PARAM_OR_ARG;
                    }
                    if !tokens.is_a(tokens.next(next), TokenType::COMMA) {
                        token = next;
                        next = tokens.next(token);
                        break;
                    }
                }
                debug_assert!(
                    type_info != NO_TYPE || tokens.is_a(tokens.next(next), TokenType::COMMA)
                );
                // Fall through to process type (if any) and consume `,`
            }
            self.type_argument_count += 1;
            token = type_info.skip_type_g(tokens, next);
            next = tokens.next(token);
            if tokens.is_a(next, Keyword::EXTENDS) {
                token = compute_type_g(
                    tokens,
                    next,
                    /* required = */ true,
                    self.in_declaration,
                    false,
                )
                .skip_type_g(tokens, next);
                next = tokens.next(token);
            }
            if !tokens.is_a(next, TokenType::COMMA) {
                self.skip_end = split_closer_g(tokens, next);
                if self.skip_end.is_some() {
                    return TypeParamOrArgInfo::Complex(self);
                }
                if !self.in_declaration {
                    return NO_TYPE_PARAM_OR_ARG;
                }

                // Recovery
                if !looks_like_type_param_or_arg_g(tokens, self.in_declaration, next) {
                    break;
                }
                // Looks like missing comma. Continue looping.
                next = token;
            }
        }

        // Recovery
        self.skip_end = split_closer_g(tokens, next);
        if self.skip_end.is_none() {
            self.recovered = true;
            if tokens.is_a(next, TokenType::OPEN_PAREN) {
                token = tokens.end_group(next).unwrap();
                next = tokens.next(token);
            }
            self.skip_end = split_closer_g(tokens, next);
            if self.skip_end.is_none() {
                let next_next = tokens.next(next);
                self.skip_end = split_closer_g(tokens, next_next);
            }
            if self.skip_end.is_none() {
                self.skip_end = Some(tokens.synthetic_gt(next));
            }
        }
        TypeParamOrArgInfo::Complex(self)
    }

    /// Dart (line 1428): `Token parseArguments(Token token, Parser parser)`
    pub(crate) fn parse_arguments<L: Listener>(
        &self,
        mut token: TokenId,
        parser: &mut Parser<L>,
    ) -> TokenId {
        debug_assert!(parser.next(token) == self.start);
        let mut next = self.start;
        parser.listener.begin_type_arguments(self.start);
        let mut count = 0;
        loop {
            let mut type_info = compute_type_g(
                parser.tokens_mut(),
                next,
                /* required = */ true,
                self.in_declaration,
                false,
            );
            if type_info == NO_TYPE {
                // Recovery
                while type_info == NO_TYPE && parser.is_a(parser.next(next), TokenType::AT) {
                    let at_token = parser.next(next);
                    next = skip_metadata_g(parser.tokens(), next);
                    parser.report_recoverable_error_with_end(
                        at_token,
                        next,
                        diag::annotation_on_type_argument(),
                    );
                    type_info = compute_type_g(
                        parser.tokens_mut(),
                        next,
                        /* required = */ true,
                        self.in_declaration,
                        false,
                    );
                }
                // Fall through to process type (if any) and consume `,`
            }
            token = type_info.ensure_type_or_void(next, parser);
            next = parser.next(token);
            count += 1;
            if !parser.is_a(next, TokenType::COMMA) {
                if parse_closer(parser.tokens_mut(), token) {
                    break;
                }

                // Recovery
                if !looks_like_type_param_or_arg(parser.tokens(), self.in_declaration, next) {
                    token = self.parse_unexpected_end(token, /* isArguments = */ true, parser);
                    break;
                }
                // Missing comma. Report error, insert comma, and continue looping.
                next = self.parse_missing_comma(token, parser);
            }
        }
        let end_group = parser.next(token);
        parser
            .listener
            .end_type_arguments(count, self.start, end_group);
        end_group
    }

    /// Dart (line 1476): `Token parseVariables(Token token, Parser parser)`
    pub(crate) fn parse_variables<L: Listener>(
        &self,
        mut token: TokenId,
        parser: &mut Parser<L>,
    ) -> TokenId {
        debug_assert!(parser.next(token) == self.start);
        let mut next = self.start;
        parser.listener.begin_type_variables(self.start);
        let mut count = 0;

        // Dart `Link`s: the last added element is the head (here: the last
        // element of the `Vec`).
        let mut type_starts: Vec<TokenId> = Vec::new();
        let mut super_type_infos: Vec<Option<TypeInfo>> = Vec::new();
        let mut variances: Vec<Option<TokenId>> = Vec::new();

        loop {
            token = parser.parse_metadata_star(next);

            let mut variance = parser.next(next);
            let mut identifier = parser.tokens().next_opt(variance);
            if self.allows_variance
                && is_variance(parser.tokens(), variance)
                && identifier.is_some_and(|identifier| parser.is_keyword_or_identifier(identifier))
            {
                variances.push(Some(variance));

                // Recovery for multiple variance modifiers
                while let Some(id) = identifier
                    && is_variance(parser.tokens(), id)
                    && parser
                        .tokens()
                        .next_opt(id)
                        .is_some_and(|next| parser.is_keyword_or_identifier(next))
                {
                    // Report an error and skip actual identifier
                    parser.report_recoverable_error(id, diag::multiple_variance_modifiers());
                    variance = parser.next(variance);
                    identifier = Some(parser.next(id));
                }

                token = variance;
            } else {
                variances.push(/* element = */ None);
            }

            next = parser.ensure_identifier(token, IdentifierContext::TypeVariableDeclaration);
            token = next;
            parser.listener.begin_type_variable(token);
            type_starts.push(token);

            next = parser.next(token);
            if parser.is_a(next, Keyword::EXTENDS) {
                let type_info = compute_type_g(
                    parser.tokens_mut(),
                    next,
                    /* required = */ true,
                    self.in_declaration,
                    false,
                );
                token = type_info.skip_type_g(parser.tokens_mut(), next);
                next = parser.next(token);
                super_type_infos.push(Some(type_info));
            } else {
                super_type_infos.push(/* element = */ None);
            }

            count += 1;
            if !parser.is_a(next, TokenType::COMMA) {
                if is_closer(parser.tokens(), token) {
                    break;
                }

                // Recovery
                if !looks_like_type_param_or_arg(parser.tokens(), self.in_declaration, next) {
                    break;
                }
                // Missing comma. Report error, insert comma, and continue looping.
                next = self.parse_missing_comma(token, parser);
            }
        }

        debug_assert!(count > 0);
        debug_assert!(type_starts.len() == count as usize);
        debug_assert!(super_type_infos.len() == count as usize);
        debug_assert!(variances.len() == count as usize);
        parser.listener.handle_type_variables_defined(token, count);

        let mut token3: Option<TokenId> = None;
        while let Some(mut token2) = type_starts.pop() {
            let type_info = super_type_infos.pop().unwrap();
            let variance = variances.pop().unwrap();

            let mut extends_or_super: Option<TokenId> = None;
            let mut next2 = parser.next(token2);
            if let Some(type_info) = type_info {
                debug_assert!(parser.is_a(next2, Keyword::EXTENDS));
                extends_or_super = Some(next2);
                token2 = type_info.ensure_type_not_void(next2, parser);
                next2 = parser.next(token2);
            } else {
                debug_assert!(!parser.is_a(next2, Keyword::EXTENDS));
                parser.listener.handle_no_type(token2);
            }
            // Type variables are "completed" in reverse order, so capture the last
            // consumed token from the first "completed" type variable.
            if token3.is_none() {
                token3 = Some(token2);
            }
            count -= 1;
            parser
                .listener
                .end_type_variable(next2, count, extends_or_super, variance);
        }

        let mut token3 = token3.unwrap();
        if !parse_closer(parser.tokens_mut(), token3) {
            token3 = self.parse_unexpected_end(token3, /* isArguments = */ false, parser);
        }
        let end_group = parser.next(token3);
        parser.listener.end_type_variables(self.start, end_group);
        end_group
    }

    /// Dart (line 1595): `Token parseMissingComma(Token token, Parser parser)`
    fn parse_missing_comma<L: Listener>(&self, token: TokenId, parser: &mut Parser<L>) -> TokenId {
        let next = parser.next(token);
        parser.report_recoverable_error(next, diag::expected_but_got(","));
        let offset = parser.char_offset(next);
        let byte_offset = parser.tokens().byte_offset(next);
        let comma = parser
            .tokens_mut()
            .push_synthetic(TokenType::COMMA, offset, byte_offset);
        parser.rewriter().insert_token(token, comma)
    }

    /// Dart (line 1607): `Token parseUnexpectedEnd(Token token, bool isArguments, Parser parser)`
    fn parse_unexpected_end<L: Listener>(
        &self,
        mut token: TokenId,
        is_arguments: bool,
        parser: &mut Parser<L>,
    ) -> TokenId {
        let mut next = parser.next(token);
        let mut error_reported =
            parser.is_synthetic(token) || (parser.is_synthetic(next) && !parser.is_eof(next));

        let mut type_follows_extends = false;
        if parser.is_a(next, Keyword::EXTENDS) {
            if !error_reported {
                parser.report_recoverable_error(token, diag::expected_after_but_got(">"));
                error_reported = true;
            }
            token = next;
            next = parser.next(token);
            type_follows_extends = is_valid_non_record_type_reference(parser.tokens(), next);

            if parse_closer(parser.tokens_mut(), token) {
                return token;
            }
        }

        if type_follows_extends
            || parser.is_a(next, Keyword::DYNAMIC)
            || parser.is_a(next, Keyword::VOID)
            || parser.is_a(next, Keyword::FUNCTION)
        {
            let invalid_type = compute_type_g(
                parser.tokens_mut(),
                token,
                /* required = */ true,
                false,
                false,
            );
            if invalid_type != NO_TYPE {
                if !error_reported {
                    parser.report_recoverable_error(token, diag::expected_after_but_got(">"));
                    error_reported = true;
                }

                // Parse the type so that the token stream is properly modified,
                // but ensure that parser events are ignored by replacing the listener.
                parser.push_silent_listener();
                token = invalid_type.parse_type(token, parser);
                next = parser.next(token);
                parser.pop_layer();

                if parse_closer(parser.tokens_mut(), token) {
                    return token;
                }
            }
        }

        let invalid_type_var =
            compute_type_param_or_arg_g(parser.tokens_mut(), token, self.in_declaration, false);
        if invalid_type_var != NO_TYPE_PARAM_OR_ARG {
            if !error_reported {
                parser.report_recoverable_error(token, diag::expected_after_but_got(">"));
                error_reported = true;
            }

            // Parse the type so that the token stream is properly modified,
            // but ensure that parser events are ignored by replacing the listener.
            parser.push_silent_listener();
            token = if is_arguments {
                invalid_type_var.parse_arguments(token, parser)
            } else {
                invalid_type_var.parse_variables(token, parser)
            };
            next = parser.next(token);
            parser.pop_layer();

            if parse_closer(parser.tokens_mut(), token) {
                return token;
            }
        }

        if parser.is_a(next, TokenType::OPEN_PAREN) && parser.end_group(next).is_some() {
            if !error_reported {
                // Only report an error if one has not already been reported.
                parser.report_recoverable_error(token, diag::expected_after_but_got(">"));
                error_reported = true;
            }
            token = parser.end_group(next).unwrap();
            next = parser.next(token);

            if parse_closer(parser.tokens_mut(), token) {
                return token;
            }
        }

        if !error_reported {
            // Only report an error if one has not already been reported.
            parser.report_recoverable_error(token, diag::expected_after_but_got(">"));
        }
        if parse_closer(parser.tokens_mut(), next) {
            return next;
        }
        let end_group = parser.end_group(self.start);
        if let Some(end_group) = end_group {
            while parser.tokens().next_opt(token) != Some(end_group)
                && !parser.is_eof(token)
                && parser.token(token).signed_offset() <= parser.token(end_group).signed_offset()
            {
                token = parser.next(token);
            }
        } else {
            let end_group = synthetic_gt(parser.tokens_mut(), next);
            parser.tokens_mut().set_next(end_group, next);
            parser.tokens_mut().set_next(token, end_group);
        }
        token
    }

    /// Dart (line 1728): `Token skip(Token token)`
    pub(crate) fn skip(&self, _token: TokenId) -> TokenId {
        self.skip_end.unwrap()
    }
}

// ---------------------------------------------------------------------------
// Free functions.

/// Return `true` if [token] is one of `in`, `inout`, or `out`
/// Dart (line 1746): `bool isVariance(Token token)`
pub fn is_variance(tokens: &Tokens, token: TokenId) -> bool {
    tokens.is_a(token, Keyword::IN)
        || tokens.is_a(token, Keyword::INOUT)
        || tokens.is_a(token, Keyword::OUT)
}

/// Return `true` if [token] is one of `>`, `>>`, `>>>`, `>=`, `>>=`, or `>>>=`.
/// Dart (line 1753): `bool isCloser(Token token)`
pub fn is_closer(tokens: &Tokens, token: TokenId) -> bool {
    is_closer_g(tokens, token)
}

/// [`is_closer`] for any [`TokenRead`].
pub(crate) fn is_closer_g<R: TokenRead + ?Sized>(tokens: &R, token: TokenId) -> bool {
    tokens.is_a(token, TokenType::GT)
        || tokens.is_a(token, TokenType::GT_GT)
        || tokens.is_a(token, TokenType::GT_EQ)
        || tokens.is_a(token, TokenType::GT_GT_GT)
        || tokens.is_a(token, TokenType::GT_GT_EQ)
        || tokens.is_a(token, TokenType::GT_GT_GT_EQ)
}

/// If [beforeCloser].next is one of `>`, `>>`, `>>>`, `>=`, `>>=`, or `>>>=`
/// then update the token stream and return `true`.
/// Dart (line 1764): `bool parseCloser(Token beforeCloser)`
pub fn parse_closer(tokens: &mut Tokens, before_closer: TokenId) -> bool {
    let unsplit = tokens.next(before_closer);
    let split = split_closer(tokens, unsplit);
    let Some(split) = split else {
        return false;
    };
    if split == unsplit {
        return true;
    }
    let split_next = tokens.next(split);
    let unsplit_next = tokens.next(unsplit);
    tokens.set_next(split_next, unsplit_next);
    tokens.set_next(before_closer, split);
    true
}

/// If [closer] is `>` then return it.
/// If [closer] is one of `>>`, `>>>`, `>=`, `>>=`,  or `>>>=` then split
/// the token and return the leading `>` without updating the token stream.
/// If [closer] is none of the above, then return null;
/// Dart (line 1781): `Token? splitCloser(Token closer)`
pub fn split_closer(tokens: &mut Tokens, closer: TokenId) -> Option<TokenId> {
    split_closer_g(tokens, closer)
}

/// [`split_closer`] for any [`TokenView`].
pub(crate) fn split_closer_g<V: TokenView + ?Sized>(
    tokens: &mut V,
    closer: TokenId,
) -> Option<TokenId> {
    if tokens.is_a(closer, TokenType::GT) {
        return Some(closer);
    } else if tokens.is_a(closer, TokenType::GT_GT) {
        return Some(tokens.split(closer, Split::GtGt));
    } else if tokens.is_a(closer, TokenType::GT_EQ) {
        return Some(tokens.split(closer, Split::GtEq));
    } else if tokens.is_a(closer, TokenType::GT_GT_GT) {
        return Some(tokens.split(closer, Split::GtFromGtGtGt));
    } else if tokens.is_a(closer, TokenType::GT_GT_EQ) {
        return Some(tokens.split(closer, Split::GtFromGtGtEq));
    } else if tokens.is_a(closer, TokenType::GT_GT_GT_EQ) {
        return Some(tokens.split(closer, Split::GtFromGtGtGtEq));
    }
    None
}
