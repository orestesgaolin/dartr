// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/parser_impl.dart (lines 2525-4392)

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

    /// Dart (line 2525): `Token parseQualified( Token token, IdentifierContext context, IdentifierContext continuationContext, )`
    pub fn parse_qualified(&mut self, token: TokenId, context: IdentifierContext, continuation_context: IdentifierContext) -> TokenId {
        todo!("parseQualified")
    }

    /// Dart (line 2542): `Token parseQualifiedRestOpt( Token token, IdentifierContext continuationContext, )`
    pub fn parse_qualified_rest_opt(&mut self, token: TokenId, continuation_context: IdentifierContext) -> TokenId {
        todo!("parseQualifiedRestOpt")
    }

    /// Dart (line 2558): `Token parseQualifiedRest(Token token, IdentifierContext context)`
    pub fn parse_qualified_rest(&mut self, token: TokenId, context: IdentifierContext) -> TokenId {
        todo!("parseQualifiedRest")
    }

    /// Dart (line 2568): `Token skipBlock(Token token)`
    pub fn skip_block(&mut self, token: TokenId) -> TokenId {
        todo!("skipBlock")
    }

    /// Dart (line 2596): `Token parseEnum(Token beginToken, Token? augmentToken, Token enumKeyword)`
    pub fn parse_enum(&mut self, begin_token: TokenId, augment_token: Option<TokenId>, enum_keyword: TokenId) -> TokenId {
        todo!("parseEnum")
    }

    /// Dart (line 2717): `Token parseEnumHeaderOpt(Token token, Token enumKeyword)`
    pub fn parse_enum_header_opt(&mut self, token: TokenId, enum_keyword: TokenId) -> TokenId {
        todo!("parseEnumHeaderOpt")
    }

    /// Dart (line 2811): `Token? recoveryEnumWith(Token token, codes.Message message)`
    pub fn recovery_enum_with(&mut self, token: TokenId, message: CfeMessage) -> Option<TokenId> {
        todo!("recoveryEnumWith")
    }

    /// Dart (line 2823): `Token? recoveryEnumImplements(Token token, codes.Message message)`
    pub fn recovery_enum_implements(&mut self, token: TokenId, message: CfeMessage) -> Option<TokenId> {
        todo!("recoveryEnumImplements")
    }

    /// Dart (line 2841): `Token? recoverySmallLookAheadSkipTokens( Token token, List<TokenType> lookFor, )`
    pub fn recovery_small_look_ahead_skip_tokens(&mut self, token: TokenId, look_for: &[TokenType]) -> Option<TokenId> {
        todo!("recoverySmallLookAheadSkipTokens")
    }

    /// Dart (line 2884): `Token parseEnumElement(Token token)`
    pub fn parse_enum_element(&mut self, token: TokenId) -> TokenId {
        todo!("parseEnumElement")
    }

    /// Dart (line 2937): `Token parseClassOrNamedMixinApplication( Token beginToken, Token? abstractToken, Token? sealedToken, Token? baseToken, Token? interfaceToken, Token? finalToken, Token? augmentToken, Token? mixinToken, Token classKeyword, )`
    pub fn parse_class_or_named_mixin_application(&mut self, begin_token: TokenId, abstract_token: Option<TokenId>, sealed_token: Option<TokenId>, base_token: Option<TokenId>, interface_token: Option<TokenId>, final_token: Option<TokenId>, augment_token: Option<TokenId>, mixin_token: Option<TokenId>, class_keyword: TokenId) -> TokenId {
        todo!("parseClassOrNamedMixinApplication")
    }

    /// Dart (line 3027): `Token parseNamedMixinApplication( Token token, Token begin, Token classKeyword, )`
    pub fn parse_named_mixin_application(&mut self, token: TokenId, begin: TokenId, class_keyword: TokenId) -> TokenId {
        todo!("parseNamedMixinApplication")
    }

    /// Dart (line 3079): `Token parseClass( Token token, Token beginToken, Token classKeyword, Token? constToken, String className, )`
    pub fn parse_class(&mut self, token: TokenId, begin_token: TokenId, class_keyword: TokenId, const_token: Option<TokenId>, class_name: &str) -> TokenId {
        todo!("parseClass")
    }

    /// Dart (line 3119): `Token parseClassHeaderOpt(Token token, Token begin, Token classKeyword)`
    pub fn parse_class_header_opt(&mut self, token: TokenId, begin: TokenId, class_keyword: TokenId) -> TokenId {
        todo!("parseClassHeaderOpt")
    }

    /// Dart (line 3133): `Token parseClassHeaderRecovery(Token token, Token begin, Token classKeyword)`
    pub fn parse_class_header_recovery(&mut self, token: TokenId, begin: TokenId, class_keyword: TokenId) -> TokenId {
        todo!("parseClassHeaderRecovery")
    }

    /// Dart (line 3143): `Token parseExtensionTypeHeaderRecovery(Token token, Token extensionKeyword)`
    pub fn parse_extension_type_header_recovery(&mut self, token: TokenId, extension_keyword: TokenId) -> TokenId {
        todo!("parseExtensionTypeHeaderRecovery")
    }

    /// Dart (line 3154): `Token parseDeclarationHeaderRecoveryInternal( Token token, Token begin, Token declarationKeyword, DeclarationHeaderKind kind, )`
    pub fn parse_declaration_header_recovery_internal(&mut self, token: TokenId, begin: TokenId, declaration_keyword: TokenId, kind: DeclarationHeaderKind) -> TokenId {
        todo!("parseDeclarationHeaderRecoveryInternal")
    }

    /// Dart (line 3288): `Token parseClassExtendsOpt(Token token, DeclarationHeaderKind kind)`
    pub fn parse_class_extends_opt(&mut self, token: TokenId, kind: DeclarationHeaderKind) -> TokenId {
        todo!("parseClassExtendsOpt")
    }

    /// Dart (line 3303): `Token parseClassExtendsSeenExtendsClause( Token extendsKeyword, Token token, DeclarationHeaderKind kind, )`
    pub fn parse_class_extends_seen_extends_clause(&mut self, extends_keyword: TokenId, token: TokenId, kind: DeclarationHeaderKind) -> TokenId {
        todo!("parseClassExtendsSeenExtendsClause")
    }

    /// Dart (line 3345): `Token parseClassOrMixinOrEnumImplementsOpt(Token token)`
    pub fn parse_class_or_mixin_or_enum_implements_opt(&mut self, token: TokenId) -> TokenId {
        todo!("parseClassOrMixinOrEnumImplementsOpt")
    }

    /// Dart (line 3371): `Token parseMixin( Token beginToken, Token? augmentToken, Token? baseToken, Token mixinKeyword, )`
    pub fn parse_mixin(&mut self, begin_token: TokenId, augment_token: Option<TokenId>, base_token: Option<TokenId>, mixin_keyword: TokenId) -> TokenId {
        todo!("parseMixin")
    }

    /// Dart (line 3429): `Token parseMixinHeaderOpt( Token token, Token? constKeyword, Token mixinKeyword, )`
    pub fn parse_mixin_header_opt(&mut self, token: TokenId, const_keyword: Option<TokenId>, mixin_keyword: TokenId) -> TokenId {
        todo!("parseMixinHeaderOpt")
    }

    /// Dart (line 3445): `Token parseMixinHeaderRecovery( Token token, Token mixinKeyword, Token headerStart, )`
    pub fn parse_mixin_header_recovery(&mut self, token: TokenId, mixin_keyword: TokenId, header_start: TokenId) -> TokenId {
        todo!("parseMixinHeaderRecovery")
    }

    /// Dart (line 3547): `Token parseMixinOnOpt(Token token)`
    pub fn parse_mixin_on_opt(&mut self, token: TokenId) -> TokenId {
        todo!("parseMixinOnOpt")
    }

    /// Dart (line 3555): `Token parseMixinOn(Token token)`
    pub fn parse_mixin_on(&mut self, token: TokenId) -> TokenId {
        todo!("parseMixinOn")
    }

    /// Dart (line 3576): `Token parseExtension( Token beginToken, Token? augmentToken, Token extensionKeyword, )`
    pub fn parse_extension(&mut self, begin_token: TokenId, augment_token: Option<TokenId>, extension_keyword: TokenId) -> TokenId {
        todo!("parseExtension")
    }

    /// Dart (line 3618): `Token parseExtensionDeclaration( Token beginToken, Token token, Token? augmentToken, Token extensionKeyword, )`
    pub fn parse_extension_declaration(&mut self, begin_token: TokenId, token: TokenId, augment_token: Option<TokenId>, extension_keyword: TokenId) -> TokenId {
        todo!("parseExtensionDeclaration")
    }

    /// Dart (line 3739): `Token parsePrimaryConstructorOpt( DeclarationKind kind, Token token, Token? constKeyword,`
    pub fn parse_primary_constructor_opt(&mut self, kind: DeclarationKind, token: TokenId, const_keyword: Option<TokenId>, allow_extension_type_representation: bool) -> TokenId {
        todo!("parsePrimaryConstructorOpt")
    }

    /// Dart (line 3869): `Token parsePrimaryConstructorBody(Token token)`
    pub fn parse_primary_constructor_body(&mut self, token: TokenId) -> TokenId {
        todo!("parsePrimaryConstructorBody")
    }

    /// Dart (line 3915): `Token parseExtensionTypeDeclaration( Token beginToken, Token token, Token? augmentToken, Token extensionKeyword, Token typeKeyword, )`
    pub fn parse_extension_type_declaration(&mut self, begin_token: TokenId, token: TokenId, augment_token: Option<TokenId>, extension_keyword: TokenId, type_keyword: TokenId) -> TokenId {
        todo!("parseExtensionTypeDeclaration")
    }

    /// Dart (line 3995): `Token parseStringPart(Token token)`
    pub fn parse_string_part(&mut self, token: TokenId) -> TokenId {
        todo!("parseStringPart")
    }

    /// Dart (line 4011): `Token insertSyntheticIdentifier( Token token, IdentifierContext context,`
    pub fn insert_synthetic_identifier(&mut self, token: TokenId, context: IdentifierContext, message: Option<CfeMessage>, message_on_token: Option<TokenId>) -> TokenId {
        todo!("insertSyntheticIdentifier")
    }

    /// Dart (line 4031): `Token ensureIdentifier(Token token, IdentifierContext context)`
    pub fn ensure_identifier(&mut self, token: TokenId, context: IdentifierContext) -> TokenId {
        todo!("ensureIdentifier")
    }

    /// Dart (line 4045): `bool _isNewOrIdentifier(Token token)`
    pub fn is_new_or_identifier(&mut self, token: TokenId) -> bool {
        todo!("_isNewOrIdentifier")
    }

    /// Dart (line 4063): `void _tryRewriteNewToIdentifier(Token token, IdentifierContext context)`
    pub fn try_rewrite_new_to_identifier(&mut self, token: TokenId, context: IdentifierContext) {
        todo!("_tryRewriteNewToIdentifier")
    }

    /// Dart (line 4068): `void _tryRewriteNewToIdentifierImpl(Token token)`
    pub fn try_rewrite_new_to_identifier_impl(&mut self, token: TokenId) {
        todo!("_tryRewriteNewToIdentifierImpl")
    }

    /// Dart (line 4096): `Token ensureIdentifierPotentiallyRecovered( Token token, IdentifierContext context, bool isRecovered, )`
    pub fn ensure_identifier_potentially_recovered(&mut self, token: TokenId, context: IdentifierContext, is_recovered: bool) -> TokenId {
        todo!("ensureIdentifierPotentiallyRecovered")
    }

    /// Dart (line 4118): `Token parseTypeVariablesOpt(Token token)`
    pub fn parse_type_variables_opt(&mut self, token: TokenId) -> TokenId {
        todo!("parseTypeVariablesOpt")
    }

    /// Dart (line 4131): `Token parseTopLevelMember(Token token)`
    pub fn parse_top_level_member(&mut self, token: TokenId) -> TokenId {
        todo!("parseTopLevelMember")
    }

    /// Dart (line 4136): `Token parseTopLevelMemberImpl(Token token)`
    pub fn parse_top_level_member_impl(&mut self, token: TokenId) -> TokenId {
        todo!("parseTopLevelMemberImpl")
    }

}
