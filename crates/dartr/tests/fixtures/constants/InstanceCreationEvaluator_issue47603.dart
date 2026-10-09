
class C {
  final void Function() c;
  const C(this.c);
}

void main() {
  const C(() {});
//        ^^^^^
// [diag.constWithNonConstantArgument] Arguments of a constant creation must be constant expressions.
}
