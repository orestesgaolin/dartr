# Editor and CLI integration research (Dart SDK 3.13.3)

Scope: how editors and CLIs start and talk to the Dart analysis server, so `dartr` can replace it.
SDK paths are relative to `third_party/dart-sdk/pkg/` (pinned 3.13.3). External sources were read from
the default branch on 2026-10-08 (`master`; IntelliJ: branch `261`, because `master` no longer holds the Dart plugin).

## 1. Dart-Code (VS Code)

Source: https://github.com/Dart-Code/Dart-Code/blob/master/src/extension/analysis/analyzer.ts

### Settings (package.json lines 1746-1800)
- `dart.analyzerPath` (string|null, scope `machine-overridable`): "path to a custom Dart Analysis Server ... intended for use by Dart Analysis Server developers".
- `dart.analyzerAdditionalArgs` (string[]): extra args for the server (appended last).
- `dart.analyzerVmAdditionalArgs` (string[]): extra args for the Dart VM (before the script).
- Others: `dart.analyzerDiagnosticsPort` (`--port=`), `dart.analyzerVmServicePort`, `dart.analyzerSshHost`, `dart.analyzerInstrumentationLogFile`, `dart.analyzerSessionLogFile`, `dart.analyzerLogFile`.
- `dart.useLsp` does NOT exist any more (no match in `package.json` or `src/`). Dart-Code uses LSP only.

### Command line (analyzer.ts `spawnServer` lines 634-641, `getAnalyzerArgs` lines 733-792)
Executable is always `<dart.sdkPath>/bin/dart` (`vmPath = path.join(sdks.dart, dartVMPath)`, line 636).
The final argv is:

```
dart [--enable-vm-service=N -DSILENT_OBSERVATORY=true -DSILENT_VM_SERVICE=true --disable-service-auth-codes --no-dds --no-serve-devtools]
     [analyzerVmAdditionalArgs...]
     <analyzerPath | "language-server">
     <"--protocol=lsp" if default | "--lsp" if analyzerPath set>
     [--port=<diagnosticsPort>]
     --client-id=VS-Code|VS-Code-Remote  --client-version=<ext version>
     [--instrumentation-log-file=<f>] [--session-log=<f>]
     [analyzerAdditionalArgs...]
```

- Default (no `analyzerPath`): `dart language-server --protocol=lsp --client-id=VS-Code --client-version=X` (lines 734, 766-768).
- (a) `analyzerPath` = `.dart` or `.snapshot` file: `dart <file> --lsp ...`. This is the supported case.
- (b) directory: no special handling. Only `fs.existsSync(analyzerPath)` is checked (line 738); the path is passed to the VM as the script, so `dart <dir>` fails.
- (c) native executable: same. It is passed as a script argument to `dart`, so it fails. There is no direct exec path.
- Consequence: dartr can not be plugged in through `dart.analyzerPath` unless it is a Dart script/snapshot. Use a fake SDK (`dart.sdkPath` whose `bin/dart` is a dartr shim that accepts `language-server ...`) or put a `dart` shim first in PATH/SDK. SDK-layout checks done by Dart-Code on `dart.sdkPath` were not verified here.
- `--instrumentation-log-file` is an alias of `--protocol-traffic-log` in the server (`analysis_server/lib/src/server/driver.dart:99-102`). dartr must accept it, or the server exits on an unknown option.
- Transport: stdio, standard LSP `Content-Length` framing through `vscode-languageclient` (lines 463-466).

### Protocol: LSP only
No legacy-protocol request is used (searched all of `src/extension` and `src/shared`; there is no `analysis.*`/`edit.*`). The only thing named "legacy" is `LegacyRefactors` (`src/extension/analysis/features/legacy_refactors.ts`) which rewrites the LSP commands `refactor.perform`/`refactor.validate`.

