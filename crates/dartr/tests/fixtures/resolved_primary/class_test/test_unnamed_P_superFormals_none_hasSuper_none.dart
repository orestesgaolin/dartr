class A {
  A(int p1);
}
class B() extends A {
  this : super();
//             ^
// [diag.notEnoughPositionalArgumentsNameSingular] 1 positional argument expected by 'A.new', but 0 found.
}
