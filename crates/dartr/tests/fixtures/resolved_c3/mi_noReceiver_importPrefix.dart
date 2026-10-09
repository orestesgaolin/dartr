// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_noReceiver_importPrefix).

import 'dart:math' as math;

main() {
  math();
//^^^^
// [diag.prefixIdentifierNotFollowedByDot] The name 'math' refers to an import prefix, so it must be followed by '.'.
}
