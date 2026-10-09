
class C<T> {}
typedef MyC<T extends Object?> = C<T>;
const a = identical(MyC<int>.new, MyC<int>.new);
