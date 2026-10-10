import 'src/other.dart';

class Point {
  int x = 0;
  int y;
  int? _cache = null;
  Point(this.y) {}

  int sum() {
    return this.x + this.y;
  }

  String toString() => "Point(${x})";

  String get label => '${x}';

  void update(List<int> items) {
    var count = 0;
    print(count);
    if (items.length == 0) print('empty');
    final m = new Map<int, int>();
    print(m);
    if (_cache == null) {
      _cache = 1;
    }
  }
}

class Derived extends Base {
  Derived(int a) : super(a);

  @override
  void run() {
    super.run();
  }
}

void consts() {
  final o = Other(1);
  print(o);
  final list = const [Other(2)];
  print(list);
  final s = "double";
  print(s);
  final x = (1 + 2);
  print(x);
}

f() {}
