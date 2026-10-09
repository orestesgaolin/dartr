import 'dart:async';
import 'dart:io';

class StaticOnly {
  static var value = 0;
}

class MutableValue {
  bool operator ==(Object other) => other is MutableValue;

  int get hashCode => 0;
}

class ConstFields {
  final int value = 1;
  const ConstFields();
}

class SetOnly {
  set value(int value) {}
}

class Fluent {
  Fluent chain() => this;
}

class NamedOrder {
  void method({int optional = 0, required int requiredValue}) {}
}

Future<int> futureValue() async => 1;

Future<int> redundantFuture() async => futureValue();

void inferredDynamicForIn(Iterable<dynamic> values) {
  for (final value in values) {}
}

void examples(dynamic target, bool positional, int? nullable) {
  var inferred = 1;
  final values = [1, 2];
  final truth = positional ? true : false;
  target.member();
  nullable = null;
  print(inferred);
  values.forEach((value) => print(value));
  FileSystemEntity.exists('/tmp');
}

FutureOr<void> futureOrVoid() {}
