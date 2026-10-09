
class C<T, U> {}
typedef MyC<T, U> = C<U, T>;
const a = identical(MyC.new, C.new);
