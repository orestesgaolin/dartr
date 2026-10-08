# dartr: plan

dartr is a port of the Dart analyzer (`pkg/analyzer`, `pkg/_fe_analyzer_shared`,
`pkg/linter`, and the parts of `pkg/analysis_server` that clients use) to Rust.

## Goals

1. Same output as the Dart analyzer of the pinned SDK version (`3.13.3`) for the
   same input: same diagnostics (code, severity, offset, length, message).
2. Drop-in use by every analyzer client:
   - command line: `dartr analyze` with the options, output formats
     (`default`, `machine`, `json`) and exit codes of `dart analyze`;
   - editors: `dartr language-server` (LSP, with the Dart LSP extensions that
     Dart-Code and other clients use) and the legacy analysis server protocol
     (`--protocol=analyzer`, used by IntelliJ);
   - VS Code: Dart-Code runs `dart.analyzerPath` with the Dart VM (`dart <path> --lsp ...`),
     so it cannot start a native binary. dartr ships `tools/shim/dartr_shim.dart`: it starts
     `dartr language-server` with the same arguments and forwards stdin/stdout/stderr and the
     exit code. `dart.analyzerPath` points to the shim (or its AOT/JIT snapshot).
   - `flutter analyze` uses LSP (`$/progress`, `publishDiagnostics`); IntelliJ uses the legacy
     protocol through `<sdk>/bin/dart language-server --protocol=analyzer` (no custom binary
     setting: needs an SDK wrapper). Details: `docs/research/editor-integration.md`.
3. Faster than the Dart analyzer. Benchmarks before (baseline, `bench/`) and after.

## Reference version

- `third_party/dart-sdk` is a sparse checkout of the SDK at tag `3.13.3`
  (`tools/fetch_sdk.sh`). It must match `dart --version`.
- Ported code keeps the structure and names of the Dart code where possible.
  Every ported file starts with a comment that names its Dart source file.
  This makes it possible to follow upstream changes file by file.

## Correctness: differential tests

`tools/oracle` is a Dart program that uses the pinned `package:analyzer` and
writes JSON Lines. `dartr dump <mode>` writes the same format.

| mode       | content                                                            |
|------------|--------------------------------------------------------------------|
| `tokens`   | scanner only (no parser): token stream (kind, offset, length, lexeme, synthetic, comments), scanner diagnostics |
| `ast`      | unresolved AST as ordered child entities (`childEntities`), parse diagnostics |
| `resolved` | diagnostics of the resolved unit, static type of each expression  |

The test corpus:

- `third_party/dart-sdk/tests/language` (many files with intentional errors: recovery),
- `third_party/dart-sdk/sdk/lib`, `pkg/analyzer/lib`,
- Flutter packages (`$FLUTTER_ROOT/packages/*`) and the pub cache.

`difftest <mode> <path-or-dir>...` (crate `dartr_difftest`; build with
`cargo build --release`, then `target/release/difftest tokens <dirs>`) runs
both in parallel batches and reports the first difference of each differing
file and a parity percentage (`--jobs`, `--write-failures <dir>`). The oracle
is compiled to `target/oracle/oracle` on first use.

## Architecture

| crate               | Dart source                                                    |
|---------------------|----------------------------------------------------------------|
| `dartr_syntax`      | `_fe_analyzer_shared/lib/src/scanner` (tokens, scanner)        |
| `dartr_diagnostics` | generated from `messages.yaml` of `_fe_analyzer_shared`, `analyzer`, `linter` |
| `dartr_ast`         | `analyzer/lib/src/dart/ast` (AST nodes, visitors)              |
| `dartr_parser`      | `_fe_analyzer_shared/lib/src/parser`, `analyzer/lib/src/fasta/ast_builder.dart` |
| `dartr_project`     | package_config, pubspec, `analysis_options.yaml`, context discovery, SDK location |
| `dartr_semantics`   | elements, types, type system, inheritance, resolution, inference, flow analysis, constants, error verifiers |
| `dartr_lints`       | `pkg/linter` rules                                             |
| `dartr_driver`      | analysis driver: file state, caches, library cycles, parallel analysis, incremental updates |
| `dartr_server`      | LSP server and legacy analysis server protocol                 |
| `dartr`             | binary: `analyze`, `language-server`, `dump`                   |

Data model rules:

- Source text, tokens and AST nodes of a file live in arenas owned by that file.
  References are typed indices, not `Rc` graphs.
- Elements and types of a library are built once per library cycle and are
  immutable after that. Libraries of one cycle are resolved in parallel with
  other independent cycles (rayon).
- No global mutable state. A cache key is the content hash of the inputs.

## Phases

| # | phase | done when |
|---|-------|-----------|
| 0 | infrastructure: oracle, baseline benchmarks, workspace | yes |
| 1 | scanner | 100% token parity on corpus |
| 2 | diagnostics codegen | all codes of the 3 `messages.yaml` files, message formatting |
| 3 | AST + parser + AST builder | 100% AST + parse-diagnostic parity on corpus |
| 4 | project model | same contexts, options and package resolution as `dart analyze` |
| 5 | element model + type model + type system | element dumps match |
| 6 | resolution, type inference, flow analysis | `resolved` types parity |
| 7 | error verifiers, constants, hints, warnings, ignore comments | `resolved` diagnostics parity |
| 8 | lints | all lint rules, `dart analyze` parity with lints enabled |
| 9 | `dartr analyze` CLI | output and exit code parity |
| 10 | LSP server, then legacy subset (`dart analyze`/`dart fix`), then full legacy (IntelliJ) | Dart-Code works through the shim; `flutter analyze` works; LSP request parity |
| 11 | performance | benchmark report vs baseline |
