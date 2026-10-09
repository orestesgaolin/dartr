// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_basic_generic).

class C<T> {
  static C member<U>(U x) => C(x);
  T x;
  C(this.x);
}

void main() {
  C c = .member<int>(1);
  print(c);
}
