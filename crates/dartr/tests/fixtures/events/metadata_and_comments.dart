/// Doc comment
@Deprecated('x')
@a.b.c
@d<int>()
library;

/** Block doc */
// line comment
@override /* inline */ void f(@required int x, {@deprecated int y = 1}) {}
