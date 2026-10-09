// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_tearOff_constructor).

class C1 {
  C1.id();

  @override
  bool operator ==(Object other) => identical(C1.id, other);
}

main() {
  bool x = C1.id() == .id;
  print(x);
}
