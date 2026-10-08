// Dart source: pkg/_fe_analyzer_shared/test/mini_types_test.dart

mod mini_ast;

use mini_ast::mini_types::*;
use std::collections::{HashMap, HashSet};

struct Fixture {
    _scope: TypeRegistryScope,
    t: TypeParameter,
    u: TypeParameter,
    v: TypeParameter,
}

fn set_up() -> Fixture {
    let scope = type_registry_scope();
    Fixture {
        t: TypeRegistry::add_type_parameter("T"),
        u: TypeRegistry::add_type_parameter("U"),
        v: TypeRegistry::add_type_parameter("V"),
        _scope: scope,
    }
}

/// Dart `Type('...')`.
fn ty(s: &str) -> Type {
    Type::parse(s)
}

mod to_string {
    use super::*;

    mod function_type {
        use super::*;

        mod positional_parameters {
            use super::*;

            #[test]
            fn all_required() {
                let f = set_up();
                assert_eq!(
                    FunctionType::new(
                        TypeParameterType::new(f.t),
                        vec![TypeParameterType::new(f.u), TypeParameterType::new(f.v)],
                    )
                    .into_type()
                    .to_string(),
                    "T Function(U, V)"
                );
            }

            #[test]
            fn all_optional() {
                let f = set_up();
                assert_eq!(
                    FunctionType::new(
                        TypeParameterType::new(f.t),
                        vec![TypeParameterType::new(f.u), TypeParameterType::new(f.v)],
                    )
                    .with_required_positional_parameter_count(0)
                    .into_type()
                    .to_string(),
                    "T Function([U, V])"
                );
            }

            #[test]
            fn mixed_required_and_optional() {
                let f = set_up();
                assert_eq!(
                    FunctionType::new(
                        TypeParameterType::new(f.t),
                        vec![TypeParameterType::new(f.u), TypeParameterType::new(f.v)],
                    )
                    .with_required_positional_parameter_count(1)
                    .into_type()
                    .to_string(),
                    "T Function(U, [V])"
                );
            }
        }

        #[test]
        fn named_parameters() {
            let f = set_up();
            assert_eq!(
                FunctionType::new(TypeParameterType::new(f.t), vec![])
                    .with_named_parameters(vec![
                        NamedFunctionParameter::new(false, "x", TypeParameterType::new(f.u)),
                        NamedFunctionParameter::new(true, "y", TypeParameterType::new(f.v)),
                    ])
                    .into_type()
                    .to_string(),
                "T Function({U x, required V y})"
            );
        }

        #[test]
        fn positional_and_named_parameters() {
            let f = set_up();
            assert_eq!(
                FunctionType::new(
                    TypeParameterType::new(f.t),
                    vec![TypeParameterType::new(f.u)]
                )
                .with_named_parameters(vec![NamedFunctionParameter::new(
                    false,
                    "y",
                    TypeParameterType::new(f.v)
                )])
                .into_type()
                .to_string(),
                "T Function(U, {V y})"
            );
        }

        #[test]
        fn type_formals_unbounded() {
            let f = set_up();
            assert_eq!(
                FunctionType::new(VoidType::instance(), vec![])
                    .with_type_parameters(vec![f.t, f.u])
                    .into_type()
                    .to_string(),
                "void Function<T, U>()"
            );
        }

        #[test]
        fn type_formals_bounded() {
            let f = set_up();
            f.t.set_explicit_bound(Some(TypeParameterType::new(f.u)));
            assert_eq!(
                FunctionType::new(VoidType::instance(), vec![])
                    .with_type_parameters(vec![f.t, f.u])
                    .into_type()
                    .to_string(),
                "void Function<T extends U, U>()"
            );
        }

        #[test]
        fn needs_parentheses() {
            let f = set_up();
            assert_eq!(
                TypeParameterType {
                    type_parameter: f.t,
                    promotion: Some(FunctionType::new(VoidType::instance(), vec![]).into_type()),
                    is_question_type: false,
                }
                .into_type()
                .to_string(),
                "T&(void Function())"
            );
        }
    }

    mod primary_type {
        use super::*;

        #[test]
        fn simple() {
            let f = set_up();
            assert_eq!(TypeParameterType::new(f.t).to_string(), "T");
        }

        #[test]
        fn with_arguments() {
            let f = set_up();
            assert_eq!(
                PrimaryType::new(
                    TypeRegistry::map(),
                    vec![TypeParameterType::new(f.t), TypeParameterType::new(f.u)],
                )
                .to_string(),
                "Map<T, U>"
            );
        }
    }

    mod promoted_type_variable_type {
        use super::*;

        #[test]
        fn basic() {
            let f = set_up();
            assert_eq!(
                TypeParameterType {
                    type_parameter: f.t,
                    promotion: Some(TypeParameterType::new(f.u)),
                    is_question_type: false,
                }
                .into_type()
                .to_string(),
                "T&U"
            );
        }

        #[test]
        fn needs_parentheses_right() {
            let f = set_up();
            assert_eq!(
                TypeParameterType {
                    type_parameter: f.t,
                    promotion: Some(
                        TypeParameterType {
                            type_parameter: f.u,
                            promotion: Some(TypeParameterType::new(f.v)),
                            is_question_type: false,
                        }
                        .into_type()
                    ),
                    is_question_type: false,
                }
                .into_type()
                .to_string(),
                "T&(U&V)"
            );
        }

        #[test]
        fn needs_parentheses_question() {
            let f = set_up();
            assert_eq!(
                TypeParameterType {
                    type_parameter: f.t,
                    promotion: Some(TypeParameterType::new(f.u)),
                    is_question_type: true,
                }
                .into_type()
                .to_string(),
                "(T&U)?"
            );
        }
    }

    mod question_type {
        use super::*;

        #[test]
        fn basic() {
            let f = set_up();
            assert_eq!(
                TypeParameterType {
                    type_parameter: f.t,
                    promotion: None,
                    is_question_type: true,
                }
                .into_type()
                .to_string(),
                "T?"
            );
        }

        #[test]
        fn needs_parentheses() {
            let f = set_up();
            assert_eq!(
                TypeParameterType {
                    type_parameter: f.t,
                    promotion: Some(
                        TypeParameterType {
                            type_parameter: f.u,
                            promotion: None,
                            is_question_type: true,
                        }
                        .into_type()
                    ),
                    is_question_type: false,
                }
                .into_type()
                .to_string(),
                "T&(U?)"
            );
        }
    }

    mod record_type {
        use super::*;

        #[test]
        fn no_arguments() {
            let _f = set_up();
            assert_eq!(RecordType::new(vec![], vec![]).to_string(), "()");
        }

