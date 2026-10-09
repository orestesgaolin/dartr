// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_functionExpression_call_extension_getter).

class C {
  static C get getter => C();
}

extension CallC on C {
  C call() => this;
}

void main() {
  final C _ = .getter();
}
