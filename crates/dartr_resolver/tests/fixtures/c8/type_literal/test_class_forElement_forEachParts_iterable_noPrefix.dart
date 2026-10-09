// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_forElement_forEachParts_iterable_noPrefix).

class C {}
var v = [for (var e in C) e];
//                     ^
// [diag.forInOfInvalidType] The type 'Type' used in the 'for' loop must implement 'Iterable'.
