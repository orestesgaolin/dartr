# `elements` fixtures

Small Dart libraries that together cover every declaration kind of the
`elements` dump (docs/design/semantics.md §5.1), and the oracle output for
each of them.

- `<name>.dart`: input. `library.dart` has a part (`library_part.dart`),
  imports (prefix, show/hide, deferred, a missing file) and exports of the
  other fixtures.
- `<name>.oracle.jsonl`: the output of `oracle elements <abs path>` for that
  file alone. The oracle writes absolute paths and `file://` URIs; the
  repository root prefix is replaced by `$ROOT` so that the files do not
  depend on the checkout location. Replace `$ROOT` with the absolute
  repository root before you compare.

Regenerate (from the repository root):

```sh
ROOT=$PWD
for f in crates/dartr/tests/fixtures/elements/*.dart; do
  target/oracle/oracle elements "$ROOT/$f" | sed "s#$ROOT#\$ROOT#g" > "${f%.dart}.oracle.jsonl"
done
```

`target/oracle/oracle` is compiled by `difftest` on first use
(`target/release/difftest elements crates/dartr/tests/fixtures/elements`).