### Custom LSP messages used (`src/shared/analysis/lsp/custom_protocol.ts`)
- Requests: `dart/textDocument/super`, `dart/textDocument/imports`, `dart/textDocument/augmented`, `dart/textDocument/augmentation`, `dart/diagnosticServer`, `dart/connectToDtd` (`{uri, registerExperimentalHandlers}`), `dart/reanalyze`, `dart/updateDiagnosticInformation` (`shared/vscode/analyzer_update_diagnostic_information.ts:125`), `command/resolve` and `workspace/executeCommand` (interactive forms, `analysis/form.ts:600-676`).
- Notifications server->client: `dart/openUri`, `dart/textDocument/publishOutline`, `dart/textDocument/publishFlutterOutline`, `dart/textDocument/publishClosingLabels`.
- `$/analyzerStatus` is NOT used by Dart-Code; status comes from `$/progress`/workDoneProgress (server still sends `$/analyzerStatus` only when the client lacks `window.workDoneProgress`).
- Client commands/capabilities: `experimental.commands = ["dart.goToLocation"]`, `experimental.supportsWindowShowMessageRequest`, `experimental.snippetTextEdit`, `experimental.closingLabels` (new, replaces the init option), `experimental.dartCodeAction.commandParameterSupport`, `experimental.interactiveResolve`. It also reads `capabilities.experimental.textDocument.*` and `executeCommandProvider.commands` for `dart.edit.sortMembers`, `organizeImports`, `fixAll`, `fixAllInWorkspace(.preview)`, `sendWorkspaceEdit` (`shared/vscode/lsp_common_capabilities.ts`).
- `workspace/configuration` section `dart` is answered by the client (`enableSnippets`, `maxCompletionItems`, `documentation`, ...).
- `initializationOptions` (analyzer.ts lines 410-433): `allowOpenUri`, `appHost`, `closingLabels`, `completionBudgetMilliseconds`, `flutterOutline`, `hostKind`, `onlyAnalyzeProjectsWithOpenFiles`, `outline: true`, `previewSurveys`, `remoteName`, `suggestFromUnimportedLibraries`, `useInEditorDartFixPrompt`. Unknown keys must be ignored.
- Restart policy: 3 crashes inside 3 minutes = the extension stops restarting (`DartErrorHandler`, lines 780+).

## 2. IntelliJ Dart plugin

The plugin is no longer on `intellij-plugins` `master`; read from branch 261:
https://github.com/JetBrains/intellij-plugins/tree/261/Dart
(`src/com/jetbrains/lang/dart/analyzer/DartAnalysisServerService.java`, `thirdPartySrc/analysisServer/com/google/dart/server/internal/remote/StdioServerSocket.java`).

- Protocol: legacy JSON over stdio, one JSON object per line (`RemoteAnalysisServerImpl`, `ByteRequestSink`). Not LSP.
- Launch (`DartAnalysisServerService.startServer`, lines 2224-2305; `StdioServerSocket.computeProcessArguments`, lines 172-196):
  `<sdk>/bin/dart [Registry dart.server.vm.options] language-server --client-id=<id> --client-version=<v> --protocol=analyzer [Registry dart.server.additional.arguments]` for SDK >= 2.16.0 (line 183, `MIN_DART_LANG_SERVER_SDK_VERSION`).
  For older SDKs: `dart [vm opts] <sdk>/bin/snapshots/analysis_server.dart.snapshot --client-id.. --useAnalysisHighlight2`.
- Custom server: no UI setting. Options are the SDK path (Dart SDK setting), the JVM property `-Ddart.server.path` (only for the legacy snapshot mode, line 2236), and the IDE registry keys `dart.server.vm.options` / `dart.server.additional.arguments` (`plugin.xml` lines 272-273). Same shim approach as Dart-Code: point the Dart SDK at a fake SDK with a `bin/dart` shim.
- Startup calls: `server.setSubscriptions [STATUS]` (+LOG), `analysis.updateOptions`, `server.setClientCapabilities(["openUrlRequest","showMessageRequest"], supportsUris, supportsWorkspaceApplyEdits)` (line 2358; `supportsUris` from SDK >= 3.4, apply-edit from >= 3.8), `analysis.setAnalysisRoots`, `analysis.setPriorityFiles`, `analysis.setSubscriptions`, `analysis.updateContent`.
- Requests in `RemoteAnalysisServerImpl` (generated names): analysis.{getErrors,getHover,getImportedElements,getLibraryDependencies,getNavigation,reanalyze,setAnalysisRoots,setGeneralSubscriptions,setPriorityFiles,setSubscriptions,updateContent,updateOptions}, completion.{getSuggestions,getSuggestionDetails,setSubscriptions} (+ `2` variants), edit.{format,getAssists,getAvailableRefactorings,getFixes,getRefactoring,importElements,organizeDirectives,sortMembers,postfixCompletion,statementCompletion,isPostfixCompletionApplicable,listPostfixCompletionTemplates}, execution.{createContext,deleteContext,getSuggestions,mapUri,setSubscriptions}, search.{findElementReferences,findMemberDeclarations,findMemberReferences,findTopLevelDeclarations,getTypeHierarchy}, server.{getVersion,setSubscriptions,shutdown,setClientCapabilities}, analytics.*, diagnostic.getServerPort, `lsp.handle`, `dart.connectToDtd`, flutter.*.
  Some of these (`completion.getSuggestions` v1, `getSuggestionDetails` v1) are gone from the 3.13.3 spec; the plugin version-guards them. Not verified per call.
