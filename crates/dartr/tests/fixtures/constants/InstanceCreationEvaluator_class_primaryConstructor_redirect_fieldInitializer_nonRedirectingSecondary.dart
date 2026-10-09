
class const A(int x) {
  final int y = x + 1;
  const A.named(int z) : this(z * 2);
  const A.other() {}
//      ^^^^^^^
// [diag.nonRedirectingGenerativeConstructorWithPrimary] Classes with primary constructors can't have non-redirecting generative constructors.
//                ^
// [diag.constConstructorWithBody] Const constructors can't have a body.
}
const a = A.named(10);
