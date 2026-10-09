
class C<T extends num> {}
typedef TC<T extends num> = C<T>;
const c = identical(C<num>, TC);