- Notifications consumed: analysis.{analyzedFiles,closingLabels,errors,flushResults,highlights,implemented,navigation,occurrences,outline,overrides}, completion.{availableSuggestions,existingImports,results}, execution.launchData, lsp.{handle,notification}, search.results, server.{connected,error,status,openUrlRequest,showMessageRequest}.
- Implication: IntelliJ needs a much larger legacy subset than `dart analyze`; it is a second milestone.

## 3. Pinned SDK 3.13.3: server surface

### LSP
- Handler classes: 61 distinct message handlers (`grep "Method get handlesMessage =>"` in `analysis_server/lib/src/lsp/handlers`): 45 standard LSP + 16 custom.
- Standard (45): `initialize`, `initialized`, `shutdown`, `exit`, `$/cancelRequest`, `textDocument/{didOpen,didChange,didClose,codeAction,codeLens,completion,definition,documentColor,colorPresentation,documentHighlight,documentLink,documentSymbol,foldingRange,formatting,rangeFormatting,onTypeFormatting,hover,implementation,inlayHint,inlineValue,prepareCallHierarchy,prepareRename,rename,prepareTypeHierarchy,references,selectionRange,semanticTokens/full,semanticTokens/range,signatureHelp,typeDefinition}`, `completionItem/resolve`, `callHierarchy/{incomingCalls,outgoingCalls}`, `typeHierarchy/{subtypes,supertypes}`, `workspace/{didChangeConfiguration,didChangeWorkspaceFolders,executeCommand,symbol,willRenameFiles}`.
  Authoritative status table: `analysis_server/tool/lsp_spec/README.md` (section "Method Status", ~line 60). Not supported: `didSave`, `declaration`, `semanticTokens/full/delta`, `codeAction/resolve`, `codeLens/resolve`, `didChangeWatchedFiles` (server watches files itself), `textDocument/diagnostic` (push diagnostics only).
- Server->client used: `client/registerCapability`, `window/{logMessage,showMessage,workDoneProgress/create}`, `$/progress`, `workspace/{applyEdit,configuration}`, `textDocument/publishDiagnostics`.
- LSP types are generated from the LSP meta model (`tool/lsp_spec/generate_all.dart`, `meta_model_reader.dart`).
- Custom (`lib/src/lsp/constants.dart:144-205`; docs in README lines ~168-330): `dart/textDocument/{super,imports,augmented,augmentation,summary,editableArguments,editArgument,getFlutterWidgetPreviews,publishClosingLabels,publishOutline,publishFlutterOutline}`, `dart/workspace/{getFlutterWidgetPreviews,migrate}`, `dart/{diagnosticServer,reanalyze,connectToDtd,openUri,updateDiagnosticInformation,textDocumentContent,textDocumentContentDidChange}`, `command/resolve`, `experimental/echo`, `$/analyzerStatus` (deprecated).
  Handlers in `handlers/custom/`: 16 (augmentation, augmented, command_resolve, connect_to_dtd, diagnostic_server, experimental_echo, get_widget_previews x2, imports, migrate, reanalyze, summary, super, update_diagnostic_information, editable_arguments x2 = 16).
