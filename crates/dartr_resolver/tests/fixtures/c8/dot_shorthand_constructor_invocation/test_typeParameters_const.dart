// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_typeParameters_const).

class C {
  const C();
}

void main() {
  C c = const .new<int>();
//                ^^^^^
// [diag.wrongNumberOfTypeArgumentsDotShorthandConstructor] The dot shorthand resolves to the constructor 'C.new', and type parameters can't be applied to dot shorthand constructor invocations.
  print(c);
}
