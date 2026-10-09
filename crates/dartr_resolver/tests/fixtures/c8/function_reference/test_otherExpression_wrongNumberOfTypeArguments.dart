// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_otherExpression_wrongNumberOfTypeArguments).

void f(void Function<T>(T a) foo, void Function<T>(T a) bar) {
  (1 == 2 ? foo : bar)<int, String>;
//                    ^^^^^^^^^^^^^
// [diag.wrongNumberOfTypeArgumentsFunction] The type of this function is 'void Function<T>(T)', which has 1 type parameters, but 2 type arguments were given.
}
