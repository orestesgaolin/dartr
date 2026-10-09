// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_error_wrongNumberOfTypeArgumentsConstructor_explicitNew).

class Foo<X> {
  Foo.bar();
}

main() {
  new Foo.bar<int>();
//           ^^^^^
// [diag.wrongNumberOfTypeArgumentsConstructor] The constructor 'Foo.bar' doesn't have type parameters.
}
