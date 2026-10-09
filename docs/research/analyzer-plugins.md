# Analyzer plugins in Dart SDK 3.13.3, and options for dartr

Sources: `third_party/dart-sdk/pkg/...` (paths below are relative to `pkg/`). Date: 2026-10-09.

## 1. Two plugin systems
| | new: `analysis_server_plugin` | legacy: `analyzer_plugin` |
|---|---|---|
| enabled by | top-level `plugins:` in `analysis_options.yaml` | `analyzer: plugins: [name]` |
| plugin location | pub dependency (`version`, `path`, `git`, `hosted`) | `tools/analyzer_plugin/` inside the host package |
| entrypoint | `lib/main.dart` exports `final plugin = MyPlugin()` | `tools/analyzer_plugin/bin/plugin.dart` |
| API | `Plugin.register(PluginRegistry)`: lint rules, warning rules, fixes, assists | subclass of `ServerPlugin` |

Both are active in 3.13.3: `PluginWatcher.addedDriver` calls `_addLegacyPlugins` and `_addPlugins`
for every context (`analysis_server/lib/src/plugin/plugin_watcher.dart:40-76`). Docs:
`analysis_server_plugin/doc/using_plugins.md`, `writing_a_plugin.md`, `writing_rules.md`.

### Configuration (new system)
- Only the root options file of a context is read: `AnalysisDriver.pluginsOptions`
  (`analyzer/lib/src/dart/analysis/driver.dart:418`). Nested options files cannot add plugins.
- Parsed into `PluginConfiguration` (name, `PluginSource`, `diagnostics:` map, enabled flag)
  in `analyzer/lib/src/analysis_options/analysis_options.dart:581`.
- Plugin warnings are on by default; lint rules are off until `diagnostics: {rule: true}`.
  Ignore comments: `// ignore: plugin_name/code`.

visible-app: `bench/corpus/visible-app/design_system/analysis_options.yaml` has
`plugins: jaspr_lints: {version: ^0.7.2, diagnostics: {prefer_html_components: true,
sort_children_last: true, styles_ordering: true}}`. `jaspr_lints` 0.7.2
(`~/.pub-cache/hosted/pub.dev/jaspr_lints-0.7.2`) registers 5 rules (`lib/main.dart`),
about 700 lines of rule code plus 700 lines of helpers (`lib/src/utils/*`), and 14 assists.

## 2. How the server builds and runs a new-style plugin
1. For each context root with `plugins:`, `PluginWatcher._addPlugins` writes a synthetic
   package into the server state folder (`.plugin_manager/<md5 of root path>`):
   `pubspec.yaml` and `bin/plugin.dart` (`plugin2/generator.dart`). The pubspec depends on
   `analysis_server_plugin: ^0.3.8` (hard coded) and on every configured plugin. The
   entrypoint is `main(List<String>, SendPort)`; it builds
   `PluginServer.new2(resourceProvider, plugins: {...})`, calls `initialize()` and
   `start(PluginIsolateChannel(sendPort))`.
2. `PluginManager._computeFiles` runs `dart pub upgrade` in that folder
   (`plugin/plugin_manager.dart:326-370, 860-890`). This needs the pub cache and network on
   the first run. pub chooses the plugin's own `analyzer` version (jaspr_lints needs
   `analyzer ^12.1.0`), not the SDK's analyzer.
3. If the server itself is an AOT build, the plugin entrypoint is compiled to an AOT
   snapshot first (`_compileAsAot`, same file, 600-640). This takes tens of seconds, then is cached.
4. `ServerIsolateChannel._spawnIsolate` calls `Isolate.spawnUri(entrypoint, [], sendPort,
   packageConfig: .dart_tool/package_config.json)` (`plugin/server_isolate_channel.dart:174`).
   The plugin runs in an isolate of the server process. All messages are JSON maps sent over
   `SendPort`, not over stdio (`analyzer_plugin/lib/src/channel/isolate_channel.dart`).
   The plugin first sends its own `SendPort` back (line 94).
5. `dart analyze --no-plugins` (hidden flag, `dartdev/lib/src/commands/analyze.dart:107`)
   switches this off. Plugin crashes arrive as `server.pluginError` and make `dart analyze`
   print to stderr and set the server-error exit code (`dartdev/lib/src/analysis_server.dart:306`).

Legacy plugins: folder copied to the state folder, `dart pub get`/`upgrade`
(`plugin_manager.dart:347-380`); the rest is the same.

## 3. Protocol between server and plugin
Message format: `analyzer_plugin` protocol (`analyzer_plugin/lib/protocol/protocol_constants.dart`).
Server to plugin requests handled by `PluginServer._getResponse`
(`analysis_server_plugin/lib/src/plugin_server.dart:685-760`):

| request | content |
|---|---|
| `plugin.versionCheck` | byte store path, SDK path, protocol version `1.0.0-alpha.0`. Plugin answers compatible, interesting files `['*.dart']`. |
| `analysis.setContextRoots` | roots, excluded paths, options file. New plugins ignore it when `setAnalysisRoots` was received. |
| `analysis.setAnalysisRoots` | included and excluded folders (`plugin_isolate.dart:_updatePluginRoots`) |
| `analysis.setPriorityFiles` | open files |
| `analysis.updateContent` | overlays (add, change, remove) |
| `analysis.handleWatchEvents` | file add, modify, remove |
| `edit.getFixes`, `edit.getAssists` | position in a file |
| `plugin.details`, `plugin.shutdown` | |

