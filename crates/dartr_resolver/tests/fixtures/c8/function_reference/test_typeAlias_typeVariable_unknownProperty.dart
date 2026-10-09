// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_typeAlias_typeVariable_unknownProperty).

typedef T<E> = E;

var a = T.foo<int>;
//        ^^^
// [diag.undefinedGetter] The getter 'foo' isn't defined for the type 'Type'.
