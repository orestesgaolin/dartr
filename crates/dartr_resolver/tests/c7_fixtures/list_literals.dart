// Ported from pkg/analyzer/test/src/dart/resolution/list_literal_test.dart
// and the if/for/spread element tests (context, upwards inference, and
// collection elements).
import 'dart:async';

void f(int a, int? b, String s, List<int> li, List<int>? lni, dynamic d,
    Iterable<num> inum, Never n, Null nul) {
  var l1 = [];
  var l2 = [1, 2];
  var l3 = [1, 2.0];
  var l4 = [a, s];
  var l5 = <num>[1];
  var l6 = <dynamic>[a];
  List<num> l7 = [1];
  List<int?> l8 = [];
  var l9 = [...li];
  var l10 = [...?lni];
  var l11 = [...inum, 1];
  var l12 = [if (a > 0) 1 else 's'];
  var l13 = [if (a > 0) 1];
  var l14 = [for (var i = 0; i < 3; i++) i];
  var l15 = [for (var x in li) x.toString()];
  var l16 = [?b];
  var l17 = [...d];
  var l18 = [...nul];
  var l19 = [...?nul];
  var l20 = [for (final (x, y) in [(1, 's')]) y];
  var l21 = [if (b case var c?) c];
  var l22 = [if (b case int c when c > 0) c else 'x'];
  Iterable<Object> l23 = [...li, 's'];
  FutureOr<List<int>> l24 = [];
  var l25 = const [1, 'a'];
  var l26 = [[], [1]];
  var l27 = <int>[...li, if (a > 1) a];
  var l28 = [null, 1];
  Object l29 = [a];
  var l30 = [for (; a < 0;) a];
}
