
class C<T, U> {}
typedef MyC<T> = C<T, int>;
const a = identical(MyC.new, C.new);
