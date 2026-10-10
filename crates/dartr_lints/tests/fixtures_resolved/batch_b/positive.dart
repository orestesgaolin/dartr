// Batch B rules without hits in the corpora.
import 'package:meta/meta.dart';

void finalParams(int x, {String? y}) {
  print(x);
  print(y);
}

void reassignedParam(int x) {
  x = 2;
  print(x);
}

class Recursive {
  int get loop => loop;
  int value() => value();
}

class Tighten {
  Tighten.named(this.p) : assert(p != null);
  final String? p;
}

void typeLiteralPattern(Object o) {
  if (o case int) {}
  switch (o) {
    case String:
      break;
  }
}

class Named {
  Named.new();
}

var namedNew = Named.new();

class GettersSetters {
  String? _x;
  String? get x => _x;
  set x(String? value) {
    _x = value;
  }
}

void nullAwareAssignment(String? s) {
  s ??= null;
  print(s);
}

extension NullableExtension on int? {
  int get extended => 1;
  int method() => 2;
}

void nullAwareOnExtension(int? i) {
  i?.extended;
  i?.method();
}

final int? finalNullable = 1;
const int? constNullable = 2;

class FinalNullable {
  final int? _field = 3;
  static final int? staticField = 4;
  int? get field => _field;
}

void localFinalNullable() {
  final int? local = 5;
  print(local);
}

var spreads = [
  ...[1, 2].map((e) => e).toList(),
  ...{3}.toList(),
];

class Constant {
  static const Constant zero = Constant(0);
  final int v;
  const Constant(this.v);
}

const constantZero = Constant(0);
var constantZero2 = const Constant(0);

int truncating(int a, int b) => (a / b).toInt();
int truncatingParenthesized(int a, int b) => ((a / b)).toInt();

var invalidRegExp = RegExp('(');
var invalidRegExpRaw = RegExp(r'[a-');
var validRegExp = RegExp('a+');

@immutable
class Immutable {
  const Immutable(this.items);
  final List<int> items;
}

var immutableLiteral = Immutable([1, 2]);
var immutableMap = ImmutableMap({1: 2});

@immutable
class ImmutableMap {
  const ImmutableMap(this.map);
  final Map<int, int> map;
}

@immutable
class NotConstConstructor {
  NotConstConstructor(this.a);
  final int a;
}

@immutable
class NotConstNamed {
  NotConstNamed.named();
  factory NotConstNamed.factory() => NotConstNamed.named();
}
