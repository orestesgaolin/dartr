#!/usr/bin/env python3
"""Differential test of `dartr format` against `dart format` on corpora.

Usage:
  tools/format_parity.py [--dartr PATH] [--work DIR] [--language-version V]
                         [--page-width N] [--show-diffs N] [--write-failures DIR]
                         [CORPUS_DIR...]

Without CORPUS_DIR, the default corpora are used:
  third_party/dart-sdk/sdk/lib
  ~/fvm/default/packages/flutter/lib
  /Users/dominik/Projects/dartr/bench/corpus/visible-app/lib

Steps, for each corpus:
  1. Copies the corpus directory to WORK/<i>_<name>/ (the originals are
     never modified; both tools only read the copies, with `-o json`).
  2. Runs `dart format -o json --language-version V [--page-width N] .` and
     `dartr format` with the same arguments in the copy, and measures the
     wall time of each.
  3. Compares each file: the formatted text in the JSON output must be
     byte-equal, or both tools must fail on the file (no JSON line: syntax
     error or formatter bug, reported on stderr). Also compares the whole
     stderr text and the exit codes.

Language version: the copy has no `.dart_tool/package_config.json` and no
`analysis_options.yaml` around it, so `dart format` would fall back to the
latest language version and page width 80. To make this explicit, both tools
always get `--language-version` (default: `latest`) and, if given,
`--page-width`. Files with a `// @dart=` comment still use that version in
both tools.

`dart` must be the pinned SDK (3.13.3) and on PATH. Build dartr first
(`cargo build --release -p dartr`). Exit code 0 if all corpora are equal.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

DEFAULT_CORPORA = [
    os.path.join(ROOT, "third_party/dart-sdk/sdk/lib"),
    os.path.expanduser("~/fvm/default/packages/flutter/lib"),
    "/Users/dominik/Projects/dartr/bench/corpus/visible-app/lib",
]


def run(cmd, cwd):
    start = time.monotonic()
    p = subprocess.run(cmd, cwd=cwd, capture_output=True, stdin=subprocess.DEVNULL)
    elapsed = time.monotonic() - start
    return p.returncode, p.stdout.decode("utf-8", "replace"), p.stderr.decode("utf-8", "replace"), elapsed


def copy_corpus(src, dest):
    if os.path.exists(dest):
        shutil.rmtree(dest)
    # Copy only directories and .dart files; hidden directories are copied
    # too, so the hidden-directory skipping is part of the comparison.
    shutil.copytree(
        src,
        dest,
        symlinks=True,
        ignore=lambda d, names: [
            n for n in names if os.path.isfile(os.path.join(d, n)) and not n.endswith(".dart")
        ],
    )


def parse_json_lines(stdout):
    """Maps path -> formatted source from `-o json` output."""
    result = {}
    for line in stdout.splitlines():
        if not line.strip():
            continue
        try:
            obj = json.loads(line)
        except json.JSONDecodeError:
            continue
        result[obj["path"]] = obj["source"]
    return result


def list_dart_files(directory):
    files = []
    for root, dirs, names in os.walk(directory):
        dirs[:] = [d for d in dirs if not d.startswith(".")]
        for n in names:
            if n.endswith(".dart"):
                files.append(os.path.relpath(os.path.join(root, n), directory))
    return sorted(files)


def first_difference(a, b):
    a_lines = a.splitlines(keepends=True)
    b_lines = b.splitlines(keepends=True)
    for i, (x, y) in enumerate(zip(a_lines, b_lines)):
        if x != y:
            return f"line {i + 1}:\n      dart:  {x!r}\n      dartr: {y!r}"
    return f"line count {len(a_lines)} vs {len(b_lines)}"


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("corpora", nargs="*", default=None)
    parser.add_argument("--dartr", default=os.path.join(ROOT, "target/release/dartr"))
    parser.add_argument("--dart", default="dart")
    parser.add_argument("--work", default=None, help="work directory (default: a new temp dir)")
    parser.add_argument("--language-version", default="latest")
    parser.add_argument("--page-width", default=None)
    parser.add_argument("--show-diffs", type=int, default=10, help="differing files to print per corpus")
    parser.add_argument("--write-failures", default=None, help="write dart/dartr outputs of differing files here")
    args = parser.parse_args()

    corpora = args.corpora or DEFAULT_CORPORA
    args.dartr = os.path.abspath(args.dartr)
    if not os.path.exists(args.dartr):
        sys.exit(f"dartr binary not found: {args.dartr} (cargo build --release -p dartr)")

    work = args.work
    if work is None:
        import tempfile

        work = tempfile.mkdtemp(prefix="format_parity_")
    os.makedirs(work, exist_ok=True)
    print(f"work directory: {work}")

    options = ["-o", "json", "--language-version", args.language_version]
    if args.page_width:
        options += ["--page-width", args.page_width]

    all_equal = True
    rows = []
    for i, corpus in enumerate(corpora):
        corpus = os.path.abspath(os.path.expanduser(corpus))
        if not os.path.isdir(corpus):
            print(f"skip (not found): {corpus}")
            continue
        name = f"{i}_{os.path.basename(corpus.rstrip('/'))}"
        copy = os.path.join(work, name)
        copy_corpus(corpus, copy)
        files = list_dart_files(copy)

        dart_code, dart_out, dart_err, dart_time = run([args.dart, "format", *options, "."], copy)
        dartr_code, dartr_out, dartr_err, dartr_time = run([args.dartr, "format", *options, "."], copy)

        dart_files = parse_json_lines(dart_out)
        dartr_files = parse_json_lines(dartr_out)

        equal = 0
        differing = []
        for f in files:
            a = dart_files.get(f)
            b = dartr_files.get(f)
            if a == b:
                equal += 1
            else:
                differing.append((f, a, b))

        stderr_equal = dart_err == dartr_err
        codes_equal = dart_code == dartr_code
        corpus_equal = not differing and stderr_equal and codes_equal
        all_equal &= corpus_equal

        print()
        print(f"== {corpus}")
        pct = 100.0 * equal / len(files) if files else 100.0
        print(f"  files equal: {equal} / {len(files)} ({pct:.2f}%)")
        print(f"  exit code: dart {dart_code}, dartr {dartr_code}{'' if codes_equal else '  (DIFFERENT)'}")
        print(f"  stderr: {'equal' if stderr_equal else 'DIFFERENT'} ({len(dart_err)} vs {len(dartr_err)} bytes)")
        throughput = f"{dart_time / dartr_time:.1f}x" if dartr_time > 0 else "-"
        print(f"  wall time: dart {dart_time:.2f}s, dartr {dartr_time:.2f}s (dartr speedup {throughput})")
        for f, a, b in differing[: args.show_diffs]:
            if a is None and b is None:
                detail = "both failed"
            elif a is None:
                detail = "dart failed, dartr formatted"
            elif b is None:
                detail = "dartr failed, dart formatted"
            else:
                detail = first_difference(a, b)
            print(f"    {f}: {detail}")
        if len(differing) > args.show_diffs:
            print(f"    ... {len(differing) - args.show_diffs} more differing files")
        if args.write_failures and (differing or not stderr_equal):
            out = os.path.join(args.write_failures, name)
            os.makedirs(out, exist_ok=True)
            with open(os.path.join(out, "dart.stderr"), "w") as fh:
                fh.write(dart_err)
            with open(os.path.join(out, "dartr.stderr"), "w") as fh:
                fh.write(dartr_err)
            for f, a, b in differing:
                base = os.path.join(out, f.replace("/", "__"))
                with open(base + ".dart.out", "w") as fh:
                    fh.write(a if a is not None else "<failed>\n")
                with open(base + ".dartr.out", "w") as fh:
                    fh.write(b if b is not None else "<failed>\n")
        rows.append((corpus, equal, len(files), dart_time, dartr_time))

    print()
    print("summary:")
    for corpus, equal, total, dart_time, dartr_time in rows:
        print(f"  {equal:6d}/{total:<6d} dart {dart_time:7.2f}s  dartr {dartr_time:7.2f}s  {corpus}")
    sys.exit(0 if all_equal else 1)


if __name__ == "__main__":
    main()
