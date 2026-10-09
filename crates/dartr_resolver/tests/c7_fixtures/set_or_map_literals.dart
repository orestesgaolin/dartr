// Ported from pkg/analyzer/test/src/dart/resolution/set_or_map_literal_test.dart
// (set-or-map disambiguation, upwards inference of key, value and element
// types).
import 'dart:async';

void f(int a, int? b, String s, Map<int, String> m, Map<int, String>? mn,
    Set<int> si, List<int> li, dynamic d, Never n) {
  var m1 = {};
  var m2 = {1: 's'};
  var m3 = {a: s, 1.0: 2};
  var s1 = {1};
  var s2 = {1, 's'};
  var s3 = <int>{};
  var m4 = <int, String>{};
  Set<num> s4 = {};
  Map<int, num> m5 = {};
  Iterable<int> s5 = {};
  var m6 = {...m};
  var m7 = {...?mn};
  var s6 = {...si};
  var s7 = {...li, 1};
  var x1 = {...d};
  var m8 = {...d, 1: 2};
  var s8 = {...d, 1};
  var m9 = {if (a > 0) 1: 's' else 2: 3};
  var s9 = {if (a > 0) 1 else 's'};
  var m10 = {for (var i = 0; i < 2; i++) i: i};
  var s10 = {for (var x in li) x};
  var m11 = {?b: 1};
  var m12 = {1: ?b};
  var s11 = {?b};
  FutureOr<Set<int>> s12 = {};
  FutureOr<Map<int, int>> m13 = {};
  Object o1 = {};
  dynamic d1 = {};
  var m14 = const {1: 'a'};
  var s13 = const {1, 2};
  var m15 = {...m, if (a > 0) ...m};
  var s14 = {...n};
  var m16 = {...m, ...d};
  var e1 = <int, String, bool>{};
  var e2 = <int, String, bool>{1};
}
