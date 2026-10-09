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
