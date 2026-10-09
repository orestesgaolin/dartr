import 'dart:async';

class Parent {
  void renamed(int value) {}
}

class Child extends Parent {
  @override
  void renamed(int other) {}
}

class ValueType {
  const ValueType();

  @override
  bool operator ==(Object other) => other is ValueType;

  @override
  int get hashCode => 0;
}

class ImplementsValueType implements ValueType {
  const ImplementsValueType();

  @override
  bool operator ==(Object other) => other is ImplementsValueType;

  @override
  int get hashCode => 0;
}

class NullableEquality {
  @override
  bool operator ==(Object? other) {
    if (other == null) return false;
    return other?.hashCode == hashCode;
  }

  @override
  int get hashCode => 0;
}

class RedeclaredBase {
  void inherited() {}
}

extension type RedeclaredExtension(RedeclaredBase value)
    implements RedeclaredBase {
  void inherited() {}
}

void withDefault([int value = 1]) {}

void acceptsFutureOrVoid(FutureOr<void> value) {}

typedef FutureOrVoidAlias = FutureOr<void>;

void semanticChecks(Object value) {
  int? initialized = null;
  withDefault(1);
  try {
    initialized = 1;
  } catch (error) {
    initialized = 2;
  }
  try {
    initialized = 3;
  } on Error catch (error) {
    initialized = 4;
  }
  if (value is double) {
    initialized = 5;
  } else if (value is int) {
    initialized = 6;
  }
  if (initialized == 7) {
    return;
  }
}

void returnsNull() => null;
