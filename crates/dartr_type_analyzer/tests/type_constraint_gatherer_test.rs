// Dart source: pkg/_fe_analyzer_shared/test/type_inference/type_constraint_gatherer_test.dart

use dartr_mini_ast as mini_ast;

use dartr_flow::shared_type::{SharedTypeSchemaView, Variance};
use dartr_flow::type_analyzer_operations::{TypeAnalyzerOperations, TypeConstraintGenerator};
use mini_ast::mini_type_constraint_gatherer::TypeConstraintGatherer;
use mini_ast::mini_types::*;
use mini_ast::node::Node;

/// Dart `setUp`: `TypeRegistry.init()` plus the interface names the tests use.
/// The returned scope calls `TypeRegistry.uninit()` when dropped.
fn set_up() -> TypeRegistryScope {
    let s = type_registry_scope();
    for n in ["Contravariant", "Invariant", "MyListOfInt", "Unrelated"] {
        TypeRegistry::add_interface_type_name(n);
    }
    s
}

/// Dart `Type('...')`.
fn t(s: &str) -> Type {
    Type::parse(s)
}

/// Dart `check(actual).unorderedEquals(expected)`.
fn unordered_equals(actual: &[String], expected: &[&str]) {
    let mut a: Vec<String> = actual.to_vec();
    let mut e: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
    a.sort();
    e.sort();
    assert_eq!(a, e);
}

mod perform_subtype_constraint_generation_for_function_types {
    use super::*;