        #[test]
        fn single_positional_argument() {
            let f = set_up();
            assert_eq!(
                RecordType::new(vec![TypeParameterType::new(f.t)], vec![]).to_string(),
                "(T,)"
            );
        }

        #[test]
        fn multiple_positional_arguments() {
            let f = set_up();
            assert_eq!(
                RecordType::new(
                    vec![TypeParameterType::new(f.t), TypeParameterType::new(f.u)],
                    vec![]
                )
                .to_string(),
                "(T, U)"
            );
        }

        #[test]
        fn single_named_argument() {
            let f = set_up();
            assert_eq!(
                RecordType::new(
                    vec![],
                    vec![NamedType::new("t", TypeParameterType::new(f.t))]
                )
                .to_string(),
                "({T t})"
            );
        }

        #[test]
        fn multiple_named_arguments() {
            let f = set_up();
            assert_eq!(
                RecordType::new(
                    vec![],
                    vec![
                        NamedType::new("t", TypeParameterType::new(f.t)),
                        NamedType::new("u", TypeParameterType::new(f.u)),
                    ]
                )
                .to_string(),
                "({T t, U u})"
            );
        }

        #[test]
        fn both_positional_and_named_arguments() {
            let f = set_up();
            assert_eq!(
                RecordType::new(
                    vec![TypeParameterType::new(f.t)],
                    vec![NamedType::new("u", TypeParameterType::new(f.u))]
                )
                .to_string(),
                "(T, {U u})"
            );
        }
    }

    #[test]
    fn unknown_type() {
        let _f = set_up();
        assert_eq!(UnknownType::new().to_string(), "_");
    }
}

mod parse {
    use super::*;

    fn throws_parse_error(s: &str) {
        assert!(Type::try_parse(s).is_err(), "expected ParseError for {s:?}");
    }

    mod primary_type {
        use super::*;

        #[test]
        fn no_type_args() {
            let _f = set_up();
            let type_ = ty("int").as_primary_type().unwrap();
            assert_eq!(type_.name(), "int");
            assert!(type_.args.is_empty());
        }

        #[test]
        fn type_arg() {
            let _f = set_up();
            let type_ = ty("List<int>").as_primary_type().unwrap();
            assert_eq!(type_.name(), "List");
            assert_eq!(type_.args.len(), 1);
            assert_eq!(type_.args[0].to_string(), "int");
        }

        #[test]
        fn type_args() {
            let _f = set_up();
            let type_ = ty("Map<int, String>").as_primary_type().unwrap();
            assert_eq!(type_.name(), "Map");
            assert_eq!(type_.args.len(), 2);
            assert_eq!(type_.args[0].to_string(), "int");
            assert_eq!(type_.args[1].to_string(), "String");
        }

        #[test]
        fn invalid_type_arg_separator() {
            let _f = set_up();
            throws_parse_error("Map<int) String>");
        }

        #[test]
        fn dynamic() {
            let _f = set_up();
            assert!(ty("dynamic").identical(DynamicType::instance()));
        }

        #[test]
        fn error() {
            let _f = set_up();
            assert!(ty("error").identical(InvalidType::instance()));
        }

        #[test]
        fn future_or() {
            let _f = set_up();
            let type_ = ty("FutureOr<int>");
            assert_eq!(type_.future_or_type_argument().unwrap().to_string(), "int");
        }

        #[test]
        fn never() {
            let _f = set_up();
            assert!(ty("Never").identical(NeverType::instance()));
        }

        #[test]
        fn null() {
            let _f = set_up();
            assert!(ty("Null").identical(NullType::instance()));
        }

        #[test]
        fn void() {
            let _f = set_up();
            assert!(ty("void").identical(VoidType::instance()));
        }
    }

    #[test]
    fn invalid_initial_token() {
        let _f = set_up();
        throws_parse_error("<");
    }

    #[test]
    fn unknown_type() {
        let _f = set_up();
        let type_ = ty("_");
        assert!(type_.as_unknown_type().is_some());
    }

    #[test]
    fn question_type() {
        let _f = set_up();
        let type_ = ty("int?");
        assert!(type_.is_question_type());
        assert_eq!(type_.as_question_type(false).to_string(), "int");
    }

    #[test]
    fn promoted_type_variable() {
        let f = set_up();
        let type_ = ty("T&int").as_type_parameter_type().unwrap();
        assert!(type_.type_parameter == f.t);
        assert_eq!(type_.promotion.unwrap().to_string(), "int");
    }

    #[test]
    fn parenthesized_type() {
        let _f = set_up();
        let type_ = ty("(int)");
        assert_eq!(type_.to_string(), "int");
    }

    #[test]
    fn invalid_token_terminating_parenthesized_type() {
        let _f = set_up();
        throws_parse_error("(?<");
    }

    mod function_type {
        use super::*;

        #[test]
        fn no_parameters() {
            let _f = set_up();
            let type_ = ty("int Function()").as_function_type().unwrap();
            assert_eq!(type_.return_type.to_string(), "int");
            assert!(type_.positional_parameters.is_empty());
            assert_eq!(type_.required_positional_parameter_count, 0);
            assert!(type_.named_parameters.is_empty());
        }

        #[test]
        fn required_positional_parameter() {
            let _f = set_up();
            let type_ = ty("int Function(String)").as_function_type().unwrap();
            assert_eq!(type_.return_type.to_string(), "int");
            assert_eq!(type_.positional_parameters.len(), 1);
            assert_eq!(type_.positional_parameters[0].to_string(), "String");
            assert_eq!(type_.required_positional_parameter_count, 1);
            assert!(type_.named_parameters.is_empty());
        }

        #[test]
        fn required_positional_parameters() {
            let _f = set_up();
            let type_ = ty("int Function(String, double)")
                .as_function_type()
                .unwrap();
            assert_eq!(type_.return_type.to_string(), "int");
            assert_eq!(type_.positional_parameters.len(), 2);
            assert_eq!(type_.positional_parameters[0].to_string(), "String");
            assert_eq!(type_.positional_parameters[1].to_string(), "double");
            assert_eq!(type_.required_positional_parameter_count, 2);
            assert!(type_.named_parameters.is_empty());
        }

        #[test]
        fn optional_positional_parameter() {
            let _f = set_up();
            let type_ = ty("int Function([String])").as_function_type().unwrap();
            assert_eq!(type_.return_type.to_string(), "int");
            assert_eq!(type_.positional_parameters.len(), 1);
            assert_eq!(type_.positional_parameters[0].to_string(), "String");
            assert_eq!(type_.required_positional_parameter_count, 0);
            assert!(type_.named_parameters.is_empty());
        }

