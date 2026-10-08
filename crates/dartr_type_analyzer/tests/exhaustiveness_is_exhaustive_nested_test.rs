// Dart source: pkg/_fe_analyzer_shared/test/exhaustiveness/is_exhaustive_nested_test.dart

//! Each Dart `group` is one Rust test; the Dart tests that the helpers of
//! `utils.dart` register in the group are the logical cases of its
//! [Collector].

#[macro_use]
mod exhaustiveness_support;

use exhaustiveness_support::{Collector, TestEnvironment, expect_exhaustive_only_all};

const FILE: &str = "is_exhaustive_nested_test.dart";

#[test]
fn nested_records() {
    let mut tests = Collector::new(FILE, "nested records |");
    //   (A)
    //   / \
    //  B   C
    let env = TestEnvironment::new();
    let a = env.create_class("A", true, &[], &[]);
    let b = env.create_class("B", false, &[a], &[]);
    let c = env.create_class("C", false, &[a], &[]);
    let t = env.create_record_type(&[("x", a), ("y", b)]);
    let u = env.create_record_type(&[("w", t), ("z", t)]);

    expect_exhaustive_only_all(
        &mut tests,
        &env,
        u,
        objs![ty!(&env, u, {
          w: ty!(&env, t, {x: a}),
          z: t,
        }),],
    );

    expect_exhaustive_only_all(
        &mut tests,
        &env,
        u,
        objs![ty!(&env, u, {
          w: ty!(&env, t, {x: a, y: a}),
          z: ty!(&env, t, {x: a, y: a}),
        }),],
    );

    expect_exhaustive_only_all(
        &mut tests,
        &env,
        u,
        objs![ty!(&env, u, {
          w: ty!(&env, t, {x: a, y: b}),
          z: ty!(&env, t, {x: a, y: b}),
        }),],
    );

    expect_exhaustive_only_all(
        &mut tests,
        &env,
        u,
        objs![
            ty!(&env, u, {
              w: ty!(&env, t, {x: b}),
              z: t,
            }),
            ty!(&env, u, {
              w: ty!(&env, t, {x: c}),
              z: t,
            }),
        ],
    );

    expect_exhaustive_only_all(
        &mut tests,
        &env,
        u,
        objs![
            ty!(&env, u, {
              w: ty!(&env, t, {x: b, y: b}),
              z: ty!(&env, t, {x: b, y: b}),
            }),
            ty!(&env, u, {
              w: ty!(&env, t, {x: b, y: b}),
              z: ty!(&env, t, {x: c, y: b}),
            }),
            ty!(&env, u, {
              w: ty!(&env, t, {x: c, y: b}),
              z: ty!(&env, t, {x: b, y: b}),
            }),
            ty!(&env, u, {
              w: ty!(&env, t, {x: c, y: b}),
              z: ty!(&env, t, {x: c, y: b}),
            }),
        ],
    );
    tests.finish();
}

#[test]
fn nested_with_different_fields_of_same_name() {
    let mut tests = Collector::new(FILE, "nested with different fields of same name |");
    // A B C D
    let env = TestEnvironment::new();
    let a = env.create_class("A", false, &[], &[]);
    let b = env.create_record_type(&[("x", a)]);
    let c = env.create_record_type(&[("x", b)]);
    let d = env.create_record_type(&[("x", c)]);

    expect_exhaustive_only_all(
        &mut tests,
        &env,
        d,
        objs![ty!(&env, d, {
          x: ty!(&env, c, {
            x: ty!(&env, b, {x: a}),
          }),
        }),],
    );
    tests.finish();
}
