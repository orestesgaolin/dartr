// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_privateEnum_sameLibrary_invocation).

enum _Private {
  one;
  static _Private instance() => one;
}

typedef Public = _Private;
final Public p = _Private.one;

void main() {
  var x = p;
  x = .instance();
  print(x);
}