- Commands (`constants.dart:108-141`): `dart.edit.codeAction.apply`, `dart.edit.sortMembers`, `dart.edit.organizeImports`, `dart.edit.fixAll`, `dart.edit.fixAllInWorkspace`, `dart.edit.fixAllInWorkspace.preview`, `dart.edit.sendWorkspaceEdit`, `dart.logAction`, `refactor.perform`, `refactor.validate`, plus one command per new refactoring. Client command `dart.goToLocation`.
- Initialization options and `dart.*` workspace settings: README lines 25-58. `dart language-server` defaults to LSP (`defaultToLsp: true`, `dartdev/lib/src/commands/language_server.dart:36-47`); `--protocol=analyzer` selects legacy.

### Legacy protocol
- Spec source: `analysis_server/tool/spec/spec_input.html` (version 1.40.1); rendered `analysis_server/doc/api.html`. Handlers in `analysis_server/lib/src/handler/legacy/` (57 files, ~53 request handlers).
- Domains / requests / notifications (counted from `spec_input.html`; total 59 requests, 22 notifications):
  server 7/5, analysis 14/12, completion 3/1, search 6/1, edit 14/0, execution 5/1, diagnostic 2/0, analytics 4/0 (experimental), flutter 3/1, lsp 1/1 (experimental). Deprecated requests are included (analysis.getReachableSources, analysis.updateOptions, completion.registerLibraryPaths, execution.setSubscriptions, analytics.isEnabled/enable).
- Framing: one JSON object per line on stdin/stdout, no `Content-Length`.
- Server flags (`lib/src/server/driver.dart:58-159`, parser lines ~797-945): `--client-id`, `--client-version`, `--disable-server-exception-handling`, `--disable-server-feature-completion`, `--disable-server-feature-search`, `--disable-status-notification-debouncing`, `--disable-silent-analysis-exceptions`, `--enable-experiment`, `--help`, `--[no-]analytics`, `--suppress-analytics`, `--protocol-traffic-log` (alias `--instrumentation-log-file`), `--session-log`, `--performance-log`, `--internal-print-to-console`, `--analysis-driver-log` (alias `--new-analysis-driver-log`), `--diagnostic-port` (alias `--port`), `--dart-sdk` (alias `--sdk`), `--cache`, `--packages`, `--report-protocol-version`, `--protocol=lsp|analyzer`, `--lsp` (hidden), `--train-using`, `--disable-file-byte-store`, `--[no-]with-fine-dependencies`, `--[no-]plugins`.

## 4. `dart analyze` (`dartdev/lib/src/commands/analyze.dart`, `dartdev/lib/src/analysis_server.dart`)

### Options (analyze.dart lines 43-113)
`[<directory|file>...]` (default: cwd; missing path = usage error, line 140), `--fatal-infos` (non-negatable), `--[no-]fatal-warnings` (default true), hidden: `--cache=<path>`, `--memory` (JSON only), `--format=default|json|machine`, `--packages=<path>`, `--sdk-path=<path>` (must contain `bin/snapshots/analysis_server[_aot].dart.snapshot`), `--[no-]use-aot-snapshot`, `--[no-]plugins`, plus `--enable-experiment` flags (unknown experiment = usage error, lines 178-193).

### Server launch and requests (analysis_server.dart)
- Process (lines 205-230): `dartaotruntime analysis_server_aot.dart.snapshot [--suppress-analytics] --client-id=dart-analyze --disable-server-feature-completion --disable-server-feature-search --disable-status-notification-debouncing --disable-silent-analysis-exceptions --sdk <sdk> [--cache=..] [--packages=..] [--enable-experiment=a,b] [--no-plugins]`. No `--protocol` flag, so the legacy protocol is the default for the snapshot. (`dart` + JIT snapshot if `--no-use-aot-snapshot`.)
- Requests sent: `server.setSubscriptions {subscriptions:["STATUS"]}` (line 161), `analysis.setAnalysisRoots {included:[abs canonical paths, no trailing slash], excluded:[]}` (196), `server.shutdown` (263). Used by `dart fix`: `edit.bulkFixes {included, inTestMode, updatePubspec, codes?}` (244) and `analysis.updateContent` with `AddContentOverlay` (279). `server.getVersion` exists in the wrapper but analyze does not call it.
- Notifications consumed: `server.status` (`analysis.isAnalyzing` true -> false marks end; line 182-193), `analysis.errors {file, errors[]}` (last result per file wins), `server.error`, `server.pluginError` (set `serverErrorReceived`, print to stderr).
- Needed `AnalysisError` JSON fields (lines 374-460): `severity` (INFO|WARNING|ERROR), `type`, `code`, `message`, `correction?`, `url?`, `location{file,offset,length,startLine,startColumn,endLine,endColumn}`, `contextMessages[]` of `{message, location}`.
- Completion detection depends on the exact `server.status` event order: analysis must emit `isAnalyzing:true` and later `false`. dartdev ignores `TODO` + `INFO` diagnostics (line 263).

