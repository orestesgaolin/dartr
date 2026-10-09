// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_conflict_instance_getter).

class A {
  final int value; // Same name as constructor
  A.value(this.value);
}

void main() {
  A _ = .value(1);
}
