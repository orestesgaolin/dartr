// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_privateNamedParameter_publicNamedArgument).

class C {
  int? _x;
//     ^^
// [diag.unusedField] The value of the field '_x' isn't used.
  C({this._x});
}

main() {
  C(x: 123);
}
