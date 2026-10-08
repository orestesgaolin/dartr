#!/usr/bin/env dart
/// Library doc.
@Deprecated('x')
library fixtures.declarations;

import 'dart:async' deferred as async show Future, Stream hide Timer;
import 'dart:io' if (dart.library.html) 'dart:html' if (dart.library.io) 'dart:io';
export 'dart:math' show max;

part 'part.dart';

/// A class with modifiers.
@pragma('vm:entry-point')
abstract base class Base<T extends Object?> extends Object
    with MixinA<T>
    implements Comparable<Base<T>> {
  static const int k = 1;
  late final String? name;
  external int ext;
  covariant num? value;

  Base(this.name, [int? x = 1]) : assert(x != null, 'x'), super();
  Base.named({required String this.name, super.key}) : this(name);
  factory Base.create() = _Impl<T>;
  const Base.c(this.name);

  /// Doc after metadata.
  @override
  int compareTo(Base<T> other) => 0;

  T operator [](int i) => throw UnimplementedError();
  operator ==(Object other) => identical(this, other);
  int get length => 0;
  set length(int value) {}
  void f<S>(S Function(T) g, {int a = 0, required int b}) async* {
    yield* Stream.empty();
  }

  void g(void h(int x)?, [final int y = 2]);
}

mixin MixinA<T> on Object implements Comparable<Object> {}
base mixin MixinB {}

sealed class S {}
final class F extends S {}
interface class I {}
mixin class M {}
class Alias = Object with MixinA<int>;

extension Ext<T> on List<T> {
  T get second => this[1];
}

extension type const Id<T>._(int value) implements Object {
  Id.named(this.value);
}

enum Color with MixinA<int> implements Comparable<Color> {
  red,
  green.named(2),
  blue<int>(3);

  final int v;
  const Color([this.v = 0]);
  const Color.named(this.v);
}

enum Empty;

typedef Old<T> = void Function(T);
typedef void OldStyle<T>(T x, [int y]);
typedef Rec = ({int a, String b});

int top = 1, other = 2;
abstract external int conflicting;
void main() {}
