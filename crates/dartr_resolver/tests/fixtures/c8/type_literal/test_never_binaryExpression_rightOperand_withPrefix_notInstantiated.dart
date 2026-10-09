// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_never_binaryExpression_rightOperand_withPrefix_notInstantiated).

import 'dart:core' as core;
void f() {
  core.int == core.Never;
}
