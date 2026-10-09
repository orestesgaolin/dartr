// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_never_conditionalExpression_elseExpression_withPrefix_notInstantiated).

import 'dart:core' as core;
core.bool b = true;
var y = b ? core.int : core.Never;
