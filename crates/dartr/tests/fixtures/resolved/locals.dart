// Local variables, local functions, closures, labels, pattern variables,
// type parameters of local functions.
int top(int p) {
  var sum = p;
  int add(int q) => q + sum;
  T id<T>(T t) => t;
  outer:
  for (var i in [1, 2, 3]) {
    for (var j = 0; j < i; j++) {
      if (j > 1) break outer;
      if (j == 0) continue outer;
      sum = add(j);
    }
  }
  [1, 2].map((e) => e * 2).forEach((v) => sum += v);
  if ((1, 2) case (var x, var y)) sum += x + y;
  switch (sum) {
    case int z when z > 10 || z < 0:
      sum = z;
  }
  return id<int>(sum);
}
