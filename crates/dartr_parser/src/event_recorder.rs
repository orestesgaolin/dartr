// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/listener.dart
//
// GENERATED FILE. DO NOT EDIT. Run `python3 tools/codegen/gen_listener.py`.


//! A listener that writes every call as JSON: `["eventName", arg, ...]`.
//! Used by `dartr dump events`; the oracle writes the same JSON with
//! `tools/oracle/bin/event_recorder.g.dart`.
//!
//! Arguments: a token is `[offset, lexeme]`, `null` for no token; an error
//! token is `[offset, errorCode]`; a message is
//! `{"code": name, "msg": problem, "fix": correction}` (`fix` only when
//! there is one); enums are their Dart names.

#![allow(unused_variables)]

use dartr_diagnostics::cfe::CfeMessage;
use dartr_syntax::{TokenId, Tokens};

use crate::assert::Assert;
use crate::block_kind::BlockKind;
use crate::constructor_reference_context::ConstructorReferenceContext;
use crate::declaration_kind::{DeclarationHeaderKind, DeclarationKind};
use crate::experimental_features::ExperimentalFlag;
use crate::formal_parameter_kind::FormalParameterKind;
use crate::identifier_context::IdentifierContext;
use crate::listener::Listener;
use crate::member_kind::MemberKind;
use dartr_diagnostics::cfe::CfeCode;

/// Records listener calls as a JSON array (without the brackets) in
/// [`EventRecorder::out`], and the recoverable errors as
/// `[code, offset, length]` in [`EventRecorder::errors`].
#[derive(Default)]
pub struct EventRecorder {
    pub out: String,
    pub errors: String,
    first: bool,
    count: usize,
}

impl EventRecorder {
    pub fn new() -> Self {
        EventRecorder {
            out: String::new(),
            errors: String::new(),
            first: true,
            count: 0,
        }
    }

    /// The number of recorded events.
    pub fn count(&self) -> usize {
        self.count
    }

    fn begin(&mut self, name: &str) {
        if self.count > 0 {
            self.out.push(',');
        }
        self.count += 1;
        self.out.push_str("[\"");
        self.out.push_str(name);
        self.out.push('"');
    }

    fn end(&mut self) {
        self.out.push(']');
    }

    fn token(&mut self, tokens: &Tokens, token: TokenId) {
        self.out.push(',');
        write_token(&mut self.out, tokens, token);
    }

    fn opt_token(&mut self, tokens: &Tokens, token: Option<TokenId>) {
        match token {
            Some(t) => self.token(tokens, t),
            None => self.out.push_str(",null"),
        }
    }

    fn error_token(&mut self, tokens: &Tokens, token: TokenId) {
        use std::fmt::Write;
        let error = tokens.error(token).expect("not an error token");
        let _ = write!(self.out, ",[{},", error.char_offset);
        write_json_string(&mut self.out, error.error_code().name());
        self.out.push(']');
    }

    fn int(&mut self, value: i64) {
        use std::fmt::Write;
        let _ = write!(self.out, ",{value}");
    }

    fn bool(&mut self, value: bool) {
        self.out.push_str(if value { ",true" } else { ",false" });
    }

    fn string(&mut self, value: &str) {
        self.out.push(',');
        write_json_string(&mut self.out, value);
    }

    fn opt_string(&mut self, value: Option<&str>) {
        match value {
            Some(s) => self.string(s),
            None => self.out.push_str(",null"),
        }
    }

    fn message(&mut self, message: &CfeMessage) {
        self.out.push_str(",{\"code\":");
        write_json_string(&mut self.out, message.code.name);
        self.out.push_str(",\"msg\":");
        write_json_string(&mut self.out, &message.problem_message);
        if let Some(fix) = &message.correction_message {
            self.out.push_str(",\"fix\":");
            write_json_string(&mut self.out, fix);
        }
        self.out.push('}');
    }

    /// Dart `ParserError.fromTokens`.
    fn record_error(&mut self, tokens: &Tokens, message: &CfeMessage, start: TokenId, end: TokenId) {
        use std::fmt::Write;
        if !self.first {
            self.errors.push(',');
        }
        self.first = false;
        let begin = tokens.get(start).offset as i64;
        let end = tokens.get(end).end() as i64;
        self.errors.push('[');
        write_json_string(&mut self.errors, message.code.name);
        let _ = write!(self.errors, ",{},{}]", begin, end - begin);
    }
}

/// Writes a token as `[offset, lexeme]`. The offset is signed (the
/// synthetic token before the first token has offset -1).
pub fn write_token(out: &mut String, tokens: &Tokens, token: TokenId) {
    use std::fmt::Write;
    let _ = write!(out, "[{},", tokens.get(token).offset as i32);
    write_json_string(out, tokens.lexeme(token));
    out.push(']');
}

/// Writes [s] as a JSON string like Dart `jsonEncode`.
pub fn write_json_string(out: &mut String, s: &str) {
    use std::fmt::Write;
    out.push('"');
    let bytes = s.as_bytes();
    let mut start = 0;
    for (i, &b) in bytes.iter().enumerate() {
        if b >= 0x20 && b != b'"' && b != b'\\' {
            continue;
        }
        out.push_str(&s[start..i]);
        start = i + 1;
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            8 => out.push_str("\\b"),
            9 => out.push_str("\\t"),
            10 => out.push_str("\\n"),
            12 => out.push_str("\\f"),
            13 => out.push_str("\\r"),
            _ => {
                let _ = write!(out, "\\u{:04x}", b);
            }
        }
    }
    out.push_str(&s[start..]);
    out.push('"');
}

impl Listener for EventRecorder {
    fn begin_arguments(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginArguments");
        self.token(tokens, token);
        self.end();
    }

