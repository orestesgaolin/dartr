import 'dart:async';

@pragma('dart2js:late:trust')
int annotated = 0;

class _PrivateType {}

_PrivateType publicValue = _PrivateType();

enum Choice { first, second }

abstract class SingleMember {
  void call();
}

class Base {
  int value = 0;
  Base(int original);
}

class Derived extends Base {
  @override
  int value = 1;
  Derived(int renamed) : super(renamed);
}

class MismatchedSuper extends Base {
  MismatchedSuper(super.renamed);
}

class AssertBody {
  AssertBody(int value) {
    assert(value > 0);
  }
}

class RuntimeTypeUse {
  String describe() => runtimeType.toString();
}

String obviousProperty = 'value';

int joined(bool flag) {
  var result = 0;
  result = flag ? 1 : 2;
  return result;
}

void examples(dynamic dynamicValue, bool value, [int? _local]) {
  if (1 == 1) print('literal');
  if (value == true) print('bool');
  List raw = <int>[];
  String obvious = 'value';
  int same = 1;
  Object implicitCast = dynamicValue;
  dynamicValue.toString();
  1.toString();
  value.runtimeType.toString();
  '${dynamicValue.toString()}';
  'has whitespace' 'without';
  <int>[1, 2].toSet();
  Map<String, int>();
  Future<int>(null);
  _local = 1;
  switch (1) {
    case 1: break;
    case 1: break;
  }
  switch (Choice.first) {
    case Choice.first: break;
    default: break;
  }
  print('$raw $obvious $same $implicitCast');
  throw 'bad';
}

T generic<T>(T? value) => value!;
