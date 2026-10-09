// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_equality_nullAssert).

class C {
  int x;
  C(this.x);
  static C? nullable = C(1);
}

main() {
  print(C(1) == .nullable!);
}
