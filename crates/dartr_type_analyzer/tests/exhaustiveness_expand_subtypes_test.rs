// Dart source: pkg/_fe_analyzer_shared/test/exhaustiveness/expand_subtypes_test.dart

#[macro_use]
mod exhaustiveness_support;

use dartr_type_analyzer::exhaustiveness::{StaticType, StaticTypeArena, expand_sealed_subtypes};
use exhaustiveness_support::{Collector, TestEnvironment, expect_eq};
use indexmap::IndexSet;

const FILE: &str = "expand_subtypes_test.dart";

fn run(name: &str, body: impl FnOnce() -> Result<(), String>) {
    let mut c = Collector::new(FILE, "");
    c.test(name, body);
    c.finish();
}

fn expect_expand(env: &TestEnvironment, type_: StaticType, expected: &str) -> Result<(), String> {
    let types: &dyn StaticTypeArena = env.types();
    expect_eq(
        expand_sealed_subtypes(types, type_, &IndexSet::new())
            .iter()
            .map(|t| types.name(*t))
            .collect::<Vec<_>>()
            .join("|")
            .as_str(),
        expected,
    )
}

#[test]
fn sealed() {
    run("sealed", || {
        //   (A)
        //   /|\
        //  B C(D)
        //     / \
        //    E   F
        let env = TestEnvironment::new();
        let a = env.create_class("A", true, &[], &[]);
        env.create_class("B", false, &[a], &[]);
        env.create_class("C", false, &[a], &[]);
        let d = env.create_class("D", true, &[a], &[]);
        env.create_class("E", false, &[d], &[]);
        env.create_class("F", false, &[d], &[]);

        expect_expand(&env, a, "B|C|E|F")?;
        expect_expand(&env, d, "E|F")
    });
}

#[test]
fn unsealed() {
    run("unsealed", || {
        //    A
        //   /|\
        //  B C D
        //     / \
        //    E   F
        let env = TestEnvironment::new();
        let a = env.create_class("A", false, &[], &[]);
        env.create_class("B", false, &[a], &[]);
        env.create_class("C", false, &[a], &[]);
        let d = env.create_class("D", false, &[a], &[]);
        env.create_class("E", false, &[d], &[]);
        env.create_class("F", false, &[d], &[]);

        expect_expand(&env, a, "A")?;
        expect_expand(&env, d, "D")
    });
}

#[test]
fn unsealed_in_middle() {
    run("unsealed in middle", || {
        //    (A)
        //    / \
        //   B   C
        //      / \
        //     D  (E)
        //        / \
        //       F   G
        let env = TestEnvironment::new();
        let a = env.create_class("A", true, &[], &[]);
        env.create_class("B", false, &[a], &[]);
        let c = env.create_class("C", false, &[a], &[]);
        env.create_class("D", false, &[c], &[]);
        let e = env.create_class("E", true, &[c], &[]);
        env.create_class("F", false, &[e], &[]);
        env.create_class("G", false, &[e], &[]);

        expect_expand(&env, a, "B|C")?;
        expect_expand(&env, c, "C")?;
        expect_expand(&env, e, "F|G")
    });
}

#[test]
fn transitive_sealed_family() {
    run("transitive sealed family", || {
        //     (A)
        //     / \
        //   (B) (C)
        //   / | | \
        //  D  E F  G
        //     \ /
        //      H
        let env = TestEnvironment::new();
        let a = env.create_class("A", true, &[], &[]);
        let b = env.create_class("B", true, &[a], &[]);
        let c = env.create_class("C", true, &[a], &[]);
        let d = env.create_class("D", false, &[b], &[]);
        let e = env.create_class("E", false, &[b], &[]);
        let f = env.create_class("F", false, &[c], &[]);
        env.create_class("G", false, &[c], &[]);
        let h = env.create_class("H", false, &[e, f], &[]);

        expect_expand(&env, a, "D|E|F|G")?;
        expect_expand(&env, b, "D|E")?;
        expect_expand(&env, d, "D")?;
        expect_expand(&env, e, "E")?;
        expect_expand(&env, h, "H")
    });
}

#[test]
fn sealed_with_multiple_paths() {
    run("sealed with multiple paths", || {
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

        expect_expand(&env, a, "D|E|C")?;
        expect_expand(&env, b, "D|E")?;
        expect_expand(&env, c, "C")?;
        expect_expand(&env, d, "D")?;
        expect_expand(&env, e, "E")
    });
}

#[test]
fn nullable() {
    run("nullable", || {
        //   (A)
        //   / \
        //  B   C
        //     / \
        //    D   E
        let env = TestEnvironment::new();
        let a = env.create_class("A", true, &[], &[]);
        env.create_class("B", false, &[a], &[]);
        let c = env.create_class("C", false, &[a], &[]);
        env.create_class("D", false, &[c], &[]);

        // Sealed subtype.
        expect_expand(&env, a.nullable(env.types()), "B|C|Null")?;

        // Unsealed subtype.
        expect_expand(&env, c.nullable(env.types()), "C|Null")
    });
}
