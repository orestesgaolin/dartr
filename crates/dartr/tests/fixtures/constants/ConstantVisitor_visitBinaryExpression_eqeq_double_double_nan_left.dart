
const c = double.nan == 2.3;
//        ^^^^^^^^^^^^^
// [diag.unnecessaryNanComparisonFalse] A double can't equal 'double.nan', so the condition is always 'false'.
