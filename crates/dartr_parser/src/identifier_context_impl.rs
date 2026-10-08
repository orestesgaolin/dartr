// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/identifier_context_impl.dart

//! The `*IdentifierContext` classes of Dart are ported as free functions
//! (`<class>_ensure_identifier`), dispatched from
//! [`IdentifierContext::ensure_identifier`] with `match self`. The `context`
//! parameter is Dart `this`.

use dartr_diagnostics::cfe_codes as diag;
use dartr_syntax::token_constants::STRING_TOKEN;
use dartr_syntax::{Keyword, TokenId, TokenType, Tokens};

use crate::identifier_context::{
    IdentifierContext, is_ok_next_value_in_formal_parameter, looks_like_statement_start,
};
use crate::listener::Listener;
use crate::parser_impl::Parser;
use crate::type_info::is_valid_non_record_type_reference;
use crate::util::is_any_of;

#[inline(always)]
fn is_a(tokens: &Tokens, token: TokenId, ty: TokenType) -> bool {
    tokens.ty(token) == ty
}

/// Dart `parser.insertSyntheticIdentifier(token, this, message:
/// diag.expectedIdentifier.withArguments(lexeme: identifier))`.
fn insert_synthetic_identifier_expected<L: Listener>(
    parser: &mut Parser<L>,
    token: TokenId,
    context: IdentifierContext,
    identifier: TokenId,
) -> TokenId {
    let message = diag::expected_identifier(parser.lexeme(identifier));
    parser.insert_synthetic_identifier(token, context, Some(message), None)
}

// -----------------------------------------------------------------------------
// CatchParameterIdentifierContext

/// See [IdentifierContext.catchParameter].
///
/// Dart (line 25): `CatchParameterIdentifierContext.ensureIdentifier`
pub(crate) fn catch_parameter_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let _ = context;
    let identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_identifier(identifier) {
        check_async_await_yield_as_identifier(identifier, parser);
        return identifier;
    }

    // Recovery
    parser.report_recoverable_error(identifier, diag::catch_syntax());
    if looks_like_statement_start(parser.tokens(), identifier)
        || parser.is_a(identifier, TokenType::COMMA)
        || parser.is_a(identifier, TokenType::CLOSE_PAREN)
        || parser.is_a(identifier, TokenType::EOF)
    {
        return parser.rewriter().insert_synthetic_identifier(token, "");
    } else if !parser.is_keyword_or_identifier(identifier) {
        // When in doubt, consume the token to ensure we make progress
        // but insert a synthetic identifier to satisfy listeners.
        return parser
            .rewriter()
            .insert_synthetic_identifier(identifier, "");
    }
    identifier
}

// -----------------------------------------------------------------------------
// ClassOrMixinOrExtensionIdentifierContext

/// Dart (line 58): `ClassOrMixinOrExtensionIdentifierContext._isOneOfFollowingValues`
fn class_or_mixin_is_one_of_following_values(tokens: &Tokens, token: TokenId) -> bool {
    is_a(tokens, token, TokenType::LT)
        || is_a(tokens, token, TokenType::OPEN_CURLY_BRACKET)
        || is_a(tokens, token, Keyword::EXTENDS)
        || is_a(tokens, token, Keyword::WITH)
        || is_a(tokens, token, Keyword::IMPLEMENTS)
        || is_a(tokens, token, Keyword::ON)
        || is_a(tokens, token, TokenType::EQ)
        || is_a(tokens, token, TokenType::OPEN_PAREN)
        || is_a(tokens, token, TokenType::PERIOD)
        || is_a(tokens, token, TokenType::EOF)
}

/// See [IdentifierContext.classOrMixinOrExtensionDeclaration].
///
/// Dart (line 72): `ClassOrMixinOrExtensionIdentifierContext.ensureIdentifier`
pub(crate) fn class_or_mixin_or_extension_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let mut identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_pseudo(identifier) {
        return identifier;
    }

    // Recovery
    let tokens = parser.tokens();
    let identifier_next = tokens.next(identifier);
    if parser.is_eof(identifier)
        || (looks_like_start_of_next_top_level_declaration(tokens, identifier)
            && (identifier_next.is_none()
                || !class_or_mixin_is_one_of_following_values(tokens, identifier_next)))
        || (class_or_mixin_is_one_of_following_values(tokens, identifier)
            && (identifier_next.is_none()
                || !class_or_mixin_is_one_of_following_values(tokens, identifier_next)))
    {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else if parser.is_built_in(identifier) {
        parser.report_recoverable_error_with_token(
            identifier,
            diag::built_in_identifier_in_declaration,
        );
    } else {
        if !parser.is_keyword_or_identifier(identifier) {
            parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
            // When in doubt, consume the token to ensure we make progress
            // but insert a synthetic identifier to satisfy listeners.
            identifier = parser
                .rewriter()
                .insert_synthetic_identifier(identifier, "");
        } else {
            // Use the keyword as the identifier.
            parser.report_recoverable_error_with_token(
                identifier,
                diag::expected_identifier_but_got_keyword,
            );
        }
    }
    identifier
}

// -----------------------------------------------------------------------------
// CombinatorIdentifierContext

/// Dart (line 122): `CombinatorIdentifierContext._isOneOfFollowingValues`
fn combinator_is_one_of_following_values(tokens: &Tokens, token: TokenId) -> bool {
    is_a(tokens, token, TokenType::SEMICOLON)
        || is_a(tokens, token, TokenType::COMMA)
        || is_a(tokens, token, Keyword::IF)
        || is_a(tokens, token, Keyword::AS)
        || is_a(tokens, token, Keyword::SHOW)
        || is_a(tokens, token, Keyword::HIDE)
        || is_a(tokens, token, TokenType::EOF)
}

