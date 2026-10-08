// Differential table for `dartr_constant` (unit A14, constant values).
//
// Evaluates a generated table of constant operations two ways and prints the
// results as JSON:
//
// - "analyzer": the pinned package:analyzer `DartObjectImpl` operation
//   (value.dart), printed with `DartObjectImpl.toString()`, or
//   `E:<diagnostic code>` (`:runtime` appended for a runtime exception) when
//   it throws an `EvaluationException`.
// - "runtime": the same operation evaluated by the Dart VM (real `int` /
//   `double` / `String` semantics), or `T:<error type>` when it throws, or
//   `-` when the operands are not plain Dart values (unknown values,
//   composites).
//
// Operands: `i:<int>`, `d:<16 hex digits of the double bits>`,
// `s:<string>`, `b:true`/`b:false`, `n` (null), `iu`/`du`/`su`/`bu`
// (unknown int/double/String/bool), or a JSON object for a composite value
// (`{"k":"list","t":T,"e":[...]}`, `{"k":"set",...}`,
// `{"k":"map","kt":K,"vt":V,"e":[k,v,...]}`,
// `{"k":"record","p":[...],"n":{...}}`, `y:<symbol>`, `t:<type>`).
//
// Regenerate the fixture of crates/dartr_constant/tests/differential.rs:
//
//   (cd tools/oracle && dart run bin/const_values.dart \
//      > ../../crates/dartr_constant/tests/fixtures/const_values.json)
import 'dart:convert';
import 'dart:io';
import 'dart:math';
import 'dart:typed_data';

import 'package:analyzer/dart/analysis/analysis_context_collection.dart';
import 'package:analyzer/dart/analysis/declared_variables.dart';
import 'package:analyzer/dart/analysis/features.dart';
import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/file_system/physical_file_system.dart';
import 'package:analyzer/src/dart/constant/from_environment_evaluator.dart';
import 'package:analyzer/src/dart/constant/value.dart';
import 'package:analyzer/src/dart/element/element.dart';
import 'package:analyzer/src/dart/element/type.dart';
import 'package:analyzer/src/dart/element/type_provider.dart';
import 'package:analyzer/src/dart/element/type_system.dart';

import 'elements.dart' show oracleSdkPath;

late TypeSystemImpl ts;
late TypeProviderImpl tp;
final featureSet = FeatureSet.latestLanguageVersion();

const ints = <int>[
  0, 1, -1, 2, 3, -7, 31, 64, 2147483648, //
  9007199254740993, 0x7fffffffffffffff, -0x8000000000000000,
  -0x7fffffffffffffff, 123456789,
];

final doubles = <double>[
  0.0, -0.0, 1.0, -1.0, 0.5, 2.5, -2.5, 0.1, 1e-7, 1e21, 1e300, 5e-324, //
  double.nan, double.infinity, double.negativeInfinity,
  9.223372036854776e18,
];

const shiftAmounts = <int>[
  0, 1, 3, 31, 32, 62, 63, 64, 65, 100, -1, 2147483647, 2147483648, //
  -2147483649,
];

const strings = <String>['', 'a', 'abc', 'héllo', '\u{1F600}', "a'b"];

const parseInputs = <String>[
  '0', '42', '-42', '+42', ' 42 ', '\t42\n', ' 42　', '042', //
  '-0', '9223372036854775807', '9223372036854775808', '-9223372036854775808',
  '-9223372036854775809', '0x10', '0X10', '-0x10', '0xFFFFFFFFFFFFFFFF',
  '0x10000000000000000', '-0x8000000000000000', '-0x8000000000000001',
  '0x', '0x-1', '', ' ', '-', '+', '1_000', '1.0', '1e3', 'abc', '12abc',
  '0b101', '00000000000000000000000000001', '١', '0xabcDEF', '--1',
];

String encodeDouble(double d) {
  var data = ByteData(8)..setFloat64(0, d);
  String hex(int word) => word.toRadixString(16).padLeft(8, '0');
  return 'd:${hex(data.getUint32(0))}${hex(data.getUint32(4))}';
}

