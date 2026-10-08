// Dart source: pkg/_fe_analyzer_shared/test/exhaustiveness/is_exhaustive_type_test.dart

//! Test `subtract()` on combinations of types.
//!
//! Each Dart `group` is one Rust test; the Dart tests that the helpers of
//! `utils.dart` register in the group are the logical cases of its
//! [Collector].

#[macro_use]
mod exhaustiveness_support;

use dartr_type_analyzer::exhaustiveness::{ObjectPropertyLookup, Path, Space, StaticType};
use exhaustiveness_support::{
    Collector, TestEnvironment, expect_exhaustive, expect_not_exhaustive,
};

const FILE: &str = "is_exhaustive_type_test.dart";

// Note: In the class diagrams, "(_)" means "sealed". A bare name is unsealed.

#[test]
fn sealed_family() {
    let mut tests = Collector::new(FILE, "sealed family |");
    //     (A)
    //     / \
    //   (B) (C)
    //   / \   \
    //  D   E   F
    let env = TestEnvironment::new();
    let a = env.create_class("A", true, &[], &[]);
    let b = env.create_class("B", true, &[a], &[]);
    let c = env.create_class("C", true, &[a], &[]);
    let d = env.create_class("D", false, &[b], &[]);
    let e = env.create_class("E", false, &[b], &[]);
    let f = env.create_class("F", false, &[c], &[]);

    let check_exhaustive = make_test_function(&env, vec![a, b, c, d, e, f]);
    check_exhaustive(&mut tests, &[a], "ABCDEF");
    check_exhaustive(&mut tests, &[b], "BDE");
    check_exhaustive(&mut tests, &[c], "CF");
    check_exhaustive(&mut tests, &[d], "D");
    check_exhaustive(&mut tests, &[f], "CF");

    check_exhaustive(&mut tests, &[a, b], "ABCDEF");
    check_exhaustive(&mut tests, &[a, c], "ABCDEF");
    check_exhaustive(&mut tests, &[a, d], "ABCDEF");
    check_exhaustive(&mut tests, &[a, f], "ABCDEF");

    check_exhaustive(&mut tests, &[b, c], "ABCDEF");
    check_exhaustive(&mut tests, &[b, d], "BDE");
    check_exhaustive(&mut tests, &[b, f], "ABCDEF");

    check_exhaustive(&mut tests, &[c, d], "CDF");
    check_exhaustive(&mut tests, &[c, e], "CEF");
    check_exhaustive(&mut tests, &[c, f], "CF");

    check_exhaustive(&mut tests, &[d, e], "BDE"); // Covers B because both cases covered.
    check_exhaustive(&mut tests, &[d, f], "CDF");
    check_exhaustive(&mut tests, &[e, f], "CEF");

    check_exhaustive(&mut tests, &[d, e, f], "ABCDEF"); // All cases covered.
    tests.finish();
}

#[test]
fn sealed_with_many_subtypes() {
    let mut tests = Collector::new(FILE, "sealed with many subtypes |");
    //     (A)
    //    //|\\
    //   / /|\ \
    //  B C D E F
    let env = TestEnvironment::new();
    let a = env.create_class("A", true, &[], &[]);
    let b = env.create_class("B", false, &[a], &[]);
    let c = env.create_class("C", false, &[a], &[]);
    let d = env.create_class("D", false, &[a], &[]);
    let e = env.create_class("E", false, &[a], &[]);
    let f = env.create_class("F", false, &[a], &[]);

    let check_exhaustive = make_test_function(&env, vec![a, b, c, d, e, f]);
    check_exhaustive(&mut tests, &[a], "ABCDEF");
    check_exhaustive(&mut tests, &[b], "B");
    check_exhaustive(&mut tests, &[c, e], "CE");
    check_exhaustive(&mut tests, &[b, d, f], "BDF");
    check_exhaustive(&mut tests, &[b, c, e, f], "BCEF");
    check_exhaustive(&mut tests, &[b, c, d, e, f], "ABCDEF"); // Covers A.
    tests.finish();
}

#[test]
fn sealed_with_multiple_paths() {
    let mut tests = Collector::new(FILE, "sealed with multiple paths |");
    //     (A)
    //     / \
    //   (B)  C
    //   / \ /
    //  D   E
    let env = TestEnvironment::new();
    let a = env.create_class("A", true, &[], &[]);
    let b = env.create_class("B", true, &[a], &[]);
    let c = env.create_class("C", false, &[a], &[]);
    let d = env.create_class("D", false, &[b], &[]);
    let e = env.create_class("E", false, &[b, c], &[]);

    let check_exhaustive = make_test_function(&env, vec![a, b, c, d, e]);
    check_exhaustive(&mut tests, &[a], "ABCDE");
    check_exhaustive(&mut tests, &[b], "BDE");
    check_exhaustive(&mut tests, &[c], "CE");
    check_exhaustive(&mut tests, &[d], "D");
    check_exhaustive(&mut tests, &[e], "E");

    check_exhaustive(&mut tests, &[b, c], "ABCDE");
    check_exhaustive(&mut tests, &[b, d], "BDE");
    check_exhaustive(&mut tests, &[b, e], "BDE");
    check_exhaustive(&mut tests, &[c, d], "ABCDE");
    check_exhaustive(&mut tests, &[d, e], "BDE");
    tests.finish();
}

