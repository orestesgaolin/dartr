
enum E {
  v(1);
  final int x;
  const E(this.x) : x = 2;
//                  ^
// [diag.fieldInitializedInParameterAndInitializer] Fields can't be initialized in both the parameter list and the initializers.
}

const a = E.v;
