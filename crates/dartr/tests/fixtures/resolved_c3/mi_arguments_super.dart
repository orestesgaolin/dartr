// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_arguments_super).

class A {
  void f() {
    g(super);
//    ^^^^^
// [diag.missingAssignableSelector] Missing selector such as '.identifier' or '[0]'.
  }
}

void g(Object a) {}
