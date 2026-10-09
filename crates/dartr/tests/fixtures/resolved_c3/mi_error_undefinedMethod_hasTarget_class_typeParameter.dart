// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_undefinedMethod_hasTarget_class_typeParameter).

class C<T> {
  static main() => C.T();
//                   ^
// [diag.undefinedMethod] The method 'T' isn't defined for the type 'C'.
}