    #[test]
    fn matching_functions_with_no_parameters() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&[]);
        assert!(
            tcg.perform_subtype_constraint_generation_for_function_types(
                t("void Function()"),
                t("void Function()"),
                false,
                Some(Node::placeholder())
            )
        );
        assert!(tcg.constraints.is_empty());
    }

    mod matching_functions_with_positional_parameters {
        use super::*;

        #[test]
        fn none_optional() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T", "U"]);
            assert!(
                tcg.perform_subtype_constraint_generation_for_function_types(
                    t("void Function(int, String)"),
                    t("void Function(T, U)"),
                    false,
                    Some(Node::placeholder())
                )
            );
            unordered_equals(&tcg.constraints, &["T <: int", "U <: String"]);
        }

        #[test]
        fn some_optional_on_lhs() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T", "U"]);
            assert!(
                tcg.perform_subtype_constraint_generation_for_function_types(
                    t("void Function(int, [String])"),
                    t("void Function(T, U)"),
                    false,
                    Some(Node::placeholder())
                )
            );
            unordered_equals(&tcg.constraints, &["T <: int", "U <: String"]);
        }
    }

    mod non_matching_functions_with_positional_parameters {
        use super::*;

        #[test]
        fn non_matching_due_to_return_types() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&[]);
            assert!(
                !tcg.perform_subtype_constraint_generation_for_function_types(
                    t("int Function(int)"),
                    t("String Function(int)"),
                    false,
                    Some(Node::placeholder())
                )
            );
            assert!(tcg.constraints.is_empty());
        }

        #[test]
        fn non_matching_due_to_parameter_types() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&[]);
            assert!(
                !tcg.perform_subtype_constraint_generation_for_function_types(
                    t("void Function(int)"),
                    t("void Function(String)"),
                    false,
                    Some(Node::placeholder())
                )
            );
            assert!(tcg.constraints.is_empty());
        }

        #[test]
        fn non_matching_due_to_optional_parameters_on_rhs() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&[]);
            assert!(
                !tcg.perform_subtype_constraint_generation_for_function_types(
                    t("void Function()"),
                    t("void Function([int])"),
                    false,
                    Some(Node::placeholder())
                )
            );
            assert!(tcg.constraints.is_empty());
        }

        #[test]
        fn non_matching_due_to_more_parameters_being_required_on_lhs() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&[]);
            assert!(
                !tcg.perform_subtype_constraint_generation_for_function_types(
                    t("void Function(int)"),
                    t("void Function([int])"),
                    false,
                    Some(Node::placeholder())
                )
            );
            assert!(tcg.constraints.is_empty());
        }
    }

    mod matching_functions_with_named_parameters {
        use super::*;

        #[test]
        fn none_optional() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T", "U"]);
            assert!(
                tcg.perform_subtype_constraint_generation_for_function_types(
                    t("void Function({required int x, required String y})"),
                    t("void Function({required T x, required U y})"),
                    false,
                    Some(Node::placeholder())
                )
            );
            unordered_equals(&tcg.constraints, &["T <: int", "U <: String"]);
        }

        #[test]
        fn some_optional_on_lhs() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T", "U"]);
            assert!(
                tcg.perform_subtype_constraint_generation_for_function_types(
                    t("void Function({required int x, String y})"),
                    t("void Function({required T x, required U y})"),
                    false,
                    Some(Node::placeholder())
                )
            );
            unordered_equals(&tcg.constraints, &["T <: int", "U <: String"]);
        }

        #[test]
        fn optional_named_parameter_on_lhs() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T"]);
            assert!(
                tcg.perform_subtype_constraint_generation_for_function_types(
                    t("void Function(int, {String x})"),
                    t("void Function(T)"),
                    false,
                    Some(Node::placeholder())
                )
            );
            unordered_equals(&tcg.constraints, &["T <: int"]);
        }

        #[test]
        fn extra_optional_named_parameter_on_lhs() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T"]);
            assert!(
                tcg.perform_subtype_constraint_generation_for_function_types(
                    t("void Function({String x, int y})"),
                    t("void Function({T y})"),
                    false,
                    Some(Node::placeholder())
                )
            );
            unordered_equals(&tcg.constraints, &["T <: int"]);
        }
    }

    mod non_matching_functions_with_named_parameters {
        use super::*;

        #[test]
        fn non_matching_due_to_return_types() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&[]);
            assert!(
                !tcg.perform_subtype_constraint_generation_for_function_types(
                    t("int Function({int x})"),
                    t("String Function({int x})"),
                    false,
                    Some(Node::placeholder())
                )
            );
            assert!(tcg.constraints.is_empty());
        }

        #[test]
        fn non_matching_due_to_named_parameter_types() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&[]);
            assert!(
                !tcg.perform_subtype_constraint_generation_for_function_types(
                    t("void Function({int x})"),
                    t("void Function({String x})"),
                    false,
                    Some(Node::placeholder())
                )
            );
            assert!(tcg.constraints.is_empty());
        }

        #[test]
        fn non_matching_due_to_required_named_parameter_on_lhs() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&[]);
            assert!(
                !tcg.perform_subtype_constraint_generation_for_function_types(
                    t("void Function({required int x})"),
                    t("void Function()"),
                    false,
                    Some(Node::placeholder())
                )
            );
            assert!(tcg.constraints.is_empty());
        }

        #[test]
        fn non_matching_due_to_optional_named_parameter_on_rhs() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&[]);
            assert!(
                !tcg.perform_subtype_constraint_generation_for_function_types(
                    t("void Function()"),
                    t("void Function({int x})"),
                    false,
                    Some(Node::placeholder())
                )
            );
            assert!(tcg.constraints.is_empty());
        }

        #[test]
        fn non_matching_due_to_named_parameter_on_rhs_with_decoys_on_lhs() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&[]);
            assert!(
                !tcg.perform_subtype_constraint_generation_for_function_types(
                    t("void Function({int x, int y})"),
                    t("void Function({int z})"),
                    false,
                    Some(Node::placeholder())
                )
            );
            assert!(tcg.constraints.is_empty());
        }
    }

    #[test]
    fn matching_functions_with_named_and_positional_parameters() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&["T", "U"]);
        assert!(
            tcg.perform_subtype_constraint_generation_for_function_types(
                t("void Function(int, {String y})"),
                t("void Function(T, {U y})"),
                false,
                Some(Node::placeholder())
            )
        );
        unordered_equals(&tcg.constraints, &["T <: int", "U <: String"]);
    }

    mod non_matching_functions_with_named_and_positional_parameters {
        use super::*;

        #[test]
        fn non_matching_due_to_lhs_not_accepting_optional_positional_parameter() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&[]);
            assert!(
                !tcg.perform_subtype_constraint_generation_for_function_types(
                    t("void Function(int, {String x})"),
                    t("void Function(int, [String])"),
                    false,
                    Some(Node::placeholder())
                )
            );
            assert!(tcg.constraints.is_empty());
        }

        #[test]
        fn non_matching_due_to_positional_parameter_length_mismatch() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&[]);
            assert!(
                !tcg.perform_subtype_constraint_generation_for_function_types(
                    t("void Function(int, {String x})"),
                    t("void Function(int, String)"),
                    false,
                    Some(Node::placeholder())
                )
            );
            assert!(tcg.constraints.is_empty());
        }
    }
}

