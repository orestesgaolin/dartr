// Diagnostics of the error verifiers (branch integ-verifiers, not merged):
// dartr must report a subset of the dart diagnostics, and every missing one
// must have a code of PENDING_VERIFIER_CODES in analyze_parity.rs.
const b = 'x' + 1;

class C {
  void m(int x, {required String name}) {}
}

abstract class A {
  void f();
}

class B extends A {}

int noReturn() {}

void main() {
  int x = 'hello';
  var unused = 1;
  C().m();
  C().m(1, name: 2);
  undefinedName;
  final y = 1;
  y = 2;
}
