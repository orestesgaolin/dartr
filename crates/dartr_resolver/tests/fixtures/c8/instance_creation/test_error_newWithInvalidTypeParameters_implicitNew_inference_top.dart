// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_error_newWithInvalidTypeParameters_implicitNew_inference_top).

final foo = Map<int>();
//          ^^^^^^^^
// [diag.wrongNumberOfTypeArguments] The type 'Map' is declared with 2 type parameters, but 1 type arguments were given.
