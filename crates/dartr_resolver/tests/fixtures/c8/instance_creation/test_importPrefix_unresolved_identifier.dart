// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_importPrefix_unresolved_identifier).

import 'dart:math' as prefix;

void f() {
  new prefix.Foo.bar(0);
//           ^^^
// [diag.newWithNonType] The name 'Foo' isn't a class.
}

