
int x = 1;
class const A() {
  this : assert(x > 0);
//              ^
// [diag.invalidConstant] Invalid constant value.
}