/// See [IdentifierContext.combinator].
///
/// Dart (line 133): `CombinatorIdentifierContext.ensureIdentifier`
pub(crate) fn combinator_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let mut identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);

    if parser.is_identifier(identifier) {
        let tokens = parser.tokens();
        if !looks_like_start_of_next_top_level_declaration(tokens, identifier)
            || combinator_is_one_of_following_values(tokens, tokens.next(identifier))
        {
            return identifier;
        }
        // Although this is a valid identifier name, the import declaration
        // is invalid and this looks like the start of the next declaration.
        // In this situation, fall through to insert a synthetic identifier.
    }

    // Recovery
    let tokens = parser.tokens();
    let identifier_next = tokens.next(identifier);
    if combinator_is_one_of_following_values(tokens, identifier) {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else if looks_like_start_of_next_top_level_declaration(tokens, identifier)
        && (identifier_next.is_none()
            || !combinator_is_one_of_following_values(tokens, identifier_next))
    {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else {
        if !parser.is_keyword_or_identifier(identifier) {
            parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
            // When in doubt, consume the token to ensure we make progress
            // but insert a synthetic identifier to satisfy listeners.
            identifier = parser
                .rewriter()
                .insert_synthetic_identifier(identifier, "");
        } else {
            // Use the keyword as the identifier.
            parser.report_recoverable_error_with_token(
                identifier,
                diag::expected_identifier_but_got_keyword,
            );
        }
    }
    identifier
}

// -----------------------------------------------------------------------------
// ConstructorReferenceIdentifierContext

/// See [IdentifierContext.constructorReference]
/// and [IdentifierContext.constructorReferenceContinuation]
/// and [IdentifierContext.constructorReferenceContinuationAfterTypeArguments].
///
/// Dart (line 203): `ConstructorReferenceIdentifierContext.ensureIdentifier`
pub(crate) fn constructor_reference_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let mut identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_identifier(identifier) {
        check_async_await_yield_as_identifier(identifier, parser);
        return identifier;
    }

    // Recovery
    if !parser.is_keyword_or_identifier(identifier) {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else {
        // Use the keyword as the identifier.
        parser.report_recoverable_error_with_token(
            identifier,
            diag::expected_identifier_but_got_keyword,
        );
    }
    identifier
}

// -----------------------------------------------------------------------------
// DottedNameIdentifierContext

/// Dart (line 236): `DottedNameIdentifierContext._isOneOfFollowingValues`
fn dotted_name_is_one_of_following_values(tokens: &Tokens, token: TokenId) -> bool {
    is_a(tokens, token, TokenType::PERIOD)
        || is_a(tokens, token, TokenType::EQ_EQ)
        || is_a(tokens, token, TokenType::CLOSE_PAREN)
        || is_a(tokens, token, TokenType::EOF)
}

/// See [IdentifierContext.dottedName].
///
/// Dart (line 244): `DottedNameIdentifierContext.ensureIdentifier`
pub(crate) fn dotted_name_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let mut identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);

    if parser.is_identifier(identifier) {
        // DottedNameIdentifierContext are only used in conditional import
        // expressions. Although some top level keywords such as `import` can be
        // used as identifiers, they are more likely the start of the next
        // directive or declaration.
        let tokens = parser.tokens();
        if !parser.is_top_level_keyword(identifier)
            || dotted_name_is_one_of_following_values(tokens, tokens.next(identifier))
        {
            return identifier;
        }
    }

    // Recovery
    let tokens = parser.tokens();
    if looks_like_start_of_next_top_level_declaration(tokens, identifier)
        || dotted_name_is_one_of_following_values(tokens, identifier)
    {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else {
        if !parser.is_keyword_or_identifier(identifier) {
            parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
            // When in doubt, consume the token to ensure we make progress
            // but insert a synthetic identifier to satisfy listeners.
            identifier = parser
                .rewriter()
                .insert_synthetic_identifier(identifier, "");
        } else {
            // Use the keyword as the identifier.
            parser.report_recoverable_error_with_token(
                identifier,
                diag::expected_identifier_but_got_keyword,
            );
        }
    }
    identifier
}

// -----------------------------------------------------------------------------
// EnumDeclarationIdentifierContext

/// See [IdentifierContext.enumDeclaration].
///
/// Dart (line 298): `EnumDeclarationIdentifierContext.ensureIdentifier`
pub(crate) fn enum_declaration_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let mut identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_pseudo(identifier) {
        return identifier;
    }

    // Recovery
    if looks_like_start_of_next_top_level_declaration(parser.tokens(), identifier)
        || parser.is_a(identifier, TokenType::OPEN_CURLY_BRACKET)
        || parser.is_a(identifier, TokenType::EOF)
    {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else if parser.is_built_in(identifier) {
        parser.report_recoverable_error_with_token(
            identifier,
            diag::built_in_identifier_in_declaration,
        );
    } else {
        if !parser.is_keyword_or_identifier(identifier) {
            parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
            // When in doubt, consume the token to ensure we make progress
            // but insert a synthetic identifier to satisfy listeners.
            identifier = parser
                .rewriter()
                .insert_synthetic_identifier(identifier, "");
        } else {
            // Use the keyword as the identifier.
            parser.report_recoverable_error_with_token(
                identifier,
                diag::expected_identifier_but_got_keyword,
            );
        }
    }
    identifier
}

// -----------------------------------------------------------------------------
// EnumValueDeclarationIdentifierContext

/// See [IdentifierContext.enumValueDeclaration].
///
/// Dart (line 346): `EnumValueDeclarationIdentifierContext.ensureIdentifier`
pub(crate) fn enum_value_declaration_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let _ = context;
    let identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_identifier(identifier) {
        return identifier;
    }

    // Recovery
    if looks_like_start_of_next_top_level_declaration(parser.tokens(), identifier)
        || parser.is_a(identifier, TokenType::COMMA)
        || parser.is_a(identifier, TokenType::CLOSE_CURLY_BRACKET)
        || parser.is_a(identifier, TokenType::EOF)
    {
        parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
        return parser.rewriter().insert_synthetic_identifier(token, "");
    } else if !parser.is_keyword_or_identifier(identifier) {
        parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
        // When in doubt, consume the token to ensure we make progress
        // but insert a synthetic identifier to satisfy listeners.
        return parser
            .rewriter()
            .insert_synthetic_identifier(identifier, "");
    } else {
        // Use the keyword as the identifier.
        parser.report_recoverable_error_with_token(
            identifier,
            diag::expected_identifier_but_got_keyword,
        );
    }
    identifier
}

// -----------------------------------------------------------------------------
// ExpressionIdentifierContext

