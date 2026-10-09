
class C<T> {}
typedef TC<T> = C<T>;
const c = identical(C<int>, TC<int>);
