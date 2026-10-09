// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_conflict_instance_setter).

class A {
  int? val;
  A.value(this.val);

  // Same name as constructor
  set value(int v) {
    val = v;
  }
}

void main() {
  A _ = .value(1);
}
