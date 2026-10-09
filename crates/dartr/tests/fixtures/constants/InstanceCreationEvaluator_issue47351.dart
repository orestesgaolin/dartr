
class Foo {
  final int bar;
  const Foo(this.bar);
}

int bar = 2;
const a = const Foo(bar);
//                  ^^^
// [diag.constWithNonConstantArgument] Arguments of a constant creation must be constant expressions.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