mod perform_subtype_constraint_generation_for_record_types {
    use super::*;

    #[test]
    fn matching_empty_records() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&[]);
        assert!(tcg.perform_subtype_constraint_generation_for_record_types(
            t("()"),
            t("()"),
            false,
            Some(Node::placeholder())
        ));
        assert!(tcg.constraints.is_empty());
    }

    mod matching_records {
        use super::*;

        #[test]
        fn without_named_parameters() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T", "U"]);
            assert!(tcg.perform_subtype_constraint_generation_for_record_types(
                t("(int, String)"),
                t("(T, U)"),
                true,
                Some(Node::placeholder())
            ));
            unordered_equals(&tcg.constraints, &["int <: T", "String <: U"]);
        }

        #[test]
        fn with_named_parameters() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T", "U"]);
            assert!(tcg.perform_subtype_constraint_generation_for_record_types(
                t("(int, {String foo})"),
                t("(T, {U foo})"),
                true,
                Some(Node::placeholder())
            ));
            unordered_equals(&tcg.constraints, &["int <: T", "String <: U"]);
        }
    }

    mod non_matching_records_without_named_parameters {
        use super::*;

        #[test]
        fn non_matching_due_to_positional_types() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&[]);
            assert!(!tcg.perform_subtype_constraint_generation_for_record_types(
                t("(int,)"),
                t("(String,)"),
                false,
                Some(Node::placeholder())
            ));
            assert!(tcg.constraints.is_empty());
        }

        #[test]
        fn non_matching_due_to_parameter_numbers() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&[]);
            assert!(
                !tcg.perform_subtype_constraint_generation_for_function_types(
                    t("()"),
                    t("(int,)"),
                    false,
                    Some(Node::placeholder())
                )
            );
            assert!(tcg.constraints.is_empty());
        }

        #[test]
        fn non_matching_due_to_more_parameters_on_lhs() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&[]);
            assert!(
                !tcg.perform_subtype_constraint_generation_for_function_types(
                    t("(int,)"),
                    t("()"),
                    false,
                    Some(Node::placeholder())
                )
            );
            assert!(tcg.constraints.is_empty());
        }
    }

    mod matching_records_with_named_parameters {
        use super::*;

        #[test]
        fn no_type_parameter_occurrences() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T", "U"]);
            assert!(tcg.perform_subtype_constraint_generation_for_record_types(
                t("({int x, String y})"),
                t("({int x, String y})"),
                false,
                Some(Node::placeholder())
            ));
            unordered_equals(&tcg.constraints, &[]);
        }

        #[test]
        fn type_parameters_in_rhs() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T", "U"]);
            assert!(tcg.perform_subtype_constraint_generation_for_record_types(
                t("({int x, String y})"),
                t("({T x, U y})"),
                false,
                Some(Node::placeholder())
            ));
            unordered_equals(&tcg.constraints, &["int <: T", "String <: U"]);
        }

        #[test]
        fn type_parameters_in_lhs() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T", "U"]);
            assert!(tcg.perform_subtype_constraint_generation_for_record_types(
                t("({T x, U y})"),
                t("({int x, String y})"),
                false,
                Some(Node::placeholder())
            ));
            unordered_equals(&tcg.constraints, &["T <: int", "U <: String"]);
        }
    }

    mod matching_records_with_named_parameters_2 {
        use super::*;

        #[test]
        fn no_type_parameter_occurrences() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T", "U"]);
            assert!(tcg.perform_subtype_constraint_generation_for_record_types(
                t("({int x, String y})"),
                t("({int x, String y})"),
                false,
                Some(Node::placeholder())
            ));
            unordered_equals(&tcg.constraints, &[]);
        }

        #[test]
        fn type_parameters_in_rhs() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T", "U"]);
            assert!(tcg.perform_subtype_constraint_generation_for_record_types(
                t("({int x, String y})"),
                t("({T x, U y})"),
                false,
                Some(Node::placeholder())
            ));
            unordered_equals(&tcg.constraints, &["int <: T", "String <: U"]);
        }

        #[test]
        fn type_parameters_in_lhs() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T", "U"]);
            assert!(tcg.perform_subtype_constraint_generation_for_record_types(
                t("({T x, U y})"),
                t("({int x, String y})"),
                false,
                Some(Node::placeholder())
            ));
            unordered_equals(&tcg.constraints, &["T <: int", "U <: String"]);
        }
    }

    mod non_matching_records_with_named_parameters {
        use super::*;

        #[test]
        fn non_matching_due_to_positional_parameter_numbers() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T", "U"]);
            assert!(!tcg.perform_subtype_constraint_generation_for_record_types(
                t("(num, num, {T x, U y})"),
                t("(num, {int x, String y})"),
                false,
                Some(Node::placeholder())
            ));
            assert!(tcg.constraints.is_empty());
        }

        #[test]
        fn non_matching_due_to_named_parameter_numbers() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T", "U"]);
            assert!(!tcg.perform_subtype_constraint_generation_for_record_types(
                t("({T x, U y, num z})"),
                t("({int x, String y})"),
                false,
                Some(Node::placeholder())
            ));
            assert!(tcg.constraints.is_empty());
        }

        #[test]
        fn non_matching_due_to_named_parameter_names() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T", "U"]);
            assert!(!tcg.perform_subtype_constraint_generation_for_record_types(
                t("(num, {T x, U y})"),
                t("(num, {int x, String x2})"),
                false,
                Some(Node::placeholder())
            ));
            assert!(tcg.constraints.is_empty());
        }
    }
}

