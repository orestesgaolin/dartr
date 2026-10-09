
final x = 1;
class const A(int p) {
//    ^^^^^
// [diag.constConstructorWithFieldInitializedByNonConst] Can't define the 'const' constructor because the field 'y' is initialized with a non-constant value.
  final int y = x + p;
}
