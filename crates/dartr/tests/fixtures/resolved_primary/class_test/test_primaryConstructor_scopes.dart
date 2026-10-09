const foo = 0;
class A<@foo T>([@foo int x = foo]) {
  static const foo = 1;
}
