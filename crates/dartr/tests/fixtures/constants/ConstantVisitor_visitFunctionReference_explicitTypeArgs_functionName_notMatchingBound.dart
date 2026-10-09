
void f<T extends num>(T a) {}
const g = f<String>;
//          ^^^^^^
// [diag.typeArgumentNotMatchingBounds] 'String' doesn't conform to the bound 'num' of the type parameter 'T'.
