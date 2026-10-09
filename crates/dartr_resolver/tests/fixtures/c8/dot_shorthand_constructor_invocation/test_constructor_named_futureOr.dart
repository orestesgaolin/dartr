// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_constructor_named_futureOr).

import 'dart:async';

class C<T> {
  T value;
  C.id(this.value);
}

void main() async {
  FutureOr<C?>? c = .id(2);
  print(c);
}
