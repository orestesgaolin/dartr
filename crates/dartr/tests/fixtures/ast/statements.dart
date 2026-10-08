void statements(List<int> list, Object? o) async {
  label:
  for (var i = 0, j = 1; i < 10; i++, j--) {
    if (i == 2) continue label;
    if (i == 3) break;
  }
  for (final x in list) {}
  for (x in list) {}
  await for (var e in Stream.fromIterable(list)) {}
  for (var (a, b) in [(1, 2)]) {}
  for (var (a, b) = (1, 2); a < b; a++) {}
  while (o != null) {}
  do {} while (false);
  switch (o) {
    case 1:
    case 2 when o is int:
      break;
    label2:
    case int(isEven: true) || double():
    default:
  }
  var s = switch (o) {
    int i when i > 0 => 'positive',
    [var a, ...var rest] => '$a $rest',
    {'k': var v} => '$v',
    (var a, b: var b) => '',
    String() as Object => '',
    _ => 'other',
  };
  try {
    throw 'x';
  } on FormatException catch (e, st) {
    rethrow;
  } catch (e) {
  } finally {}
  assert(s.isNotEmpty, 'message');
  var (a, b) = (1, 2);
  final [c, d] = [3, 4];
  (a, b) = (b, a);
  if (o case int x when x > 0) {}
  int localFunction(int x) => x;
  void Function()? f;
  late final int l;
  ;
  {}
  return;
}
Iterable<int> gen() sync* {
  yield 1;
}
