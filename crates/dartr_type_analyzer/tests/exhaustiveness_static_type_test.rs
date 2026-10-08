// Dart source: pkg/_fe_analyzer_shared/test/exhaustiveness/static_type_test.dart

#[macro_use]
mod exhaustiveness_support;

use dartr_type_analyzer::exhaustiveness::{Key, StaticType, StaticTypeArena};
use exhaustiveness_support::{Collector, TestEnvironment, expect_eq};
use indexmap::IndexSet;

const FILE: &str = "static_type_test.dart";

fn run(name: &str, body: impl FnOnce() -> Result<(), String>) {
    let mut c = Collector::new(FILE, "");
    c.test(name, body);
    c.finish();
}

/// `expect(s.isSubtypeOf(t), isTrue/isFalse)`.
fn sub(env: &TestEnvironment, s: StaticType, t: StaticType, expected: bool) -> Result<(), String> {
    let types = env.types();
    if s.is_subtype_of(types, t) == expected {
        Ok(())
    } else {
        Err(format!(
            "Expected {}.isSubtypeOf({}) to be {expected}",
            types.name(s),
            types.name(t)
        ))
    }
}

/// Dart `unorderedEquals`.
fn unordered_equals(
    types: &dyn StaticTypeArena,
    actual: Vec<StaticType>,
    expected: Vec<StaticType>,
) -> Result<(), String> {
    let mut remaining = expected.clone();
    for a in &actual {
        match remaining.iter().position(|e| e == a) {
            Some(i) => {
                remaining.remove(i);
            }
            None => {
                return Err(format!("Unexpected {}", types.name(*a)));
            }
        }
    }
    if remaining.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Missing {:?}",
            remaining.iter().map(|t| types.name(*t)).collect::<Vec<_>>()
        ))
    }
}

#[test]
fn is_subtype_of() {
    let mut c = Collector::new(FILE, "isSubtypeOf() |");
    let env = TestEnvironment::new();
    let a = env.create_class("A", false, &[], &[]);
    let b = env.create_class("B", false, &[a], &[]);
    let b2 = env.create_class("B2", false, &[a], &[]);
    let c_ = env.create_class("C", false, &[b], &[]);
    let d = env.create_class("D", false, &[c_], &[]);

    c.test("subtype includes self", || {
        sub(&env, a, a, true)?;
        sub(&env, b, b, true)?;
        sub(&env, c_, c_, true)?;
        sub(&env, d, d, true)
    });

    c.test("immediate", || {
        sub(&env, a, b, false)?;
        sub(&env, b, a, true)?;

        sub(&env, b, c_, false)?;
        sub(&env, c_, b, true)?;

        sub(&env, c_, d, false)?;
        sub(&env, d, c_, true)
    });

    c.test("transitive", || {
        sub(&env, a, c_, false)?;
        sub(&env, c_, a, true)?;

        sub(&env, b, d, false)?;
        sub(&env, d, b, true)?;

        sub(&env, a, d, false)?;
        sub(&env, d, a, true)
    });

    c.test("unrelated", || {
        sub(&env, b, b2, false)?;
        sub(&env, b2, b, false)?;

        sub(&env, c_, b2, false)?;
        sub(&env, b2, c_, false)?;

        sub(&env, d, b2, false)?;
        sub(&env, b2, d, false)
    });

    c.test("multiple supertypes", || {
        //      I1   I2   I3
        //        \ /  \ /
        //        I12  I23
        //           \/
        //          I123
        let i1 = env.create_class("I1", false, &[], &[]);
        let i2 = env.create_class("I2", false, &[], &[]);
        let i3 = env.create_class("I3", false, &[], &[]);
        let i12 = env.create_class("I12", false, &[i1, i2], &[]);
        let i23 = env.create_class("I23", false, &[i2, i3], &[]);
        let i123 = env.create_class("I123", false, &[i12, i23], &[]);

        sub(&env, i1, i2, false)?;
        sub(&env, i2, i1, false)?;
        sub(&env, i2, i3, false)?;
        sub(&env, i3, i2, false)?;
        sub(&env, i1, i3, false)?;
        sub(&env, i3, i1, false)?;

        sub(&env, i1, i12, false)?;
        sub(&env, i12, i1, true)?;
        sub(&env, i2, i12, false)?;
        sub(&env, i12, i2, true)?;
        sub(&env, i3, i12, false)?;
        sub(&env, i12, i3, false)?;

        sub(&env, i1, i23, false)?;
        sub(&env, i23, i1, false)?;
        sub(&env, i2, i23, false)?;
        sub(&env, i23, i2, true)?;
        sub(&env, i3, i23, false)?;
        sub(&env, i23, i3, true)?;

        sub(&env, i1, i123, false)?;
        sub(&env, i123, i1, true)?;
        sub(&env, i2, i123, false)?;
        sub(&env, i123, i2, true)?;
        sub(&env, i3, i123, false)?;
        sub(&env, i123, i3, true)?;
        sub(&env, i12, i123, false)?;
        sub(&env, i123, i12, true)?;
        sub(&env, i23, i123, false)?;
        sub(&env, i123, i23, true)
    });
    c.finish();
}

