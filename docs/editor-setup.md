# Editor setup: dartr as the Dart language server

`dartr language-server` is an LSP server over stdin/stdout. It accepts the
command-line options of `dart language-server` (SDK 3.13.3) and the Dart LSP
extensions that Dart-Code uses. dartr reads the `dart:` libraries from a Dart
or Flutter SDK (3.13.3) on `PATH`.

## Install

Homebrew:

```sh
brew install orestesgaolin/tap/dartr
```

The Homebrew formula installs `dartr` in `bin` and `dartr_shim.dart` (with a
symlink to `dartr` next to it) in `libexec` (`$(brew --prefix dartr)/libexec/dartr_shim.dart`).

GitHub Releases: download `dartr-<version>-<target>.tar.gz` for
`aarch64-apple-darwin` or `x86_64-unknown-linux-gnu` from
<https://github.com/orestesgaolin/dartr/releases>. The archive contains
`bin/dartr` and `tools/shim/dartr_shim.dart`.

From source (Rust stable, edition 2024):

```sh
cargo build --release -p dartr     # target/release/dartr
```

## VS Code (Dart-Code)

Dart-Code always starts a custom analysis server with the Dart VM:
`dart <dart.analyzerPath> --lsp --client-id=VS-Code --client-version=… [dart.analyzerAdditionalArgs]`.
It cannot start a native binary directly, so `dart.analyzerPath` points to
`dartr_shim.dart`. The shim starts `dartr language-server` with the same
arguments, forwards stdin, stdout and stderr byte for byte, and exits with the
exit code of dartr.

### With Homebrew

Point `dart.analyzerPath` to `$(brew --prefix dartr)/libexec/dartr_shim.dart` in
your user or workspace `settings.json` (see `brew info orestesgaolin/tap/dartr`):

```json
{
  "dart.analyzerPath": "/opt/homebrew/opt/dartr/libexec/dartr_shim.dart"
}
```

### Without Homebrew (release archive or source build)

The shim finds `dartr` in this order:

1. the environment variable `DARTR_BIN` (it must be set for the VS Code process,
   for example in the shell that runs `code`), or
2. a file `dartr` in the folder of the shim (or of its compiled `.dill` form).

You can either set `DARTR_BIN=/path/to/dartr` and point `dart.analyzerPath` to
`tools/shim/dartr_shim.dart`, or place `dartr` and a compiled kernel shim in one
folder:

```sh
mkdir -p ~/.dartr
dart compile kernel tools/shim/dartr_shim.dart -o ~/.dartr/dartr_shim.dill
cp bin/dartr ~/.dartr/dartr
```

```json
{
  "dart.analyzerPath": "/Users/<you>/.dartr/dartr_shim.dill"
}
```

The kernel file starts faster than the source (about 0.09 s against 0.16 s for
`--help` on an M-series Mac). Do not use `dart compile aot-snapshot`: Dart-Code
runs the file with `dart`, not with `dartaotruntime`.

### Useful settings and switching back

- `"dart.analyzerInstrumentationLogFile": "/path/log.txt"`: Dart-Code passes
  `--instrumentation-log-file`, and dartr writes the protocol traffic there
  (one line per message: `<epoch ms>:=>:` for messages from the client,
  `<epoch ms>:<=:` for messages to the client).
- `"dart.analyzerAdditionalArgs"`: passed to dartr unchanged.
- Check that dartr runs with `ps aux | grep "dartr language-server"`. The
  `initialize` result has `serverInfo.name` `dartr LSP Analysis Server`.
- **To go back to the Dart server:** remove `dart.analyzerPath` from
  `settings.json` and restart the extension (`Developer: Reload Window`).

## Other editors

Editors that start a language server command directly (Neovim, Helix, Zed,
Emacs, Sublime) run dartr without the shim:

```sh
dartr language-server --client-id=<editor> --client-version=<version>
```

For example Helix (`languages.toml`):

```toml
[language-server.dartr]
command = "dartr"
args = ["language-server"]

[[language]]
name = "dart"
language-servers = ["dartr"]
```

`flutter analyze` and IntelliJ start `<sdk>/bin/dart language-server` from the
Flutter or Dart SDK and have no setting for another server. For clients that use
the legacy protocol (`--protocol=analyzer`), dartr implements all 59 requests
and 22 notifications; see [legacy-protocol.md](legacy-protocol.md).

## What works

Diagnostics, outline, closing labels, folding, selection ranges, document
symbols, formatting; navigation (definition, type definition, implementation,
references, document highlight), hover, signature help, semantic tokens, inlay
hints, call and type hierarchy, workspace symbols; completion; code actions
(quick fixes, assists, fix all, organize imports, sort members); rename;
refactorings (extract method, local variable and widget, inline local and
method, convert getter/method, move file); completion in `pubspec.yaml`,
`analysis_options.yaml` and `fix_data.yaml`; document colors, document links,
code lens, inline values; Flutter outline; files changed on disk are
re-analyzed; analyzer plugins (diagnostics, fixes and assists).

`dartr language-server` registers every capability of `dart language-server`
3.13.3 (`initialize`, static and dynamic registration, `shutdown`, `exit`,
`$/cancelRequest`, `$/progress` / `$/analyzerStatus`,
`dart/workspace/analysis/complete`).
