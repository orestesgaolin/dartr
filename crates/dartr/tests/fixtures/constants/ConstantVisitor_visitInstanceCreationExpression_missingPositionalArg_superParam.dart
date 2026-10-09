
class A {
  const A(int x);
}
class B extends A {
  const B(super.x);
}
const a = B();
//          ^
// [diag.notEnoughPositionalArgumentsNameSingular] 1 positional argument expected by 'B.new', but 0 found.
