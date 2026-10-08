// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/parser_impl.dart (lines 6687-8450)

#![allow(unused_imports, unused_variables, unused_mut, clippy::all)]

use dartr_diagnostics::cfe::{CfeCode, CfeMessage};
use dartr_diagnostics::cfe_codes as diag;
use dartr_syntax::token_constants::*;
use dartr_syntax::{Keyword, TokenId, TokenType, Tokens};

use super::{AwaitOrYieldContext, ConstantPatternContext, ForPartsContext, Parser, PatternContext};
use crate::assert::Assert;
use crate::async_modifier::AsyncModifier;
use crate::block_kind::BlockKind;
use crate::constructor_reference_context::ConstructorReferenceContext;
use crate::declaration_kind::{DeclarationHeaderKind, DeclarationKind};
use crate::directive_context::DirectiveContext;
use crate::experimental_features::ExperimentalFlag;
use crate::formal_parameter_kind::FormalParameterKind;
use crate::identifier_context::IdentifierContext;
use crate::listener::Listener;
use crate::listener_stack::Layer;
use crate::literal_entry_info::LiteralEntryInfo;
use crate::loop_state::LoopState;
use crate::member_kind::MemberKind;
use crate::modifier_context::ModifierContext;
use crate::type_info::{TypeInfo, TypeParamOrArgInfo};

impl<L: Listener> Parser<L> {

    /// Dart (line 6687): `Token parseStatement(Token token)`
    pub fn parse_statement(&mut self, token: TokenId) -> TokenId {
        todo!("parseStatement")
    }

    /// Dart (line 6699): `Token parseStatementX(Token token)`
    pub fn parse_statement_x(&mut self, token: TokenId) -> TokenId {
        todo!("parseStatementX")
    }

    /// Dart (line 6814): `Token parseYieldStatement(Token token)`
    pub fn parse_yield_statement(&mut self, token: TokenId) -> TokenId {
        todo!("parseYieldStatement")
    }

    /// Dart (line 6842): `Token parseReturnStatement(Token token)`
    pub fn parse_return_statement(&mut self, token: TokenId) -> TokenId {
        todo!("parseReturnStatement")
    }

    /// Dart (line 6865): `Token parseLabel(Token token)`
    pub fn parse_label(&mut self, token: TokenId) -> TokenId {
        todo!("parseLabel")
    }

    /// Dart (line 6878): `Token parseLabeledStatement(Token token)`
    pub fn parse_labeled_statement(&mut self, token: TokenId) -> TokenId {
        todo!("parseLabeledStatement")
    }

    /// Dart (line 6904): `Token parseExpressionStatement(Token token)`
    pub fn parse_expression_statement(&mut self, token: TokenId) -> TokenId {
        todo!("parseExpressionStatement")
    }

    /// Dart (line 6917): `Token parseExpression(Token token)`
    pub fn parse_expression(&mut self, token: TokenId) -> TokenId {
        todo!("parseExpression")
    }

    /// Dart (line 6952): `Token parseExpressionWithoutCascade(Token token)`
    pub fn parse_expression_without_cascade(&mut self, token: TokenId) -> TokenId {
        todo!("parseExpressionWithoutCascade")
    }

    /// Dart (line 6957): `Token _parseExpression(Token token, bool allowCascades)`
    pub fn parse_expression_impl(&mut self, token: TokenId, allow_cascades: bool) -> TokenId {
        todo!("_parseExpression")
    }

    /// Dart (line 6973): `bool canParseAsConditional(Token question)`
    pub fn can_parse_as_conditional(&mut self, question: TokenId) -> bool {
        todo!("canParseAsConditional")
    }

    /// Dart (line 7003): `Token parseConditionalExpressionRest(Token token)`
    pub fn parse_conditional_expression_rest(&mut self, token: TokenId) -> TokenId {
        todo!("parseConditionalExpressionRest")
    }

    /// Dart (line 7021): `bool _isDotShorthand(Token token)`
    pub fn is_dot_shorthand(&mut self, token: TokenId) -> bool {
        todo!("_isDotShorthand")
    }

    /// Dart (line 7035): `Token parsePrecedenceExpression( Token token, int precedence, bool allowCascades, ConstantPatternContext constantPatternContext, )`
    pub fn parse_precedence_expression(&mut self, token: TokenId, precedence: i32, allow_cascades: bool, constant_pattern_context: ConstantPatternContext) -> TokenId {
        todo!("parsePrecedenceExpression")
    }

    /// Dart (line 7125): `Token _parseDotShorthand( Token token, Token nextToken, bool isDotShorthand, bool allowCascades, TypeParamOrArgInfo typeArg, ConstantPatternContext constantPatternContext, )`
    pub fn parse_dot_shorthand(&mut self, token: TokenId, next_token: TokenId, is_dot_shorthand: bool, allow_cascades: bool, type_arg: TypeParamOrArgInfo, constant_pattern_context: ConstantPatternContext) -> TokenId {
        todo!("_parseDotShorthand")
    }

