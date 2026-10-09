// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_never_setLiteral_ifElement_else_withPrefix_notInstantiated).

import 'dart:core' as core;
core.Set<core.Object> f(core.bool b) {
  return {if (b) core.int else core.Never};
}
