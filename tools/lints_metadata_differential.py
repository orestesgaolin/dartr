#!/usr/bin/env python3
# Dart source: pkg/analyzer/lib/src/lint/registry.dart
"""Verify all registered rule metadata against the pinned Dart linter runtime."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
sys.dont_write_bytecode = True
from lints_differential import ROOT, dart_binary

def run(binary, output, packages=None):
    output.mkdir(parents=True, exist_ok=True)
    if packages is None:
        configured = os.environ.get('DARTR_ORACLE_PACKAGE_CONFIG')
        packages = Path(configured) if configured else ROOT / 'tools/oracle/.dart_tool/package_config.json'
        if not packages.exists() and not configured:
            # A worktree does not inherit the main checkout's ignored pub config.
            common = Path(subprocess.check_output(['git','rev-parse','--git-common-dir'],cwd=ROOT,text=True).strip())
            packages = common.parent / 'tools/oracle/.dart_tool/package_config.json'
    if not packages.exists():
        raise RuntimeError('Run dart pub get in tools/oracle, or set DARTR_ORACLE_PACKAGE_CONFIG')
    version = subprocess.check_output([dart_binary(), '--version'], text=True, stderr=subprocess.STDOUT)
    if '3.13.3 ' not in version:
        raise RuntimeError(f'Expected Dart 3.13.3: {version}')
    oracle = subprocess.run([dart_binary(), f'--packages={packages.resolve()}',
                            str(ROOT / 'tools/oracle/bin/lints_metadata.dart')], text=True, capture_output=True)
    (output / 'oracle.json').write_text(oracle.stdout)
    (output / 'oracle.stderr.log').write_text(oracle.stderr)
    if oracle.returncode:
        raise RuntimeError(oracle.stderr)
    rust = subprocess.check_output([str(binary), '--metadata'], text=True)
    (output / 'dartr.json').write_text(rust)
    expected, actual = json.loads(oracle.stdout), json.loads(rust)
    diffs = []
    if len(expected) != len(actual):
        diffs.append({'count':{'oracle':len(expected),'dartr':len(actual)}})
    for i, (e, a) in enumerate(zip(expected, actual)):
        if e != a:
            diffs.append({'index':i, 'oracle':e, 'dartr':a})
    (output / 'diffs.json').write_text(json.dumps(diffs,indent=2)+'\n')
    summary = {'rules':len(expected),'matched':len(expected)-len(diffs),'differences':len(diffs)}
    (output / 'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps(summary,indent=2))
    return bool(diffs)

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, default=ROOT/'target/debug/lints_dump')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--packages', type=Path)
    args = parser.parse_args()
    raise SystemExit(run(args.binary.resolve(),args.output.resolve(),args.packages))
