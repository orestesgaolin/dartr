// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_never_argumentList_argument_withPrefix_notInstantiated_parenthesized).

import 'dart:core' as core;
void f(core.Type t) {}
void g() {
  f((core.Never));
}
