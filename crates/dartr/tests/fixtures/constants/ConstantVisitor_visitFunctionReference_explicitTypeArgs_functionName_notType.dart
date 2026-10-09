
void foo<T>(T a) {}
const g = foo<true>;
//        ^^^^^^^^
// [diag.constEvalTypeNum] In constant expressions, operands of this operator must be of type 'num'.
//           ^
// [diag.undefinedOperator] The operator '<' isn't defined for the type 'void Function<T>(T)'.
//                ^
// [diag.equalityCannotBeEqualityOperand] A comparison expression can't be an operand of another comparison expression.
//                 ^
// [diag.missingIdentifier] Expected an identifier.
