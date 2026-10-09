// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_namedArgument).

void foo({int? a, bool? b}) {}

main() {
  foo(b: false, a: 0);
}
