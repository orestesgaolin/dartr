enum E { a, b }

int f(Object a, Object b, Object c, Enum e, E concrete) {
  final _ = e.index;
  final _ = concrete.index;
  return Object.hash(a, b, c);
}
