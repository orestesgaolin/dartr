/// Directive URI checks of `LibraryAnalyzer._resolveDirectives`.
///
/// @docImport 'missing_doc.dart';
library;

import 'missing.dart';
import 'missing.dart' as missing;
import 'missing_shown.dart' as shown show a;
import 'package:not_a_package/a.dart';
import 'dart:not_a_library';
import 'gen.g.dart';
import 'part_a.dart';
import 'other.dart' if (dart.library.io) 'missing_io.dart';
import 'deferred_missing.dart' deferred as lazy;
import ':';
export 'missing_export.dart';
export 'part_a.dart';
export 'other.dart';

part 'part_a.dart';
part 'part_a.dart';
part 'missing_part.dart';
part 'missing_part.g.dart';
part 'other.dart';
part 'named_part.dart';
part 'uri_part.dart';

void f() {
  missing.foo();
  missing.bar;
  shown.a;
  shown.b;
  lazy.loadLibrary();
  missing.Type? t;
  print(t);
}
