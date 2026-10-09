import 'dart:math' as math;
import 'other.dart';

// Constant evaluation.
const int a = 1 ~/ 0;
const List<int> list = [1, 2];
const e = list[5];

class C {
  final int f;
  const C(this.f);
  void m(int x, {String? name}) {}
}

void main() {
  // Resolution.
  undefinedTop();
  var x = UnknownType();
  C(1).nothing;
  C(1).m(1, 2);
  math.undefinedFn();
  // ignore: undefined_identifier
  ignoredName;
  print(x);
  print(Other().missing);
  const C(1 ~/ 0);
  String s = '';
  s.lengthh;
  s.substring(0).nope();
}
