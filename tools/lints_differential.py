#!/usr/bin/env python3
# Dart source: pkg/analyzer/lib/src/dart/analysis/library_analyzer.dart (_computeLints)
"""Compare parsed or resolved lints to dart analyze; retain exact diagnostics and per-rule counts.

Fixtures: python3 tools/lints_differential.py --fixtures --output target/lints/fixtures
Corpora:  python3 tools/lints_differential.py --corpus sdk --output target/lints/sdk
          python3 tools/lints_differential.py --corpus flutter --output target/lints/flutter
"""
import argparse
from collections import Counter
from functools import cache
import json
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import time
from urllib.parse import urljoin, urlparse, unquote

ROOT = Path(__file__).resolve().parents[1]

def dart_binary():
    if os.environ.get('DARTR_DART'):
        return os.environ['DARTR_DART']
    direct = Path.home() / 'fvm/default/bin/cache/dart-sdk/bin/dart'
    return str(direct) if direct.exists() else shutil.which('dart')

def diagnostic_key(path, code, severity, offset, length, message, correction=None):
    return path, code, severity, offset, length, message, correction

def run(binary, output, fixtures=False, corpus=None, input_dir=None,
        language_version='3.13', experiments=(), package_config=True, rules_file=None, timeout=600, reuse=False,
        rust_only=False):
    started = time.monotonic()
    output.mkdir(parents=True, exist_ok=True)
    metadata = json.loads(subprocess.check_output([str(binary), '--list'], text=True, timeout=30))
    if rules_file:
        requested = {line.strip().lower() for line in Path(rules_file).read_text().splitlines() if line.strip()}
        metadata = [rule for rule in metadata if rule['name'] in requested]
        unknown = requested - {rule['name'] for rule in metadata}
        if unknown:
            raise RuntimeError(f'Unknown rules: {sorted(unknown)}')
    else:
        metadata = [rule for rule in metadata if rule.get('implemented', True)]
    rules = [rule['name'] for rule in metadata]
    if not rules:
        raise RuntimeError('No lint rules implemented')
    project = output / 'project'
    if rust_only:
        # Rerun only the Rust runner against the saved project and oracle output.
        saved = json.loads((output / 'enabled_rules.json').read_text())
        if [rule['name'] for rule in saved] != rules:
            raise RuntimeError('Saved metadata differs from the requested rules')
        version = subprocess.check_output([dart_binary(), '--version'], text=True, stderr=subprocess.STDOUT, timeout=30).strip()
        rust = subprocess.run([str(binary)], input=(output / 'request.jsonl').read_text(), text=True,
                              capture_output=True, timeout=timeout)
        (output / 'dartr.stdout.jsonl').write_text(rust.stdout)
        (output / 'dartr.stderr.log').write_text(rust.stderr)
        if rust.returncode:
            raise RuntimeError(f'Rust runner failed: {rust.returncode}, {rust.stderr[-4000:]}')
        return compare_outputs(output, saved, project, sorted((project / 'lib').rglob('*.dart')),
                               json.loads((output / 'oracle.stdout.json').read_text()), rust.stdout, version, started)
    if reuse:
        saved = json.loads((output / 'enabled_rules.json').read_text())
        if saved != metadata:
            raise RuntimeError('Saved metadata differs from the requested rules')
        version = subprocess.check_output([dart_binary(), '--version'], text=True, stderr=subprocess.STDOUT, timeout=30).strip()
        return compare_outputs(output, metadata, project, sorted((project / 'lib').rglob('*.dart')),
                               json.loads((output / 'oracle.stdout.json').read_text()),
                               (output / 'dartr.stdout.jsonl').read_text(), version, started)
    (output / 'runner_manifest.json').write_text(json.dumps({'binary':str(binary), 'sha256':hashlib.sha256(binary.read_bytes()).hexdigest()}, indent=2) + '\n')
    (output / 'enabled_rules.json').write_text(json.dumps(metadata, indent=2) + '\n')
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
    (project / 'pubspec.yaml').write_text(f"name: dartr_lint_corpus\nenvironment:\n  sdk: '>={language_version}.0 <4.0.0'\n")
    experiment_options = ('analyzer:\n  enable-experiment:\n' +
                          ''.join(f'    - {name}\n' for name in experiments)) if experiments else ''
    (project / 'analysis_options.yaml').write_text(experiment_options + 'linter:\n  rules:\n' + ''.join(f'    - {rule}\n' for rule in rules))
    # The analyzer uses package context even without external dependencies.
    if package_config:
        (project / '.dart_tool').mkdir()
        source = source.resolve()
        original_config = next((root / '.dart_tool/package_config.json'
                                for root in [source, *source.parents]
                                if (root / '.dart_tool/package_config.json').is_file()), None)
        packages = []
        source_name = 'dartr_lint_corpus'
        if original_config:
            for package in json.loads(original_config.read_text())['packages']:
                package = dict(package)
                root_uri = urljoin(original_config.as_uri(), package['rootUri'])
                package_dir = Path(unquote(urlparse(urljoin(root_uri.rstrip('/') + '/', package.get('packageUri', ''))).path)).resolve()
                package['rootUri'] = root_uri
                if package_dir == source:
                    source_name = package['name']
                    package.update(rootUri=project.resolve().as_uri() + '/', packageUri='lib/', languageVersion=language_version)
                packages.append(package)
        # The pinned analyzer oracle config supplies meta for standalone fixtures.
        if fixtures or 'fixtures_resolved' in str(source):
            oracle_config = ROOT / 'tools/oracle/.dart_tool/package_config.json'
            if not oracle_config.is_file():
                common_dir = Path(subprocess.check_output(
                    ['git', '-C', str(ROOT), 'rev-parse', '--path-format=absolute', '--git-common-dir'],
                    text=True, timeout=30).strip())
                oracle_config = common_dir.parent / 'tools/oracle/.dart_tool/package_config.json'
            if oracle_config.is_file():
                existing = {p['name'] for p in packages}
                for package in json.loads(oracle_config.read_text())['packages']:
                    if package['name'] == 'meta' and package['name'] not in existing:
                        package = dict(package)
                        package['rootUri'] = urljoin(oracle_config.resolve().as_uri(), package['rootUri'])
                        packages.append(package)
        if source_name == 'dartr_lint_corpus':
            packages.append({'name':source_name,'rootUri':'../','packageUri':'lib/','languageVersion':language_version})
        (project / 'pubspec.yaml').write_text(f"name: {source_name}\nenvironment:\n  sdk: '>={language_version}.0 <4.0.0'\n")
        (project / '.dart_tool/package_config.json').write_text(json.dumps({'configVersion':2, 'packages':packages}, indent=2))
    dart = dart_binary()
    version = subprocess.check_output([dart, '--version'], text=True, stderr=subprocess.STDOUT, timeout=30).strip()
    if '3.13.3 ' not in version:
        raise RuntimeError(f'Expected Dart 3.13.3: {version}')
    oracle = subprocess.run([dart, 'analyze', '--format=json', str(project)], text=True, capture_output=True, timeout=timeout)
    (output / 'oracle.stdout.json').write_text(oracle.stdout)
    (output / 'oracle.stderr.log').write_text(oracle.stderr)
    raw = json.loads(oracle.stdout)
    if oracle.returncode not in (0, 1, 2, 3):
        raise RuntimeError(f'dart analyze failed: {oracle.returncode}')
    files = sorted((project / 'lib').rglob('*.dart'))
    request = ''.join(json.dumps({'path':str(path.resolve()), 'enabled':rules,
                                 'languageVersion':[int(v) for v in language_version.split('.')],
                                 'experiments':list(experiments)}) + '\n' for path in files)
    (output / 'request.jsonl').write_text(request)
    rust = subprocess.run([str(binary)], input=request, text=True, capture_output=True, timeout=timeout)
    (output / 'dartr.stdout.jsonl').write_text(rust.stdout)
    (output / 'dartr.stderr.log').write_text(rust.stderr)
    if rust.returncode:
        raise RuntimeError(f'Rust runner failed: {rust.returncode}, {rust.stderr[-4000:]}')
    return compare_outputs(output, metadata, project, files, raw, rust.stdout, version, started)


