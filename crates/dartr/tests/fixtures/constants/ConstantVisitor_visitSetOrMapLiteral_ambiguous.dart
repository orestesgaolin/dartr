
const l = [];
const ambiguous = {...l, 1: 2};
//                ^^^^^^^^^^^^
// [diag.ambiguousSetOrMapLiteralBoth] The literal can't be either a map or a set because it contains at least one literal map entry or a spread operator spreading a 'Map', and at least one element which is neither of these.
