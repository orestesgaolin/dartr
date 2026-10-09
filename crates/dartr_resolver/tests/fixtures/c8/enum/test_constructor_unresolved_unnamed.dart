// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_constructor_unresolved_unnamed).

enum E {
  v(42);
//^
// [diag.undefinedEnumConstructorUnnamed] The enum doesn't have an unnamed constructor.
  const E.named(int a);
}
