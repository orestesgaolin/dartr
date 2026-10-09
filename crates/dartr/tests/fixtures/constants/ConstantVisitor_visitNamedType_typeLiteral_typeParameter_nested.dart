
void f<T>(Object? x) {
  if (x case const (T)) {}
//                  ^
// [diag.constTypeParameter] Type parameters can't be used in a constant expression.
}
