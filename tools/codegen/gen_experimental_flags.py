#!/usr/bin/env python3
"""Generates crates/dartr_parser/src/experimental_flags.rs from the pinned SDK.

Source: third_party/dart-sdk/pkg/_fe_analyzer_shared/lib/src/experiments/flags.dart
(`ExperimentalFlag`). Run from the repository root:

    python3 tools/codegen/gen_experimental_flags.py
"""
import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SRC = os.path.join(ROOT, 'third_party/dart-sdk/pkg/_fe_analyzer_shared/lib/src/experiments/flags.dart')
OUT = os.path.join(ROOT, 'crates/dartr_parser/src/experimental_flags.rs')


def version(text, default):
    text = text.strip()
    if text == 'defaultLanguageVersion':
        return default
    m = re.match(r'const Version\((\d+), (\d+)\)', text)
    return (int(m.group(1)), int(m.group(2)))


def main():
    src = open(SRC).read()
    m = re.search(r'const Version defaultLanguageVersion = const Version\((\d+), (\d+)\);', src)
    default = (int(m.group(1)), int(m.group(2)))
    flags = []
    body = src[src.index('enum ExperimentalFlag {'):]
    body = body[:body.index(';')]
    for m in re.finditer(r'\n  (\w+)\((.*?)\n  \)', body, re.S):
        ident, fields = m.groups()
        def field(name):
            return re.search(name + r': ((?:const Version\(\d+, \d+\))|[\w\']+(?:-[\w\']+)*)', fields).group(1)
        name = field('name').strip("'")
        flags.append((ident, name, field('isEnabledByDefault') == 'true',
                      field('isExpired') == 'true',
                      version(field('experimentEnabledVersion'), default),
                      version(field('experimentReleasedVersion'), default)))
    out = ['''// Dart source: pkg/_fe_analyzer_shared/lib/src/experiments/flags.dart
//
// GENERATED FILE. DO NOT EDIT. Run `python3 tools/codegen/gen_experimental_flags.py`.

//! Dart `ExperimentalFlag`: the experimental flags shared between the CFE
//! and the analyzer.
''']
    out.append(f'/// Dart `defaultLanguageVersion`.\npub const DEFAULT_LANGUAGE_VERSION: (u32, u32) = ({default[0]}, {default[1]});\n')
    out.append('/// Dart `ExperimentalFlag`.\n#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\npub enum ExperimentalFlag {')
    for f in flags:
        out.append(f'    {f[0][0].upper() + f[0][1:]},')
    out.append('}\n')
    out.append('impl ExperimentalFlag {')
    out.append('    /// All flags, in declaration order (`ExperimentalFlag.values`).')
    out.append('    pub const VALUES: &[ExperimentalFlag] = &[')
    for f in flags:
        out.append(f'        ExperimentalFlag::{f[0][0].upper() + f[0][1:]},')
    out.append('    ];\n')
    def table(fn, ty, value):
        out.append(f'    pub fn {fn}(self) -> {ty} {{')
        out.append('        match self {')
        for f in flags:
            out.append(f'            ExperimentalFlag::{f[0][0].upper() + f[0][1:]} => {value(f)},')
        out.append('        }')
        out.append('    }\n')
    out.append('    /// Dart `ExperimentalFlag.name` (the command line name).')
    table('name', "&'static str", lambda f: f'"{f[1]}"')
    out.append('    /// Dart `ExperimentalFlag.isEnabledByDefault`.')
    table('is_enabled_by_default', 'bool', lambda f: 'true' if f[2] else 'false')
    out.append('    /// Dart `ExperimentalFlag.isExpired`.')
    table('is_expired', 'bool', lambda f: 'true' if f[3] else 'false')
    out.append('    /// Dart `ExperimentalFlag.experimentEnabledVersion` (major, minor).')
    table('experiment_enabled_version', '(u32, u32)', lambda f: f'({f[4][0]}, {f[4][1]})')
    out.append('    /// Dart `ExperimentalFlag.experimentReleasedVersion` (major, minor).')
    table('experiment_released_version', '(u32, u32)', lambda f: f'({f[5][0]}, {f[5][1]})')
    out.append('}')
    with open(OUT, 'w') as fh:
        fh.write('\n'.join(out) + '\n')
    print(f'{len(flags)} flags')


if __name__ == '__main__':
    main()
