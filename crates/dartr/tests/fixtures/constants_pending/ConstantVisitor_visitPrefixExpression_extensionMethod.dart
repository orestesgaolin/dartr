
extension on Object {
  int operator -() => 0;
}

const Object v1 = 1;
const v2 = -v1;
//         ^^^
// [diag.constEvalExtensionMethod] Extension methods can't be used in constant expressions.
