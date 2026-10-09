// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_undefinedFunction_hasTarget_importPrefix).

import 'dart:math' as math;

main() {
  math.foo(0);
//     ^^^
// [diag.undefinedFunction] The function 'foo' isn't defined.
}
