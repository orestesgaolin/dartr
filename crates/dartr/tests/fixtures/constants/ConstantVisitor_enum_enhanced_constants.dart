
enum E {
  v1(42), v2(v1);
  final Object? a;
  const E([this.a]);
}
