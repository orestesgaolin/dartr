List<List<List<int>>> a = [];
Map<String, List<int>> b = {};
var c = x >>> 2;
var d = x >>= 1;
var e = x >>>= 3;
var f = x < y > z;
var g = f<int>(1) < 2;
class A<T extends Comparable<T>> {}
var h = <int, List<Map<int, int>>>{};
var i = a<b, c>(d);
void m<T>() => this.m<List<T>>();
var j = a < b && c > d;
var k = <List<int>>[];
var l = List<int Function(List<int>)>;
var n = a<b<c<d<e>>>>;
var o = a<b; var p = c>d;