        #[test]
        fn optional_positional_parameters() {
            let _f = set_up();
            let type_ = ty("int Function([String, double])")
                .as_function_type()
                .unwrap();
            assert_eq!(type_.return_type.to_string(), "int");
            assert_eq!(type_.positional_parameters.len(), 2);
            assert_eq!(type_.positional_parameters[0].to_string(), "String");
            assert_eq!(type_.positional_parameters[1].to_string(), "double");
            assert_eq!(type_.required_positional_parameter_count, 0);
            assert!(type_.named_parameters.is_empty());
        }

        mod named_parameter {
            use super::*;

            #[test]
            fn not_required() {
                let _f = set_up();
                let type_ = ty("int Function({String x})").as_function_type().unwrap();
                assert_eq!(type_.return_type.to_string(), "int");
                assert!(type_.positional_parameters.is_empty());
                assert_eq!(type_.required_positional_parameter_count, 0);
                assert_eq!(type_.named_parameters.len(), 1);
                assert!(!type_.named_parameters[0].is_required);
                assert_eq!(type_.named_parameters[0].ty.to_string(), "String");
                assert_eq!(type_.named_parameters[0].name, "x");
            }

            #[test]
            fn required() {
                let _f = set_up();
                let type_ = ty("int Function({required String x})")
                    .as_function_type()
                    .unwrap();
                assert_eq!(type_.return_type.to_string(), "int");
                assert!(type_.positional_parameters.is_empty());
                assert_eq!(type_.required_positional_parameter_count, 0);
                assert_eq!(type_.named_parameters.len(), 1);
                assert!(type_.named_parameters[0].is_required);
                assert_eq!(type_.named_parameters[0].ty.to_string(), "String");
                assert_eq!(type_.named_parameters[0].name, "x");
            }
        }

        #[test]
        fn named_parameters() {
            let _f = set_up();
            let type_ = ty("int Function({String x, double y})")
                .as_function_type()
                .unwrap();
            assert_eq!(type_.return_type.to_string(), "int");
            assert!(type_.positional_parameters.is_empty());
            assert_eq!(type_.required_positional_parameter_count, 0);
            assert_eq!(type_.named_parameters.len(), 2);
            assert!(!type_.named_parameters[0].is_required);
            assert_eq!(type_.named_parameters[0].ty.to_string(), "String");
            assert_eq!(type_.named_parameters[0].name, "x");
            assert!(!type_.named_parameters[1].is_required);
            assert_eq!(type_.named_parameters[1].ty.to_string(), "double");
            assert_eq!(type_.named_parameters[1].name, "y");
        }

        #[test]
        fn named_parameter_sorting() {
            let _f = set_up();
            let type_ = ty("int Function({double y, String x})")
                .as_function_type()
                .unwrap();
            assert_eq!(type_.return_type.to_string(), "int");
            assert!(type_.positional_parameters.is_empty());
            assert_eq!(type_.required_positional_parameter_count, 0);
            assert_eq!(type_.named_parameters.len(), 2);
            assert!(!type_.named_parameters[0].is_required);
            assert_eq!(type_.named_parameters[0].ty.to_string(), "String");
            assert_eq!(type_.named_parameters[0].name, "x");
            assert!(!type_.named_parameters[1].is_required);
            assert_eq!(type_.named_parameters[1].ty.to_string(), "double");
            assert_eq!(type_.named_parameters[1].name, "y");
        }

        mod type_formals {
            use super::*;

            #[test]
            fn single() {
                let _f = set_up();
                let type_ = ty("int Function<T>()").as_function_type().unwrap();
                assert_eq!(type_.type_parameters_shared.len(), 1);
                assert_eq!(type_.type_parameters_shared[0].name(), "T");
            }

            #[test]
            fn multiple() {
                let _f = set_up();
                let type_ = ty("int Function<T, U>()").as_function_type().unwrap();
                assert_eq!(type_.type_parameters_shared.len(), 2);
                assert_eq!(type_.type_parameters_shared[0].name(), "T");
                assert_eq!(type_.type_parameters_shared[1].name(), "U");
            }

            #[test]
            fn return_type_and_parameters_can_refer_to_type_formal() {
                let _f = set_up();
                let type_ = ty("T Function<T>(T, {T t})").as_function_type().unwrap();
                assert_eq!(type_.type_parameters_shared.len(), 1);
                let t = type_.type_parameters_shared[0];
                assert!(
                    type_
                        .return_type
                        .as_type_parameter_type()
                        .unwrap()
                        .type_parameter
                        == t
                );
                assert_eq!(type_.positional_parameters.len(), 1);
                assert!(
                    type_.positional_parameters[0]
                        .as_type_parameter_type()
                        .unwrap()
                        .type_parameter
                        == t
                );
                assert_eq!(type_.named_parameters.len(), 1);
                assert!(
                    type_.named_parameters[0]
                        .ty
                        .as_type_parameter_type()
                        .unwrap()
                        .type_parameter
                        == t
                );
            }

            #[test]
            fn unbounded() {
                let _f = set_up();
                let type_ = ty("void Function<T>()").as_function_type().unwrap();
                assert_eq!(type_.type_parameters_shared.len(), 1);
                let t = type_.type_parameters_shared[0];
                assert!(t.explicit_bound().is_none());
            }

            #[test]
            fn bounded() {
                let _f = set_up();
                let type_ = ty("void Function<T extends Object>()")
                    .as_function_type()
                    .unwrap();
                assert_eq!(type_.type_parameters_shared.len(), 1);
                let t = type_.type_parameters_shared[0];
                assert_eq!(t.explicit_bound().unwrap().to_string(), "Object");
            }

            #[test]
            fn f_bounded() {
                let _f = set_up();
                let type_ = ty("void Function<T extends U, U>()")
                    .as_function_type()
                    .unwrap();
                let t = type_.type_parameters_shared[0];
                let u = type_.type_parameters_shared[1];
                assert!(
                    t.explicit_bound()
                        .unwrap()
                        .as_type_parameter_type()
                        .unwrap()
                        .type_parameter
                        == u
                );
            }

            #[test]
            fn invalid_token_in_type_formals() {
                let _f = set_up();
                throws_parse_error("int Function<{>()");
            }

            #[test]
            fn invalid_token_at_end_of_type_formals() {
                let _f = set_up();
                throws_parse_error("int Function<T}()");
            }
        }

        #[test]
        fn invalid_parameter_separator() {
            let _f = set_up();
            throws_parse_error("int Function(String Function()< double)");
        }

