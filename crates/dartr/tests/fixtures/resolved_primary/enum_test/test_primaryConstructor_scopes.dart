const foo = 0;
enum A<@foo T>([@foo int x = foo]) {
  v;
  static const foo = 1;
}
