import 'dart:async';

class Stateful {
  var value = 0;
}

class Pair {
  final int value;
  const Pair(this.value);
}

class Property {
  int get value => 0;
  set value(int value) {}
}

class Builder {
  void apply() {}
}

void named({required int requiredValue, int optional = 0}) {}

Future<int> futureValue() async => 1;

Future<int> awaitedFuture() async => await futureValue();

void examples(Object target, {required bool enabled}) {
  int inferred = 1;
  List<int> values = <int>[1, 2];
  bool truth = enabled;
  target.toString();
  values.forEach(printValue);
  inferred += truth ? 1 : 0;
}

void printValue(int value) {}

FutureOr<int> futureOrInt() => 1;
