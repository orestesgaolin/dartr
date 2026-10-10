// Dart source: pkg/analyzer_plugin/lib/src/channel/isolate_channel.dart
// (ServerIsolateChannel: spawn the plugin isolate, relay JSON maps)

// The bridge between `dartr analyze` and an analyzer plugin isolate.
//
// An analyzer plugin runs in an isolate of the analysis server and talks to
// it with JSON maps over a SendPort (`ServerIsolateChannel`). dartr is not a
// Dart program, so this script does the isolate part for it:
//
//   dart plugin_bridge.dart <entrypoint.dart> <package_config.json>
//
// It spawns the plugin entrypoint (`Isolate.spawnUri`, like
// `DiscoveredServerIsolateChannel`), then relays messages as JSON lines:
// stdin lines (requests) go to the plugin, plugin messages (responses and
// notifications) go to stdout. Isolate errors and the isolate exit are
// written as `{"bridge": "error", ...}` and `{"bridge": "exit"}` lines.
//
// No dependencies: it only uses `dart:` libraries, so it runs without
// `pub get`.

import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:isolate';

Future<void> main(List<String> args) async {
  if (args.length != 2) {
    stderr.writeln('usage: plugin_bridge.dart <entrypoint> <package_config>');
    exit(64);
  }
  var entrypoint = Uri.file(args[0]);
  var packageConfig = Uri.file(args[1]);

  var receivePort = ReceivePort();
  var errorPort = ReceivePort();
  var exitPort = ReceivePort();

  void emit(Object? message) {
    stdout.writeln(json.encode(message));
  }

  var sendPortCompleter = Completer<SendPort>();
  receivePort.listen((message) {
    if (message is SendPort) {
      if (!sendPortCompleter.isCompleted) {
        sendPortCompleter.complete(message);
      }
      return;
    }
    emit(message);
  });
  errorPort.listen((error) {
    // [error, stackTrace] as strings.
    var list = error is List ? error : [error, null];
    emit({
      'bridge': 'error',
      'message': '${list[0]}',
      'stackTrace': list.length > 1 && list[1] != null ? '${list[1]}' : '',
    });
  });
  var exited = Completer<void>();
  exitPort.listen((_) {
    emit({'bridge': 'exit'});
    exited.complete();
  });

  try {
    await Isolate.spawnUri(
      entrypoint,
      <String>[],
      receivePort.sendPort,
      onError: errorPort.sendPort,
      onExit: exitPort.sendPort,
      packageConfig: packageConfig,
    );
  } catch (exception, stackTrace) {
    emit({
      'bridge': 'error',
      'message': '$exception',
      'stackTrace': '$stackTrace',
    });
    emit({'bridge': 'exit'});
    await stdout.flush();
    exit(0);
  }

  var sendPort = await Future.any([
    sendPortCompleter.future,
    exited.future.then((_) => null),
  ]);
  if (sendPort == null) {
    await stdout.flush();
    exit(0);
  }
  emit({'bridge': 'ready'});

  var input = stdin
      .transform(utf8.decoder)
      .transform(const LineSplitter())
      .listen((line) {
        if (line.trim().isEmpty) return;
        var decoded = json.decode(line);
        if (decoded is Map) {
          // `PluginIsolateChannel` casts the message to
          // `Map<String, Object>`.
          sendPort.send(<String, Object>{
            for (var MapEntry(:key, :value) in decoded.entries)
              if (value != null) key as String: value as Object,
          });
        }
      });

  // The plugin isolate exits after `plugin.shutdown`; dartr may also close
  // stdin.
  await Future.any([exited.future, input.asFuture<void>()]);
  receivePort.close();
  errorPort.close();
  exitPort.close();
  await stdout.flush();
  exit(0);
}
