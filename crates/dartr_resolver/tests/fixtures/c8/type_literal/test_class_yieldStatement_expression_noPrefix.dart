// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_yieldStatement_expression_noPrefix).

import 'dart:async';
class C {}
Stream<Type> f() async* {
  yield C;
}
