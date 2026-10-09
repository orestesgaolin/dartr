
const dynamic nonBool = 'a';
const c = const {if (nonBool) 3};
//                   ^^^^^^^
// [diag.nonBoolCondition] Conditions must have a static type of 'bool'.
