class A {
  A({int? n1, required int n2});
}
class B({super.n1, required super.n2}) extends A;
