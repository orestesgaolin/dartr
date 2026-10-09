
class C<T> {}
typedef TC<T> = C<T>;
const c = identical(C<dynamic>, TC);
