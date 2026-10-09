// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_conflict_instance_method).

class A {
  final int val;
  A.value(this.val);
  A? value() => null; // Same name as constructor
}

void main() {
  A _ = .value(1);
}