#[test]
fn sealed_with_unsealed_supertype() {
    let mut tests = Collector::new(FILE, "sealed with unsealed supertype |");
    //    A
    //    |
    //   (B)
    //   / \
    //  C   D
    let env = TestEnvironment::new();
    let a = env.create_class("A", false, &[], &[]);
    let b = env.create_class("B", true, &[a], &[]);
    let c = env.create_class("C", false, &[b], &[]);
    let d = env.create_class("D", false, &[b], &[]);

    let check_exhaustive = make_test_function(&env, vec![a, b, c, d]);
    check_exhaustive(&mut tests, &[a], "ABCD");
    check_exhaustive(&mut tests, &[b], "BCD");
    check_exhaustive(&mut tests, &[c], "C");
    check_exhaustive(&mut tests, &[d], "D");
    check_exhaustive(&mut tests, &[c, d], "BCD");
    tests.finish();
}

#[test]
fn sealed_with_single_subclass() {
    let mut tests = Collector::new(FILE, "sealed with single subclass |");
    // (A)
    //  |
    // (B)
    //  |
    //  C
    let env = TestEnvironment::new();
    let a = env.create_class("A", true, &[], &[]);
    let b = env.create_class("B", true, &[a], &[]);
    let c = env.create_class("C", false, &[b], &[]);

    let check_exhaustive = make_test_function(&env, vec![a, b, c]);
    check_exhaustive(&mut tests, &[a], "ABC");
    check_exhaustive(&mut tests, &[b], "ABC"); // Every A must be a B, so A is covered.
    check_exhaustive(&mut tests, &[c], "ABC"); // Every C must be a B, which must be an A.
    check_exhaustive(&mut tests, &[a, b], "ABC");
    check_exhaustive(&mut tests, &[a, c], "ABC");
    check_exhaustive(&mut tests, &[b, c], "ABC");
    check_exhaustive(&mut tests, &[a, b, c], "ABC");
    tests.finish();
}

#[test]
fn unsealed() {
    let mut tests = Collector::new(FILE, "unsealed |");
    //      A
    //     / \
    //    B   C
    //   / \ / \
    //  D   E   F
    let env = TestEnvironment::new();
    let a = env.create_class("A", false, &[], &[]);
    let b = env.create_class("B", false, &[a], &[]);
    let c = env.create_class("C", false, &[a], &[]);
    let d = env.create_class("D", false, &[b], &[]);
    let e = env.create_class("E", false, &[b, c], &[]);
    let f = env.create_class("F", false, &[c], &[]);

    let check_exhaustive = make_test_function(&env, vec![a, b, c, d, e, f]);
    check_exhaustive(&mut tests, &[a], "ABCDEF");
    check_exhaustive(&mut tests, &[b], "BDE");
    check_exhaustive(&mut tests, &[d], "D");
    check_exhaustive(&mut tests, &[a, b], "ABCDEF"); // Same as A.
    check_exhaustive(&mut tests, &[a, d], "ABCDEF"); // Same as A.
    check_exhaustive(&mut tests, &[d, e], "DE"); // Doesn't cover B because unsealed.
    check_exhaustive(&mut tests, &[d, f], "DF");
    check_exhaustive(&mut tests, &[e, f], "EF"); // Doesn't cover C because unsealed.
    check_exhaustive(&mut tests, &[b, f], "BDEF");
    check_exhaustive(&mut tests, &[c, d], "CDEF");
    check_exhaustive(&mut tests, &[d, e, f], "DEF");
    tests.finish();
}

/// Returns a function that takes a list of `types` and a string containing a
/// list of type letters that map to the types in [all_types].
///
/// The function checks that the list of types exhaustively covers every type
/// whose name appears in the string. So:
///
/// ```text
///     checkExhaustive([d, e], 'bde');
/// ```
///
/// Means that the union of D|E should be exhaustive over B, D, and E and not
/// exhaustive over the other types in [all_types].
fn make_test_function(
    field_lookup: &dyn ObjectPropertyLookup,
    all_types: Vec<StaticType>,
) -> impl Fn(&mut Collector, &[StaticType], &str) + '_ {
    assert!(all_types.len() <= 6, "Only supports up to six types.");
    let letters = "ABCDEF";

    move |tests: &mut Collector, types: &[StaticType], covered: &str| {
        let spaces: Vec<Space> = types
            .iter()
            .map(|type_| Space::new(Path::root(), *type_))
            .collect();

        for (i, type_) in all_types.iter().enumerate() {
            let value = Space::new(Path::root(), *type_);
            if covered.contains(&letters[i..i + 1]) {
                expect_exhaustive(tests, field_lookup, &value, &spaces);
            } else {
                expect_not_exhaustive(tests, field_lookup, &value, &spaces);
            }
        }
    }
}