/// See [IdentifierContext.expression].
///
/// Dart (line 394): `ExpressionIdentifierContext.ensureIdentifier`
pub(crate) fn expression_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    mut token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_identifier(identifier) {
        if parser.is_a(identifier, Keyword::AWAIT) && parser.is_identifier(parser.next(identifier))
        {
            // Although the `await` can be used in an expression,
            // it is followed by another identifier which does not form
            // a valid expression. Report an error on the `await` token
            // rather than the token following it.
            parser.report_recoverable_error_with_token(identifier, diag::unexpected_token);

            // TODO(danrubel) Consider a new listener event so that analyzer
            // can represent this as an await expression in a context that does
            // not allow await.
            return parser.next(identifier);
        } else {
            check_async_await_yield_as_identifier(identifier, parser);
        }
        return identifier;
    }

    // Recovery
    let report_error_at = identifier;
    if parser.is_a(token, TokenType::STRING_INTERPOLATION_IDENTIFIER)
        && parser.is_keyword(identifier)
        && parser.kind(parser.next(identifier)) == STRING_TOKEN
    {
        // Keyword used as identifier in string interpolation
        parser.report_recoverable_error_with_token(
            identifier,
            diag::expected_identifier_but_got_keyword,
        );
        return identifier;
    } else if !looks_like_statement_start(parser.tokens(), identifier) {
        if parser.is_keyword_or_identifier(identifier) {
            if context.is_continuation()
                || !(parser.is_a(identifier, Keyword::AS)
                    || parser.is_a(identifier, Keyword::IS)
                    || parser.is_a(identifier, TokenType::EOF))
            {
                // Use the keyword as the identifier.
                parser.report_recoverable_error_with_token(
                    identifier,
                    diag::expected_identifier_but_got_keyword,
                );
                return identifier;
            }
        } else if !parser.is_operator(identifier)
            && !(parser.is_a(identifier, TokenType::PERIOD)
                || parser.is_a(identifier, TokenType::COMMA)
                || parser.is_a(identifier, TokenType::OPEN_PAREN)
                || parser.is_a(identifier, TokenType::CLOSE_PAREN)
                || parser.is_a(identifier, TokenType::OPEN_SQUARE_BRACKET)
                || parser.is_a(identifier, TokenType::CLOSE_SQUARE_BRACKET)
                || parser.is_a(identifier, TokenType::OPEN_CURLY_BRACKET)
                || parser.is_a(identifier, TokenType::CLOSE_CURLY_BRACKET)
                || parser.is_a(identifier, TokenType::QUESTION)
                || parser.is_a(identifier, TokenType::COLON)
                || parser.is_a(identifier, TokenType::SEMICOLON)
                || parser.is_a(identifier, TokenType::EOF))
        {
            // When in doubt, consume the token to ensure we make progress
            token = identifier;
            // identifier = token.next!; (the value is not used after this)
        }
    }

    parser.report_recoverable_error_with_token(report_error_at, diag::expected_identifier);

    // Insert a synthetic identifier to satisfy listeners.
    parser.rewriter().insert_synthetic_identifier(token, "")
}

// -----------------------------------------------------------------------------
// FieldDeclarationIdentifierContext

/// See [IdentifierContext.fieldDeclaration].
///
/// Dart (line 477): `FieldDeclarationIdentifierContext.ensureIdentifier`
pub(crate) fn field_declaration_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_identifier(identifier) {
        return identifier;
    }

    // Recovery
    if parser.is_a(identifier, TokenType::SEMICOLON)
        || parser.is_a(identifier, TokenType::EQ)
        || parser.is_a(identifier, TokenType::COMMA)
        || parser.is_a(identifier, TokenType::CLOSE_CURLY_BRACKET)
        || parser.is_a(identifier, TokenType::EOF)
        || looks_like_start_of_next_class_member(parser.tokens(), identifier)
    {
        // TODO(jensj): Why aren't an error reported here?
        parser.insert_synthetic_identifier(token, context, None, None)
    } else if !parser.is_keyword_or_identifier(identifier) {
        // When in doubt, consume the token to ensure we make progress
        // but insert a synthetic identifier to satisfy listeners.
        let message = diag::expected_identifier(parser.lexeme(identifier));
        parser.insert_synthetic_identifier(identifier, context, Some(message), Some(identifier))
    } else {
        // Use the keyword as the identifier.
        parser.report_recoverable_error_with_token(
            identifier,
            diag::expected_identifier_but_got_keyword,
        );
        identifier
    }
}

/// Dart (line 513): `FieldDeclarationIdentifierContext.ensureIdentifierPotentiallyRecovered`
pub(crate) fn field_declaration_ensure_identifier_potentially_recovered<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
    is_recovered: bool,
) -> TokenId {
    // Fast path good case.
    let identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_identifier(identifier) {
        return identifier;
    }
    // If not recovered, recover as normal.
    if !is_recovered || !parser.is_keyword_or_identifier(identifier) {
        return field_declaration_ensure_identifier(context, token, parser);
    }

    // If already recovered, use the given token.
    parser
        .report_recoverable_error_with_token(identifier, diag::expected_identifier_but_got_keyword);
    identifier
}

// -----------------------------------------------------------------------------
// FieldInitializerIdentifierContext

/// See [IdentifierContext.fieldInitializer].
///
/// Dart (line 547): `FieldInitializerIdentifierContext.ensureIdentifier`
pub(crate) fn field_initializer_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let _ = context;
    // assert(token.isA(TokenType.PERIOD));
    let identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_identifier(identifier) {
        return identifier;
    }

    // Recovery
    parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
    // Insert a synthetic identifier to satisfy listeners.
    parser.rewriter().insert_synthetic_identifier(token, "")
}

// -----------------------------------------------------------------------------
// FormalParameterDeclarationIdentifierContext

/// Dart (line 567): `FormalParameterDeclarationIdentifierContext._isOneOfFollowingValues`
fn formal_parameter_is_one_of_following_values(tokens: &Tokens, token: TokenId) -> bool {
    is_a(tokens, token, TokenType::COLON)
        || is_a(tokens, token, TokenType::EQ)
        || is_a(tokens, token, TokenType::COMMA)
        || is_a(tokens, token, TokenType::OPEN_PAREN)
        || is_a(tokens, token, TokenType::CLOSE_PAREN)
        || is_a(tokens, token, TokenType::OPEN_SQUARE_BRACKET)
        || is_a(tokens, token, TokenType::CLOSE_SQUARE_BRACKET)
        || is_a(tokens, token, TokenType::OPEN_CURLY_BRACKET)
        || is_a(tokens, token, TokenType::CLOSE_CURLY_BRACKET)
        || is_a(tokens, token, TokenType::EOF)
}

/// See [IdentifierContext.formalParameterDeclaration].
///
/// Dart (line 581): `FormalParameterDeclarationIdentifierContext.ensureIdentifier`
pub(crate) fn formal_parameter_declaration_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let mut identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_identifier(identifier) {
        check_async_await_yield_as_identifier(identifier, parser);
        return identifier;
    }

    // Recovery
    let tokens = parser.tokens();
    if ((looks_like_start_of_next_top_level_declaration(tokens, identifier)
        || looks_like_start_of_next_class_member(tokens, identifier)
        || looks_like_statement_start(tokens, identifier))
        && !is_ok_next_value_in_formal_parameter(tokens, tokens.next(identifier)))
        || formal_parameter_is_one_of_following_values(tokens, identifier)
    {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else {
        if !parser.is_keyword_or_identifier(identifier) {
            parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
            // When in doubt, consume the token to ensure we make progress
            // but insert a synthetic identifier to satisfy listeners.
            identifier = parser
                .rewriter()
                .insert_synthetic_identifier(identifier, "");
        } else {
            // Use the keyword as the identifier.
            parser.report_recoverable_error_with_token(
                identifier,
                diag::expected_identifier_but_got_keyword,
            );
        }
    }
    identifier
}

