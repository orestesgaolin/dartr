// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_uriDoesNotExist_show).

import 'missing.dart' show foo, bar;
//     ^^^^^^^^^^^^^^
// [diag.uriDoesNotExist] Target of URI doesn't exist: 'missing.dart'.

main() {
  foo(1);
  bar(2);
}
