
void foo<T>(T a) {}
const g = foo<int, String>;
//           ^^^^^^^^^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The function 'foo' is declared with 1 type parameters, but 2 type arguments are given.