double decodeDouble(String hex) {
  var data = ByteData(8)
    ..setUint32(0, int.parse(hex.substring(0, 8), radix: 16))
    ..setUint32(4, int.parse(hex.substring(8), radix: 16));
  return data.getFloat64(0);
}

/// The runtime encoding of a Dart value (`null` when it is not plain).
String? encode(Object? v) {
  if (v == null) return 'n';
  if (v is int) return 'i:$v';
  if (v is double) return encodeDouble(v);
  if (v is bool) return 'b:$v';
  if (v is String) return 's:$v';
  return null;
}

/// The runtime value of an operand token, or a marker for "not plain".
const notPlain = Object();

Object? runtimeValue(Object operand) {
  if (operand is! String) return notPlain;
  if (operand == 'n') return null;
  if (operand.startsWith('i:')) return int.parse(operand.substring(2));
  if (operand.startsWith('d:')) return decodeDouble(operand.substring(2));
  if (operand.startsWith('s:')) return operand.substring(2);
  if (operand.startsWith('b:')) return operand == 'b:true';
  return notPlain;
}

TypeImpl typeNamed(String name) {
  switch (name) {
    case 'int':
      return tp.intType;
    case 'double':
      return tp.doubleType;
    case 'num':
      return tp.numType;
    case 'String':
      return tp.stringType;
    case 'bool':
      return tp.boolType;
    case 'Object':
      return tp.objectType;
    case 'Object?':
      return tp.objectQuestionType;
    case 'Null':
      return tp.nullType;
  }
  throw ArgumentError(name);
}

DartObjectImpl analyzerValue(Object operand) {
  if (operand is Map) {
    switch (operand['k']) {
      case 'list':
        var t = typeNamed(operand['t'] as String);
        return DartObjectImpl(
          ts,
          tp.listType(t),
          ListState(
            elementType: t,
            elements: [for (var e in operand['e'] as List) analyzerValue(e)],
          ),
        );
      case 'set':
        var t = typeNamed(operand['t'] as String);
        return DartObjectImpl(
          ts,
          tp.setType(t),
          SetState(
            elementType: t,
            elements: {for (var e in operand['e'] as List) analyzerValue(e)},
          ),
        );
      case 'map':
        var k = typeNamed(operand['kt'] as String);
        var v = typeNamed(operand['vt'] as String);
        var e = operand['e'] as List;
        var entries = <DartObjectImpl, DartObjectImpl>{};
        for (var i = 0; i < e.length; i += 2) {
          entries[analyzerValue(e[i])] = analyzerValue(e[i + 1]);
        }
        return DartObjectImpl(
          ts,
          tp.mapType(k, v),
          MapState(keyType: k, valueType: v, entries: entries),
        );
      case 'record':
        return DartObjectImpl(
          ts,
          tp.recordType,
          RecordState(
            [for (var e in operand['p'] as List) analyzerValue(e)],
            {
              for (var e in (operand['n'] as Map).entries)
                e.key as String: analyzerValue(e.value as Object),
            },
          ),
        );
    }
    throw ArgumentError(operand);
  }
  var s = operand as String;
  switch (s) {
    case 'n':
      return DartObjectImpl(ts, tp.nullType, NullState.NULL_STATE);
    case 'iu':
      return DartObjectImpl(ts, tp.intType, IntState.UNKNOWN_VALUE);
    case 'du':
      return DartObjectImpl(ts, tp.doubleType, DoubleState.UNKNOWN_VALUE);
    case 'su':
      return DartObjectImpl(ts, tp.stringType, StringState.UNKNOWN_VALUE);
    case 'bu':
      return DartObjectImpl(ts, tp.boolType, BoolState.UNKNOWN_VALUE);
  }
  if (s.startsWith('i:')) {
    return DartObjectImpl(ts, tp.intType, IntState(int.parse(s.substring(2))));
  }
  if (s.startsWith('d:')) {
    return DartObjectImpl(
      ts,
      tp.doubleType,
      DoubleState(decodeDouble(s.substring(2))),
    );
  }
  if (s.startsWith('s:')) {
    return DartObjectImpl(ts, tp.stringType, StringState(s.substring(2)));
  }
  if (s.startsWith('b:')) {
    return DartObjectImpl(ts, tp.boolType, BoolState.from(s == 'b:true'));
  }
  if (s.startsWith('y:')) {
    return DartObjectImpl(ts, tp.symbolType, SymbolState(s.substring(2)));
  }
  if (s.startsWith('t:')) {
    return DartObjectImpl(
      ts,
      tp.typeType,
      TypeState(typeNamed(s.substring(2))),
    );
  }
  throw ArgumentError(s);
}

