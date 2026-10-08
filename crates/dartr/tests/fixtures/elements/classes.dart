// Fixture for `elements`: classes, class modifiers, fields, accessors,
// constructors (field formals, super formals, factories, redirecting),
// operators and generic methods.

abstract class Shape<T extends num> implements Comparable<Shape<T>> {
  final T size;
  static const int count = 3;
  static var created = 0;
  late String label;
  covariant Object? tag;
  external int ext;

  Shape(this.size);
  Shape.named({required T size, String label = 'x'}) : this(size);
  factory Shape.square(T size) = Square<T>;

  double get area;
  set area(double value) {}
  int get computed => 1;

  Shape<T> operator +(Shape<T> other);
  S pick<S extends T>(S a, [S? b]) => a;
  inferred(x) => x;

  @override
  int compareTo(Shape<T> other) => 0;
}

base class Square<T extends num> extends Shape<T> {
  Square(super.size);

  @override
  double get area => 0;

  @override
  Shape<T> operator +(Shape<T> other) => this;
}

sealed class Node {}

final class Leaf extends Node {}

interface class Api {
  const Api();
}

mixin class Both {}

class Plain {
  var x = 1;
  int y;
  Plain(this.y);
}

mixin M {}

class App = Object with M implements Api;
