// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_nested_property).

class C<T> {
  static C get member => C(1);
  T x;
  C(this.x);
}

void main() {
  C<C> c = .new(.member);
  print(c);
}
