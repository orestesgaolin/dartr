// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_constructor_unresolved_named).

enum E {
  v.named(42);
//  ^^^^^
// [diag.undefinedEnumConstructorNamed] The enum doesn't have a constructor named 'named'.
  const E(int a);
}
