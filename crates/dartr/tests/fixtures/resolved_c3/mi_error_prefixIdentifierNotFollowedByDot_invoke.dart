// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_prefixIdentifierNotFollowedByDot_invoke).

import 'dart:math' as foo;

main() {
  foo();
//^^^
// [diag.prefixIdentifierNotFollowedByDot] The name 'foo' refers to an import prefix, so it must be followed by '.'.
}