String analyzerResult(DartObjectImpl Function() f) {
  try {
    return f().toString();
  } on EvaluationException catch (e) {
    var code = e.locatableDiagnostic.code.lowerCaseUniqueName;
    return 'E:$code${e.isRuntimeException ? ':runtime' : ''}';
  }
}

String runtimeResult(Object? Function() f) {
  try {
    return encode(f()) ?? '-';
  } catch (e) {
    return 'T:${e.runtimeType}';
  }
}

final binaryAnalyzer =
    <String, DartObjectImpl Function(DartObjectImpl, DartObjectImpl)>{
      '+': (a, b) => a.add(ts, b),
      '-': (a, b) => a.minus(ts, b),
      '*': (a, b) => a.times(ts, b),
      '/': (a, b) => a.divide(ts, b),
      '~/': (a, b) => a.integerDivide(ts, b),
      '%': (a, b) => a.remainder(ts, b),
      '<': (a, b) => a.lessThan(ts, b),
      '<=': (a, b) => a.lessThanOrEqual(ts, b),
      '>': (a, b) => a.greaterThan(ts, b),
      '>=': (a, b) => a.greaterThanOrEqual(ts, b),
      '==': (a, b) => a.equalEqual(ts, featureSet, b),
      '!=': (a, b) => a.notEqual(ts, featureSet, b),
      'identical': (a, b) => a.isIdentical2(ts, b),
      '&': (a, b) => a.eagerAnd(ts, b),
      '|': (a, b) => a.eagerOr(ts, b),
      '^': (a, b) => a.eagerXor(ts, b),
      '<<': (a, b) => a.shiftLeft(ts, b),
      '>>': (a, b) => a.shiftRight(ts, b),
      '>>>': (a, b) => a.logicalShiftRight(ts, b),
      '&&': (a, b) => a.lazyAnd(ts, () => b),
      '||': (a, b) => a.lazyOr(ts, () => b),
      'concat': (a, b) => a.concatenate(ts, b),
    };

final binaryRuntime = <String, Object? Function(dynamic, dynamic)>{
  '+': (a, b) => a + b,
  '-': (a, b) => a - b,
  '*': (a, b) => a * b,
  '/': (a, b) => a / b,
  '~/': (a, b) => a ~/ b,
  '%': (a, b) => a % b,
  'remainder': (a, b) => a.remainder(b),
  '<': (a, b) => a < b,
  '<=': (a, b) => a <= b,
  '>': (a, b) => a > b,
  '>=': (a, b) => a >= b,
  '==': (a, b) => a == b,
  '!=': (a, b) => a != b,
  'identical': (a, b) => identical(a, b),
  '&': (a, b) => a & b,
  '|': (a, b) => a | b,
  '^': (a, b) => a ^ b,
  '<<': (a, b) => a << b,
  '>>': (a, b) => a >> b,
  '>>>': (a, b) => a >>> b,
  '&&': (a, b) => (a as bool) && (b as bool),
  '||': (a, b) => (a as bool) || (b as bool),
  'concat': (a, b) => '$a$b',
};

