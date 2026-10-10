// ignore_for_file: unused_import
import 'exists.dart'
    if (dart.library.io) 'missing.dart'
    if (dart.library.html) 'exists.dart'
    if (dart.library.async) 'dart:missing_sdk'
    if (dart.library.core) 'dart:core'
    if (dart.library.isolate) 'package:missing_pkg/foo.dart'
    if (dart.library.mirrors) 'package:p/exists.dart'
    if (dart.library.cli) 'package:p/missing_in_p.dart';

export 'exists.dart'
    if (dart.library.io) 'missing_export.dart'
    if (dart.library.html) 'exists.dart';
