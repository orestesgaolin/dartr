// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_privateClass_sameLibrary_invocation).

class _Private {
  static _Private instance() => _Private();
}

typedef Public = _Private;
final Public p = _Private();

void main() {
  var x = p;
  x = .instance();
  print(x);
}
