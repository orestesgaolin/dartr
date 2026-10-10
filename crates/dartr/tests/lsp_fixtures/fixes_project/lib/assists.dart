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

class Holder {
  void Function(List<String> paths)? callback;
  int Function(int a, {int? b})? other;

  void use(Object o, Map<String, int> map) {
    var copy = callback;
    var second = other;
    print([copy, second]);
    if (o case String(:int length)) {
      print(length);
    }
    for (var MapEntry(:key, :value) in map.entries) {
      print([key, value]);
    }
    if (o case String(length: int size)) {
      print(size);
    }
    final Map<dynamic, Object?> loose = {};
    for (var MapEntry(:key, :value) in loose.entries) {
      print([key, value]);
    }
    if (loose['a'] case String(:int length)) {
      print(length);
    }
  }
}

HelperKind dotShorthand(bool flag) => flag ? .a : HelperKind.b;

class PrimaryDecl(int x) {
  int get doubled => x * 2;
}

class InitFormal {
  final int x;
  InitFormal(int a) : x = a;
}

class MoveInit {
  final int x;
  MoveInit() : x = 1;
}

int switchConversions(HelperKind k) {
  if (k == HelperKind.a) {
    return 1;
  } else if (k == HelperKind.b) {
    return 2;
  }
  switch (k) {
    case HelperKind.a:
      return 10;
    case HelperKind.b:
      return 20;
  }
}

int switchExprToStmt(HelperKind k) {
  return switch (k) {
    HelperKind.a => 1,
    HelperKind.b => 2,
  };
}

void ifAssists(bool a, bool b, Object? x) {
  if (a) {
    print(1);
  } else {
    if (b) {
      print(2);
    }
  }
  if (a) {
    if (b) {
      print(3);
    }
  }
  if (a) {
    print(4);
  }
  if (!a) {
    print(5);
  }
  final v = x;
  if (v is int) {
    print(v);
  }
}

int replaceIfElseCond(bool b) {
  if (b) {
    return 1;
  } else {
    return 2;
  }
}

const unseparated = 1000000;

List<int> collectionAssists(List<int> more, bool b) => [
  b ? 1 : 2,
]..addAll(more);

Map<int, int> mapForLiteral(List<int> xs) =>
    Map.fromIterable(xs, key: (e) => e, value: (e) => e * 2);

bool exprAssists(Object x, String s) => !(x is String) && !s.isEmpty;

int? nullAwareAssist(String? s) => s == null ? null : s.length;

int joinVarAssist(int a) {
  int b;
  b = a + 1;
  b += 1;
  return b;
}

Future<int> removeAsyncAssist() async {
  return 1;
}

class ShadowCheck {
  Object? field;

  void check() {
    if (field is String) {
      print(field);
    }
  }
}

