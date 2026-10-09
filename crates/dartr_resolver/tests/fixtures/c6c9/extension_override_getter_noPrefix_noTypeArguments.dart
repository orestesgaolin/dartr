// Dart source: pkg/analyzer/test/src/dart/resolution/extension_override_test.dart test_getter_noPrefix_noTypeArguments
class A {}
extension E on A {
  int get g => 0;
}
void f(A a) {
  E(a).g;
}
