// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_constant_augmentation_valuesGetter_recovery).

enum A {
  v1
}

augment enum A {;
  static int get values => 0;
//               ^^^^^^
// [diag.valuesDeclarationInEnum] A member named 'values' can't be declared in an enum.
}

augment enum A {
  v2
}

void f() {
  A.values;
}
