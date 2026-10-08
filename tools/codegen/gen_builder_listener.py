#!/usr/bin/env python3
"""Generates the `Listener` implementation of the AST builder.

The AST builder (crates/dartr_ast_builder) implements the events as inherent
methods of `AstBuilder` with the arguments of the `Listener` method without
the token arena (`fn end_block(&mut self, count: i32, ...)`); the builder
reads and changes the tokens through `self.ast.tokens`. This script writes
crates/dartr_ast_builder/src/ast_builder/listener_impl.rs: for every
`Listener` method that has an inherent method of the same name, a trait
method that moves the parser's token arena into `self.ast.tokens`, calls the
inherent method, and moves the arena back.

Run from the repository root after adding or removing an event method:

    python3 tools/codegen/gen_builder_listener.py
"""
import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
LISTENER = os.path.join(ROOT, 'crates/dartr_parser/src/listener.rs')
BUILDER = os.path.join(ROOT, 'crates/dartr_ast_builder/src')
OUT = os.path.join(BUILDER, 'ast_builder/listener_impl.rs')

SIG = re.compile(r'^    fn (\w+)\(&mut self, tokens: &mut Tokens(?:, (.*?))?\)(?: -> ([^{]*))? \{')
INHERENT = re.compile(r'^\s*(?:pub(?:\(crate\))? )?fn (\w+)\(')


def split_args(args):
    if not args:
        return []
    out, depth, cur = [], 0, ''
    for ch in args:
        if ch in '<([':
            depth += 1
        elif ch in '>)]':
            depth -= 1
        if ch == ',' and depth == 0:
            out.append(cur.strip())
            cur = ''
        else:
            cur += ch
    if cur.strip():
        out.append(cur.strip())
    return out


def main():
    methods = []
    with open(LISTENER) as f:
        for line in f:
            m = SIG.match(line)
            if m:
                methods.append((m.group(1), split_args(m.group(2)), (m.group(3) or '').strip()))

    inherent = set()
    for dirpath, _, files in os.walk(BUILDER):
        for name in files:
            path = os.path.join(dirpath, name)
            if not name.endswith('.rs') or path == OUT:
                continue
            with open(path) as f:
                for line in f:
                    m = INHERENT.match(line)
                    if m:
                        inherent.add(m.group(1))

    out = []
    out.append('// Dart source: pkg/analyzer/lib/src/fasta/ast_builder.dart\n')
    out.append('//\n')
    out.append('// GENERATED FILE. DO NOT EDIT. Run `python3 tools/codegen/gen_builder_listener.py`.\n\n')
    out.append('//! The `Listener` implementation of [`AstBuilder`]: every event that\n')
    out.append('//! the builder implements moves the token arena of the parser into\n')
    out.append('//! `self.ast.tokens` for the duration of the event and calls the inherent\n')
    out.append('//! method of the same name. The other events are the no-op defaults of\n')
    out.append('//! the trait.\n\n')
    out.append('#![allow(unused_imports)]\n\n')
    out.append('use dartr_diagnostics::cfe::{CfeCode, CfeMessage};\n')
    out.append('use dartr_parser::assert::Assert;\n')
    out.append('use dartr_parser::block_kind::BlockKind;\n')
    out.append('use dartr_parser::constructor_reference_context::ConstructorReferenceContext;\n')
    out.append('use dartr_parser::declaration_kind::{DeclarationHeaderKind, DeclarationKind};\n')
    out.append('use dartr_parser::experimental_features::ExperimentalFlag;\n')
    out.append('use dartr_parser::formal_parameter_kind::FormalParameterKind;\n')
    out.append('use dartr_parser::identifier_context::IdentifierContext;\n')
    out.append('use dartr_parser::listener::Listener;\n')
    out.append('use dartr_parser::member_kind::MemberKind;\n')
    out.append('use dartr_syntax::{TokenId, Tokens};\n\n')
    out.append('use super::AstBuilder;\n\n')
    out.append('impl Listener for AstBuilder {\n')
    count = 0
    for name, args, ret in methods:
        if name not in inherent:
            continue
        if count:
            out.append('\n')
        count += 1
        params = ''.join(', ' + a for a in args)
        names = [a.split(':')[0].strip() for a in args]
        call_args = ''.join(', ' + n for n in names)
        ret_sig = f' -> {ret}' if ret else ''
        out.append(f'    #[inline]\n')
        out.append(f'    fn {name}(&mut self, tokens: &mut Tokens{params}){ret_sig} {{\n')
        out.append('        std::mem::swap(tokens, &mut self.ast.tokens);\n')
        if ret:
            out.append(f'        let result = AstBuilder::{name}(self{call_args});\n')
        else:
            out.append(f'        AstBuilder::{name}(self{call_args});\n')
        out.append('        std::mem::swap(tokens, &mut self.ast.tokens);\n')
        if ret:
            out.append('        result\n')
        out.append('    }\n')
    out.append('}\n')
    with open(OUT, 'w') as f:
        f.write(''.join(out))
    print(f'{count} of {len(methods)} listener methods forwarded')


if __name__ == '__main__':
    main()
