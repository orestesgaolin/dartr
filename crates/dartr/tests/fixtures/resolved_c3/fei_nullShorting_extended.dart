// Ported from pkg/analyzer/test/src/dart/resolution/function_expression_invocation_test.dart (test_nullShorting_extended).

abstract class A {
  int Function() f();
}
test(A? a) => a?.f()();
