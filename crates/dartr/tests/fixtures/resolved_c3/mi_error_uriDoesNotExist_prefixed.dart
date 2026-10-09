// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_uriDoesNotExist_prefixed).

import 'missing.dart' as p;
//     ^^^^^^^^^^^^^^
// [diag.uriDoesNotExist] Target of URI doesn't exist: 'missing.dart'.

main() {
  p.foo(1);
  p.bar(2);
}
