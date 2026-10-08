# dartr_yaml

Rust port of `package:yaml` **3.1.4**. Each source file names its Dart reference.
The scanner and parser retain the upstream state machines. The loader applies
its core schema, explicit tags, aliases, duplicate-key equality, and collection
span rules. Mapping entries preserve source order and support complex keys.

```rust
let node = dartr_yaml::load_yaml_node("name: package\nrules: [one, two]\n")?;
let recovered = dartr_yaml::load_yaml_node_with_options("one: any\ntwo\n", true);
// recovered.node may be None: recovery does not suppress fatal parser errors.
// recovered.errors contains recovered errors, then the fatal exception, if any.
// recovered.warnings contains unknown-directive and version warnings.
# Ok::<(), dartr_yaml::YamlException>(())
```

`load_yaml_document`, `load_yaml_documents`, and `load_yaml_stream` expose
stream/document metadata. `load_yaml_node_with_listener` reports recovered
errors to an `ErrorListener`. `load_yaml_node_with_warning_callback` supplies
per-load warning callbacks instead of Dart's process-global callback.

`FileSpan` and `SourceLocation` use **UTF-16 code units**, matching Dart.
`SourceFile::byte_offset` converts a location for UTF-8 consumers. A location
between surrogate code units maps to the beginning of the corresponding UTF-8
character; use the original UTF-16 span for diagnostics. `dartr_project` keeps
these original error spans alongside its existing byte-span node model.
The analyzer reads span locations directly, so terminal `SourceSpan.message()`
rendering is outside this crate's scope.

A few malformed inputs cause the upstream package to throw a non-YAML Dart
exception (numeric tags with empty values, overflowing version integers, and
invalid URI escapes). The Rust API returns these as failures with
`YamlException::runtime_error` set to the exact Dart exception text. Such
failures have no YAML diagnostic span. This preserves observable oracle output
without using Rust panics. Callback dispatch occurs after the load completes.

## Validation

The Dart oracle dumps values, styles, ordered node trees, both span endpoints,
recovered/fatal errors, and non-YAML exceptions. Separate oracles compare
warning callbacks and document metadata. The integration tests require Dart
3.13.3 and a package configuration resolving yaml 3.1.4 and the pinned analyzer.

```sh
export DARTR_DART=/path/to/dart-sdk/bin/dart
export DARTR_ORACLE_PACKAGES=/path/to/tools/oracle/.dart_tool/package_config.json
cargo test -p dartr_yaml -- --nocapture
cargo test -p dartr_yaml --test differential fvm_and_pub_cache_corpus_parity -- --ignored --nocapture
```

The ignored corpus visits all `.yaml`/`.yml` files under `~/fvm/default` and all
pubspecs under `~/.pub-cache/hosted/pub.dev`, comparing strict and recovery loads.
The active suite extracts static YAML inputs from the upstream `yaml_test.dart`
and `span_test.dart`, and adds directed cases and deterministic malformed
mutations. The separate yaml-test-suite descriptor checkout is optional and is
not included in the hosted package archive.

Code derived from yaml uses MIT licensing; source-span code uses BSD-3-Clause.
See `LICENSE` and `source_span-LICENSE`.
