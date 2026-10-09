// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_unqualifiedReferenceToNonLocalStaticMember_method).

class A {
  static void foo() {}
}

class B extends A {
  main() {
    foo(0);
//  ^^^
// [diag.unqualifiedReferenceToNonLocalStaticMember] Static members from supertypes must be qualified by the name of the defining type.
//      ^
// [diag.extraPositionalArguments] Too many positional arguments: 0 expected, but 1 found.
  }
}