        #[test]
        fn invalid_token_after_function() {
            let _f = set_up();
            throws_parse_error("int Function&)");
        }
    }

    mod record_type {
        use super::*;

        #[test]
        fn no_fields() {
            let _f = set_up();
            let type_ = ty("()").as_record_type().unwrap();
            assert!(type_.positional_types.is_empty());
            assert!(type_.named_types.is_empty());
        }

        #[test]
        fn named_field() {
            let _f = set_up();
            let type_ = ty("({int x})").as_record_type().unwrap();
            assert!(type_.positional_types.is_empty());
            assert_eq!(type_.named_types.len(), 1);
            assert_eq!(type_.named_types[0].name, "x");
            assert_eq!(type_.named_types[0].ty.to_string(), "int");
        }

        #[test]
        fn named_field_followed_by_comma() {
            let _f = set_up();
            let type_ = ty("({int x,})").as_record_type().unwrap();
            assert!(type_.positional_types.is_empty());
            assert_eq!(type_.named_types.len(), 1);
            assert_eq!(type_.named_types[0].name, "x");
            assert_eq!(type_.named_types[0].ty.to_string(), "int");
        }

        #[test]
        fn named_field_followed_by_invalid_token() {
            let _f = set_up();
            throws_parse_error("({int x))");
        }

        #[test]
        fn named_field_name_is_not_an_identifier() {
            let _f = set_up();
            throws_parse_error("({int )})");
        }

        #[test]
        fn named_fields() {
            let _f = set_up();
            let type_ = ty("({int x, String y})").as_record_type().unwrap();
            assert!(type_.positional_types.is_empty());
            assert_eq!(type_.named_types.len(), 2);
            assert_eq!(type_.named_types[0].name, "x");
            assert_eq!(type_.named_types[0].ty.to_string(), "int");
            assert_eq!(type_.named_types[1].name, "y");
            assert_eq!(type_.named_types[1].ty.to_string(), "String");
        }

        #[test]
        fn curly_braces_followed_by_invalid_token() {
            let _f = set_up();
            throws_parse_error("({int x}&");
        }

        #[test]
        fn curly_braces_but_no_named_fields() {
            let _f = set_up();
            throws_parse_error("({})");
        }

        #[test]
        fn positional_field() {
            let _f = set_up();
            let type_ = ty("(int,)").as_record_type().unwrap();
            assert!(type_.named_types.is_empty());
            assert_eq!(type_.positional_types.len(), 1);
            assert_eq!(type_.positional_types[0].to_string(), "int");
        }

        mod positional_fields {
            use super::*;

            #[test]
            fn two() {
                let _f = set_up();
                let type_ = ty("(int, String)").as_record_type().unwrap();
                assert!(type_.named_types.is_empty());
                assert_eq!(type_.positional_types.len(), 2);
                assert_eq!(type_.positional_types[0].to_string(), "int");
                assert_eq!(type_.positional_types[1].to_string(), "String");
            }

            #[test]
            fn three() {
                let _f = set_up();
                let type_ = ty("(int, String, double)").as_record_type().unwrap();
                assert!(type_.named_types.is_empty());
                assert_eq!(type_.positional_types.len(), 3);
                assert_eq!(type_.positional_types[0].to_string(), "int");
                assert_eq!(type_.positional_types[1].to_string(), "String");
                assert_eq!(type_.positional_types[2].to_string(), "double");
            }
        }

        #[test]
        fn named_and_positional_fields() {
            let _f = set_up();
            let type_ = ty("(int, {String x})").as_record_type().unwrap();
            assert_eq!(type_.positional_types.len(), 1);
            assert_eq!(type_.positional_types[0].to_string(), "int");
            assert_eq!(type_.named_types.len(), 1);
            assert_eq!(type_.named_types[0].name, "x");
            assert_eq!(type_.named_types[0].ty.to_string(), "String");
        }

        #[test]
        fn terminated_by_invalid_token() {
            let _f = set_up();
            throws_parse_error("(int, String(");
        }
    }

    mod invalid_token {
        use super::*;

        #[test]
        fn before_other_tokens() {
            let _f = set_up();
            throws_parse_error("#int");
        }

        #[test]
        fn at_end() {
            let _f = set_up();
            throws_parse_error("int#");
        }
    }

    #[test]
    fn extra_token_after_type() {
        let _f = set_up();
        throws_parse_error("int)");
    }
}

mod hash_code_and_equality {
    use super::*;

    fn check_equal(t1: Type, t2: Type) {
        assert!(t1 == t2);
        assert!(t1.hash_code() == t2.hash_code());
    }

    fn check_not_equal(t1: Type, t2: Type) {
        assert!(t1 != t2);
        // Note: don't compare `t1.hash_code()` to `t2.hash_code()` because it's
        // not guaranteed whether they will be different or not. And besides, it
        // really only matters for efficiency, and efficiency is not needed for
        // the "mini_types" representation because it's only used in unit tests.
    }

    #[test]
    fn function_type() {
        let _f = set_up();
        check_equal(ty("void Function()"), ty("void Function()"));
        check_not_equal(ty("void Function()?"), ty("void Function()"));
        check_not_equal(ty("T Function()"), ty("void Function()"));
        check_not_equal(ty("void Function(T)"), ty("void Function()"));
        check_equal(ty("void Function(T)"), ty("void Function(T)"));
        check_not_equal(ty("void Function(T)"), ty("void Function(U)"));
        check_not_equal(ty("void Function(T)"), ty("void Function([T])"));
        check_not_equal(ty("void Function({T t})"), ty("void Function()"));
        check_equal(ty("void Function({T t})"), ty("void Function({T t})"));
        check_not_equal(
            ty("void Function({T t})"),
            ty("void Function({required T t})"),
        );
        check_not_equal(ty("void Function({T t})"), ty("void Function({U t})"));
        check_not_equal(ty("void Function({T t})"), ty("void Function({T u})"));
        check_not_equal(ty("void Function()"), ty("void Function<T>()"));
        check_not_equal(
            ty("void Function<T, U>(T, U)?"),
            ty("void Function<U, T>(U, T)"),
        );
        check_equal(
            ty("void Function<T, U>(T, U)"),
            ty("void Function<U, T>(U, T)"),
        );
        check_not_equal(
            ty("void Function<T, U>(T, U)"),
            ty("void Function<T, U>(U, T)"),
        );
        check_equal(
            ty("void Function<T, U>({T p1, U p2})"),
            ty("void Function<U, T>({U p1, T p2})"),
        );
        check_not_equal(
            ty("void Function<T, U>({T p1, U p2})"),
            ty("void Function<T, U>({U p1, T p2})"),
        );
        check_equal(
            ty("void Function<T extends Object>()"),
            ty("void Function<U extends Object>()"),
        );
        check_equal(
            ty("void Function<T extends Object?>()"),
            ty("void Function<U>()"),
        );
        check_not_equal(
            ty("void Function<T extends Object>()"),
            ty("void Function<U extends int>()"),
        );
        check_equal(
            ty("void Function<T extends U, U>()"),
            ty("void Function<V extends W, W>()"),
        );
        check_equal(
            ty("void Function<T>(void Function<U extends T>(T, U))"),
            ty("void Function<V>(void Function<W extends V>(V, W))"),
        );

        // For these final test cases, we give one of the type parameters a name
        // that would be chosen by `FreshTypeParameterGenerator`, to verify that
        // the logic for avoiding name collisions does the right thing.
        let name = FreshTypeParameterGenerator::new().generate().name();
        check_equal(
            ty(&format!("{name} Function<{name}>()")),
            ty("U Function<U>()"),
        );
        check_not_equal(
            ty(&format!("void Function<{name}>(X Function<X>({name}))")),
            ty(&format!(
                "void Function<{name}>({name} Function<X>({name}))"
            )),
        );
    }

