
const dynamic nonBool = null;
const c = const {if (nonBool) 'a' : 1};
//                   ^^^^^^^
// [diag.nonBoolCondition] Conditions must have a static type of 'bool'.
