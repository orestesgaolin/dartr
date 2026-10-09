
int x = 1;
class const A() {
  final int f;
  this : f = x;
//           ^
// [diag.invalidConstant] Invalid constant value.
}
