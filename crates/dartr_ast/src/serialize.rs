//! Compact binary serialization for [`Ast`].
//!
//! Used by the persistent disk cache to serialize `ConstExprs` AST arenas in
//! linked SDK library cycles (and any embedded `@docImport` ASTs).

use std::sync::Arc;

use dartr_syntax::{ErrorKind, ErrorToken, ScannerMessageCode, Token, TokenId, TokenType, Tokens};

use crate::arena::{Ast, Id, NodeId, NodeList, TokenList};
use crate::doc_comment::{
    BlockDocDirective, CodeBlockType, DocDirective, DocDirectiveArgument,
    DocDirectiveNamedArgument, DocDirectivePositionalArgument, DocDirectiveTag, DocDirectiveType,
    DocImport, MdCodeBlock, MdCodeBlockLine, SimpleDocDirective,
};
use crate::generated::nodes::*;
use crate::node_impl::ParameterKind;

#[inline]
fn write_u8(w: &mut Vec<u8>, v: u8) {
    w.push(v);
}

#[inline]
fn read_u8(r: &mut &[u8]) -> Option<u8> {
    let (&b, rest) = r.split_first()?;
    *r = rest;
    Some(b)
}

#[inline]
fn write_bool(w: &mut Vec<u8>, v: bool) {
    w.push(u8::from(v));
}

