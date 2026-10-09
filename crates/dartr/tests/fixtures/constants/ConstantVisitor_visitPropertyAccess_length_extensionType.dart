
extension type const A(String it) {
  int get length => 0;
}

const v1 = A('');
const v2 = v1.length;
//         ^^^^^^^^^
// [diag.constEvalExtensionTypeMethod] Extension type methods can't be used in constant expressions.
