// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_privateClass_sameLibrary_constructor).

class _Private {
  _Private();
}

typedef Public = _Private;
final Public p = _Private();

void main() {
  var x = p;
  x = .new();
  print(x);
}
