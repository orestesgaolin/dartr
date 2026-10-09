/// [missingName]
class EqualityOnly {
  @override
  bool operator ==(Object other) => other is EqualityOnly;
}

class UnusedParameter {
  UnusedParameter(int unused);
}

void voidAsync() async {
  await 1;
}

Future<int> future() async => 1;

void examples(List<int> values, int? nullable) {
  nullable as int;
  future();
  values.contains('unrelated');

  final buffer = StringBuffer();
  buffer.write('a');
  buffer.write('b');

  try {
    return;
  } finally {
    return;
  }
}

const environment = String.fromEnvironment('NAME');

void labeledControlFlow() {
  outer:
  while (true) {
    try {
      return;
    } finally {
      break outer;
    }
  }
}
