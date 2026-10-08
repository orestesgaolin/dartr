// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/parser_impl.dart (lines 4393-6686)

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

    /// Dart (line 4393): `Token parseFields( Token beforeStart, Token? abstractToken, Token? augmentToken, Token? externalToken, Token? staticToken, Token? covariantToken, Token? lateToken, Token? varFinalOrConst, Token beforeType, TypeInfo typeInfo, Token name, DeclarationKind kind, String? enclosingDeclarationName, bool nameIsRecovered, )`
    pub fn parse_fields(&mut self, before_start: TokenId, abstract_token: Option<TokenId>, augment_token: Option<TokenId>, external_token: Option<TokenId>, static_token: Option<TokenId>, covariant_token: Option<TokenId>, late_token: Option<TokenId>, var_final_or_const: Option<TokenId>, before_type: TokenId, type_info: &TypeInfo, name: TokenId, kind: DeclarationKind, enclosing_declaration_name: Option<&str>, name_is_recovered: bool) -> TokenId {
        todo!("parseFields")
    }

    /// Dart (line 4561): `Token parseTopLevelMethod( Token beforeStart, Token? augmentToken, Token? externalToken, Token beforeType, TypeInfo typeInfo, Token? getOrSet, Token name, bool nameIsRecovered, )`
    pub fn parse_top_level_method(&mut self, before_start: TokenId, augment_token: Option<TokenId>, external_token: Option<TokenId>, before_type: TokenId, type_info: &TypeInfo, get_or_set: Option<TokenId>, name: TokenId, name_is_recovered: bool) -> TokenId {
        todo!("parseTopLevelMethod")
    }

    /// Dart (line 4615): `Token parseMethodTypeVar(Token name)`
    pub fn parse_method_type_var(&mut self, name: TokenId) -> TokenId {
        todo!("parseMethodTypeVar")
    }

    /// Dart (line 4637): `Token parseFieldInitializerOpt( Token token, Token name, Token? lateToken, Token? abstractToken, Token? augmentToken, Token? externalToken, Token? varFinalOrConst, DeclarationKind kind, String? enclosingDeclarationName, )`
    pub fn parse_field_initializer_opt(&mut self, token: TokenId, name: TokenId, late_token: Option<TokenId>, abstract_token: Option<TokenId>, augment_token: Option<TokenId>, external_token: Option<TokenId>, var_final_or_const: Option<TokenId>, kind: DeclarationKind, enclosing_declaration_name: Option<&str>) -> TokenId {
        todo!("parseFieldInitializerOpt")
    }

    /// Dart (line 4680): `Token parseVariableInitializerOpt(Token token)`
    pub fn parse_variable_initializer_opt(&mut self, token: TokenId) -> TokenId {
        todo!("parseVariableInitializerOpt")
    }

    /// Dart (line 4692): `Token parseInitializersOpt(Token token)`
    pub fn parse_initializers_opt(&mut self, token: TokenId) -> TokenId {
        todo!("parseInitializersOpt")
    }

    /// Dart (line 4706): `Token parseInitializers(Token token)`
    pub fn parse_initializers(&mut self, token: TokenId) -> TokenId {
        todo!("parseInitializers")
    }

    /// Dart (line 4770): `Token parseInitializer(Token token)`
    pub fn parse_initializer(&mut self, token: TokenId) -> TokenId {
        todo!("parseInitializer")
    }

    /// Dart (line 4876): `Token parseSuperInitializerExpression(Token start)`
    pub fn parse_super_initializer_expression(&mut self, start: TokenId) -> TokenId {
        todo!("parseSuperInitializerExpression")
    }

    /// Dart (line 4929): `Token parseInitializerExpressionRest(Token token)`
    pub fn parse_initializer_expression_rest(&mut self, token: TokenId) -> TokenId {
        todo!("parseInitializerExpressionRest")
    }

    /// Dart (line 4939): `Token ensureBlock(Token token, BlockKind? missingBlockKind)`
    pub fn ensure_block(&mut self, token: TokenId, missing_block_kind: Option<BlockKind>) -> TokenId {
        todo!("ensureBlock")
    }

    /// Dart (line 4961): `Token insertBlock(Token token)`
    pub fn insert_block(&mut self, token: TokenId) -> TokenId {
        todo!("insertBlock")
    }

    /// Dart (line 4983): `Token ensureCloseParen(Token token, Token openParen)`
    pub fn ensure_close_paren(&mut self, token: TokenId, open_paren: TokenId) -> TokenId {
        todo!("ensureCloseParen")
    }

    /// Dart (line 5008): `Token ensureColon(Token token)`
    pub fn ensure_colon(&mut self, token: TokenId) -> TokenId {
        todo!("ensureColon")
    }

    /// Dart (line 5019): `Token ensureFunctionArrow(Token token)`
    pub fn ensure_function_arrow(&mut self, token: TokenId) -> TokenId {
        todo!("ensureFunctionArrow")
    }

    /// Dart (line 5030): `Token ensureLiteralString(Token token)`
    pub fn ensure_literal_string(&mut self, token: TokenId) -> TokenId {
        todo!("ensureLiteralString")
    }

    /// Dart (line 5048): `Token ensureSemicolon(Token token)`
    pub fn ensure_semicolon(&mut self, token: TokenId) -> TokenId {
        todo!("ensureSemicolon")
    }

    /// Dart (line 5067): `Token rewriteAndRecover(Token token, codes.Message message, Token newToken)`
    pub fn rewrite_and_recover(&mut self, token: TokenId, message: CfeMessage, new_token: TokenId) -> TokenId {
        todo!("rewriteAndRecover")
    }

    /// Dart (line 5074): `Token rewriteSquareBrackets(Token token)`
    pub fn rewrite_square_brackets(&mut self, token: TokenId) -> TokenId {
        todo!("rewriteSquareBrackets")
    }

    /// Dart (line 5103): `Token skipUnexpectedTokenOpt(Token token, List<String> expectedNext)`
    pub fn skip_unexpected_token_opt(&mut self, token: TokenId, expected_next: &[&str]) -> TokenId {
        todo!("skipUnexpectedTokenOpt")
    }

    /// Dart (line 5117): `Token parseNativeClause(Token token)`
    pub fn parse_native_clause(&mut self, token: TokenId) -> TokenId {
        todo!("parseNativeClause")
    }

    /// Dart (line 5130): `Token skipClassOrMixinOrExtensionBody(Token token)`
    pub fn skip_class_or_mixin_or_extension_body(&mut self, token: TokenId) -> TokenId {
        todo!("skipClassOrMixinOrExtensionBody")
    }

    /// Dart (line 5140): `Token parseClassOrMixinOrExtensionBody( Token token, DeclarationKind kind, String? enclosingDeclarationName, )`
    pub fn parse_class_or_mixin_or_extension_body(&mut self, token: TokenId, kind: DeclarationKind, enclosing_declaration_name: Option<&str>) -> TokenId {
        todo!("parseClassOrMixinOrExtensionBody")
    }

    /// Dart (line 5163): `bool isUnaryMinus(Token token) =>`
    pub fn is_unary_minus(&mut self, token: TokenId) -> bool {
        todo!("isUnaryMinus")
    }

    /// Dart (line 5174): `Token parseClassMember(Token token, String? className)`
    pub fn parse_class_member(&mut self, token: TokenId, class_name: Option<&str>) -> TokenId {
        todo!("parseClassMember")
    }

    /// Dart (line 5188): `Token parseMixinMember(Token token, String mixinName)`
    pub fn parse_mixin_member(&mut self, token: TokenId, mixin_name: &str) -> TokenId {
        todo!("parseMixinMember")
    }

    /// Dart (line 5202): `Token parseExtensionMember(Token token, String extensionName)`
    pub fn parse_extension_member(&mut self, token: TokenId, extension_name: &str) -> TokenId {
        todo!("parseExtensionMember")
    }

    /// Dart (line 5210): `bool isReservedKeyword(Token token)`
    pub fn is_reserved_keyword(&mut self, token: TokenId) -> bool {
        todo!("isReservedKeyword")
    }

    /// Dart (line 5215): `bool indicatesMethodOrField(Token token)`
    pub fn indicates_method_or_field(&mut self, token: TokenId) -> bool {
        todo!("indicatesMethodOrField")
    }

    /// Dart (line 5245): `Token parseClassOrMixinOrExtensionOrEnumMemberImpl( Token token, DeclarationKind kind, String? enclosingDeclarationName, )`
    pub fn parse_class_or_mixin_or_extension_or_enum_member_impl(&mut self, token: TokenId, kind: DeclarationKind, enclosing_declaration_name: Option<&str>) -> TokenId {
        todo!("parseClassOrMixinOrExtensionOrEnumMemberImpl")
    }

    /// Dart (line 5689): `bool _isConstructor( Token name, Token? getOrSet, Token? newToken, String? enclosingDeclarationName, bool isOperator, )`
    pub fn is_constructor(&mut self, name: TokenId, get_or_set: Option<TokenId>, new_token: Option<TokenId>, enclosing_declaration_name: Option<&str>, is_operator: bool) -> bool {
        todo!("_isConstructor")
    }

    /// Dart (line 5754): `Token parseMethod( Token beforeStart, Token? abstractToken, Token? augmentToken, Token? externalToken, Token? staticToken, Token? covariantToken, Token? lateToken, Token? varFinalOrConst, Token beforeType, TypeInfo typeInfo, Token? getOrSet, Token? newToken, Token name, DeclarationKind kind, String? enclosingDeclarationName, bool nameIsRecovered, )`
    pub fn parse_method(&mut self, before_start: TokenId, abstract_token: Option<TokenId>, augment_token: Option<TokenId>, external_token: Option<TokenId>, static_token: Option<TokenId>, covariant_token: Option<TokenId>, late_token: Option<TokenId>, var_final_or_const: Option<TokenId>, before_type: TokenId, type_info: &TypeInfo, get_or_set: Option<TokenId>, new_token: Option<TokenId>, name: TokenId, kind: DeclarationKind, enclosing_declaration_name: Option<&str>, name_is_recovered: bool) -> TokenId {
        todo!("parseMethod")
    }

    /// Dart (line 6141): `Token parseFactoryMethod( Token token, DeclarationKind kind, Token beforeStart, Token? augmentToken, Token? externalToken, Token? staticOrCovariant, Token? varFinalOrConst, bool hasName, )`
    pub fn parse_factory_method(&mut self, token: TokenId, kind: DeclarationKind, before_start: TokenId, augment_token: Option<TokenId>, external_token: Option<TokenId>, static_or_covariant: Option<TokenId>, var_final_or_const: Option<TokenId>, has_name: bool) -> TokenId {
        todo!("parseFactoryMethod")
    }

    /// Dart (line 6258): `Token parseOperatorName(Token token)`
    pub fn parse_operator_name(&mut self, token: TokenId) -> TokenId {
        todo!("parseOperatorName")
    }

    /// Dart (line 6295): `Token parseFunctionExpression(Token token)`
    pub fn parse_function_expression(&mut self, token: TokenId) -> TokenId {
        todo!("parseFunctionExpression")
    }

    /// Dart (line 6308): `Token parseFunctionLiteral( Token start, Token beforeName, Token name, TypeInfo typeInfo, TypeParamOrArgInfo typeParam, IdentifierContext context, )`
    pub fn parse_function_literal(&mut self, start: TokenId, before_name: TokenId, name: TokenId, type_info: &TypeInfo, type_param: TypeParamOrArgInfo, context: IdentifierContext) -> TokenId {
        todo!("parseFunctionLiteral")
    }

    /// Dart (line 6343): `Token parseNamedFunctionRest( Token beforeName, Token begin, Token formals, bool isFunctionExpression, )`
    pub fn parse_named_function_rest(&mut self, before_name: TokenId, begin: TokenId, formals: TokenId, is_function_expression: bool) -> TokenId {
        todo!("parseNamedFunctionRest")
    }

    /// Dart (line 6381): `Token parseAsyncOptBody( Token token, bool ofFunctionExpression, bool allowAbstract, )`
    pub fn parse_async_opt_body(&mut self, token: TokenId, of_function_expression: bool, allow_abstract: bool) -> TokenId {
        todo!("parseAsyncOptBody")
    }

    /// Dart (line 6393): `Token parseConstructorReference( Token token, ConstructorReferenceContext constructorReferenceContext, [ TypeParamOrArgInfo? typeArg, ])`
    pub fn parse_constructor_reference(&mut self, token: TokenId, constructor_reference_context: ConstructorReferenceContext, type_arg: Option<TypeParamOrArgInfo>) -> TokenId {
        todo!("parseConstructorReference")
    }

    /// Dart (line 6430): `Token parseRedirectingFactoryBody(Token token)`
    pub fn parse_redirecting_factory_body(&mut self, token: TokenId) -> TokenId {
        todo!("parseRedirectingFactoryBody")
    }

    /// Dart (line 6444): `Token skipFunctionBody(Token token, bool isExpression, bool allowAbstract)`
    pub fn skip_function_body(&mut self, token: TokenId, is_expression: bool, allow_abstract: bool) -> TokenId {
        todo!("skipFunctionBody")
    }

    /// Dart (line 6516): `Token parseFunctionBody( Token token, bool ofFunctionExpression, bool allowAbstract, )`
    pub fn parse_function_body(&mut self, token: TokenId, of_function_expression: bool, allow_abstract: bool) -> TokenId {
        todo!("parseFunctionBody")
    }

    /// Dart (line 6617): `Token parseExpressionFunctionBody(Token token, bool ofFunctionExpression)`
    pub fn parse_expression_function_body(&mut self, token: TokenId, of_function_expression: bool) -> TokenId {
        todo!("parseExpressionFunctionBody")
    }

    /// Dart (line 6633): `Token skipAsyncModifier(Token token)`
    pub fn skip_async_modifier(&mut self, token: TokenId) -> TokenId {
        todo!("skipAsyncModifier")
    }

    /// Dart (line 6653): `Token parseAsyncModifierOpt(Token token)`
    pub fn parse_async_modifier_opt(&mut self, token: TokenId) -> TokenId {
        todo!("parseAsyncModifierOpt")
    }

}
