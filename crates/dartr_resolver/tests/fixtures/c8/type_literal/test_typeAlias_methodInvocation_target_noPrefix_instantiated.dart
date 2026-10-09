// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_typeAlias_methodInvocation_target_noPrefix_instantiated).

typedef Fn<T> = void Function(T);

void bar() {
  Fn<int>.foo();
//        ^^^
// [diag.undefinedMethodOnFunctionType] The method 'foo' isn't defined for the 'Fn' function type.
}

extension E on Type {
  void foo() {}
}
