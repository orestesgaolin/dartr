
class C<T> {}
typedef MyC<T> = C<T>;
const a = identical(MyC.new, MyC.new);
