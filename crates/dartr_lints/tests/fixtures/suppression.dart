// ignore_for_file: prefer_single_quotes
class Example {}
String unicode = "😀";
void examples() {
  // ignore: unnecessary_new
  new Example();
  new Example(); // ignore: unnecessary_new
  new Example(); // ignore: plugin/unnecessary_new
  // ignore: type=lint
  new Example();
  const text = '// ignore: unnecessary_new';
  final suffix = "$text${unicode}";
  assert(suffix.isNotEmpty, 'content');
}