def compare_outputs(output, metadata, project, files, raw, rust_stdout, version, started):
    @cache
    def relative(path):
        return str(Path(path).resolve().relative_to(project.resolve()))
    codes = {code for rule in metadata for code in rule['codes']}
    expected = Counter()
    for d in raw['diagnostics']:
        if d['code'] not in codes or d['type'] != 'LINT':
            continue
        location = d['location']
        span = location['range']
        key = diagnostic_key(relative(location['file']), d['code'], d['severity'], span['start']['offset'],
                             span['end']['offset'] - span['start']['offset'], d['problemMessage'], d.get('correctionMessage'))
        expected[key] += 1
    actual = Counter()
    panics, unresolved_parts, errors = [], [], []
    records = set()
    for line in rust_stdout.splitlines():
        record = json.loads(line)
        record_path = relative(record['path'])
        if record_path in records:
            raise RuntimeError(f'Duplicate Rust result for {record_path}')
        records.add(record_path)
        if record.get('panic'):
            panics.append({'path':record_path,'panic':record['panic']})
        if record.get('unresolvedPart'):
            unresolved_parts.append(record_path)
        if record.get('error'):
            errors.append({'path':record_path,'error':record['error']})
        for d in record['diagnostics']:
            key = diagnostic_key(relative(record['path']), d['code'], d['severity'], d['offset'], d['length'], d['message'], d.get('correction'))
            actual[key] += 1
    wanted = {relative(path) for path in files}
    if records != wanted:
        raise RuntimeError(f'Incomplete Rust output; missing files: {sorted(wanted-records)}; extra files: {sorted(records-wanted)}')
    missing, extra = expected - actual, actual - expected
    affected_files = {key[0] for key in list(missing) + list(extra)}
    rows = []
    for rule in metadata:
        rule_codes = set(rule['codes'])
        counts = [sum(count for key, count in counter.items() if key[1] in rule_codes)
                  for counter in (expected, actual, missing, extra)]
        rows.append(dict(zip(['rule','oracle','dartr','missing','extra'], [rule['name'], *counts])))
    summary = {'dart':version,'files':len(files),'rules':len(metadata),'oracle':sum(expected.values()),
               'dartr':sum(actual.values()),'matched':sum((expected & actual).values()),
               'missing':sum(missing.values()),'extra':sum(extra.values()),
               'exact_files':sum(relative(path) not in affected_files for path in files),
               'seconds':round(time.monotonic()-started, 2),
               'resolver_panics':panics,'unresolved_parts':unresolved_parts,'analysis_errors':errors,'per_rule':rows}
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
    parser.add_argument('--language-version', default='3.13')
    parser.add_argument('--enable-experiment', action='append', default=[])
    parser.add_argument('--no-package-config', action='store_true')
    parser.add_argument('--rules-file', type=Path)
    parser.add_argument('--timeout', type=int, default=600)
    parser.add_argument('--reuse', action='store_true', help='Recompare saved complete outputs without rerunning either analyzer')
    parser.add_argument('--rust-only', action='store_true', help='Rerun only the Rust runner against the saved project and oracle output')
    args = parser.parse_args()
    raise SystemExit(run(args.binary.resolve(), args.output.resolve(), args.fixtures, args.corpus,
                         args.input_dir, args.language_version, args.enable_experiment,
                         not args.no_package_config, args.rules_file, args.timeout, args.reuse,
                         args.rust_only))
