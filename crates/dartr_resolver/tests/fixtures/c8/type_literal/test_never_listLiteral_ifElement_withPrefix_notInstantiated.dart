// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_never_listLiteral_ifElement_withPrefix_notInstantiated).

import 'dart:core' as core;
core.bool b = true;
var l = [if (b) core.Never];
