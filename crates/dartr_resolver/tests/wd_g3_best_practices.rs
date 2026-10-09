//! Integration tests of the wave D group g3 (D10): `best_practices_verifier`
//! and the verifiers it drives (deprecated, experimental and
//! `@doNotSubmit` usage, invalid access, annotations, `@mustCallSuper`,
//! `@immutable`, null-safe APIs, widget previews). The cases are ported
//! from the analyzer diagnostic tests
//! (`pkg/analyzer/test/src/diagnostics/<code>_test.dart`): the same
//! sources, the expected diagnostics as (code, text of the reported range,
//! occurrence of that text). Only the codes of the group
//! (`tools/difftest/error_codes_g3.txt`) are compared.
//!
//! The test file is `package:test/main.dart` in a pub package `test` (Dart
//! `PubPackageResolutionTest`); `package:meta` and `package:angular_meta`
//! are the mock packages of the analyzer tests
//! (`MockPackages.addMetaPackageFiles`), other packages are written by the
//! case.

mod support;

use support::g3::{c, check, cp};

#[test]
fn deprecated_member_use() {
    check(vec![cp(
        "deprecated class, other package",
        &[("aaa", &[("a.dart", "@deprecated\nclass A {}\n")])],
        "import 'package:aaa/a.dart';\n\nvoid f(A a) {}\n",
        &[("deprecated_member_use", "A", 0)],
    )]);
}