mod perform_subtype_constraint_generation_for_future_or {
    use super::*;

    #[test]
    fn futureor_matches_futureor_with_constraints_based_on_arguments() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&["T"]);
        assert!(
            tcg.perform_subtype_constraint_generation_for_right_future_or(
                t("FutureOr<T>"),
                t("FutureOr<int>"),
                false,
                Some(Node::placeholder())
            )
        );
        assert_eq!(tcg.constraints, vec!["T <: int"]);
    }

    #[test]
    fn futureor_does_not_match_futureor_because_arguments_fail_to_match() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&[]);
        assert!(
            !tcg.perform_subtype_constraint_generation_for_right_future_or(
                t("FutureOr<int>"),
                t("FutureOr<String>"),
                false,
                Some(Node::placeholder())
            )
        );
        assert!(tcg.constraints.is_empty());
    }

    #[test]
    fn future_matches_futureor_favoring_future_branch() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&["T"]);
        assert!(
            tcg.perform_subtype_constraint_generation_for_right_future_or(
                t("Future<int>"),
                t("FutureOr<T>"),
                false,
                Some(Node::placeholder())
            )
        );
        assert_eq!(tcg.constraints, vec!["int <: T"]);
    }

    #[test]
    fn future_matches_futureor_preferring_to_generate_constraints() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&["T"]);
        assert!(
            tcg.perform_subtype_constraint_generation_for_right_future_or(
                t("Future<_>"),
                t("FutureOr<T>"),
                true,
                Some(Node::placeholder())
            )
        );
        assert_eq!(tcg.constraints, vec!["Future<_> <: T"]);
    }

    #[test]
    fn type_matches_futureor_favoring_the_future_branch() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&["T"]);
        assert!(
            tcg.perform_subtype_constraint_generation_for_right_future_or(
                t("T"),
                t("FutureOr<int>"),
                false,
                Some(Node::placeholder())
            )
        );
        assert_eq!(tcg.constraints, vec!["T <: Future<int>"]);
    }

    #[test]
    fn testing_futureor_as_the_lower_bound_of_the_constraint() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&["T"]);
        assert!(
            tcg.perform_subtype_constraint_generation_for_left_future_or(
                t("FutureOr<T>"),
                t("dynamic"),
                false,
                Some(Node::placeholder())
            )
        );
        assert_eq!(tcg.constraints, vec!["T <: dynamic"]);
    }

    #[test]
    fn futureor_does_not_match_future_in_general() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&["T"]);
        assert!(
            !tcg.perform_subtype_constraint_generation_for_left_future_or(
                t("FutureOr<(T,)>"),
                t("Future<(int,)>"),
                false,
                Some(Node::placeholder())
            )
        );
        assert!(tcg.constraints.is_empty());
    }

    #[test]
    fn testing_nested_futureor_as_the_lower_bound_of_the_constraint() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&["T"]);
        assert!(
            tcg.perform_subtype_constraint_generation_for_left_future_or(
                t("FutureOr<FutureOr<T>>"),
                t("FutureOr<dynamic>"),
                false,
                Some(Node::placeholder())
            )
        );
        assert_eq!(tcg.constraints, vec!["T <: dynamic", "T <: dynamic"]);
    }

    #[test]
    fn future_matches_futureor_with_no_constraints() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&[]);
        assert!(
            tcg.perform_subtype_constraint_generation_for_right_future_or(
                t("Future<int>"),
                t("FutureOr<int>"),
                false,
                Some(Node::placeholder())
            )
        );
        assert!(tcg.constraints.is_empty());
    }

    #[test]
    fn type_matches_futureor_favoring_the_branch_that_matches() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&["T"]);
        assert!(
            tcg.perform_subtype_constraint_generation_for_right_future_or(
                t("List<T>"),
                t("FutureOr<List<int>>"),
                false,
                Some(Node::placeholder())
            )
        );
        assert_eq!(tcg.constraints, vec!["T <: int"]);
    }

    mod nullable_futureor_on_rhs {
        use super::*;

        #[test]
        fn does_not_match_according_to_spec() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T"]);
            assert!(
                !tcg.perform_subtype_constraint_generation_for_right_future_or(
                    t("FutureOr<T>"),
                    t("FutureOr<int>?"),
                    false,
                    Some(Node::placeholder())
                )
            );
            assert!(tcg.constraints.is_empty());
        }

        #[test]
        fn matches_according_to_cfe_discrepancy() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::with_options(&["T"], true);
            assert!(
                tcg.perform_subtype_constraint_generation_for_right_future_or(
                    t("FutureOr<T>"),
                    t("FutureOr<int>?"),
                    false,
                    Some(Node::placeholder())
                )
            );
            assert_eq!(tcg.constraints, vec!["T <: int"]);
        }
    }

    mod nullable_futureor_on_lhs {
        use super::*;

        #[test]
        fn does_not_match_according_to_spec() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T"]);
            assert!(
                !tcg.perform_subtype_constraint_generation_for_right_future_or(
                    t("FutureOr<T>?"),
                    t("FutureOr<int>"),
                    false,
                    Some(Node::placeholder())
                )
            );
            assert!(tcg.constraints.is_empty());
        }

        #[test]
        fn matches_according_to_cfe_discrepancy() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::with_options(&["T"], true);
            assert!(
                tcg.perform_subtype_constraint_generation_for_right_future_or(
                    t("FutureOr<T>?"),
                    t("FutureOr<int>"),
                    false,
                    Some(Node::placeholder())
                )
            );
            assert_eq!(tcg.constraints, vec!["T <: int"]);
        }
    }
}

