// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_constant_argumentList_functionExpression_flowAnalysis).

enum E {
  v(() {
// [diag.constWithNonConstantArgument][column 5][length 69] Arguments of a constant creation must be constant expressions.
    Object? x = 0;
    if (x is int) {
      x.isEven;
    }
  });

  final void Function() f;
  const E(this.f);
}
