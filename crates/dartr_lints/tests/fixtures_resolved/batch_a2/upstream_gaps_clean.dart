import 'dart:async';
import 'dart:collection';
import 'dart:js_interop';

void typedParameterName(Queue<Object> Queue) {}

void inferredClosureHasNoContext() {
  final callback = (int value) => value;
  print(callback(1));
}

int Function(dynamic) explicitlyDynamicClosure() =>
    (dynamic value) => value as int;

class StaticCalls {
  static void first() {}
  static void second() {}
}

void staticCallsAreNotCascaded() {
  StaticCalls.first();
  StaticCalls.second();
}

enum OneEnum { value }

bool relatedEnum(List<OneEnum> values) => values.contains(OneEnum.value);

@Deprecated('Old implementation')
void oldImplementation() {
  oldImplementation();
}

void canceledInInitializer(Stream<Object?> stream) {
  final subscription = stream.listen((_) {})..cancel();
  print(subscription.isPaused);
}

bool supportedInteropCheck(JSString value) => value is JSAny;

class Diagnosticable {}

class DescribedPrimary(final int count) extends Diagnosticable {
  void debugFillProperties() {
    print(count);
  }
}

class CompleteEnumLike {
  final int value;
  const CompleteEnumLike._(this.value);

  static const first = CompleteEnumLike._(1);
  static const second = CompleteEnumLike._(2);
}

int completeEnumLikeSwitch(CompleteEnumLike value) {
  switch (value) {
    case CompleteEnumLike.first:
      return 1;
    case CompleteEnumLike.second:
      return 2;
  }
  return 0;
}
