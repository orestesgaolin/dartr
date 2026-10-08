// @dart = 2.9
// Features after 2.9 are reported by the AST builder.
int? x;
int y = 1 >>> 2;
var r = (1, 2);
void f(int a, {int b}) => g(b: 1, 2);
typedef T = int;
extension type E(int i) {}
sealed class S {}
base mixin M {}
enum E3 { a(1); const E3(this.v); final int v; }
var t = List<int>.filled;
var s = A<int>;
class C { C(super.x); }
var l = [?null];
var n = 1_000;
library;
