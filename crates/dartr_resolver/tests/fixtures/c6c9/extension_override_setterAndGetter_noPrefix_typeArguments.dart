// Dart source: pkg/analyzer/test/src/dart/resolution/extension_override_test.dart test_setterAndGetter_noPrefix_typeArguments
class A {}
extension E<T> on A {
  int get s => 0;
  set s(int x) {}
}
void f(A a) {
  E<int>(a).s += 0;
}
