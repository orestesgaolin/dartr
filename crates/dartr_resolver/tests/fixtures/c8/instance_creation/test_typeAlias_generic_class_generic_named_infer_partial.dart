// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_typeAlias_generic_class_generic_named_infer_partial).

class A<T, U> {
  A.named(T t, U u);
}

typedef B<V> = A<V, String>;

void f() {
  B.named(0, '');
}
