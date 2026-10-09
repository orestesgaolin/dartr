/// Shapes library.
library;

import 'dart:math' as math;
import 'dart:async';

export 'src/util.dart';

part 'src/part.dart';

// A top-level constant.
const double kPi = math.pi;

final List<int> numbers = [
  1,
  2,
  3,
];

/// A shape.
abstract class Shape {
  /// The area.
  double get area;

  String describe() {
    return 'Shape with area $area';
  }
}

class Circle extends Shape {
  final double radius;
  static int count = 0;

  Circle(this.radius);

  Circle.unit() : this(1);

  factory Circle.fromDiameter(double d) => Circle(d / 2);

  @override
  double get area => kPi * radius * radius;

  set scale(double factor) {}

  Circle operator +(Circle other) => Circle(radius + other.radius);
}

enum Color {
  red,
  green,
  blue;

  bool get isRed => this == Color.red;
}

mixin Named {
  String get name => 'named';
}

extension CircleX on Circle {
  double get diameter => radius * 2;
}

extension type Meters(double value) {
  Meters add(Meters other) => Meters(value + other.value);
}

typedef Callback = void Function(int value);
typedef IntList = List<int>;

int add(int a, int b) => a + b;

Future<void> run() async {
  var c = Circle(2);
  void local() {
    print(c.area);
  }

  local();
  await Future<void>.delayed(Duration.zero);
  final values = [
    for (var i = 0; i < 3; i++)
      i * 2,
  ];
  switch (values.length) {
    case 0:
      print('none');
    default:
      print('some');
  }
  try {
    throw StateError('x');
  } catch (e) {
    print(e);
  } finally {
    print('done');
  }
}

// #region custom region
int get topGetter => 1;
set topSetter(int v) {}
// #endregion

/* block
   comment */
var record = (1, name: 'x');
