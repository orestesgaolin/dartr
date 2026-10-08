void f() {
  var await = 1;
  var yield = 2;
  await x;
  yield y;
}
f2() async {
  var await = 1;
  async;
}
g() sync* {
  yield* x;
}
h() async* { await for (var x in y) yield x; }
i() sync { }
j() async* => 1;
