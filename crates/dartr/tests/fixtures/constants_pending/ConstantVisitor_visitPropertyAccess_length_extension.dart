
extension ExtObject on Object {
  int get length => 4;
}

class B {
  final l;
  const B(Object o) : l = o.length;
//                        ^^^^^^^^
// [context 1] The error is in the field initializer of 'B', and occurs here.
}

const b = B('');
//        ^^^^^
// [diag.constEvalExtensionMethod][context 1] Extension methods can't be used in constant expressions.
