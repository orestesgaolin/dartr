
class A {
  const A({required int x });
}
class B extends A {
  const B({required super.x });
}
const a = B();
//        ^
// [diag.missingRequiredArgument] The named parameter 'x' is required, but there's no corresponding argument.
