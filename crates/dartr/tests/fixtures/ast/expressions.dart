var expressions = [
  1, 0x1F, 1_000, 1.5, 1e10, 1_0.0_1, true, false, null, 'a', "b", r'c', '''d''',
  'a' 'b' "c", 'interp $x ${x + 1}', #symbol, #a.b, #+, #void,
  [1, if (true) 2 else 3, for (var i in []) i, ...[], ...?null, ?null],
  <int>{1, 2}, {1: 2, ?3: 4, 5: ?6}, <String, int>{}, const [1], const {},
  (1, 2), (a: 1), (1,), const (1, b: 2),
  a.b.c, a?.b, a!.b, a..b = 1..c(), a?..b, a[0], a?[0], a!, -a, !a, ~a, ++a, a--,
  a + b * c - d / e % f ~/ g, a << 1 >> 2 >>> 3, a & b | c ^ d, a && b || c,
  a == b, a != b, a < b, a >= b, a ?? b, a ? b : c, a is int, a is! String, a as num,
  a = 1, a += 1, a ??= 2, a >>>= 3, throw 'e', await x, f<int>(), f<int>,
  List<int>.filled(1, 0), new A(), const A.named(), A.new, A<int>.new,
  (int x) => x, (x) {}, <T>(T x) => x, super.x, this.y, .foo, .bar(), const .baz(),
  x is (int, {String s}), x as void Function<T>(T)?,
];
