
class C<T, U> {}
typedef MyC<T> = C;
const a = identical(MyC.new, C.new);
