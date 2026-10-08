void main() async {
  for (var i = 0; i < 10; i++) {}
  for (final x in y) {}
  await for (var e in s) {}
  for (var (a, b) in pairs) {}
  while (true) { break; continue; }
  do { } while (false);
  try { } on E catch (e, s) { } finally { }
  try { } catch (e, s, t) { }
  label: for (;;) { break label; }
  assert(x, 'msg');
  yield 1;
  return;
  throw x;
  rethrow;
  switch (x) { case 1: case 2: f(); default: }
  if (a) b(); else if (c) d(); else e();
  { nested; }
  ;
}
