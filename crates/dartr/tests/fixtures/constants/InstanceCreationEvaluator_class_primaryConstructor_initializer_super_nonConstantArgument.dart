
int x = 1;
class A {
  const A(int a);
}
class const B() extends A {
  this : super(x);
//             ^
// [diag.invalidConstant] Invalid constant value.
}
