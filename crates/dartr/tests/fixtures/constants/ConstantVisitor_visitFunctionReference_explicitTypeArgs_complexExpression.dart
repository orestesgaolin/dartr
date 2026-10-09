
const b = true;
void foo<T>(T a) {}
void bar<T>(T a) {}
const g = (b ? foo : bar)<int>;
