// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_test.dart
// (lines 12884-13064: group 'Demotion and type of interest promotion:')

//! Dart group `Demotion and type of interest promotion:`.

use super::common::*;

#[test]
fn partial_demotion() {
    // Promote `Object` to `num`, and then `int`, then assigning a `double`
    // demotes to `num`.
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare(x).with_initializer(expr("Object")),
        x.as_("num"),
        x.as_("int"),
        check_promoted(x, "int"),
        x.write(expr("double")),
        check_promoted(x, "num"),
    ]);
}

#[test]
fn full_demotion() {
    // Promote `Object` to `num` and then `int`, then assigning a `String`
    // demotes to `Object`
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare(x).with_initializer(expr("Object")),
        x.as_("num"),
        x.as_("int"),
        check_promoted(x, "int"),
        x.write(expr("String")),
        check_not_promoted(x),
    ]);
}

mod types_of_interest {
    use super::*;

    #[test]
    fn non_null_declared_is_a_type_of_interest() {
        // Declared type is `num?`; assigning a `num` promotes to `num`.
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("num?")),
            check_not_promoted(x),
            x.write(expr("num")),
            check_promoted(x, "num"),
        ]);
    }

    #[test]
    fn invalid_type_does_not_promote_on_assignment() {
        // Declared type is `num?`; assigning an invalid type does not promote
        // to `num`.
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("num?")),
            check_not_promoted(x),
            x.write(expr("error")),
            check_not_promoted(x),
        ]);
    }

    #[test]
    fn invalid_type_does_not_promote_on_declaration() {
        // Declared type is `num?`; initializing an invalid type does not
        // promote to `num`.
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x)
                .with_declared_type("num?")
                .with_initializer(expr("error")),
            check_not_promoted(x),
        ]);
    }

    #[test]
    fn untested_type_is_not_a_type_of_interest() {
        // Declared type is `Object`; assigning an `int` does not promote.
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("Object")),
            check_not_promoted(x),
            x.write(expr("int")),
            check_not_promoted(x),
        ]);
    }

    #[test]
    fn tested_type_is_a_type_of_interest() {
        // Declared type is `Object`; assigning an `int` after testing `int`
        // promotes.
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("Object")),
            if_else(x.is_("int"), vec![], vec![]),
            check_not_promoted(x),
            x.write(expr("int")),
            check_promoted(x, "int"),
        ]);
    }

    #[test]
    fn non_null_of_tested_type_is_a_type_of_interest() {
        // Declared type is `Object`; assigning an `int` after testing `int?`
        // promotes to `int`.
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("Object")),
            if_else(x.is_("int?"), vec![], vec![]),
            check_not_promoted(x),
            x.write(expr("int")),
            check_promoted(x, "int"),
        ]);
    }
}

mod choosing_among_types_of_interest {
    use super::*;

    #[test]
    fn if_one_type_is_a_subtype_of_all_the_others_it_is_chosen() {
        // Types of interest are `List<num>` and `List<Object>`; writing
        // `List<int>` causes promotion to `List<num>`, because `List<num>` is a
        // subtype of `List<Object>`.
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("Object")),
            if_else(x.is_("List<num>"), vec![], vec![]),
            if_else(x.is_("List<Object>"), vec![], vec![]),
            check_not_promoted(x),
            x.write(expr("List<int>")),
            check_promoted(x, "List<num>"),
        ]);
    }

    #[test]
    fn if_no_type_is_a_subtype_of_all_the_others_no_promotion() {
        // Types of interest are `List<Object?>` and `List<dynamic>`. Since
        // these are mutual subytpes, neither is preferred over the other. So
        // assignment of `List<int>` does not promote, even though both
        // `List<Object?>` and `List<dynamic>` are promotion candidates.
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("Object")),
            if_else(x.is_("List<Object?>"), vec![], vec![]),
            if_else(x.is_("List<dynamic>"), vec![], vec![]),
            check_not_promoted(x),
            x.write(expr("List<int>")),
            check_not_promoted(x),
        ]);
    }

    #[test]
    fn if_a_type_of_interest_matches_exactly_it_is_chosen() {
        // Types of interest are `List<Object?>` and `List<dynamic>`. Since
        // these are mutual subytpes, neither is preferred over the other. But
        // assignment of `List<Object?>` promotes, because it matches one of the
        // types of interest exactly.
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("Object")),
            if_else(x.is_("List<Object?>"), vec![], vec![]),
            if_else(x.is_("List<dynamic>"), vec![], vec![]),
            check_not_promoted(x),
            x.write(expr("List<Object?>")),
            check_promoted(x, "List<Object?>"),
        ]);
    }

    #[test]
    fn only_supertypes_of_written_type_are_considered() {
        // Types of interest are `num` and `String`; writing `int` causes
        // promotion to `num`, because `int` is not a subtype of `String`.
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("Object")),
            if_else(x.is_("num"), vec![], vec![]),
            if_else(x.is_("String"), vec![], vec![]),
            check_not_promoted(x),
            x.write(expr("int")),
            check_promoted(x, "num"),
        ]);
    }

    #[test]
    fn only_subtypes_of_declared_type_are_considered() {
        // Declared type is `List<Object>`. Types of interest are `List<num>`
        // and `List<int?>`, but `List<int?>` is not a subtype of
        // `List<Object>`. Writing `List<int>` (which is a subtype of both types
        // of interest) causes promotion to `List<num>`, because `List<int?>` is
        // not a subtype of the declared type.
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("List<Object>")),
            if_else(x.is_("List<num>"), vec![], vec![]),
            if_else(x.is_("List<int?>"), vec![], vec![]),
            check_not_promoted(x),
            x.write(expr("List<int>")),
            check_promoted(x, "List<num>"),
        ]);
    }
}
