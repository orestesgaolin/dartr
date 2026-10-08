#!/usr/bin/env python3
# Dart source: pkg/analyzer/lib/src/dart/analysis/library_analyzer.dart (_computeLints)
"""Compare AST-only lints to dart analyze; retain exact diagnostics and per-rule counts.

Fixtures: python3 tools/lints_differential.py --fixtures --output target/lints/fixtures
Corpora:  python3 tools/lints_differential.py --corpus sdk --output target/lints/sdk
          python3 tools/lints_differential.py --corpus flutter --output target/lints/flutter
"""
import argparse
from collections import Counter
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]

def dart_binary():
    if os.environ.get('DARTR_DART'):
        return os.environ['DARTR_DART']
    direct = Path.home() / 'fvm/default/bin/cache/dart-sdk/bin/dart'
    return str(direct) if direct.exists() else shutil.which('dart')

def diagnostic_key(path, code, severity, offset, length, message):
    return path, code, severity, offset, length, message

def run(binary, output, fixtures=False, corpus=None, input_dir=None):
    started = time.monotonic()
    output.mkdir(parents=True, exist_ok=True)
    metadata = json.loads(subprocess.check_output([str(binary), '--list'], text=True))
    rules = [rule['name'] for rule in metadata]
    codes = {code for rule in metadata for code in rule['codes']}
    if not rules:
        raise RuntimeError('No lint rules implemented')
    (output / 'enabled_rules.json').write_text(json.dumps(metadata, indent=2) + '\n')
    project = output / 'project'
    if project.exists():
        shutil.rmtree(project)
    project.mkdir()
    if fixtures:
        source = ROOT / 'crates/dartr_lints/tests/fixtures'
    elif corpus == 'sdk':
        source = ROOT / 'third_party/dart-sdk/sdk/lib'
    elif corpus == 'flutter':
        source = Path.home() / 'fvm/default/packages/flutter/lib'
    else:
        source = Path(input_dir)
    shutil.copytree(source, project / 'lib')
    for options in project.rglob('analysis_options.yaml'):
        options.unlink()
    (project / 'pubspec.yaml').write_text("name: dartr_lint_corpus\nenvironment:\n  sdk: '>=3.13.0 <4.0.0'\n")
    (project / 'analysis_options.yaml').write_text('linter:\n  rules:\n' + ''.join(f'    - {rule}\n' for rule in rules))
    # The analyzer uses package context even without external dependencies.
    (project / '.dart_tool').mkdir()
    (project / '.dart_tool/package_config.json').write_text(json.dumps({'configVersion':2, 'packages':[
        {'name':'dartr_lint_corpus','rootUri':'../','packageUri':'lib/','languageVersion':'3.13'}]}))
    dart = dart_binary()
    version = subprocess.check_output([dart, '--version'], text=True, stderr=subprocess.STDOUT).strip()
    if '3.13.3 ' not in version:
        raise RuntimeError(f'Expected Dart 3.13.3: {version}')
    oracle = subprocess.run([dart, 'analyze', '--format=json', str(project)], text=True, capture_output=True)
    (output / 'oracle.stdout.json').write_text(oracle.stdout)
    (output / 'oracle.stderr.log').write_text(oracle.stderr)
    raw = json.loads(oracle.stdout)
    if oracle.returncode not in (0, 1, 2, 3):
        raise RuntimeError(f'dart analyze failed: {oracle.returncode}')
    files = sorted((project / 'lib').rglob('*.dart'))
    request = ''.join(json.dumps({'path':str(path.resolve()), 'enabled':rules}) + '\n' for path in files)
    rust = subprocess.run([str(binary)], input=request, text=True, capture_output=True)
    (output / 'dartr.stdout.jsonl').write_text(rust.stdout)
    (output / 'dartr.stderr.log').write_text(rust.stderr)
    if rust.returncode:
        raise RuntimeError(f'Rust runner failed: {rust.returncode}, {rust.stderr[-4000:]}')
    def relative(path):
        return str(Path(path).resolve().relative_to(project.resolve()))
    expected = Counter()
    for d in raw['diagnostics']:
        if d['code'] not in codes or d['type'] != 'LINT':
            continue
        location = d['location']
        span = location['range']
        key = diagnostic_key(relative(location['file']), d['code'], d['severity'], span['start']['offset'],
                             span['end']['offset'] - span['start']['offset'], d['problemMessage'])
        expected[key] += 1
    actual = Counter()
    for line in rust.stdout.splitlines():
        record = json.loads(line)
        for d in record['diagnostics']:
            key = diagnostic_key(relative(record['path']), d['code'], d['severity'], d['offset'], d['length'], d['message'])
            actual[key] += 1
    missing, extra = expected - actual, actual - expected
    rows = []
    for rule in metadata:
        rule_codes = set(rule['codes'])
        counts = [sum(count for key, count in counter.items() if key[1] in rule_codes)
                  for counter in (expected, actual, missing, extra)]
        rows.append(dict(zip(['rule','oracle','dartr','missing','extra'], [rule['name'], *counts])))
    summary = {'dart':version,'files':len(files),'rules':len(rules),'oracle':sum(expected.values()),
               'dartr':sum(actual.values()),'matched':sum((expected & actual).values()),
               'missing':sum(missing.values()),'extra':sum(extra.values()),
               'exact_files':sum(not any(key[0] == relative(path) for key in list(missing) + list(extra)) for path in files),
               'seconds':round(time.monotonic()-started, 2),'per_rule':rows}
    (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    (output / 'diffs.json').write_text(json.dumps({'missing':[{'diagnostic':key,'count':count} for key,count in missing.items()],
                                                'extra':[{'diagnostic':key,'count':count} for key,count in extra.items()]}, indent=2) + '\n')
    table = 'rule\toracle\tdartr\tmissing\textra\n' + ''.join('\t'.join(str(row[key]) for key in ['rule','oracle','dartr','missing','extra'])+'\n' for row in rows)
    (output / 'per_rule.tsv').write_text(table)
    print(json.dumps({key:value for key,value in summary.items() if key != 'per_rule'}, indent=2))
    print(table)
    return bool(missing or extra)

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--fixtures', action='store_true')
    group.add_argument('--corpus', choices=['sdk','flutter'])
    group.add_argument('--input-dir')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/debug/lints_dump')
    args = parser.parse_args()
    raise SystemExit(run(args.binary.resolve(), args.output.resolve(), args.fixtures, args.corpus, args.input_dir))
