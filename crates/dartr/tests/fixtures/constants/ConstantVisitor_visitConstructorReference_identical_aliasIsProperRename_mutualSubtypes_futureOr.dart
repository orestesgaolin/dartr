
import 'dart:async';
class C<T extends FutureOr<num>> {}
typedef MyC<T extends num> = C<T>;
const a = identical(MyC<int>.new, MyC<int>.new);