mod perform_subtype_constraint_generation_for_left_nullable_type {
    use super::*;

    #[test]
    fn nullable_matches_nullable_with_constraints_based_on_base_types() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&["T"]);
        assert!(
            tcg.perform_subtype_constraint_generation_for_left_nullable_type(
                t("T?"),
                t("Null"),
                false,
                Some(Node::placeholder())
            )
        );
        assert_eq!(tcg.constraints, vec!["T <: Null"]);
    }

    #[test]
    fn nullable_does_not_match_nullable_because_base_types_fail_to_match() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&[]);
        assert!(
            !tcg.perform_subtype_constraint_generation_for_left_nullable_type(
                t("int?"),
                t("String?"),
                false,
                Some(Node::placeholder())
            )
        );
        assert!(tcg.constraints.is_empty());
    }

    #[test]
    fn nullable_does_not_match_non_nullable() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&["T"]);
        assert!(
            !tcg.perform_subtype_constraint_generation_for_left_nullable_type(
                t("(int, T)?"),
                t("(int, String)"),
                false,
                Some(Node::placeholder())
            )
        );
        assert!(tcg.constraints.is_empty());
    }

    #[test]
    fn both_lhs_and_rhs_nullable_matching() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&["T"]);
        assert!(
            tcg.perform_subtype_constraint_generation_for_right_nullable_type(
                t("T?"),
                t("int?"),
                false,
                Some(Node::placeholder())
            )
        );
        assert_eq!(tcg.constraints, vec!["T <: int"]);
    }

    #[test]
    fn both_lhs_and_rhs_nullable_not_matching() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&["T"]);
        assert!(
            !tcg.perform_subtype_constraint_generation_for_right_nullable_type(
                t("(T, int)?"),
                t("(int, String)?"),
                false,
                Some(Node::placeholder())
            )
        );
        assert!(tcg.constraints.is_empty());
    }
}