    #[test]
    fn primary_type() {
        let _f = set_up();
        check_equal(ty("int"), ty("int"));
        check_not_equal(ty("int"), ty("String"));
        check_not_equal(ty("int"), ty("int?"));
        check_equal(ty("Map<int, String>"), ty("Map<int, String>"));
        check_not_equal(ty("Map<int, String>"), ty("Map<int, double>"));
        check_not_equal(ty("Map<int, String>"), ty("Map<num, String>"));
        check_not_equal(ty("List<int>"), ty("Iterable<int>"));
        check_equal(ty("dynamic"), ty("dynamic"));
        check_equal(ty("error"), ty("error"));
        check_equal(ty("Never"), ty("Never"));
        check_equal(ty("Null"), ty("Null"));
        check_equal(ty("void"), ty("void"));
        check_equal(ty("FutureOr<int>"), ty("FutureOr<int>"));
        check_not_equal(ty("dynamic"), ty("error"));
        check_not_equal(ty("error"), ty("Never"));
        check_not_equal(ty("Never"), ty("Null"));
        check_not_equal(ty("Null"), ty("void"));
        check_not_equal(ty("void"), ty("dynamic"));
        check_not_equal(ty("FutureOr<int>"), ty("FutureOr<String>"));
        check_not_equal(ty("FutureOr<int>"), ty("dynamic"));
    }

    #[test]
    fn record_type() {
        let _f = set_up();
        check_equal(ty("(int,)"), ty("(int,)"));
        check_not_equal(ty("(int,)?"), ty("(int,)"));
        check_not_equal(ty("(int, T)"), ty("(int,)"));
        check_not_equal(ty("(T,)"), ty("(U,)"));
        check_not_equal(ty("(int, {T t})"), ty("(int,)"));
        check_equal(ty("({T t})"), ty("({T t})"));
        check_not_equal(ty("({T t})"), ty("({U t})"));
        check_not_equal(ty("({T t})"), ty("({T u})"));
    }

    #[test]
    fn type_parameter_type() {
        let _f = set_up();
        check_equal(ty("T"), ty("T"));
        check_not_equal(ty("T?"), ty("T"));
        check_not_equal(ty("T"), ty("U"));
        check_not_equal(ty("T&int"), ty("T"));
        check_equal(ty("T&int"), ty("T&int"));
        check_not_equal(ty("T&int"), ty("T&String"));
        // Type formals from different function types are not equal
        check_not_equal(
            TypeParameterType::new(
                ty("void Function<T>()")
                    .as_function_type()
                    .unwrap()
                    .type_parameters_shared[0],
            ),
            TypeParameterType::new(
                ty("void Function<T>()")
                    .as_function_type()
                    .unwrap()
                    .type_parameters_shared[0],
            ),
        );
    }

    #[test]
    fn unknown_type() {
        let _f = set_up();
        check_equal(ty("_"), ty("_"));
        check_not_equal(ty("_?"), ty("_"));
    }
}

mod fresh_type_parameter_generator {
    use super::*;

    #[test]
    fn generates_type_parameters_with_a_bound_of_object() {
        let _f = set_up();
        let mut ftpg = FreshTypeParameterGenerator::new();
        assert_eq!(ftpg.generate().bound().to_string(), "Object?");
        assert_eq!(ftpg.generate().bound().to_string(), "Object?");
        assert_eq!(ftpg.generate().bound().to_string(), "Object?");
    }

    #[test]
    fn generates_a_fresh_name_each_time_generate_is_called() {
        let _f = set_up();
        let mut ftpg = FreshTypeParameterGenerator::new();
        assert_eq!(ftpg.generate().name(), "T0");
        assert_eq!(ftpg.generate().name(), "T1");
        assert_eq!(ftpg.generate().name(), "T2");
    }

    #[test]
    fn skips_names_appearing_in_types_passed_to_exclude_names_used_in() {
        let _f = set_up();
        let mut ftpg = FreshTypeParameterGenerator::new();
        TypeRegistry::add_interface_type_name("T0");
        TypeRegistry::add_interface_type_name("T2");
        TypeRegistry::add_interface_type_name("T3");
        ftpg.exclude_names_used_in(ty("T0<T2, T3>"));
        assert_eq!(ftpg.generate().name(), "T1");
        assert_eq!(ftpg.generate().name(), "T4");
        assert_eq!(ftpg.generate().name(), "T5");
    }
}

mod recursively_demote {
    use super::*;

    mod function_type {
        use super::*;

        mod return_type {
            use super::*;

            #[test]
            fn unchanged() {
                let _f = set_up();
                assert!(ty("int Function()").recursively_demote(true).is_none());
                assert!(ty("int Function()").recursively_demote(false).is_none());
            }

            #[test]
            fn covariant() {
                let _f = set_up();
                assert_eq!(
                    ty("T&int Function()")
                        .recursively_demote(true)
                        .unwrap()
                        .to_string(),
                    "T Function()"
                );
            }

            #[test]
            fn contravariant() {
                let _f = set_up();
                assert_eq!(
                    ty("T&int Function()")
                        .recursively_demote(false)
                        .unwrap()
                        .to_string(),
                    "Never Function()"
                );
            }

            #[test]
            fn generic() {
                let _f = set_up();
                assert_eq!(
                    ty("T&int Function<U>()")
                        .recursively_demote(true)
                        .unwrap()
                        .to_string(),
                    "T Function<U>()"
                );
            }
        }

