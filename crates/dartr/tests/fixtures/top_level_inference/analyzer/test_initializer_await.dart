// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_await).
import 'dart:async';
int fValue() => 42;
Future<int> fFuture() async => 42;
var uValue = () async => await fValue();
var uFuture = () async => await fFuture();
