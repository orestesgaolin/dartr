
void f<T>(Object? x) {
  if (x case const (List<T>)) {}
//                  ^^^^^^^
// [diag.constTypeParameter] Type parameters can't be used in a constant expression.
}
