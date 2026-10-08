// Override inference across libraries of a cycle, private names.

import 'library_b.dart';

class A {
  int _private(int a) => a;
  String public(String s) => s;
  num get value => 0;
}

class FromB extends B {
  _private(a) => a;
  public(s) => s;
  get value => 0;
}
