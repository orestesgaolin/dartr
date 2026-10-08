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