mod perform_subtype_constraint_generation_for_right_nullable_type {
    use super::*;

    #[test]
    fn null_matches_nullable_favoring_non_null_branch() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&["T"]);
        assert!(
            tcg.perform_subtype_constraint_generation_for_right_nullable_type(
                t("Null"),
                t("T?"),
                false,
                Some(Node::placeholder())
            )
        );
        assert_eq!(tcg.constraints, vec!["Null <: T"]);
    }

    #[test]
    fn type_matches_nullable_favoring_the_non_null_branch() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&["T"]);
        assert!(
            tcg.perform_subtype_constraint_generation_for_right_nullable_type(
                t("T"),
                t("int?"),
                false,
                Some(Node::placeholder())
            )
        );
        assert_eq!(tcg.constraints, vec!["T <: int"]);
    }

    #[test]
    fn null_matches_nullable_with_no_constraints() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&[]);
        assert!(
            tcg.perform_subtype_constraint_generation_for_right_nullable_type(
                t("Null"),
                t("int?"),
                false,
                Some(Node::placeholder())
            )
        );
        assert!(tcg.constraints.is_empty());
    }

    #[test]
    fn dynamic_matches_object() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&[]);
        assert!(
            tcg.perform_subtype_constraint_generation_for_right_nullable_type(
                t("dynamic"),
                t("Object?"),
                false,
                Some(Node::placeholder())
            )
        );
        assert!(tcg.constraints.is_empty());
    }

    #[test]
    fn void_matches_object() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&[]);
        assert!(
            tcg.perform_subtype_constraint_generation_for_right_nullable_type(
                t("void"),
                t("Object?"),
                false,
                Some(Node::placeholder())
            )
        );
        assert!(tcg.constraints.is_empty());
    }

    #[test]
    fn lhs_not_nullable_matches_with_no_constraints() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&[]);
        assert!(
            tcg.perform_subtype_constraint_generation_for_right_nullable_type(
                t("int"),
                t("int?"),
                false,
                Some(Node::placeholder())
            )
        );
        assert!(tcg.constraints.is_empty());
    }
}

mod perform_subtype_constraint_generation_for_type_declaration_types {
    use super::*;

    mod same_base_type_on_both_sides {
        use super::*;

        #[test]
        fn covariant_matching() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T", "U"]);
            assert_eq!(
                tcg.perform_subtype_constraint_generation_for_type_declaration_types(
                    t("Map<T, U>"),
                    t("Map<int, String>"),
                    false,
                    Some(Node::placeholder())
                ),
                Some(true)
            );
            unordered_equals(&tcg.constraints, &["T <: int", "U <: String"]);
        }

