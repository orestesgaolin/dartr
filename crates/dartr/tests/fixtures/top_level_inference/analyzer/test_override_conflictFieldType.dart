// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_override_conflictFieldType).
abstract class A {
  int aaa = 0;
//    ^^^
// [context 1] The member being overridden.
}
abstract class B {
  String aaa = '0';
//       ^^^
// [context 2] The member being overridden.
}
class C implements A, B {
  var aaa;
//    ^^^
// [diag.invalidOverride][context 1] 'C.aaa' ('dynamic Function()') isn't a valid override of 'A.aaa' ('int Function()').
// [diag.invalidOverride][context 2] 'C.aaa' ('dynamic Function()') isn't a valid override of 'B.aaa' ('String Function()').
}
