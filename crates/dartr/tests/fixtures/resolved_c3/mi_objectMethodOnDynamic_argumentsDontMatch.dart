// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_objectMethodOnDynamic_argumentsDontMatch).

void f(a, int b) {
  a.toString(b);
}
