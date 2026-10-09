// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_privateEnum_sameLibrary_constructor).

enum _Private {
  one;
  factory _Private.a() => one;
}

typedef Public = _Private;
final Public p = _Private.one;

void main() {
  var x = p;
  x = .a();
  print(x);
}
