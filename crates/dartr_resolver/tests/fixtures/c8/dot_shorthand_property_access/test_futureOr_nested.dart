// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_futureOr_nested).

import 'dart:async';

enum C { red }

void main() {
  FutureOr<FutureOr<C>> c = .red;
  print(c);
}