Plugin to server notifications: `analysis.errors` (per file, `AnalysisError` list) and
`plugin.status` (`isAnalyzing`), `plugin.error`. The server merges plugin errors into its own
`analysis.errors` (`plugin/notification_manager.dart`, `result_merger.dart`) and counts
`pluginStatusAnalyzing` in `server.status` (`legacy_analysis_server.dart:457`).

## 4. What the plugin receives: no AST, no elements
The server does not send any parsed or resolved data. The plugin analyzes the code by itself:

- `PluginServer._createContextCollection` creates its own `AnalysisContextCollectionImpl` with
  its own driver and byte store; it calls `driver.addFile` for all Dart files of each context
  (`plugin_server.dart:623-663, 349-368`). It turns off warnings and lints in that driver
  (`..warning = false ..lint = false`) because the server reports those.
- For each library it calls `session.getResolvedLibrary`, builds `RuleContextUnit`s with the
  resolved `CompilationUnit`, `typeProvider` and `typeSystem`, runs the rule visitors, filters
  ignore comments (`IgnoreInfo.forDart(..., pluginName:)`) and sends the result
  (`_computeAnalysisErrors`, 372-530).
- The diagnostic is always `AnalysisErrorType.STATIC_WARNING`, severity from `diagnostics:`
  config, default `INFO` (`plugin_server.dart:480-485`). That is why `styles_ordering` shows as
  `INFO - STATIC_WARNING` in `dart analyze`. The code is `diagnosticCode.lowerCaseName`, without
  a plugin prefix.
- Rule code uses the public API of `package:analyzer` (visitors, `node.staticType`,
  `fn.element is MethodElement`, `enclosingElement`, `formalParameters`; see
  `jaspr_lints/lib/src/rules/styles_ordering_rule.dart`). It runs against the analyzer version that
  pub selects, inside a Dart isolate.

Result: dartr cannot feed these plugins with its parse or resolution results: they need a Dart
`AnalysisSession`. dartr does not have to feed anything: the plugin reads files from disk itself.
The cost is a second parse and resolve of the whole project in Dart. Legacy `ServerPlugin` does
the same (`analyzer_plugin/lib/plugin/plugin.dart:251`).

## 5. Options for dartr
### (a) Run the plugin out of process, implement the server side of the protocol

dartr must do:
- Write the synthetic package (pubspec + entrypoint, about 90 lines of Dart in `generator.dart`) in
  a dartr state folder; run `dart pub upgrade`; copy legacy plugin folders.
- A small Dart bridge program (JIT, run by `dart`): `Isolate.spawnUri(entrypoint, [], port,
  packageConfig: ...)` and relay the JSON maps as lines over stdio. Rust cannot own a `SendPort`.
  The existing `tools/shim/dartr_shim.dart` is a model. No AOT step is needed with the JIT VM.
- A Rust client for the requests in section 3 and 3 notifications; the data sent is only
  paths, overlays and watch events. Positions in `AnalysisError` already contain offset, line and
  column, so no mapping is needed.
- Wait for `plugin.status` idle before `dartr analyze` prints; print `plugin.error` like dartdev.
- Merge plugin diagnostics with the `errors:` processors and the output formats (already in dartr).

Cost: needs a Dart SDK, `dart pub` and network (as `dart analyze` does). Time: one full Dart
resolve of the project in the plugin isolate (no lints, no warnings). Run in parallel with the
native analysis, wall time is the larger of the two, not the sum. The plugin analyzer version can
differ from the pinned SDK; real `dart analyze` has the same property.
### (b) Port popular plugins to Rust

Pro: no VM, full speed, no network. Con: needs dartr's element model and type system to be
complete (phases 5-7); each plugin needs a rule-by-rule port (jaspr_lints: 5 rules, about 1500
lines, plus `scope_tree`/`imports_verifier` helpers); no coverage for custom or private plugins and
for `custom_lint`-based plugins (riverpod_lint and others); a port must follow every plugin
release. Good as a later optimization for the few plugins that matter.
### (c) Delegate to the real analysis server when plugins are configured

Run `dart analyze --format=json` (or `dart language-server`) and keep only plugin diagnostics.
Pro: smallest code, always compatible. Con: the real server does the full analysis, including
lints and warnings, in addition to the plugin isolate, so the run is slower than `dart analyze`
alone and dartr gains nothing. Plugin diagnostics are not marked as such in the output; they
must be guessed by subtracting dartr's own diagnostics, or by code names from `plugin.details`
(which a plain `dart analyze` does not return). In LSP, a second analysis server doubles memory.

## 6. Recommendation
1. Implement (a): the only option that is correct for all plugins, needs no AST from dartr,
   and costs less than (c).
2. Start it only when the root options file has `plugins:` or `analyzer: plugins:`; add
   `--no-plugins` like dartdev.
3. First `dartr analyze` (setAnalysisRoots, wait for idle, shutdown), then LSP (overlays,
   priority files, fix and assist forwarding).
4. Run the plugin concurrently with the native analysis; report its time separately in `bench/`.
5. Consider (b) later only for `jaspr_lints`-class plugins, if VM start and double resolve
   dominate; never as the only path.
