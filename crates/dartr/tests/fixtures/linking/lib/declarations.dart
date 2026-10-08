library declarations;

import 'dart:async' as async show Future, FutureOr;
import 'cycle_a.dart';

part 'src/part_one.dart';

/// A class with all member kinds.
abstract base class Shape<T extends num, U extends Comparable<U>> implements Comparable<Shape<T, U>> {
  static const int sides = 0;
  static int counter = 0;
  final T size;
  late final String label;
  covariant Object? extra;
  U? key;

  Shape(this.size, {this.extra});
  Shape.named(T size) : this(size);
  factory Shape.create(T size) = Square<T, U>;

  T get doubled;
  set doubled(T value) {}
  int compareTo(Shape<T, U> other) => 0;
  Shape<T, U> operator +(Shape<T, U> other) => this;
  S convert<S extends T>(S value, [int factor = 2, String Function(S)? f]) => value;
  external void nativeThing();
  static void util({required int a, int b = 3}) {}
}

final class Square<T extends num, U extends Comparable<U>> extends Shape<T, U> {
  Square(super.size, {super.extra});
  @override
  T get doubled => size;
}

sealed class Result {}
interface class Iface {}
mixin class MixinClass {}
class Impl extends Result with MixinClass implements Iface {}
class App = Result with MixinClass;

mixin Logger<T> on Shape<num, T> {
  void log(T message) {}
  int get count => super.hashCode;
}

class LoggedShape extends Square<int, String> with Logger {
  LoggedShape(super.size);
}

enum Color with MixinClass implements Iface {
  red(1),
  green.named(2),
  blue(3);

  final int value;
  const Color(this.value);
  const Color.named(this.value);
  static Color get first => red;
}

enum Plain { a, b }

extension StringExt on String {
  String get shout => toUpperCase();
  static int helper() => 1;
}

extension on int {
  int get twice => this * 2;
}

extension type Meters(double value) implements Object {
  Meters operator +(Meters other) => Meters(value + other.value);
}

extension type const Id<T>._(T _raw) {}

typedef IntMapper = int Function(int);
typedef Pair<A, B> = (A first, B second);
typedef void OldStyle<T>(T value, [int count]);
typedef Json = Map<String, Object?>;

int topLevel = 1;
final String finalTop = 'x';
const List<int> constTop = [1, 2];
var inferred = 3;
int get topGetter => 0;
set topSetter(int value) {}
async.Future<void> run([async.FutureOr<int>? x]) async {}
Iterable<int> gen() sync* {}
T identity<T extends Object?>(T value) => value;
(int, {String name}) record() => (1, name: '');
void withFunctionParam(int callback(String s), {required covariant int Function()? other}) {}
CycleA? useCycle;
