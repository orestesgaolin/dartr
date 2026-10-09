
final x = 1;
class const A() {
//    ^^^^^
// [diag.constConstructorWithFieldInitializedByNonConst] Can't define the 'const' constructor because the field 'y' is initialized with a non-constant value.
  final int y = x + 2;
//              ^
// [diag.invalidConstant] Invalid constant value.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
}
const a = A();
