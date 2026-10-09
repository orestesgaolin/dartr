// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_abstractClass_factory).

void main() async {
  var iter = [1, 2];
  await for (var x in .fromIterable(iter)) {
    print(x);
  }
}
