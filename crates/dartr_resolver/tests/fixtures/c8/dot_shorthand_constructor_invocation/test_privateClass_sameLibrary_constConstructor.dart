// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_privateClass_sameLibrary_constConstructor).

class _Private {
  const _Private.named();
}

typedef Public = _Private;
const Public p = _Private.named();

void main() {
  var x = p;
  x = const .named();
  print(x);
}
