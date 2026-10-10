import 'src/helpers.dart';
import 'src/other.dart';

void dataDriven(Base b) {
  var o = OldOther(1);
  print(o);
  OldOther? x;
  print(x);
  b.oldRun();
  oldHelper();
}

class Point2 {
  final int x;
  Point2(this.x);
  int get twice => x * 2;
}

void members(Point2 p) {
  print(p.missingField);
  p.missingSetter = 1;
  p.extMethod(2);
}

void parameters(int a, {int? b}) {
  print(missingParameter);
}

class Mixed with UnknownMixin {}

class Implementer implements Point2, Point2 {}
