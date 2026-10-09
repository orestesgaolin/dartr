// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_constructorReference_prefixed).

import 'dart:async' as a;
var x = a.Future.delayed<int>;
//                      ^^^^^
// [diag.wrongNumberOfTypeArgumentsConstructor] The constructor 'a.Future.delayed' doesn't have type parameters.
