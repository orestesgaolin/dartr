
dynamic y = 2;
class A {
  const A();
//^^^^^
// [diag.constConstructorWithFieldInitializedByNonConst] Can't define the 'const' constructor because the field 'x' is initialized with a non-constant value.
  final x = y as num;
}
