
class const A(int x) {
//    ^^^^^
// [diag.constConstructorWithFieldInitializedByNonConst] Can't define the 'const' constructor because the field 'foo' is initialized with a non-constant value.
  final int Function() foo = () => x;
}