#[inline]
fn read_bool(r: &mut &[u8]) -> Option<bool> {
    match read_u8(r)? {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

#[inline]
fn write_u32(w: &mut Vec<u8>, v: u32) {
    w.extend_from_slice(&v.to_le_bytes());
}

#[inline]
fn read_u32(r: &mut &[u8]) -> Option<u32> {
    if r.len() < 4 {
        return None;
    }
    let (bytes, rest) = r.split_at(4);
    *r = rest;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

#[inline]
fn write_i32(w: &mut Vec<u8>, v: i32) {
    w.extend_from_slice(&v.to_le_bytes());
}

#[inline]
fn read_i32(r: &mut &[u8]) -> Option<i32> {
    if r.len() < 4 {
        return None;
    }
    let (bytes, rest) = r.split_at(4);
    *r = rest;
    Some(i32::from_le_bytes(bytes.try_into().ok()?))
}

#[inline]
fn write_i64(w: &mut Vec<u8>, v: i64) {
    w.extend_from_slice(&v.to_le_bytes());
}

#[inline]
fn read_i64(r: &mut &[u8]) -> Option<i64> {
    if r.len() < 8 {
        return None;
    }
    let (bytes, rest) = r.split_at(8);
    *r = rest;
    Some(i64::from_le_bytes(bytes.try_into().ok()?))
}

#[inline]
fn write_f64(w: &mut Vec<u8>, v: f64) {
    w.extend_from_slice(&v.to_bits().to_le_bytes());
}

#[inline]
fn read_f64(r: &mut &[u8]) -> Option<f64> {
    if r.len() < 8 {
        return None;
    }
    let (bytes, rest) = r.split_at(8);
    *r = rest;
    Some(f64::from_bits(u64::from_le_bytes(bytes.try_into().ok()?)))
}

#[inline]
fn write_str(w: &mut Vec<u8>, s: &str) {
    write_u32(w, s.len() as u32);
    w.extend_from_slice(s.as_bytes());
}

#[inline]
fn read_str<'a>(r: &mut &'a [u8]) -> Option<&'a str> {
    let len = read_u32(r)? as usize;
    if r.len() < len {
        return None;
    }
    let (bytes, rest) = r.split_at(len);
    *r = rest;
    std::str::from_utf8(bytes).ok()
}

trait FieldCodec: Sized {
    fn encode(&self, w: &mut Vec<u8>);
    fn decode(r: &mut &[u8]) -> Option<Self>;
}

impl FieldCodec for bool {
    #[inline]
    fn encode(&self, w: &mut Vec<u8>) {
        write_bool(w, *self);
    }
    #[inline]
    fn decode(r: &mut &[u8]) -> Option<Self> {
        read_bool(r)
    }
}

impl FieldCodec for f64 {
    #[inline]
    fn encode(&self, w: &mut Vec<u8>) {
        write_f64(w, *self);
    }
    #[inline]
    fn decode(r: &mut &[u8]) -> Option<Self> {
        read_f64(r)
    }
}

impl FieldCodec for Option<i64> {
    #[inline]
    fn encode(&self, w: &mut Vec<u8>) {
        match self {
            None => write_u8(w, 0),
            Some(v) => {
                write_u8(w, 1);
                write_i64(w, *v);
            }
        }
    }
    #[inline]
    fn decode(r: &mut &[u8]) -> Option<Self> {
        match read_u8(r)? {
            0 => Some(None),
            1 => Some(Some(read_i64(r)?)),
            _ => None,
        }
    }
}

impl FieldCodec for Box<str> {
    #[inline]
    fn encode(&self, w: &mut Vec<u8>) {
        write_str(w, self);
    }
    #[inline]
    fn decode(r: &mut &[u8]) -> Option<Self> {
        Some(read_str(r)?.into())
    }
}

impl FieldCodec for TokenId {
    #[inline]
    fn encode(&self, w: &mut Vec<u8>) {
        write_u32(w, self.0);
    }
    #[inline]
    fn decode(r: &mut &[u8]) -> Option<Self> {
        Some(TokenId(read_u32(r)?))
    }
}

impl FieldCodec for Option<TokenId> {
    #[inline]
    fn encode(&self, w: &mut Vec<u8>) {
        match self {
            None => write_u32(w, u32::MAX),
            Some(id) => write_u32(w, id.0),
        }
    }
    #[inline]
    fn decode(r: &mut &[u8]) -> Option<Self> {
        let raw = read_u32(r)?;
        if raw == u32::MAX {
            Some(None)
        } else {
            Some(Some(TokenId(raw)))
        }
    }
}

impl FieldCodec for TokenList {
    #[inline]
    fn encode(&self, w: &mut Vec<u8>) {
        let (start, len) = self.raw_parts();
        write_u32(w, start);
        write_u32(w, len);
    }
    #[inline]
    fn decode(r: &mut &[u8]) -> Option<Self> {
        let start = read_u32(r)?;
        let len = read_u32(r)?;
        Some(TokenList::from_raw_parts(start, len))
    }
}

impl<T: ?Sized> FieldCodec for Id<T> {
    #[inline]
    fn encode(&self, w: &mut Vec<u8>) {
        write_u32(w, self.raw().index() as u32);
    }
    #[inline]
    fn decode(r: &mut &[u8]) -> Option<Self> {
        let idx = read_u32(r)? as usize;
        Some(Id::from_raw(NodeId::from_index(idx)))
    }
}

impl<T: ?Sized> FieldCodec for Option<Id<T>> {
    #[inline]
    fn encode(&self, w: &mut Vec<u8>) {
        match self {
            None => write_u32(w, u32::MAX),
            Some(id) => write_u32(w, id.raw().index() as u32),
        }
    }
    #[inline]
    fn decode(r: &mut &[u8]) -> Option<Self> {
        let raw = read_u32(r)?;
        if raw == u32::MAX {
            Some(None)
        } else {
            Some(Some(Id::from_raw(NodeId::from_index(raw as usize))))
        }
    }
}

impl<T: ?Sized> FieldCodec for NodeList<T> {
    #[inline]
    fn encode(&self, w: &mut Vec<u8>) {
        let (start, len) = self.raw_parts();
        write_u32(w, start);
        write_u32(w, len);
    }
    #[inline]
    fn decode(r: &mut &[u8]) -> Option<Self> {
        let start = read_u32(r)?;
        let len = read_u32(r)?;
        Some(NodeList::from_raw_parts(start, len))
    }
}

impl FieldCodec for ParameterKind {
    #[inline]
    fn encode(&self, w: &mut Vec<u8>) {
        let tag = match self {
            ParameterKind::Required => 0,
            ParameterKind::Positional => 1,
            ParameterKind::NamedRequired => 2,
            ParameterKind::Named => 3,
        };
        write_u8(w, tag);
    }
    #[inline]
    fn decode(r: &mut &[u8]) -> Option<Self> {
        match read_u8(r)? {
            0 => Some(ParameterKind::Required),
            1 => Some(ParameterKind::Positional),
            2 => Some(ParameterKind::NamedRequired),
            3 => Some(ParameterKind::Named),
            _ => None,
        }
    }
}

impl FieldCodec for Vec<MdCodeBlock> {
    fn encode(&self, w: &mut Vec<u8>) {
        write_u32(w, self.len() as u32);
        for b in self {
            match &b.info_string {
                None => write_u8(w, 0),
                Some(s) => {
                    write_u8(w, 1);
                    write_str(w, s);
                }
            }
            write_u32(w, b.lines.len() as u32);
            for line in &b.lines {
                write_u32(w, line.offset);
                write_u32(w, line.length);
            }
            write_u8(
                w,
                match b.ty {
                    CodeBlockType::Fenced => 0,
                    CodeBlockType::Indented => 1,
                },
            );
        }
    }

    fn decode(r: &mut &[u8]) -> Option<Self> {
        let len = read_u32(r)? as usize;
        let mut out = Vec::with_capacity(len.min(1024));
        for _ in 0..len {
            let info_string = match read_u8(r)? {
                0 => None,
                1 => Some(Box::<str>::from(read_str(r)?)),
                _ => return None,
            };
            let lines_len = read_u32(r)? as usize;
            let mut lines = Vec::with_capacity(lines_len.min(1024));
            for _ in 0..lines_len {
                lines.push(MdCodeBlockLine {
                    offset: read_u32(r)?,
                    length: read_u32(r)?,
                });
            }
            let ty = match read_u8(r)? {
                0 => CodeBlockType::Fenced,
                1 => CodeBlockType::Indented,
                _ => return None,
            };
            out.push(MdCodeBlock {
                info_string,
                lines,
                ty,
            });
        }
        Some(out)
    }
}

impl FieldCodec for Vec<DocImport> {
    fn encode(&self, w: &mut Vec<u8>) {
        write_u32(w, self.len() as u32);
        for d in self {
            write_u32(w, d.offset);
            d.ast.encode(w);
            d.import.encode(w);
        }
    }

    fn decode(r: &mut &[u8]) -> Option<Self> {
        let len = read_u32(r)? as usize;
        let mut out = Vec::with_capacity(len.min(256));
        for _ in 0..len {
            let offset = read_u32(r)?;
            let ast = Box::new(Ast::decode(r)?);
            let import = <Id<ImportDirective> as FieldCodec>::decode(r)?;
            out.push(DocImport {
                offset,
                ast,
                import,
            });
        }
        Some(out)
    }
}

fn encode_doc_directive_tag(tag: &DocDirectiveTag, w: &mut Vec<u8>) {
    write_u32(w, tag.offset);
    write_u32(w, tag.end);
    write_u32(w, tag.name_offset);
    write_u32(w, tag.name_end);
    let ty_idx = DocDirectiveType::VALUES
        .iter()
        .position(|&t| t == tag.ty)
        .unwrap_or(0) as u8;
    write_u8(w, ty_idx);
    write_u32(w, tag.positional_arguments.len() as u32);
    for arg in &tag.positional_arguments {
        match arg {
            DocDirectiveArgument::Positional(p) => {
                write_u8(w, 0);
                write_u32(w, p.offset);
                write_u32(w, p.end);
                write_str(w, &p.value);
            }
            DocDirectiveArgument::Named(n) => {
                write_u8(w, 1);
                write_u32(w, n.offset);
                write_u32(w, n.end);
                write_str(w, &n.name);
                write_str(w, &n.value);
            }
        }
    }
    write_u32(w, tag.named_arguments.len() as u32);
    for n in &tag.named_arguments {
        write_u32(w, n.offset);
        write_u32(w, n.end);
        write_str(w, &n.name);
        write_str(w, &n.value);
    }
}

fn decode_doc_directive_tag(r: &mut &[u8]) -> Option<DocDirectiveTag> {
    let offset = read_u32(r)?;
    let end = read_u32(r)?;
    let name_offset = read_u32(r)?;
    let name_end = read_u32(r)?;
    let ty = *DocDirectiveType::VALUES.get(read_u8(r)? as usize)?;
    let pos_len = read_u32(r)? as usize;
    let mut positional_arguments = Vec::with_capacity(pos_len.min(256));
    for _ in 0..pos_len {
        let arg = match read_u8(r)? {
            0 => DocDirectiveArgument::Positional(DocDirectivePositionalArgument {
                offset: read_u32(r)?,
                end: read_u32(r)?,
                value: read_str(r)?.into(),
            }),
            1 => DocDirectiveArgument::Named(DocDirectiveNamedArgument {
                offset: read_u32(r)?,
                end: read_u32(r)?,
                name: read_str(r)?.into(),
                value: read_str(r)?.into(),
            }),
            _ => return None,
        };
        positional_arguments.push(arg);
    }
    let named_len = read_u32(r)? as usize;
    let mut named_arguments = Vec::with_capacity(named_len.min(256));
    for _ in 0..named_len {
        named_arguments.push(DocDirectiveNamedArgument {
            offset: read_u32(r)?,
            end: read_u32(r)?,
            name: read_str(r)?.into(),
            value: read_str(r)?.into(),
        });
    }
    Some(DocDirectiveTag {
        offset,
        end,
        name_offset,
        name_end,
        ty,
        positional_arguments,
        named_arguments,
    })
}

impl FieldCodec for Vec<DocDirective> {
    fn encode(&self, w: &mut Vec<u8>) {
        write_u32(w, self.len() as u32);
        for d in self {
            match d {
                DocDirective::Simple(s) => {
                    write_u8(w, 0);
                    encode_doc_directive_tag(&s.tag, w);
                }
                DocDirective::Block(b) => {
                    write_u8(w, 1);
                    encode_doc_directive_tag(&b.opening_tag, w);
                    match &b.closing_tag {
                        None => write_u8(w, 0),
                        Some(c) => {
                            write_u8(w, 1);
                            encode_doc_directive_tag(c, w);
                        }
                    }
                }
            }
        }
    }

    fn decode(r: &mut &[u8]) -> Option<Self> {
        let len = read_u32(r)? as usize;
        let mut out = Vec::with_capacity(len.min(256));
        for _ in 0..len {
            let d = match read_u8(r)? {
                0 => DocDirective::Simple(SimpleDocDirective {
                    tag: decode_doc_directive_tag(r)?,
                }),
                1 => {
                    let opening_tag = decode_doc_directive_tag(r)?;
                    let closing_tag = match read_u8(r)? {
                        0 => None,
                        1 => Some(decode_doc_directive_tag(r)?),
                        _ => return None,
                    };
                    DocDirective::Block(BlockDocDirective {
                        opening_tag,
                        closing_tag,
                    })
                }
                _ => return None,
            };
            out.push(d);
        }
        Some(out)
    }
}

fn encode_scanner_code(code: ScannerMessageCode, w: &mut Vec<u8>) {
    let tag = match code {
        ScannerMessageCode::AsciiControlCharacter => 0,
        ScannerMessageCode::Encoding => 1,
        ScannerMessageCode::ExpectedHexDigit => 2,
        ScannerMessageCode::MissingExponent => 3,
        ScannerMessageCode::NonAsciiIdentifier => 4,
        ScannerMessageCode::NonAsciiWhitespace => 5,
        ScannerMessageCode::UnexpectedDollarInString => 6,
        ScannerMessageCode::UnexpectedSeparatorInNumber => 7,
        ScannerMessageCode::UnmatchedToken => 8,
        ScannerMessageCode::UnsupportedOperator => 9,
        ScannerMessageCode::UnterminatedComment => 10,
        ScannerMessageCode::UnterminatedString => 11,
    };
    write_u8(w, tag);
}

fn decode_scanner_code(r: &mut &[u8]) -> Option<ScannerMessageCode> {
    match read_u8(r)? {
        0 => Some(ScannerMessageCode::AsciiControlCharacter),
        1 => Some(ScannerMessageCode::Encoding),
        2 => Some(ScannerMessageCode::ExpectedHexDigit),
        3 => Some(ScannerMessageCode::MissingExponent),
        4 => Some(ScannerMessageCode::NonAsciiIdentifier),
        5 => Some(ScannerMessageCode::NonAsciiWhitespace),
        6 => Some(ScannerMessageCode::UnexpectedDollarInString),
        7 => Some(ScannerMessageCode::UnexpectedSeparatorInNumber),
        8 => Some(ScannerMessageCode::UnmatchedToken),
        9 => Some(ScannerMessageCode::UnsupportedOperator),
        10 => Some(ScannerMessageCode::UnterminatedComment),
        11 => Some(ScannerMessageCode::UnterminatedString),
        _ => None,
    }
}

const QUOTE_STARTS: &[&str] = &["\"", "'", "\"\"\"", "'''", "r\"", "r'", "r\"\"\"", "r'''"];

fn encode_tokens(tokens: &Tokens, w: &mut Vec<u8>) {
    let (source, tok_slice, owned_lexemes, errors) = tokens.raw_parts();
    write_str(w, source);
    write_u32(w, tok_slice.len() as u32);
    for t in tok_slice {
        write_u8(w, t.ty.0);
        write_u8(w, t.flags);
        write_u32(w, t.offset);
        write_u32(w, t.length);
        write_u32(w, t.lex_start);
        write_u32(w, t.lex_end);
        write_u32(w, t.next.0);
        write_u32(w, t.previous.0);
        write_u32(w, t.preceding_comments.0);
        write_u32(w, t.end_group.0);
        write_u32(w, t.before_synthetic.0);
    }
    write_u32(w, owned_lexemes.len() as u32);
    for lex in owned_lexemes {
        write_str(w, lex);
    }
    write_u32(w, errors.len() as u32);
    for err in errors {
        write_u32(w, err.char_offset);
        match &err.kind {
            ErrorKind::Encoding => write_u8(w, 0),
            ErrorKind::NonAsciiIdentifier { character } => {
                write_u8(w, 1);
                write_i32(w, *character);
            }
            ErrorKind::NonAsciiWhitespace { character } => {
                write_u8(w, 2);
                write_i32(w, *character);
            }
            ErrorKind::AsciiControlCharacter { character } => {
                write_u8(w, 3);
                write_i32(w, *character);
            }
            ErrorKind::UnsupportedOperator { token } => {
                write_u8(w, 4);
                write_u32(w, token.0);
            }
            ErrorKind::UnterminatedString { start, end_offset } => {
                write_u8(w, 5);
                let idx = QUOTE_STARTS.iter().position(|s| s == start).unwrap_or(0) as u8;
                write_u8(w, idx);
                write_u32(w, *end_offset);
            }
            ErrorKind::UnterminatedToken { code, end_offset } => {
                write_u8(w, 6);
                encode_scanner_code(*code, w);
                write_u32(w, *end_offset);
            }
            ErrorKind::UnmatchedToken { begin } => {
                write_u8(w, 7);
                write_u32(w, begin.0);
            }
        }
    }
}

fn decode_tokens(r: &mut &[u8]) -> Option<Tokens> {
    let source: Arc<str> = Arc::from(read_str(r)?);
    let tok_len = read_u32(r)? as usize;
    let mut tokens = Vec::with_capacity(tok_len.min(65536));
    for _ in 0..tok_len {
        tokens.push(Token {
            ty: TokenType(read_u8(r)?),
            flags: read_u8(r)?,
            offset: read_u32(r)?,
            length: read_u32(r)?,
            lex_start: read_u32(r)?,
            lex_end: read_u32(r)?,
            next: TokenId(read_u32(r)?),
            previous: TokenId(read_u32(r)?),
            preceding_comments: TokenId(read_u32(r)?),
            end_group: TokenId(read_u32(r)?),
            before_synthetic: TokenId(read_u32(r)?),
        });
    }
    let lex_len = read_u32(r)? as usize;
    let mut owned_lexemes = Vec::with_capacity(lex_len.min(16384));
    for _ in 0..lex_len {
        owned_lexemes.push(read_str(r)?.into());
    }
    let err_len = read_u32(r)? as usize;
    let mut errors = Vec::with_capacity(err_len.min(1024));
    for _ in 0..err_len {
        let char_offset = read_u32(r)?;
        let kind = match read_u8(r)? {
            0 => ErrorKind::Encoding,
            1 => ErrorKind::NonAsciiIdentifier {
                character: read_i32(r)?,
            },
            2 => ErrorKind::NonAsciiWhitespace {
                character: read_i32(r)?,
            },
            3 => ErrorKind::AsciiControlCharacter {
                character: read_i32(r)?,
            },
            4 => ErrorKind::UnsupportedOperator {
                token: TokenId(read_u32(r)?),
            },
            5 => {
                let start = *QUOTE_STARTS.get(read_u8(r)? as usize)?;
                let end_offset = read_u32(r)?;
                ErrorKind::UnterminatedString { start, end_offset }
            }
            6 => {
                let code = decode_scanner_code(r)?;
                let end_offset = read_u32(r)?;
                ErrorKind::UnterminatedToken { code, end_offset }
            }
            7 => ErrorKind::UnmatchedToken {
                begin: TokenId(read_u32(r)?),
            },
            _ => return None,
        };
        errors.push(ErrorToken { kind, char_offset });
    }
    Some(Tokens::from_raw_parts(
        source,
        tokens,
        owned_lexemes,
        errors,
    ))
}

macro_rules! ast_stores_codec {
    ($( $kind:ident, $field:ident : $ty:ident { $( $f:ident ),* $(,)? } );* $(;)?) => {
        fn encode_stores(stores: &Stores, w: &mut Vec<u8>) {
            $(
                if !stores.$field.is_empty() {
                    write_u8(w, NodeKind::$kind as u8);
                    write_u32(w, stores.$field.len() as u32);
                    for item in &stores.$field {
                        $(
                            FieldCodec::encode(&item.$f, w);
                        )*
                    }
                }
            )*
            write_u8(w, u8::MAX);
        }

        fn decode_stores(r: &mut &[u8]) -> Option<Stores> {
            let mut stores = Stores::default();
            loop {
                let tag = read_u8(r)?;
                if tag == u8::MAX {
                    break;
                }
                let kind = *NodeKind::ALL.get(tag as usize)?;
                let count = read_u32(r)? as usize;
                match kind {
                    $(
                        NodeKind::$kind => {
                            stores.$field.reserve(count.min(16384));
                            for _ in 0..count {
                                stores.$field.push($ty {
                                    $(
                                        $f: FieldCodec::decode(r)?,
                                    )*
                                });
                            }
                        }
                    )*
                }
            }
            Some(stores)
        }
    };
}

ast_stores_codec! {
    AdjacentStrings, adjacent_strings: AdjacentStrings { strings };
    Annotation, annotation: Annotation { at_sign, name, type_arguments, period, constructor_name, arguments };
    AnonymousBlockBody, anonymous_block_body: AnonymousBlockBody { block };
    AnonymousExpressionBody, anonymous_expression_body: AnonymousExpressionBody { function_definition, expression };
    AnonymousMethodInvocation, anonymous_method_invocation: AnonymousMethodInvocation { target, operator, parameters, body };
    ArgumentList, argument_list: ArgumentList { left_parenthesis, arguments, right_parenthesis };
    AsExpression, as_expression: AsExpression { expression, as_operator, type_ };
    AssertInitializer, assert_initializer: AssertInitializer { assert_keyword, left_parenthesis, condition, comma, message, right_parenthesis };
    AssertStatement, assert_statement: AssertStatement { assert_keyword, left_parenthesis, condition, comma, message, right_parenthesis, semicolon };
    AssignedVariablePattern, assigned_variable_pattern: AssignedVariablePattern { name };
    AssignmentExpression, assignment_expression: AssignmentExpression { left_hand_side, operator, right_hand_side };
    AwaitExpression, await_expression: AwaitExpression { await_keyword, expression };
    BinaryExpression, binary_expression: BinaryExpression { left_operand, operator, right_operand };
    Block, block: Block { left_bracket, statements, right_bracket };
    BlockClassBody, block_class_body: BlockClassBody { left_bracket, members, right_bracket };
    BlockEnumBody, block_enum_body: BlockEnumBody { left_bracket, constants, semicolon, members, right_bracket };
    BlockFunctionBody, block_function_body: BlockFunctionBody { keyword, star, block };
    BooleanLiteral, boolean_literal: BooleanLiteral { literal, value };
    BreakStatement, break_statement: BreakStatement { break_keyword, label, semicolon };
    CascadeExpression, cascade_expression: CascadeExpression { target, cascade_sections };
    CaseClause, case_clause: CaseClause { case_keyword, guarded_pattern };
    CastPattern, cast_pattern: CastPattern { pattern, as_token, type_ };
    CatchClause, catch_clause: CatchClause { on_keyword, exception_type, catch_keyword, left_parenthesis, exception_parameter, comma, stack_trace_parameter, right_parenthesis, body };
    CatchClauseParameter, catch_clause_parameter: CatchClauseParameter { name };
    ClassDeclaration, class_declaration: ClassDeclaration { documentation_comment, metadata, augment_keyword, abstract_keyword, sealed_keyword, base_keyword, interface_keyword, final_keyword, mixin_keyword, class_keyword, name_part, extends_clause, with_clause, implements_clause, native_clause, body };
    ClassTypeAlias, class_type_alias: ClassTypeAlias { documentation_comment, metadata, augment_keyword, abstract_keyword, sealed_keyword, base_keyword, interface_keyword, final_keyword, mixin_keyword, typedef_keyword, name, type_parameters, equals, superclass, with_clause, implements_clause, semicolon };
    Comment, comment: Comment { references, tokens, code_blocks, doc_imports, doc_directives, has_nodoc };
    CommentReference, comment_reference: CommentReference { new_keyword, expression, is_synthetic };
    CompilationUnit, compilation_unit: CompilationUnit { begin_token, script_tag, directives, declarations, end_token };
    ConditionalExpression, conditional_expression: ConditionalExpression { condition, question, then_expression, colon, else_expression };
    Configuration, configuration: Configuration { if_keyword, left_parenthesis, name, equal_token, value, right_parenthesis, uri };
    ConstantPattern, constant_pattern: ConstantPattern { const_keyword, expression };
    ConstructorDeclaration, constructor_declaration: ConstructorDeclaration { documentation_comment, metadata, augment_keyword, external_keyword, const_keyword, factory_keyword, new_keyword, type_name, period, name, parameters, separator, initializers, redirected_constructor, body };
    ConstructorFieldInitializer, constructor_field_initializer: ConstructorFieldInitializer { this_keyword, period, field_name, equals, expression };
    ConstructorName, constructor_name: ConstructorName { type_, period, name };
    ConstructorReference, constructor_reference: ConstructorReference { constructor_name };
    ConstructorSelector, constructor_selector: ConstructorSelector { period, name };
    ContinueStatement, continue_statement: ContinueStatement { continue_keyword, label, semicolon };
    DeclaredIdentifier, declared_identifier: DeclaredIdentifier { documentation_comment, metadata, keyword, type_, name };
    DeclaredVariablePattern, declared_variable_pattern: DeclaredVariablePattern { keyword, type_, name };
    DoStatement, do_statement: DoStatement { do_keyword, body, while_keyword, left_parenthesis, condition, right_parenthesis, semicolon };
    DotShorthandConstructorInvocation, dot_shorthand_constructor_invocation: DotShorthandConstructorInvocation { const_keyword, period, constructor_name, type_arguments, argument_list };
    DotShorthandInvocation, dot_shorthand_invocation: DotShorthandInvocation { period, member_name, type_arguments, argument_list };
    DotShorthandPropertyAccess, dot_shorthand_property_access: DotShorthandPropertyAccess { period, property_name };
    DottedName, dotted_name: DottedName { tokens };
    DoubleLiteral, double_literal: DoubleLiteral { literal, value };
    EmptyClassBody, empty_class_body: EmptyClassBody { semicolon };
    EmptyEnumBody, empty_enum_body: EmptyEnumBody { semicolon };
    EmptyFunctionBody, empty_function_body: EmptyFunctionBody { semicolon };
    EmptyStatement, empty_statement: EmptyStatement { semicolon };
    EnumConstantArguments, enum_constant_arguments: EnumConstantArguments { type_arguments, constructor_selector, argument_list };
    EnumConstantDeclaration, enum_constant_declaration: EnumConstantDeclaration { documentation_comment, metadata, augment_keyword, name, arguments };
    EnumDeclaration, enum_declaration: EnumDeclaration { documentation_comment, metadata, augment_keyword, enum_keyword, name_part, with_clause, implements_clause, body };
    ExportDirective, export_directive: ExportDirective { documentation_comment, metadata, export_keyword, uri, configurations, combinators, semicolon };
    ExpressionFunctionBody, expression_function_body: ExpressionFunctionBody { keyword, star, function_definition, expression, semicolon };
    ExpressionStatement, expression_statement: ExpressionStatement { expression, semicolon };
    ExtendsClause, extends_clause: ExtendsClause { extends_keyword, superclass };
    ExtensionDeclaration, extension_declaration: ExtensionDeclaration { documentation_comment, metadata, augment_keyword, extension_keyword, type_keyword, name, type_parameters, on_clause, body };
    ExtensionOnClause, extension_on_clause: ExtensionOnClause { on_keyword, extended_type };
    ExtensionOverride, extension_override: ExtensionOverride { import_prefix, name, type_arguments, argument_list };
    ExtensionTypeDeclaration, extension_type_declaration: ExtensionTypeDeclaration { documentation_comment, metadata, augment_keyword, extension_keyword, type_keyword, name_part, implements_clause, body };
    FieldDeclaration, field_declaration: FieldDeclaration { documentation_comment, metadata, augment_keyword, external_keyword, static_keyword, abstract_keyword, covariant_keyword, fields, semicolon };
    FieldFormalParameter, field_formal_parameter: FieldFormalParameter { documentation_comment, metadata, kind, required_keyword, covariant_keyword, const_final_or_var_keyword, type_, this_keyword, period, name, function_typed_suffix, default_clause };
    ForEachPartsWithDeclaration, for_each_parts_with_declaration: ForEachPartsWithDeclaration { loop_variable, in_keyword, iterable };
    ForEachPartsWithIdentifier, for_each_parts_with_identifier: ForEachPartsWithIdentifier { identifier, in_keyword, iterable };
    ForEachPartsWithPattern, for_each_parts_with_pattern: ForEachPartsWithPattern { metadata, keyword, pattern, in_keyword, iterable };
    ForElement, for_element: ForElement { await_keyword, for_keyword, left_parenthesis, for_loop_parts, right_parenthesis, body };
    ForPartsWithDeclarations, for_parts_with_declarations: ForPartsWithDeclarations { variables, left_separator, condition, right_separator, updaters };
    ForPartsWithExpression, for_parts_with_expression: ForPartsWithExpression { initialization, left_separator, condition, right_separator, updaters };
    ForPartsWithPattern, for_parts_with_pattern: ForPartsWithPattern { variables, left_separator, condition, right_separator, updaters };
    ForStatement, for_statement: ForStatement { await_keyword, for_keyword, left_parenthesis, for_loop_parts, right_parenthesis, body };
    FormalParameterDefaultClause, formal_parameter_default_clause: FormalParameterDefaultClause { separator, value };
    FormalParameterList, formal_parameter_list: FormalParameterList { left_parenthesis, parameters, left_delimiter, right_delimiter, right_parenthesis };
    FunctionDeclaration, function_declaration: FunctionDeclaration { documentation_comment, metadata, augment_keyword, external_keyword, return_type, property_keyword, name, function_expression };
    FunctionDeclarationStatement, function_declaration_statement: FunctionDeclarationStatement { function_declaration };
    FunctionExpression, function_expression: FunctionExpression { type_parameters, parameters, body };
    FunctionExpressionInvocation, function_expression_invocation: FunctionExpressionInvocation { function, type_arguments, argument_list };
    FunctionReference, function_reference: FunctionReference { function, type_arguments };
    FunctionTypeAlias, function_type_alias: FunctionTypeAlias { documentation_comment, metadata, augment_keyword, typedef_keyword, return_type, name, type_parameters, parameters, semicolon };
    FunctionTypedFormalParameterSuffix, function_typed_formal_parameter_suffix: FunctionTypedFormalParameterSuffix { type_parameters, formal_parameters, question };
    GenericFunctionType, generic_function_type: GenericFunctionType { return_type, function_keyword, type_parameters, parameters, question };
    GenericTypeAlias, generic_type_alias: GenericTypeAlias { documentation_comment, metadata, augment_keyword, typedef_keyword, name, type_parameters, equals, type_, semicolon };
    GuardedPattern, guarded_pattern: GuardedPattern { pattern, when_clause };
    HideCombinator, hide_combinator: HideCombinator { keyword, hidden_names };
    IfElement, if_element: IfElement { if_keyword, left_parenthesis, expression, case_clause, right_parenthesis, then_element, else_keyword, else_element };
    IfStatement, if_statement: IfStatement { if_keyword, left_parenthesis, expression, case_clause, right_parenthesis, then_statement, else_keyword, else_statement };
    ImplementsClause, implements_clause: ImplementsClause { implements_keyword, interfaces };
    ImplicitCallReference, implicit_call_reference: ImplicitCallReference { expression, type_arguments };
    ImportDirective, import_directive: ImportDirective { documentation_comment, metadata, import_keyword, uri, configurations, deferred_keyword, as_keyword, prefix, combinators, semicolon };
    ImportPrefixReference, import_prefix_reference: ImportPrefixReference { name, period };
    IndexExpression, index_expression: IndexExpression { target, period, question, left_bracket, index, right_bracket };
    InstanceCreationExpression, instance_creation_expression: InstanceCreationExpression { keyword, constructor_name, type_arguments, argument_list };
    IntegerLiteral, integer_literal: IntegerLiteral { literal, value };
    InterpolationExpression, interpolation_expression: InterpolationExpression { left_bracket, expression, right_bracket };
    InterpolationString, interpolation_string: InterpolationString { contents, value };
    IsExpression, is_expression: IsExpression { expression, is_operator, not_operator, type_ };
    Label, label: Label { name, colon };
    LabelReference, label_reference: LabelReference { name };
    LabeledStatement, labeled_statement: LabeledStatement { labels, statement };
    LibraryDirective, library_directive: LibraryDirective { documentation_comment, metadata, library_keyword, name, semicolon };
    ListLiteral, list_literal: ListLiteral { const_keyword, type_arguments, left_bracket, elements, right_bracket };
    ListPattern, list_pattern: ListPattern { type_arguments, left_bracket, elements, right_bracket };
    LogicalAndPattern, logical_and_pattern: LogicalAndPattern { left_operand, operator, right_operand };
    LogicalOrPattern, logical_or_pattern: LogicalOrPattern { left_operand, operator, right_operand };
    MapLiteralEntry, map_literal_entry: MapLiteralEntry { key_question, key, separator, value_question, value };
    MapPattern, map_pattern: MapPattern { type_arguments, left_bracket, elements, right_bracket };
    MapPatternEntry, map_pattern_entry: MapPatternEntry { key, separator, value };
    MethodDeclaration, method_declaration: MethodDeclaration { documentation_comment, metadata, augment_keyword, external_keyword, modifier_keyword, return_type, property_keyword, operator_keyword, name, type_parameters, parameters, body };
    MethodInvocation, method_invocation: MethodInvocation { target, operator, method_name, type_arguments, argument_list };
    MixinDeclaration, mixin_declaration: MixinDeclaration { documentation_comment, metadata, augment_keyword, base_keyword, mixin_keyword, name, type_parameters, on_clause, implements_clause, body };
    MixinOnClause, mixin_on_clause: MixinOnClause { on_keyword, superclass_constraints };
    NameWithTypeParameters, name_with_type_parameters: NameWithTypeParameters { type_name, type_parameters };
    NamedArgument, named_argument: NamedArgument { name, colon, argument_expression };
    NamedType, named_type: NamedType { import_prefix, name, type_arguments, question };
    NativeClause, native_clause: NativeClause { native_keyword, name };
    NativeFunctionBody, native_function_body: NativeFunctionBody { native_keyword, string_literal, semicolon };
    NullAssertPattern, null_assert_pattern: NullAssertPattern { pattern, operator };
    NullAwareElement, null_aware_element: NullAwareElement { question, value };
    NullCheckPattern, null_check_pattern: NullCheckPattern { pattern, operator };
    NullLiteral, null_literal: NullLiteral { literal };
    ObjectPattern, object_pattern: ObjectPattern { type_, left_parenthesis, fields, right_parenthesis };
    ParenthesizedExpression, parenthesized_expression: ParenthesizedExpression { left_parenthesis, expression, right_parenthesis };
    ParenthesizedPattern, parenthesized_pattern: ParenthesizedPattern { left_parenthesis, pattern, right_parenthesis };
    PartDirective, part_directive: PartDirective { documentation_comment, metadata, part_keyword, uri, semicolon };
    PartOfDirective, part_of_directive: PartOfDirective { documentation_comment, metadata, part_keyword, of_keyword, uri, library_name, semicolon };
    PatternAssignment, pattern_assignment: PatternAssignment { pattern, equals, expression };
    PatternField, pattern_field: PatternField { name, pattern };
    PatternFieldName, pattern_field_name: PatternFieldName { name, colon };
    PatternVariableDeclaration, pattern_variable_declaration: PatternVariableDeclaration { documentation_comment, metadata, keyword, pattern, equals, expression };
    PatternVariableDeclarationStatement, pattern_variable_declaration_statement: PatternVariableDeclarationStatement { declaration, semicolon };
    PostfixExpression, postfix_expression: PostfixExpression { operand, operator };
    PrefixExpression, prefix_expression: PrefixExpression { operator, operand };
    PrefixedIdentifier, prefixed_identifier: PrefixedIdentifier { prefix, period, identifier };
    PrimaryConstructorBody, primary_constructor_body: PrimaryConstructorBody { documentation_comment, metadata, this_keyword, colon, initializers, body };
    PrimaryConstructorDeclaration, primary_constructor_declaration: PrimaryConstructorDeclaration { const_keyword, type_name, type_parameters, constructor_name, formal_parameters };
    PrimaryConstructorName, primary_constructor_name: PrimaryConstructorName { period, name };
    PropertyAccess, property_access: PropertyAccess { target, operator, property_name };
    RecordLiteral, record_literal: RecordLiteral { const_keyword, left_parenthesis, fields, right_parenthesis };
    RecordLiteralNamedField, record_literal_named_field: RecordLiteralNamedField { name, colon, field_expression };
    RecordPattern, record_pattern: RecordPattern { left_parenthesis, fields, right_parenthesis };
    RecordTypeAnnotation, record_type_annotation: RecordTypeAnnotation { left_parenthesis, positional_fields, named_fields, right_parenthesis, question };
    RecordTypeAnnotationNamedField, record_type_annotation_named_field: RecordTypeAnnotationNamedField { metadata, type_, name };
    RecordTypeAnnotationNamedFields, record_type_annotation_named_fields: RecordTypeAnnotationNamedFields { left_bracket, fields, right_bracket };
    RecordTypeAnnotationPositionalField, record_type_annotation_positional_field: RecordTypeAnnotationPositionalField { metadata, type_, name };
    RedirectingConstructorInvocation, redirecting_constructor_invocation: RedirectingConstructorInvocation { this_keyword, period, constructor_name, argument_list };
    RegularFormalParameter, regular_formal_parameter: RegularFormalParameter { documentation_comment, metadata, kind, required_keyword, covariant_keyword, const_final_or_var_keyword, type_, name, function_typed_suffix, default_clause };
    RelationalPattern, relational_pattern: RelationalPattern { operator, operand };
    RestPatternElement, rest_pattern_element: RestPatternElement { operator, pattern };
    RethrowExpression, rethrow_expression: RethrowExpression { rethrow_keyword };
    ReturnStatement, return_statement: ReturnStatement { return_keyword, expression, semicolon };
    ScriptTag, script_tag: ScriptTag { script_tag };
    SetOrMapLiteral, set_or_map_literal: SetOrMapLiteral { const_keyword, type_arguments, left_bracket, elements, right_bracket };
    ShowCombinator, show_combinator: ShowCombinator { keyword, shown_names };
    SimpleIdentifier, simple_identifier: SimpleIdentifier { token };
    SimpleStringLiteral, simple_string_literal: SimpleStringLiteral { literal, value };
    SpreadElement, spread_element: SpreadElement { spread_operator, expression };
    StringInterpolation, string_interpolation: StringInterpolation { elements };
    SuperConstructorInvocation, super_constructor_invocation: SuperConstructorInvocation { super_keyword, period, constructor_name, argument_list };
    SuperExpression, super_expression: SuperExpression { super_keyword };
    SuperFormalParameter, super_formal_parameter: SuperFormalParameter { documentation_comment, metadata, kind, required_keyword, covariant_keyword, const_final_or_var_keyword, type_, super_keyword, period, name, function_typed_suffix, default_clause };
    SwitchCase, switch_case: SwitchCase { labels, keyword, expression, colon, statements };
    SwitchDefault, switch_default: SwitchDefault { labels, keyword, colon, statements };
    SwitchExpression, switch_expression: SwitchExpression { switch_keyword, left_parenthesis, expression, right_parenthesis, left_bracket, cases, right_bracket };
    SwitchExpressionCase, switch_expression_case: SwitchExpressionCase { guarded_pattern, arrow, expression };
    SwitchPatternCase, switch_pattern_case: SwitchPatternCase { labels, keyword, guarded_pattern, colon, statements };
    SwitchStatement, switch_statement: SwitchStatement { switch_keyword, left_parenthesis, expression, right_parenthesis, left_bracket, members, right_bracket };
    SymbolLiteral, symbol_literal: SymbolLiteral { pound_sign, components };
    ThisExpression, this_expression: ThisExpression { this_keyword };
    ThrowExpression, throw_expression: ThrowExpression { throw_keyword, expression };
    TopLevelVariableDeclaration, top_level_variable_declaration: TopLevelVariableDeclaration { documentation_comment, metadata, augment_keyword, external_keyword, abstract_keyword, variables, semicolon };
    TryStatement, try_statement: TryStatement { try_keyword, body, catch_clauses, finally_keyword, finally_block };
    TypeArgumentList, type_argument_list: TypeArgumentList { left_bracket, arguments, right_bracket };
    TypeLiteral, type_literal: TypeLiteral { type_ };
    TypeParameter, type_parameter: TypeParameter { documentation_comment, metadata, variance_keyword, name, extends_keyword, bound };
    TypeParameterList, type_parameter_list: TypeParameterList { left_bracket, type_parameters, right_bracket };
    VariableDeclaration, variable_declaration: VariableDeclaration { documentation_comment, metadata, name, equals, initializer };
    VariableDeclarationList, variable_declaration_list: VariableDeclarationList { documentation_comment, metadata, late_keyword, keyword, type_, variables };
    VariableDeclarationStatement, variable_declaration_statement: VariableDeclarationStatement { variables, semicolon };
    WhenClause, when_clause: WhenClause { when_keyword, expression };
    WhileStatement, while_statement: WhileStatement { while_keyword, left_parenthesis, condition, right_parenthesis, body };
    WildcardPattern, wildcard_pattern: WildcardPattern { keyword, type_, name };
    WithClause, with_clause: WithClause { with_keyword, mixin_types };
    YieldStatement, yield_statement: YieldStatement { yield_keyword, star, expression, semicolon };
}

impl Ast {
    /// Appends a binary representation of this [`Ast`] to `w`.
    pub fn encode(&self, w: &mut Vec<u8>) {
        encode_tokens(&self.tokens, w);
        write_u32(w, self.kinds.len() as u32);
        for &k in &self.kinds {
            write_u8(w, k as u8);
        }
        for &slot in &self.slots {
            write_u32(w, slot);
        }
        for &parent in &self.parents {
            match parent {
                None => write_u32(w, 0),
                Some(id) => write_u32(w, id.index() as u32 + 1),
            }
        }
        write_u32(w, self.lists.len() as u32);
        for &id in &self.lists {
            write_u32(w, id.index() as u32);
        }
        write_u32(w, self.token_lists.len() as u32);
        for &id in &self.token_lists {
            write_u32(w, id.0);
        }
        encode_stores(&self.stores, w);
    }

    /// Decodes an [`Ast`] written by [`Ast::encode`].
    pub fn decode(r: &mut &[u8]) -> Option<Ast> {
        let tokens = decode_tokens(r)?;
        let node_count = read_u32(r)? as usize;
        let mut kinds = Vec::with_capacity(node_count.min(65536));
        for _ in 0..node_count {
            kinds.push(*NodeKind::ALL.get(read_u8(r)? as usize)?);
        }
        let mut slots = Vec::with_capacity(node_count.min(65536));
        for _ in 0..node_count {
            slots.push(read_u32(r)?);
        }
        let mut parents = Vec::with_capacity(node_count.min(65536));
        for _ in 0..node_count {
            let raw = read_u32(r)?;
            if raw == 0 {
                parents.push(None);
            } else {
                parents.push(Some(NodeId::from_index((raw - 1) as usize)));
            }
        }
        let lists_len = read_u32(r)? as usize;
        let mut lists = Vec::with_capacity(lists_len.min(65536));
        for _ in 0..lists_len {
            lists.push(NodeId::from_index(read_u32(r)? as usize));
        }
        let tok_lists_len = read_u32(r)? as usize;
        let mut token_lists = Vec::with_capacity(tok_lists_len.min(16384));
        for _ in 0..tok_lists_len {
            token_lists.push(TokenId(read_u32(r)?));
        }
        let stores = decode_stores(r)?;
        let mut ast = Ast::new(tokens);
        ast.kinds = kinds;
        ast.slots = slots;
        ast.parents = parents;
        ast.lists = lists;
        ast.token_lists = token_lists;
        ast.stores = stores;
        Some(ast)
    }
}
