void main() {
  print('hello');
  // ignore: my_lint/no_print_calls
  print('ignored');
  // ignore: no_print_calls
  print('not ignored: the plugin name is missing');
  var unused = 1;
}
