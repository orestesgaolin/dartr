// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_never_listLiteral_spread_withPrefix_notInstantiated).

import 'dart:core' as core;
var l = [...core.Never];
//          ^^^^^^^^^^
// [diag.notIterableSpread] Spread elements in list or set literals must implement 'Iterable'.
