// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/parser_impl.dart (lines 8451-10489)

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

    /// Dart (line 8451): `Token parseLiteralListSuffix(Token token, Token? constKeyword)`
    pub fn parse_literal_list_suffix(&mut self, token: TokenId, const_keyword: Option<TokenId>) -> TokenId {
        todo!("parseLiteralListSuffix")
    }

    /// Dart (line 8536): `Token parseLiteralSetOrMapSuffix(Token token, Token? constKeyword)`
    pub fn parse_literal_set_or_map_suffix(&mut self, token: TokenId, const_keyword: Option<TokenId>) -> TokenId {
        todo!("parseLiteralSetOrMapSuffix")
    }

    /// Dart (line 8687): `LiteralEntryInfo? _computeLiteralEntry(Token token)`
    pub fn compute_literal_entry(&mut self, token: TokenId) -> Option<LiteralEntryInfo> {
        todo!("_computeLiteralEntry")
    }

    /// Dart (line 8694): `LiteralEntryInfo? _nextLiteralEntry(LiteralEntryInfo info, Token token)`
    pub fn next_literal_entry(&mut self, info: LiteralEntryInfo, token: TokenId) -> Option<LiteralEntryInfo> {
        todo!("_nextLiteralEntry")
    }

    /// Dart (line 8701): `Token _splitFollowingQuestionPeriod(Token token)`
    pub fn split_following_question_period(&mut self, token: TokenId) -> TokenId {
        todo!("_splitFollowingQuestionPeriod")
    }

    /// Dart (line 8717): `Token parseLiteralFunctionSuffix(Token token)`
    pub fn parse_literal_function_suffix(&mut self, token: TokenId) -> TokenId {
        todo!("parseLiteralFunctionSuffix")
    }

    /// Dart (line 8740): `Token parseLiteralListSetMapOrFunction(Token start, Token? constKeyword)`
    pub fn parse_literal_list_set_map_or_function(&mut self, start: TokenId, const_keyword: Option<TokenId>) -> TokenId {
        todo!("parseLiteralListSetMapOrFunction")
    }

    /// Dart (line 8787): `Token parseMapLiteralEntry(Token token)`
    pub fn parse_map_literal_entry(&mut self, token: TokenId) -> TokenId {
        todo!("parseMapLiteralEntry")
    }

    /// Dart (line 8807): `Token parseSendOrFunctionLiteral( Token token, IdentifierContext context, ConstantPatternContext constantPatternContext, )`
    pub fn parse_send_or_function_literal(&mut self, token: TokenId, context: IdentifierContext, constant_pattern_context: ConstantPatternContext) -> TokenId {
        todo!("parseSendOrFunctionLiteral")
    }

    /// Dart (line 8840): `Token ensureArguments(Token token)`
    pub fn ensure_arguments(&mut self, token: TokenId) -> TokenId {
        todo!("ensureArguments")
    }

    /// Dart (line 8852): `Token parseConstructorInvocationArguments(Token token)`
    pub fn parse_constructor_invocation_arguments(&mut self, token: TokenId) -> TokenId {
        todo!("parseConstructorInvocationArguments")
    }

    /// Dart (line 8880): `Token parseNewExpression(Token token)`
    pub fn parse_new_expression(&mut self, token: TokenId) -> TokenId {
        todo!("parseNewExpression")
    }

    /// Dart (line 8977): `Token parseImplicitCreationExpression( Token token, Token openAngleBracket, TypeParamOrArgInfo typeArg, )`
    pub fn parse_implicit_creation_expression(&mut self, token: TokenId, open_angle_bracket: TokenId, type_arg: TypeParamOrArgInfo) -> TokenId {
        todo!("parseImplicitCreationExpression")
    }

    /// Dart (line 9011): `Token parseConstExpression(Token token)`
    pub fn parse_const_expression(&mut self, token: TokenId) -> TokenId {
        todo!("parseConstExpression")
    }

    /// Dart (line 9145): `Token parseLiteralInt(Token token)`
    pub fn parse_literal_int(&mut self, token: TokenId) -> TokenId {
        todo!("parseLiteralInt")
    }

    /// Dart (line 9155): `Token parseLiteralIntWithSeparators(Token token)`
    pub fn parse_literal_int_with_separators(&mut self, token: TokenId) -> TokenId {
        todo!("parseLiteralIntWithSeparators")
    }

    /// Dart (line 9170): `Token parseLiteralDouble(Token token)`
    pub fn parse_literal_double(&mut self, token: TokenId) -> TokenId {
        todo!("parseLiteralDouble")
    }

    /// Dart (line 9177): `Token parseLiteralDoubleWithSeparators(Token token)`
    pub fn parse_literal_double_with_separators(&mut self, token: TokenId) -> TokenId {
        todo!("parseLiteralDoubleWithSeparators")
    }

    /// Dart (line 9189): `Token parseLiteralString(Token token)`
    pub fn parse_literal_string(&mut self, token: TokenId) -> TokenId {
        todo!("parseLiteralString")
    }

    /// Dart (line 9212): `Token parseLiteralSymbol(Token token)`
    pub fn parse_literal_symbol(&mut self, token: TokenId) -> TokenId {
        todo!("parseLiteralSymbol")
    }

    /// Dart (line 9240): `Token parseSingleLiteralString(Token token)`
    pub fn parse_single_literal_string(&mut self, token: TokenId) -> TokenId {
        todo!("parseSingleLiteralString")
    }

    /// Dart (line 9277): `Token parseIdentifierExpression(Token token)`
    pub fn parse_identifier_expression(&mut self, token: TokenId) -> TokenId {
        todo!("parseIdentifierExpression")
    }

    /// Dart (line 9297): `Token parseLiteralBool(Token token)`
    pub fn parse_literal_bool(&mut self, token: TokenId) -> TokenId {
        todo!("parseLiteralBool")
    }

    /// Dart (line 9309): `Token parseLiteralNull(Token token)`
    pub fn parse_literal_null(&mut self, token: TokenId) -> TokenId {
        todo!("parseLiteralNull")
    }

    /// Dart (line 9316): `Token parseSend( Token token, IdentifierContext context, ConstantPatternContext constantPatternContext, )`
    pub fn parse_send(&mut self, token: TokenId, context: IdentifierContext, constant_pattern_context: ConstantPatternContext) -> TokenId {
        todo!("parseSend")
    }

    /// Dart (line 9407): `Token skipArgumentsOpt(Token token)`
    pub fn skip_arguments_opt(&mut self, token: TokenId) -> TokenId {
        todo!("skipArgumentsOpt")
    }

    /// Dart (line 9421): `Token parseArgumentsOptMetadata(Token token, bool hasTypeArguments)`
    pub fn parse_arguments_opt_metadata(&mut self, token: TokenId, has_type_arguments: bool) -> TokenId {
        todo!("parseArgumentsOptMetadata")
    }

    /// Dart (line 9455): `Token parseArgumentsOpt(Token token)`
    pub fn parse_arguments_opt(&mut self, token: TokenId) -> TokenId {
        todo!("parseArgumentsOpt")
    }

    /// Dart (line 9479): `Token parseArguments(Token token)`
    pub fn parse_arguments(&mut self, token: TokenId) -> TokenId {
        todo!("parseArguments")
    }

    /// Dart (line 9484): `Token parseArgumentsRest(Token token)`
    pub fn parse_arguments_rest(&mut self, token: TokenId) -> TokenId {
        todo!("parseArgumentsRest")
    }

    /// Dart (line 9610): `Token parseIsOperatorRest(Token token)`
    pub fn parse_is_operator_rest(&mut self, token: TokenId) -> TokenId {
        todo!("parseIsOperatorRest")
    }

    /// Dart (line 9625): `TypeInfo computeTypeAfterIsOrAs(Token token)`
    pub fn compute_type_after_is_or_as(&mut self, token: TokenId) -> TypeInfo {
        todo!("computeTypeAfterIsOrAs")
    }

    /// Dart (line 9673): `Token parseAsOperatorRest(Token token)`
    pub fn parse_as_operator_rest(&mut self, token: TokenId) -> TokenId {
        todo!("parseAsOperatorRest")
    }

    /// Dart (line 9684): `Token skipChainedAsIsOperators(Token token)`
    pub fn skip_chained_as_is_operators(&mut self, token: TokenId) -> TokenId {
        todo!("skipChainedAsIsOperators")
    }

    /// Dart (line 9706): `bool looksLikeLocalFunction(Token token)`
    pub fn looks_like_local_function(&mut self, token: TokenId) -> bool {
        todo!("looksLikeLocalFunction")
    }

    /// Dart (line 9731): `bool looksLikeFunctionBody(Token token)`
    pub fn looks_like_function_body(&mut self, token: TokenId) -> bool {
        todo!("looksLikeFunctionBody")
    }

    /// Dart (line 9738): `Token parseExpressionStatementOrConstDeclaration(Token start)`
    pub fn parse_expression_statement_or_const_declaration(&mut self, start: TokenId) -> TokenId {
        todo!("parseExpressionStatementOrConstDeclaration")
    }

    /// Dart (line 9786): `Token parseExpressionStatementOrDeclaration( Token start, [ ForPartsContext? forPartsContext, ])`
    pub fn parse_expression_statement_or_declaration(&mut self, start: TokenId, for_parts_context: Option<&mut ForPartsContext>) -> TokenId {
        todo!("parseExpressionStatementOrDeclaration")
    }

    /// Dart (line 9847): `Token parseExpressionStatementOrDeclarationAfterModifiers( Token beforeType, Token start, Token? lateToken, Token? varFinalOrConst, TypeInfo? typeInfo, [ ForPartsContext? forPartsContext, ])`
    pub fn parse_expression_statement_or_declaration_after_modifiers(&mut self, before_type: TokenId, start: TokenId, late_token: Option<TokenId>, var_final_or_const: Option<TokenId>, type_info: Option<&TypeInfo>, for_parts_context: Option<&mut ForPartsContext>) -> TokenId {
        todo!("parseExpressionStatementOrDeclarationAfterModifiers")
    }

    /// Dart (line 10063): `Token parseVariablesDeclarationRest(Token token, bool endWithSemicolon)`
    pub fn parse_variables_declaration_rest(&mut self, token: TokenId, end_with_semicolon: bool) -> TokenId {
        todo!("parseVariablesDeclarationRest")
    }

    /// Dart (line 10080): `Token parseOptionallyInitializedIdentifier(Token token)`
    pub fn parse_optionally_initialized_identifier(&mut self, token: TokenId) -> TokenId {
        todo!("parseOptionallyInitializedIdentifier")
    }

    /// Dart (line 10096): `Token parseIfStatement(Token token)`
    pub fn parse_if_statement(&mut self, token: TokenId) -> TokenId {
        todo!("parseIfStatement")
    }

    /// Dart (line 10137): `Token parseForStatement(Token token, Token? awaitToken)`
    pub fn parse_for_statement(&mut self, token: TokenId, await_token: Option<TokenId>) -> TokenId {
        todo!("parseForStatement")
    }

    /// Dart (line 10187): `Token parseForLoopPartsStart( Token? awaitToken, Token forToken, ForPartsContext forPartsContext, )`
    pub fn parse_for_loop_parts_start(&mut self, await_token: Option<TokenId>, for_token: TokenId, for_parts_context: &mut ForPartsContext) -> TokenId {
        todo!("parseForLoopPartsStart")
    }

    /// Dart (line 10243): `Token parseForLoopPartsMid(Token token, Token? awaitToken, Token forToken)`
    pub fn parse_for_loop_parts_mid(&mut self, token: TokenId, await_token: Option<TokenId>, for_token: TokenId) -> TokenId {
        todo!("parseForLoopPartsMid")
    }

    /// Dart (line 10299): `Token parseForRest(Token? awaitToken, Token token, Token forToken)`
    pub fn parse_for_rest(&mut self, await_token: Option<TokenId>, token: TokenId, for_token: TokenId) -> TokenId {
        todo!("parseForRest")
    }

    /// Dart (line 10311): `Token parseForLoopPartsRest(Token token, Token forToken, Token? awaitToken)`
    pub fn parse_for_loop_parts_rest(&mut self, token: TokenId, for_token: TokenId, await_token: Option<TokenId>) -> TokenId {
        todo!("parseForLoopPartsRest")
    }

    /// Dart (line 10363): `Token parseForInRest( Token token, Token? awaitToken, Token forToken, Token? patternKeyword, Token? identifier, )`
    pub fn parse_for_in_rest(&mut self, token: TokenId, await_token: Option<TokenId>, for_token: TokenId, pattern_keyword: Option<TokenId>, identifier: Option<TokenId>) -> TokenId {
        todo!("parseForInRest")
    }

    /// Dart (line 10387): `Token parseForInLoopPartsRest( Token token, Token? awaitToken, Token forToken, Token? patternKeyword, Token? identifier, )`
    pub fn parse_for_in_loop_parts_rest(&mut self, token: TokenId, await_token: Option<TokenId>, for_token: TokenId, pattern_keyword: Option<TokenId>, identifier: Option<TokenId>) -> TokenId {
        todo!("parseForInLoopPartsRest")
    }

    /// Dart (line 10441): `Token parseWhileStatement(Token token)`
    pub fn parse_while_statement(&mut self, token: TokenId) -> TokenId {
        todo!("parseWhileStatement")
    }

    /// Dart (line 10461): `Token parseDoWhileStatement(Token token)`
    pub fn parse_do_while_statement(&mut self, token: TokenId) -> TokenId {
        todo!("parseDoWhileStatement")
    }

}
