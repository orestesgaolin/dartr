// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_typeParameter_promotedToNonNullable).

void f<T>(T? t) {
  if (t is int) {
    t.abs();
  }
}
