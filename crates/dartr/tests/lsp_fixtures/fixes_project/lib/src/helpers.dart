class Helper {
  const Helper();
  int value() => 1;
}

int helperFunction() => 2;

const helperConstant = 3;

enum HelperKind { a, b }

@Deprecated('Use compute2')
int compute(int a, int b, {int? scale}) => a + b;

int compute2(int a, int b) => a + b;

class Box {
  @Deprecated('Use Box')
  Box.old(int value);
  Box(int value);

  @Deprecated('Use size')
  int get length => 0;

  int size() => 0;

  @Deprecated('Use resize2')
  void resize(int width, {int? height}) {}
}
