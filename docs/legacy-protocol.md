# Legacy analysis server protocol

`dartr language-server --protocol=analyzer` and `dartr analysis-server` use
Dart's legacy protocol: one JSON object per UTF-8 line on stdin and stdout.
`language-server` defaults to LSP. `analysis-server --lsp` selects LSP.

The protocol types are generated from the pinned Dart 3.13.3 specification
(version 1.40.1) and the analyzer-plugin common types. Regenerate or verify them:

```sh
python3 tools/codegen/gen_legacy_protocol.py
python3 tools/codegen/gen_legacy_protocol.py --check
```

The implementation lives in `dartr_legacy`; it reuses the CLI diagnostic
pipeline and the LSP folding computer without changing the LSP server.
It emits `server.connected` with the protocol version and process ID on start.
`server.getVersion` returns the protocol version, as the Dart server does.

## Implemented surface

Counts include deprecated and experimental methods in the pinned specification.
Generated types cover every domain; runtime support is the subset below.

| Domain | Requests implemented / total | Notifications implemented / total |
| --- | ---: | ---: |
| server | 3 / 7 | 3 / 5 |
| analysis | 6 / 14 | 3 / 12 |
| completion | 0 / 3 | 0 / 1 |
| search | 0 / 6 | 0 / 1 |
| edit | 0 / 14 | 0 / 0 |
| execution | 0 / 5 | 0 / 1 |
| diagnostic | 0 / 2 | 0 / 0 |
| analytics | 0 / 4 | 0 / 0 |
| flutter | 0 / 3 | 0 / 1 |
| lsp | 0 / 1 | 0 / 1 |
| Total | 9 / 59 | 6 / 22 |

Requests: `server.getVersion`, `server.shutdown`, `server.setSubscriptions`,
`analysis.setAnalysisRoots`, `analysis.updateContent`, `analysis.setPriorityFiles`,
`analysis.setSubscriptions`, `analysis.getErrors`, `analysis.reanalyze`.

Notifications: `server.connected`, `server.status`, `server.error`,
`analysis.errors`, `analysis.flushResults`, `analysis.folding`.
`analysis.flushResults` is a notification, not a request.

Other requests return `UNKNOWN_REQUEST` with message `Unknown request`.
This includes `edit.bulkFixes`: `dart fix` is not functional with this subset.
Known analysis subscriptions are accepted, but only `FOLDING` emits feature
results. `STATUS` emits analysis start and end. `LOG` is accepted without
statistics notifications.

## Diagnostics and overlays

Analysis emits scanner/parser diagnostics, implemented AST-only lints,
ignore-comment diagnostics, and options/pubspec/manifest diagnostics. It applies
configured `errors:` processors and exclusions through the shared CLI pipeline.
Locations and SourceEdit offsets use UTF-16 code units and one-based line/column
numbers. Corrections, URLs, and context messages follow the shared converters;
absent optional values are omitted. `hasFix` is false for Dart diagnostics, matching the pinned
`newAnalysisError_fromEngine` implementation, and true for non-Dart diagnostics,
matching `AnalyzerConverter`.

Overlay edits are applied in their supplied order. `change` requires an existing
overlay and valid ranges. `remove` restores disk content. Overlay-only Dart files
inside a context are analyzed. Priority files precede other files during analysis.
Files removed from analysis send `analysis.flushResults`; shutdown flushes the
published files before its response.

`packageRoots` is decoded and accepted, but does not override package resolution.
The pinned `AnalysisSetAnalysisRootsHandler` also decodes this field without
passing it to the server. `--packages` and `.dart_tool/package_config.json`
remain the supported package configuration inputs.

## Validation

```sh
cargo test -p dartr_legacy
cargo test -p dartr --test legacy_parity --test legacy_folding --test legacy_transport
```

The differential tests run the real Dart 3.13.3 server. They compare full
syntactic and implemented lint diagnostic objects, lifecycle responses,
UTF-16 overlay edits, invalid requests, reanalysis, exclusions, result flushing,
and folding for disk/overlay/outside-root subscriptions. They normalize IDs,
process IDs, and nondeterministic notification timing; they do not establish
resolver or full IntelliJ compatibility. A missing Dart installation skips the
differential tests. `DART_BIN` selects a standalone SDK executable. The test
helper also detects the SDK binary behind a Flutter wrapper.

Record raw JSONL evidence:

```sh
DART_BIN=/path/to/dart-sdk/bin/dart python3 tools/legacy_parity.py --server dart --output /tmp/dart-legacy.jsonl
python3 tools/legacy_parity.py --server target/debug/dartr --output /tmp/dartr-legacy.jsonl
```

## Remaining work

Resolution and static types are needed for semantic diagnostics, highlights,
occurrences, navigation, completion, fixes, and most editor services. The existing
outline computer uses syntactic substitutes and omits required legacy flags and
aliased types, so legacy outlines are deferred. No filesystem watcher is installed;
on-disk changes become visible after a request that triggers analysis, such as
`analysis.reanalyze`. Analysis currently rebuilds contexts and analyzes the roots
synchronously instead of using the incremental driver scheduler. Notification
batching and response ordering can therefore differ from Dart, while analysis
start precedes completion and clients receive the final diagnostic state.
