// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_methodInvocation_function).
int f1() => 0;
T f2<T>() => throw 0;
var t1 = f1();
var t2 = f2();
var t3 = f2<int>();
