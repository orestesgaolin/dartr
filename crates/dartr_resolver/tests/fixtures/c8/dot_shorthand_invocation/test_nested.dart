// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_nested).

class C<T> {
  static C<int> member() => C(1);
  static C<U> memberType<U, V>(U u) => C(u);
  T x;
  C(this.x);
}

void main() {
  C<C<C>> c = .memberType(.new(.member()));
  print(c);
}
