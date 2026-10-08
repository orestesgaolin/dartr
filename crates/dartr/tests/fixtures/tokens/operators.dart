var ops = [a + b, a - b, a * b, a / b, a ~/ b, a % b, a << b, a >> b, a >>> b,
  a & b, a | b, a ^ b, ~a, !a, -a, a == b, a != b, a === b, a !== b, a < b, a <= b,
  a > b, a >= b, a && b, a || b, a ?? b, a?.b, a?..b, a..b, a?[0], [...?a, ...a],
  a is b, a as b, a is! b, a!, a = b, a += b, a -= b, a *= b, a /= b, a ~/= b,
  a %= b, a <<= b, a >>= b, a >>>= b, a &= b, a |= b, a ^= b, a ??= b, a++, --a,
  (a) => b, a ? b : c, @a, #a, a[b], a[] = 1, `x`, \ , a &&= b, a ||= b];
operator [](i) => 1; operator []=(i, v) {}
