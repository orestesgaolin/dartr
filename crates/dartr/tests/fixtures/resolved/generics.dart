// Members of generic classes (substituted members), constructor names,
// import prefixes and generic method invocations.
import 'dart:math' as m;

part 'generics_part.dart';

class A<T> {
  T x;
  A(this.x);
  A.named(T value) : this(value);
  T get y => x;
  S foo<S>(S s) => s;
}

void main() {
  var a = A<int>(1);
  var b = a.x + a.y;
  var c = A<String>.named('c');
  var l = List<int>.filled(3, 0);
  print(m.max(b, l.length) + a.foo<String>('x').length + c.x.length);
  print(B<double>(1.0).value);
}
