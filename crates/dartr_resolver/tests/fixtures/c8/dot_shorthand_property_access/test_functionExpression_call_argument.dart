// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_functionExpression_call_argument).

class C {
  static final C field = C();
  C call(int a) => this;
}

void main() {
  final C _ = .field(1);
}