// -----------------------------------------------------------------------------
// RecordFieldDeclarationIdentifierContext

/// See [IdentifierContext.recordFieldDeclaration].
/// TODO(jensj): Initially this is just a copy of
/// FormalParameterDeclarationIdentifierContext. This should be updated
/// to better fit the specific use case.
///
/// Dart (line 630): `RecordFieldDeclarationIdentifierContext.ensureIdentifier`
pub(crate) fn record_field_declaration_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let mut identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_identifier(identifier) {
        check_async_await_yield_as_identifier(identifier, parser);
        return identifier;
    }

    // Recovery
    let tokens = parser.tokens();
    if ((looks_like_start_of_next_top_level_declaration(tokens, identifier)
        || looks_like_start_of_next_class_member(tokens, identifier)
        || looks_like_statement_start(tokens, identifier))
        && !is_ok_next_value_in_formal_parameter(tokens, tokens.next(identifier)))
        || is_a(tokens, identifier, TokenType::COLON)
        || is_a(tokens, identifier, TokenType::EQ)
        || is_a(tokens, identifier, TokenType::COMMA)
        || is_a(tokens, identifier, TokenType::OPEN_PAREN)
        || is_a(tokens, identifier, TokenType::CLOSE_PAREN)
        || is_a(tokens, identifier, TokenType::OPEN_SQUARE_BRACKET)
        || is_a(tokens, identifier, TokenType::CLOSE_SQUARE_BRACKET)
        || is_a(tokens, identifier, TokenType::OPEN_CURLY_BRACKET)
        || is_a(tokens, identifier, TokenType::CLOSE_CURLY_BRACKET)
        || is_a(tokens, identifier, TokenType::EOF)
    {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else {
        if !parser.is_keyword_or_identifier(identifier) {
            parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
            // When in doubt, consume the token to ensure we make progress
            // but insert a synthetic identifier to satisfy listeners.
            identifier = parser
                .rewriter()
                .insert_synthetic_identifier(identifier, "");
        } else {
            // Use the keyword as the identifier.
            parser.report_recoverable_error_with_token(
                identifier,
                diag::expected_identifier_but_got_keyword,
            );
        }
    }
    identifier
}

// -----------------------------------------------------------------------------
// ImportPrefixIdentifierContext

/// Dart (line 688): `ImportPrefixIdentifierContext._isOneOfFollowingValues`
fn import_prefix_is_one_of_following_values(tokens: &Tokens, token: TokenId) -> bool {
    is_a(tokens, token, TokenType::SEMICOLON)
        || is_a(tokens, token, Keyword::IF)
        || is_a(tokens, token, Keyword::SHOW)
        || is_a(tokens, token, Keyword::HIDE)
        || is_a(tokens, token, Keyword::DEFERRED)
        || is_a(tokens, token, Keyword::AS)
        || is_a(tokens, token, TokenType::EOF)
}