        mod positional_parameters {
            use super::*;

            #[test]
            fn unchanged() {
                let _f = set_up();
                assert!(
                    ty("void Function(int, String)")
                        .recursively_demote(true)
                        .is_none()
                );
                assert!(
                    ty("void Function(int, String)")
                        .recursively_demote(false)
                        .is_none()
                );
            }

            #[test]
            fn covariant() {
                let _f = set_up();
                assert_eq!(
                    ty("void Function(T&int, String)")
                        .recursively_demote(true)
                        .unwrap()
                        .to_string(),
                    "void Function(Never, String)"
                );
            }

            #[test]
            fn contravariant() {
                let _f = set_up();
                assert_eq!(
                    ty("void Function(T&int, String)")
                        .recursively_demote(false)
                        .unwrap()
                        .to_string(),
                    "void Function(T, String)"
                );
            }
        }

        mod named_parameters {
            use super::*;

            #[test]
            fn unchanged() {
                let _f = set_up();
                assert!(
                    ty("void Function({int x, String y})")
                        .recursively_demote(true)
                        .is_none()
                );
                assert!(
                    ty("void Function({int x, String y})")
                        .recursively_demote(false)
                        .is_none()
                );
            }

            #[test]
            fn covariant() {
                let _f = set_up();
                assert_eq!(
                    ty("void Function({T&int x, String y})")
                        .recursively_demote(true)
                        .unwrap()
                        .to_string(),
                    "void Function({Never x, String y})"
                );
            }

            #[test]
            fn contravariant() {
                let _f = set_up();
                assert_eq!(
                    ty("void Function({T&int x, String y})")
                        .recursively_demote(false)
                        .unwrap()
                        .to_string(),
                    "void Function({T x, String y})"
                );
            }
        }
    }

    mod non_function_type {
        use super::*;

        #[test]
        fn unchanged() {
            let _f = set_up();
            assert!(ty("int").recursively_demote(true).is_none());
            assert!(ty("int").recursively_demote(false).is_none());
        }

        mod type_parameters {
            use super::*;

            #[test]
            fn unchanged() {
                let _f = set_up();
                assert!(ty("Map<int, String>").recursively_demote(true).is_none());
                assert!(ty("Map<int, String>").recursively_demote(false).is_none());
            }

            #[test]
            fn covariant() {
                let _f = set_up();
                assert_eq!(
                    ty("Map<T&int, String>")
                        .recursively_demote(true)
                        .unwrap()
                        .to_string(),
                    "Map<T, String>"
                );
            }

            #[test]
            fn contravariant() {
                let _f = set_up();
                assert_eq!(
                    ty("Map<T&int, String>")
                        .recursively_demote(false)
                        .unwrap()
                        .to_string(),
                    "Map<Never, String>"
                );
            }
        }
    }

    mod question_type {
        use super::*;

        #[test]
        fn unchanged() {
            let _f = set_up();
            assert!(ty("int?").recursively_demote(true).is_none());
            assert!(ty("int?").recursively_demote(false).is_none());
        }

        #[test]
        fn covariant() {
            let _f = set_up();
            assert_eq!(
                ty("(T&int)?").recursively_demote(true).unwrap().to_string(),
                "T?"
            );
        }

        #[test]
        fn contravariant() {
            let _f = set_up();
            // Note: we don't normalize `Never?` to `Null`.
            assert_eq!(
                ty("(T&int)?")
                    .recursively_demote(false)
                    .unwrap()
                    .to_string(),
                "Never?"
            );
        }
    }

    mod record_type {
        use super::*;

        #[test]
        fn unchanged() {
            let _f = set_up();
            assert!(ty("(int, {double a})").recursively_demote(true).is_none());
            assert!(ty("(int, {double a})").recursively_demote(false).is_none());
        }

        mod changed {
            use super::*;

            mod positional {
                use super::*;

                #[test]
                fn covariant() {
                    let _f = set_up();
                    assert_eq!(
                        ty("(T&int, {double a})")
                            .recursively_demote(true)
                            .unwrap()
                            .to_string(),
                        "(T, {double a})"
                    );
                }

                #[test]
                fn contravariant() {
                    let _f = set_up();
                    assert_eq!(
                        ty("(T&int, {double a})")
                            .recursively_demote(false)
                            .unwrap()
                            .to_string(),
                        "(Never, {double a})"
                    );
                }
            }

            mod named {
                use super::*;

                #[test]
                fn covariant() {
                    let _f = set_up();
                    assert_eq!(
                        ty("(double, {T&int a})")
                            .recursively_demote(true)
                            .unwrap()
                            .to_string(),
                        "(double, {T a})"
                    );
                }

                #[test]
                fn contravariant() {
                    let _f = set_up();
                    assert_eq!(
                        ty("(double, {T&int a})")
                            .recursively_demote(false)
                            .unwrap()
                            .to_string(),
                        "(double, {Never a})"
                    );
                }
            }
        }
    }

    #[test]
    fn unknown_type() {
        let _f = set_up();
        assert!(ty("_").recursively_demote(true).is_none());
        assert!(ty("_").recursively_demote(false).is_none());
    }
}

mod closure_with_respect_to_unknown {
    use super::*;

    #[test]
    fn unknown_type() {
        let _f = set_up();
        assert_eq!(
            ty("_")
                .closure_with_respect_to_unknown(true)
                .unwrap()
                .to_string(),
            "Object?"
        );
        assert_eq!(
            ty("_")
                .closure_with_respect_to_unknown(false)
                .unwrap()
                .to_string(),
            "Never"
        );
    }

    mod function_type {
        use super::*;

        mod return_type {
            use super::*;

            #[test]
            fn unchanged() {
                let _f = set_up();
                assert!(
                    ty("int Function()")
                        .closure_with_respect_to_unknown(true)
                        .is_none()
                );
                assert!(
                    ty("int Function()")
                        .closure_with_respect_to_unknown(false)
                        .is_none()
                );
            }

            #[test]
            fn covariant() {
                let _f = set_up();
                assert_eq!(
                    ty("_ Function()")
                        .closure_with_respect_to_unknown(true)
                        .unwrap()
                        .to_string(),
                    "Object? Function()"
                );
            }

            #[test]
            fn contravariant() {
                let _f = set_up();
                assert_eq!(
                    ty("_ Function()")
                        .closure_with_respect_to_unknown(false)
                        .unwrap()
                        .to_string(),
                    "Never Function()"
                );
            }

