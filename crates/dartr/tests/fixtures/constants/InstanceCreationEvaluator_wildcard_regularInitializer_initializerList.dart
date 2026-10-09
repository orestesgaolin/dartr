
class A {
  final int _;
  final int y;
  const A(this._): y = _;
//                     ^
// [diag.invalidConstant] Invalid constant value.
// [diag.implicitThisReferenceInInitializer] The instance member '_' can't be accessed in an initializer.
}
