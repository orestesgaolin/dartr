import 'dart:collection';

class _PrivateType {}

class _PrivateOwner {
  _PrivateType publicMember(_PrivateType value) => value;
}

typedef PublicAlias = _PrivateType Function();

T inferredResult<T>() => throw StateError('fixture');

void inferenceContext() {
  String value = inferredResult();
  print(value);
}

void dynamicForIn(dynamic values) {
  for (final int value in values) {
    print(value);
  }
}

class FieldFormalAssert {
  final int value;

  FieldFormalAssert(this.value) {
    assert(value > 0);
  }
}

typedef AliasMap<K, V> = Map<K, V>;

void obviousTypes() {
  List<int> explicit = <int>[];
  List<int> inferred = [1, 2];
  int local = 1;
  int copy = local;
  StringBuffer buffer = StringBuffer()..write('value');
  int hash = Object().hashCode;
  AliasMap<String, int>();
  print('$explicit $inferred $copy $buffer $hash');
}

void hashCollectionContext() {
  LinkedHashSet<int> values = LinkedHashSet<int>();
  print(values);
}

void nestedParameterMutation(List<int> target, int index) {
  target[index++] = 1;
}

void selfParameterMutation(int index) {
  index = index++;
}

void optionalParameterMutation([int? index]) {
  List<int> target = <int>[0];
  target[index ??= 0] = 1;
  index ??= 1;
}
