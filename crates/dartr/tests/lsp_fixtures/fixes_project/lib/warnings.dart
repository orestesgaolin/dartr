import 'dart:collection';
import 'src/helpers.dart';

int unusedWork() {
  var unused = 1;
  int another = 2;
  return 0;
}

class C {
  int value;
  final String name;

  C(this.name);

  void m({String label}) {
    print(label);
  }
}

int noReturn() {
  if (helperConstant == 3) {
    return 1;
  }
}

void nullable(String? s) {
  print(s.length);
  print(s!.length);
}

void deadCode() {
  return;
  print('dead');
}
