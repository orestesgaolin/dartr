// Differential fixture for the q-z AST-only lint ports.
library q_z_fixture;

import './q_z.dart';

/** Java-style documentation: slash_for_doc_comments. */
class SortBad {
  int value = 0;
  SortBad();
}

/// Slash-style documentation is the negative case.
class SortGood {
  SortGood();
  int value = 0;
}

class EqualsBad {
  @override
  bool operator ==(Object other) => (other as EqualsBad).value == value;

  int value = 0;
}

class EqualsGood {
  @override
  bool operator ==(Object other) => other is EqualsGood;
}

void throwCases() {
  try {
    print('body');
  } finally {
    throw StateError('bad');
  }

  try {
    print('body');
  } finally {
    void nested() => throw StateError('allowed in nested function');
    nested();
  }
}

/// <Tasty> is unintended HTML; <code>code</code> is valid HTML.
void documentationCase() {}

/// `<name>: <description>
/// Balanced `<code>` and an <Unintended> tag.
void documentationCodeSpanCases() {}

void stringAndCollectionCases(String value) {
  print('${value}');
  print('${value}Suffix');

  const nested = <Object>[const Object()];
  final standalone = const Object();
  print((nested, standalone));

  final unnecessaryRaw = r'plain text';
  final necessaryRaw = r'a\\b';
  final unnecessaryEscape = 'double: \"';
  final necessaryEscape = 'single: \'';
  final preferRaw = 'dollar: \$ and slash: \\';
  print((
    unnecessaryRaw,
    necessaryRaw,
    unnecessaryEscape,
    necessaryEscape,
    preferRaw,
  ));

  final Object? nullable = value.isEmpty ? null : value;
  print(nullable ?? null);
  print(nullable ?? value);
}

enum EnumBad {
  a;

  const EnumBad();
}

enum EnumGood {
  a;

  EnumGood();
}

final int topFinalWithType = 1;
final topFinalWithoutType = 2;

class LateCases {
  static late int unnecessary = 1;
  static late int necessary;
}

late int unnecessaryTopLate = 1;
late int necessaryTopLate;

void localFinalCases() {
  final int withType = 1;
  final withoutType = 2;
  var allowed = 3;
  print((withType, withoutType, allowed));
}

void patternAndForFinalCases() {
  final (int left, int right) = (1, 2);
  for (final int value in <int>[1]) {
    print(value);
  }
  var collectionValues = <int>[
    for (final int value in <int>[1]) value,
  ];
  print((left, right, collectionValues));
}

Object constructionCases() {
  final oldStyle = new Object();
  final modern = Object();
  return (oldStyle, modern);
}

class PrimaryBodyBad() {
  this {}
}

class PrimaryBodyGood(int value) {
  this {
    print(value);
  }
}

class ExplicitTypeNameBad {
  ExplicitTypeNameBad();
}

void functionTypedParameterBad(void callback()) => callback();
void functionTypedParameterGood(void Function() callback) => callback();

void unnecessaryBreak(Object value) {
  switch (value) {
    case 0:
      print(value);
      break;
    default:
      print(value);
  }
}

void necessaryBreak(Object value) {
  while (true) {
    break;
  }
  print(value);
}
