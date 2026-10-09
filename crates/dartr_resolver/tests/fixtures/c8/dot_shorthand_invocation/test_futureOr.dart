// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_futureOr).

import 'dart:async';

class C {
  static C member() => C(1);
  int x;
  C(this.x);
}

void main() {
  FutureOr<C> c = .member();
  print(c);
}