    /// Dart (line 7182): `Token _parsePrecedenceExpressionLoop( int precedence, bool allowCascades, TypeParamOrArgInfo typeArg, Token token, ConstantPatternContext constantPatternContext,`
    pub fn parse_precedence_expression_loop(&mut self, precedence: i32, allow_cascades: bool, type_arg: TypeParamOrArgInfo, token: TokenId, constant_pattern_context: ConstantPatternContext, is_dot_shorthand: bool) -> TokenId {
        todo!("_parsePrecedenceExpressionLoop")
    }

    /// Dart (line 7478): `ConstantPatternContext _checkForInvalidConstantPatternOperator( ConstantPatternContext constantPatternContext, int precedence, Token token, Token next, TokenType type, int tokenLevel, )`
    pub fn check_for_invalid_constant_pattern_operator(&mut self, constant_pattern_context: ConstantPatternContext, precedence: i32, token: TokenId, next: TokenId, type_: TokenType, token_level: i32) -> ConstantPatternContext {
        todo!("_checkForInvalidConstantPatternOperator")
    }

    /// Dart (line 7524): `bool _beginsAnonymousMethod(Token token)`
    pub fn begins_anonymous_method(&mut self, token: TokenId) -> bool {
        todo!("_beginsAnonymousMethod")
    }

    /// Dart (line 7550): `Token _parseAnonymousMethod(Token punctuation, Token afterPunctuation)`
    pub fn parse_anonymous_method(&mut self, punctuation: TokenId, after_punctuation: TokenId) -> TokenId {
        todo!("_parseAnonymousMethod")
    }

    /// Dart (line 7594): `bool _attemptPrecedenceLevelRecovery( Token token, int precedence, int currentLevel, bool allowCascades, TypeParamOrArgInfo typeArg, )`
    pub fn attempt_precedence_level_recovery(&mut self, token: TokenId, precedence: i32, current_level: i32, allow_cascades: bool, type_arg: TypeParamOrArgInfo) -> bool {
        todo!("_attemptPrecedenceLevelRecovery")
    }

    /// Dart (line 7707): `int _computePrecedence(Token token, {required bool forPattern})`
    pub fn compute_precedence(&mut self, token: TokenId, for_pattern: bool) -> i32 {
        todo!("_computePrecedence")
    }

    /// Dart (line 7756): `Token parseCascadeExpression(Token token)`
    pub fn parse_cascade_expression(&mut self, token: TokenId) -> TokenId {
        todo!("parseCascadeExpression")
    }

    /// Dart (line 7851): `Token parseUnaryExpression( Token token, bool allowCascades, ConstantPatternContext constantPatternContext, )`
    pub fn parse_unary_expression(&mut self, token: TokenId, allow_cascades: bool, constant_pattern_context: ConstantPatternContext) -> TokenId {
        todo!("parseUnaryExpression")
    }

    /// Dart (line 7974): `Token parseArgumentOrIndexStar( Token token, TypeParamOrArgInfo typeArg, bool checkedNullAware, )`
    pub fn parse_argument_or_index_star(&mut self, token: TokenId, type_arg: TypeParamOrArgInfo, checked_null_aware: bool) -> TokenId {
        todo!("parseArgumentOrIndexStar")
    }

    /// Dart (line 8073): `Token parsePrimary( Token token, IdentifierContext context, ConstantPatternContext constantPatternContext, )`
    pub fn parse_primary(&mut self, token: TokenId, context: IdentifierContext, constant_pattern_context: ConstantPatternContext) -> TokenId {
        todo!("parsePrimary")
    }

    /// Dart (line 8211): `Token parseParenthesizedExpressionFunctionLiteralOrRecordLiteral( Token token, ConstantPatternContext constantPatternContext, )`
    pub fn parse_parenthesized_expression_function_literal_or_record_literal(&mut self, token: TokenId, constant_pattern_context: ConstantPatternContext) -> TokenId {
        todo!("parseParenthesizedExpressionFunctionLiteralOrRecordLiteral")
    }

    /// Dart (line 8258): `Token ensureParenthesizedCondition(Token token, {required bool allowCase})`
    pub fn ensure_parenthesized_condition(&mut self, token: TokenId, allow_case: bool) -> TokenId {
        todo!("ensureParenthesizedCondition")
    }

    /// Dart (line 8275): `Token parseParenthesizedExpressionOrRecordLiteral( Token token, Token? constKeywordForRecord, ConstantPatternContext constantPatternContext, )`
    pub fn parse_parenthesized_expression_or_record_literal(&mut self, token: TokenId, const_keyword_for_record: Option<TokenId>, constant_pattern_context: ConstantPatternContext) -> TokenId {
        todo!("parseParenthesizedExpressionOrRecordLiteral")
    }

    /// Dart (line 8376): `Token parseExpressionInParenthesisRest( Token token,`
    pub fn parse_expression_in_parenthesis_rest(&mut self, token: TokenId, allow_case: bool) -> TokenId {
        todo!("parseExpressionInParenthesisRest")
    }

    /// Dart (line 8409): `Token parseThisExpression(Token token, IdentifierContext context)`
    pub fn parse_this_expression(&mut self, token: TokenId, context: IdentifierContext) -> TokenId {
        todo!("parseThisExpression")
    }

    /// Dart (line 8423): `Token parseSuperExpression(Token token, IdentifierContext context)`
    pub fn parse_super_expression(&mut self, token: TokenId, context: IdentifierContext) -> TokenId {
        todo!("parseSuperExpression")
    }

}
