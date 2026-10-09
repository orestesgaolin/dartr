
void f<T>(T p, {T? q}) {}

const void Function<T>(T p) g = f;

const bool b = false;
const void Function(int p) h = b ? g : g;
