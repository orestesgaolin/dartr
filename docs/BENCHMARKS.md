# Benchmarks

Machine: Apple M5 Pro, 15 cores, 48 GB. `dart` 3.13.3. Cold: `~/.dartServer/.analysis-driver`
is replaced by an isolated `--cache` directory that is deleted before each run. `hyperfine -w 1 -r 3`. Scripts: `bench/setup_corpus.sh`, `bench/baseline.sh`.

## Baseline: `dart analyze` (2026-10-08, PROVISIONAL)

These runs happened while other agents used the CPU (load average ~11-14 on 15 cores).
They must be repeated on an idle machine before any comparison.

| corpus | lines in lib | mean | σ |
|---|---|---|---|
| flutter (packages/flutter) | 568,203 | 20.535 s | 0.173 s |
| flutter_tools | 188,489 | 7.515 s | 0.156 s |
| analyzer-9.0.0 (pub) | 248,920 | 7.956 s | 0.257 s |
| visible-app @ 005d4cfad (all Dart files, pub workspace + nested packages, analyzer plugins) | 1,559,637 | 80.03 s (1 run) | - |

All three corpora resolve their packages (`dart analyze` reports no issues).

visible-app reports 20 issues (lint infos), no errors. Its nested packages outside the root
workspace are resolved by `bench/setup_corpus.sh`; without that, the clone reports 588
URI_DOES_NOT_EXIST errors. Line counts: flutter, flutter_tools, analyzer count `lib/` only;
visible-app counts all Dart files except `.dart_tool`.

## `dartr analyze`, PARSE-ONLY pipeline (2026-10-09) — NOT comparable to the baseline

`dartr analyze` with `ParseOnlyProvider` (crate `dartr_cli`): context discovery, file list,
scan + parse + AST build of every analyzed `.dart` file in parallel (rayon), ignore comments,
`errors:` processing and output. No element model, resolution, verifiers or lints, so these
numbers are a lower bound for the I/O + parse part only. `hyperfine -N -w 2 -r 10`, release
build, `--format=json`; load average ~21 during the runs (other agents). Phase split from
`DARTR_TIMINGS=1` (single run).

| corpus | analyzed files | mean | σ | contexts / file list / parse |
|---|---|---|---|---|
| flutter (packages/flutter) | 1,697 | 142.2 ms | 8.1 ms | 17 / 18 / 96 ms |
| flutter_tools | 912 | 110.4 ms | 3.7 ms | 28 / 24 / 69 ms |
| analyzer-9.0.0 (pub) | 1,660 | 87.5 ms | 2.0 ms | 11 / 8 / 76 ms |
| visible-app @ 005d4cfad | 3,090 | 472.0 ms | 61.3 ms | 141 / 88 / 272 ms |

Note: `bench/setup_corpus.sh` runs `flutter pub get` in `packages/hrv` of visible-app, and
Flutter then adds `analyzer: exclude: [build/**]` to that package's `analysis_options.yaml`.
The corpus therefore differs from the commit in this one file. It is reproducible and the
same for `dart analyze` and `dartr analyze`.

## Baseline on an idle machine (2026-10-09 08:31–08:39)

Load average before the run: 2.75. One other process used one core (a hung `dartr` run of the
resolver agent, 1 of 15 cores); no agent was working. `hyperfine -i -w 1 -r 3`, isolated
`--cache` directory deleted before each run. Line counts are all Dart files of the analyzed tree
except `.dart_tool` (the provisional table above counted `lib/` only for the first three).

| corpus | Dart lines | mean | σ |
|---|---|---|---|
| flutter (packages/flutter) | 1,309,663 | 19.631 s | 0.243 s |
| flutter_tools | 455,099 | 7.554 s | 0.172 s |
| analyzer-9.0.0 (pub) | 909,249 | 7.119 s | 0.150 s |
| visible-app @ 005d4cfad | 1,559,637 | 72.791 s | 0.211 s |

These replace the provisional numbers as the baseline for comparisons. Raw log:
`bench/results/baseline-idle.log` (not committed; results are git-ignored).

## `dartr analyze` against `dart analyze` (2026-10-10 19:45–20:30, busy machine)

Release build of main at db36afb (`dartr` with the full pipeline: errors, warnings, lints, analyzer
plugins). `bench/compare.sh`: `hyperfine -i -N -w 1 -r 3`, run from the project directory. "dart, warm"
uses the normal `~/.dartServer` cache, warmed by the warm-up run; "dart, empty" uses an isolated
`--cache` directory deleted before each run (`COLD=1`). Both tools report the same diagnostics on all
four projects (checked with `tools/analyze_compare.py`). Build agents used the machine during the runs:
load average 8 to 18 on 15 cores.

| corpus | dart, warm: mean (min–max) | dart, empty: mean (min–max) | dartr: mean (min–max) |
|---|---|---|---|
| visible-app | 78.775 s (57.102–93.537) | 97.083 s (91.528–102.357) | 24.945 s (24.323–25.557) |
| flutter (packages/flutter) | 4.325 s (3.847–4.848) | 27.990 s (24.857–29.695) | 7.380 s (7.297–7.483) |
| analyzer-9.0.0 | 3.299 s (2.325–4.959) | 8.710 s (8.529–9.031) | 1.424 s (1.418–1.431) |
| flutter_tools | 2.673 s (2.466–2.804) | 9.467 s (9.397–9.550) | 3.665 s (3.628–3.698) |

CPU time (user + system, mean per run):

| corpus | dart, warm | dart, empty | dartr |
|---|---|---|---|
| visible-app | 76.0 + 15.5 s | 122.0 + 17.7 s | 92.7 + 68.2 s |
| flutter | 3.8 + 2.2 s | 29.0 + 7.3 s | 26.8 + 38.8 s |
| analyzer-9.0.0 | 3.0 + 2.0 s | 9.4 + 3.3 s | 6.9 + 1.1 s |
| flutter_tools | 1.8 + 1.4 s | 10.5 + 3.4 s | 7.4 + 22.5 s |

Notes:
- The earlier single runs (README until this date) had Flutter at 15.4 s for `dart analyze`; with a
  properly warmed cache it is 4.3 s, so dartr (7.4 s) is slower there.
- dartr's system time (38.8 s on flutter, 22.5 s on flutter_tools) is far above its wall time. It
  points to threads spinning or contending; the optimization work starts there.
- Raw results: `bench/results/compare-*.json` (git-ignored).
