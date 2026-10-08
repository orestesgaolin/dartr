// Dart source: pkg/_fe_analyzer_shared/test/exhaustiveness/is_exhaustive_field_test.dart

//! Each Dart `group` is one Rust test; the Dart tests that the helpers of
//! `utils.dart` register in the group are the logical cases of its
//! [Collector].

#[macro_use]
mod exhaustiveness_support;

use exhaustiveness_support::{
    Collector, TestEnvironment, expect_exhaustive_only_all, expect_never_exhaustive,
};

const FILE: &str = "is_exhaustive_field_test.dart";

#[test]
fn sealed_subtypes() {
    let mut tests = Collector::new(FILE, "sealed subtypes |");
    //   (A)
    //   / \
    //  B   C
    let env = TestEnvironment::new();
    let a = env.create_class("A", true, &[], &[]);
    let b = env.create_class("B", false, &[a], &[]);
    let c = env.create_class("C", false, &[a], &[]);
    let t = env.create_record_type(&[("x", a), ("y", a)]);

    expect_exhaustive_only_all(
        &mut tests,
        &env,
        t,
        objs![
            ty!(&env, t, {x: b, y: b}),
            ty!(&env, t, {x: b, y: c}),
            ty!(&env, t, {x: c, y: b}),
            ty!(&env, t, {x: c, y: c}),
        ],
    );
    tests.finish();
}

#[test]
fn sealed_subtypes_medium() {
    let mut tests = Collector::new(FILE, "sealed subtypes medium |");
    //   (A)
    //   /|\
    //  B C D
    let env = TestEnvironment::new();
    let a = env.create_class("A", true, &[], &[]);
    let b = env.create_class("B", false, &[a], &[]);
    let c = env.create_class("C", false, &[a], &[]);
    let d = env.create_class("D", false, &[a], &[]);
    let t = env.create_record_type(&[("y", a), ("z", a)]);

    expect_exhaustive_only_all(
        &mut tests,
        &env,
        t,
        objs![
            ty!(&env, t, {y: b, z: b}),
            ty!(&env, t, {y: b, z: c}),
            ty!(&env, t, {y: b, z: d}),
            ty!(&env, t, {y: c, z: b}),
            ty!(&env, t, {y: c, z: c}),
            ty!(&env, t, {y: c, z: d}),
            ty!(&env, t, {y: d, z: b}),
            ty!(&env, t, {y: d, z: c}),
            ty!(&env, t, {y: d, z: d}),
        ],
    );
    tests.finish();
}

