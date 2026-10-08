#!/usr/bin/env python3
"""Generates crates/dartr_syntax/src/token_type.rs from the pinned Dart SDK.

Source: third_party/dart-sdk/pkg/_fe_analyzer_shared/lib/src/scanner/token.dart
(`TokenType` and `Keyword` constants). Run from the repository root:

    python3 tools/codegen/gen_token_types.py
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SRC = os.path.join(ROOT, 'third_party/dart-sdk/pkg/_fe_analyzer_shared/lib/src/scanner/token.dart')
OUT = os.path.join(ROOT, 'crates/dartr_syntax/src/token_type.rs')

PRECEDENCE = {
    'NO_PRECEDENCE': 0, 'ASSIGNMENT_PRECEDENCE': 1, 'CASCADE_PRECEDENCE': 2,
    'CONDITIONAL_PRECEDENCE': 3, 'IF_NULL_PRECEDENCE': 4, 'LOGICAL_OR_PRECEDENCE': 5,
    'LOGICAL_AND_PRECEDENCE': 6, 'EQUALITY_PRECEDENCE': 7, 'RELATIONAL_PRECEDENCE': 8,
    'BITWISE_OR_PRECEDENCE': 9, 'BITWISE_XOR_PRECEDENCE': 10, 'BITWISE_AND_PRECEDENCE': 11,
    'SHIFT_PRECEDENCE': 12, 'ADDITIVE_PRECEDENCE': 13, 'MULTIPLICATIVE_PRECEDENCE': 14,
    'PREFIX_PRECEDENCE': 15, 'POSTFIX_PRECEDENCE': 16, 'SELECTOR_PRECEDENCE': 17,
}


def dart_string(lit):
    lit = lit.strip()
    q = lit[0]
    assert lit[-1] == q, lit
    body = lit[1:-1]
    return body.replace('\\$', '$').replace("\\'", "'").replace('\\\\', '\\')


def parse(src):
    types = {}
    # static const TokenType NAME = const TokenType( ... );
    for m in re.finditer(r'static const (TokenType|Keyword) (\w+) = const (TokenType|Keyword)\((.*?)\);', src, re.S):
        decl_kind, const_name, _, body = m.groups()
        body = re.sub(r'/\*.*?\*/', '', body)
        args = [a.strip() for a in re.split(r',\s*\n', body.strip().rstrip(',')) if a.strip()]
        positional = [a for a in args if not re.match(r"^\w+:", a)][1:]
        named = dict((a.split(':', 1)[0].strip(), a.split(':', 1)[1].strip()) for a in args if re.match(r'^\w+:', a))
        index = int(re.search(r'/\*\s*index\s*=\s*\*/\s*(\d+)', m.group(4)).group(1))
        if decl_kind == 'TokenType':
            lexeme, name, prec, kind = positional
            entry = dict(
                index=index, const=const_name, lexeme=dart_string(lexeme), name=dart_string(name),
                precedence=PRECEDENCE[prec], kind=kind, keyword_style=None,
                is_operator=named.get('isOperator') == 'true',
                is_binary_operator=named.get('isBinaryOperator') == 'true',
                is_modifier=named.get('isModifier') == 'true',
                is_top_level_keyword=named.get('isTopLevelKeyword') == 'true',
                is_user_definable_operator=named.get('isUserDefinableOperator') == 'true',
                string_value_null=named.get('stringValueShouldBeNull') == 'true',
                compound=named.get('binaryOperatorOfCompoundAssignment', '').replace('TokenType.', '') or None,
            )
        else:
            lexeme, name, style = positional
            entry = dict(
                index=index, const=const_name, lexeme=dart_string(lexeme), name=dart_string(name),
                precedence=PRECEDENCE[named.get('precedence', 'NO_PRECEDENCE')], kind='KEYWORD_TOKEN',
                keyword_style=style.replace('KeywordStyle.', ''),
                is_operator=False, is_binary_operator=False,
                is_modifier=named.get('isModifier') == 'true',
                is_top_level_keyword=named.get('isTopLevelKeyword') == 'true',
                is_user_definable_operator=False, string_value_null=False, compound=None,
            )
            # Keyword.name is lexeme.toUpperCase().
            assert entry['name'] == entry['lexeme'].upper(), entry
        assert index not in types or types[index]['const'] == const_name, (index, const_name)
        types[index] = entry
    return types


def rust_str(s):
    return '"' + s.replace('\\', '\\\\').replace('"', '\\"') + '"'


def main():
    src = open(SRC).read()
    types = parse(src)
    by_const = {t['const'] + ('#kw' if t['keyword_style'] else ''): t for t in types.values()}
    out = []
    w = out.append
    w('// GENERATED FILE. DO NOT EDIT. Run `python3 tools/codegen/gen_token_types.py`.')
    w('//')
    w('// Dart source: pkg/_fe_analyzer_shared/lib/src/scanner/token.dart')
    w('// (`TokenType`, `Keyword`, `_tokenTypesByIndex`).')
    w('')
    w('use crate::token_constants::*;')
    w('')
    w('/// The type of a token: a port of Dart `TokenType` and `Keyword`.')
    w('///')
    w('/// The value is the Dart `TokenType.index`, so that `TokenType(i)` equals')
    w('/// `_tokenTypesByIndex[i]`. Keywords are token types too (as in Dart, where')
    w('/// `Keyword extends TokenType`); their constants are on [`Keyword`].')
    w('#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]')
    w('pub struct TokenType(pub u8);')
    w('')
    w('/// Namespace for the keyword token types (Dart `Keyword.X`).')
    w('pub struct Keyword;')
    w('')
    w('/// Dart `KeywordStyle`.')
    w('#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]')
    w('pub enum KeywordStyle {')
    w('    Reserved,')
    w('    BuiltIn,')
    w('    Pseudo,')
    w('}')
    w('')
    w('impl TokenType {')
    for i in sorted(types):
        t = types[i]
        if t['keyword_style']:
            continue
        w(f"    pub const {t['const']}: TokenType = TokenType({i});")
    w('    pub const AS: TokenType = Keyword::AS;')
    w('    pub const IS: TokenType = Keyword::IS;')
    w('}')
    w('')
    w('impl Keyword {')
    kws = [types[i] for i in sorted(types) if types[i]['keyword_style']]
    for t in kws:
        w(f"    pub const {t['const']}: TokenType = TokenType({t['index']});")
    w('')
    w('    /// Dart `Keyword.values`, in declaration order.')
    w('    pub const VALUES: &[TokenType] = &[')
    for t in kws:
        w(f"        Keyword::{t['const']},")
    w('    ];')
    w('}')
    w('')
    w('pub(crate) struct TokenTypeInfo {')
    w('    pub lexeme: &\'static str,')
    w('    pub name: &\'static str,')
    w('    pub precedence: u8,')
    w('    pub kind: i32,')
    w('    pub flags: u8,')
    w('    pub keyword_style: Option<KeywordStyle>,')
    w('    pub compound: Option<TokenType>,')
    w('}')
    w('')
    w('pub(crate) const F_OPERATOR: u8 = 1;')
    w('pub(crate) const F_BINARY_OPERATOR: u8 = 2;')
    w('pub(crate) const F_MODIFIER: u8 = 4;')
    w('pub(crate) const F_TOP_LEVEL_KEYWORD: u8 = 8;')
    w('pub(crate) const F_USER_DEFINABLE_OPERATOR: u8 = 16;')
    w('pub(crate) const F_STRING_VALUE_NULL: u8 = 32;')
    w('')
    w('const UNUSED_INFO: TokenTypeInfo = TokenTypeInfo {')
    u = types[255]
    w(f"    lexeme: {rust_str(u['lexeme'])},")
    w(f"    name: {rust_str(u['name'])},")
    w('    precedence: 0,')
    w('    kind: EOF_TOKEN,')
    w('    flags: 0,')
    w('    keyword_style: None,')
    w('    compound: None,')
    w('};')
    w('')
    w('pub(crate) static TOKEN_TYPE_INFO: [TokenTypeInfo; 256] = {')
    w('    let mut t = [UNUSED_INFO; 256];')
    for i in sorted(types):
        if i == 255:
            continue
        t = types[i]
        flags = []
        for key, f in [('is_operator', 'F_OPERATOR'), ('is_binary_operator', 'F_BINARY_OPERATOR'),
                       ('is_modifier', 'F_MODIFIER'), ('is_top_level_keyword', 'F_TOP_LEVEL_KEYWORD'),
                       ('is_user_definable_operator', 'F_USER_DEFINABLE_OPERATOR'),
                       ('string_value_null', 'F_STRING_VALUE_NULL')]:
            if t[key]:
                flags.append(f)
        style = {'reserved': 'Some(KeywordStyle::Reserved)', 'builtIn': 'Some(KeywordStyle::BuiltIn)',
                 'pseudo': 'Some(KeywordStyle::Pseudo)', None: 'None'}[t['keyword_style']]
        compound = f"Some(TokenType::{t['compound']})" if t['compound'] else 'None'
        w(f'    t[{i}] = TokenTypeInfo {{')
        w(f"        lexeme: {rust_str(t['lexeme'])},")
        w(f"        name: {rust_str(t['name'])},")
        w(f"        precedence: {t['precedence']},")
        w(f"        kind: {t['kind']},")
        w(f"        flags: {' | '.join(flags) if flags else '0'},")
        w(f'        keyword_style: {style},')
        w(f'        compound: {compound},')
        w('    };')
    w('    t')
    w('};')
    w('')
    open(OUT, 'w').write('\n'.join(out))
    print(f'wrote {OUT}: {len(types)} token types, {len(kws)} keywords')


if __name__ == '__main__':
    sys.exit(main())
