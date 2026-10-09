// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_inference_listLiteral).

enum E1 {a, b}
enum E2 {a, b}

var v = [E1.a, E2.b];
