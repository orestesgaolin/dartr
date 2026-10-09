// Ported from pkg/analyzer/test/src/dart/resolution/record_literal_test.dart
// (context types, named and positional fields, implicit casts of
// `dynamic`).
void f(int a, String s, dynamic d) {
  var r1 = (1, 2);
  var r2 = (a: 1, b: 's');
  var r3 = (1, b: s, 2.0);
  (num, String) r4 = (1, 's');
  ({num a, Object b}) r5 = (a: 1, b: 2);
  (int, String) r6 = (d, d);
  ({int x}) r7 = (x: d);
  (num, {int y}) r8 = (1, z: 2);
  (int,) r9 = (d,);
  Object r10 = (d, 1);
  var r11 = (a, (s, [1]));
  (List<num>, Set<int>) r12 = ([], {});
  var r13 = const (1, x: 'a');
  (int, int)? r14 = (1, 2);
}
