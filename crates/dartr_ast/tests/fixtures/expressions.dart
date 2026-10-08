import 'dart:async';

class C {
  C? next;
  List<int>? list;
  int x = 0;
  C();
  C.n();
  static C make() => C();
}

Future<void> statements(List<Object?> items, C? c, Map<String, int> m) async {
  var a = 1, b = 2;
  final (int p, q) = (1, 2);
  var {'k': v} = m;
  a += b++ - --b * -a ~/ 2 % 3 << 1 >> 1 >>> 1 & 1 | 2 ^ 3;
  a = a > 1 && b < 2 || !(a == b) ? a ?? b : b;
  c?.next?..x = 1..next = null;
  c!.list?[0];
  c?.list?[0] = 1;
  c..list![0] = 2;
  var list = [1, ...?items, if (a > 0) 2 else 3, for (var i in items) i, for (var i = 0; i < 3; i++) i, ?c];
  var set = <int>{1, 2};
  var map = <String, int>{'a': 1, ?'b': ?null, 'c': 3};
  var record = (1, b: 2, c: 'x');
  var s = 'a $a ${b + 1}' "b" r'raw';
  var sym = #foo.bar;
  var f = <T>(T t, [int? o]) => t;
  int Function(int)? g = (int i) {
    return i;
  };
  var t = a is int ? a as int : a is! String;
  label:
  for (final x in items) {
    if (x case int y when y > 0) {
      continue label;
    } else if (x == null) {
      break label;
    }
  }
  for (var (i, j) = (0, 1); i < 3; i++, j--) {}
  for (; ;) {
    break;
  }
  while (a < 3) a++;
  do {
    a--;
  } while (a > 0);
  switch (a) {
    case 1:
    case 2 when b > 0:
      break;
    case int(isEven: true) || [_, ...] || {'k': _} || (1, x: _):
      break;
    default:
      break;
  }
  var e = switch (a) {
    1 => 'one',
    > 2 && < 10 => 'some',
    final int z? => '$z',
    _ => 'other',
  };
  try {
    await Future.value(1);
    throw Exception();
  } on StateError catch (e, st) {
    rethrow;
  } catch (e) {
  } finally {}
  assert(a > 0, 'positive');
  int local(int n) => n;
  local(1);
  late int lateVar;
  C.make().next = C.n();
  const C2 = C;
  new C();
  [a, b] = [b, a];
  (a, b) = (b, a);
  Color color = .red;
  var shorthand = .new(), sh2 = const .named(), sh3 = .make();
  print(statements);
  int.parse('1');
  List<int>.filled(1, 0);
  x<int>;
  c?.x;
  a = c!.x;
  return;
}

Stream<int> gen() async* {
  yield 1;
}

Iterable<int> sync() sync* {
  yield* [1];
}

enum Color { red }