    fn end_arguments(&mut self, tokens: &mut Tokens, count: i32, begin_token: TokenId, end_token: TokenId) {
        self.begin("endArguments");
        self.int(count as i64);
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_object_pattern_fields(&mut self, tokens: &mut Tokens, count: i32, begin_token: TokenId, end_token: TokenId) {
        self.begin("handleObjectPatternFields");
        self.int(count as i64);
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_async_modifier(&mut self, tokens: &mut Tokens, async_token: Option<TokenId>, star_token: Option<TokenId>) {
        self.begin("handleAsyncModifier");
        self.opt_token(tokens, async_token);
        self.opt_token(tokens, star_token);
        self.end();
    }

    fn begin_await_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginAwaitExpression");
        self.token(tokens, token);
        self.end();
    }

    fn end_await_expression(&mut self, tokens: &mut Tokens, begin_token: TokenId, end_token: TokenId) {
        self.begin("endAwaitExpression");
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn end_invalid_await_expression(&mut self, tokens: &mut Tokens, begin_token: TokenId, end_token: TokenId, error_code: &'static CfeCode) {
        self.begin("endInvalidAwaitExpression");
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.string(error_code.name);
        self.end();
    }

    fn begin_block(&mut self, tokens: &mut Tokens, token: TokenId, block_kind: BlockKind) {
        self.begin("beginBlock");
        self.token(tokens, token);
        self.string(block_kind.name());
        self.end();
    }

    fn end_block(&mut self, tokens: &mut Tokens, count: i32, begin_token: TokenId, end_token: TokenId, block_kind: BlockKind) {
        self.begin("endBlock");
        self.int(count as i64);
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.string(block_kind.name());
        self.end();
    }

    fn handle_invalid_top_level_block(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleInvalidTopLevelBlock");
        self.token(tokens, token);
        self.end();
    }

    fn begin_cascade(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginCascade");
        self.token(tokens, token);
        self.end();
    }

    fn end_cascade(&mut self, tokens: &mut Tokens) {
        self.begin("endCascade");
        self.end();
    }

    fn begin_case_expression(&mut self, tokens: &mut Tokens, case_keyword: TokenId) {
        self.begin("beginCaseExpression");
        self.token(tokens, case_keyword);
        self.end();
    }

    fn end_case_expression(&mut self, tokens: &mut Tokens, case_keyword: TokenId, when: Option<TokenId>, colon: TokenId) {
        self.begin("endCaseExpression");
        self.token(tokens, case_keyword);
        self.opt_token(tokens, when);
        self.token(tokens, colon);
        self.end();
    }

    fn begin_class_or_mixin_or_extension_body(&mut self, tokens: &mut Tokens, kind: DeclarationKind, token: TokenId) {
        self.begin("beginClassOrMixinOrExtensionBody");
        self.string(kind.name());
        self.token(tokens, token);
        self.end();
    }

    fn end_class_or_mixin_or_extension_body(&mut self, tokens: &mut Tokens, kind: DeclarationKind, member_count: i32, begin_token: TokenId, end_token: TokenId) {
        self.begin("endClassOrMixinOrExtensionBody");
        self.string(kind.name());
        self.int(member_count as i64);
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_class_or_mixin_or_named_mixin_application_prelude(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginClassOrMixinOrNamedMixinApplicationPrelude");
        self.token(tokens, token);
        self.end();
    }

    fn begin_class_declaration(&mut self, tokens: &mut Tokens, begin: TokenId, abstract_token: Option<TokenId>, sealed_token: Option<TokenId>, base_token: Option<TokenId>, interface_token: Option<TokenId>, final_token: Option<TokenId>, augment_token: Option<TokenId>, mixin_token: Option<TokenId>, name: TokenId) {
        self.begin("beginClassDeclaration");
        self.token(tokens, begin);
        self.opt_token(tokens, abstract_token);
        self.opt_token(tokens, sealed_token);
        self.opt_token(tokens, base_token);
        self.opt_token(tokens, interface_token);
        self.opt_token(tokens, final_token);
        self.opt_token(tokens, augment_token);
        self.opt_token(tokens, mixin_token);
        self.token(tokens, name);
        self.end();
    }

    fn handle_class_extends(&mut self, tokens: &mut Tokens, extends_keyword: Option<TokenId>, type_count: i32) {
        self.begin("handleClassExtends");
        self.opt_token(tokens, extends_keyword);
        self.int(type_count as i64);
        self.end();
    }

    fn handle_implements(&mut self, tokens: &mut Tokens, implements_keyword: Option<TokenId>, interfaces_count: i32) {
        self.begin("handleImplements");
        self.opt_token(tokens, implements_keyword);
        self.int(interfaces_count as i64);
        self.end();
    }

    fn handle_class_header(&mut self, tokens: &mut Tokens, begin: TokenId, class_keyword: TokenId, native_token: Option<TokenId>) {
        self.begin("handleClassHeader");
        self.token(tokens, begin);
        self.token(tokens, class_keyword);
        self.opt_token(tokens, native_token);
        self.end();
    }

    fn handle_recover_declaration_header(&mut self, tokens: &mut Tokens, kind: DeclarationHeaderKind) {
        self.begin("handleRecoverDeclarationHeader");
        self.string(kind.name());
        self.end();
    }

    fn end_class_declaration(&mut self, tokens: &mut Tokens, begin_token: TokenId, end_token: TokenId) {
        self.begin("endClassDeclaration");
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_no_class_body(&mut self, tokens: &mut Tokens, semicolon_token: TokenId) {
        self.begin("handleNoClassBody");
        self.token(tokens, semicolon_token);
        self.end();
    }

    fn handle_no_extension_type_body(&mut self, tokens: &mut Tokens, semicolon_token: TokenId) {
        self.begin("handleNoExtensionTypeBody");
        self.token(tokens, semicolon_token);
        self.end();
    }

    fn begin_mixin_declaration(&mut self, tokens: &mut Tokens, begin_token: TokenId, augment_token: Option<TokenId>, base_token: Option<TokenId>, mixin_keyword: TokenId, name: TokenId) {
        self.begin("beginMixinDeclaration");
        self.token(tokens, begin_token);
        self.opt_token(tokens, augment_token);
        self.opt_token(tokens, base_token);
        self.token(tokens, mixin_keyword);
        self.token(tokens, name);
        self.end();
    }

    fn handle_mixin_on(&mut self, tokens: &mut Tokens, on_keyword: Option<TokenId>, type_count: i32) {
        self.begin("handleMixinOn");
        self.opt_token(tokens, on_keyword);
        self.int(type_count as i64);
        self.end();
    }

    fn handle_mixin_header(&mut self, tokens: &mut Tokens, mixin_keyword: TokenId) {
        self.begin("handleMixinHeader");
        self.token(tokens, mixin_keyword);
        self.end();
    }

    fn handle_recover_mixin_header(&mut self, tokens: &mut Tokens) {
        self.begin("handleRecoverMixinHeader");
        self.end();
    }

    fn handle_no_mixin_body(&mut self, tokens: &mut Tokens, semicolon_token: TokenId) {
        self.begin("handleNoMixinBody");
        self.token(tokens, semicolon_token);
        self.end();
    }

    fn end_mixin_declaration(&mut self, tokens: &mut Tokens, begin_token: TokenId, end_token: TokenId) {
        self.begin("endMixinDeclaration");
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_uncategorized_top_level_declaration(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginUncategorizedTopLevelDeclaration");
        self.token(tokens, token);
        self.end();
    }

    fn begin_extension_declaration_prelude(&mut self, tokens: &mut Tokens, extension_keyword: TokenId) {
        self.begin("beginExtensionDeclarationPrelude");
        self.token(tokens, extension_keyword);
        self.end();
    }

    fn begin_extension_declaration(&mut self, tokens: &mut Tokens, augment_token: Option<TokenId>, extension_keyword: TokenId, name: Option<TokenId>) {
        self.begin("beginExtensionDeclaration");
        self.opt_token(tokens, augment_token);
        self.token(tokens, extension_keyword);
        self.opt_token(tokens, name);
        self.end();
    }

    fn end_extension_declaration(&mut self, tokens: &mut Tokens, begin_token: TokenId, extension_keyword: TokenId, on_keyword: Option<TokenId>, end_token: TokenId) {
        self.begin("endExtensionDeclaration");
        self.token(tokens, begin_token);
        self.token(tokens, extension_keyword);
        self.opt_token(tokens, on_keyword);
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_no_extension_body(&mut self, tokens: &mut Tokens, semicolon_token: TokenId) {
        self.begin("handleNoExtensionBody");
        self.token(tokens, semicolon_token);
        self.end();
    }

    fn begin_extension_type_declaration(&mut self, tokens: &mut Tokens, augment_keyword: Option<TokenId>, extension_keyword: TokenId, name: TokenId) {
        self.begin("beginExtensionTypeDeclaration");
        self.opt_token(tokens, augment_keyword);
        self.token(tokens, extension_keyword);
        self.token(tokens, name);
        self.end();
    }

    fn end_extension_type_declaration(&mut self, tokens: &mut Tokens, begin_token: TokenId, augment_token: Option<TokenId>, extension_keyword: TokenId, type_keyword: TokenId, end_token: TokenId) {
        self.begin("endExtensionTypeDeclaration");
        self.token(tokens, begin_token);
        self.opt_token(tokens, augment_token);
        self.token(tokens, extension_keyword);
        self.token(tokens, type_keyword);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_primary_constructor(&mut self, tokens: &mut Tokens, begin_token: TokenId) {
        self.begin("beginPrimaryConstructor");
        self.token(tokens, begin_token);
        self.end();
    }

    fn end_primary_constructor(&mut self, tokens: &mut Tokens, kind: DeclarationKind, begin_token: TokenId, end_token: TokenId, const_keyword: Option<TokenId>, has_constructor_name: bool) {
        self.begin("endPrimaryConstructor");
        self.string(kind.name());
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.opt_token(tokens, const_keyword);
        self.bool(has_constructor_name);
        self.end();
    }

    fn handle_no_primary_constructor(&mut self, tokens: &mut Tokens, kind: DeclarationKind, token: TokenId, const_keyword: Option<TokenId>) {
        self.begin("handleNoPrimaryConstructor");
        self.string(kind.name());
        self.token(tokens, token);
        self.opt_token(tokens, const_keyword);
        self.end();
    }

    fn begin_primary_constructor_body(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginPrimaryConstructorBody");
        self.token(tokens, token);
        self.end();
    }

    fn end_primary_constructor_body(&mut self, tokens: &mut Tokens, begin_token: TokenId, begin_initializers: Option<TokenId>, end_token: TokenId) {
        self.begin("endPrimaryConstructorBody");
        self.token(tokens, begin_token);
        self.opt_token(tokens, begin_initializers);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_combinators(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginCombinators");
        self.token(tokens, token);
        self.end();
    }

    fn end_combinators(&mut self, tokens: &mut Tokens, count: i32) {
        self.begin("endCombinators");
        self.int(count as i64);
        self.end();
    }

    fn begin_compilation_unit(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginCompilationUnit");
        self.token(tokens, token);
        self.end();
    }

    fn handle_directives_only(&mut self, tokens: &mut Tokens) {
        self.begin("handleDirectivesOnly");
        self.end();
    }

    fn end_compilation_unit(&mut self, tokens: &mut Tokens, count: i32, token: TokenId) {
        self.begin("endCompilationUnit");
        self.int(count as i64);
        self.token(tokens, token);
        self.end();
    }

    fn begin_const_literal(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginConstLiteral");
        self.token(tokens, token);
        self.end();
    }

    fn end_const_literal(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        self.begin("endConstLiteral");
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_constructor_reference(&mut self, tokens: &mut Tokens, start: TokenId) {
        self.begin("beginConstructorReference");
        self.token(tokens, start);
        self.end();
    }

    fn end_constructor_reference(&mut self, tokens: &mut Tokens, start: TokenId, period_before_name: Option<TokenId>, end_token: TokenId, constructor_reference_context: ConstructorReferenceContext) {
        self.begin("endConstructorReference");
        self.token(tokens, start);
        self.opt_token(tokens, period_before_name);
        self.token(tokens, end_token);
        self.string(constructor_reference_context.name());
        self.end();
    }

    fn begin_do_while_statement(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginDoWhileStatement");
        self.token(tokens, token);
        self.end();
    }

    fn end_do_while_statement(&mut self, tokens: &mut Tokens, do_keyword: TokenId, while_keyword: TokenId, end_token: TokenId) {
        self.begin("endDoWhileStatement");
        self.token(tokens, do_keyword);
        self.token(tokens, while_keyword);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_do_while_statement_body(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginDoWhileStatementBody");
        self.token(tokens, token);
        self.end();
    }

    fn end_do_while_statement_body(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("endDoWhileStatementBody");
        self.token(tokens, token);
        self.end();
    }

    fn begin_while_statement_body(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginWhileStatementBody");
        self.token(tokens, token);
        self.end();
    }

    fn end_while_statement_body(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        self.begin("endWhileStatementBody");
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_enum_declaration_prelude(&mut self, tokens: &mut Tokens, enum_keyword: TokenId) {
        self.begin("beginEnumDeclarationPrelude");
        self.token(tokens, enum_keyword);
        self.end();
    }

    fn begin_enum_declaration(&mut self, tokens: &mut Tokens, begin_token: TokenId, augment_token: Option<TokenId>, enum_keyword: TokenId, name: TokenId) {
        self.begin("beginEnumDeclaration");
        self.token(tokens, begin_token);
        self.opt_token(tokens, augment_token);
        self.token(tokens, enum_keyword);
        self.token(tokens, name);
        self.end();
    }

    fn end_enum_declaration(&mut self, tokens: &mut Tokens, begin_token: TokenId, enum_keyword: TokenId, left_brace: TokenId, member_count: i32, end_token: TokenId) {
        self.begin("endEnumDeclaration");
        self.token(tokens, begin_token);
        self.token(tokens, enum_keyword);
        self.token(tokens, left_brace);
        self.int(member_count as i64);
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_enum_elements(&mut self, tokens: &mut Tokens, elements_end_token: TokenId, elements_count: i32) {
        self.begin("handleEnumElements");
        self.token(tokens, elements_end_token);
        self.int(elements_count as i64);
        self.end();
    }

    fn handle_enum_header(&mut self, tokens: &mut Tokens, augment_token: Option<TokenId>, enum_keyword: TokenId, left_brace: TokenId) {
        self.begin("handleEnumHeader");
        self.opt_token(tokens, augment_token);
        self.token(tokens, enum_keyword);
        self.token(tokens, left_brace);
        self.end();
    }

    fn begin_enum_body(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginEnumBody");
        self.token(tokens, token);
        self.end();
    }

    fn end_enum_body(&mut self, tokens: &mut Tokens, begin_token: TokenId, end_token: TokenId) {
        self.begin("endEnumBody");
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_no_enum_body(&mut self, tokens: &mut Tokens, semicolon_token: TokenId) {
        self.begin("handleNoEnumBody");
        self.token(tokens, semicolon_token);
        self.end();
    }

    fn handle_enum_element(&mut self, tokens: &mut Tokens, begin_token: TokenId, augment_token: Option<TokenId>) {
        self.begin("handleEnumElement");
        self.token(tokens, begin_token);
        self.opt_token(tokens, augment_token);
        self.end();
    }

    fn begin_export(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginExport");
        self.token(tokens, token);
        self.end();
    }

    fn end_export(&mut self, tokens: &mut Tokens, export_keyword: TokenId, semicolon: TokenId) {
        self.begin("endExport");
        self.token(tokens, export_keyword);
        self.token(tokens, semicolon);
        self.end();
    }

    fn handle_extraneous_expression(&mut self, tokens: &mut Tokens, token: TokenId, message: CfeMessage) {
        self.begin("handleExtraneousExpression");
        self.token(tokens, token);
        self.message(&message);
        self.end();
    }

    fn handle_expression_statement(&mut self, tokens: &mut Tokens, begin_token: TokenId, end_token: TokenId) {
        self.begin("handleExpressionStatement");
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_factory(&mut self, tokens: &mut Tokens, declaration_kind: DeclarationKind, last_consumed: TokenId, augment_token: Option<TokenId>, external_token: Option<TokenId>, const_token: Option<TokenId>) {
        self.begin("beginFactory");
        self.string(declaration_kind.name());
        self.token(tokens, last_consumed);
        self.opt_token(tokens, augment_token);
        self.opt_token(tokens, external_token);
        self.opt_token(tokens, const_token);
        self.end();
    }

    fn end_factory(&mut self, tokens: &mut Tokens, kind: DeclarationKind, begin_token: TokenId, factory_keyword: TokenId, end_token: TokenId) {
        self.begin("endFactory");
        self.string(kind.name());
        self.token(tokens, begin_token);
        self.token(tokens, factory_keyword);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_formal_parameter(&mut self, tokens: &mut Tokens, token: TokenId, kind: MemberKind, required_token: Option<TokenId>, covariant_token: Option<TokenId>, var_final_or_const: Option<TokenId>) {
        self.begin("beginFormalParameter");
        self.token(tokens, token);
        self.string(kind.name());
        self.opt_token(tokens, required_token);
        self.opt_token(tokens, covariant_token);
        self.opt_token(tokens, var_final_or_const);
        self.end();
    }

    fn end_formal_parameter(&mut self, tokens: &mut Tokens, var_or_final: Option<TokenId>, this_keyword: Option<TokenId>, super_keyword: Option<TokenId>, period_after_this_or_super: Option<TokenId>, name_token: TokenId, initializer_start: Option<TokenId>, initializer_end: Option<TokenId>, kind: FormalParameterKind, member_kind: MemberKind) {
        self.begin("endFormalParameter");
        self.opt_token(tokens, var_or_final);
        self.opt_token(tokens, this_keyword);
        self.opt_token(tokens, super_keyword);
        self.opt_token(tokens, period_after_this_or_super);
        self.token(tokens, name_token);
        self.opt_token(tokens, initializer_start);
        self.opt_token(tokens, initializer_end);
        self.string(kind.name());
        self.string(member_kind.name());
        self.end();
    }

    fn handle_no_formal_parameters(&mut self, tokens: &mut Tokens, token: TokenId, kind: MemberKind) {
        self.begin("handleNoFormalParameters");
        self.token(tokens, token);
        self.string(kind.name());
        self.end();
    }

    fn begin_formal_parameters(&mut self, tokens: &mut Tokens, token: TokenId, kind: MemberKind) {
        self.begin("beginFormalParameters");
        self.token(tokens, token);
        self.string(kind.name());
        self.end();
    }

    fn end_formal_parameters(&mut self, tokens: &mut Tokens, count: i32, begin_token: TokenId, end_token: TokenId, kind: MemberKind) {
        self.begin("endFormalParameters");
        self.int(count as i64);
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.string(kind.name());
        self.end();
    }

    fn end_fields(&mut self, tokens: &mut Tokens, kind: DeclarationKind, abstract_token: Option<TokenId>, augment_token: Option<TokenId>, external_token: Option<TokenId>, static_token: Option<TokenId>, covariant_token: Option<TokenId>, late_token: Option<TokenId>, var_final_or_const: Option<TokenId>, count: i32, begin_token: TokenId, end_token: TokenId) {
        self.begin("endFields");
        self.string(kind.name());
        self.opt_token(tokens, abstract_token);
        self.opt_token(tokens, augment_token);
        self.opt_token(tokens, external_token);
        self.opt_token(tokens, static_token);
        self.opt_token(tokens, covariant_token);
        self.opt_token(tokens, late_token);
        self.opt_token(tokens, var_final_or_const);
        self.int(count as i64);
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_for_initializer_empty_statement(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleForInitializerEmptyStatement");
        self.token(tokens, token);
        self.end();
    }

    fn handle_for_initializer_expression_statement(&mut self, tokens: &mut Tokens, token: TokenId, for_in: bool) {
        self.begin("handleForInitializerExpressionStatement");
        self.token(tokens, token);
        self.bool(for_in);
        self.end();
    }

    fn handle_for_initializer_local_variable_declaration(&mut self, tokens: &mut Tokens, token: TokenId, for_in: bool) {
        self.begin("handleForInitializerLocalVariableDeclaration");
        self.token(tokens, token);
        self.bool(for_in);
        self.end();
    }

    fn handle_for_initializer_pattern_variable_assignment(&mut self, tokens: &mut Tokens, keyword: TokenId, equals: TokenId) {
        self.begin("handleForInitializerPatternVariableAssignment");
        self.token(tokens, keyword);
        self.token(tokens, equals);
        self.end();
    }

    fn begin_for_statement(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginForStatement");
        self.token(tokens, token);
        self.end();
    }

    fn handle_for_loop_parts(&mut self, tokens: &mut Tokens, for_keyword: TokenId, left_paren: TokenId, left_separator: TokenId, right_separator: TokenId, update_expression_count: i32) {
        self.begin("handleForLoopParts");
        self.token(tokens, for_keyword);
        self.token(tokens, left_paren);
        self.token(tokens, left_separator);
        self.token(tokens, right_separator);
        self.int(update_expression_count as i64);
        self.end();
    }

    fn end_for_statement(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        self.begin("endForStatement");
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_for_statement_body(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginForStatementBody");
        self.token(tokens, token);
        self.end();
    }

    fn end_for_statement_body(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        self.begin("endForStatementBody");
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_for_in_loop_parts(&mut self, tokens: &mut Tokens, await_token: Option<TokenId>, for_token: TokenId, left_parenthesis: TokenId, pattern_keyword: Option<TokenId>, in_keyword: TokenId) {
        self.begin("handleForInLoopParts");
        self.opt_token(tokens, await_token);
        self.token(tokens, for_token);
        self.token(tokens, left_parenthesis);
        self.opt_token(tokens, pattern_keyword);
        self.token(tokens, in_keyword);
        self.end();
    }

    fn end_for_in(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        self.begin("endForIn");
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_for_in_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginForInExpression");
        self.token(tokens, token);
        self.end();
    }

    fn end_for_in_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("endForInExpression");
        self.token(tokens, token);
        self.end();
    }

    fn begin_for_in_body(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginForInBody");
        self.token(tokens, token);
        self.end();
    }

    fn end_for_in_body(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        self.begin("endForInBody");
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_named_function_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginNamedFunctionExpression");
        self.token(tokens, token);
        self.end();
    }

    fn end_named_function_expression(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        self.begin("endNamedFunctionExpression");
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_local_function_declaration(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginLocalFunctionDeclaration");
        self.token(tokens, token);
        self.end();
    }

    fn end_local_function_declaration(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        self.begin("endLocalFunctionDeclaration");
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_block_function_body(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginBlockFunctionBody");
        self.token(tokens, token);
        self.end();
    }

    fn end_block_function_body(&mut self, tokens: &mut Tokens, count: i32, begin_token: TokenId, end_token: TokenId) {
        self.begin("endBlockFunctionBody");
        self.int(count as i64);
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_no_function_body(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleNoFunctionBody");
        self.token(tokens, token);
        self.end();
    }

    fn handle_function_body_skipped(&mut self, tokens: &mut Tokens, begin_token: TokenId, end_token: TokenId, is_expression_body: bool) {
        self.begin("handleFunctionBodySkipped");
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.bool(is_expression_body);
        self.end();
    }

    fn begin_function_name(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginFunctionName");
        self.token(tokens, token);
        self.end();
    }

    fn end_function_name(&mut self, tokens: &mut Tokens, begin_token: TokenId, token: TokenId, is_function_expression: bool) {
        self.begin("endFunctionName");
        self.token(tokens, begin_token);
        self.token(tokens, token);
        self.bool(is_function_expression);
        self.end();
    }

    fn begin_typedef(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginTypedef");
        self.token(tokens, token);
        self.end();
    }

    fn end_typedef(&mut self, tokens: &mut Tokens, augment_token: Option<TokenId>, typedef_keyword: TokenId, equals: Option<TokenId>, end_token: TokenId) {
        self.begin("endTypedef");
        self.opt_token(tokens, augment_token);
        self.token(tokens, typedef_keyword);
        self.opt_token(tokens, equals);
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_class_with_clause(&mut self, tokens: &mut Tokens, with_keyword: TokenId) {
        self.begin("handleClassWithClause");
        self.token(tokens, with_keyword);
        self.end();
    }

    fn handle_class_no_with_clause(&mut self, tokens: &mut Tokens) {
        self.begin("handleClassNoWithClause");
        self.end();
    }

    fn handle_enum_with_clause(&mut self, tokens: &mut Tokens, with_keyword: TokenId) {
        self.begin("handleEnumWithClause");
        self.token(tokens, with_keyword);
        self.end();
    }

    fn handle_enum_no_with_clause(&mut self, tokens: &mut Tokens) {
        self.begin("handleEnumNoWithClause");
        self.end();
    }

    fn handle_mixin_with_clause(&mut self, tokens: &mut Tokens, with_keyword: TokenId) {
        self.begin("handleMixinWithClause");
        self.token(tokens, with_keyword);
        self.end();
    }

    fn begin_named_mixin_application(&mut self, tokens: &mut Tokens, begin_token: TokenId, abstract_token: Option<TokenId>, sealed_token: Option<TokenId>, base_token: Option<TokenId>, interface_token: Option<TokenId>, final_token: Option<TokenId>, augment_token: Option<TokenId>, mixin_token: Option<TokenId>, name: TokenId) {
        self.begin("beginNamedMixinApplication");
        self.token(tokens, begin_token);
        self.opt_token(tokens, abstract_token);
        self.opt_token(tokens, sealed_token);
        self.opt_token(tokens, base_token);
        self.opt_token(tokens, interface_token);
        self.opt_token(tokens, final_token);
        self.opt_token(tokens, augment_token);
        self.opt_token(tokens, mixin_token);
        self.token(tokens, name);
        self.end();
    }

    fn handle_named_mixin_application_with_clause(&mut self, tokens: &mut Tokens, with_keyword: TokenId) {
        self.begin("handleNamedMixinApplicationWithClause");
        self.token(tokens, with_keyword);
        self.end();
    }

    fn end_named_mixin_application(&mut self, tokens: &mut Tokens, begin: TokenId, class_keyword: TokenId, equals: TokenId, implements_keyword: Option<TokenId>, end_token: TokenId) {
        self.begin("endNamedMixinApplication");
        self.token(tokens, begin);
        self.token(tokens, class_keyword);
        self.token(tokens, equals);
        self.opt_token(tokens, implements_keyword);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_hide(&mut self, tokens: &mut Tokens, hide_keyword: TokenId) {
        self.begin("beginHide");
        self.token(tokens, hide_keyword);
        self.end();
    }

    fn end_hide(&mut self, tokens: &mut Tokens, hide_keyword: TokenId) {
        self.begin("endHide");
        self.token(tokens, hide_keyword);
        self.end();
    }

    fn handle_identifier_list(&mut self, tokens: &mut Tokens, count: i32) {
        self.begin("handleIdentifierList");
        self.int(count as i64);
        self.end();
    }

    fn begin_type_list(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginTypeList");
        self.token(tokens, token);
        self.end();
    }

    fn end_type_list(&mut self, tokens: &mut Tokens, count: i32) {
        self.begin("endTypeList");
        self.int(count as i64);
        self.end();
    }

    fn begin_if_statement(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginIfStatement");
        self.token(tokens, token);
        self.end();
    }

    fn end_if_statement(&mut self, tokens: &mut Tokens, if_token: TokenId, else_token: Option<TokenId>, end_token: TokenId) {
        self.begin("endIfStatement");
        self.token(tokens, if_token);
        self.opt_token(tokens, else_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_then_statement(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginThenStatement");
        self.token(tokens, token);
        self.end();
    }

    fn end_then_statement(&mut self, tokens: &mut Tokens, begin_token: TokenId, end_token: TokenId) {
        self.begin("endThenStatement");
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_else_statement(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginElseStatement");
        self.token(tokens, token);
        self.end();
    }

    fn end_else_statement(&mut self, tokens: &mut Tokens, begin_token: TokenId, end_token: TokenId) {
        self.begin("endElseStatement");
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_import(&mut self, tokens: &mut Tokens, import_keyword: TokenId) {
        self.begin("beginImport");
        self.token(tokens, import_keyword);
        self.end();
    }

    fn handle_import_prefix(&mut self, tokens: &mut Tokens, deferred_keyword: Option<TokenId>, as_keyword: Option<TokenId>) {
        self.begin("handleImportPrefix");
        self.opt_token(tokens, deferred_keyword);
        self.opt_token(tokens, as_keyword);
        self.end();
    }

    fn end_import(&mut self, tokens: &mut Tokens, import_keyword: TokenId, semicolon: Option<TokenId>) {
        self.begin("endImport");
        self.token(tokens, import_keyword);
        self.opt_token(tokens, semicolon);
        self.end();
    }

    fn handle_recover_import(&mut self, tokens: &mut Tokens, semicolon: Option<TokenId>) {
        self.begin("handleRecoverImport");
        self.opt_token(tokens, semicolon);
        self.end();
    }

    fn begin_conditional_uris(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginConditionalUris");
        self.token(tokens, token);
        self.end();
    }

    fn end_conditional_uris(&mut self, tokens: &mut Tokens, count: i32) {
        self.begin("endConditionalUris");
        self.int(count as i64);
        self.end();
    }

    fn begin_conditional_uri(&mut self, tokens: &mut Tokens, if_keyword: TokenId) {
        self.begin("beginConditionalUri");
        self.token(tokens, if_keyword);
        self.end();
    }

    fn end_conditional_uri(&mut self, tokens: &mut Tokens, if_keyword: TokenId, left_paren: TokenId, equal_sign: Option<TokenId>) {
        self.begin("endConditionalUri");
        self.token(tokens, if_keyword);
        self.token(tokens, left_paren);
        self.opt_token(tokens, equal_sign);
        self.end();
    }

    fn handle_dotted_name(&mut self, tokens: &mut Tokens, count: i32, first_identifier: TokenId) {
        self.begin("handleDottedName");
        self.int(count as i64);
        self.token(tokens, first_identifier);
        self.end();
    }

    fn begin_implicit_creation_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginImplicitCreationExpression");
        self.token(tokens, token);
        self.end();
    }

    fn end_implicit_creation_expression(&mut self, tokens: &mut Tokens, token: TokenId, open_angle_bracket: TokenId) {
        self.begin("endImplicitCreationExpression");
        self.token(tokens, token);
        self.token(tokens, open_angle_bracket);
        self.end();
    }

    fn begin_initialized_identifier(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginInitializedIdentifier");
        self.token(tokens, token);
        self.end();
    }

    fn end_initialized_identifier(&mut self, tokens: &mut Tokens, name_token: TokenId) {
        self.begin("endInitializedIdentifier");
        self.token(tokens, name_token);
        self.end();
    }

    fn begin_field_initializer(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginFieldInitializer");
        self.token(tokens, token);
        self.end();
    }

    fn end_field_initializer(&mut self, tokens: &mut Tokens, assignment: TokenId, end_token: TokenId) {
        self.begin("endFieldInitializer");
        self.token(tokens, assignment);
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_no_field_initializer(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleNoFieldInitializer");
        self.token(tokens, token);
        self.end();
    }

    fn begin_variable_initializer(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginVariableInitializer");
        self.token(tokens, token);
        self.end();
    }

    fn end_variable_initializer(&mut self, tokens: &mut Tokens, assignment_operator: TokenId) {
        self.begin("endVariableInitializer");
        self.token(tokens, assignment_operator);
        self.end();
    }

    fn handle_no_variable_initializer(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleNoVariableInitializer");
        self.token(tokens, token);
        self.end();
    }

    fn begin_initializer(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginInitializer");
        self.token(tokens, token);
        self.end();
    }

    fn end_initializer(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        self.begin("endInitializer");
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_initializers(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginInitializers");
        self.token(tokens, token);
        self.end();
    }

    fn end_initializers(&mut self, tokens: &mut Tokens, count: i32, begin_token: TokenId, end_token: TokenId) {
        self.begin("endInitializers");
        self.int(count as i64);
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_no_initializers(&mut self, tokens: &mut Tokens) {
        self.begin("handleNoInitializers");
        self.end();
    }

    fn handle_invalid_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleInvalidExpression");
        self.token(tokens, token);
        self.end();
    }

    fn handle_invalid_function_body(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleInvalidFunctionBody");
        self.token(tokens, token);
        self.end();
    }

    fn handle_invalid_type_reference(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleInvalidTypeReference");
        self.token(tokens, token);
        self.end();
    }

    fn handle_label(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleLabel");
        self.token(tokens, token);
        self.end();
    }

    fn begin_labeled_statement(&mut self, tokens: &mut Tokens, token: TokenId, label_count: i32) {
        self.begin("beginLabeledStatement");
        self.token(tokens, token);
        self.int(label_count as i64);
        self.end();
    }

    fn end_labeled_statement(&mut self, tokens: &mut Tokens, label_count: i32) {
        self.begin("endLabeledStatement");
        self.int(label_count as i64);
        self.end();
    }

    fn begin_library_augmentation(&mut self, tokens: &mut Tokens, augment_keyword: TokenId, library_keyword: TokenId) {
        self.begin("beginLibraryAugmentation");
        self.token(tokens, augment_keyword);
        self.token(tokens, library_keyword);
        self.end();
    }

    fn end_library_augmentation(&mut self, tokens: &mut Tokens, augment_keyword: TokenId, library_keyword: TokenId, semicolon: TokenId) {
        self.begin("endLibraryAugmentation");
        self.token(tokens, augment_keyword);
        self.token(tokens, library_keyword);
        self.token(tokens, semicolon);
        self.end();
    }

    fn begin_library_name(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginLibraryName");
        self.token(tokens, token);
        self.end();
    }

    fn end_library_name(&mut self, tokens: &mut Tokens, library_keyword: TokenId, semicolon: TokenId, has_name: bool) {
        self.begin("endLibraryName");
        self.token(tokens, library_keyword);
        self.token(tokens, semicolon);
        self.bool(has_name);
        self.end();
    }

    fn handle_literal_map_entry(&mut self, tokens: &mut Tokens, colon: TokenId, end_token: TokenId, null_aware_key_token: Option<TokenId>, null_aware_value_token: Option<TokenId>) {
        self.begin("handleLiteralMapEntry");
        self.token(tokens, colon);
        self.token(tokens, end_token);
        self.opt_token(tokens, null_aware_key_token);
        self.opt_token(tokens, null_aware_value_token);
        self.end();
    }

    fn handle_map_pattern_entry(&mut self, tokens: &mut Tokens, colon: TokenId, end_token: TokenId) {
        self.begin("handleMapPatternEntry");
        self.token(tokens, colon);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_literal_string(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginLiteralString");
        self.token(tokens, token);
        self.end();
    }

    fn handle_interpolation_expression(&mut self, tokens: &mut Tokens, left_bracket: TokenId, right_bracket: Option<TokenId>) {
        self.begin("handleInterpolationExpression");
        self.token(tokens, left_bracket);
        self.opt_token(tokens, right_bracket);
        self.end();
    }

    fn end_literal_string(&mut self, tokens: &mut Tokens, interpolation_count: i32, end_token: TokenId) {
        self.begin("endLiteralString");
        self.int(interpolation_count as i64);
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_adjacent_string_literals(&mut self, tokens: &mut Tokens, start_token: TokenId, literal_count: i32) {
        self.begin("handleAdjacentStringLiterals");
        self.token(tokens, start_token);
        self.int(literal_count as i64);
        self.end();
    }

    fn begin_member(&mut self, tokens: &mut Tokens) {
        self.begin("beginMember");
        self.end();
    }

    fn handle_invalid_member(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        self.begin("handleInvalidMember");
        self.token(tokens, end_token);
        self.end();
    }

    fn end_member(&mut self, tokens: &mut Tokens) {
        self.begin("endMember");
        self.end();
    }

    fn begin_method(&mut self, tokens: &mut Tokens, declaration_kind: DeclarationKind, augment_token: Option<TokenId>, external_token: Option<TokenId>, static_token: Option<TokenId>, covariant_token: Option<TokenId>, var_final_or_const: Option<TokenId>, get_or_set: Option<TokenId>, name: TokenId, enclosing_declaration_name: Option<&str>) {
        self.begin("beginMethod");
        self.string(declaration_kind.name());
        self.opt_token(tokens, augment_token);
        self.opt_token(tokens, external_token);
        self.opt_token(tokens, static_token);
        self.opt_token(tokens, covariant_token);
        self.opt_token(tokens, var_final_or_const);
        self.opt_token(tokens, get_or_set);
        self.token(tokens, name);
        self.opt_string(enclosing_declaration_name);
        self.end();
    }

    fn end_method(&mut self, tokens: &mut Tokens, kind: DeclarationKind, get_or_set: Option<TokenId>, begin_token: TokenId, begin_param: TokenId, begin_initializers: Option<TokenId>, end_token: TokenId) {
        self.begin("endMethod");
        self.string(kind.name());
        self.opt_token(tokens, get_or_set);
        self.token(tokens, begin_token);
        self.token(tokens, begin_param);
        self.opt_token(tokens, begin_initializers);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_constructor(&mut self, tokens: &mut Tokens, declaration_kind: DeclarationKind, augment_token: Option<TokenId>, external_token: Option<TokenId>, static_token: Option<TokenId>, covariant_token: Option<TokenId>, var_final_or_const: Option<TokenId>, get_or_set: Option<TokenId>, new_token: Option<TokenId>, name: TokenId, enclosing_declaration_name: Option<&str>) {
        self.begin("beginConstructor");
        self.string(declaration_kind.name());
        self.opt_token(tokens, augment_token);
        self.opt_token(tokens, external_token);
        self.opt_token(tokens, static_token);
        self.opt_token(tokens, covariant_token);
        self.opt_token(tokens, var_final_or_const);
        self.opt_token(tokens, get_or_set);
        self.opt_token(tokens, new_token);
        self.token(tokens, name);
        self.opt_string(enclosing_declaration_name);
        self.end();
    }

    fn end_constructor(&mut self, tokens: &mut Tokens, kind: DeclarationKind, begin_token: TokenId, new_token: Option<TokenId>, begin_param: TokenId, begin_initializers: Option<TokenId>, end_token: TokenId) {
        self.begin("endConstructor");
        self.string(kind.name());
        self.token(tokens, begin_token);
        self.opt_token(tokens, new_token);
        self.token(tokens, begin_param);
        self.opt_token(tokens, begin_initializers);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_metadata_star(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginMetadataStar");
        self.token(tokens, token);
        self.end();
    }

    fn end_metadata_star(&mut self, tokens: &mut Tokens, count: i32) {
        self.begin("endMetadataStar");
        self.int(count as i64);
        self.end();
    }

    fn begin_metadata(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginMetadata");
        self.token(tokens, token);
        self.end();
    }

    fn end_metadata(&mut self, tokens: &mut Tokens, begin_token: TokenId, period_before_name: Option<TokenId>, end_token: TokenId) {
        self.begin("endMetadata");
        self.token(tokens, begin_token);
        self.opt_token(tokens, period_before_name);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_optional_formal_parameters(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginOptionalFormalParameters");
        self.token(tokens, token);
        self.end();
    }

    fn end_optional_formal_parameters(&mut self, tokens: &mut Tokens, count: i32, begin_token: TokenId, end_token: TokenId, kind: MemberKind) {
        self.begin("endOptionalFormalParameters");
        self.int(count as i64);
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.string(kind.name());
        self.end();
    }

    fn begin_part(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginPart");
        self.token(tokens, token);
        self.end();
    }

    fn end_part(&mut self, tokens: &mut Tokens, part_keyword: TokenId, semicolon: TokenId) {
        self.begin("endPart");
        self.token(tokens, part_keyword);
        self.token(tokens, semicolon);
        self.end();
    }

    fn begin_part_of(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginPartOf");
        self.token(tokens, token);
        self.end();
    }

    fn end_part_of(&mut self, tokens: &mut Tokens, part_keyword: TokenId, of_keyword: TokenId, semicolon: TokenId, has_name: bool) {
        self.begin("endPartOf");
        self.token(tokens, part_keyword);
        self.token(tokens, of_keyword);
        self.token(tokens, semicolon);
        self.bool(has_name);
        self.end();
    }

    fn begin_redirecting_factory_body(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginRedirectingFactoryBody");
        self.token(tokens, token);
        self.end();
    }

    fn end_redirecting_factory_body(&mut self, tokens: &mut Tokens, begin_token: TokenId, end_token: TokenId) {
        self.begin("endRedirectingFactoryBody");
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_return_statement(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginReturnStatement");
        self.token(tokens, token);
        self.end();
    }

    fn handle_native_function_body(&mut self, tokens: &mut Tokens, native_token: TokenId, semicolon: TokenId) {
        self.begin("handleNativeFunctionBody");
        self.token(tokens, native_token);
        self.token(tokens, semicolon);
        self.end();
    }

    fn handle_native_function_body_ignored(&mut self, tokens: &mut Tokens, native_token: TokenId, semicolon: TokenId) {
        self.begin("handleNativeFunctionBodyIgnored");
        self.token(tokens, native_token);
        self.token(tokens, semicolon);
        self.end();
    }

    fn handle_native_function_body_skipped(&mut self, tokens: &mut Tokens, native_token: TokenId, semicolon: TokenId) {
        self.begin("handleNativeFunctionBodySkipped");
        self.token(tokens, native_token);
        self.token(tokens, semicolon);
        self.end();
    }

    fn handle_empty_function_body(&mut self, tokens: &mut Tokens, semicolon: TokenId) {
        self.begin("handleEmptyFunctionBody");
        self.token(tokens, semicolon);
        self.end();
    }

    fn handle_expression_function_body(&mut self, tokens: &mut Tokens, arrow_token: TokenId, end_token: Option<TokenId>) {
        self.begin("handleExpressionFunctionBody");
        self.token(tokens, arrow_token);
        self.opt_token(tokens, end_token);
        self.end();
    }

    fn end_return_statement(&mut self, tokens: &mut Tokens, has_expression: bool, begin_token: TokenId, end_token: TokenId) {
        self.begin("endReturnStatement");
        self.bool(has_expression);
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_send(&mut self, tokens: &mut Tokens, begin_token: TokenId, end_token: TokenId) {
        self.begin("handleSend");
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_show(&mut self, tokens: &mut Tokens, show_keyword: TokenId) {
        self.begin("beginShow");
        self.token(tokens, show_keyword);
        self.end();
    }

    fn end_show(&mut self, tokens: &mut Tokens, show_keyword: TokenId) {
        self.begin("endShow");
        self.token(tokens, show_keyword);
        self.end();
    }

    fn begin_switch_statement(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginSwitchStatement");
        self.token(tokens, token);
        self.end();
    }

    fn end_switch_statement(&mut self, tokens: &mut Tokens, switch_keyword: TokenId, end_token: TokenId) {
        self.begin("endSwitchStatement");
        self.token(tokens, switch_keyword);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_switch_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginSwitchExpression");
        self.token(tokens, token);
        self.end();
    }

    fn end_switch_expression(&mut self, tokens: &mut Tokens, switch_keyword: TokenId, end_token: TokenId) {
        self.begin("endSwitchExpression");
        self.token(tokens, switch_keyword);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_switch_block(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginSwitchBlock");
        self.token(tokens, token);
        self.end();
    }

    fn end_switch_block(&mut self, tokens: &mut Tokens, case_count: i32, begin_token: TokenId, end_token: TokenId) {
        self.begin("endSwitchBlock");
        self.int(case_count as i64);
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_switch_expression_block(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginSwitchExpressionBlock");
        self.token(tokens, token);
        self.end();
    }

    fn end_switch_expression_block(&mut self, tokens: &mut Tokens, case_count: i32, begin_token: TokenId, end_token: TokenId) {
        self.begin("endSwitchExpressionBlock");
        self.int(case_count as i64);
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_literal_symbol(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginLiteralSymbol");
        self.token(tokens, token);
        self.end();
    }

    fn end_literal_symbol(&mut self, tokens: &mut Tokens, hash_token: TokenId, identifier_count: i32) {
        self.begin("endLiteralSymbol");
        self.token(tokens, hash_token);
        self.int(identifier_count as i64);
        self.end();
    }

    fn handle_throw_expression(&mut self, tokens: &mut Tokens, throw_token: TokenId, end_token: TokenId) {
        self.begin("handleThrowExpression");
        self.token(tokens, throw_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_rethrow_statement(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginRethrowStatement");
        self.token(tokens, token);
        self.end();
    }

    fn end_rethrow_statement(&mut self, tokens: &mut Tokens, rethrow_token: TokenId, end_token: TokenId) {
        self.begin("endRethrowStatement");
        self.token(tokens, rethrow_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn end_top_level_declaration(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        self.begin("endTopLevelDeclaration");
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_invalid_top_level_declaration(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        self.begin("handleInvalidTopLevelDeclaration");
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_top_level_member(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginTopLevelMember");
        self.token(tokens, token);
        self.end();
    }

    fn begin_fields(&mut self, tokens: &mut Tokens, declaration_kind: DeclarationKind, augment_token: Option<TokenId>, abstract_token: Option<TokenId>, external_token: Option<TokenId>, static_token: Option<TokenId>, covariant_token: Option<TokenId>, late_token: Option<TokenId>, var_final_or_const: Option<TokenId>, last_consumed: TokenId) {
        self.begin("beginFields");
        self.string(declaration_kind.name());
        self.opt_token(tokens, augment_token);
        self.opt_token(tokens, abstract_token);
        self.opt_token(tokens, external_token);
        self.opt_token(tokens, static_token);
        self.opt_token(tokens, covariant_token);
        self.opt_token(tokens, late_token);
        self.opt_token(tokens, var_final_or_const);
        self.token(tokens, last_consumed);
        self.end();
    }

    fn end_top_level_fields(&mut self, tokens: &mut Tokens, augment_token: Option<TokenId>, abstract_token: Option<TokenId>, external_token: Option<TokenId>, static_token: Option<TokenId>, covariant_token: Option<TokenId>, late_token: Option<TokenId>, var_final_or_const: Option<TokenId>, count: i32, begin_token: TokenId, end_token: TokenId) {
        self.begin("endTopLevelFields");
        self.opt_token(tokens, augment_token);
        self.opt_token(tokens, abstract_token);
        self.opt_token(tokens, external_token);
        self.opt_token(tokens, static_token);
        self.opt_token(tokens, covariant_token);
        self.opt_token(tokens, late_token);
        self.opt_token(tokens, var_final_or_const);
        self.int(count as i64);
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_top_level_method(&mut self, tokens: &mut Tokens, last_consumed: TokenId, augment_token: Option<TokenId>, external_token: Option<TokenId>) {
        self.begin("beginTopLevelMethod");
        self.token(tokens, last_consumed);
        self.opt_token(tokens, augment_token);
        self.opt_token(tokens, external_token);
        self.end();
    }

    fn end_top_level_method(&mut self, tokens: &mut Tokens, begin_token: TokenId, get_or_set: Option<TokenId>, end_token: TokenId) {
        self.begin("endTopLevelMethod");
        self.token(tokens, begin_token);
        self.opt_token(tokens, get_or_set);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_try_statement(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginTryStatement");
        self.token(tokens, token);
        self.end();
    }

    fn begin_catch_clause(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginCatchClause");
        self.token(tokens, token);
        self.end();
    }

    fn end_catch_clause(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("endCatchClause");
        self.token(tokens, token);
        self.end();
    }

    fn handle_catch_block(&mut self, tokens: &mut Tokens, on_keyword: Option<TokenId>, catch_keyword: Option<TokenId>, comma: Option<TokenId>) {
        self.begin("handleCatchBlock");
        self.opt_token(tokens, on_keyword);
        self.opt_token(tokens, catch_keyword);
        self.opt_token(tokens, comma);
        self.end();
    }

    fn handle_finally_block(&mut self, tokens: &mut Tokens, finally_keyword: TokenId) {
        self.begin("handleFinallyBlock");
        self.token(tokens, finally_keyword);
        self.end();
    }

    fn end_try_statement(&mut self, tokens: &mut Tokens, catch_count: i32, try_keyword: TokenId, finally_keyword: Option<TokenId>, end_token: TokenId) {
        self.begin("endTryStatement");
        self.int(catch_count as i64);
        self.token(tokens, try_keyword);
        self.opt_token(tokens, finally_keyword);
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_type(&mut self, tokens: &mut Tokens, begin_token: TokenId, question_mark: Option<TokenId>) {
        self.begin("handleType");
        self.token(tokens, begin_token);
        self.opt_token(tokens, question_mark);
        self.end();
    }

    fn handle_non_null_assert_expression(&mut self, tokens: &mut Tokens, bang: TokenId) {
        self.begin("handleNonNullAssertExpression");
        self.token(tokens, bang);
        self.end();
    }

    fn handle_null_assert_pattern(&mut self, tokens: &mut Tokens, bang: TokenId) {
        self.begin("handleNullAssertPattern");
        self.token(tokens, bang);
        self.end();
    }

    fn handle_null_check_pattern(&mut self, tokens: &mut Tokens, question: TokenId) {
        self.begin("handleNullCheckPattern");
        self.token(tokens, question);
        self.end();
    }

    fn handle_assigned_variable_pattern(&mut self, tokens: &mut Tokens, variable: TokenId) {
        self.begin("handleAssignedVariablePattern");
        self.token(tokens, variable);
        self.end();
    }

    fn handle_declared_variable_pattern(&mut self, tokens: &mut Tokens, keyword: Option<TokenId>, variable: TokenId, in_assignment_pattern: bool) {
        self.begin("handleDeclaredVariablePattern");
        self.opt_token(tokens, keyword);
        self.token(tokens, variable);
        self.bool(in_assignment_pattern);
        self.end();
    }

    fn handle_wildcard_pattern(&mut self, tokens: &mut Tokens, keyword: Option<TokenId>, wildcard: TokenId) {
        self.begin("handleWildcardPattern");
        self.opt_token(tokens, keyword);
        self.token(tokens, wildcard);
        self.end();
    }

    fn handle_no_name(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleNoName");
        self.token(tokens, token);
        self.end();
    }

    fn begin_record_type(&mut self, tokens: &mut Tokens, left_bracket: TokenId) {
        self.begin("beginRecordType");
        self.token(tokens, left_bracket);
        self.end();
    }

    fn end_record_type(&mut self, tokens: &mut Tokens, left_bracket: TokenId, question_mark: Option<TokenId>, count: i32, has_named_fields: bool) {
        self.begin("endRecordType");
        self.token(tokens, left_bracket);
        self.opt_token(tokens, question_mark);
        self.int(count as i64);
        self.bool(has_named_fields);
        self.end();
    }

    fn begin_record_type_entry(&mut self, tokens: &mut Tokens) {
        self.begin("beginRecordTypeEntry");
        self.end();
    }

    fn end_record_type_entry(&mut self, tokens: &mut Tokens) {
        self.begin("endRecordTypeEntry");
        self.end();
    }

    fn begin_record_type_named_fields(&mut self, tokens: &mut Tokens, left_bracket: TokenId) {
        self.begin("beginRecordTypeNamedFields");
        self.token(tokens, left_bracket);
        self.end();
    }

    fn end_record_type_named_fields(&mut self, tokens: &mut Tokens, count: i32, left_bracket: TokenId) {
        self.begin("endRecordTypeNamedFields");
        self.int(count as i64);
        self.token(tokens, left_bracket);
        self.end();
    }

    fn begin_function_type(&mut self, tokens: &mut Tokens, begin_token: TokenId) {
        self.begin("beginFunctionType");
        self.token(tokens, begin_token);
        self.end();
    }

    fn end_function_type(&mut self, tokens: &mut Tokens, function_token: TokenId, question_mark: Option<TokenId>) {
        self.begin("endFunctionType");
        self.token(tokens, function_token);
        self.opt_token(tokens, question_mark);
        self.end();
    }

    fn begin_type_arguments(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginTypeArguments");
        self.token(tokens, token);
        self.end();
    }

    fn end_type_arguments(&mut self, tokens: &mut Tokens, count: i32, begin_token: TokenId, end_token: TokenId) {
        self.begin("endTypeArguments");
        self.int(count as i64);
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_invalid_type_arguments(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleInvalidTypeArguments");
        self.token(tokens, token);
        self.end();
    }

    fn handle_no_type_arguments(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleNoTypeArguments");
        self.token(tokens, token);
        self.end();
    }

    fn begin_type_variable(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginTypeVariable");
        self.token(tokens, token);
        self.end();
    }

    fn handle_type_variables_defined(&mut self, tokens: &mut Tokens, token: TokenId, count: i32) {
        self.begin("handleTypeVariablesDefined");
        self.token(tokens, token);
        self.int(count as i64);
        self.end();
    }

    fn end_type_variable(&mut self, tokens: &mut Tokens, token: TokenId, index: i32, extends_or_super: Option<TokenId>, variance: Option<TokenId>) {
        self.begin("endTypeVariable");
        self.token(tokens, token);
        self.int(index as i64);
        self.opt_token(tokens, extends_or_super);
        self.opt_token(tokens, variance);
        self.end();
    }

    fn begin_type_variables(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginTypeVariables");
        self.token(tokens, token);
        self.end();
    }

    fn end_type_variables(&mut self, tokens: &mut Tokens, begin_token: TokenId, end_token: TokenId) {
        self.begin("endTypeVariables");
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn report_variance_modifier_not_enabled(&mut self, tokens: &mut Tokens, variance: Option<TokenId>) {
        self.begin("reportVarianceModifierNotEnabled");
        self.opt_token(tokens, variance);
        self.end();
    }

    fn begin_function_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginFunctionExpression");
        self.token(tokens, token);
        self.end();
    }

    fn end_function_expression(&mut self, tokens: &mut Tokens, begin_token: TokenId, end_token: TokenId) {
        self.begin("endFunctionExpression");
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_variables_declaration(&mut self, tokens: &mut Tokens, token: TokenId, late_token: Option<TokenId>, var_final_or_const: Option<TokenId>) {
        self.begin("beginVariablesDeclaration");
        self.token(tokens, token);
        self.opt_token(tokens, late_token);
        self.opt_token(tokens, var_final_or_const);
        self.end();
    }

    fn end_variables_declaration(&mut self, tokens: &mut Tokens, count: i32, end_token: Option<TokenId>) {
        self.begin("endVariablesDeclaration");
        self.int(count as i64);
        self.opt_token(tokens, end_token);
        self.end();
    }

    fn begin_while_statement(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginWhileStatement");
        self.token(tokens, token);
        self.end();
    }

    fn end_while_statement(&mut self, tokens: &mut Tokens, while_keyword: TokenId, end_token: TokenId) {
        self.begin("endWhileStatement");
        self.token(tokens, while_keyword);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_as_operator_type(&mut self, tokens: &mut Tokens, operator: TokenId) {
        self.begin("beginAsOperatorType");
        self.token(tokens, operator);
        self.end();
    }

    fn end_as_operator_type(&mut self, tokens: &mut Tokens, operator: TokenId) {
        self.begin("endAsOperatorType");
        self.token(tokens, operator);
        self.end();
    }

    fn handle_as_operator(&mut self, tokens: &mut Tokens, operator: TokenId) {
        self.begin("handleAsOperator");
        self.token(tokens, operator);
        self.end();
    }

    fn handle_cast_pattern(&mut self, tokens: &mut Tokens, operator: TokenId) {
        self.begin("handleCastPattern");
        self.token(tokens, operator);
        self.end();
    }

    fn handle_assignment_expression(&mut self, tokens: &mut Tokens, token: TokenId, end_token: TokenId) {
        self.begin("handleAssignmentExpression");
        self.token(tokens, token);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_anonymous_method_invocation(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginAnonymousMethodInvocation");
        self.token(tokens, token);
        self.end();
    }

    fn end_anonymous_method_invocation(&mut self, tokens: &mut Tokens, begin_token: TokenId, function_definition: Option<TokenId>, end_token: TokenId, is_expression: bool) {
        self.begin("endAnonymousMethodInvocation");
        self.token(tokens, begin_token);
        self.opt_token(tokens, function_definition);
        self.token(tokens, end_token);
        self.bool(is_expression);
        self.end();
    }

    fn handle_implicit_formal_parameters(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleImplicitFormalParameters");
        self.token(tokens, token);
        self.end();
    }

    fn begin_binary_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginBinaryExpression");
        self.token(tokens, token);
        self.end();
    }

    fn end_binary_expression(&mut self, tokens: &mut Tokens, token: TokenId, end_token: TokenId) {
        self.begin("endBinaryExpression");
        self.token(tokens, token);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_binary_pattern(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginBinaryPattern");
        self.token(tokens, token);
        self.end();
    }

    fn end_binary_pattern(&mut self, tokens: &mut Tokens, operator_token: TokenId) {
        self.begin("endBinaryPattern");
        self.token(tokens, operator_token);
        self.end();
    }

    fn handle_dot_access(&mut self, tokens: &mut Tokens, token: TokenId, end_token: TokenId, is_null_aware: bool) {
        self.begin("handleDotAccess");
        self.token(tokens, token);
        self.token(tokens, end_token);
        self.bool(is_null_aware);
        self.end();
    }

    fn handle_cascade_access(&mut self, tokens: &mut Tokens, token: TokenId, end_token: TokenId, is_null_aware: bool) {
        self.begin("handleCascadeAccess");
        self.token(tokens, token);
        self.token(tokens, end_token);
        self.bool(is_null_aware);
        self.end();
    }

    fn begin_conditional_expression(&mut self, tokens: &mut Tokens, question: TokenId) {
        self.begin("beginConditionalExpression");
        self.token(tokens, question);
        self.end();
    }

    fn handle_conditional_expression_colon(&mut self, tokens: &mut Tokens) {
        self.begin("handleConditionalExpressionColon");
        self.end();
    }

    fn end_conditional_expression(&mut self, tokens: &mut Tokens, question: TokenId, colon: TokenId, end_token: TokenId) {
        self.begin("endConditionalExpression");
        self.token(tokens, question);
        self.token(tokens, colon);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_const_expression(&mut self, tokens: &mut Tokens, const_keyword: TokenId) {
        self.begin("beginConstExpression");
        self.token(tokens, const_keyword);
        self.end();
    }

    fn end_const_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("endConstExpression");
        self.token(tokens, token);
        self.end();
    }

    fn handle_const_factory(&mut self, tokens: &mut Tokens, const_keyword: TokenId) {
        self.begin("handleConstFactory");
        self.token(tokens, const_keyword);
        self.end();
    }

    fn begin_for_control_flow(&mut self, tokens: &mut Tokens, await_token: Option<TokenId>, for_token: TokenId) {
        self.begin("beginForControlFlow");
        self.opt_token(tokens, await_token);
        self.token(tokens, for_token);
        self.end();
    }

    fn end_for_control_flow(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("endForControlFlow");
        self.token(tokens, token);
        self.end();
    }

    fn end_for_in_control_flow(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("endForInControlFlow");
        self.token(tokens, token);
        self.end();
    }

    fn begin_if_control_flow(&mut self, tokens: &mut Tokens, if_token: TokenId) {
        self.begin("beginIfControlFlow");
        self.token(tokens, if_token);
        self.end();
    }

    fn handle_then_control_flow(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleThenControlFlow");
        self.token(tokens, token);
        self.end();
    }

    fn handle_else_control_flow(&mut self, tokens: &mut Tokens, else_token: TokenId) {
        self.begin("handleElseControlFlow");
        self.token(tokens, else_token);
        self.end();
    }

    fn end_if_control_flow(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("endIfControlFlow");
        self.token(tokens, token);
        self.end();
    }

    fn end_if_else_control_flow(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("endIfElseControlFlow");
        self.token(tokens, token);
        self.end();
    }

    fn handle_spread_expression(&mut self, tokens: &mut Tokens, spread_token: TokenId) {
        self.begin("handleSpreadExpression");
        self.token(tokens, spread_token);
        self.end();
    }

    fn handle_null_aware_element(&mut self, tokens: &mut Tokens, null_aware_token: TokenId) {
        self.begin("handleNullAwareElement");
        self.token(tokens, null_aware_token);
        self.end();
    }

    fn handle_rest_pattern(&mut self, tokens: &mut Tokens, dots: TokenId, has_sub_pattern: bool) {
        self.begin("handleRestPattern");
        self.token(tokens, dots);
        self.bool(has_sub_pattern);
        self.end();
    }

    fn begin_function_typed_formal_parameter(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginFunctionTypedFormalParameter");
        self.token(tokens, token);
        self.end();
    }

    fn end_function_typed_formal_parameter(&mut self, tokens: &mut Tokens, name_token: TokenId, question: Option<TokenId>) {
        self.begin("endFunctionTypedFormalParameter");
        self.token(tokens, name_token);
        self.opt_token(tokens, question);
        self.end();
    }

    fn handle_identifier(&mut self, tokens: &mut Tokens, token: TokenId, context: IdentifierContext) {
        self.begin("handleIdentifier");
        self.token(tokens, token);
        self.string(context.name());
        self.end();
    }

    fn handle_indexed_expression(&mut self, tokens: &mut Tokens, question: Option<TokenId>, open_square_bracket: TokenId, close_square_bracket: TokenId) {
        self.begin("handleIndexedExpression");
        self.opt_token(tokens, question);
        self.token(tokens, open_square_bracket);
        self.token(tokens, close_square_bracket);
        self.end();
    }

    fn begin_is_operator_type(&mut self, tokens: &mut Tokens, operator: TokenId) {
        self.begin("beginIsOperatorType");
        self.token(tokens, operator);
        self.end();
    }

    fn end_is_operator_type(&mut self, tokens: &mut Tokens, operator: TokenId) {
        self.begin("endIsOperatorType");
        self.token(tokens, operator);
        self.end();
    }

    fn handle_is_operator(&mut self, tokens: &mut Tokens, is_operator: TokenId, not: Option<TokenId>) {
        self.begin("handleIsOperator");
        self.token(tokens, is_operator);
        self.opt_token(tokens, not);
        self.end();
    }

    fn handle_literal_bool(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleLiteralBool");
        self.token(tokens, token);
        self.end();
    }

    fn handle_break_statement(&mut self, tokens: &mut Tokens, has_target: bool, break_keyword: TokenId, end_token: TokenId) {
        self.begin("handleBreakStatement");
        self.bool(has_target);
        self.token(tokens, break_keyword);
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_continue_statement(&mut self, tokens: &mut Tokens, has_target: bool, continue_keyword: TokenId, end_token: TokenId) {
        self.begin("handleContinueStatement");
        self.bool(has_target);
        self.token(tokens, continue_keyword);
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_empty_statement(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleEmptyStatement");
        self.token(tokens, token);
        self.end();
    }

    fn begin_assert(&mut self, tokens: &mut Tokens, assert_keyword: TokenId, kind: Assert) {
        self.begin("beginAssert");
        self.token(tokens, assert_keyword);
        self.string(kind.name());
        self.end();
    }

    fn end_assert(&mut self, tokens: &mut Tokens, assert_keyword: TokenId, kind: Assert, left_parenthesis: TokenId, comma_token: Option<TokenId>, end_token: TokenId) {
        self.begin("endAssert");
        self.token(tokens, assert_keyword);
        self.string(kind.name());
        self.token(tokens, left_parenthesis);
        self.opt_token(tokens, comma_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_literal_double(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleLiteralDouble");
        self.token(tokens, token);
        self.end();
    }

    fn handle_literal_double_with_separators(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleLiteralDoubleWithSeparators");
        self.token(tokens, token);
        self.end();
    }

    fn handle_literal_int(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleLiteralInt");
        self.token(tokens, token);
        self.end();
    }

    fn handle_literal_int_with_separators(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleLiteralIntWithSeparators");
        self.token(tokens, token);
        self.end();
    }

    fn handle_literal_list(&mut self, tokens: &mut Tokens, count: i32, left_bracket: TokenId, const_keyword: Option<TokenId>, right_bracket: TokenId) {
        self.begin("handleLiteralList");
        self.int(count as i64);
        self.token(tokens, left_bracket);
        self.opt_token(tokens, const_keyword);
        self.token(tokens, right_bracket);
        self.end();
    }

    fn handle_list_pattern(&mut self, tokens: &mut Tokens, count: i32, left_bracket: TokenId, right_bracket: TokenId) {
        self.begin("handleListPattern");
        self.int(count as i64);
        self.token(tokens, left_bracket);
        self.token(tokens, right_bracket);
        self.end();
    }

    fn handle_literal_set_or_map(&mut self, tokens: &mut Tokens, count: i32, left_brace: TokenId, const_keyword: Option<TokenId>, right_brace: TokenId) {
        self.begin("handleLiteralSetOrMap");
        self.int(count as i64);
        self.token(tokens, left_brace);
        self.opt_token(tokens, const_keyword);
        self.token(tokens, right_brace);
        self.end();
    }

    fn handle_map_pattern(&mut self, tokens: &mut Tokens, count: i32, left_brace: TokenId, right_brace: TokenId) {
        self.begin("handleMapPattern");
        self.int(count as i64);
        self.token(tokens, left_brace);
        self.token(tokens, right_brace);
        self.end();
    }

    fn handle_literal_null(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleLiteralNull");
        self.token(tokens, token);
        self.end();
    }

    fn handle_native_clause(&mut self, tokens: &mut Tokens, native_token: TokenId, has_name: bool) {
        self.begin("handleNativeClause");
        self.token(tokens, native_token);
        self.bool(has_name);
        self.end();
    }

    fn handle_named_argument(&mut self, tokens: &mut Tokens, colon: TokenId) {
        self.begin("handleNamedArgument");
        self.token(tokens, colon);
        self.end();
    }

    fn handle_positional_argument(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handlePositionalArgument");
        self.token(tokens, token);
        self.end();
    }

    fn handle_pattern_field(&mut self, tokens: &mut Tokens, colon: Option<TokenId>) {
        self.begin("handlePatternField");
        self.opt_token(tokens, colon);
        self.end();
    }

    fn handle_named_record_field(&mut self, tokens: &mut Tokens, colon: TokenId) {
        self.begin("handleNamedRecordField");
        self.token(tokens, colon);
        self.end();
    }

    fn handle_positional_record_field(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handlePositionalRecordField");
        self.token(tokens, token);
        self.end();
    }

    fn begin_new_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginNewExpression");
        self.token(tokens, token);
        self.end();
    }

    fn end_new_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("endNewExpression");
        self.token(tokens, token);
        self.end();
    }

    fn handle_no_arguments(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleNoArguments");
        self.token(tokens, token);
        self.end();
    }

    fn handle_no_constructor_reference_continuation_after_type_arguments(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleNoConstructorReferenceContinuationAfterTypeArguments");
        self.token(tokens, token);
        self.end();
    }

    fn handle_no_identifier(&mut self, tokens: &mut Tokens, token: TokenId, identifier_context: IdentifierContext) {
        self.begin("handleNoIdentifier");
        self.token(tokens, token);
        self.string(identifier_context.name());
        self.end();
    }

    fn handle_no_type_name_in_constructor_reference(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleNoTypeNameInConstructorReference");
        self.token(tokens, token);
        self.end();
    }

    fn handle_no_type(&mut self, tokens: &mut Tokens, last_consumed: TokenId) {
        self.begin("handleNoType");
        self.token(tokens, last_consumed);
        self.end();
    }

    fn handle_no_type_variables(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleNoTypeVariables");
        self.token(tokens, token);
        self.end();
    }

    fn handle_operator(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleOperator");
        self.token(tokens, token);
        self.end();
    }

    fn handle_switch_case_no_when_clause(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleSwitchCaseNoWhenClause");
        self.token(tokens, token);
        self.end();
    }

    fn handle_switch_expression_case_pattern(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleSwitchExpressionCasePattern");
        self.token(tokens, token);
        self.end();
    }

    fn handle_symbol_void(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleSymbolVoid");
        self.token(tokens, token);
        self.end();
    }

    fn handle_operator_name(&mut self, tokens: &mut Tokens, operator_keyword: TokenId, token: TokenId) {
        self.begin("handleOperatorName");
        self.token(tokens, operator_keyword);
        self.token(tokens, token);
        self.end();
    }

    fn handle_invalid_operator_name(&mut self, tokens: &mut Tokens, operator_keyword: TokenId, token: TokenId) {
        self.begin("handleInvalidOperatorName");
        self.token(tokens, operator_keyword);
        self.token(tokens, token);
        self.end();
    }

    fn handle_parenthesized_condition(&mut self, tokens: &mut Tokens, token: TokenId, case_: Option<TokenId>, when: Option<TokenId>) {
        self.begin("handleParenthesizedCondition");
        self.token(tokens, token);
        self.opt_token(tokens, case_);
        self.opt_token(tokens, when);
        self.end();
    }

    fn begin_pattern(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginPattern");
        self.token(tokens, token);
        self.end();
    }

    fn begin_pattern_guard(&mut self, tokens: &mut Tokens, when: TokenId) {
        self.begin("beginPatternGuard");
        self.token(tokens, when);
        self.end();
    }

    fn begin_parenthesized_expression_or_record_literal(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginParenthesizedExpressionOrRecordLiteral");
        self.token(tokens, token);
        self.end();
    }

    fn begin_switch_case_when_clause(&mut self, tokens: &mut Tokens, when: TokenId) {
        self.begin("beginSwitchCaseWhenClause");
        self.token(tokens, when);
        self.end();
    }

    fn end_record_literal(&mut self, tokens: &mut Tokens, token: TokenId, count: i32, const_keyword: Option<TokenId>) {
        self.begin("endRecordLiteral");
        self.token(tokens, token);
        self.int(count as i64);
        self.opt_token(tokens, const_keyword);
        self.end();
    }

    fn handle_record_pattern(&mut self, tokens: &mut Tokens, token: TokenId, count: i32) {
        self.begin("handleRecordPattern");
        self.token(tokens, token);
        self.int(count as i64);
        self.end();
    }

    fn end_pattern(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("endPattern");
        self.token(tokens, token);
        self.end();
    }

    fn end_pattern_guard(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("endPatternGuard");
        self.token(tokens, token);
        self.end();
    }

    fn end_parenthesized_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("endParenthesizedExpression");
        self.token(tokens, token);
        self.end();
    }

    fn end_switch_case_when_clause(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("endSwitchCaseWhenClause");
        self.token(tokens, token);
        self.end();
    }

    fn handle_parenthesized_pattern(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleParenthesizedPattern");
        self.token(tokens, token);
        self.end();
    }

    fn begin_constant_pattern(&mut self, tokens: &mut Tokens, const_keyword: Option<TokenId>) {
        self.begin("beginConstantPattern");
        self.opt_token(tokens, const_keyword);
        self.end();
    }

    fn end_constant_pattern(&mut self, tokens: &mut Tokens, const_keyword: Option<TokenId>) {
        self.begin("endConstantPattern");
        self.opt_token(tokens, const_keyword);
        self.end();
    }

    fn handle_object_pattern(&mut self, tokens: &mut Tokens, first_identifier: TokenId, dot: Option<TokenId>, second_identifier: Option<TokenId>) {
        self.begin("handleObjectPattern");
        self.token(tokens, first_identifier);
        self.opt_token(tokens, dot);
        self.opt_token(tokens, second_identifier);
        self.end();
    }

    fn handle_qualified(&mut self, tokens: &mut Tokens, period: TokenId) {
        self.begin("handleQualified");
        self.token(tokens, period);
        self.end();
    }

    fn handle_string_part(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleStringPart");
        self.token(tokens, token);
        self.end();
    }

    fn handle_super_expression(&mut self, tokens: &mut Tokens, token: TokenId, context: IdentifierContext) {
        self.begin("handleSuperExpression");
        self.token(tokens, token);
        self.string(context.name());
        self.end();
    }

    fn begin_switch_case(&mut self, tokens: &mut Tokens, label_count: i32, expression_count: i32, begin_token: TokenId) {
        self.begin("beginSwitchCase");
        self.int(label_count as i64);
        self.int(expression_count as i64);
        self.token(tokens, begin_token);
        self.end();
    }

    fn end_switch_case(&mut self, tokens: &mut Tokens, label_count: i32, expression_count: i32, default_keyword: Option<TokenId>, colon_after_default: Option<TokenId>, statement_count: i32, begin_token: TokenId, end_token: TokenId) {
        self.begin("endSwitchCase");
        self.int(label_count as i64);
        self.int(expression_count as i64);
        self.opt_token(tokens, default_keyword);
        self.opt_token(tokens, colon_after_default);
        self.int(statement_count as i64);
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn begin_switch_expression_case(&mut self, tokens: &mut Tokens) {
        self.begin("beginSwitchExpressionCase");
        self.end();
    }

    fn end_switch_expression_case(&mut self, tokens: &mut Tokens, begin_token: TokenId, when: Option<TokenId>, arrow: TokenId, end_token: TokenId) {
        self.begin("endSwitchExpressionCase");
        self.token(tokens, begin_token);
        self.opt_token(tokens, when);
        self.token(tokens, arrow);
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_this_expression(&mut self, tokens: &mut Tokens, token: TokenId, context: IdentifierContext) {
        self.begin("handleThisExpression");
        self.token(tokens, token);
        self.string(context.name());
        self.end();
    }

    fn handle_unary_postfix_assignment_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleUnaryPostfixAssignmentExpression");
        self.token(tokens, token);
        self.end();
    }

    fn handle_unary_prefix_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleUnaryPrefixExpression");
        self.token(tokens, token);
        self.end();
    }

    fn handle_relational_pattern(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleRelationalPattern");
        self.token(tokens, token);
        self.end();
    }

    fn handle_unary_prefix_assignment_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleUnaryPrefixAssignmentExpression");
        self.token(tokens, token);
        self.end();
    }

    fn begin_formal_parameter_default_value_expression(&mut self, tokens: &mut Tokens) {
        self.begin("beginFormalParameterDefaultValueExpression");
        self.end();
    }

    fn end_formal_parameter_default_value_expression(&mut self, tokens: &mut Tokens) {
        self.begin("endFormalParameterDefaultValueExpression");
        self.end();
    }

    fn handle_valued_formal_parameter(&mut self, tokens: &mut Tokens, equals: TokenId, token: TokenId, kind: FormalParameterKind) {
        self.begin("handleValuedFormalParameter");
        self.token(tokens, equals);
        self.token(tokens, token);
        self.string(kind.name());
        self.end();
    }

    fn handle_formal_parameter_without_value(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleFormalParameterWithoutValue");
        self.token(tokens, token);
        self.end();
    }

    fn handle_void_keyword(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleVoidKeyword");
        self.token(tokens, token);
        self.end();
    }

    fn handle_void_keyword_with_type_arguments(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleVoidKeywordWithTypeArguments");
        self.token(tokens, token);
        self.end();
    }

    fn begin_yield_statement(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginYieldStatement");
        self.token(tokens, token);
        self.end();
    }

    fn end_yield_statement(&mut self, tokens: &mut Tokens, yield_token: TokenId, star_token: Option<TokenId>, end_token: TokenId) {
        self.begin("endYieldStatement");
        self.token(tokens, yield_token);
        self.opt_token(tokens, star_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn end_invalid_yield_statement(&mut self, tokens: &mut Tokens, begin_token: TokenId, star_token: Option<TokenId>, end_token: TokenId, error_code: &'static CfeCode) {
        self.begin("endInvalidYieldStatement");
        self.token(tokens, begin_token);
        self.opt_token(tokens, star_token);
        self.token(tokens, end_token);
        self.string(error_code.name);
        self.end();
    }

    fn handle_recoverable_error(&mut self, tokens: &mut Tokens, message: CfeMessage, start_token: TokenId, end_token: TokenId) {
        self.begin("handleRecoverableError");
        self.message(&message);
        self.token(tokens, start_token);
        self.token(tokens, end_token);
        self.end();
        self.record_error(tokens, &message, start_token, end_token);
    }

    fn handle_experiment_not_enabled(&mut self, tokens: &mut Tokens, experimental_flag: ExperimentalFlag, begin_token: TokenId, end_token: TokenId) {
        self.begin("handleExperimentNotEnabled");
        self.string(experimental_flag.name());
        self.token(tokens, begin_token);
        self.token(tokens, end_token);
        self.end();
    }

    fn handle_error_token(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleErrorToken");
        self.error_token(tokens, token);
        self.end();
    }

    fn handle_unescape_error(&mut self, tokens: &mut Tokens, message: CfeMessage, location: TokenId, string_offset: i32, length: i32) {
        self.begin("handleUnescapeError");
        self.message(&message);
        self.token(tokens, location);
        self.int(string_offset as i64);
        self.int(length as i64);
        self.end();
    }

    fn handle_invalid_statement(&mut self, tokens: &mut Tokens, token: TokenId, message: CfeMessage) {
        self.begin("handleInvalidStatement");
        self.token(tokens, token);
        self.message(&message);
        self.end();
    }

    fn handle_script(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleScript");
        self.token(tokens, token);
        self.end();
    }

    fn handle_type_argument_application(&mut self, tokens: &mut Tokens, open_angle_bracket: TokenId) {
        self.begin("handleTypeArgumentApplication");
        self.token(tokens, open_angle_bracket);
        self.end();
    }

    fn handle_new_as_identifier(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleNewAsIdentifier");
        self.token(tokens, token);
        self.end();
    }

    fn handle_pattern_variable_declaration_statement(&mut self, tokens: &mut Tokens, keyword: TokenId, equals: TokenId, semicolon: TokenId) {
        self.begin("handlePatternVariableDeclarationStatement");
        self.token(tokens, keyword);
        self.token(tokens, equals);
        self.token(tokens, semicolon);
        self.end();
    }

    fn handle_pattern_assignment(&mut self, tokens: &mut Tokens, equals: TokenId) {
        self.begin("handlePatternAssignment");
        self.token(tokens, equals);
        self.end();
    }

    fn handle_dot_shorthand_context(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleDotShorthandContext");
        self.token(tokens, token);
        self.end();
    }

    fn handle_dot_shorthand_head(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("handleDotShorthandHead");
        self.token(tokens, token);
        self.end();
    }

    fn begin_const_dot_shorthand(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("beginConstDotShorthand");
        self.token(tokens, token);
        self.end();
    }

    fn end_const_dot_shorthand(&mut self, tokens: &mut Tokens, token: TokenId) {
        self.begin("endConstDotShorthand");
        self.token(tokens, token);
        self.end();
    }

}

#[allow(dead_code)]
fn _types(_: Assert, _: BlockKind, _: ConstructorReferenceContext, _: DeclarationHeaderKind, _: DeclarationKind, _: ExperimentalFlag, _: FormalParameterKind, _: IdentifierContext, _: MemberKind, _: &CfeCode) {}
