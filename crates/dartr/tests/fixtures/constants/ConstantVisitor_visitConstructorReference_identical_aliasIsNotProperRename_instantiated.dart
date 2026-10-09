
class C<T> {}
typedef MyC<T extends num> = C<T>;
const a = identical(MyC<int>.new, C<int>.new);
