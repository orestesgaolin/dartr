// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_untypedMap).
var a = 1;
var t = {
    (a = 1) :
        (a = 2),
};
