// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_constant_augmentation_add).

enum A {
  v1
}

augment enum A {
  v2
}

void f() {
  A.v2;
}
