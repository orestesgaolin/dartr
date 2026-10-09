
class C<T> {
  C();
  C.named();
}
const a = identical(C<int>.new, C<int>.named);
