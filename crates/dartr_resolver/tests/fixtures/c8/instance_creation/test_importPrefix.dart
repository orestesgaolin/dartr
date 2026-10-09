// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_importPrefix).

import 'dart:math' as prefix;

void f() {
  new prefix(0);
//    ^^^^^^
// [diag.newWithNonType] The name 'prefix' isn't a class.
}