/// See [IdentifierContext.importPrefixDeclaration].
///
/// Dart (line 699): `ImportPrefixIdentifierContext.ensureIdentifier`
pub(crate) fn import_prefix_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let mut identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_pseudo(identifier) {
        return identifier;
    }

    // Recovery
    let tokens = parser.tokens();
    let identifier_next = tokens.next(identifier);
    if parser.is_built_in(identifier)
        && import_prefix_is_one_of_following_values(tokens, identifier_next)
    {
        parser.report_recoverable_error_with_token(
            identifier,
            diag::built_in_identifier_in_declaration,
        );
    } else if looks_like_start_of_next_top_level_declaration(tokens, identifier)
        && (identifier_next.is_none()
            || !import_prefix_is_one_of_following_values(tokens, identifier_next))
    {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else if import_prefix_is_one_of_following_values(tokens, identifier) {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else {
        if !parser.is_keyword_or_identifier(identifier) {
            parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
            // When in doubt, consume the token to ensure we make progress
            // but insert a synthetic identifier to satisfy listeners.
            identifier = parser
                .rewriter()
                .insert_synthetic_identifier(identifier, "");
        } else {
            // Use the keyword as the identifier.
            parser.report_recoverable_error_with_token(
                identifier,
                diag::expected_identifier_but_got_keyword,
            );
        }
    }
    identifier
}

// -----------------------------------------------------------------------------
// LiteralSymbolIdentifierContext

/// Dart (line 756): `LiteralSymbolIdentifierContext.ensureIdentifier`
pub(crate) fn literal_symbol_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let mut identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_identifier(identifier) {
        return identifier;
    }

    // Recovery
    if !parser.is_keyword_or_identifier(identifier) {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else {
        // Use the keyword as the identifier.
        parser.report_recoverable_error_with_token(
            identifier,
            diag::expected_identifier_but_got_keyword,
        );
    }

    identifier
}

// -----------------------------------------------------------------------------
// LocalFunctionDeclarationIdentifierContext

/// See [IdentifierContext.localFunctionDeclaration]
/// and [IdentifierContext.localFunctionDeclarationContinuation].
///
/// Dart (line 796): `LocalFunctionDeclarationIdentifierContext.ensureIdentifier`
pub(crate) fn local_function_declaration_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let mut identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_identifier(identifier) {
        check_async_await_yield_as_identifier(identifier, parser);
        return identifier;
    }

    // Recovery
    if parser.is_a(identifier, TokenType::PERIOD)
        || parser.is_a(identifier, TokenType::OPEN_PAREN)
        || parser.is_a(identifier, TokenType::OPEN_CURLY_BRACKET)
        || parser.is_a(identifier, TokenType::FUNCTION)
        || parser.is_a(identifier, TokenType::EOF)
        || looks_like_statement_start(parser.tokens(), identifier)
    {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else {
        if !parser.is_keyword_or_identifier(identifier) {
            parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
            // When in doubt, consume the token to ensure we make progress
            // but insert a synthetic identifier to satisfy listeners.
            identifier = parser
                .rewriter()
                .insert_synthetic_identifier(identifier, "");
        } else {
            // Use the keyword as the identifier.
            parser.report_recoverable_error_with_token(
                identifier,
                diag::expected_identifier_but_got_keyword,
            );
        }
    }
    identifier
}

// -----------------------------------------------------------------------------
// LabelDeclarationIdentifierContext

/// See [IdentifierContext.labelDeclaration].
///
/// Dart (line 843): `LabelDeclarationIdentifierContext.ensureIdentifier`
pub(crate) fn label_declaration_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let mut identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_identifier(identifier) {
        check_async_await_yield_as_identifier(identifier, parser);
        return identifier;
    }

    // Recovery
    if parser.is_a(identifier, TokenType::COLON)
        || parser.is_a(identifier, TokenType::EOF)
        || looks_like_statement_start(parser.tokens(), identifier)
    {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else {
        if !parser.is_keyword_or_identifier(identifier) {
            parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
            // When in doubt, consume the token to ensure we make progress
            // but insert a synthetic identifier to satisfy listeners.
            identifier = parser
                .rewriter()
                .insert_synthetic_identifier(identifier, "");
        } else {
            // Use the keyword as the identifier.
            parser.report_recoverable_error_with_token(
                identifier,
                diag::expected_identifier_but_got_keyword,
            );
        }
    }
    identifier
}

// -----------------------------------------------------------------------------
// LabelReferenceIdentifierContext

/// See [IdentifierContext.labelReference].
///
/// Dart (line 886): `LabelReferenceIdentifierContext.ensureIdentifier`
pub(crate) fn label_reference_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let mut identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_identifier(identifier) {
        check_async_await_yield_as_identifier(identifier, parser);
        return identifier;
    }

    // Recovery
    if parser.is_a(identifier, TokenType::SEMICOLON) || parser.is_a(identifier, TokenType::EOF) {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else {
        if !parser.is_keyword_or_identifier(identifier) {
            parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
            // When in doubt, consume the token to ensure we make progress
            // but insert a synthetic identifier to satisfy listeners.
            identifier = parser
                .rewriter()
                .insert_synthetic_identifier(identifier, "");
        } else {
            // Use the keyword as the identifier.
            parser.report_recoverable_error_with_token(
                identifier,
                diag::expected_identifier_but_got_keyword,
            );
        }
    }
    identifier
}

// -----------------------------------------------------------------------------
// LibraryIdentifierContext

/// Dart (line 947): `LibraryIdentifierContext._isOneOfFollowingValues`
fn library_is_one_of_following_values(tokens: &Tokens, token: TokenId) -> bool {
    is_a(tokens, token, TokenType::PERIOD)
        || is_a(tokens, token, TokenType::SEMICOLON)
        || is_a(tokens, token, TokenType::EOF)
}

/// See [IdentifierContext.libraryName],
/// and [IdentifierContext.libraryNameContinuation]
/// and [IdentifierContext.partName],
/// and [IdentifierContext.partNameContinuation].
///
/// Dart (line 954): `LibraryIdentifierContext.ensureIdentifier`
pub(crate) fn library_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let mut identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);

    if parser.is_identifier(identifier) {
        let tokens = parser.tokens();
        let next = tokens.next(identifier);
        if !looks_like_start_of_next_top_level_declaration(tokens, identifier)
            || library_is_one_of_following_values(tokens, next)
        {
            return identifier;
        }
        // Although this is a valid library name, the library declaration
        // is invalid and this looks like the start of the next declaration.
        // In this situation, fall through to insert a synthetic library name.
    }

    // Recovery
    let tokens = parser.tokens();
    let identifier_next = tokens.next(identifier);
    if library_is_one_of_following_values(tokens, identifier) {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else if looks_like_start_of_next_top_level_declaration(tokens, identifier)
        && (identifier_next.is_none()
            || !library_is_one_of_following_values(tokens, identifier_next))
    {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else {
        if !parser.is_keyword_or_identifier(identifier) {
            parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
            // When in doubt, consume the token to ensure we make progress
            // but insert a synthetic identifier to satisfy listeners.
            identifier = parser
                .rewriter()
                .insert_synthetic_identifier(identifier, "");
        } else {
            // Use the keyword as the identifier.
            parser.report_recoverable_error_with_token(
                identifier,
                diag::expected_identifier_but_got_keyword,
            );
        }
    }
    identifier
}

// -----------------------------------------------------------------------------
// LocalVariableDeclarationIdentifierContext

/// See [IdentifierContext.localVariableDeclaration].
///
/// Dart (line 1011): `LocalVariableDeclarationIdentifierContext.ensureIdentifier`
pub(crate) fn local_variable_declaration_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let mut identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_identifier(identifier) {
        check_async_await_yield_as_identifier(identifier, parser);
        return identifier;
    }

    // Recovery
    if parser.is_a(identifier, TokenType::SEMICOLON)
        || parser.is_a(identifier, TokenType::EQ)
        || parser.is_a(identifier, TokenType::COMMA)
        || parser.is_a(identifier, TokenType::OPEN_CURLY_BRACKET)
        || parser.is_a(identifier, TokenType::CLOSE_CURLY_BRACKET)
        || parser.is_a(identifier, TokenType::EOF)
        || looks_like_statement_start(parser.tokens(), identifier)
        || parser.kind(identifier) == STRING_TOKEN
    {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else {
        if !parser.is_keyword_or_identifier(identifier) {
            parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
            // When in doubt, consume the token to ensure we make progress
            // but insert a synthetic identifier to satisfy listeners.
            identifier = parser
                .rewriter()
                .insert_synthetic_identifier(identifier, "");
        } else {
            // Use the keyword as the identifier.
            parser.report_recoverable_error_with_token(
                identifier,
                diag::expected_identifier_but_got_keyword,
            );
        }
    }
    identifier
}

// -----------------------------------------------------------------------------
// MetadataReferenceIdentifierContext

/// See [IdentifierContext.metadataReference]
/// and [IdentifierContext.metadataContinuation]
/// and [IdentifierContext.metadataContinuationAfterTypeArguments].
///
/// Dart (line 1068): `MetadataReferenceIdentifierContext.ensureIdentifier`
pub(crate) fn metadata_reference_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let mut identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_identifier(identifier) {
        check_async_await_yield_as_identifier(identifier, parser);
        return identifier;
    }

    // Recovery
    let tokens = parser.tokens();
    if is_a(tokens, identifier, TokenType::OPEN_CURLY_BRACKET)
        || is_a(tokens, identifier, TokenType::CLOSE_CURLY_BRACKET)
        || is_a(tokens, identifier, TokenType::OPEN_PAREN)
        || is_a(tokens, identifier, TokenType::CLOSE_PAREN)
        || is_a(tokens, identifier, TokenType::CLOSE_SQUARE_BRACKET)
        || is_a(tokens, identifier, TokenType::EOF)
        || looks_like_start_of_next_top_level_declaration(tokens, identifier)
        || looks_like_start_of_next_class_member(tokens, identifier)
        || looks_like_statement_start(tokens, identifier)
    {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else {
        if !parser.is_keyword_or_identifier(identifier) {
            parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
            // When in doubt, consume the token to ensure we make progress
            // but insert a synthetic identifier to satisfy listeners.
            identifier = parser
                .rewriter()
                .insert_synthetic_identifier(identifier, "");
        } else {
            // Use the keyword as the identifier.
            parser.report_recoverable_error_with_token(
                identifier,
                diag::expected_identifier_but_got_keyword,
            );
        }
    }
    identifier
}

// -----------------------------------------------------------------------------
// MethodDeclarationIdentifierContext

/// See [IdentifierContext.methodDeclaration],
/// and [IdentifierContext.methodDeclarationContinuation],
/// and [IdentifierContext.operatorName],
/// and [IdentifierContext.primaryConstructorDeclaration].
///
/// Dart (line 1144): `MethodDeclarationIdentifierContext.ensureIdentifier`
pub(crate) fn method_declaration_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_identifier(identifier) {
        return identifier;
    }

    // Recovery
    if parser.is_user_definable_operator(identifier) && !context.is_continuation() {
        parser.insert_synthetic_identifier(
            identifier,
            context,
            Some(diag::missing_operator_keyword()),
            Some(identifier),
        )
    } else if parser.is_a(identifier, TokenType::PERIOD)
        || parser.is_a(identifier, TokenType::OPEN_PAREN)
        || parser.is_a(identifier, TokenType::OPEN_CURLY_BRACKET)
        || parser.is_a(identifier, TokenType::FUNCTION)
        || parser.is_a(identifier, TokenType::CLOSE_CURLY_BRACKET)
        || parser.is_a(identifier, TokenType::EOF)
        || looks_like_start_of_next_class_member(parser.tokens(), identifier)
    {
        parser.insert_synthetic_identifier(token, context, None, None)
    } else if !parser.is_keyword_or_identifier(identifier) {
        // When in doubt, consume the token to ensure we make progress
        // but insert a synthetic identifier to satisfy listeners.
        let message = diag::expected_identifier(parser.lexeme(identifier));
        parser.insert_synthetic_identifier(identifier, context, Some(message), Some(identifier))
    } else {
        // Use the keyword as the identifier.
        parser.report_recoverable_error_with_token(
            identifier,
            diag::expected_identifier_but_got_keyword,
        );
        identifier
    }
}

