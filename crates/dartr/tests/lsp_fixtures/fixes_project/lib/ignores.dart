// ignore_for_file: unused_import, dead_code

int? maybe(bool c) {
  if (c) {
    return 1;
  }
}

String? other(int x) {
  // ignore: unused_local_variable
  var used = x;
  print(used);
  // ignore: unused_element, unused_local_variable
  var unused = 2;
  if (x > 0) return 'a';
}

class K {
  int? get value {
    if (DateTime.now().year > 0) {
      return 1;
    }
  }
}
