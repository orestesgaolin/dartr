// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_privateMixin_sameLibrary).

mixin _Private {
  static _Private get getter => C();
}

class C with _Private {}
typedef Public = _Private;
final Public p = C();

void main() {
  var x = p;
  x = .getter;
  print(x);
}
