import 'package:fixes_project/src/helpers.dart';
import 'src/other.dart';

const big = 1_000_000;
const ratio = 1_0.5;

class Base2 {
  final int a;
  Base2(this.a);
}

class Derived extends Base2 {
  late int value;
  Derived(int a) : super(a);

  int twice() {
    return value * 2;
  }

  void show() => print(value);

  bool smaller(int other) => value < other;

  String get title {
    return 'Derived $value';
  }

  void work();
}

abstract class Shape {
  double area();
}

mixin Marker;

int compute(int x, int y) {
  var total = x + y;
  var name = 'name', other = 'other';
  int count = 0;
  final fixed = x * y;
  count += total;
  if (x >= y && y > 0) {
    print("$name $other $fixed");
  }
  return count;
}

void callbacks(List<int> items) {
  items.forEach((item) {
    print(item);
  });
  items.map((item) => item + 1);
}

dynamic helper() {
  return helperConstant + const Other(1).x;
}
