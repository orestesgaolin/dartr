# dartr_lints

AST and source-based lint rules from Dart SDK 3.13.3. The registry includes metadata
for all 266 upstream rules; 79 AST-only rules have node processors. See [RULES.md](RULES.md)
for the complete classification and the reason each remaining rule is deferred.

```rust
use dartr_ast_builder::parse_string;
use dartr_lints::lint;

let source = "class A {} void f() { new A(); }";
let parsed = parse_string(source, "/project/lib/example.dart");
let diagnostics = lint(&parsed, source, "/project/lib/example.dart", &["unnecessary_new"]);
assert_eq!(diagnostics[0].message, "Unnecessary 'new' keyword.");
```

`run_lints(parsed, enabled)` uses the source owned by `ParsedUnit` and no file path.
Use `lint` for rules that need a real path or package membership. Package membership
is found from the defining unit's nearest `pubspec.yaml` ancestor. `lint_library`
accepts ordered `RuleContextUnit` values: defining unit first, then its parts.
It registers processors once, visits every unit, runs after-library callbacks, and
filters diagnostics using the scanner's ignore comments. Offsets and lengths use
UTF-16 code units. `lint_with_config` also applies configured severities.

Unsupported rules have metadata but no processors. Call
`Registry::builtin().get_rule(name).is_implemented()` before using a rule when the
caller needs to reject unsupported names. Removed rules intentionally report nothing.
The active upstream `package_prefixed_library_names` visitor is also a no-op.

`RuleVisitorRegistry` has an ordered processor list for every concrete AST node kind,
a generated `add_<node_kind>` method, optional rule timing, and after-library callbacks.
Its visitor visits subscriptions before children. Rust panics propagate to the caller.
`NodeLintRegistry`, `LintRule`, and `RuleContext` are compatibility aliases.
The pinned source has no legacy lint category API.

Regenerate metadata and node subscriptions:

```sh
python3 tools/codegen/gen_lints.py
```

Run differential integration tests. Registry comparison uses the pinned analyzer
and linter dependencies from `tools/oracle`; run `dart pub get` there once if its
package configuration is absent. A worktree can reuse the main checkout's configuration.
Set `DARTR_ORACLE_PACKAGE_CONFIG` to select a different existing configuration.
The tests require Dart 3.13.3; `DARTR_DART` can select its direct SDK binary.

```sh
cargo test -p dartr_lints -- --nocapture
cargo clippy -p dartr_lints --all-targets --no-deps -- -D warnings
cargo build --release -p dartr_lints
python3 tools/lints_differential.py --corpus sdk --binary target/release/lints_dump --output target/lints/sdk
python3 tools/lints_differential.py --corpus flutter --binary target/release/lints_dump --output target/lints/flutter
```

The corpus tool copies sources into temporary fixture projects under its output
directory. It enables exactly the implemented rules, removes copied nested analysis
options, sets Dart language version 3.13, and compares only their lint codes. It saves
the complete oracle output, Rust output, exact differences, enabled-rule list, summary,
and per-rule TSV counts. Other diagnostics from unresolved copied SDK/Flutter imports
are excluded from the comparison; this does not test resolution.
