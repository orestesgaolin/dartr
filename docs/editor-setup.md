# Editor setup: dartr as the Dart language server

`dartr language-server` is an LSP server over stdin/stdout. It accepts the
command line options of `dart language-server` (SDK 3.13.3) and the Dart LSP
extensions that Dart-Code uses. Status: phase 10a (see "What works" below).

## Build

```sh
cargo build --release -p dartr     # target/release/dartr
```

## VS Code (Dart-Code)

Dart-Code always starts a custom analysis server with the Dart VM:
`dart <dart.analyzerPath> --lsp --client-id=VS-Code --client-version=… [dart.analyzerAdditionalArgs]`.
It cannot start a native binary, so `dart.analyzerPath` points to the shim
`tools/shim/dartr_shim.dart`. The shim starts `dartr language-server` with the
same arguments, forwards stdin, stdout and stderr byte for byte, and exits with
the exit code of dartr.

The shim finds dartr in this order:

1. the environment variable `DARTR_BIN` (it must be set for the VS Code process,
   for example in the shell that runs `code`), or
2. a file `dartr` in the folder of the shim (or of its compiled form).

### Option A: the shim source

```jsonc
// settings.json (user or workspace)
{
  "dart.analyzerPath": "/path/to/dartr/tools/shim/dartr_shim.dart"
}
```

and start VS Code with `DARTR_BIN=/path/to/dartr/target/release/dartr code .`.

### Option B: a compiled shim next to the binary (no environment variable)

```sh
mkdir -p ~/.dartr
dart compile kernel tools/shim/dartr_shim.dart -o ~/.dartr/dartr_shim.dill
cp target/release/dartr ~/.dartr/dartr
```

```jsonc
{
  "dart.analyzerPath": "/Users/<you>/.dartr/dartr_shim.dill"
}
```

The kernel file starts faster than the source (about 0.09 s against 0.16 s for
`--help` on an M-series Mac). Do not use `dart compile aot-snapshot`: Dart-Code
runs the file with `dart`, not with `dartaotruntime`.

### Useful settings

- `"dart.analyzerInstrumentationLogFile": "/path/log.txt"`: Dart-Code passes
  `--instrumentation-log-file`, and dartr writes the protocol traffic there
  (one line per message: `<epoch ms>:=>:` for messages from the client,
  `<epoch ms>:<=:` for messages to the client).
- `"dart.analyzerAdditionalArgs"`: passed to dartr unchanged.
- To go back to the Dart server, remove `dart.analyzerPath` and restart the
  extension ("Developer: Reload Window").

### Check that dartr runs

Run
`ps aux | grep "dartr language-server"`. The `initialize` result has
`serverInfo.name` `dartr LSP Analysis Server`.

## Other editors

Editors that start a language server command directly (Neovim, Helix, Zed,
Emacs, Sublime) can run dartr without the shim:

```sh
dartr language-server --client-id=<editor> --client-version=<version>
```

For example Helix (`languages.toml`):

```toml
[language-server.dartr]
command = "/path/to/dartr"
args = ["language-server"]

[[language]]
name = "dart"
language-servers = ["dartr"]
```

## `flutter analyze`, IntelliJ

`flutter analyze` and IntelliJ start `<sdk>/bin/dart language-server …` from the
Flutter/Dart SDK and have no setting for another server; they need an SDK whose
`bin/dart` forwards `language-server` to dartr. That is not part of phase 10a.
IntelliJ also uses the legacy protocol (`--protocol=analyzer`). dartr now
implements its initial analysis subset; full IntelliJ editing services still
need resolution. See [legacy protocol support](legacy-protocol.md) for the
implemented requests and limitations.

## What works (phase 10a)

Implemented and compared with `dart language-server` 3.13.3 by
`cargo test -p dartr --test lsp_parity --test lsp_capabilities`:

- lifecycle: `initialize` (same capabilities as Dart for the implemented
  features, static and dynamic registration), `initialized`, `shutdown`, `exit`
  (exit code 0 after `shutdown`, else 1), `$/cancelRequest`;
- documents: `textDocument/didOpen`, `didChange` (incremental and full),
  `didClose`; `workspace/didChangeWorkspaceFolders`,
  `workspace/didChangeConfiguration` (`dart.analysisExcludedFolders`);
- diagnostics: `textDocument/publishDiagnostics` with the **parse** diagnostics
  of every Dart file in the workspace folders (no type errors, warnings, lints
  or TODOs yet), `$/progress` for analysis (`$/analyzerStatus` for clients
  without `window.workDoneProgress`);
- `textDocument/documentSymbol`, `textDocument/foldingRange`,
  `textDocument/selectionRange`;
- `dart/textDocument/publishClosingLabels`, `dart/textDocument/publishOutline`
  for open files; `dart/workspace/analysis/complete` (not in 3.13.3; newer
  Flutter tools call it).

Not yet: everything that needs resolution (hover, completion, navigation,
code actions, formatting, semantic tokens, `publishFlutterOutline`, ...). The
list of capabilities that the Dart server has and dartr does not is
`crates/dartr/tests/lsp_fixtures/missing_capabilities.txt`. Closing labels and
outlines use syntactic stand-ins for resolved facts (see
`crates/dartr_server/src/computer/heuristics.rs`); Flutter widget nodes in the
outline need static types. dartr does not watch the file system yet: files
that change on disk (for example after `git checkout`) are analyzed again
after a change of the workspace folders or a restart.