final unaryAnalyzer = <String, DartObjectImpl Function(DartObjectImpl)>{
  'neg': (a) => a.negated(ts),
  '~': (a) => a.bitNot(ts),
  '!': (a) => a.logicalNot(ts),
  'toString': (a) => a.performToString(ts),
  'length': (a) => a.stringLength(ts),
};

final unaryRuntime = <String, Object? Function(dynamic)>{
  'neg': (a) => -a,
  '~': (a) => ~a,
  '!': (a) => !(a as bool),
  'toString': (a) => a.toString(),
  'length': (a) => (a as String).length,
};

final cases = <List<Object?>>[];

void binary(String op, Object a, Object b) {
  var analyzer = binaryAnalyzer.containsKey(op)
      ? analyzerResult(
          () => binaryAnalyzer[op]!(analyzerValue(a), analyzerValue(b)),
        )
      : '-';
  var ra = runtimeValue(a), rb = runtimeValue(b);
  var runtime = identical(ra, notPlain) || identical(rb, notPlain)
      ? '-'
      : runtimeResult(() => binaryRuntime[op]!(ra, rb));
  cases.add([op, a, b, analyzer, runtime]);
}

void unary(String op, Object a) {
  var analyzer = analyzerResult(() => unaryAnalyzer[op]!(analyzerValue(a)));
  var ra = runtimeValue(a);
  var runtime = identical(ra, notPlain)
      ? '-'
      : runtimeResult(() => unaryRuntime[op]!(ra));
  cases.add([op, a, null, analyzer, runtime]);
}

