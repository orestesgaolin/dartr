class PublicType {}

PublicType publicValue = PublicType();

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
  final List<int> values = <int>[1, 2];
  throw StateError('bad');
  switch (Choice.first) {
    case Choice.first: break;
    case Choice.second: break;
  }
  print('${value.runtimeType} $values');
}
