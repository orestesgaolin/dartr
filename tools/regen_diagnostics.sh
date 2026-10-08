#!/usr/bin/env zsh
# Regenerates the diagnostics crate and its parity fixture from the pinned
# Dart SDK sources (third_party/dart-sdk, see tools/fetch_sdk.sh).
#
# 1. tools/diagnostics_gen (Rust) reads the messages.yaml files and writes
#    crates/dartr_diagnostics/src/generated/{diag,cfe_codes}.rs.
# 2. tools/diagnostics_oracle (Dart, pinned package:analyzer) lists every
#    code with sample-formatted messages and writes
#    crates/dartr_diagnostics/tests/fixtures/dart_diagnostics.json.
#
# Then `cargo test -p dartr_diagnostics --test parity` compares Rust with Dart.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"

cargo run --quiet --release --manifest-path "$root/tools/diagnostics_gen/Cargo.toml" -- "$root"
rustfmt --edition 2024 "$root"/crates/dartr_diagnostics/src/generated/*.rs

oracle="$root/tools/diagnostics_oracle"
(cd "$oracle" && dart pub get >/dev/null)
# package:analysis_server cannot be resolved by pub here (its dependencies are
# not in the sparse checkout), but its diagnostic library only imports
# package:analyzer and package:_fe_analyzer_shared. Add it by hand.
jq '.packages += [{"name": "analysis_server", "rootUri": "../../../third_party/dart-sdk/pkg/analysis_server", "packageUri": "lib/", "languageVersion": "3.13"}]' \
  "$oracle/.dart_tool/package_config.json" > "$oracle/.dart_tool/package_config_oracle.json"
mkdir -p "$root/crates/dartr_diagnostics/tests/fixtures"
dart --packages="$oracle/.dart_tool/package_config_oracle.json" "$oracle/bin/list_diagnostics.dart" \
  > "$root/crates/dartr_diagnostics/tests/fixtures/dart_diagnostics.json"
echo "wrote crates/dartr_diagnostics/tests/fixtures/dart_diagnostics.json" >&2
