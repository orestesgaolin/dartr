
class A {
  final int x = 0;
  final int f = x;
//              ^
// [diag.invalidConstant] Invalid constant value.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
// [diag.implicitThisReferenceInInitializer] The instance member 'x' can't be accessed in an initializer.
  const A();
//^^^^^
// [diag.constConstructorWithFieldInitializedByNonConst] Can't define the 'const' constructor because the field 'f' is initialized with a non-constant value.
}
const a = A();