/// Dart (line 1187): `MethodDeclarationIdentifierContext.ensureIdentifierPotentiallyRecovered`
pub(crate) fn method_declaration_ensure_identifier_potentially_recovered<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
    is_recovered: bool,
) -> TokenId {
    // Fast path good case.
    let identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_identifier(identifier) {
        return identifier;
    }
    // If not recovered, recover as normal.
    if !is_recovered || !parser.is_keyword_or_identifier(identifier) {
        return method_declaration_ensure_identifier(context, token, parser);
    }

    // If already recovered, use the given token.
    parser
        .report_recoverable_error_with_token(identifier, diag::expected_identifier_but_got_keyword);
    identifier
}

// -----------------------------------------------------------------------------
// NamedArgumentReferenceIdentifierContext

/// See [IdentifierContext.namedArgumentReference].
///
/// Dart (line 1218): `NamedArgumentReferenceIdentifierContext.ensureIdentifier`
pub(crate) fn named_argument_reference_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let mut identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_identifier(identifier) {
        check_async_await_yield_as_identifier(identifier, parser);
        return identifier;
    }

    // Recovery
    if parser.is_a(identifier, TokenType::COLON) || parser.is_a(identifier, TokenType::EOF) {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else {
        if !parser.is_keyword_or_identifier(identifier) {
            parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
            // When in doubt, consume the token to ensure we make progress
            // but insert a synthetic identifier to satisfy listeners.
            identifier = parser
                .rewriter()
                .insert_synthetic_identifier(identifier, "");
        } else {
            // Use the keyword as the identifier.
            parser.report_recoverable_error_with_token(
                identifier,
                diag::expected_identifier_but_got_keyword,
            );
        }
    }
    identifier
}

// -----------------------------------------------------------------------------
// NamedRecordFieldReferenceIdentifierContext

/// See [IdentifierContext.namedRecordFieldReference].
/// TODO(jensj): Initially this is just a copy of
/// NamedArgumentReferenceIdentifierContext. This should be updated
/// to better fit the specific use case.
///
/// Dart (line 1263): `NamedRecordFieldReferenceIdentifierContext.ensureIdentifier`
pub(crate) fn named_record_field_reference_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let mut identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_identifier(identifier) {
        check_async_await_yield_as_identifier(identifier, parser);
        return identifier;
    }

    // Recovery
    if parser.is_a(identifier, TokenType::COLON) || parser.is_a(identifier, TokenType::EOF) {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else {
        if !parser.is_keyword_or_identifier(identifier) {
            parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
            // When in doubt, consume the token to ensure we make progress
            // but insert a synthetic identifier to satisfy listeners.
            identifier = parser
                .rewriter()
                .insert_synthetic_identifier(identifier, "");
        } else {
            // Use the keyword as the identifier.
            parser.report_recoverable_error_with_token(
                identifier,
                diag::expected_identifier_but_got_keyword,
            );
        }
    }
    identifier
}

// -----------------------------------------------------------------------------
// TopLevelDeclarationIdentifierContext

/// Dart (line 141): the `followingValues` of
/// `IdentifierContext.topLevelVariableDeclaration`.
const TOP_LEVEL_VARIABLE_DECLARATION_FOLLOWING_VALUES: &[TokenType] = &[
    TokenType::SEMICOLON,
    TokenType::EQ,
    TokenType::COMMA,
    TokenType::EOF,
];

/// Dart (line 152): the `followingValues` of
/// `IdentifierContext.topLevelFunctionDeclaration`.
const TOP_LEVEL_FUNCTION_DECLARATION_FOLLOWING_VALUES: &[TokenType] = &[
    TokenType::LT,
    TokenType::OPEN_PAREN,
    TokenType::OPEN_CURLY_BRACKET,
    TokenType::FUNCTION,
    Keyword::ASYNC,
    Keyword::SYNC,
    TokenType::EOF,
];

