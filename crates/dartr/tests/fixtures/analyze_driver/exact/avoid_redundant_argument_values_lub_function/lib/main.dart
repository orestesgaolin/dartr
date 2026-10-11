List<T> multi<T>(List<T> items, {int max = 5}) => items;
T single<T>(List<T> items, {int max = 5}) => items.first;

Object? pick(bool b, List<String> items) {
  final fn = b ? multi : single;
  return fn(items, max: 5);
}
