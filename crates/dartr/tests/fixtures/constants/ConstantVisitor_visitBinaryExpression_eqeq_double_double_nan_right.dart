
const c = 2.3 == double.nan;
//            ^^^^^^^^^^^^^
// [diag.unnecessaryNanComparisonFalse] A double can't equal 'double.nan', so the condition is always 'false'.
