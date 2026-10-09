
abstract class A {
   void b< extends int>();
//         ^^^^^^^
// [diag.missingIdentifier] Expected an identifier.
}
void f(A a) {
  a.b[0] = 0;
//   ^^^
// [diag.undefinedOperator] The operator '[]=' isn't defined for the type 'void Function< extends int>()'.
}