/// Dart `TopLevelDeclarationIdentifierContext.followingValues` (a constructor
/// argument per constant).
fn top_level_declaration_following_values(context: IdentifierContext) -> &'static [TokenType] {
    match context {
        IdentifierContext::TopLevelVariableDeclaration => {
            TOP_LEVEL_VARIABLE_DECLARATION_FOLLOWING_VALUES
        }
        IdentifierContext::TopLevelFunctionDeclaration => {
            TOP_LEVEL_FUNCTION_DECLARATION_FOLLOWING_VALUES
        }
        _ => panic!(
            "Internal error: {} is not a TopLevelDeclarationIdentifierContext",
            context.name()
        ),
    }
}

/// See [IdentifierContext.topLevelFunctionDeclaration]
/// and [IdentifierContext.topLevelVariableDeclaration].
///
/// Dart (line 1308): `TopLevelDeclarationIdentifierContext.ensureIdentifier`
pub(crate) fn top_level_declaration_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let following_values = top_level_declaration_following_values(context);
    let mut identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);

    if parser.is_identifier(identifier) {
        let tokens = parser.tokens();
        let next = tokens.next(identifier);
        if !looks_like_start_of_next_top_level_declaration(tokens, identifier)
            || is_any_of(tokens, next, following_values)
        {
            return identifier;
        }
        // Although this is a valid top level name, the declaration
        // is invalid and this looks like the start of the next declaration.
        // In this situation, fall through to insert a synthetic name.
    }

    // Recovery
    let tokens = parser.tokens();
    if looks_like_start_of_next_top_level_declaration(tokens, identifier)
        || is_any_of(tokens, identifier, following_values)
    {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else if parser.is_built_in(identifier) {
        parser.report_recoverable_error_with_token(
            identifier,
            diag::built_in_identifier_in_declaration,
        );
    } else {
        if !parser.is_keyword_or_identifier(identifier) {
            parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
            // When in doubt, consume the token to ensure we make progress
            // but insert a synthetic identifier to satisfy listeners.
            identifier = parser
                .rewriter()
                .insert_synthetic_identifier(identifier, "");
        } else {
            // Use the keyword as the identifier.
            parser.report_recoverable_error_with_token(
                identifier,
                diag::expected_identifier_but_got_keyword,
            );
        }
    }
    identifier
}

/// Dart (line 1357): `TopLevelDeclarationIdentifierContext.ensureIdentifierPotentiallyRecovered`
pub(crate) fn top_level_declaration_ensure_identifier_potentially_recovered<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
    is_recovered: bool,
) -> TokenId {
    let following_values = top_level_declaration_following_values(context);
    // Fast path good case.
    let identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);

    if parser.is_identifier(identifier) {
        let tokens = parser.tokens();
        let next = tokens.next(identifier);
        if !looks_like_start_of_next_top_level_declaration(tokens, identifier)
            || is_any_of(tokens, next, following_values)
        {
            return identifier;
        }
    }
    // If not recovered, recover as normal.
    if !is_recovered || !parser.is_keyword_or_identifier(identifier) {
        return top_level_declaration_ensure_identifier(context, token, parser);
    }

    // If already recovered, use the given token.
    parser
        .report_recoverable_error_with_token(identifier, diag::expected_identifier_but_got_keyword);
    identifier
}

// -----------------------------------------------------------------------------
// TypedefDeclarationIdentifierContext

/// Dart (line 1396): `TypedefDeclarationIdentifierContext._isOneOfFollowingValues`
fn typedef_is_one_of_following_values(tokens: &Tokens, token: TokenId) -> bool {
    is_a(tokens, token, TokenType::OPEN_PAREN)
        || is_a(tokens, token, TokenType::LT)
        || is_a(tokens, token, TokenType::EQ)
        || is_a(tokens, token, TokenType::SEMICOLON)
        || is_a(tokens, token, TokenType::EOF)
}

/// See [IdentifierContext.typedefDeclaration].
///
/// Dart (line 1405): `TypedefDeclarationIdentifierContext.ensureIdentifier`
pub(crate) fn typedef_declaration_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let mut identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_pseudo(identifier) {
        if parser.is_a(identifier, Keyword::FUNCTION) {
            parser.report_recoverable_error_with_token(
                identifier,
                diag::expected_identifier_but_got_keyword,
            );
        }
        return identifier;
    }

    // Recovery
    let tokens = parser.tokens();
    if parser.is_built_in(identifier)
        && typedef_is_one_of_following_values(tokens, tokens.next(identifier))
    {
        parser.report_recoverable_error_with_token(
            identifier,
            diag::built_in_identifier_in_declaration,
        );
    } else if looks_like_start_of_next_top_level_declaration(tokens, identifier)
        || typedef_is_one_of_following_values(tokens, identifier)
    {
        identifier = insert_synthetic_identifier_expected(parser, token, context, identifier);
    } else {
        if !parser.is_keyword_or_identifier(identifier) {
            parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
            // When in doubt, consume the token to ensure we make progress
            // but insert a synthetic identifier to satisfy listeners.
            identifier = parser
                .rewriter()
                .insert_synthetic_identifier(identifier, "");
        } else {
            // Use the keyword as the identifier.
            parser.report_recoverable_error_with_token(
                identifier,
                diag::expected_identifier_but_got_keyword,
            );
        }
    }
    identifier
}

/// Dart (line 1453): `TypedefDeclarationIdentifierContext.ensureIdentifierPotentiallyRecovered`
pub(crate) fn typedef_declaration_ensure_identifier_potentially_recovered<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
    is_recovered: bool,
) -> TokenId {
    // Fast path good case.
    let identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_pseudo(identifier) {
        if parser.is_a(identifier, Keyword::FUNCTION) {
            parser.report_recoverable_error_with_token(
                identifier,
                diag::expected_identifier_but_got_keyword,
            );
        }
        return identifier;
    }

    // If not recovered, recover as normal.
    if !is_recovered || !parser.is_keyword_or_identifier(identifier) {
        return typedef_declaration_ensure_identifier(context, token, parser);
    }

    // If already recovered, use the given token.
    parser
        .report_recoverable_error_with_token(identifier, diag::expected_identifier_but_got_keyword);
    identifier
}

// -----------------------------------------------------------------------------
// TypeReferenceIdentifierContext

