
void foo<T>(T a) {}
void bar<T>(T a) {}
const g = identical(foo<int>, bar<int>);
