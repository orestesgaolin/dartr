
class C<T> {}
const C<int> Function() c1 = C.new;
const c2 = C<int>.new;
const a = identical(c1, c2);
