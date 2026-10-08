import 'a.dart' show A hide B as p deferred;
import 'b.dart' as q if (dart.library.io) 'c.dart';
import 'c.dart' deferred as r show X;
import 'd.dart' foo show Y;
export 'e.dart' hide Z
part 'f.dart';
library lib;
