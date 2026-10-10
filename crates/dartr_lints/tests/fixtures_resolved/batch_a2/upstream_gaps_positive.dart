import 'dart:async';
import 'dart:collection';
import 'dart:io';
import 'dart:js_interop';
import 'package:meta/src/meta.dart';

void acceptTearOff(String Function() callback) {}

extension TypeStringExtension on Type {
  void implicitTypeToString() {
    toString();
    acceptTearOff(toString);
  }
}

void parameterNamedForType(Queue) {}

void takesClosure(int Function(int) callback) {}

void typedClosureInContext() {
  takesClosure(((int value) => value));
}

int Function(int) typedClosureFromExpressionBody() =>
    (int value) => value;

int Function(int) typedClosureFromReturn() {
  return (int value) => value;
}

Future<int Function(int)> typedClosureFromAsyncReturn() async {
  return (int value) => value;
}

Iterable<int Function(int)> typedClosureFromYield() sync* {
  yield (int value) => value;
}

void takesNamedClosure({required int Function(int) callback}) {}

void typedClosureInNamedContext() {
  takesNamedClosure(callback: (int value) => value);
}

class CascadeTarget {
  int value = 0;

  void first() {}
  void second() {}
}

void consecutiveInvocations(CascadeTarget target) {
  target.first();
  target.second();
  target.value = 1;
  target.value = 2;
}

enum FirstEnum { value }
enum SecondEnum { value }

bool unrelatedEnum(List<FirstEnum> values) =>
    values.contains(SecondEnum.value);

@Deprecated('Use replacement')
void oldApi() {}

void deprecatedUse() {
  oldApi();
}

void leakedSubscription(Stream<Object?> stream) {
  final subscription = stream.listen((_) {});
  print(subscription.isPaused);
}

class LeakedSink {
  late IOSink sink;
}

bool nestedInteropCheck(List<Object> value) => value is List<JSString>;

void nestedInteropCatch() {
  try {
    print('body');
  } on List<JSString> catch (_) {
    print('caught');
  }
}

extension type UserJSString(JSString value) {}

bool userInteropCheck(Object value) => value is UserJSString;

class Diagnosticable {}

class PrimaryDiagnostic(final int count) extends Diagnosticable {}

class EnumLike {
  final int value;
  const EnumLike._(this.value);

  static const first = EnumLike._(1);
  static const second = EnumLike._(2);
}

int incompleteEnumLikeSwitch(EnumLike value) {
  switch (value) {
    case EnumLike.first:
      return 1;
  }
  return 0;
}