        #[test]
        fn covariant_not_matching() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T"]);
            assert_eq!(
                tcg.perform_subtype_constraint_generation_for_type_declaration_types(
                    t("Map<T, int>"),
                    t("Map<int, String>"),
                    false,
                    Some(Node::placeholder())
                ),
                Some(false)
            );
            assert!(tcg.constraints.is_empty());
        }

        #[test]
        fn contravariant_matching() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T", "U"]);
            tcg.type_analyzer_operations.add_variance(
                "Contravariant",
                vec![Variance::Contravariant, Variance::Contravariant],
            );
            assert_eq!(
                tcg.perform_subtype_constraint_generation_for_type_declaration_types(
                    t("Contravariant<T, U>"),
                    t("Contravariant<int, String>"),
                    false,
                    Some(Node::placeholder())
                ),
                Some(true)
            );
            unordered_equals(&tcg.constraints, &["int <: T", "String <: U"]);
        }

        #[test]
        fn contravariant_not_matching() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T"]);
            tcg.type_analyzer_operations.add_variance(
                "Contravariant",
                vec![Variance::Contravariant, Variance::Contravariant],
            );
            assert_eq!(
                tcg.perform_subtype_constraint_generation_for_type_declaration_types(
                    t("Contravariant<T, int>"),
                    t("Contravariant<int, String>"),
                    false,
                    Some(Node::placeholder())
                ),
                Some(false)
            );
            assert!(tcg.constraints.is_empty());
        }

        #[test]
        fn invariant_matching() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T", "U"]);
            tcg.type_analyzer_operations
                .add_variance("Invariant", vec![Variance::Invariant, Variance::Invariant]);
            assert_eq!(
                tcg.perform_subtype_constraint_generation_for_type_declaration_types(
                    t("Invariant<T, U>"),
                    t("Invariant<int, String>"),
                    false,
                    Some(Node::placeholder())
                ),
                Some(true)
            );
            unordered_equals(
                &tcg.constraints,
                &["T <: int", "U <: String", "int <: T", "String <: U"],
            );
        }

        #[test]
        fn invariant_not_matching() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T"]);
            tcg.type_analyzer_operations
                .add_variance("Invariant", vec![Variance::Invariant, Variance::Invariant]);
            assert_eq!(
                tcg.perform_subtype_constraint_generation_for_type_declaration_types(
                    t("Invariant<T, int>"),
                    t("Invariant<int, String>"),
                    false,
                    Some(Node::placeholder())
                ),
                Some(false)
            );
            assert!(tcg.constraints.is_empty());
        }

        #[test]
        fn unrelated_matchable() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T", "U"]);
            tcg.type_analyzer_operations
                .add_variance("Unrelated", vec![Variance::Unrelated, Variance::Unrelated]);
            assert_eq!(
                tcg.perform_subtype_constraint_generation_for_type_declaration_types(
                    t("Unrelated<T, U>"),
                    t("Unrelated<int, String>"),
                    false,
                    Some(Node::placeholder())
                ),
                Some(true)
            );
            assert!(tcg.constraints.is_empty());
        }

        #[test]
        fn unrelated_not_matchable() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T"]);
            tcg.type_analyzer_operations
                .add_variance("Unrelated", vec![Variance::Unrelated, Variance::Unrelated]);
            assert_eq!(
                tcg.perform_subtype_constraint_generation_for_type_declaration_types(
                    t("Unrelated<T, int>"),
                    t("Unrelated<int, String>"),
                    false,
                    Some(Node::placeholder())
                ),
                Some(true)
            );
            assert!(tcg.constraints.is_empty());
        }
    }

    mod related_types_on_both_sides {
        use super::*;

        #[test]
        fn no_change_in_type_args() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T"]);
            assert_eq!(
                tcg.perform_subtype_constraint_generation_for_type_declaration_types(
                    t("List<T>"),
                    t("Iterable<int>"),
                    false,
                    Some(Node::placeholder())
                ),
                Some(true)
            );
            assert_eq!(tcg.constraints, vec!["T <: int"]);
        }

        #[test]
        fn change_in_type_args() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T"]);
            assert_eq!(
                tcg.perform_subtype_constraint_generation_for_type_declaration_types(
                    t("MyListOfInt"),
                    t("List<T>"),
                    false,
                    Some(Node::placeholder())
                ),
                Some(true)
            );
            assert_eq!(tcg.constraints, vec!["int <: T"]);
        }

        #[test]
        fn lhs_nullable() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T"]);
            assert_eq!(
                tcg.perform_subtype_constraint_generation_for_type_declaration_types(
                    t("List<T>?"),
                    t("Iterable<int>"),
                    false,
                    Some(Node::placeholder())
                ),
                Some(false)
            );
            assert!(tcg.constraints.is_empty());
        }

        #[test]
        fn rhs_nullable() {
            let _s = set_up();
            let mut tcg = TypeConstraintGatherer::new(&["T"]);
            assert_eq!(
                tcg.perform_subtype_constraint_generation_for_type_declaration_types(
                    t("List<T>"),
                    t("Iterable<int>?"),
                    false,
                    Some(Node::placeholder())
                ),
                Some(false)
            );
            assert!(tcg.constraints.is_empty());
        }
    }

    #[test]
    fn non_interface_type_on_lhs() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&[]);
        assert_eq!(
            tcg.perform_subtype_constraint_generation_for_type_declaration_types(
                t("void Function()"),
                t("int"),
                false,
                Some(Node::placeholder())
            ),
            None
        );
        assert!(tcg.constraints.is_empty());
    }

    #[test]
    fn non_interface_type_on_rhs() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&[]);
        assert_eq!(
            tcg.perform_subtype_constraint_generation_for_type_declaration_types(
                t("int"),
                t("void Function()"),
                false,
                Some(Node::placeholder())
            ),
            None
        );
        assert!(tcg.constraints.is_empty());
    }
}

