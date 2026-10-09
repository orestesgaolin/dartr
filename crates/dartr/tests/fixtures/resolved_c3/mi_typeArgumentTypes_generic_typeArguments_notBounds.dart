// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_typeArgumentTypes_generic_typeArguments_notBounds).

void foo<T extends num>() {}

main() {
  foo<bool>();
//    ^^^^
// [diag.typeArgumentNotMatchingBounds] 'bool' doesn't conform to the bound 'num' of the type parameter 'T'.
}
