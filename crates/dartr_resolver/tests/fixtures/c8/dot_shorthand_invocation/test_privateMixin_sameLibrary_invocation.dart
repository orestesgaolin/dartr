// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_privateMixin_sameLibrary_invocation).

mixin _Private {
  static _Private instance() => C();
}

class C with _Private {}
typedef Public = _Private;
final Public p = C();

void main() {
  var x = p;
  x = .instance();
  print(x);
}
