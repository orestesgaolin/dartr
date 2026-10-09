// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_undefinedGetter_message).

int f() => .foo;
//          ^^^
// [diag.dotShorthandUndefinedGetter] The static getter 'foo' isn't defined for the context type 'int'.
