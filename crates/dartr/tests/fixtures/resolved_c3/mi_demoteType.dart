// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_demoteType).

void test<T>(T t) {}

void f<S>(S s) {
  if (s is int) {
    test(s);
  }
}

