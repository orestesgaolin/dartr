// Top-level inference (unit C10): the types of top-level variables and
// fields that come from their initializers, inferred on demand.

// Literals.
var vInt = 1;
var vDouble = 1.5;
final vString = 'a';
const vBool = true;
var vNull = null;
final vSymbol = #s;

// On demand: `vA` reads `vB` before `vB` is inferred in declaration order.
var vA = vB;
var vB = vC;
final vC = 2;

// Dependency cycles: `dynamic` and `dependencyCycle`.
var cycleA = cycleB;
var cycleB = cycleA;
var selfCycle = selfCycle;

// Function expressions: parameter elements and type parameters of the
// closure are local; the inferred type must not mention them.
final vFunction = (int x) => x;
final vGeneric = <T>(T x) => x;
final vGenericBound = <T extends num>(T x, [String? y]) => x;
final vNamed = ({required int a, String b = ''}) => b;

// Late and typed variables are not inferred from the initializer.
late var vLate = vInt;
int vTyped = 3;

class A {
  var f1 = 1;
  final f2 = vString;
  static var s1 = f3Static;
  static const f3Static = 'x';
  late var f4 = f1;
  var f5;

  // A field formal parameter gets the inferred type of its field.
  A(this.f1, [this.f5]);
}

class B extends A {
  // Override inference reads the inferred type of `A.f1`.
  B(super.f1);
  var f1;
  get f2 => 'b';
}

mixin M {
  final m1 = vDouble;
}

extension E on int {
  static var e1 = vInt;
}

class Cycle {
  static var c1 = c2;
  static var c2 = c1;
}
