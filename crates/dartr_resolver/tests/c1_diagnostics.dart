import 'dart:async' as async;

int v = 0;
class NotMixin {}
class A extends v {}
class B implements v {}
class C with v {}
class D extends Object? {}
Undefined u1;
v u2;
Map<int> m1 = {};
async.Undefined u3;

void f(Object o) {
  outer: for (;;) {
    void g() {
      break outer;
    }
  }
  l2: {
    for (;;) {
      continue l2;
    }
  }
  break nowhere;
  if (o case (int a, int a)) {}
  if (o case int b || String c) {}
  if (o case int d when (d = 1) > 0) {}
  o is Unknown;
  o as Unknown2;
  List<v> list = [];
  (int _x, {String s, int s}) r = (1, s: '');
  (int, int $1) r2 = (1, 2);
}
