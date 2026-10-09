
class C<T> {}
typedef MyC = C<int>;
const a = identical(MyC.new, C<int>.new);
