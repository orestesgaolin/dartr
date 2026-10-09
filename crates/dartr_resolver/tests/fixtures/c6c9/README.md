These 19 standalone Dart inputs are source excerpts from the pinned analyzer's
`metadata_test.dart` (10), `comment_test.dart` (4), and
`extension_override_test.dart` (5). Each input identifies its original test.
The analyzer's resolved-node expectations are checked with the real Dart oracle:

```sh
perl -e 'alarm 300; exec @ARGV' target/release/difftest resolved-el \
  crates/dartr_resolver/tests/fixtures/c6c9 --no-diagnostics --timeout-per-file 5
perl -e 'alarm 300; exec @ARGV' target/release/difftest resolved \
  crates/dartr_resolver/tests/fixtures/c6c9 --no-diagnostics --timeout-per-file 5 \
  --kinds Annotation,CommentReference,ExtensionOverride,SimpleIdentifier,PrefixedIdentifier
```

The dump format exposes annotation and comment child identifiers. It does not
emit Annotation or CommentReference records, or a type record for a valid
ExtensionOverride. Resolver integration tests check the additional node fields.
Remaining mismatches from other resolver units are reported, not hidden.
