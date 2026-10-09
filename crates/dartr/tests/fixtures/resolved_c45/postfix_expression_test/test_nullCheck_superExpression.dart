
class A {
  int foo() => 0;
}

class B extends A {
  void bar() {
    super!.foo();
//  ^^^^^^
// [diag.missingAssignableSelector] Missing selector such as '.identifier' or '[0]'.
  }
}
