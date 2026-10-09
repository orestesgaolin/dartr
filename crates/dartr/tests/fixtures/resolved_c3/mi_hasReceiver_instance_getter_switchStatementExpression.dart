// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_instance_getter_switchStatementExpression).

class C {
  int Function() get foo => throw Error();
}

void f(C c) {
  switch ( c.foo() ) {
    default:
      break;
  }
}
