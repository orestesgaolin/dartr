// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_functionReference).

class C<T> {
  static String foo<X>() => "C<$X>";

  @override
  bool operator ==(Object other) {
    return false;
  }
}

void test<T extends num>() {
  C() == .foo<T>;
}

main() {
  test<int>();
}
