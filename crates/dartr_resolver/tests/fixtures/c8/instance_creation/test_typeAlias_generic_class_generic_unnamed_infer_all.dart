// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_typeAlias_generic_class_generic_unnamed_infer_all).

class A<T> {
  A(T t);
}

typedef B<U> = A<U>;

void f() {
  B(0);
}