#[test]
fn nullable() {
    run("nullable", || {
        let env = TestEnvironment::new();
        let types = env.types();
        let a = env.create_class("A", false, &[], &[]);
        let b = env.create_class("B", false, &[a], &[]);
        let null = StaticType::NULL_TYPE;

        sub(&env, null, a, false)?;
        sub(&env, null, b, false)?;
        sub(&env, null, a.nullable(types), true)?;
        sub(&env, null, b.nullable(types), true)?;

        sub(&env, a, null, false)?;
        sub(&env, b, null, false)?;
        sub(&env, a.nullable(types), null, false)?;
        sub(&env, b.nullable(types), null, false)?;

        sub(&env, a, a.nullable(types), true)?;
        sub(&env, a.nullable(types), a, false)?;
        sub(&env, a.nullable(types), a.nullable(types), true)?;

        sub(&env, a, b.nullable(types), false)?;
        sub(&env, a.nullable(types), b, false)?;
        sub(&env, a.nullable(types), b.nullable(types), false)?;

        sub(&env, b, a.nullable(types), true)?;
        sub(&env, b.nullable(types), a, false)?;
        sub(&env, b.nullable(types), a.nullable(types), true)
    });
}

#[test]
fn fields() {
    run("fields", || {
        let env = TestEnvironment::new();
        let types = env.types();
        let a = env.create_class("A", false, &[], &[]);
        let b = env.create_class("B", false, &[], &[]);
        let c = env.create_class("C", false, &[], &[("x", a), ("y", b)]);
        let d = env.create_class("D", false, &[], &[("w", a)]);
        let e = env.create_class("E", false, &[c, d], &[("z", b)]);

        expect_eq(a.fields(types).len(), 0)?;
        expect_eq(b.fields(types).len(), 0)?;

        expect_eq(c.fields(types).len(), 2)?;
        expect_eq(c.fields(types).get(&Key::name_key("x")).copied(), Some(a))?;
        expect_eq(c.fields(types).get(&Key::name_key("y")).copied(), Some(b))?;

        // Fields are inherited.
        expect_eq(e.fields(types).len(), 4)?;
        expect_eq(e.fields(types).get(&Key::name_key("x")).copied(), Some(a))?;
        expect_eq(e.fields(types).get(&Key::name_key("y")).copied(), Some(b))?;
        expect_eq(e.fields(types).get(&Key::name_key("w")).copied(), Some(a))?;
        expect_eq(e.fields(types).get(&Key::name_key("z")).copied(), Some(b))?;

        // Overridden field types win.
        let f = env.create_class("F", false, &[], &[("x", a)]);
        let g = env.create_class("G", false, &[f], &[("x", b)]);
        expect_eq(g.fields(types).len(), 1)?;
        expect_eq(g.fields(types).get(&Key::name_key("x")).copied(), Some(b))
    });
}

#[test]
fn subtypes() {
    run("subtypes", || {
        let env = TestEnvironment::new();
        let types = env.types();
        let a = env.create_class("A", true, &[], &[]);
        let b = env.create_class("B", false, &[a], &[]);
        let c = env.create_class("C", false, &[a], &[]);
        env.create_class("D", false, &[c], &[]);
        let e = env.create_class("E", true, &[a], &[]);
        let f = env.create_class("F", false, &[e], &[]);

        // Gets subtypes for sealed type.
        let a_subtypes = a.get_subtypes(types, &IndexSet::new());
        unordered_equals(
            types,
            a_subtypes,
            vec![
                types.new_wrapped_static_type(b, a),
                types.new_wrapped_static_type(c, a),
                types.new_wrapped_static_type(e, a),
            ],
        )?;

        // Unsealed subtype.
        let c_subtypes = c.get_subtypes(types, &IndexSet::new());
        unordered_equals(types, c_subtypes, vec![])?;

        // Sealed subtype.
        let e_subtypes = e.get_subtypes(types, &IndexSet::new());
        unordered_equals(types, e_subtypes, vec![types.new_wrapped_static_type(f, e)])
    });
}
