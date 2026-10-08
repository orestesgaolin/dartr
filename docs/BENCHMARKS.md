# Benchmarks

Machine: Apple M5 Pro, 15 cores, 48 GB. `dart` 3.13.3. Cold: `~/.dartServer/.analysis-driver`
is deleted before each run. `hyperfine -w 1 -r 3`. Scripts: `bench/setup_corpus.sh`, `bench/baseline.sh`.

## Baseline: `dart analyze` (2026-10-08)

| corpus | lines in lib | mean | σ |
|---|---|---|---|
| flutter (packages/flutter) | 568,203 | 20.535 s | 0.173 s |
| flutter_tools | 188,489 | 7.515 s | 0.156 s |
| analyzer-9.0.0 (pub) | 248,920 | 7.956 s | 0.257 s |

All three corpora resolve their packages (`dart analyze` reports no issues).
