# dartr

dartr is a port of the Dart analyzer to Rust. The reference version is the
Dart SDK **3.13.3**. It is one native binary with these commands:

- `dartr analyze`: the same diagnostics as `dart analyze` (errors, warnings,
  lints, analyzer plugins), with the same options, output formats and exit codes.
- `dartr format`: a port of `dart_style` (`dart format`).
- `dartr language-server`: an LSP server. Dart-Code (VS Code) starts it through a
  small shim; other editors can start it directly.
- `dartr analysis-server` or `--protocol=analyzer`: the legacy analysis server
  protocol (partial).

The aim is the same output as the Dart tools, produced faster. Correctness is
checked by comparing dartr with the real Dart analyzer on large corpora (Dart
SDK tests, Flutter, `analyzer`, a 1.5 million line application).

## Status

| Area | State |
|---|---|
| Scanner, parser, AST | At parity (token, event and AST dumps match the Dart analyzer on the corpora) |
| Element model, types, resolution, constants | At parity on Flutter, `flutter_tools` and `analyzer` |
| Error and warning verifiers | At parity on the SDK tests (a few known differences) |
| Lints | All rules ported; compared with `dart analyze` on the corpora |
| `dartr analyze` | Same options, formats and exit codes as `dart analyze` |
| `dartr format` | Same output as `dart format` on the `dart_style` test data and corpora |
| `dartr language-server` | Diagnostics, outline, closing labels, folding, selection ranges, document symbols, formatting. **Missing:** completion, quick fixes and assists, rename |
| Legacy protocol | **Partial:** an initial subset (see `docs/legacy-protocol.md`) |

`docs/PLAN.md` has the plan and `docs/TIMELINE.md` the project history. This is
early software: if dartr and `dart analyze` differ, dartr is wrong. Please report it
(see "Report a difference").

## Requirements

dartr needs a **Dart or Flutter SDK** on the machine. It reads the `dart:`
libraries (`dart:core` and the others) from that SDK, so it does not contain them.
It uses the SDK of the first `dart` executable on `PATH` (symbolic links are
resolved; for a Flutter SDK it uses `bin/cache/dart-sdk`). The SDK version
should be 3.13.3: dartr copies the behavior of that version. Run `dart --version`
to check.

Supported platforms: macOS arm64 and Linux x86_64.

## Install

Homebrew:

```sh
brew install orestesgaolin/tap/dartr
```

GitHub Releases: download `dartr-<version>-<target>.tar.gz` for
`aarch64-apple-darwin` or `x86_64-unknown-linux-gnu` from
<https://github.com/orestesgaolin/dartr/releases>, check it against the `.sha256`
file, and extract it. `bin/dartr` is the binary. The archive also has
`tools/shim/dartr_shim.dart` (for VS Code), `LICENSE` and `THIRD_PARTY_NOTICES.md`.

From source (Rust stable, edition 2024):

```sh
cargo build --release -p dartr     # target/release/dartr
```

`dartr --version` prints the dartr version and the pinned Dart SDK version.

## Use

```sh
dartr analyze                       # current package, like `dart analyze`
dartr analyze lib test/foo_test.dart
dartr analyze --format=json --fatal-infos
dartr format lib                    # like `dart format`
dartr format --output=none --set-exit-if-changed .
```

Options, output and exit codes are those of `dart analyze` and `dart format`
(including `--format=machine|json`, `--fatal-warnings`, `--fatal-infos`,
`--no-fatal-warnings`, `-o`, `--set-exit-if-changed`). Run `dartr analyze --help`.

**Analyzer plugins** (`plugins:` in `analysis_options.yaml`): as in `dart analyze`,
the plugins are resolved with `pub upgrade` on first use, so the first run needs
network access. Plugins run out of process and need the Dart SDK.

### Editors

- **VS Code (Dart-Code):** Dart-Code can only start the analysis server as a Dart
  script, so `dart.analyzerPath` must point to `tools/shim/dartr_shim.dart`. The
  shim starts `dartr language-server` with the same arguments. Set the environment
  variable `DARTR_BIN` to the dartr binary, or put `dartr` next to the shim. With
  Homebrew, the shim is at `$(brew --prefix dartr)/libexec/dartr_shim.dart` and
  `dartr` is next to it. See [docs/editor-setup.md](docs/editor-setup.md).
- **Neovim, Helix, Zed, Emacs, Sublime:** run `dartr language-server` directly.
- **`flutter analyze` and IntelliJ** start `<sdk>/bin/dart language-server` and have
  no setting for another server. They do not work with dartr yet.

## Compare with `dart analyze`

```sh
dart analyze --format=json  > dart.json;  echo "dart: $?"
dartr analyze --format=json > dartr.json; echo "dartr: $?"
diff <(jq -S . dart.json) <(jq -S . dartr.json)
```

Use the same SDK version (3.13.3) for both. Differences in `--format=machine`
output or in the exit code count as differences.

### Report a difference

Open an issue at <https://github.com/orestesgaolin/dartr/issues> with:

1. the smallest Dart file (and `pubspec.yaml` / `analysis_options.yaml` if they matter)
   that shows it;
2. the exact command, `dartr --version` and `dart --version`;
3. the output of both tools.

## Known limitations

- Language server: no completion, quick fixes, assists or rename. Files that change
  on disk are not watched yet.
- Legacy analysis server protocol: partial.
- Primary constructor bodies (a newer language feature) are not fully resolved.
- Cold analysis of a large package can be slower than `dart analyze` today (resolved
  lints, search index). See the benchmarks.
- macOS arm64 and Linux x86_64 only. No Windows or Intel macOS builds.

## Benchmarks

Baseline `dart analyze` 3.13.3, cold cache, Apple M5 Pro (15 cores):
Flutter `packages/flutter` 19.6 s, `flutter_tools` 7.6 s, `analyzer` 7.1 s, a 1.56 million
line application 72.8 s. Early `dartr analyze` runs (parse and syntax errors only)
take 0.1-0.5 s on the same corpora; with the full pipeline and lints, `flutter_tools`
took about 3.5 s against 2.6 s for Dart at the time of measurement.

**Caveat:** several of these measurements ran while other processes loaded the
machine (load average 11-21 on 15 cores). The final comparison on an idle machine is
pending, so treat the numbers as indicative only. Details and method:
[docs/BENCHMARKS.md](docs/BENCHMARKS.md).

## License

dartr is licensed under the BSD 3-Clause License (`LICENSE`). It is a port of
code of the Dart project and uses test data and diagnostic messages from it; the
licenses and copyright lines of that code are in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). dartr is not affiliated with or
endorsed by Google or the Dart team.
