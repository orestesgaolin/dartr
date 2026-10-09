// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_error_wrongNumberOfTypeArgumentsConstructor_explicitNew_new).

class Foo<X> {
  Foo.new();
}

main() {
  new Foo.new<int>();
//           ^^^^^
// [diag.wrongNumberOfTypeArgumentsConstructor] The constructor 'Foo.new' doesn't have type parameters.
}
