import 'dart:async';
import 'dart:math';
import 'src/other.dart';

void useImports() {
  var h = Helper();
  helperFunction();
  print(helperConstant);
  var c = Completer<int>();
  var r = Random();
  var d = Directory('x');
  var l = LinkedHashMap<int, int>();
  print([h, c, r, d, l]);
  var u = Unknown();
  print(u);
  print(HelperKind.a);
}
