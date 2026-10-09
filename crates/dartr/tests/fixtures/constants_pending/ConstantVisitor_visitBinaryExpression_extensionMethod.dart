
extension on Object {
  int operator +(Object other) => 0;
}

const Object v1 = 0;
const v2 = v1 + v1;
//         ^^^^^^^
// [diag.constEvalExtensionMethod] Extension methods can't be used in constant expressions.
