# Timeline and process log

Running log of the port. Each entry is a merge into `main`, with the agent that did the work and
the evidence the coordinator checked before the merge. Times are local (CEST). Update this file
at each merge; the final process report is written from it.

## Process

- **Coordinator:** one Claude Opus session. It plans, writes the task prompts, verifies results
  and merges. It does not port code itself.
- **Workers:** each unit runs in its own worktree (`wt switch --create`) and branch.
  - Claude Opus: complex porting units.
  - Claude Sonnet: wiring and other simple work (from 2026-10-09 01:15).
  - Codex (`gpt-6.1-sol`, through Herdr panes): isolated units (from 2026-10-09 00:55).
- **Definition of done:** a unit is done when its differential test against the pinned Dart
  analyzer (3.13.3) passes and the coordinator has re-run it on a corpus the worker did not use.
  For units with no oracle mode yet, the analyzer's own unit tests are ported instead, with
  Dart and Rust test counts compared per file.
- **Proof that a test can fail:** each worker breaks one rule on purpose and shows the failure.
- **Merge rule:** the full workspace tests pass on `main` after every merge, in release mode
  (and debug, for units with debug-only assert tests).

## Timeline

| time | commit | unit | agent | evidence checked by the coordinator |
|---|---|---|---|---|
| 10-08 20:04 | 02f83ce | workspace, oracle, plan | coordinator | oracle smoke test |
| 10-08 20:10 | fddf8a8 | baseline benchmarks | coordinator | provisional (machine was busy) |
| 10-08 20:15 | b521e29 | editor integration research | Sonnet | report with source references |
| 10-08 20:21 | b4d1d00 | P2 diagnostics codegen | Opus | 1488/1488 codes, messages equal to Dart |
| 10-08 20:40 | a6687fd | P1 scanner + difftest | Opus | tokens 100%; 16,046/16,046 on unseen corpus |
| 10-08 20:40 | f2d6f44 | P4 project model | Opus | contexts/options 0 differences, 9 cases |
| 10-08 21:14 | 8965b13 | P3b AST model | Opus | AST dump 100% via oracle loader |
| 10-08 21:24 | 8bf593d | semantic layer design | Opus (Plan) | reviewed by the coordinator |
| 10-08 21:32 | 8d6fbfd | P3a parser | Opus | events 100%; 11,633/11,633 unseen; 891 recovery cases |
| 10-08 21:56 | 3f51ff7 | U0 element/type/flow interfaces | Opus | tests; deterministic oracle `elements` |
| 10-08 22:09 | 3fd04bc | P3c AST builder | Opus | `ast` 100%; 11,392/11,392 unseen |
| 10-08 22:21 | e375e4d | A14 constant values | Opus | 391/391 Dart tests; 17,439 ops vs analyzer |
| 10-08 22:39 | a84ce97 | A1–A5 type system | Opus | 879 ported tests |
| 10-08 22:47 | 551bcf6 | A12–A13 type analyzer, exhaustiveness | Opus | 506 tests + 480 cases |
| 10-08 23:05 | 8808aec | A6 inference | Opus | 62 tests; messages equal to `dart analyze` |
| 10-08 23:12 | 2b8450f | A7 inheritance manager | Opus | 122/139 (rest waited for linker) |
| 10-08 23:17 | 7c28cdb | A9–A11 flow analysis | Opus | 783/783 Dart tests |
| 10-08 23:20 | a46adf8 | integration fixes | coordinator | 3 cross-branch breaks fixed |
| 10-08 23:24 | aa11160 | wave B linker (C5a) | Opus | elements masked 100% on 5 corpora |
| 10-09 00:31 | 6d67cab | P9 `dartr analyze` (parse-only) | Opus | 68 byte-exact cases vs `dart analyze` |
| 10-09 00:52 | 14dd7e0 | B5 override inference | Opus | unmasked fixture test 0/6 → 6/6 |
| 10-09 00:59 | f82c5ed | options/pubspec/manifest diagnostics | Codex | 101/101; 0 differences on Flutter, visible-app |
| 10-09 01:06 | 4f7daa3 | P10a LSP foundation | Opus | 31/31 session steps vs `dart language-server` |
| 10-09 01:15 | dda1c2b | lint framework + 79 AST-only rules | Codex | 2 bugs found on unseen corpora, fixed; then exact on 6 corpora |
| 10-09 01:49 | 168d891 | non-Dart diagnostics in CLI and LSP | Sonnet | 120/120 CLI cases, 7/7 LSP steps |
| 10-09 01:49 | 2bade7d | `package:yaml` 3.1.4 port | Codex | 3,704/3,704 comparisons |
| 10-09 02:23 | ecdba76 | lints in CLI and LSP | Sonnet | 14 CLI cases, 6/6 LSP steps; lint corpora still exact |

## Incidents and lessons

- **Busy-machine benchmarks (10-08):** the first baselines ran while agents used the CPU
  (load ~11–14 on 15 cores). They are marked provisional and must be repeated on an idle machine.
- **Cross-branch integration (10-08 23:20):** each branch passed its own tests, but main did not
  build after merging two of them. The causes were a duplicate `Cargo.toml` key that git merged
  silently, a new required trait method, and debug-only assert tests. Lesson: always run the full
  workspace tests after each merge, not only the branch tests.
- **API connection errors (10-09 ~00:00):** four agents stopped at the same time (ECONNRESET, one
  stall). None had committed, so nothing was lost. Lesson: workers now commit after each step.
- **Codex TUI start failure (10-09 01:20):** the interactive Codex screen kept the prompt queued
  and then exited without output. `codex exec` in the pane worked, so isolated Codex tasks now
  run that way.
- **Unseen-corpus checks matter (10-09):** the Codex lint port passed all of its own corpora.
  It still had two bugs (`file_names` on part files, `lines_longer_than_80_chars` with escaped
  strings), and only the coordinator's runs on other corpora found them.
- **Analyzer plugins (10-09):** visible-app loads the `jaspr_lints` plugin. Plugin diagnostics
  are not covered by the plan yet; see the open item in `PLAN.md`.
