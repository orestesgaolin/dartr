// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_rewrite_without_target).

extension E<T extends Function> on List<T> {
  test() {
    first();
  }
}
