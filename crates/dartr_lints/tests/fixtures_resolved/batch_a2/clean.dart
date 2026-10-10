import 'dart:async';
import 'dart:io';

class Value {
  @override
  bool operator ==(Object other) => other is Value;

  @override
  int get hashCode => 0;
}

class UsedParameter {
  final int value;
  UsedParameter(int supplied) : value = supplied;
}

Future<void> validAsync() async {}

Future<int> future() async => 1;

Future<void> examples(List<int> values, int? nullable) async {
  await future();
  nullable as int?;
  await future();
  values.contains(1);
}

Future<void> closedSink() async {
  final controller = StreamController<int>();
  controller.add(1);
  await controller.close();
}

Future<void> destroyedSocket() async {
  final socket = await Socket.connect('localhost', 0);
  socket.destroy();
}

class CascadedFieldSink {
  final controller = StreamController<int>();

  void dispose() {
    this..controller.close();
  }
}

void localLabeledControlFlow() {
  try {
    return;
  } finally {
    inner:
    while (true) {
      break inner;
    }
  }
}
