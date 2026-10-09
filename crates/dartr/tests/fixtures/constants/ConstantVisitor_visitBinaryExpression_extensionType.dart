
extension type const A(int it) {
  int operator +(Object other) => 0;
}

const v1 = A(1);
const v2 = v1 + 2;
//         ^^^^^^
// [diag.constEvalExtensionTypeMethod] Extension type methods can't be used in constant expressions.
