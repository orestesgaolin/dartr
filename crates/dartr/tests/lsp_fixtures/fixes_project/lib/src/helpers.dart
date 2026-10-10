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

@Deprecated('Use paintFilled or paintColored')
void paint({String? color, bool? fill}) {}

void paintFilled({String? color}) {}

void paintColored({String? color, bool? fill}) {}

@Deprecated('Add mode')
void configure({bool? enabled, String? mode}) {}

@Deprecated('Non-null label')
void setLabel([String? label, int? count]) {}

