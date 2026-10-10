import 'dart:math';
import 'src/helpers.dart';

/// A documented class.
/// Second line.
class Shape {
  void area();
}

class Square implements Comparable<Square> {
  final int side = 1;
}

extension on String {
  int get size => length;
}

int deadCode(int x) {
  return x;
  print('dead');
}

int deadIf() {
  if (false) {
    print('never');
  }
  final value = true || deadIf() > 0;
  return value ? 1 : 0;
}

void setters(String s) {
  s.width = 3;
  s.size;
}

void loops(List<int> items) {
  for (int item in items) {
    print(item);
  }
  int counter = 0;
  final int total = max(counter, 2);
  print(total);
  print(const Helper().value());
  String? maybe;
  print(maybe!);
  var text = 'single line';
  print(text);
}

void cascades(List<int> items) {
  items..add(1);
}

int syncFunction() {
  return 1;
}
