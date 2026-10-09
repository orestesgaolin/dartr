// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_enum_constructor).

enum E {
  v.named();

  const E.named();
}

void f() {
  E e = .named();
//       ^^^^^
// [diag.invalidReferenceToGenerativeEnumConstructor] Generative enum constructors can only be used to create an enum constant.
  print(e);
}