            #[test]
            fn generic() {
                let _f = set_up();
                assert_eq!(
                    ty("_ Function<T>()")
                        .closure_with_respect_to_unknown(true)
                        .unwrap()
                        .to_string(),
                    "Object? Function<T>()"
                );
            }
        }

        mod positional_parameters {
            use super::*;

            #[test]
            fn unchanged() {
                let _f = set_up();
                assert!(
                    ty("void Function(int, String)")
                        .closure_with_respect_to_unknown(true)
                        .is_none()
                );
                assert!(
                    ty("void Function(int, String)")
                        .closure_with_respect_to_unknown(false)
                        .is_none()
                );
            }

            #[test]
            fn covariant() {
                let _f = set_up();
                assert_eq!(
                    ty("void Function(_, String)")
                        .closure_with_respect_to_unknown(true)
                        .unwrap()
                        .to_string(),
                    "void Function(Never, String)"
                );
            }

            #[test]
            fn contravariant() {
                let _f = set_up();
                assert_eq!(
                    ty("void Function(_, String)")
                        .closure_with_respect_to_unknown(false)
                        .unwrap()
                        .to_string(),
                    "void Function(Object?, String)"
                );
            }
        }

        mod named_parameters {
            use super::*;

            #[test]
            fn unchanged() {
                let _f = set_up();
                assert!(
                    ty("void Function({int x, String y})")
                        .closure_with_respect_to_unknown(true)
                        .is_none()
                );
                assert!(
                    ty("void Function({int x, String y})")
                        .closure_with_respect_to_unknown(false)
                        .is_none()
                );
            }

            #[test]
            fn covariant() {
                let _f = set_up();
                assert_eq!(
                    ty("void Function({_ x, String y})")
                        .closure_with_respect_to_unknown(true)
                        .unwrap()
                        .to_string(),
                    "void Function({Never x, String y})"
                );
            }

            #[test]
            fn contravariant() {
                let _f = set_up();
                assert_eq!(
                    ty("void Function({_ x, String y})")
                        .closure_with_respect_to_unknown(false)
                        .unwrap()
                        .to_string(),
                    "void Function({Object? x, String y})"
                );
            }
        }
    }

    mod non_function_type {
        use super::*;

        #[test]
        fn unchanged() {
            let _f = set_up();
            assert!(ty("int").closure_with_respect_to_unknown(true).is_none());
            assert!(ty("int").closure_with_respect_to_unknown(false).is_none());
        }

        mod type_parameters {
            use super::*;

            #[test]
            fn unchanged() {
                let _f = set_up();
                assert!(
                    ty("Map<int, String>")
                        .closure_with_respect_to_unknown(true)
                        .is_none()
                );
                assert!(
                    ty("Map<int, String>")
                        .closure_with_respect_to_unknown(false)
                        .is_none()
                );
            }

            #[test]
            fn covariant() {
                let _f = set_up();
                assert_eq!(
                    ty("Map<_, String>")
                        .closure_with_respect_to_unknown(true)
                        .unwrap()
                        .to_string(),
                    "Map<Object?, String>"
                );
            }

            #[test]
            fn contravariant() {
                let _f = set_up();
                assert_eq!(
                    ty("Map<_, String>")
                        .closure_with_respect_to_unknown(false)
                        .unwrap()
                        .to_string(),
                    "Map<Never, String>"
                );
            }
        }
    }

    mod question_type {
        use super::*;

        #[test]
        fn unchanged() {
            let _f = set_up();
            assert!(ty("int?").closure_with_respect_to_unknown(true).is_none());
            assert!(ty("int?").closure_with_respect_to_unknown(false).is_none());
        }

        #[test]
        fn covariant() {
            let _f = set_up();
            assert_eq!(
                ty("_?")
                    .closure_with_respect_to_unknown(true)
                    .unwrap()
                    .to_string(),
                "Object?"
            );
        }
    }

    mod record_type {
        use super::*;

        #[test]
        fn unchanged() {
            let _f = set_up();
            assert!(
                ty("(int, {double a})")
                    .closure_with_respect_to_unknown(true)
                    .is_none()
            );
            assert!(
                ty("(int, {double a})")
                    .closure_with_respect_to_unknown(false)
                    .is_none()
            );
        }

        mod changed {
            use super::*;

            mod positional {
                use super::*;

                #[test]
                fn covariant() {
                    let _f = set_up();
                    assert_eq!(
                        ty("(_, {double a})")
                            .closure_with_respect_to_unknown(true)
                            .unwrap()
                            .to_string(),
                        "(Object?, {double a})"
                    );
                }

                #[test]
                fn contravariant() {
                    let _f = set_up();
                    assert_eq!(
                        ty("(_, {double a})")
                            .closure_with_respect_to_unknown(false)
                            .unwrap()
                            .to_string(),
                        "(Never, {double a})"
                    );
                }
            }

            mod named {
                use super::*;

                #[test]
                fn covariant() {
                    let _f = set_up();
                    assert_eq!(
                        ty("(double, {_ a})")
                            .closure_with_respect_to_unknown(true)
                            .unwrap()
                            .to_string(),
                        "(double, {Object? a})"
                    );
                }

                #[test]
                fn contravariant() {
                    let _f = set_up();
                    assert_eq!(
                        ty("(double, {_ a})")
                            .closure_with_respect_to_unknown(false)
                            .unwrap()
                            .to_string(),
                        "(double, {Never a})"
                    );
                }
            }
        }
    }
}

mod gather_used_identifiers {
    use super::*;

    fn query_used_identifiers(t: Type) -> HashSet<String> {
        let mut identifiers = HashSet::new();
        t.gather_used_identifiers(&mut identifiers);
        identifiers
    }

    fn set(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn function_type() {
        let _f = set_up();
        assert_eq!(
            query_used_identifiers(ty("int Function<X>(String, {bool b})")),
            set(&["int", "X", "String", "bool", "b"])
        );
        assert_eq!(
            query_used_identifiers(ty("void Function<X extends int>()")),
            set(&["void", "X", "int"])
        );
    }

    #[test]
    fn primary_type() {
        let _f = set_up();
        assert_eq!(
            query_used_identifiers(ty("Map<String, int>")),
            set(&["Map", "String", "int"])
        );
        assert_eq!(query_used_identifiers(ty("dynamic")), set(&["dynamic"]));
        assert_eq!(query_used_identifiers(ty("error")), set(&["error"]));
        assert_eq!(query_used_identifiers(ty("Never")), set(&["Never"]));
        assert_eq!(query_used_identifiers(ty("Null")), set(&["Null"]));
        assert_eq!(query_used_identifiers(ty("void")), set(&["void"]));
        assert_eq!(
            query_used_identifiers(ty("FutureOr<int>")),
            set(&["FutureOr", "int"])
        );
    }

    #[test]
    fn record_type() {
        let _f = set_up();
        assert_eq!(
            query_used_identifiers(ty("(int, {String s})")),
            set(&["int", "String", "s"])
        );
    }

    #[test]
    fn type_parameter_type() {
        let _f = set_up();
        assert_eq!(query_used_identifiers(ty("T")), set(&["T"]));
        assert_eq!(query_used_identifiers(ty("T&int")), set(&["T", "int"]));
    }

    #[test]
    fn unknown_type() {
        let _f = set_up();
        assert!(query_used_identifiers(ty("_")).is_empty());
    }
}

mod substitute {
    use super::*;

