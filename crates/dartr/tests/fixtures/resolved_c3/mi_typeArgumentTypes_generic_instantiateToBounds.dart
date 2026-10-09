// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_typeArgumentTypes_generic_instantiateToBounds).

void foo<T extends num>() {}

main() {
  foo();
}
