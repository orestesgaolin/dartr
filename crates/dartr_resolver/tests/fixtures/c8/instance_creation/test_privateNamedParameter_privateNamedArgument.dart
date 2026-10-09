// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_privateNamedParameter_privateNamedArgument).

class C {
  int? _x;
//     ^^
// [diag.unusedField] The value of the field '_x' isn't used.
  C({this._x});
}

main() {
  C(_x: 123);
//  ^^
// [diag.useOfPrivateParameterName] The named parameter '_x' should use the corresponding public name 'x' at the callsite.
}
