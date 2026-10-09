// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_abstractClass_const_typeArguments).

abstract class C {
  static C fn() => CB.named(1);
}

class CB implements C {
  final int x;
  CB.named(this.x);
}

void main() {
  C c = const .fn<int>(1);
//             ^^
// [diag.constWithUndefinedConstructor] The class 'C' doesn't have a constant constructor 'fn'.
//               ^^^^^
// [diag.wrongNumberOfTypeArgumentsDotShorthandConstructor] The dot shorthand resolves to the constructor 'C.fn', and type parameters can't be applied to dot shorthand constructor invocations.
  print(c);
}
