// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_const_class).

class C {
  static const C member = const C._(1);
  final int x;
  C(this.x);
  const C._(this.x);
}

void main() {
  const C c = .member;
  print(c);
}
