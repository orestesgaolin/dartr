// Dart source: pkg/_fe_analyzer_shared/test/exhaustiveness/report_errors_test.dart

#[macro_use]
mod exhaustiveness_support;

use dartr_type_analyzer::exhaustiveness::{
    CaseUnreachability, ObjectPropertyLookup, StaticType, compute_exhaustiveness,
};
use exhaustiveness_support::{Collector, Obj, TestEnvironment, expect_eq, parse_spaces};

const FILE: &str = "report_errors_test.dart";

fn expect_exhaustiveness(
    object_field_lookup: &dyn ObjectPropertyLookup,
    value_type: StaticType,
    cases: Vec<Obj>,
    case_is_guarded: Option<Vec<bool>>,
    errors: &str,
) -> Result<(), String> {
    let types = object_field_lookup.static_types();
    let mut case_unreachabilities: Vec<CaseUnreachability> = vec![];
    let case_spaces = parse_spaces(types, &cases);
    let non_exhaustiveness = compute_exhaustiveness(
        object_field_lookup,
        value_type,
        &case_is_guarded.unwrap_or_else(|| vec![false; cases.len()]),
        &case_spaces,
        Some(&mut case_unreachabilities),
    );
    let mut lines: Vec<String> = case_unreachabilities
        .iter()
        .map(|u| u.to_text(types))
        .collect();
    if let Some(non_exhaustiveness) = &non_exhaustiveness {
        lines.push(non_exhaustiveness.to_text(types));
    }
    expect_eq(lines.join("\n").as_str(), errors)
}

