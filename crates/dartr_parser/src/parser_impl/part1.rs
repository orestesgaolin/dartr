// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/parser_impl.dart (lines 419-2524)

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

    /// Dart (line 419): `Token parseUnit(Token token)`
    pub fn parse_unit(&mut self, token: TokenId) -> TokenId {
        todo!("parseUnit")
    }

    /// Dart (line 472): `Token parseDirectives(Token token)`
    pub fn parse_directives(&mut self, token: TokenId) -> TokenId {
        todo!("parseDirectives")
    }

    /// Dart (line 531): `Token parseTopLevelDeclaration(Token token)`
    pub fn parse_top_level_declaration(&mut self, token: TokenId) -> TokenId {
        todo!("parseTopLevelDeclaration")
    }

    /// Dart (line 555): `Token parseTopLevelDeclarationImpl( Token token, DirectiveContext? directiveState, )`
    pub fn parse_top_level_declaration_impl(&mut self, token: TokenId, directive_state: Option<&mut DirectiveContext>) -> TokenId {
        todo!("parseTopLevelDeclarationImpl")
    }

    /// Dart (line 673): `Token parseTopLevelKeywordDeclaration( Token beginToken, Token modifierStart, Token keyword, Token? sealedToken, Token? baseToken, Token? interfaceToken, DirectiveContext? directiveState, )`
    pub fn parse_top_level_keyword_declaration(&mut self, begin_token: TokenId, modifier_start: TokenId, keyword: TokenId, sealed_token: Option<TokenId>, base_token: Option<TokenId>, interface_token: Option<TokenId>, directive_state: Option<&mut DirectiveContext>) -> TokenId {
        todo!("parseTopLevelKeywordDeclaration")
    }

    /// Dart (line 836): `Token _handleModifiersForClassDeclaration( Token beginToken, Token modifierStart, Token classKeyword, Token? sealedToken, Token? baseToken, Token? interfaceToken, Token? mixinToken, DirectiveContext? directiveState, )`
    pub fn handle_modifiers_for_class_declaration(&mut self, begin_token: TokenId, modifier_start: TokenId, class_keyword: TokenId, sealed_token: Option<TokenId>, base_token: Option<TokenId>, interface_token: Option<TokenId>, mixin_token: Option<TokenId>, directive_state: Option<&mut DirectiveContext>) -> TokenId {
        todo!("_handleModifiersForClassDeclaration")
    }

    /// Dart (line 877): `bool _isIdentifierOrQuestionIdentifier(Token token)`
    pub fn is_identifier_or_question_identifier(&mut self, token: TokenId) -> bool {
        todo!("_isIdentifierOrQuestionIdentifier")
    }

    /// Dart (line 890): `Token parseLibraryAugmentation(Token augmentKeyword, Token libraryKeyword)`
    pub fn parse_library_augmentation(&mut self, augment_keyword: TokenId, library_keyword: TokenId) -> TokenId {
        todo!("parseLibraryAugmentation")
    }

    /// Dart (line 907): `Token parseLibraryName(Token libraryKeyword)`
    pub fn parse_library_name(&mut self, library_keyword: TokenId) -> TokenId {
        todo!("parseLibraryName")
    }

    /// Dart (line 932): `Token parseImportPrefixOpt(Token token)`
    pub fn parse_import_prefix_opt(&mut self, token: TokenId) -> TokenId {
        todo!("parseImportPrefixOpt")
    }

    /// Dart (line 960): `Token parseImport(Token importKeyword)`
    pub fn parse_import(&mut self, import_keyword: TokenId) -> TokenId {
        todo!("parseImport")
    }

    /// Dart (line 982): `Token parseImportRecovery(Token token)`
    pub fn parse_import_recovery(&mut self, token: TokenId) -> TokenId {
        todo!("parseImportRecovery")
    }

    /// Dart (line 1101): `Token parseConditionalUriStar(Token token)`
    pub fn parse_conditional_uri_star(&mut self, token: TokenId) -> TokenId {
        todo!("parseConditionalUriStar")
    }

    /// Dart (line 1117): `Token parseConditionalUri(Token token)`
    pub fn parse_conditional_uri(&mut self, token: TokenId) -> TokenId {
        todo!("parseConditionalUri")
    }

    /// Dart (line 1160): `Token parseDottedName(Token token)`
    pub fn parse_dotted_name(&mut self, token: TokenId) -> TokenId {
        todo!("parseDottedName")
    }

    /// Dart (line 1180): `Token parseExport(Token exportKeyword)`
    pub fn parse_export(&mut self, export_keyword: TokenId) -> TokenId {
        todo!("parseExport")
    }

    /// Dart (line 1197): `Token parseCombinatorStar(Token token)`
    pub fn parse_combinator_star(&mut self, token: TokenId) -> TokenId {
        todo!("parseCombinatorStar")
    }

    /// Dart (line 1222): `Token parseHide(Token token)`
    pub fn parse_hide(&mut self, token: TokenId) -> TokenId {
        todo!("parseHide")
    }

    /// Dart (line 1236): `Token parseShow(Token token)`
    pub fn parse_show(&mut self, token: TokenId) -> TokenId {
        todo!("parseShow")
    }

    /// Dart (line 1250): `Token parseIdentifierList(Token token)`
    pub fn parse_identifier_list(&mut self, token: TokenId) -> TokenId {
        todo!("parseIdentifierList")
    }

    /// Dart (line 1266): `Token parseTypeList(Token token)`
    pub fn parse_type_list(&mut self, token: TokenId) -> TokenId {
        todo!("parseTypeList")
    }

    /// Dart (line 1284): `Token parsePartOrPartOf(Token partKeyword, DirectiveContext? directiveState)`
    pub fn parse_part_or_part_of(&mut self, part_keyword: TokenId, directive_state: Option<&mut DirectiveContext>) -> TokenId {
        todo!("parsePartOrPartOf")
    }

    /// Dart (line 1301): `Token parsePart(Token partKeyword)`
    pub fn parse_part(&mut self, part_keyword: TokenId) -> TokenId {
        todo!("parsePart")
    }

    /// Dart (line 1315): `Token parsePartOf(Token partKeyword)`
    pub fn parse_part_of(&mut self, part_keyword: TokenId) -> TokenId {
        todo!("parsePartOf")
    }

    /// Dart (line 1341): `Token parseMetadataStar(Token token)`
    pub fn parse_metadata_star(&mut self, token: TokenId) -> TokenId {
        todo!("parseMetadataStar")
    }

    /// Dart (line 1390): `Token parseMetadata(Token token)`
    pub fn parse_metadata(&mut self, token: TokenId) -> TokenId {
        todo!("parseMetadata")
    }

    /// Dart (line 1422): `Token parseScript(Token token)`
    pub fn parse_script(&mut self, token: TokenId) -> TokenId {
        todo!("parseScript")
    }

    /// Dart (line 1450): `Token parseTypedef(Token? augmentToken, Token typedefKeyword)`
    pub fn parse_typedef(&mut self, augment_token: Option<TokenId>, typedef_keyword: TokenId) -> TokenId {
        todo!("parseTypedef")
    }

    /// Dart (line 1616): `Token parseMixinApplicationRest(Token token)`
    pub fn parse_mixin_application_rest(&mut self, token: TokenId) -> TokenId {
        todo!("parseMixinApplicationRest")
    }

    /// Dart (line 1634): `Token parseClassWithClauseOpt(Token token)`
    pub fn parse_class_with_clause_opt(&mut self, token: TokenId) -> TokenId {
        todo!("parseClassWithClauseOpt")
    }

    /// Dart (line 1646): `Token parseEnumWithClauseOpt(Token token)`
    pub fn parse_enum_with_clause_opt(&mut self, token: TokenId) -> TokenId {
        todo!("parseEnumWithClauseOpt")
    }

    /// Dart (line 1660): `Token parseGetterOrFormalParameters( Token token, Token name, bool isGetter, MemberKind kind, )`
    pub fn parse_getter_or_formal_parameters(&mut self, token: TokenId, name: TokenId, is_getter: bool, kind: MemberKind) -> TokenId {
        todo!("parseGetterOrFormalParameters")
    }

    /// Dart (line 1691): `Token parseFormalParametersOpt(Token token, MemberKind kind)`
    pub fn parse_formal_parameters_opt(&mut self, token: TokenId, kind: MemberKind) -> TokenId {
        todo!("parseFormalParametersOpt")
    }

    /// Dart (line 1701): `Token skipFormalParameters(Token token, MemberKind kind)`
    pub fn skip_formal_parameters(&mut self, token: TokenId, kind: MemberKind) -> TokenId {
        todo!("skipFormalParameters")
    }

    /// Dart (line 1705): `Token skipFormalParametersRest(Token token, MemberKind kind)`
    pub fn skip_formal_parameters_rest(&mut self, token: TokenId, kind: MemberKind) -> TokenId {
        todo!("skipFormalParametersRest")
    }

    /// Dart (line 1727): `Token parseRecordType( Token start, Token token, bool isQuestionMarkPartOfType, )`
    pub fn parse_record_type(&mut self, start: TokenId, token: TokenId, is_question_mark_part_of_type: bool) -> TokenId {
        todo!("parseRecordType")
    }

    /// Dart (line 1831): `Token parseRecordTypeField( Token token,`
    pub fn parse_record_type_field(&mut self, token: TokenId, identifier_is_optional: bool) -> TokenId {
        todo!("parseRecordTypeField")
    }

    /// Dart (line 1850): `Token parseRecordTypeNamedFields(Token token)`
    pub fn parse_record_type_named_fields(&mut self, token: TokenId) -> TokenId {
        todo!("parseRecordTypeNamedFields")
    }

    /// Dart (line 1893): `Token parseFormalParametersRequiredOpt(Token token, MemberKind kind)`
    pub fn parse_formal_parameters_required_opt(&mut self, token: TokenId, kind: MemberKind) -> TokenId {
        todo!("parseFormalParametersRequiredOpt")
    }

    /// Dart (line 1907): `Token parseFormalParameters(Token token, MemberKind kind)`
    pub fn parse_formal_parameters(&mut self, token: TokenId, kind: MemberKind) -> TokenId {
        todo!("parseFormalParameters")
    }

    /// Dart (line 1916): `Token parseFormalParametersRest(Token token, MemberKind kind)`
    pub fn parse_formal_parameters_rest(&mut self, token: TokenId, kind: MemberKind) -> TokenId {
        todo!("parseFormalParametersRest")
    }

    /// Dart (line 1984): `codes.Message missingParameterMessage(MemberKind kind)`
    pub fn missing_parameter_message(&mut self, kind: MemberKind) -> CfeMessage {
        todo!("missingParameterMessage")
    }

    /// Dart (line 2029): `Token parseFormalParameter( Token token, FormalParameterKind parameterKind, MemberKind memberKind, )`
    pub fn parse_formal_parameter(&mut self, token: TokenId, parameter_kind: FormalParameterKind, member_kind: MemberKind) -> TokenId {
        todo!("parseFormalParameter")
    }

    /// Dart (line 2398): `Token parseOptionalPositionalParameters(Token token, MemberKind kind)`
    pub fn parse_optional_positional_parameters(&mut self, token: TokenId, kind: MemberKind) -> TokenId {
        todo!("parseOptionalPositionalParameters")
    }

    /// Dart (line 2462): `Token parseOptionalNamedParameters(Token token, MemberKind kind)`
    pub fn parse_optional_named_parameters(&mut self, token: TokenId, kind: MemberKind) -> TokenId {
        todo!("parseOptionalNamedParameters")
    }

}
