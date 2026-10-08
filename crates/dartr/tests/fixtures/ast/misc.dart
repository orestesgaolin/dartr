#!/usr/bin/env dart
@A<int>(1) @B.c() @d
library misc;

@native("x")
class N native "Foo" {
  void m() native "bar";
}
class C {
  void m() {
    var x = this.y;
    var f = (a, [b]) {};
    var g = a<b, c>(d);
    var h = a < b;
  }
}
void Function(int)? fn;
List<List<int>>? nested;
Map<String, List<int>>> extra;
var t = x is List<int>> 0;
class Ops {
  operator >>>(o) => 0;
  operator ~() => 0;
  operator ===(o) => 0;
}
