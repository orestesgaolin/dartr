
const dynamic c = 2;
const x = [1, if (c) 2 else 3, 4];
//                ^
// [diag.nonBoolCondition] Conditions must have a static type of 'bool'.
