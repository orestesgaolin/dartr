
class B {
  final l;
  const B(String o) : l = o.length;
//                        ^^^^^^^^
// [context 1] The error is in the field initializer of 'B', and occurs here.
}

const y = B(x);
//        ^^^^
// [diag.constEvalTypeString][context 1] In constant expressions, operands of this operator must be of type 'String'.
//          ^
// [diag.undefinedIdentifier] Undefined name 'x'.
// [diag.constWithNonConstantArgument] Arguments of a constant creation must be constant expressions.
