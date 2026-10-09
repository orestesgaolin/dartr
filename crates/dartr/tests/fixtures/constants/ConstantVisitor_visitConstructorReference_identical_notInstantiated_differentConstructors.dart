
class C<T> {
  C();
  C.named();
}
const a = identical(C.new, C.named);
