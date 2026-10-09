
void f(Object? x) {
  (switch (x) {
    _ => 1,
  } == 0);
}
