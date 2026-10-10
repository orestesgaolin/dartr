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
| server | 5 / 7 | 5 / 5 |
| analysis | 14 / 14 | 12 / 12 |
| completion | 0 / 3 | 0 / 1 |
| search | 6 / 6 | 1 / 1 |
| edit | 3 / 14 | 0 / 0 |
| execution | 5 / 5 | 1 / 1 |
| diagnostic | 2 / 2 | 0 / 0 |
| analytics | 4 / 4 | 0 / 0 |
| flutter | 0 / 3 | 0 / 1 |
| lsp | 0 / 1 | 0 / 1 |
| Total | 39 / 59 | 19 / 22 |

Requests: `server.getVersion`, `server.shutdown`, `server.setSubscriptions`,
`server.cancelRequest`, `server.setClientCapabilities`,
`analysis.getErrors`, `analysis.getHover`, `analysis.getImportedElements`,
`analysis.getLibraryDependencies`, `analysis.getNavigation`,
`analysis.getReachableSources`, `analysis.getSignature`, `analysis.reanalyze`,
`analysis.setAnalysisRoots`, `analysis.setGeneralSubscriptions`,
`analysis.setPriorityFiles`, `analysis.setSubscriptions`,
`analysis.updateContent`, `analysis.updateOptions`,
`search.findElementReferences`, `search.findMemberDeclarations`,
`search.findMemberReferences`, `search.findTopLevelDeclarations`,
`search.getElementDeclarations`, `search.getTypeHierarchy`,
`edit.format`, `edit.sortMembers`, `edit.organizeDirectives`,
`execution.createContext`, `execution.deleteContext`,
`execution.getSuggestions`, `execution.mapUri`, `execution.setSubscriptions`,
`diagnostic.getDiagnostics`, `diagnostic.getServerPort`,
`analytics.isEnabled`, `analytics.enable`, `analytics.sendEvent`,
`analytics.sendTiming`.

Notifications: `server.connected`, `server.error`, `server.pluginError`,
`server.log`, `server.status`, `analysis.analyzedFiles`,
`analysis.closingLabels`, `analysis.errors`, `analysis.flushResults`,
`analysis.folding`, `analysis.highlights`, `analysis.implemented`,
`analysis.invalidate`, `analysis.navigation`, `analysis.occurrences`,
`analysis.outline`, `analysis.overrides`, `search.results`,
`execution.launchData`.
`analysis.flushResults` is a notification, not a request.

Server-initiated requests (`server.openUrlRequest`, `server.showMessageRequest`)
return `UNKNOWN_REQUEST` if sent by a client as a request; client responses to
those server-initiated requests (`{"id": "...", "result": ...}`) are accepted
without emitting `INVALID_REQUEST`. Other unimplemented requests return
`UNKNOWN_REQUEST` with message `Unknown request`. This includes `edit.bulkFixes`:
`dart fix` is not functional with this subset. `analysis.getReachableSources`
and `analysis.getLibraryDependencies` return `UNSUPPORTED_FEATURE` matching the
pinned Dart 3.13.3 `UnsupportedRequestHandler`. Known analysis subscriptions are
accepted; `FOLDING`, `NAVIGATION`, `HIGHLIGHTS`, `OCCURRENCES`, `OUTLINE`,
`IMPLEMENTED`, `OVERRIDES`, and `CLOSING_LABELS` emit feature notifications.
`ANALYZED_FILES` in `analysis.setGeneralSubscriptions` emits
`analysis.analyzedFiles` notifications. `STATUS` emits analysis start and end,
and `LOG` emits `server.log` request/response/notification statistics entries.

## Diagnostics and overlays

Analysis emits scanner/parser/resolver diagnostics, implemented lints,
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
diagnostic objects, lifecycle responses, UTF-16 overlay edits, invalid requests,
reanalysis, exclusions, result flushing, folding, hover, navigation, highlights,
occurrences, outline, implemented, overrides, closing labels, analyzed files,
imported elements, signatures, search requests (`search.results`,
`search.getElementDeclarations`, and type hierarchy), edit requests (`edit.format`,
`edit.sortMembers`, `edit.organizeDirectives`), execution context and URI mapping,
diagnostic context inspection and server port allocation, and analytics handlers.
They normalize IDs, process IDs, ports, timestamps, and nondeterministic
notification timing. A missing Dart installation skips the differential tests.
`DART_BIN` selects a standalone SDK executable. The test helper also detects the
SDK binary behind a Flutter wrapper.

Record raw JSONL evidence:

```sh
DART_BIN=/path/to/dart-sdk/bin/dart python3 tools/legacy_parity.py --server dart --output /tmp/dart-legacy.jsonl
python3 tools/legacy_parity.py --server target/debug/dartr --output /tmp/dartr-legacy.jsonl
```

## Remaining work

Completion, quick fixes, assists, refactorings, bulk fixes, Flutter outline
requests, and LSP-over-legacy requests are not implemented in the legacy
protocol yet. No filesystem watcher is installed; on-disk changes become visible
after a request that triggers analysis, such as `analysis.reanalyze`. Analysis
currently analyzes the roots synchronously on each analysis pass instead of using
an asynchronous background scheduler. Notification batching and response ordering
can therefore differ from Dart, while analysis start precedes completion and
clients receive the final state.
