
class A {
  final int x;
  const A(this.x) : x = 1;
//                  ^
// [diag.fieldInitializedInParameterAndInitializer] Fields can't be initialized in both the parameter list and the initializers.
}

const a = A(2);