#[test]
fn sealed_subtypes_large() {
    let mut tests = Collector::new(FILE, "sealed subtypes large |");
    //   (A)
    //   /|\
    //  B C D
    let env = TestEnvironment::new();
    let a = env.create_class("A", true, &[], &[]);
    let b = env.create_class("B", false, &[a], &[]);
    let c = env.create_class("C", false, &[a], &[]);
    let d = env.create_class("D", false, &[a], &[]);
    let t = env.create_record_type(&[("w", a), ("x", a), ("y", a), ("z", a)]);

    expect_exhaustive_only_all(
        &mut tests,
        &env,
        t,
        objs![
            ty!(&env, t, {w: b, x: b, y: b, z: b}),
            ty!(&env, t, {w: b, x: b, y: b, z: c}),
            ty!(&env, t, {w: b, x: b, y: b, z: d}),
            ty!(&env, t, {w: b, x: b, y: c, z: b}),
            ty!(&env, t, {w: b, x: b, y: c, z: c}),
            ty!(&env, t, {w: b, x: b, y: c, z: d}),
            ty!(&env, t, {w: b, x: b, y: d, z: b}),
            ty!(&env, t, {w: b, x: b, y: d, z: c}),
            ty!(&env, t, {w: b, x: b, y: d, z: d}),
            ty!(&env, t, {w: b, x: c, y: b, z: b}),
            ty!(&env, t, {w: b, x: c, y: b, z: c}),
            ty!(&env, t, {w: b, x: c, y: b, z: d}),
            ty!(&env, t, {w: b, x: c, y: c, z: b}),
            ty!(&env, t, {w: b, x: c, y: c, z: c}),
            ty!(&env, t, {w: b, x: c, y: c, z: d}),
            ty!(&env, t, {w: b, x: c, y: d, z: b}),
            ty!(&env, t, {w: b, x: c, y: d, z: c}),
            ty!(&env, t, {w: b, x: c, y: d, z: d}),
            ty!(&env, t, {w: b, x: d, y: b, z: b}),
            ty!(&env, t, {w: b, x: d, y: b, z: c}),
            ty!(&env, t, {w: b, x: d, y: b, z: d}),
            ty!(&env, t, {w: b, x: d, y: c, z: b}),
            ty!(&env, t, {w: b, x: d, y: c, z: c}),
            ty!(&env, t, {w: b, x: d, y: c, z: d}),
            ty!(&env, t, {w: b, x: d, y: d, z: b}),
            ty!(&env, t, {w: b, x: d, y: d, z: c}),
            ty!(&env, t, {w: b, x: d, y: d, z: d}),
            ty!(&env, t, {w: c, x: b, y: b, z: b}),
            ty!(&env, t, {w: c, x: b, y: b, z: c}),
            ty!(&env, t, {w: c, x: b, y: b, z: d}),
            ty!(&env, t, {w: c, x: b, y: c, z: b}),
            ty!(&env, t, {w: c, x: b, y: c, z: c}),
            ty!(&env, t, {w: c, x: b, y: c, z: d}),
            ty!(&env, t, {w: c, x: b, y: d, z: b}),
            ty!(&env, t, {w: c, x: b, y: d, z: c}),
            ty!(&env, t, {w: c, x: b, y: d, z: d}),
            ty!(&env, t, {w: c, x: c, y: b, z: b}),
            ty!(&env, t, {w: c, x: c, y: b, z: c}),
            ty!(&env, t, {w: c, x: c, y: b, z: d}),
            ty!(&env, t, {w: c, x: c, y: c, z: b}),
            ty!(&env, t, {w: c, x: c, y: c, z: c}),
            ty!(&env, t, {w: c, x: c, y: c, z: d}),
            ty!(&env, t, {w: c, x: c, y: d, z: b}),
            ty!(&env, t, {w: c, x: c, y: d, z: c}),
            ty!(&env, t, {w: c, x: c, y: d, z: d}),
            ty!(&env, t, {w: c, x: d, y: b, z: b}),
            ty!(&env, t, {w: c, x: d, y: b, z: c}),
            ty!(&env, t, {w: c, x: d, y: b, z: d}),
            ty!(&env, t, {w: c, x: d, y: c, z: b}),
            ty!(&env, t, {w: c, x: d, y: c, z: c}),
            ty!(&env, t, {w: c, x: d, y: c, z: d}),
            ty!(&env, t, {w: c, x: d, y: d, z: b}),
            ty!(&env, t, {w: c, x: d, y: d, z: c}),
            ty!(&env, t, {w: c, x: d, y: d, z: d}),
            ty!(&env, t, {w: d, x: b, y: b, z: b}),
            ty!(&env, t, {w: d, x: b, y: b, z: c}),
            ty!(&env, t, {w: d, x: b, y: b, z: d}),
            ty!(&env, t, {w: d, x: b, y: c, z: b}),
            ty!(&env, t, {w: d, x: b, y: c, z: c}),
            ty!(&env, t, {w: d, x: b, y: c, z: d}),
            ty!(&env, t, {w: d, x: b, y: d, z: b}),
            ty!(&env, t, {w: d, x: b, y: d, z: c}),
            ty!(&env, t, {w: d, x: b, y: d, z: d}),
            ty!(&env, t, {w: d, x: c, y: b, z: b}),
            ty!(&env, t, {w: d, x: c, y: b, z: c}),
            ty!(&env, t, {w: d, x: c, y: b, z: d}),
            ty!(&env, t, {w: d, x: c, y: c, z: b}),
            ty!(&env, t, {w: d, x: c, y: c, z: c}),
            ty!(&env, t, {w: d, x: c, y: c, z: d}),
            ty!(&env, t, {w: d, x: c, y: d, z: b}),
            ty!(&env, t, {w: d, x: c, y: d, z: c}),
            ty!(&env, t, {w: d, x: c, y: d, z: d}),
            ty!(&env, t, {w: d, x: d, y: b, z: b}),
            ty!(&env, t, {w: d, x: d, y: b, z: c}),
            ty!(&env, t, {w: d, x: d, y: b, z: d}),
            ty!(&env, t, {w: d, x: d, y: c, z: b}),
            ty!(&env, t, {w: d, x: d, y: c, z: c}),
            ty!(&env, t, {w: d, x: d, y: c, z: d}),
            ty!(&env, t, {w: d, x: d, y: d, z: b}),
            ty!(&env, t, {w: d, x: d, y: d, z: c}),
            ty!(&env, t, {w: d, x: d, y: d, z: d}),
        ],
    );
    tests.finish();
}

