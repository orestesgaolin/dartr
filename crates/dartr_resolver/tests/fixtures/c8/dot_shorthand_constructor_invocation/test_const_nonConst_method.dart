// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_const_nonConst_method).

class C {
  static C fn() => C.named(1);
  final int x;
  C.named(this.x);
}

void main() {
  C c = const .fn(1);
//             ^^
// [diag.constWithUndefinedConstructor] The class 'C' doesn't have a constant constructor 'fn'.
  print(c);
}
