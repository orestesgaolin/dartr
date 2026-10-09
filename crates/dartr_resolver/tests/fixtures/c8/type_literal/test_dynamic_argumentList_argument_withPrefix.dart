// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_dynamic_argumentList_argument_withPrefix).

import 'dart:core' as core;
void f(core.Type t) {}
void g() {
  f(core.dynamic);
}
