
class C<T> {}
typedef MyC<T> = C<T>;
const a = identical(MyC<int>.new, MyC<int>.new);