mod match_type_parameter_bound_internal {
    use super::*;

    #[test]
    fn non_promoted_parameter_on_lhs() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&["T"]);
        let x = TypeRegistry::add_type_parameter("X");
        x.set_explicit_bound(Some(t("Future<String>")));
        assert!(tcg.perform_subtype_constraint_generation_internal(
            TypeParameterType::new(x),
            t("Future<T>"),
            false,
            Some(Node::placeholder())
        ));
        unordered_equals(&tcg.constraints, &["String <: T"]);
    }

    #[test]
    fn promoted_parameter_on_lhs() {
        let _s = set_up();
        let mut tcg = TypeConstraintGatherer::new(&["T"]);
        let x = TypeRegistry::add_type_parameter("X");
        x.set_explicit_bound(Some(t("Object")));
        let ty = TypeParameterType {
            type_parameter: x,
            promotion: Some(t("Future<num>")),
            is_question_type: false,
        }
        .into_type();
        assert!(tcg.perform_subtype_constraint_generation_internal(
            ty,
            t("Future<T>"),
            false,
            Some(Node::placeholder())
        ));
        unordered_equals(&tcg.constraints, &["num <: T"]);
    }
}

mod is_known_type {
    use super::*;

    #[test]
    fn simple_types() {
        let _s = set_up();
        let tcg = TypeConstraintGatherer::new(&["T"]);
        assert!(
            tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t("String")))
        );
        assert!(
            tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t("dynamic")))
        );
        assert!(
            tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t("Object")))
        );
        assert!(
            tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t("void")))
        );
        assert!(
            tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t("T")))
        );
        assert!(
            !tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t("_")))
        );
    }

    #[test]
    fn compound_types() {
        let _s = set_up();
        let tcg = TypeConstraintGatherer::new(&["T"]);
        assert!(
            tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t("List<String>")))
        );
        assert!(
            !tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t("List<_>")))
        );
        assert!(
            tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t("List<List<int>>")))
        );
        assert!(
            !tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t("List<List<_>>")))
        );
        assert!(
            tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t("dynamic Function()")))
        );
        assert!(
            !tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t("_ Function()")))
        );
        assert!(
            tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t("int Function(int)")))
        );
        assert!(
            !tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t("int Function(_)")))
        );
        assert!(
            tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t("int Function({String named})")))
        );
        assert!(
            !tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t("int Function({_ named})")))
        );
        assert!(
            tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t("(int, String, Object)")))
        );
        assert!(
            !tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t("(int, String, _)")))
        );
        assert!(
            tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t(
                    "(int, String, {dynamic named})"
                )))
        );
        assert!(
            !tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t("(int, String, {_ named})")))
        );
        assert!(
            tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t(
                    "(int, String, {List<dynamic> Function(int) named})"
                )))
        );
        assert!(
            !tcg.type_analyzer_operations
                .is_known_type(SharedTypeSchemaView::new(t(
                    "(int, String, {List<_> Function(int) named})"
                )))
        );
    }
}
