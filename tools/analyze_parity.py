#!/usr/bin/env python3
"""Differential test of `dartr analyze` against `dart analyze` on a corpus.

Usage:
  tools/analyze_parity.py [--dartr PATH] [--work DIR] [--limit N] SOURCE_DIR...

Steps:
  1. Copies the `.dart` files of each SOURCE_DIR into WORK/full/<i>_<name>/
     (no pubspec: the files get the current language version, like files
     outside of a package).
  2. Runs `dart analyze --format=json` and `dartr analyze --format=json` on
     WORK/full. dartr only reports parse diagnostics (parse-only provider),
     so the comparison is filtered: for each file, the diagnostics of the
     real tool whose type is SYNTACTIC_ERROR (plus `duplicate_ignore` and
     `unignorable_ignore`, which come from the ignore comments) must equal
     the dartr diagnostics of that file.
  3. "Pure" files are files where every diagnostic of the real tool is of
     that kind. Copies the pure files that have diagnostics into
     WORK/subset/ and compares all three formats (stdout, stderr) and the
     exit codes byte for byte, without any filter.

`dart` must be the pinned SDK (3.13.3) and on PATH. Exit code 0 if all
comparisons are equal.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import time

IGNORE_CODES = {"duplicate_ignore", "unignorable_ignore"}


def is_syntactic(d):
    return d["type"] == "SYNTACTIC_ERROR" or d["code"] in IGNORE_CODES


def run(cmd, cwd):
    start = time.monotonic()
    p = subprocess.run(cmd, cwd=cwd, capture_output=True)
    return p.returncode, p.stdout.decode("utf-8", "replace"), p.stderr.decode("utf-8", "replace"), time.monotonic() - start


def copy_corpus(sources, dest, limit):
    if os.path.exists(dest):
        shutil.rmtree(dest)
    os.makedirs(dest)
    count = 0
    for i, src in enumerate(sources):
        src = os.path.abspath(src)
        base = os.path.join(dest, f"{i}_{os.path.basename(src.rstrip('/'))}")
        for root, dirs, files in os.walk(src):
            dirs[:] = sorted(d for d in dirs if not d.startswith("."))
            for f in sorted(files):
                if not f.endswith(".dart"):
                    continue
                if limit and count >= limit:
                    return count
                rel = os.path.relpath(os.path.join(root, f), src)
                target = os.path.join(base, rel)
                os.makedirs(os.path.dirname(target), exist_ok=True)
                shutil.copyfile(os.path.join(root, f), target)
                count += 1
    return count


def by_file(diagnostics):
    result = {}
    for d in diagnostics:
        result.setdefault(d["location"]["file"], []).append(d)
    return result


def key(d):
    r = d["location"]["range"]
    return (d["code"], d["severity"], d["type"], r["start"]["offset"], r["end"]["offset"],
            r["start"]["line"], r["start"]["column"], d["problemMessage"], d.get("correctionMessage"))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dartr", default=os.path.join(os.path.dirname(__file__), "..", "target", "release", "dartr"))
    ap.add_argument("--work", default=None)
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("sources", nargs="+")
    args = ap.parse_args()
    dartr = os.path.abspath(args.dartr)
    work = os.path.realpath(args.work or os.path.join(os.environ.get("TMPDIR", "/tmp"), "dartr_analyze_parity"))
    os.makedirs(work, exist_ok=True)

    full = os.path.join(work, "full")
    n = copy_corpus(args.sources, full, args.limit)
    print(f"corpus: {n} files in {full}")

    code_d, out_d, err_d, t_d = run(["dart", "analyze", "--format=json", full], work)
    code_r, out_r, err_r, t_r = run([dartr, "analyze", "--format=json", full], work)
    print(f"dart analyze: exit {code_d}, {t_d:.1f}s; dartr analyze (parse-only): exit {code_r}, {t_r:.1f}s")
    real = by_file(json.loads(out_d)["diagnostics"])
    mine = by_file(json.loads(out_r)["diagnostics"])

    files = sorted(set(real) | set(mine))
    total_real = sum(len(v) for v in real.values())
    syntactic_real = sum(1 for v in real.values() for d in v if is_syntactic(d))
    mismatched = []
    for f in files:
        expected = sorted(key(d) for d in real.get(f, []) if is_syntactic(d))
        actual = sorted(key(d) for d in mine.get(f, []))
        if expected != actual:
            mismatched.append((f, expected, actual))
    print(f"real diagnostics: {total_real}, syntactic: {syntactic_real}; dartr: {sum(len(v) for v in mine.values())}")
    print(f"files with diagnostics: {len(files)}; filtered mismatches: {len(mismatched)}")
    for f, e, a in mismatched[:20]:
        print(f"  MISMATCH {f}")
        for d in sorted(set(e) - set(a))[:3]:
            print(f"    only dart:  {d}")
        for d in sorted(set(a) - set(e))[:3]:
            print(f"    only dartr: {d}")

    # Exact comparison on the pure files.
    pure = [f for f in files if f in real and all(is_syntactic(d) for d in real[f])
            and f not in {m[0] for m in mismatched}]
    subset = os.path.join(work, "subset")
    failures = 0
    for attempt in range(2):
        if os.path.exists(subset):
            shutil.rmtree(subset)
        for f in pure:
            target = os.path.join(subset, os.path.relpath(f, full))
            os.makedirs(os.path.dirname(target), exist_ok=True)
            shutil.copyfile(f, target)
        _, out, _, _ = run(["dart", "analyze", "--format=json", subset], work)
        diags = json.loads(out)["diagnostics"]
        impure = {d["location"]["file"] for d in diags if not is_syntactic(d)}
        if not impure:
            break
        drop = {os.path.join(full, os.path.relpath(f, subset)) for f in impure}
        pure = [f for f in pure if f not in drop]
        print(f"  subset attempt {attempt}: dropped {len(drop)} files with non-syntactic diagnostics")
    print(f"exact comparison on {len(pure)} pure files in {subset}")
    variants = [[], ["--format=machine"], ["--format=json"], ["--fatal-infos"], ["--no-fatal-warnings"]]
    for cwd, target in [(work, [subset]), (subset, [])]:
        for v in variants:
            cmd = v + target
            d = run(["dart", "analyze"] + cmd, cwd)
            r = run([dartr, "analyze"] + cmd, cwd)
            same = d[0] == r[0] and d[1] == r[1] and d[2] == r[2]
            label = " ".join(cmd).replace(work, "$WORK") + f" (cwd {cwd.replace(work, '$WORK')})"
            print(f"  {'OK  ' if same else 'DIFF'} analyze {label}: exit dart={d[0]} dartr={r[0]}, "
                  f"stdout {len(d[1])}/{len(r[1])} bytes, {d[3]:.1f}s/{r[3]:.1f}s")
            if not same:
                failures += 1
                with open(os.path.join(work, f"diff_{failures}_dart.txt"), "w") as fh:
                    fh.write(d[1] + "\n--- stderr\n" + d[2])
                with open(os.path.join(work, f"diff_{failures}_dartr.txt"), "w") as fh:
                    fh.write(r[1] + "\n--- stderr\n" + r[2])
    return 1 if mismatched or failures else 0


if __name__ == "__main__":
    sys.exit(main())
