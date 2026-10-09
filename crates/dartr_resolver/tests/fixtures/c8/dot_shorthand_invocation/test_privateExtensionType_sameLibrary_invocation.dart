// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_privateExtensionType_sameLibrary_invocation).

extension type _Private(int it) {
  static _Private instance() => _Private(0);
}

typedef Public = _Private;
final Public p = _Private(1);

void main() {
  var x = p;
  x = .instance();
  print(x);
}
