// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_functionExpression_call_field).

class C {
  static final C field = C();
  C call() => this;
}

void main() {
  final C _ = .field();
}
