
void f(Object? x) {
  ++switch (x) {
    _ => 0,
  };
//^
// [diag.missingAssignableSelector] Missing selector such as '.identifier' or '[0]'.
}
