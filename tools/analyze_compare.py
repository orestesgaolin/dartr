#!/usr/bin/env python3
"""Compares the diagnostics of `dartr analyze` and `dart analyze` on a folder.

Usage:
  tools/analyze_compare.py [--dartr PATH] [--timeout S] [--top N] [--out DIR] TARGET...

Runs `dart analyze --format=machine TARGET` and `dartr analyze
--format=machine TARGET` (with `nice -n 10`, one after the other) and
compares the diagnostics as multisets. A diagnostic is matched when code,
file, line, column and length are equal; matched diagnostics with another
severity or message are counted separately. Prints the totals, matched,
missing (only dart), extra (only dartr), the top codes of each group, the
exit codes and the wall time of each run.

`dart` must be the pinned SDK (3.13.3) and on PATH.
"""

import argparse
import collections
import os
import subprocess
import sys
import time


def run(cmd, timeout):
    start = time.monotonic()
    p = subprocess.run(["nice", "-n", "10"] + cmd, capture_output=True, timeout=timeout)
    return p, time.monotonic() - start


def parse(stdout):
    result = []
    for line in stdout.decode("utf-8", "replace").splitlines():
        parts = line.split("|", 7)
        if len(parts) != 8:
            continue
        severity, kind, code, path, line_, col, length, message = parts
        result.append(((code, path, line_, col, length), (severity, kind, message)))
    return result


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dartr", default=os.path.join(os.path.dirname(__file__), "..", "target", "release", "dartr"))
    ap.add_argument("--timeout", type=int, default=1800)
    ap.add_argument("--top", type=int, default=12)
    ap.add_argument("--out", help="write missing.txt and extra.txt here")
    ap.add_argument("targets", nargs="+")
    args = ap.parse_args()

    targets = [os.path.abspath(t) for t in args.targets]
    dart, dart_time = run(["dart", "analyze", "--format=machine"] + targets, args.timeout)
    dartr, dartr_time = run([os.path.abspath(args.dartr), "analyze", "--format=machine"] + targets, args.timeout)
    expected = parse(dart.stderr + dart.stdout)
    actual = parse(dartr.stderr + dartr.stdout)

    pending = collections.defaultdict(list)
    for key, rest in expected:
        pending[key].append(rest)
    matched = 0
    different = 0
    extra = []
    for key, rest in actual:
        if pending.get(key):
            other = pending[key].pop(0)
            matched += 1
            if other != rest:
                different += 1
        else:
            extra.append((key, rest))
    missing = [(key, r) for key, rests in pending.items() for r in rests]

    def top(items):
        c = collections.Counter(key[0] for key, _ in items)
        return ", ".join(f"{code} {n}" for code, n in c.most_common(args.top))

    print(f"targets: {' '.join(targets)}")
    print(f"dart:  {len(expected)} diagnostics, exit {dart.returncode}, {dart_time:.1f} s")
    print(f"dartr: {len(actual)} diagnostics, exit {dartr.returncode}, {dartr_time:.1f} s")
    print(f"matched: {matched} ({different} with another severity or message)")
    print(f"missing (dart only): {len(missing)}")
    print(f"  top: {top(missing)}")
    print(f"extra (dartr only): {len(extra)}")
    print(f"  top: {top(extra)}")
    print(f"exit codes agree: {dart.returncode == dartr.returncode}")
    print(f"top codes (dart): {top(expected)}")
    if args.out:
        os.makedirs(args.out, exist_ok=True)
        for name, items in (("missing.txt", missing), ("extra.txt", extra)):
            with open(os.path.join(args.out, name), "w") as f:
                for key, rest in sorted(items):
                    f.write("|".join(key) + "|" + "|".join(rest) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
