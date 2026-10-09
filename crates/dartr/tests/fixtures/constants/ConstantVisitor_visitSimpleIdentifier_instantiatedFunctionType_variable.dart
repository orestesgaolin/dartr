
void f<T>(T a, {T? b}) {}

const void Function<T>(T a) g = f;

const void Function(int a) h = g;
