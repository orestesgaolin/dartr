// Fixture for `elements`: directives, parts, typedefs, top-level functions,
// variables and accessors.
library;

import 'dart:async' deferred as async_lib;
import 'dart:math' as math show max, min hide pi;
import 'classes.dart' hide Leaf;
import 'missing_file.dart';

export 'mixins_enums.dart' show Color;
export 'extensions.dart';

part 'library_part.dart';

typedef IntMapper = int Function(int);
typedef Mapper<T, R> = R Function(T);
typedef void OldCallback(String message, [int code]);
typedef ShapeList = List<Shape<int>>;

const limit = 10;
final values = [1, 2, 3];
late final String name;
external int counter;
int? _private;

int get total => limit * 2;
set total(int value) {}

T identity<T>(T x) => x;
Future<int> load() async => math.max(1, 2);
Iterable<int> range(int n) sync* {
  yield n;
}
Stream<int> stream() async* {}
void withParams(int a, [String b = 'b'], {required bool c, double? d}) {}
