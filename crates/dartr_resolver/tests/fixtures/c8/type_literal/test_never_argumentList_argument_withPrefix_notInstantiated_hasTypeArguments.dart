// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_never_argumentList_argument_withPrefix_notInstantiated_hasTypeArguments).

import 'dart:core' as core;
void f(core.Object? x) {}
void g() {
  f(core.Never<core.int>);
}
