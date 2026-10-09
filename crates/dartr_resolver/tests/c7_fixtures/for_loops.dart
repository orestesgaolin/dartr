// Ported from pkg/analyzer/test/src/dart/resolution/for_statement_test.dart
// and for_element_test.dart (for-in with a declared variable, an
// identifier and a pattern, await for, C-style for loops).
import 'dart:async';

class C {
  int field = 0;
  set setter(String value) {}

  void m(List<int> li, Stream<String> ss) async {
    for (field in li) {
      field;
    }
    for (setter in ['a']) {}
    await for (var x1 in ss) {
      x1;
    }
  }
}

int top = 0;

void f(List<int> li, Iterable<String> is_, Map<int, String> m, dynamic d,
    Stream<double> sd, Object o, List<(int, String)> lr) async {
  for (var x2 in li) {
    x2;
  }
  for (final x3 in is_) {
    x3;
  }
  for (num x4 in li) {
    x4;
  }
  for (var x5 in d) {
    x5;
  }
  for (var e6 in m.entries) {
    e6.key;
    e6.value;
  }
  for (var x7 in []) {
    x7;
  }
  await for (final x8 in sd) {
    x8;
  }
  int? y;
  for (y in li) {
    y;
  }
  for (top in li) {
    top;
  }
  for (var i = 0, j = 1.0; i < 10; i++, j++) {
    i;
    j;
  }
  int k;
  for (k = 0; k < 3; k++) {
    k;
  }
  for (;;) {
    break;
  }
  for (var (a1, b1) in lr) {
    a1;
    b1;
  }
  for (final (int a2, b: b2) in [(1, b: 's')]) {
    a2;
    b2;
  }
  for (var (i3, j3) = (0, 1); i3 < j3; i3++) {
    i3;
    j3;
  }
  int? z = 1;
  for (; z != null;) {
    z;
    z = null;
  }
  for (var x9 in o as List<int>) {
    x9;
  }
  final list = [for (var x in li) for (var y in is_) (x, y)];
  final set = {for (var x in li) x};
  final map = {for (var x in li) x: '$x'};
  final nested = [
    for (var x in li)
      if (x > 0) x else -x,
  ];
  await for (var (a4, b4) in Stream.fromIterable(lr)) {
    a4;
    b4;
  }
  for (var x10 in li.where((e) => e > 0)) {
    x10;
  }
}
