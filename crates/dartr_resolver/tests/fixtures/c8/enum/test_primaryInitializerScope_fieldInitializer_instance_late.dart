// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_primaryInitializerScope_fieldInitializer_instance_late).

enum A(int foo) {
//   ^
// [diag.constConstructorWithFieldInitializedByNonConst] Can't define the 'const' constructor because the field 'bar' is initialized with a non-constant value.
  v(0);
  late final bar = foo;
//^^^^
// [diag.lateFinalFieldWithConstConstructor] Can't have a late final field in a class with a generative const constructor.
//                 ^^^
// [diag.undefinedIdentifier] Undefined name 'foo'.
}
