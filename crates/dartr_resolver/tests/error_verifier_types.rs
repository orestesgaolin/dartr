//! The ErrorVerifier checks of section D7 (type arguments, type
//! parameters, formal parameters, functions and methods, operators):
//! ports of `pkg/analyzer/test/src/diagnostics/*_test.dart`.

mod ev_support;
mod support;

use ev_support::assert_errors_in_code;

#[test]
fn invalid_use_of_covariant_method_ok() {
    assert_errors_in_code(
        r#"
class A {
  void m(covariant int a) {}
}
"#,
        &[],
    );
}
