
extension E on int {}

void f(int a) {
  E(a) == 0;
//     ^^
// [diag.undefinedExtensionOperator] The operator '==' isn't defined for the extension 'E'.
}
