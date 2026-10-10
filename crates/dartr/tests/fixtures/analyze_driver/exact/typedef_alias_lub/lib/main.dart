// The static type of `??`, `?:` and collection literals is the least upper
// bound of the operand types. Dart keeps a typedef alias only when both
// operand types are the same object (the same declaration): two record or
// function types from different declarations give a type without the
// alias, an interface type keeps it.
typedef Pair = ({double a, double b});
typedef IntList = List<int>;
typedef Fn = int Function(int);

class C {
  Pair? x;
  Pair? y;
  IntList? l1;
  IntList? l2;
  Fn? f1;
  Fn? f2;

  void m(bool c) {
    var v1 = x ?? y!;
    var v2 = l1 ?? l2!;
    var v3 = f1 ?? f2!;
    var v4 = c ? x! : y!;
    var v5 = x!;
    int i = v1;
    int j = v2;
    int k = v3;
    int n = v4;
    int o = v5;
    print([i, j, k, n, o]);
  }
}

void f(bool c, Pair a, Pair b, Fn g, Fn h, Pair? n) {
  int i2 = c ? a : b;
  int i3 = c ? g : h;
  int i4 = n ?? a;
  int i5 = [a, b];
  print([i2, i3, i4, i5]);
}