#[test]
fn sealed_transitive_subtypes() {
    let mut tests = Collector::new(FILE, "sealed transitive subtypes |");
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

    let r = env.create_record_type(&[("x", a), ("y", a)]);
    expect_exhaustive_only_all(&mut tests, &env, r, objs![ty!(&env, r, {x: a, y: a}),]);

    expect_exhaustive_only_all(
        &mut tests,
        &env,
        r,
        objs![
            ty!(&env, r, {x: b, y: b}),
            ty!(&env, r, {x: b, y: c}),
            ty!(&env, r, {x: c, y: b}),
            ty!(&env, r, {x: c, y: c}),
        ],
    );

    expect_exhaustive_only_all(
        &mut tests,
        &env,
        r,
        objs![
            ty!(&env, r, {x: b, y: d}),
            ty!(&env, r, {x: b, y: e}),
            ty!(&env, r, {x: b, y: f}),
            ty!(&env, r, {x: c, y: d}),
            ty!(&env, r, {x: c, y: e}),
            ty!(&env, r, {x: c, y: f}),
        ],
    );
    tests.finish();
}

#[test]
fn unsealed_subtypes() {
    let mut tests = Collector::new(FILE, "unsealed subtypes |");
    //    A
    //   / \
    //  B   C
    let env = TestEnvironment::new();
    let a = env.create_class("A", false, &[], &[]);
    let b = env.create_class("B", false, &[a], &[]);
    let c = env.create_class("C", false, &[a], &[]);

    // Not exhaustive even when known subtypes covered.
    let t = env.create_record_type(&[("x", a), ("y", a)]);
    expect_never_exhaustive(
        &mut tests,
        &env,
        t,
        objs![
            ty!(&env, t, {x: b, y: b}),
            ty!(&env, t, {x: b, y: c}),
            ty!(&env, t, {x: c, y: b}),
            ty!(&env, t, {x: c, y: c}),
        ],
    );

    // Exhaustive if field static type is a covered subtype.
    let u = env.create_record_type(&[("x", b), ("y", c)]);
    expect_exhaustive_only_all(&mut tests, &env, u, objs![ty!(&env, u, {x: b, y: c}),]);
    tests.finish();
}

#[test]
fn different_fields() {
    let mut tests = Collector::new(FILE, "different fields |");
    //   (A)
    //   / \
    //  B   C
    let env = TestEnvironment::new();
    let a = env.create_class("A", true, &[], &[]);
    let b = env.create_class("B", false, &[a], &[]);
    let c = env.create_class("C", false, &[a], &[]);
    let r = env.create_record_type(&[("x", a), ("y", a), ("z", a)]);

    expect_never_exhaustive(
        &mut tests,
        &env,
        r,
        objs![
            ty!(&env, r, {x: b}),
            ty!(&env, r, {y: b}),
            ty!(&env, r, {z: b}),
        ],
    );

    expect_exhaustive_only_all(
        &mut tests,
        &env,
        r,
        objs![ty!(&env, r, {x: b, y: a}), ty!(&env, r, {x: c, z: a}),],
    );

    expect_exhaustive_only_all(
        &mut tests,
        &env,
        r,
        objs![
            ty!(&env, r, {x: b, y: b}),
            ty!(&env, r, {x: b, y: c}),
            ty!(&env, r, {x: c, y: b}),
            ty!(&env, r, {x: c, y: c}),
        ],
    );
    tests.finish();
}

#[test]
fn field_types() {
    let mut tests = Collector::new(FILE, "field types |");
    //   (A)
    //   / \
    //  B   C
    let env = TestEnvironment::new();
    let a = env.create_class("A", true, &[], &[]);
    let b = env.create_class("B", false, &[a], &[]);
    let c = env.create_class("C", false, &[a], &[]);
    let r = env.create_record_type(&[("x", a), ("y", b), ("z", c)]);

    expect_exhaustive_only_all(
        &mut tests,
        &env,
        r,
        objs![ty!(&env, r, {x: a, y: b, z: c}),],
    );

    expect_exhaustive_only_all(
        &mut tests,
        &env,
        r,
        objs![ty!(&env, r, {x: b}), ty!(&env, r, {x: c}),],
    );

    expect_exhaustive_only_all(&mut tests, &env, r, objs![ty!(&env, r, {y: b}),]);

    expect_exhaustive_only_all(&mut tests, &env, r, objs![ty!(&env, r, {z: c}),]);
    tests.finish();
}
