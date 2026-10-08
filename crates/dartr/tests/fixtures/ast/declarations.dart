// Every declaration kind of a compilation unit and of class bodies.
library declarations;

import 'dart:async' as async show Future, Stream hide Timer;
import 'dart:io' if (dart.library.html) 'dart:html' deferred as io;
export 'dart:math' show max, min;
part 'part.dart';

/// A top level variable.
@pragma('vm:entry-point')
final int topLevel = 1, other = 2;
late String lateTop;
external int externalTop;
const c = [1, 2, ...[3]];

int get getter => 1;
set setter(int value) {}
T generic<T extends Object?>(T value, [int a = 1, int? b]) => value;
void named({required int a, int b = 2}) {}
external void externalFunction();
Future<void> asyncFunction() async {
  await null;
}
Stream<int> asyncStar() async* {
  yield 1;
  yield* Stream.empty();
}
Iterable<int> syncStar() sync* {
  yield 1;
}

typedef F = int Function(int);
typedef void OldStyle(int x);
typedef G<T> = List<T>;

abstract class A<T> extends Object with M implements I, J {
  static const int s = 1;
  final T? field;
  covariant late int c2;
  A(this.field) : assert(field != null), super();
  A.named({this.field}) : this(field);
  const factory A.redirect(T field) = B<T>;
  factory A.factory() {
    return B(null);
  }
  T get value;
  set value(T v);
  int operator +(int other) => 0;
  bool operator ==(Object other) => true;
  int operator [](int i) => i;
  void operator []=(int i, int v) {}
  int operator -() => 0;
  void method<S>(S s, void Function(int) f, int g(String s)) {}
  static void staticMethod() {}
  external void externalMethod();
  @override
  String toString() => super.toString();
}

class B<T> extends A<T> {
  B(super.field) : super();
  @override
  T get value => throw '';
  @override
  set value(T v) {}
}

mixin M on Object implements I {
  int x = 0;
}
base mixin BM {}
mixin class MC {}
sealed class S {}
final class FC {}
interface class IC {}
base class BC {}
abstract final class AFC {}
class I {}
class J {}
class N = Object with M;
abstract base class NA = Object with M implements I;

enum E { a, b, c }
enum E2<T> with M implements I {
  a<int>(1),
  b.named(2),
  c(3);

  const E2(this.v);
  const E2.named(this.v);
  final int v;
  int get w => v;
}

extension Ext on int {
  int get twice => this * 2;
  static int z = 1;
}
extension<T> on List<T> {
  T get firstOrSelf => first;
}
extension type ET(int i) implements Object {
  int get j => i;
}
extension type const ET2._(String s) {}
class Point(final int x, final int y);
class Empty;