    fn subst(tp: TypeParameter, replacement: &str) -> HashMap<TypeParameter, Type> {
        HashMap::from([(tp, ty(replacement))])
    }

    #[test]
    fn function_type() {
        let f = set_up();
        let m = subst(f.t, "String");
        assert_eq!(ty("int Function<U>(int, {int i})").substitute(&m), None);
        assert_eq!(
            ty("T Function<U>(int, {int i})").substitute(&m),
            Some(ty("String Function<U>(int, {int i})"))
        );
        assert_eq!(
            ty("int Function<U>(T, {int i})?").substitute(&m),
            Some(ty("int Function<U>(String, {int i})?"))
        );
        assert_eq!(
            ty("int Function<U>(int, {T i})").substitute(&m),
            Some(ty("int Function<U>(int, {String i})"))
        );
        assert_eq!(
            ty("int Function<U>(int, {int i})").substitute_function_type(&m, true),
            Some(ty("int Function(int, {int i})"))
        );
        assert_eq!(
            ty("int Function<U>(int, {int i})?").substitute_function_type(&m, true),
            Some(ty("int Function(int, {int i})?"))
        );
        assert_eq!(
            ty("int Function(T, T)").substitute(&m),
            Some(ty("int Function(String, String)"))
        );
        assert_eq!(
            ty("int Function({T t1, T t2})").substitute(&m),
            Some(ty("int Function({String t1, String t2})"))
        );

        // Verify that bounds of type parameters are substituted
        let orig_type = ty("Map<U, V> Function<U extends T, V extends U>(U, V, {U u, V v})");
        let substituted_type_ = orig_type.substitute(&m).unwrap();
        let substituted_type = substituted_type_.as_function_type().unwrap();
        assert_eq!(
            substituted_type_,
            ty("Map<U, V> Function<U extends String, V extends U>(U, V, {U u, V v})")
        );
        // And verify that references to the type parameters now point to the
        // new, updated type parameters.
        let return_args = substituted_type.return_type.as_primary_type().unwrap().args;
        assert!(
            return_args[0]
                .as_type_parameter_type()
                .unwrap()
                .type_parameter
                == substituted_type.type_parameters_shared[0]
        );
        assert!(
            return_args[1]
                .as_type_parameter_type()
                .unwrap()
                .type_parameter
                == substituted_type.type_parameters_shared[1]
        );
        assert!(
            substituted_type.type_parameters_shared[1]
                .explicit_bound()
                .unwrap()
                .as_type_parameter_type()
                .unwrap()
                .type_parameter
                == substituted_type.type_parameters_shared[0]
        );
        assert!(
            substituted_type.positional_parameters[0]
                .as_type_parameter_type()
                .unwrap()
                .type_parameter
                == substituted_type.type_parameters_shared[0]
        );
        assert!(
            substituted_type.positional_parameters[1]
                .as_type_parameter_type()
                .unwrap()
                .type_parameter
                == substituted_type.type_parameters_shared[1]
        );
        assert!(
            substituted_type.named_parameters[0]
                .ty
                .as_type_parameter_type()
                .unwrap()
                .type_parameter
                == substituted_type.type_parameters_shared[0]
        );
        assert!(
            substituted_type.named_parameters[1]
                .ty
                .as_type_parameter_type()
                .unwrap()
                .type_parameter
                == substituted_type.type_parameters_shared[1]
        );
        // Finally, verify that the original type didn't change (this is important
        // because `TypeParameter.explicitBound` is mutable in order to allow
        // for the creation of F-bounded types).
        assert_eq!(
            orig_type,
            ty("Map<U, V> Function<U extends T, V extends U>(U, V, {U u, V v})")
        );
    }

    #[test]
    fn primary_type() {
        let f = set_up();
        let m = subst(f.t, "String");
        assert_eq!(ty("Map<int, int>").substitute(&m), None);
        assert_eq!(
            ty("Map<T, int>").substitute(&m),
            Some(ty("Map<String, int>"))
        );
        assert_eq!(
            ty("Map<int, T>").substitute(&m),
            Some(ty("Map<int, String>"))
        );
        assert_eq!(
            ty("Map<T, T>").substitute(&m),
            Some(ty("Map<String, String>"))
        );
        assert_eq!(ty("dynamic").substitute(&m), None);
        assert_eq!(ty("error").substitute(&m), None);
        assert_eq!(ty("Never").substitute(&m), None);
        assert_eq!(ty("Null").substitute(&m), None);
        assert_eq!(ty("void").substitute(&m), None);
        assert_eq!(ty("FutureOr<int>").substitute(&m), None);
        assert_eq!(
            ty("FutureOr<T>").substitute(&m),
            Some(ty("FutureOr<String>"))
        );
    }

    #[test]
    fn record_type() {
        let f = set_up();
        let m = subst(f.t, "String");
        assert_eq!(ty("(int, {int i})").substitute(&m), None);
        assert_eq!(
            ty("(T, {int i})?").substitute(&m),
            Some(ty("(String, {int i})?"))
        );
        assert_eq!(
            ty("(int, {T i})").substitute(&m),
            Some(ty("(int, {String i})"))
        );
        assert_eq!(ty("(T, T)").substitute(&m), Some(ty("(String, String)")));
        assert_eq!(
            ty("({T t1, T t2})").substitute(&m),
            Some(ty("({String t1, String t2})"))
        );
    }

    #[test]
    fn type_parameter_type() {
        let f = set_up();
        assert_eq!(ty("T").substitute(&subst(f.u, "String")), None);
        assert_eq!(
            ty("T").substitute(&subst(f.t, "String")),
            Some(ty("String"))
        );
        assert_eq!(
            ty("T&Object").substitute(&subst(f.t, "String")),
            Some(ty("String"))
        );
    }

    #[test]
    fn unknown_type() {
        let f = set_up();
        assert_eq!(ty("_").substitute(&subst(f.t, "String")), None);
    }
}
