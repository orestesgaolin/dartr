// Dart source: pkg/analyzer/test/src/dart/resolution/extension_override_test.dart test_setter_noPrefix_typeArguments
class A {}
extension E<T> on A {
  set s(int x) {}
}
void f(A a) {
  E<int>(a).s = 0;
}
