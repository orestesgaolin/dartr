// The diagnostics that the C7 resolvers report (ported from the analyzer
// diagnostic tests of the same codes).
void f(dynamic d, Never n, List<int>? ln, Map<int, int>? mn, void v,
    Iterable<int>? itn, Stream<int>? sn) async {
  print({...d});
  print({...n});
  print((_a: 1));
  print((x: 1, x: 2));
  print((1, $1: 2));
  print((1, $2: 2));
  print((hashCode: 1, toString: 2));
  print((v, x: v));
  print([...ln]);
  print({...mn});
  print([...?ln]);
  for (var x in itn) {
    print(x);
  }
  await for (var x in sn) {
    print(x);
  }
  for (var (x, y) in itn) {
    print(x);
  }
  print([for (var x in itn) x]);
}
