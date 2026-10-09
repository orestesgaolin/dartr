
void foo<T>(T a) {}
const void Function(int) f = foo;
const void Function(int) g = foo;
const c = identical(f, g);
