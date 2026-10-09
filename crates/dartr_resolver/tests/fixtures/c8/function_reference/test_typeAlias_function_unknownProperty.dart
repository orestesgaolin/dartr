// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_typeAlias_function_unknownProperty).

typedef Cb = void Function();

var a = Cb.foo<int>;
//         ^^^
// [diag.undefinedGetter] The getter 'foo' isn't defined for the type 'Type'.