### Output formats
- `default` (lines 372-432): blank line; per diagnostic `"<severity lowercase, left-padded to 7> • <path>:<line>:<col> • <message> <correction> • <code>"` where path is relative to the single target dir (or its parent if a file) when shorter; verbose uses the doc URL instead of the code; text wrapped at the dartdev line length with continuation indent of 10; context messages as `<indent> - <message sans trailing '.'> at <path>:<line>:<col>.`; blank line; final `N issue(s) found.` or `No issues found!`. Errors in `pubspec.yaml`/`analysis_options.yaml` are printed first with a warning paragraph (lines 308-320). Sorted (`AnalysisError.compareTo`). Spec says the format "is not specified and can change".
- `machine` (lines 507-540): one line per diagnostic: `SEVERITY|TYPE|ERROR_CODE|FILE_PATH|LINE|COLUMN|LENGTH|ERROR_MESSAGE`. CODE is uppercased. Escaping in path and message: `\` -> `\\`, `|` -> `\|`, LF -> `\n`, CR -> `\r`. No progress output, no summary. Priority-file errors are not printed (TODO in code).
- `json` (lines 434-505): one line, `{"version":1,"diagnostics":[...],"memory":<KB, only with --memory>}`. Each diagnostic: `code`, `severity`, `type`, `location{file,range{start{offset,line,column},end{offset,line,column}}}`, `problemMessage`, optional `correctionMessage`, `contextMessages[{location,message}]`, `documentation` (url). Empty result prints `{"version":1,"diagnostics":[]}`.

### Exit codes (lines 330-355, 550-570)
0 = no issues (or only non-fatal ones), 1 = infos found and `--fatal-infos`, 2 = warnings found and `--fatal-warnings` (default on), 3 = any ERROR, 4 = server crash or `server.error`/`server.pluginError` received (even with zero diagnostics). Order: errors (3) > fatal warnings (2) > fatal infos (1). Usage errors exit 64 (verified with `dart analyze --bogus`, `nope`, `--format=xml`, `--enable-experiment=foo`, `--packages=nofile`, `--sdk-path=/x`: message on stderr, then a blank line and the usage text; `--packages` is checked after the `Analyzing ...` line is printed). If the server exits before analysis finishes, the process exit code is the server's.

## 5. Other clients
- `dart fix` (`dartdev/lib/src/commands/fix.dart`): same wrapper, `--client-id=dart-fix`, `--no-plugins`, `start(setAnalysisRoots: false)`; loop of `edit.bulkFixes` (max passes, line 29) with `analysis.updateContent` overlays between passes. Flags `--dry-run/-n`, `--apply`, `--code=a,b`, hidden `--compare-to-golden`.
- `dart language-server` (`language_server.dart`): thin launcher of the AOT snapshot with `--protocol=lsp` by default; passes all other flags through; exits 255 if the snapshot is missing.
- `flutter analyze` (flutter_tools `lib/src/dart/analysis.dart`, https://github.com/flutter/flutter/blob/master/packages/flutter_tools/lib/src/dart/analysis.dart): uses LSP, not legacy. Command (lines 73-83): `<sdk>/bin/dart language-server --dart-sdk <sdk> --disable-server-feature-completion --disable-server-feature-search [--no-with-fine-dependencies] [--no-plugins] [--suppress-analytics] [--protocol-traffic-log=..]`. It sends `initialize` (rootUri, workspaceFolders, `capabilities.window.workDoneProgress=true`) then `initialized`; reads `$/progress`, `textDocument/publishDiagnostics`, `window/showMessage`; answers `window/workDoneProgress/create`; optional `dart/connectToDtd`. Stable branch tracks analysis by `$/progress` begin/end. Flutter master (commit a851a18c, 2026-10-02) instead calls a custom request `dart/workspace/analysis/complete`, which is NOT present in the pinned 3.13.3 sources (grep found nothing). Flutter master expects a newer SDK than 3.13.3; for dartr, support it as a cheap addition (return null after analysis is idle) to stay compatible with newer Flutter.
  Exit code (`analyze_once.dart`): 1 if any error, or warning with `--fatal-warnings`, or info with `--fatal-infos` (flutter defaults: `--fatal-infos` true, `--fatal-warnings` true); else 0.
- `analyzer_cli` package exists in the SDK tree (`pkg/analyzer_cli`, old `dartanalyzer`); `dart analyze` no longer uses it.
- Other consumers of `dart language-server` LSP: Neovim, Emacs lsp-dart, Helix, Zed, Sublime (documented in `lsp_spec/README.md` lines 5-9). They send no Dart-specific init options usually.
- DTD (`dart/connectToDtd`): Dart-Code and flutter_tools both call it. It is optional; a not-supported error is tolerated by both clients (Dart-Code catches and logs).

## Requirements for dartr
1. Ship a binary that can be started as `dartr language-server [--protocol=lsp|analyzer] [flags]` and parse every flag in section 3 (accept and ignore unknown-but-listed ones, including `--instrumentation-log-file`, `--session-log`, `--port`, `--dart-sdk/--sdk`, `--client-id/--client-version`, `--disable-server-feature-*`, `--no-plugins`, `--no-with-fine-dependencies`). Default protocol is LSP for `language-server`.
2. Editor drop-in: Dart-Code and IntelliJ always exec `<sdk>/bin/dart ...`. So dartr must work as (a) a `dart` shim inside a fake SDK dir (`bin/dart`, plus whatever layout files editors read: `version`, `bin/snapshots/...` for `dart analyze --sdk-path` checks), or (b) a wrapper `.dart` script for `dart.analyzerPath`. Native-binary `analyzerPath` is not supported by Dart-Code. Verify the SDK-detection files used by Dart-Code and IntelliJ in a follow-up.
3. LSP priority 1 (needed by Dart-Code and flutter analyze): `initialize`/`initialized`/`shutdown`/`exit`, text sync, `textDocument/publishDiagnostics`, `workspace/configuration` + `didChangeConfiguration`, `$/progress` + `window/workDoneProgress/create`, workspace folders, `workspace/executeCommand`, `codeAction`. Priority 2: the other 45 standard methods and the custom methods in section 3 (`dart/textDocument/publishOutline`, `publishFlutterOutline`, `publishClosingLabels`, `dart/reanalyze`, `dart/diagnosticServer`, `super`, `imports`, `augmented`, `augmentation`, `dart/connectToDtd`, `dart/openUri`, `dart/updateDiagnosticInformation`, `command/resolve`). Accept the Dart-Code `initializationOptions` keys and `experimental` client capabilities listed in section 1 without error.
4. Legacy protocol milestone A (`dart analyze` / `dart fix` clone, tiny): `server.setSubscriptions` (STATUS), `server.shutdown`, `server.getVersion`, `analysis.setAnalysisRoots`, `analysis.updateContent`, `edit.bulkFixes`; events `server.status`, `analysis.errors`, `server.error`. Line-delimited JSON. Must emit `isAnalyzing` true then false, and full `AnalysisError` fields. Milestone B (IntelliJ): the rest of the 59 requests / 22 notifications, starting with analysis.{getErrors,getHover,getNavigation,setPriorityFiles,setSubscriptions,reanalyze}, completion.getSuggestions2, edit.{format,getFixes,getAssists,organizeDirectives,sortMembers,getRefactoring}, search.*.
5. Own `dartr analyze` CLI: implement the options, three formats and exit codes in section 4 exactly (golden tests against output from SDK 3.13.3; machine format escaping and json `version:1` are stable contracts; default format is user-facing and can drift but copy it). It can call the analyzer in-process without legacy protocol, but keep the TODO/INFO filter and the priority-file ordering.
6. `flutter analyze` compatibility: LSP, `dart language-server --dart-sdk ...`, `$/progress` begin/end for analysis, publishDiagnostics; add `dart/workspace/analysis/complete` (not in 3.13.3) for newer Flutter.
7. Test with real clients: scripted LSP/legacy transcripts recorded from the 3.13.3 server (`--protocol-traffic-log`) plus a run of Dart-Code and `flutter analyze` against dartr.
