
import 'dart:async';
void foo<T>(T a) {}
const g = identical(foo<Object>, foo<FutureOr<Object>>);
