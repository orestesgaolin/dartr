extension BoxExt<T> on List<T> {
  void addMaybe([T? value]) {}
}

void f(List<String?> items) {
  items.addMaybe(null);
}
