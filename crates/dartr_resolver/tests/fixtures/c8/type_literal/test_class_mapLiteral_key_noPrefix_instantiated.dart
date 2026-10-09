// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_mapLiteral_key_noPrefix_instantiated).

class C<T> {}
var m = {C<int>: 1};
