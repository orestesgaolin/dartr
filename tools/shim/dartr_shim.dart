// A launcher for `dartr language-server`, for clients that can only start
// the analysis server as a Dart script.
//
// Dart-Code starts a custom analysis server (`dart.analyzerPath`) with the
// Dart VM: `dart [vm args] <analyzerPath> --lsp [server args]`. This script
// starts `dartr language-server` with the same server arguments, forwards
// stdin, stdout and stderr byte for byte, and exits with the exit code of
// dartr.
//
// The dartr binary is `$DARTR_BIN`, or `dartr` in the folder of this
// script (or of its snapshot). See docs/editor-setup.md.
//
// No dependencies: it only uses `dart:io`, so it runs without `pub get` and
// can be compiled to a snapshot (`dart compile jit-snapshot` or `aot-snapshot`).

import 'dart:async';
import 'dart:io';

Future<void> main(List<String> args) async {
  var dartr = _dartrPath();
  if (dartr == null) {
    stderr.writeln(
      'dartr_shim: dartr not found. Set DARTR_BIN to the dartr binary or '
      'put dartr next to ${Platform.script.toFilePath()}.',
    );
    exit(255);
  }

  Process process;
  try {
    process = await Process.start(dartr, ['language-server', ...args]);
  } on ProcessException catch (e) {
    stderr.writeln('dartr_shim: cannot start $dartr: ${e.message}');
    exit(255);
  }

  // stdin -> dartr. When the client closes stdin, close dartr's stdin.
  var stdinDone = Completer<void>();
  stdin.listen(
    process.stdin.add,
    onDone: () async {
      try {
        await process.stdin.flush();
        await process.stdin.close();
      } catch (_) {
        // dartr has exited already.
      }
      stdinDone.complete();
    },
    onError: (Object _) {},
    cancelOnError: false,
  );
  // dartr may exit before it reads all input (after `exit`).
  unawaited(process.stdin.done.catchError((Object _) {}));

  // dartr -> stdout/stderr, byte for byte.
  var stdoutDone = stdout.addStream(process.stdout);
  var stderrDone = stderr.addStream(process.stderr);

  var code = await process.exitCode;
  await Future.wait([stdoutDone, stderrDone]);
  await stdout.flush();
  await stderr.flush();
  exit(code);
}

/// `$DARTR_BIN`, or `dartr` next to this script.
String? _dartrPath() {
  var fromEnv = Platform.environment['DARTR_BIN'];
  if (fromEnv != null && fromEnv.isNotEmpty) {
    return fromEnv;
  }
  var dir = File(Platform.script.toFilePath()).parent.path;
  var name = Platform.isWindows ? 'dartr.exe' : 'dartr';
  var candidate = '$dir${Platform.pathSeparator}$name';
  return File(candidate).existsSync() ? candidate : null;
}
