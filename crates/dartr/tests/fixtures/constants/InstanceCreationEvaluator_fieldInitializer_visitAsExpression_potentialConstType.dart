
const num three = 3;

class C<T extends num> {
  final T w;
  const C() : w = three as T;
}

void main() {
  const C<int>().w;
}
