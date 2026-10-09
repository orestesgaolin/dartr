
class C {}

extension Ext on C {
  int operator +(int _) {
    return 0;
  }
}

void f(C c) {
  ++Ext(c);
//       ^
// [diag.missingAssignableSelector] Missing selector such as '.identifier' or '[0]'.
}
