//! The error verifier runs after resolution and reports nothing on valid
//! code.

mod ev_support;
mod support;

use ev_support::assert_errors_in_code;

#[test]
fn valid_code_has_no_error_verifier_diagnostics() {
    assert_errors_in_code(
        r#"
class A {
  final int x;
  A(this.x);
  int get y => x + 1;
}

void main() {
  var a = A(1);
  print(a.y);
}
"#,
        &[],
    );
}