/// See [IdentifierContext.typeReference].
///
/// Dart (line 1511): `TypeReferenceIdentifierContext.ensureIdentifier`
pub(crate) fn type_reference_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    mut token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let next = parser.next(token);
    // assert(next.kind != IDENTIFIER_TOKEN);
    if is_valid_non_record_type_reference(parser.tokens(), next) {
        return next;
    } else if parser.is_keyword_or_identifier(next) {
        if parser.is_a(next, Keyword::VOID) {
            parser.report_recoverable_error(next, diag::invalid_void());
        } else if parser.is_built_in(next) {
            if !context.is_built_in_identifier_allowed() {
                parser.report_recoverable_error_with_token(next, diag::built_in_identifier_as_type);
            }
        } else if parser.is_a(next, Keyword::VAR) {
            parser.report_recoverable_error(next, diag::var_as_type_name());
        } else {
            parser.report_recoverable_error_with_token(next, diag::expected_type);
        }
        return next;
    }
    parser.report_recoverable_error_with_token(next, diag::expected_type);
    if !(parser.is_a(next, TokenType::LT)
        || parser.is_a(next, TokenType::GT)
        || parser.is_a(next, TokenType::GT_GT)
        || parser.is_a(next, TokenType::GT_GT_GT)
        || parser.is_a(next, TokenType::CLOSE_PAREN)
        || parser.is_a(next, TokenType::OPEN_SQUARE_BRACKET)
        || parser.is_a(next, TokenType::CLOSE_SQUARE_BRACKET)
        || parser.is_a(next, TokenType::INDEX)
        || parser.is_a(next, TokenType::OPEN_CURLY_BRACKET)
        || parser.is_a(next, TokenType::CLOSE_CURLY_BRACKET)
        || parser.is_a(next, TokenType::COMMA)
        || parser.is_a(next, TokenType::SEMICOLON)
        || parser.is_a(next, TokenType::EOF))
    {
        // When in doubt, consume the token to ensure we make progress
        token = next;
        // next = token.next!; (the value is not used after this)
    }
    // Insert a synthetic identifier to satisfy listeners.
    parser.rewriter().insert_synthetic_identifier(token, "")
}

// -----------------------------------------------------------------------------
// TypeVariableDeclarationIdentifierContext

/// Dart (line 1565): `TypeVariableDeclarationIdentifierContext._isOneOfFollowingValues`
fn type_variable_is_one_of_following_values(tokens: &Tokens, token: TokenId) -> bool {
    is_a(tokens, token, TokenType::LT)
        || is_a(tokens, token, TokenType::GT)
        || is_a(tokens, token, TokenType::GT_GT)
        || is_a(tokens, token, TokenType::GT_GT_GT)
        || is_a(tokens, token, TokenType::SEMICOLON)
        || is_a(tokens, token, TokenType::CLOSE_CURLY_BRACKET)
        || is_a(tokens, token, Keyword::EXTENDS)
        || is_a(tokens, token, Keyword::SUPER)
        // If currently adding type variables to a typedef this could easily
        // occur and we don't want to 'eat' the equal sign.
        || is_a(tokens, token, TokenType::EQ)
        || is_a(tokens, token, TokenType::GT_EQ)
        // Also EOF.
        || is_a(tokens, token, TokenType::EOF)
}

// See [IdentifierContext.typeVariableDeclaration].
/// Dart (line 1583): `TypeVariableDeclarationIdentifierContext.ensureIdentifier`
pub(crate) fn type_variable_declaration_ensure_identifier<L: Listener>(
    context: IdentifierContext,
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let _ = context;
    let mut identifier = parser.next(token);
    // assert(identifier.kind != IDENTIFIER_TOKEN);
    if parser.is_pseudo(identifier) {
        return identifier;
    }

    // Recovery: If the next token  (the one currently in 'identifier') is any
    // of these values we don't "eat" the it but instead insert an identifier
    // between "token" and "token.next" and return that as the last consumed
    // token. Otherwise such a token would be consumed: an identifier would be
    // inserted after "token.next" and that would be returned as the last
    // consumed token, effectively skipping the token.
    let tokens = parser.tokens();
    if looks_like_start_of_next_top_level_declaration(tokens, identifier)
        || looks_like_start_of_next_class_member(tokens, identifier)
        || looks_like_statement_start(tokens, identifier)
        || type_variable_is_one_of_following_values(tokens, identifier)
    {
        parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
        identifier = parser.rewriter().insert_synthetic_identifier(token, "");
    } else if parser.is_built_in(identifier) {
        parser.report_recoverable_error_with_token(
            identifier,
            diag::built_in_identifier_in_declaration,
        );
    } else {
        if !parser.is_keyword_or_identifier(identifier) {
            parser.report_recoverable_error_with_token(identifier, diag::expected_identifier);
            // When in doubt, consume the token to ensure we make progress
            // but insert a synthetic identifier to satisfy listeners.
            identifier = parser
                .rewriter()
                .insert_synthetic_identifier(identifier, "");
        } else {
            // Use the keyword as the identifier.
            parser.report_recoverable_error_with_token(
                identifier,
                diag::expected_identifier_but_got_keyword,
            );
        }
    }
    identifier
}

// -----------------------------------------------------------------------------
// Free functions

/// Dart `checkAsyncAwaitYieldAsIdentifier`.
///
/// Dart (line 1631): `void checkAsyncAwaitYieldAsIdentifier(Token identifier, Parser parser)`
pub fn check_async_await_yield_as_identifier<L: Listener>(
    identifier: TokenId,
    parser: &mut Parser<L>,
) {
    if !parser.in_plain_sync() && parser.is_pseudo(identifier) {
        if parser.is_a(identifier, Keyword::AWAIT) {
            parser.report_recoverable_error(identifier, diag::await_as_identifier());
        } else if parser.is_a(identifier, Keyword::YIELD) {
            parser.report_recoverable_error(identifier, diag::yield_as_identifier());
        }
    }
}

/// Dart `looksLikeStartOfNextClassMember`.
///
/// Dart (line 1641): `bool looksLikeStartOfNextClassMember(Token token)`
pub fn looks_like_start_of_next_class_member(tokens: &Tokens, token: TokenId) -> bool {
    tokens.ty(token).is_modifier()
        || is_a(tokens, token, TokenType::AT)
        || is_a(tokens, token, Keyword::GET)
        || is_a(tokens, token, Keyword::SET)
        || is_a(tokens, token, Keyword::VOID)
        || is_a(tokens, token, TokenType::EOF)
}

/// Dart `looksLikeStartOfNextTopLevelDeclaration`.
///
/// Dart (line 1649): `bool looksLikeStartOfNextTopLevelDeclaration(Token token)`
pub fn looks_like_start_of_next_top_level_declaration(tokens: &Tokens, token: TokenId) -> bool {
    tokens.ty(token).is_top_level_keyword()
        || is_a(tokens, token, Keyword::CONST)
        || is_a(tokens, token, Keyword::GET)
        || is_a(tokens, token, Keyword::FINAL)
        || is_a(tokens, token, Keyword::SET)
        || is_a(tokens, token, Keyword::VAR)
        || is_a(tokens, token, Keyword::VOID)
        || is_a(tokens, token, TokenType::EOF)
}
