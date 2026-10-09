// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_interfaceType_switchExpression).

Object f(Object? x) {
  return switch (x) {
    _ => 0,
  }.toString();
}
