// Positive and negative cases for the AST-only g-p lint rules.
library Bad.Name;

import 'dart:async' as BadPrefix;
import 'dart:collection' as _privatePrefix;
import 'dart:convert' as good_prefix;
import 'dart:math' as _;
import 'package:flutter/material.dart';

/// ```
/// final missingLanguage = true;
/// ```
///
/// ```dart
/// final hasLanguage = true;
/// ```
class Examples {
  var untypedField;
  int typedField;

  Examples() : typedField = 0, assert(true);

  int expressionBodyCandidate() {
    return 1;
  }

  int blockBodyIsUseful() {
    final value = 1;
    return value;
  }

  void assertions() {
    assert(typedField >= 0);
    assert(typedField >= 0, 'typedField must be non-negative');
  }

  void assignments() {
    typedField = typedField;
    typedField = 1;
  }
}

final badMultiline = '''text starts before the newline
and continues here''';
final goodMultiline = '''
text starts after the newline
and continues here''';
final ordinaryLongLine = 'This ordinary source line is intentionally made longer than eighty characters for the line-length rule.';
final escapedNewlineLongLine = 'This escaped newline does not make the source line exempt from the eighty character limit.\n';
final nestedUriInInterpolation = 'A nested URI in an interpolation expression does not exempt this long source line: ${true ? 'https://example.com' : ''}';
final splitInterpolation = "${true ? 'first part'
        'This non-multiline interpolation spans source lines, and this nested line still exceeds eighty characters.' : ''}";
final allowedLongUri = 'https://example.com/a/very/long/path/that/is/intentionally/longer/than/eighty/source/characters';

typedef int OldStyle<T>(T value);
typedef NewStyle<T> = int Function(T value);

void Bad_function_name(int BadParameter, int goodParameter) {
  var BadLocalName = 1;
  int typedLocal;
  var untypedLocal;
  final condition = BadLocalName > 0;
  final Object? nullable = condition ? Object() : null;
  final void Function()? callback = condition ? () {} : null;
  final values = <int>[condition ? 1 : 2];
  final ordinaryConditional = condition ? 1 : 2;
  final adjacent = <String>['first' 'second'];
  final separated = <String>['first', 'second'];
  final concatenated = 'first' + 'second';
  final interpolated = "value: $BadLocalName";
  final shouldBeSingle = "double quoted";
  final doubleQuoteNeeded = "contains 'single quote'";
  final shouldBeDouble = 'single quoted';
  final singleQuoteNeeded = 'contains "double quote"';
  final coalescingCandidate = nullable == null ? Object() : nullable;
  final nullAwareCandidate = nullable == null ? null : nullable.hashCode;
  final nullAwareCallback = callback != null ? callback!() : null;
  final isNotCandidate = !(nullable is String);
  final alreadyIsNot = nullable is! String;
  final inlined = <int>[]..add(1);
  final inlinedMultiple = <int>[]..addAll(<int>[1, 2]);
  final spread = <int>[]..addAll(values);
  final alreadySpread = <int>[...values];
  typedLocal = goodParameter;
  untypedLocal = ordinaryConditional;
  print([
    BadParameter,
    typedLocal,
    untypedLocal,
    values,
    adjacent,
    separated,
    concatenated,
    interpolated,
    shouldBeSingle,
    doubleQuoteNeeded,
    shouldBeDouble,
    singleQuoteNeeded,
    coalescingCandidate,
    nullAwareCandidate,
    nullAwareCallback,
    isNotCandidate,
    alreadyIsNot,
    inlined,
    inlinedMultiple,
    spread,
    alreadySpread,
  ]);
}

void goodFunctionName(int goodParameter) {
  final goodLocalName = goodParameter;
  print(goodLocalName);
}
