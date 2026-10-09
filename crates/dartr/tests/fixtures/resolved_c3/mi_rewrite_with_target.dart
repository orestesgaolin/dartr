// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_rewrite_with_target).

test<T extends Function>(List<T> x) {
  x.first();
}
