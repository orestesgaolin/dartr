// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_functionExpression).
import 'dart:async';
var vFuture = new Future<int>(42);
var v_noParameters_inferredReturnType = () => 42;
var v_hasParameter_withType_inferredReturnType = (String a) => 42;
var v_hasParameter_withType_returnParameter = (String a) => a;
var v_async_returnValue = () async => 42;
var v_async_returnFuture = () async => vFuture;
