// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_never_binaryExpression_rightOperand_ifNull_withPrefix_notInstantiated).

import 'dart:core' as core;
core.Object? x;
var y = x ?? core.Never;