Future<void> main() async {
  var dir = Directory.systemTemp
      .createTempSync('dartr_const_values_')
      .resolveSymbolicLinksSync();
  var file = '$dir/a.dart';
  File(file).writeAsStringSync('');
  var collection = AnalysisContextCollection(
    includedPaths: [dir],
    resourceProvider: PhysicalResourceProvider.INSTANCE,
    sdkPath: oracleSdkPath(),
  );
  var result =
      await collection.contextFor(file).currentSession.getResolvedLibrary(file)
          as ResolvedLibraryResult;
  var library = result.element as LibraryElementImpl;
  ts = library.typeSystem;
  tp = library.typeProvider;

  var numbers = <String>[
    for (var i in ints) 'i:$i',
    for (var d in doubles) encodeDouble(d),
  ];
  const numOps = [
    '+', '-', '*', '/', '~/', '%', 'remainder', '<', '<=', '>', '>=', '==', //
    'identical',
  ];
  for (var op in numOps) {
    for (var a in numbers) {
      for (var b in numbers) {
        binary(op, a, b);
      }
    }
  }
  for (var op in ['&', '|', '^']) {
    for (var a in ints) {
      for (var b in ints) {
        binary(op, 'i:$a', 'i:$b');
      }
    }
  }
  for (var op in ['<<', '>>', '>>>']) {
    for (var a in ints) {
      for (var b in shiftAmounts) {
        binary(op, 'i:$a', 'i:$b');
      }
    }
  }
  for (var a in numbers) {
    unary('neg', a);
    unary('toString', a);
    if (a.startsWith('i:')) unary('~', a);
  }

  // Formatting of doubles: random bit patterns and random decimals.
  var random = Random(14);
  for (var i = 0; i < 600; i++) {
    var bits = (random.nextInt(1 << 32) << 32) | random.nextInt(1 << 32);
    var d = (ByteData(8)..setUint64(0, bits)).getFloat64(0);
    if (d.isNaN) continue;
    unary('toString', encodeDouble(d));
  }
  for (var i = 0; i < 600; i++) {
    var exponent = random.nextInt(50) - 25;
    var d = random.nextDouble() * pow(10, exponent);
    if (random.nextBool()) d = -d;
    unary('toString', encodeDouble(d));
    unary('toString', encodeDouble(d.roundToDouble()));
  }

  // Strings and bools.
  var stringValues = [for (var s in strings) 's:$s'];
  for (var a in stringValues) {
    unary('length', a);
    unary('toString', a);
    for (var b in stringValues) {
      binary('+', a, b);
      binary('concat', a, b);
      binary('==', a, b);
      binary('identical', a, b);
    }
  }
  const boolValues = ['b:true', 'b:false'];
  for (var a in boolValues) {
    unary('!', a);
    unary('toString', a);
    for (var b in boolValues) {
      for (var op in ['&', '|', '^', '&&', '||', '==', 'identical']) {
        binary(op, a, b);
      }
    }
  }

  // Unknown values and operands of the wrong type (analyzer only).
  const others = [
    'iu', 'du', 'su', 'bu', 'n', 'i:2', 'i:0', 'd:4000000000000000', //
    's:x', 'b:true', 'b:false',
  ];
  for (var op in binaryAnalyzer.keys) {
    for (var a in others) {
      for (var b in others) {
        binary(op, a, b);
      }
    }
  }
  for (var op in unaryAnalyzer.keys) {
    for (var a in others) {
      unary(op, a);
    }
  }

  // Composite values: toString, ==, identical.
  var composites = <Object>[
    {'k': 'list', 't': 'int', 'e': []},
    {
      'k': 'list',
      't': 'int',
      'e': ['i:1', 'i:2'],
    },
    {
      'k': 'list',
      't': 'num',
      'e': ['i:1', 'i:2'],
    },
    {
      'k': 'list',
      't': 'Object?',
      'e': ['i:1', encodeDouble(2.5), 's:a', 'n', 'b:true'],
    },
    {
      'k': 'list',
      't': 'int',
      'e': ['i:1', 'iu'],
    },
    {
      'k': 'set',
      't': 'String',
      'e': ['s:b', 's:a', 's:b'],
    },
    {
      'k': 'set',
      't': 'num',
      'e': ['i:1', encodeDouble(1.0), 'i:1'],
    },
    {
      'k': 'map',
      'kt': 'String',
      'vt': 'int',
      'e': ['s:a', 'i:1', 's:b', 'i:2', 's:a', 'i:3'],
    },
    {
      'k': 'map',
      'kt': 'Object',
      'vt': 'Object?',
      'e': [
        {
          'k': 'list',
          't': 'int',
          'e': ['i:1'],
        },
        'n',
        {
          'k': 'list',
          't': 'int',
          'e': ['i:1'],
        },
        's:dup',
      ],
    },
    {
      'k': 'record',
      'p': ['i:1', 's:x'],
      'n': {'b': 'b:true', 'a': encodeDouble(-0.0)},
    },
    {
      'k': 'record',
      'p': [],
      'n': {'z': 'n'},
    },
    {
      'k': 'record',
      'p': ['i:1'],
      'n': {},
    },
    'y:foo',
    'y:bar',
    't:int',
    't:String',
    'n',
    's:it\'s',
  ];
  for (var a in composites) {
    unary('toString', a);
    // `DartObjectImpl.toString()` itself (the `const` field of the
    // `elements` dump).
    cases.add(['display', a, null, analyzerValue(a).toString(), '-']);
    for (var b in composites) {
      binary('==', a, b);
      binary('identical', a, b);
    }
  }

  // int.fromEnvironment / bool.fromEnvironment (`int.parse`).
  var parse = <List<Object?>>[];
  for (var input in parseInputs) {
    var evaluator = FromEnvironmentEvaluator(
      ts,
      DeclaredVariables.fromMap({'x': input}),
    );
    parse.add([
      input,
      evaluator.getInt('x', null).toString(),
      evaluator.getBool('x', null).toString(),
      runtimeResult(() => int.parse(input)),
    ]);
  }

  var out = StringBuffer('{"cases": [\n');
  out.write(cases.map(jsonEncode).join(',\n'));
  out.write('\n],\n"parse": [\n');
  out.write(parse.map(jsonEncode).join(',\n'));
  out.write('\n]}\n');
  stdout.write(out);
  Directory(dir).deleteSync(recursive: true);
  exit(0);
}
