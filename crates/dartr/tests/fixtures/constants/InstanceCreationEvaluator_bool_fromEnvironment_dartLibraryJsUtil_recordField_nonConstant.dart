
const a = bool.fromEnvironment('dart.library.js_util');
var b = 7;
var x = const A((b, ));
//               ^
// [diag.invalidConstant] Invalid constant value.

class A {
  const A((int, ) p);
}
