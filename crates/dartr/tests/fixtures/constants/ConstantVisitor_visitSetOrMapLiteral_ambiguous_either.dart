
const int? i = 1;
const res  = {...?i};
//           ^^^^^^^
// [diag.ambiguousSetOrMapLiteralEither] This literal must be either a map or a set, but the elements don't have enough information for type inference to work.
