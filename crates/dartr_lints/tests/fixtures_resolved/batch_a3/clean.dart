class PublicType {}

class PragmaMember {
  @pragma('dart2js:late:trust')
  void method() {}
}

PublicType makePublicType() => PublicType();
PublicType publicValue = makePublicType();

class PublicBase {
  PublicBase(int value);
}

class MatchingSuper extends PublicBase {
  MatchingSuper(super.value);
}

class InstanceAssert {
  bool get valid => true;

  InstanceAssert() {
    assert(valid);
  }
}

enum Choice { first, second }

abstract class MultipleMembers {
  void first();
  void second();
}

int joined(bool flag) {
  return flag ? 1 : 2;
}

void examples(Object value, bool enabled) {
  if (enabled) print('enabled');
  final List<num> values = <int>[1, 2];
  final Set<num> set = <int>{1, 2};
  final Future<num> future = Future<int>(() => 1);
  'has ' 'space';
  switch (Choice.first) {
    case Choice.first: break;
    case Choice.second: break;
  }
  print('${value.runtimeType} $values $set $future');
  throw StateError('bad');
}

void namedParameter({int _privateName = 0}) {
  print(_privateName);
}
