// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_functionExpression_call_generic).

class C {
  static final C field = C();
  C call<T>(T t) => this;
}

void main() {
  final C _ = .field<int>(1);
}
