// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_typeAlias_notGeneric_class_generic_named_argumentTypeMismatch).

class A<T> {
  A.named(T t);
}

typedef B = A<String>;

void f() {
  B.named(0);
//        ^
// [diag.argumentTypeNotAssignable] The argument type 'int' can't be assigned to the parameter type 'String'.
}
