
void f<T>(T t) => t;

class C<T> {
  final void Function(T) p;
  const C({this.p = f});
//                  ^
// [diag.constWithTypeParametersFunctionTearoff] A constant function tearoff can't use a type parameter as a type argument.
}
