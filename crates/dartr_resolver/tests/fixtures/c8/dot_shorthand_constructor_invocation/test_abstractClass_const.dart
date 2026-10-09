// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_abstractClass_const).

abstract class C {
  static C fn() => CB.named(1);
}

class CB implements C {
  final int x;
  CB.named(this.x);
}

void main() {
  C c = const .fn(1);
//             ^^
// [diag.constWithUndefinedConstructor] The class 'C' doesn't have a constant constructor 'fn'.
  print(c);
}
