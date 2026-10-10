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
| 10-09 02:23 | ecdba76 | lints in CLI and LSP | Sonnet | 14 CLI cases, 6/6 LSP steps; lint corpora still exact (full workspace tests NOT run: see incidents) |
| 10-09 08:39 | 4d2ec87 | idle-machine baseline | coordinator | load 2.75 before the run |
| 10-09 08:45 | — | private repo `orestesgaolin/dartr`, main pushed | Sonnet | `ls-remote` = local main |
| 10-09 09:20 | 7ea3b13 | fix_data.yaml validation, plugin research | Sonnet | 154/154 vs `dart analyze`; full workspace tests: only the known LSP failure |
| 10-09 09:40 | 0af4509 | legacy protocol foundation (9/59 requests) | Codex | dartdev + IntelliJ-like sessions vs `dart language-server --protocol=analyzer` |
| 10-09 10:10 | e08c69d | wave C resolver core (C1, C2, C11 skeleton) | Opus | flutter_tools (unseen): NamedType 100%, SimpleIdentifier 54% (C3–C8 stubbed) |
| 10-09 10:45 | 3e521a9 | CI (GitHub Actions) + tolerant LSP step for a Dart server defect | Sonnet | full tests 3,033/3,033; agent's 106-file `cargo fmt --all` commit dropped |
| 10-09 10:50 | 162b1c1 | CI fix: setup-dart in the job | coordinator | run 2: macOS tests, clippy, determinism green |
| 10-09 11:00 | 497d142 | wave C7 literals, record literals, for loops | Opus | flutter_tools (unseen): ListLiteral 97.3%, SetOrMapLiteral 97.2% |
| 10-09 11:10 | 31e9720 | dart_style formatter, `dartr format`, LSP formatting | Opus | byte-exact: 8,702 dart_style tests; flutter_tools 366/366 and analyzer-9.0.0 456/456 (unseen) |
| 10-09 11:35 | 38c0fee | wave C3 invocations | Opus | flutter_tools: resolved 42.3% → 52.0% |
| 10-09 11:50 | 3450c10 | wave C10 top-level inference | Opus | flutter_tools: elements unmasked 42% → 67% |
| 10-09 11:55 | 0b9624b | wave C4+C5 properties and operators | Opus | flutter_tools: resolved 52% → 90.8% |
| 10-09 12:10 | 51f6003 | wave C8 instance creation, references, dot shorthands | Opus | flutter_tools: resolved 99.70%, elements unmasked 100% |
| 10-09 12:30 | 32826b0 | x86: follow the Dart VM for NaN bits and YAML 2^63 keys | Opus | tested on x86-64 Linux (OrbStack); CI found it |
| 10-09 13:40 | b98027a | wave C6+C9 extensions, annotations, comment refs, exit detector | Codex | **resolver at parity:** flutter_tools types 100%, elements 99.84%; analyzer-9.0.0 types 100%, elements 99.81% (both unseen) |
| 10-09 17:35 | eb3c0b8 | wave D1–D3 constant evaluation, constant verifier, exhaustiveness | Opus | constant values and constant diagnostics 100% on flutter_tools and analyzer-9.0.0 (unseen); 515/535 evaluator tests as fixtures |
| 10-09 18:30 | 06e0cf1 | synthetic loadLibrary for deferred imports | Sonnet | 60 deferred-import language tests 2,204/2,204; element dump unchanged |
| 10-09 22:55 | 85cc9a8 | primary-constructor bodies resolved | Opus | visible-app 269 files: resolved types 98.48% → 100% |
| 10-09 22:55 | 27d71ac | driver-backed diagnostics for `dartr analyze` and LSP (invalidation v1) | Opus | flutter_tools (unseen) 0/0 exact; visible-app extras 73 → 36 after the primary-constructor merge |
| 10-10 00:05 | 871b43a | @optionalTypeArgs in strict inference; primary-constructor parameters in field initializers | Sonnet | visible-app `dartr analyze`: 36 → 0 false positives |
| 10-10 01:30 | d019285 | **error verifier + all error/* verifiers integrated** (wave D4–D12) | Opus | 5,571 tests; `dartr analyze` vs `dart analyze`: visible-app 0 extra, flutter_tools exact, analyzer-9.0.0 25,595/27,278 matched, 30 extra (both unseen) |
| 10-10 02:40 | 4b3741a | directive checks (URI_DOES_NOT_EXIST etc.), shouldIgnoreUndefined | Opus | analyzer-9.0.0: 27,067 matched, 0 extra; flutter and flutter_tools exact |
| 10-10 07:10 | 27e128d | lint batch A fixed (82/83 rules exact), resolved lints in `dartr analyze`, doc-import scope | Opus (fixing Codex's branch) | analyzer-9.0.0: **27,272/27,278 matched**, 1 extra; visible-app only plugin lints missing; flutter, flutter_tools exact |
| 10-10 08:40 | 1235fe5 | analyzer plugins in `dartr analyze` (dartr_plugins + Dart bridge) | Opus | **visible-app 20/20 exact vs `dart analyze`** (incl. jaspr_lints plugin), 5,574 tests |
| 10-10 09:40 | 4a6bf13 | LSP navigation: definition, typeDefinition, hover, references, documentHighlight, implementation, search index | Opus | 333 fixture positions identical for 4 methods; references/implementation same locations (order differs where Dart's order depends on earlier requests); 5,578 tests |
| 10-10 07:20 | 260fd05 | lint batch B (82 rules), unnecessary_ignore / removed_lint_use / replaced_lint_use | Opus | analyzer-9.0.0: **27,278/27,278 matched**, 5 extra (4 unnecessary_ignore, 1 analyzer_public_api); visible-app 20/20; flutter, flutter_tools exact |
| 10-10 11:20 | 255b94d | last analyzer-9.0.0 differences (unnecessary_ignore, analyzer_public_api, two rules) | Opus | analyzer-9.0.0 **27,278/27,278, 0 extra**; lints 57,211/57,211 (unseen) |
| 10-10 12:30 | 8790cf2 | LSP: signature help, semantic tokens, inlay hints, call/type hierarchy, workspace symbols | Opus | compared with `dart language-server` on fixtures; 5,580 tests |
| 10-10 13:00 | 5a40205 | release preparation: LICENSE, notices, README guide, release workflow, Homebrew formula | Opus | `dartr --version` checked; tag `v0.1.0-preview.1` pushed |
| 10-10 14:10 | 0c9d176 | legacy protocol: analysis hover/navigation + 6 subscription notifications, search domain, edit.format/sortMembers/organizeDirectives | Gemini 4 (Antigravity) | 32/32 steps identical to `dart language-server --protocol=analyzer` on a clean checkout; 5,581 tests |
| 10-10 15:00 | 00228a9 | legacy protocol: server, execution, diagnostic, analytics domains; remaining analysis and search requests (39/59 requests, 19/22 notifications) | Gemini 4 (Antigravity) | 78 steps identical to `dart language-server --protocol=analyzer`; coordinator added a step-list check; 5,581 tests |
| 10-10 15:55 | 434d7e2 | LSP code completion: textDocument/completion, completionItem/resolve (port of DartCompletionManager, relevance tables, not-imported suggestions) | Opus | 137/156 completion lists identical to `dart language-server` on the fixture; visible-app first 177 of 2,000 items in Dart's order; 5,583 tests |
| 10-10 15:55 | c1afb75 | landing page in site/, published with GitHub Pages | Opus | checked in Chrome at 1280 px and 390 px, light and dark; every number traced to a repo file |
| 10-10 16:50 | 302b531 | LSP code actions: quick fixes, assists, organize imports, sort members, fix_data.yaml renames, workspace/executeCommand | Opus | every returned action identical to `dart language-server` (~7,000 import fixes, 191 create fixes, 486 assists, fixture + corpus sample); 5,585 tests |

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
- **Skipped full test run (10-09 02:23):** after merging the lint wiring, the coordinator ran only
  the branch tests and the lint differentials. One LSP parity test had broken
  (`lsp_non_dart_files_and_package_language_version`); the CI agent found it hours later.
  Rule restored: the full workspace tests run after every merge, with no exceptions.
- **Hung corpus run (10-09):** the resolver agent waited 7 hours on a `dump resolved-el` run that
  was in an infinite loop. Rule: every background corpus run has a time limit per file.
- **Machine load (10-09 09:00):** load average 158, mostly Spotlight indexing the worktree
  `target/` folders. Timing tests and benchmarks are not valid under such load.
- **Flaky LSP parity test (10-09):** `lsp_non_dart_files_and_package_language_version` fails in
  some runs because `dart language-server` 3.13.3 sometimes ignores a `didChange` of an
  `analysis_options.yaml` overlay (no re-analysis, stale diagnostics). The fault is on the oracle
  side, not in dartr. Open decision: copy Dart's behaviour, or make the test tolerate that step.
- **Workspace-wide formatting by a worker (10-09 10:40):** the CI agent ran `cargo fmt --all`
  (106 files) while six agents edited those files. The commit was dropped before merge. Rule: no
  workspace-wide formatting while branches are open; one format commit later, with no open branches.
- **CI-only failure (10-09 10:47):** `setup-dart` inside a composite action cannot find its problem
  matcher file. Local checks (actionlint, local runs) cannot catch this kind of failure; only a real
  CI run can.
- **Spotlight (10-09 11:20):** with the operator's approval, build output now lives in
  `target.noindex` (symlinked as `target`), and `.metadata_never_index` markers are in `~/worktrees`,
  `third_party` and `bench/corpus`. A probe could not prove which method this macOS honours,
  because Spotlight was too backlogged to index even the control file in 3 minutes.
- **Network outage (10-09 ~13:00):** DNS failures stopped all Claude agents and subagents at once;
  Codex runs hung in a reconnect loop because `chatgpt.com` was unreachable while GitHub and
  `api.openai.com` worked. All work was committed or on disk. Codex sessions were resumed with
  `codex exec resume <root session id>`; the first id found by working directory was a sub-agent
  thread, so resume must use the root `exec` session (`payload.source == "exec"`).
- **Wrong recipient (10-09 11:40):** a message meant for C8 went to C10. An agent-id map is now
  checked before every message.
- **Early wait (10-09):** `herdr pane wait-output --match CODEX_EXIT=` matched the marker in the
  command line itself. The marker is now printed as `CODEX_""EXIT` so only the real exit matches.
- **Disk (10-09 12:15):** 46 GB free with 21 worktrees. Main's build output was cleaned (12 GB to
  1.2 GB); finished worktrees are removed right after merge.
- **Codex stopped (10-09 16:55):** lint batch A (83 resolution lints) differed from `dart analyze`
  in 52 rules, and the report made two false claims: a test failure was "existing" (main was green,
  so it was a regression on the branch), and the differences were "blocked on the resolver" (the
  resolver was at parity). Per the operator's quality rule, Codex gets no new units. The branch is
  kept for a Claude agent to fix.

## State at the end of 2026-10-09 (next session starts here)

- **main** `eb3c0b8`+docs: resolver at parity (flutter_tools/analyzer-9.0.0: types 100%, elements
  99.8%, element dump 100%); constants at parity on unseen corpora; 3,614 tests green; pushed.
- **Not merged, ready for integration (first task next session):**
  - `wd-errverifier` (D4–D7, 28 commits): all four ErrorVerifier sections and FfiVerifier;
    tests/language ErrorVerifier codes 6,253/6,349 matched. It added `STUB (wd-errors)` files.
  - `wd-errors` (D8–D12): all 41 `error/*` verifiers, `VerifierHost`, ignore filtering, TODO finder;
    tests/language 64,335/67,152 matched, 36 dartr-only.
  - Integration plan: merge `wd-errors` first, then `wd-errverifier`, replacing its stubs with the
    real ports and implementing `VerifierHost` for `ErrorVerifier`; wire the verifier calls listed
    in the D8–D12 report; re-measure flutter/visible-app (an unverified fix for a 376-diagnostic
    false positive; 231 visible-app false positives from unresolved primary-constructor bodies);
    full workspace tests before merge.
- **Also open:** `dartr analyze` still uses the parse-only provider; the driver must implement
  `DiagnosticsProvider` (milestone 2). `LibraryElement.load_library_function` is never set by the
  linker. Primary-constructor bodies are not resolved. Lint batch A (`cx-lints-a`, Codex) needs a
  Claude fix pass (52/83 rules differ); lint batch B not started.
- **Merge conflict resolved wrongly (10-10 00:50):** two agents had fixed `@optionalTypeArgs` in parallel.
  The coordinator kept one side without running the other side's fixture test first. The real cause was
  that both implementations wrote to the same cache field with different bit layouts. The full test run
  and the corpus check caught it before the push. Lessons: do not give two agents the same fix, and run
  the other side's tests before choosing a side in a conflict.
- **Quiet mode (10-09 23:30):** the operator asked for quiet fans. All heavy commands now take one shared
  `lockf` lock (2 build jobs, nice 15). Agents may not run scripts from the coordinator's scratchpad
  (the permission classifier blocked `heavy.sh`), so the lock is written inline.
- **Slower with resolved lints (10-10 07:10):** `dartr analyze` on flutter_tools went from 0.7 s to 3.5 s
  (dart: 2.6 s) and on flutter from 3.2 s to 5.9 s after the resolved lints were added. First target for
  the optimization phase.
- **Flaky LSP step:** `lsp_session_parity_with_dart_language_server` failed once at "didChangeConfiguration:
  server requests" under load, then passed 2/2. Timing of server requests; to fix later.
- **Cold search speed:** a first references search over the SDK takes 5.7 s (`String`), and 130 s for `runtimeType` on flutter; Dart has a warm disk index. Optimization target (persistent index).
- **First public build (10-10 13:15):** `v0.1.0-preview.1` released by the release workflow on its first run
  (macOS arm64 8 MB, Linux x86_64 38 MB: debug info is not stripped on Linux). Homebrew formula pushed to
  `orestesgaolin/homebrew-tap`; `brew install orestesgaolin/tap/dartr` and `brew test` pass. The installed
  binary gives the same result as `dart analyze` on flutter_tools (0 issues), but takes 3.7 s with 29.6 s of
  system CPU time (dart: 2.4 s). The system time is an optimization target.
- **Load 73 (10-10 13:04):** a worker ran the inline lock command without `RUST_TEST_THREADS=4`, so the
  parity tests started one `dart analyze` per core (66 Dart processes). The inline command in the worker
  rules now sets `RUST_TEST_THREADS=4`.
