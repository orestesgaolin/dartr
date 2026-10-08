// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/parser_impl.dart (lines 10490-12597)

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

    /// Dart (line 10490): `Token parseBlock(Token token, BlockKind blockKind)`
    pub fn parse_block(&mut self, token: TokenId, block_kind: BlockKind) -> TokenId {
        todo!("parseBlock")
    }

    /// Dart (line 10515): `Token parseInvalidBlock(Token token)`
    pub fn parse_invalid_block(&mut self, token: TokenId) -> TokenId {
        todo!("parseInvalidBlock")
    }

    /// Dart (line 10531): `bool looksLikeExpressionAfterAwaitOrYield( Token token, AwaitOrYieldContext context, )`
    pub fn looks_like_expression_after_await_or_yield(&mut self, token: TokenId, context: AwaitOrYieldContext) -> bool {
        todo!("looksLikeExpressionAfterAwaitOrYield")
    }

    /// Dart (line 10598): `bool looksLikeAwaitExpression(Token token, AwaitOrYieldContext context)`
    pub fn looks_like_await_expression(&mut self, token: TokenId, context: AwaitOrYieldContext) -> bool {
        todo!("looksLikeAwaitExpression")
    }

    /// Dart (line 10607): `bool looksLikeYieldStatement(Token token, AwaitOrYieldContext context)`
    pub fn looks_like_yield_statement(&mut self, token: TokenId, context: AwaitOrYieldContext) -> bool {
        todo!("looksLikeYieldStatement")
    }

    /// Dart (line 10619): `Token parseAwaitExpression(Token token, bool allowCascades)`
    pub fn parse_await_expression(&mut self, token: TokenId, allow_cascades: bool) -> TokenId {
        todo!("parseAwaitExpression")
    }

    /// Dart (line 10648): `Token parseThrowExpression(Token token, bool allowCascades)`
    pub fn parse_throw_expression(&mut self, token: TokenId, allow_cascades: bool) -> TokenId {
        todo!("parseThrowExpression")
    }

    /// Dart (line 10678): `Token parseRethrowStatement(Token token)`
    pub fn parse_rethrow_statement(&mut self, token: TokenId) -> TokenId {
        todo!("parseRethrowStatement")
    }

    /// Dart (line 10705): `Token parseTryStatement(Token token)`
    pub fn parse_try_statement(&mut self, token: TokenId) -> TokenId {
        todo!("parseTryStatement")
    }

    /// Dart (line 10856): `Token parseSwitchStatement(Token token)`
    pub fn parse_switch_statement(&mut self, token: TokenId) -> TokenId {
        todo!("parseSwitchStatement")
    }

    /// Dart (line 10876): `Token parseSwitchBlock(Token token)`
    pub fn parse_switch_block(&mut self, token: TokenId) -> TokenId {
        todo!("parseSwitchBlock")
    }

    /// Dart (line 10967): `Token peekPastLabels(Token token)`
    pub fn peek_past_labels(&mut self, token: TokenId) -> TokenId {
        todo!("peekPastLabels")
    }

    /// Dart (line 10975): `Token parseStatementsInSwitchCase( Token token, Token peek, Token begin, int labelCount, int expressionCount, Token? defaultKeyword, Token? colonAfterDefault, )`
    pub fn parse_statements_in_switch_case(&mut self, token: TokenId, peek: TokenId, begin: TokenId, label_count: i32, expression_count: i32, default_keyword: Option<TokenId>, colon_after_default: Option<TokenId>) -> TokenId {
        todo!("parseStatementsInSwitchCase")
    }

    /// Dart (line 11028): `Token parseBreakStatement(Token token)`
    pub fn parse_break_statement(&mut self, token: TokenId) -> TokenId {
        todo!("parseBreakStatement")
    }

    /// Dart (line 11048): `Token parseAssert(Token token, Assert kind)`
    pub fn parse_assert(&mut self, token: TokenId, kind: Assert) -> TokenId {
        todo!("parseAssert")
    }

    /// Dart (line 11113): `Token parseAssertStatement(Token token)`
    pub fn parse_assert_statement(&mut self, token: TokenId) -> TokenId {
        todo!("parseAssertStatement")
    }

    /// Dart (line 11124): `Token parseContinueStatement(Token token)`
    pub fn parse_continue_statement(&mut self, token: TokenId) -> TokenId {
        todo!("parseContinueStatement")
    }

    /// Dart (line 11152): `Token parseEmptyStatement(Token token)`
    pub fn parse_empty_statement(&mut self, token: TokenId) -> TokenId {
        todo!("parseEmptyStatement")
    }

    /// Dart (line 11173): `Token parseInvalidOperatorDeclaration( Token beforeStart, Token? abstractToken, Token? augmentToken, Token? externalToken, Token? staticToken, Token? covariantToken, Token? lateToken, Token? varFinalOrConst, Token beforeType, DeclarationKind kind, String? enclosingDeclarationName, )`
    pub fn parse_invalid_operator_declaration(&mut self, before_start: TokenId, abstract_token: Option<TokenId>, augment_token: Option<TokenId>, external_token: Option<TokenId>, static_token: Option<TokenId>, covariant_token: Option<TokenId>, late_token: Option<TokenId>, var_final_or_const: Option<TokenId>, before_type: TokenId, kind: DeclarationKind, enclosing_declaration_name: Option<&str>) -> TokenId {
        todo!("parseInvalidOperatorDeclaration")
    }

    /// Dart (line 11256): `Token recoverFromInvalidMember( Token token, Token beforeStart, Token? abstractToken, Token? augmentToken, Token? externalToken, Token? staticToken, Token? covariantToken, Token? lateToken, Token? varFinalOrConst, Token beforeType, TypeInfo typeInfo, Token? getOrSet, Token? newToken, DeclarationKind kind, String? enclosingDeclarationName, )`
    pub fn recover_from_invalid_member(&mut self, token: TokenId, before_start: TokenId, abstract_token: Option<TokenId>, augment_token: Option<TokenId>, external_token: Option<TokenId>, static_token: Option<TokenId>, covariant_token: Option<TokenId>, late_token: Option<TokenId>, var_final_or_const: Option<TokenId>, before_type: TokenId, type_info: &TypeInfo, get_or_set: Option<TokenId>, new_token: Option<TokenId>, kind: DeclarationKind, enclosing_declaration_name: Option<&str>) -> TokenId {
        todo!("recoverFromInvalidMember")
    }

    /// Dart (line 11353): `Token recoverFromStackOverflow(Token token)`
    pub fn recover_from_stack_overflow(&mut self, token: TokenId) -> TokenId {
        todo!("recoverFromStackOverflow")
    }

    /// Dart (line 11416): `Token parseInvalidTopLevelDeclaration(Token token)`
    pub fn parse_invalid_top_level_declaration(&mut self, token: TokenId) -> TokenId {
        todo!("parseInvalidTopLevelDeclaration")
    }

    /// Dart (line 11431): `Token reportAndSkipClassInClass(Token token)`
    pub fn report_and_skip_class_in_class(&mut self, token: TokenId) -> TokenId {
        todo!("reportAndSkipClassInClass")
    }

    /// Dart (line 11454): `Token reportAndSkipEnumInClass(Token token)`
    pub fn report_and_skip_enum_in_class(&mut self, token: TokenId) -> TokenId {
        todo!("reportAndSkipEnumInClass")
    }

    /// Dart (line 11477): `Token reportAndSkipTypedefInClass(Token token)`
    pub fn report_and_skip_typedef_in_class(&mut self, token: TokenId) -> TokenId {
        todo!("reportAndSkipTypedefInClass")
    }

    /// Dart (line 11549): `Token parsePattern( Token token, PatternContext patternContext,`
    pub fn parse_pattern(&mut self, token: TokenId, pattern_context: PatternContext, precedence: i32) -> TokenId {
        todo!("parsePattern")
    }

    /// Dart (line 11658): `Token parsePrimaryPattern(Token token, PatternContext patternContext)`
    pub fn parse_primary_pattern(&mut self, token: TokenId, pattern_context: PatternContext) -> TokenId {
        todo!("parsePrimaryPattern")
    }

    /// Dart (line 11869): `Token parseVariablePattern( Token token, PatternContext patternContext,`
    pub fn parse_variable_pattern(&mut self, token: TokenId, pattern_context: PatternContext, type_info: &TypeInfo) -> TokenId {
        todo!("parseVariablePattern")
    }

    /// Dart (line 11971): `Token parseListPatternSuffix(Token token, PatternContext patternContext)`
    pub fn parse_list_pattern_suffix(&mut self, token: TokenId, pattern_context: PatternContext) -> TokenId {
        todo!("parseListPatternSuffix")
    }

    /// Dart (line 12054): `Token parseMapPatternSuffix(Token token, PatternContext patternContext)`
    pub fn parse_map_pattern_suffix(&mut self, token: TokenId, pattern_context: PatternContext) -> TokenId {
        todo!("parseMapPatternSuffix")
    }

    /// Dart (line 12143): `Token parseParenthesizedPatternOrRecordPattern( Token token, PatternContext patternContext, )`
    pub fn parse_parenthesized_pattern_or_record_pattern(&mut self, token: TokenId, pattern_context: PatternContext) -> TokenId {
        todo!("parseParenthesizedPatternOrRecordPattern")
    }

    /// Dart (line 12223): `Token parseObjectPatternRest(Token token, PatternContext patternContext)`
    pub fn parse_object_pattern_rest(&mut self, token: TokenId, pattern_context: PatternContext) -> TokenId {
        todo!("parseObjectPatternRest")
    }

    /// Dart (line 12286): `bool looksLikeOuterPatternEquals(Token token)`
    pub fn looks_like_outer_pattern_equals(&mut self, token: TokenId) -> bool {
        todo!("looksLikeOuterPatternEquals")
    }

    /// Dart (line 12300): `Token? skipOuterPattern(Token token)`
    pub fn skip_outer_pattern(&mut self, token: TokenId) -> Option<TokenId> {
        todo!("skipOuterPattern")
    }

    /// Dart (line 12342): `Token? skipObjectPatternRest(Token token)`
    pub fn skip_object_pattern_rest(&mut self, token: TokenId) -> Option<TokenId> {
        todo!("skipObjectPatternRest")
    }

    /// Dart (line 12353): `Token parsePatternVariableDeclarationStatement( Token keyword, Token start, Token varOrFinal, )`
    pub fn parse_pattern_variable_declaration_statement(&mut self, keyword: TokenId, start: TokenId, var_or_final: TokenId) -> TokenId {
        todo!("parsePatternVariableDeclarationStatement")
    }

    /// Dart (line 12373): `Token parsePatternAssignment(Token token)`
    pub fn parse_pattern_assignment(&mut self, token: TokenId) -> TokenId {
        todo!("parsePatternAssignment")
    }

    /// Dart (line 12377): `Token _parsePatternAssignment(Token token, bool allowCascades)`
    pub fn parse_pattern_assignment_impl(&mut self, token: TokenId, allow_cascades: bool) -> TokenId {
        todo!("_parsePatternAssignment")
    }

    /// Dart (line 12393): `Token parseSwitchExpression(Token token)`
    pub fn parse_switch_expression(&mut self, token: TokenId) -> TokenId {
        todo!("parseSwitchExpression")
    }

    /// Dart (line 12518): `Token? findNextCommaOrSemicolon(Token token, Token limit)`
    pub fn find_next_comma_or_semicolon(&mut self, token: TokenId, limit: TokenId) -> Option<TokenId> {
        todo!("findNextCommaOrSemicolon")
    }

}
