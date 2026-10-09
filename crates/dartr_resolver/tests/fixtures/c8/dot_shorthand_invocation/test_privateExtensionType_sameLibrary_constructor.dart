// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_privateExtensionType_sameLibrary_constructor).

extension type _Private(int i) {}

typedef Public = _Private;
final Public p = _Private(1);

void main() {
  var x = p;
  x = .new(1);
  print(x);
}
