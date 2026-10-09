
enum const E(int x) {
//   ^^^^^
// [diag.constConstructorWithFieldInitializedByNonConst] Can't define the 'const' constructor because the field 'foo' is initialized with a non-constant value.
  v(0);

  final int Function() foo = () => x;
}
