// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_typeParameter_argumentList_argument_hasTypeArguments).

class C<T> {
  void f(Object? x) {}
  void g() {
    f(T<int>);
//    ^
// [diag.disallowedTypeInstantiationExpression] Only a generic type, generic function, generic instance method, or generic constructor can have type arguments.
  }
}
