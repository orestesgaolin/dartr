
void foo<T, U>(T a, U b) {}
const g = foo<int>;
//           ^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The function 'foo' is declared with 2 type parameters, but 1 type arguments are given.
