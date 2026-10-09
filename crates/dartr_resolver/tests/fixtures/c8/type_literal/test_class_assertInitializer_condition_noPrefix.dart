// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_assertInitializer_condition_noPrefix).

class C {}
class A {
  A() : assert(C);
//             ^
// [diag.nonBoolExpression] The expression in an assert must be of type 'bool'.
}
