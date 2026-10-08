// Positive and negative cases for AST-only a-f lint rules.
/// This looks like a library comment but is attached to an import.
/// ```dart
/// // TODO(engineer): A TODO inside a documentation code block is not the
/// // comment token's TODO.
/// ```
import 'z.dart' show zName, aName;
import 'dart:math';
import 'package:example/example.dart';
import 'a.dart';
import '../other/lib/source.dart';
export 'z.dart';
export 'a.dart';

typedef _UnusedAlias(int value);
typedef UsedAlias = void Function();
UsedAlias usedAlias() => () {};

class bad_type<T> {
  bad_type();

  void set value(int value) {}

  method<T>(dynamic value) {
    if (value == null) print('empty else'); else;
    if (value != null) print("value"); else print('none');
    if (value != null)
      print('missing braces');
    while (false) print('never');
    for (;;) break;
    do continue; while (false);

    final escaped = 'it\'s changeable';
    final stable = 'contains "both" and \'quotes\'';
    final rounded = 9007199254740993;
    final exact = 9007199254740992;
    var first = 1, second = 2;
    for (var left = 0, right = 1; left < right; left++) {}
    StringBuffer()..write('one section');
    StringBuffer()
      ..write('first')
      ..write('second');
    const BAD_CONSTANT = 1;
    const goodConstant = 2;
    ;
    try {
      print(escaped);
    } catch (error) {}
    try {
      print(stable);
    } catch (_) {}
    // ignore: unused_local_variable
    final ignored = [rounded, exact, first, second, BAD_CONSTANT, goodConstant];
    // This ignore is documented.
    // ignore: unused_local_variable
    final documented = ignored;
    // TODO: invalid Flutter-style TODO.
    // TODO(engineer): valid Flutter-style TODO.
  }
}

class GoodType {
  GoodType() {}
  int method(int value) => value;
}

class empty_container {}

extension bad_extension on String {}
extension GoodExtension on int {
  int get doubled => this * 2;
}

enum bad_enum { BAD_VALUE, goodValue }

void topLevelWithoutType() {}
int topLevelWithType() => 0;
