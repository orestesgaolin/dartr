// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_conflict_instance_method_factory).

class A {
  final int val;
  A._(this.val);
  factory A.foo() => A._(1);
  A? foo() => A._(2); // Same name as constructor
}

void main() {
  A _ = .foo();
}