#[test]
fn sealed() {
    let mut c = Collector::new(FILE, "sealed | ");
    // Here, "(_)" means "sealed". A bare name is unsealed.
    //
    //     (A)
    //     / \
    //   (B) (C)
    //   / \   \
    //  D   E   F
    //         / \
    //        G   H
    let env = TestEnvironment::new();
    let a = env.create_class("A", true, &[], &[]);
    let b = env.create_class("B", true, &[a], &[]);
    let c_ = env.create_class("C", true, &[a], &[]);
    let d = env.create_class("D", false, &[b], &[]);
    let e = env.create_class("E", false, &[b], &[]);
    let f = env.create_class("F", false, &[c_], &[]);
    let g = env.create_class("G", false, &[f], &[]);
    let h = env.create_class("H", false, &[f], &[]);

    c.test("exhaustiveness", || {
        // Case matching top type covers all subtypes.
        expect_exhaustiveness(&env, a, objs![a], None, "")?;
        expect_exhaustiveness(&env, b, objs![a], None, "")?;
        expect_exhaustiveness(&env, d, objs![a], None, "")?;

        // Case matching subtype doesn't cover supertype.
        expect_exhaustiveness(
            &env,
            a,
            objs![b],
            None,
            "A is not exhaustively matched by B.",
        )?;
        expect_exhaustiveness(&env, b, objs![b], None, "")?;
        expect_exhaustiveness(&env, d, objs![b], None, "")?;
        expect_exhaustiveness(&env, e, objs![b], None, "")?;

        // Matching subtypes of sealed type is exhaustive.
        expect_exhaustiveness(&env, a, objs![b, c_], None, "")?;
        expect_exhaustiveness(&env, a, objs![d, e, f], None, "")?;
        expect_exhaustiveness(&env, a, objs![b, f], None, "")?;
        expect_exhaustiveness(
            &env,
            a,
            objs![c_, d],
            None,
            "A is not exhaustively matched by C|D.",
        )?;
        expect_exhaustiveness(
            &env,
            f,
            objs![g, h],
            None,
            "F is not exhaustively matched by G|H.",
        )
    });

    c.test("unreachable case", || {
        // Same type.
        expect_exhaustiveness(&env, b, objs![b, b], None, "Case #2 B is unreachable.")?;

        // Previous case is supertype.
        expect_exhaustiveness(&env, b, objs![a, b], None, "Case #2 B is unreachable.")?;

        // Previous subtype cases cover sealed supertype.
        expect_exhaustiveness(&env, a, objs![b, c_, a], None, "Case #3 A is unreachable.")?;
        expect_exhaustiveness(
            &env,
            a,
            objs![d, e, f, a],
            None,
            "Case #4 A is unreachable.",
        )?;
        expect_exhaustiveness(&env, a, objs![b, f, a], None, "Case #3 A is unreachable.")?;
        expect_exhaustiveness(&env, a, objs![c_, d, a], None, "")?;

        // Previous subtype cases do not cover unsealed supertype.
        expect_exhaustiveness(&env, f, objs![g, h, f], None, "")?;

        // Guarded case is reachable
        expect_exhaustiveness(&env, b, objs![d, e, d], Some(vec![true, false, false]), "")?;

        // Guarded case is unreachable
        expect_exhaustiveness(
            &env,
            b,
            objs![d, e, d],
            Some(vec![false, false, true]),
            "Case #3 D is unreachable.",
        )
    });

    c.test("covered record destructuring |", || {
        let r = env.create_record_type(&[("x", a), ("y", a), ("z", a)]);

        // Wider field is not covered.
        expect_exhaustiveness(
            &env,
            r,
            objs![ty!(&env, r, { x: b }), ty!(&env, r, { x: a })],
            None,
            "",
        )?;

        // Narrower field is covered.
        expect_exhaustiveness(
            &env,
            r,
            objs![ty!(&env, r, { x: a }), ty!(&env, r, { x: b })],
            None,
            "Case #2 (x: B, y: A, z: A) is unreachable.",
        )
    });

    c.test("nullable sealed |", || {
        //     (A)
        //     / \
        //    B  (C)
        //       / \
        //      D   E
        let env = TestEnvironment::new();
        let types = env.types();
        let a = env.create_class("A", true, &[], &[]);
        let b = env.create_class("B", false, &[a], &[]);
        let c = env.create_class("C", true, &[a], &[]);
        let d = env.create_class("D", false, &[c], &[]);
        let e = env.create_class("E", false, &[c], &[]);
        let null = StaticType::NULL_TYPE;

        // Must cover null.
        expect_exhaustiveness(
            &env,
            a.nullable(types),
            objs![b, d, e],
            None,
            "A? is not exhaustively matched by B|D|E.",
        )?;

        // Can cover null with any nullable subtype.
        expect_exhaustiveness(
            &env,
            a.nullable(types),
            objs![b.nullable(types), c],
            None,
            "",
        )?;
        expect_exhaustiveness(
            &env,
            a.nullable(types),
            objs![b, c.nullable(types)],
            None,
            "",
        )?;
        expect_exhaustiveness(
            &env,
            a.nullable(types),
            objs![b, d.nullable(types), e],
            None,
            "",
        )?;
        expect_exhaustiveness(
            &env,
            a.nullable(types),
            objs![b, d, e.nullable(types)],
            None,
            "",
        )?;

        // Can cover null with a null space.
        expect_exhaustiveness(&env, a.nullable(types), objs![b, c, null], None, "")?;
        expect_exhaustiveness(&env, a.nullable(types), objs![b, d, e, null], None, "")?;

        // Nullable covers the non-null.
        expect_exhaustiveness(
            &env,
            a.nullable(types),
            objs![a.nullable(types), a],
            None,
            "Case #2 A is unreachable.",
        )?;
        expect_exhaustiveness(
            &env,
            b.nullable(types),
            objs![a.nullable(types), b],
            None,
            "Case #2 B is unreachable.",
        )?;

        // Nullable covers null.
        expect_exhaustiveness(
            &env,
            a.nullable(types),
            objs![a.nullable(types), null],
            None,
            "Case #2 Null is unreachable.",
        )?;
        expect_exhaustiveness(
            &env,
            b.nullable(types),
            objs![a.nullable(types), null],
            None,
            "Case #2 Null is unreachable.",
        )
    });
    c.finish();
}
