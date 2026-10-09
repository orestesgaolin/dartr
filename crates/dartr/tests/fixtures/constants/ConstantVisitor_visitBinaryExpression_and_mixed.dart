
const c = 3 & false;
//        ^^^^^^^^^
// [diag.constEvalTypeBoolInt] In constant expressions, operands of this operator must be of type 'bool' or 'int'.
//            ^^^^^
// [diag.argumentTypeNotAssignable] The argument type 'bool' can't be assigned to the parameter type 'int'.
