// Override inference of fields, getters and setters
// (InstanceMemberInferrer._inferAccessorOrField).

abstract class Base {
  int get getterOnly;
  set setterOnly(String value);
  num get both;
  set both(num value);
  int get mismatch;
  set mismatch(String value);
  final double finalField = 0;
  covariant Object covariantField = 0;
  set covariantSetter(covariant num value);
}

abstract class FromInterfaces implements Base {
  // Field types from getters, setters, or both.
  var getterOnly;
  var setterOnly;
  var both;
  final mismatch;
  var finalField;
  var covariantField;
  FromInterfaces(this.mismatch);
}

abstract class NonFinalMismatch implements Base {
  // Getter and setter types differ: no override type, no initializer.
  var mismatch;
}

abstract class Accessors implements Base {
  get getterOnly;
  get setterOnly;
  set getterOnly(value);
  set setterOnly(value);
  get both;
  set both(value);
  set covariantSetter(int value);
}

abstract class Static implements Base {
  static var getterOnly;
  static get both => 0;
}

class NoOverride {
  var a;
  final b;
  get c => 0;
  set d(value) {}
  NoOverride(this.b);
}

abstract class Generic<T> {
  T get value;
  set value(T value);
  List<T> get list;
}

abstract class Specialized implements Generic<int> {
  var value;
  get list;
}

abstract class TwoGetters {
  int get x;
}

abstract class TwoGettersB {
  num get x;
}

abstract class CombinedGetter implements TwoGetters, TwoGettersB {
  get x;
}

abstract class TwoSetters {
  set y(int value);
}

abstract class TwoSettersB {
  set y(num value);
}

abstract class CombinedSetter implements TwoSetters, TwoSettersB {
  set y(value);
}
