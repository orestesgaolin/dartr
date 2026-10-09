
const b = true;
void foo<T>(String a, T b) {}
void bar<T>(T a, String b) {}
const g = (b ? foo : bar)<int>;
