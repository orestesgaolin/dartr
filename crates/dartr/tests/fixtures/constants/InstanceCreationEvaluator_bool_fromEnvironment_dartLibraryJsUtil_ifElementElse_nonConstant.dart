
const a = bool.fromEnvironment('dart.library.js_util');
var b = 7;
var x = const A([if (a) 3 else b]);
//                             ^
// [diag.nonConstantListElement] The values in a const list literal must be constants.

class A {
  const A(List<int> p);
}
