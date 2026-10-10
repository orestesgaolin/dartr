# Pinned lint rule classification

Sources: Dart SDK 3.13.3, `pkg/linter/lib/src/rules.dart` and `pkg/linter/lib/src/rules/`.

The directory has 262 top-level Dart files and a `pub/` directory with four Dart files.
The task's 263 directory entries therefore represent **266 registered rules**.
All 266 are classified below.

- **AST-only**: syntax, tokens, comments, decoded literal text, file path, source text, parsed feature state, or syntax across the library's ordered units. No resolved elements or static types. All 79 are ported.
- **Needs resolution**: requires elements, static types, resolved targets, constant evaluation, type-system queries, or resolved helper behavior. 83 batch-A rules are ported; 82 remain deferred until their assigned batches.
- **Non-AST context**: requires pubspec/workspace data, filesystem existence, or the completed diagnostic stream. Eight are deferred; they are not AST visitor rules.
- **Removed**: 14 upstream `RemovedAnalysisRule` instances remain in the registry as metadata and do not report diagnostics.

`package_prefixed_library_names` is active upstream but its visitor intentionally returns without reporting; the port preserves that behavior.

Batch-A measurements (`tools/lints_differential.py` with the resolved runner) cover the resolved fixtures, `sdk/lib`, `flutter/lib` and the Visible app `lib`: 74 rules have positive oracle diagnostics and all of them match exactly (path, code, offset, length, message, correction), except 34 `comment_references` diagnostics in the copy of `sdk/lib/core/core.dart` (the analyzer leaves the library doc-comment references unresolved there; dartr resolves them). Nine rules have no positive coverage; a zero count is not proof of parity. Element metadata (`@override`, `@Deprecated`, `@awaitNotRequired`, ...) and constant values come from the resolver's constant evaluation engine through `ElementMetadata`.

The pinned framework uses `AnalysisRule` and `RuleState`; it has no legacy lint category API.
The metadata preserves the states, descriptions, diagnostic code sets, and incompatible rule lists.
Upstream `canUseParsedResult` is preserved separately from this classification: many syntactic rules leave that property at its default `false`.
This classification permits reconstructing library source context from ordered
parsed units and package configuration. `file_names` uses the defining unit;
library feature conditions use its feature set, and source-URI checks use the
visited unit. These are explicit translations of upstream element-backed context
reads, not semantic resolution of declarations or expressions.

| Rule | Class | Reason | Port status |
|---|---|---|---|
| always_declare_return_types | AST-only | Declaration tokens and return-type syntax; the rule context provides package test-directory membership | implemented |
| always_put_control_body_on_new_line | AST-only | Control statement bodies, tokens, and parsed line information only | implemented |
| always_put_required_named_parameters_first | needs resolution | Reads the declared parameter element to distinguish required named parameters | implemented (resolution; measured exact) |
| always_require_non_null_named_parameters | removed | Upstream RemovedAnalysisRule since Dart 3.3 | removed upstream |
| always_specify_types | needs resolution | Reads declared elements and static types to infer omitted annotations | implemented (resolution; measured exact) |
| always_use_package_imports | AST-only | Checks relative import URI syntax when the rule context places the defining unit under its package lib directory | implemented |
| analyzer_element_model_tracking | needs resolution | Internal analyzer rule reads declared elements and annotations | implemented (resolution; positive path untested) |
| analyzer_public_api | needs resolution | Internal analyzer rule traverses public elements and exported libraries | implemented (resolution; positive path untested) |
| annotate_overrides | needs resolution | Requires inherited-member and overridden-element lookup | implemented (resolution; measured exact) |
| annotate_redeclares | needs resolution | Requires extension-type redeclaration element data | implemented (resolution; measured exact) |
| async_return_with_no_await | needs resolution | Uses AsyncReturnVisitor with type provider and type system | implemented (resolution; measured exact) |
| avoid_annotating_with_dynamic | AST-only | Explicit dynamic type annotation and augmentation syntax only | implemented |
| avoid_as | removed | Upstream RemovedAnalysisRule since Dart 2.12 | removed upstream |
| avoid_bool_literals_in_conditional_expressions | needs resolution | Uses conditional-expression static type and bool from the type provider | implemented (resolution; measured exact) |
| avoid_catches_without_on_clauses | needs resolution | Uses resolved exception parameter type and subtype information | implemented (resolution; measured exact) |
| avoid_catching_errors | needs resolution | Checks whether the resolved exception type implements dart.core Error | implemented (resolution; measured exact) |
| avoid_classes_with_only_static_members | needs resolution | Reads constructors, methods, fields, and superclass elements | implemented (resolution; measured exact) |
| avoid_double_and_int_checks | needs resolution | Uses identifier elements and core int/double types | implemented (resolution; measured exact) |
| avoid_dynamic_calls | needs resolution | Depends on expression static/read/write types | implemented (resolution; measured exact) |
| avoid_empty_else | AST-only | Checks an IfStatement else child for a real EmptyStatement | implemented |
| avoid_equals_and_hash_code_on_mutable_classes | needs resolution | Requires class annotations and declared method elements | implemented (resolution; measured exact) |
| avoid_escaping_inner_quotes | AST-only | Uses string-literal syntax and decoded interpolation text only | implemented |
| avoid_field_initializers_in_const_classes | needs resolution | Requires enclosing class and constructor elements | implemented (resolution; measured exact) |
| avoid_final_parameters | AST-only | Checks formal-parameter syntax with the defining library primary-constructor feature | implemented |
| avoid_function_literals_in_foreach_calls | needs resolution | Uses receiver static type to identify Iterable.forEach | implemented (resolution; measured exact) |
| avoid_futureor_void | needs resolution | Variance checker consumes resolved DartType objects | implemented (resolution; measured exact) |
| avoid_implementing_value_types | needs resolution | Inspects implemented interface elements and annotations | implemented (resolution; measured exact) |
| avoid_init_to_null | needs resolution | Uses declared variable/parameter types and nullability type system | implemented (resolution; measured exact) |
| avoid_js_rounded_ints | AST-only | Checks parsed integer-literal value only | implemented |
| avoid_multiple_declarations_per_line | AST-only | Checks VariableDeclarationList shape and parent syntax | implemented |
| avoid_null_checks_in_equality_operators | needs resolution | Deprecated active rule identifies operator elements and equality contracts | implemented (resolution; measured exact) |
| avoid_positional_boolean_parameters | needs resolution | Needs resolved bool parameter types and override/inheritance data | implemented (resolution; measured exact) |
| avoid_print | needs resolution | Identifies dart.core print through resolved elements and Flutter context | implemented (resolution; measured exact) |
| avoid_private_typedef_functions | AST-only | Counts NamedType syntax references across ordered library units | implemented |
| avoid_redundant_argument_values | needs resolution | Compares arguments with resolved executable default values | implemented (resolution; measured exact) |
| avoid_relative_lib_imports | AST-only | Checks import URI syntax only | implemented |
| avoid_renaming_method_parameters | needs resolution | Compares resolved method parameters with inherited parameters | implemented (resolution; measured exact) |
| avoid_return_types_on_setters | AST-only | Checks setter and explicit return-type syntax only | implemented |
| avoid_returning_null | removed | Upstream RemovedAnalysisRule since Dart 3.3 | removed upstream |
| avoid_returning_null_for_future | removed | Upstream RemovedAnalysisRule since Dart 3.3 | removed upstream |
| avoid_returning_null_for_void | needs resolution | Uses enclosing executable element return type | implemented (resolution; measured exact) |
| avoid_returning_this | needs resolution | Uses method element and return-type/override information | implemented (resolution; measured exact) |
| avoid_setters_without_getters | needs resolution | Matches getter and setter elements across containers | implemented (resolution; measured exact) |
| avoid_shadowing_type_parameters | AST-only | Compares lexical type-parameter names using the defining library wildcard feature | implemented |
| avoid_single_cascade_in_expression_statements | AST-only | Checks cascade section count, operator token, and AST parent | implemented |
| avoid_slow_async_io | needs resolution | Identifies dart.io members through resolved method elements | implemented (resolution; measured exact) |
| avoid_type_to_string | needs resolution | Uses expression static types, elements, and type system | implemented (resolution; positive path untested) |
| avoid_types_as_parameter_names | needs resolution | Builds element scopes and inspects declared elements | implemented (resolution; measured exact) |
| avoid_types_on_closure_parameters | needs resolution | Requires approximate contextual FunctionType | implemented (resolution; measured exact) |
| avoid_unnecessary_containers | needs resolution | Requires Flutter Widget static types and constructor arguments | implemented (resolution; positive path untested) |
| avoid_unstable_final_fields | removed | Upstream RemovedAnalysisRule | removed upstream |
| avoid_unused_constructor_parameters | needs resolution | Uses constructor/field elements and parameter references | implemented (resolution; measured exact) |
| avoid_void_async | needs resolution | Uses executable fragments, async state, and resolved return type | implemented (resolution; measured exact) |
| avoid_web_libraries_in_flutter | non-AST context | Reads package root and pubspec Flutter plugin configuration | deferred |
| await_only_futures | needs resolution | Uses awaited expression static type and type system | implemented (resolution; measured exact) |
| camel_case_extensions | AST-only | Checks extension declaration name token only | implemented |
| camel_case_types | AST-only | Checks declaration name tokens only | implemented |
| cancel_subscriptions | needs resolution | Leak detector follows resolved StreamSubscription types and references | implemented (resolution; measured exact) |
| cascade_invocations | needs resolution | Tracks resolved variable and target elements across statements | implemented (resolution; measured exact) |
| cast_nullable_to_non_nullable | needs resolution | Compares expression and cast types with nullability type system | implemented (resolution; measured exact) |
| close_sinks | needs resolution | Leak detector follows resolved Sink types and references | implemented (resolution; measured exact) |
| collection_methods_unrelated_type | needs resolution | Uses receiver/argument static types, method elements, and type system | implemented (resolution; measured exact) |
| combinators_ordering | AST-only | Sort-checks show/hide identifier tokens | implemented |
| comment_references | needs resolution | Checks resolved doc-comment reference elements | implemented (resolution; measured exact except a copy of `dart:core`, see below) |
| conditional_uri_does_not_exist | non-AST context | Requires URI resolution and source/filesystem existence | deferred |
| constant_identifier_names | AST-only | Checks const declaration syntax and identifier tokens | implemented |
| control_flow_in_finally | needs resolution | Break/continue target links are populated by resolution | implemented (resolution; measured exact) |
| curly_braces_in_flow_control_structures | AST-only | Checks statement shape and parsed line information only | implemented |
| dangling_library_doc_comments | AST-only | Uses declaration/doc-comment attachment and token line positions | implemented |
| depend_on_referenced_packages | non-AST context | Requires pub package/pubspec dependencies and defining-unit directory context. | deferred |
| deprecated_consistency | needs resolution | Compares @deprecated annotations on constructor and parameter elements | implemented (resolution; measured exact) |
| deprecated_member_use_from_same_package | needs resolution | Uses emitted diagnostics, element packages, and workspace data | implemented (resolution; measured exact) |
| diagnostic_describe_all_properties | needs resolution | Uses Flutter class hierarchy and declared method elements | implemented (resolution; measured exact) |
| directives_ordering | AST-only | Orders parsed import/export/doc-import URI syntax | implemented |
| discarded_futures | needs resolution | Uses expression static types and Future type provider | implemented (resolution; measured exact) |
| do_not_use_environment | needs resolution | Identifies core environment constructors through resolved elements | implemented (resolution; measured exact) |
| document_ignores | AST-only | Parses ignore comment tokens and adjacent source lines | implemented |
| empty_catches | AST-only | Checks catch body, parameter spelling, and comment tokens | implemented |
| empty_constructor_bodies | AST-only | Checks constructor body shape and trailing comments | implemented |
| empty_container_bodies | AST-only | Checks container body tokens when the defining library enables primary constructors | implemented |
| empty_statements | AST-only | Checks EmptyStatement parent and switch-case statement list | implemented |
| enable_null_safety | removed | Upstream RemovedAnalysisRule since Dart 3 | removed upstream |
| eol_at_end_of_file | AST-only | Checks current source text at CompilationUnit | implemented |
| erase_dart_type_extension_types | needs resolution | Internal rule requires resolved extension-type erasure information | implemented (resolution; positive path untested) |
| exhaustive_cases | needs resolution | Uses switch expression type and enum/class elements | implemented (resolution; measured exact) |
| file_names | AST-only | Validates the defining library unit basename even when visiting a part | implemented |
| flutter_style_todos | AST-only | Scans parser token comments only | implemented |
| hash_and_equals | needs resolution | Inspects declared class elements and overridden equality/hash members. | implemented (resolution; measured exact) |
| implementation_imports | needs resolution | Uses resolved imported library elements and source URIs. | implemented (resolution; measured exact) |
| implicit_call_tearoffs | needs resolution | Visits ImplicitCallReference nodes synthesized by resolution. | implemented (resolution; measured exact) |
| implicit_reopen | needs resolution | Compares resolved declaration and library elements. | implemented (resolution; measured exact) |
| initialize_in_field_declaration | needs resolution | Uses constructor and enclosing field elements. | implemented (resolution; measured exact) |
| invalid_case_patterns | needs resolution | Uses resolved pattern variable elements across case branches. | implemented (resolution; positive path untested) |
| invalid_runtime_check_with_js_interop_types | needs resolution | Requires static types, extension-type elements, and the type system. | implemented (resolution; measured exact) |
| invariant_booleans | removed | Upstream is RemovedAnalysisRule. | n/a |
| iterable_contains_unrelated_type | removed | Upstream is RemovedAnalysisRule. | n/a |
| join_return_with_assignment | needs resolution | Compares canonical elements of assignment and return targets. | implemented (resolution; measured exact) |
| leading_newlines_in_multiline_strings | AST-only | Uses parsed string syntax, source offsets, and line information without resolution. | implemented |
| library_annotations | needs resolution | Uses annotation and enclosing library elements. | implemented (resolution; measured exact) |
| library_names | AST-only | Checks the syntactic dotted library name. | implemented |
| library_prefixes | AST-only | Checks the syntactic import prefix and defining library wildcard feature. | implemented |
| library_private_types_in_public_api | needs resolution | Traverses public elements and resolved API types. | implemented (resolution; measured exact) |
| lines_longer_than_80_chars | AST-only | Scans source lines and comments; string URI/path exemptions use decoded AST values and multiline source ranges. | implemented |
| list_remove_unrelated_type | removed | Upstream is RemovedAnalysisRule. | n/a |
| literal_only_boolean_expressions | needs resolution | Computes string constants and resolves type-parameter annotations. | implemented (resolution; measured exact) |
| matching_super_parameters | needs resolution | Matches parameters against resolved super constructors. | implemented (resolution; measured exact) |
| migrate_design_widgets | AST-only | Checks import URI string literals. | implemented |
| missing_code_block_language_in_doc_comment | AST-only | Uses parsed Markdown code-block data on Comment nodes. | implemented |
| missing_whitespace_between_adjacent_strings | needs resolution | Its RegExp exemption identifies the resolved constructor element. | implemented (resolution; measured exact) |
| no_adjacent_strings_in_list | AST-only | Checks collection elements and list-pattern constants syntactically. | implemented |
| no_default_cases | needs resolution | Uses the switch expression static type for enum exemptions. | implemented (resolution; measured exact) |
| no_duplicate_case_values | needs resolution | Computes and compares constant values. | implemented (resolution; positive path untested) |
| no_dynamic_casts | needs resolution | Requires static types, read elements, and the type system. | implemented (resolution; measured exact) |
| no_leading_underscores_for_library_prefixes | AST-only | Checks the syntactic import prefix and defining library wildcard feature. | implemented |
| no_leading_underscores_for_local_identifiers | needs resolution | Uses declared parameter elements to exempt field-declaring parameters. | implemented (resolution; measured exact) |
| no_literal_bool_comparisons | needs resolution | Uses static types and the type system. | implemented (resolution; measured exact) |
| no_logic_in_create_state | needs resolution | Identifies resolved Flutter State/createState elements. | implemented (resolution; positive path untested) |
| no_raw_types | needs resolution | Requires resolved generic type elements and type arguments. | implemented (resolution; measured exact) |
| no_runtimeType_toString | needs resolution | Identifies resolved Object.runtimeType members. | implemented (resolution; measured exact) |
| no_self_assignments | AST-only | Compares identifier syntax on simple assignments. | implemented |
| no_wildcard_variable_uses | needs resolution | Uses resolved wildcard variable elements and library feature state. | implemented (resolution; positive path untested) |
| non_constant_identifier_names | AST-only | Checks declaration tokens and syntactic context, including augmentation exclusions. | implemented |
| noop_primitive_operations | needs resolution | Uses operand static types and resolved operator elements. | implemented (resolution; measured exact) |
| null_check_on_nullable_type_parameter | needs resolution | Uses static type-parameter types and the type system. | implemented (resolution; measured exact) |
| null_closures | needs resolution | Requires contextual/static function types. | implemented (resolution; measured exact) |
| omit_local_variable_types | needs resolution | Compares declared annotations with inferred static types. | implemented (resolution; measured exact) |
| omit_obvious_local_variable_types | needs resolution | Uses inferred initializer types and type-provider data. | implemented (resolution; measured exact) |
| omit_obvious_property_types | needs resolution | Uses inferred property/initializer types. | implemented (resolution; measured exact) |
| one_member_abstracts | needs resolution | Counts resolved public interface members. | implemented (resolution; measured exact) |
| only_throw_errors | needs resolution | Checks the thrown expression static type. | implemented (resolution; measured exact) |
| overridden_fields | needs resolution | Finds inherited fields through resolved elements. | implemented (resolution; measured exact) |
| package_api_docs | removed | Upstream is RemovedAnalysisRule. | n/a |
| package_names | non-AST context | Runs on the parsed pubspec rather than a Dart AST. | deferred |
| package_prefixed_library_names | AST-only | Active upstream rule whose visitor intentionally returns without reporting until project information is restored. | implemented no-op |
| parameter_assignments | needs resolution | Uses declared parameter elements and write references. | implemented (resolution; measured exact) |
| prefer_adjacent_string_concatenation | AST-only | Checks binary operator and string-literal node kinds. | implemented |
| prefer_asserts_in_initializer_lists | needs resolution | Uses constructor elements and enclosing constructor state. | implemented (resolution; measured exact) |
| prefer_asserts_with_message | AST-only | Checks assert message syntax. | implemented |
| prefer_bool_in_asserts | removed | Upstream is RemovedAnalysisRule. | n/a |
| prefer_collection_literals | needs resolution | Identifies core collection constructors through elements and types. | implemented (resolution; measured exact) |
| prefer_conditional_assignment | needs resolution | Compares canonical resolved elements of null-check and assignment targets. | deferred |
| prefer_const_constructors | needs resolution | Uses resolved constructors and constant-context analysis. | deferred |
| prefer_const_constructors_in_immutables | needs resolution | Uses class/constructor elements and immutability metadata. | deferred |
| prefer_const_declarations | needs resolution | Calls constant evaluation/error analysis for each initializer. | deferred |
| prefer_const_literals_to_create_immutables | needs resolution | Uses static types and immutable-class metadata. | deferred |
| prefer_constructors_over_static_methods | needs resolution | Requires return/interface types and enclosing elements. | deferred |
| prefer_contains | needs resolution | Uses target static types to validate index/search rewrites. | deferred |
| prefer_double_quotes | AST-only | Checks string delimiters, contents, nesting, and interpolation syntax. | implemented |
| prefer_equal_for_default_values | removed | Upstream is RemovedAnalysisRule. | n/a |
| prefer_expression_function_bodies | AST-only | Checks a block body for one value-returning statement. | implemented |
| prefer_final_fields | needs resolution | Uses field elements and write-reference analysis. | deferred |
| prefer_final_in_for_each | needs resolution | Uses declared-variable elements and mutation analysis. | deferred |
| prefer_final_locals | needs resolution | Uses local elements and potentially-mutated-in-scope analysis. | deferred |
| prefer_final_parameters | needs resolution | Uses parameter elements and mutation analysis. | deferred |
| prefer_for_elements_to_map_fromIterable | needs resolution | Identifies resolved core Map.fromIterable calls. | deferred |
| prefer_foreach | needs resolution | Uses resolved iterable methods and closure parameter elements. | deferred |
| prefer_function_declarations_over_variables | needs resolution | Uses declared elements and mutation analysis for local variables. | deferred |
| prefer_generic_function_type_aliases | AST-only | Builds a replacement from parsed typedef syntax. | implemented |
| prefer_if_elements_to_conditional_expressions | AST-only | Checks conditional-expression collection parents. | implemented |
| prefer_if_null_operators | AST-only | Compares null-test and result expression syntax. | implemented |
| prefer_initializing_formals | needs resolution | Uses field writes, field elements, and the type system. | deferred |
| prefer_inlined_adds | AST-only | Checks list-literal cascade sections and arguments. | implemented |
| prefer_int_literals | needs resolution | Uses contextual parameter, collection, variable, and return types. | deferred |
| prefer_interpolation_to_compose_strings | needs resolution | Uses operand static types to preserve overloaded + semantics. | deferred |
| prefer_is_empty | needs resolution | Uses target static types and resolved members. | deferred |
| prefer_is_not_empty | needs resolution | Uses resolved isEmpty member elements. | deferred |
| prefer_is_not_operator | AST-only | Checks parenthesized is expressions under syntactic negation. | implemented |
| prefer_iterable_whereType | needs resolution | Uses target and type-argument static types. | deferred |
| prefer_mixin | needs resolution | Uses resolved class elements and inheritance constraints. | deferred |
| prefer_null_aware_method_calls | AST-only | Matches null checks and explicit function invocations syntactically. | implemented |
| prefer_null_aware_operators | AST-only | Matches null checks and property/invocation target chains syntactically. | implemented |
| prefer_relative_imports | needs resolution | Requires resolved import sources, package roots, and library URIs. | deferred |
| prefer_single_quotes | AST-only | Checks string delimiters, contents, nesting, and interpolation syntax. | implemented |
| prefer_spread_collections | AST-only | Checks list-literal cascades and syntactic constant context. | implemented |
| prefer_typing_uninitialized_variables | AST-only | Checks uninitialized variable syntax and excludes augmentation declarations. | implemented |
| prefer_void_to_null | needs resolution | Uses resolved type annotations, element use sites, and ancestry. | deferred |
| provide_deprecation_message | needs resolution | Uses resolved ElementAnnotation.isDeprecated to distinguish the core annotation. | deferred |
| public_member_api_docs | needs resolution | Uses resolved public elements, overrides, and documentation inheritance. | deferred |
| recursive_getters | needs resolution | Compares identifier elements with the enclosing getter element. | deferred |
| remove_deprecations_in_breaking_version | non-AST context | Reads package workspace and pubspec version information. | deferred |
| require_trailing_commas | AST-only | Uses tokens, line information, language version, and syntax shapes only. | implemented |
| secure_pubspec_urls | non-AST context | Runs on pubspec URL and dependency nodes. | deferred |
| simple_directive_paths | AST-only | Uses directive string syntax plus the current source path for relative minimality. | implemented |
| simplify_variable_pattern | needs resolution | Reads field elements and their types. | deferred |
| sized_box_for_whitespace | needs resolution | Identifies Flutter Container from the resolved static type and library URI. | deferred |
| sized_box_shrink_expand | needs resolution | Identifies Flutter SizedBox from the resolved static type and library URI. | deferred |
| slash_for_doc_comments | AST-only | Checks attached documentation-comment token spelling. | implemented |
| sort_child_properties_last | needs resolution | Uses resolved Flutter widget types and named parameter elements. | deferred |
| sort_constructors_first | AST-only | Examines class member order and constructor syntax. | implemented |
| sort_pub_dependencies | non-AST context | Runs on ordered pubspec dependency nodes and source spans. | deferred |
| sort_unnamed_constructors_first | needs resolution | Uses declared constructor elements to identify the canonical unnamed name. | deferred |
| specify_nonobvious_local_variable_types | needs resolution | Uses inferred static types and obvious-type analysis. | deferred |
| specify_nonobvious_property_types | needs resolution | Uses declared elements and inferred initializer types. | deferred |
| strict_top_level_inference | needs resolution | Reads declared fragments and inferred element types. | deferred |
| super_goes_last | removed | Upstream is RemovedAnalysisRule since Dart 3. | removed upstream |
| switch_on_type | needs resolution | Uses matched-value types and the type system. | deferred |
| test_types_in_equals | AST-only | Matches operator syntax, its single parameter, and an as-expression. | implemented |
| throw_in_finally | AST-only | Uses ancestor and finally-block syntax only. | implemented |
| tighten_type_of_initializing_formals | needs resolution | Compares field and formal-parameter resolved types. | deferred |
| type_annotate_public_apis | needs resolution | Uses element visibility and inferred/static types. | deferred |
| type_init_formals | needs resolution | Reads corresponding field and super-constructor elements. | deferred |
| type_literal_in_constant_pattern | needs resolution | Uses matched-value/static types and subtype checks. | deferred |
| unawaited_futures | needs resolution | Uses expression static types and enclosing executable elements. | deferred |
| unintended_html_in_doc_comment | AST-only | Scans documentation-comment tokens and parsed code-block ranges. | implemented |
| unnecessary_async | needs resolution | Uses return static types and element declarations. | deferred |
| unnecessary_await_in_return | needs resolution | Uses return types and the type system. | deferred |
| unnecessary_brace_in_string_interps | AST-only | Checks interpolation expression shape and adjacent token text. | implemented |
| unnecessary_breaks | AST-only | Checks an unlabeled final break when the defining library enables patterns. | implemented |
| unnecessary_const | AST-only | Checks explicit const tokens and syntactic constant context. | implemented |
| unnecessary_const_in_enum_constructor | AST-only | Checks explicit const tokens on enum constructors. | implemented |
| unnecessary_constructor_name | needs resolution | Uses enclosing constructor elements to suppress duplicate-constructor cases. | deferred |
| unnecessary_final | AST-only | Checks final tokens on local declarations and parameters. | implemented |
| unnecessary_getters_setters | needs resolution | Uses declared getter/setter elements and resolved metadata. | deferred |
| unnecessary_ignore | non-AST context | Runs after all diagnostics through IgnoreValidator rather than an AST visitor. | deferred |
| unnecessary_lambdas | needs resolution | Compares function and invoked elements and static types. | deferred |
| unnecessary_late | AST-only | Checks static/top-level variable syntax and initializers. | implemented |
| unnecessary_library_directive | AST-only | Checks directive siblings, annotations, and documentation comments. | implemented |
| unnecessary_library_name | AST-only | Checks for a library name when the defining library enables unnamed libraries. | implemented |
| unnecessary_new | AST-only | Checks the instance-creation keyword token. | implemented |
| unnecessary_null_aware_assignments | needs resolution | Uses write/read elements to prove target identity. | deferred |
| unnecessary_null_aware_operator_on_extension_on_nullable | needs resolution | Uses resolved extension elements and nullable types. | deferred |
| unnecessary_null_checks | needs resolution | Uses static types, elements, and the type system. | deferred |
| unnecessary_null_in_if_null_operators | AST-only | Checks a ?? operand for a null literal. | implemented |
| unnecessary_nullable_for_final_variable_declarations | needs resolution | Uses declared-variable elements, inferred types, and the type system. | deferred |
| unnecessary_overrides | needs resolution | Compares declared methods with inherited elements. | deferred |
| unnecessary_parenthesis | needs resolution | Uses corresponding parameters, record types, extension elements, and the type system. | deferred |
| unnecessary_primary_constructor_body | AST-only | The upstream type-system field is unused; the decision is empty-body syntax only. | implemented |
| unnecessary_raw_strings | AST-only | Checks raw literal token text for backslash or dollar usage. | implemented |
| unnecessary_statements | needs resolution | Uses getter elements to avoid side-effect false positives. | deferred |
| unnecessary_string_escapes | AST-only | Scans string literal and interpolation token contents. | implemented |
| unnecessary_string_interpolations | needs resolution | Requires the interpolated expression static String type and nullability. | deferred |
| unnecessary_this | needs resolution | Resolves referenced members and lexical scope shadowing. | deferred |
| unnecessary_to_list_in_spreads | needs resolution | Uses target static type and method element identity. | deferred |
| unnecessary_type_name_in_constructor | AST-only | Reports the explicit type-name child of a constructor declaration. | implemented |
| unnecessary_unawaited | needs resolution | Uses the resolved unawaited function element. | deferred |
| unnecessary_underscores | needs resolution | Tracks declared and referenced elements through function bodies. | deferred |
| unreachable_from_main | needs resolution | Builds a reachability graph from declared and referenced elements. | deferred |
| unrelated_type_equality_checks | needs resolution | Uses operand static types, operator elements, and the type system. | deferred |
| unsafe_html | removed | Upstream is RemovedAnalysisRule. | removed upstream |
| unsafe_variance | needs resolution | Uses interface/member elements and variance of resolved types. | deferred |
| use_build_context_synchronously | needs resolution | Uses BuildContext static types, elements, and async-state tracking. | deferred |
| use_colored_box | needs resolution | Identifies Flutter Container from its resolved static type. | deferred |
| use_declaring_parameters | needs resolution | Uses constructor/field elements and assigned-field relationships. | deferred |
| use_decorated_box | needs resolution | Identifies Flutter Container from its resolved static type and URI. | deferred |
| use_enums | needs resolution | Uses class and member elements to validate enum conversion. | deferred |
| use_full_hex_values_for_flutter_colors | needs resolution | Uses the resolved Flutter Color constructor element. | deferred |
| use_function_type_syntax_for_parameters | AST-only | Checks the function-typed suffix on formal parameters. | implemented |
| use_if_null_to_convert_nulls_to_bools | needs resolution | Uses operand static types and nullability subtype checks. | deferred |
| use_is_even_rather_than_modulo | needs resolution | Requires resolved int static types. | deferred |
| use_key_in_widget_constructors | needs resolution | Uses Flutter widget ancestry and constructor parameter elements. | deferred |
| use_late_for_private_fields_and_variables | needs resolution | Uses declared fields, references, flow, and inferred types. | deferred |
| use_named_constants | needs resolution | Compares constant values and resolved constructor/type elements. | deferred |
| use_null_aware_elements | needs resolution | Compares canonical elements for null-check target identity. | deferred |
| use_primary_constructors | needs resolution | Uses redirecting-constructor elements. | deferred |
| use_raw_strings | AST-only | Scans simple literal escapes and quote syntax. | implemented |
| use_rethrow_when_possible | needs resolution | Compares the thrown expression element with the catch parameter element. | deferred |
| use_setters_to_change_properties | needs resolution | Uses method and field elements plus inherited-member information. | deferred |
| use_string_buffers | needs resolution | Uses assignment/read elements to track the same String variable. | deferred |
| use_string_in_part_of_directives | AST-only | Checks the library-name versus URI alternatives on part-of syntax. | implemented |
| use_super_parameters | needs resolution | Uses super-constructor parameter and field elements plus subtype checks. | deferred |
| use_test_throws_matchers | needs resolution | Uses resolved test package function elements. | deferred |
| use_to_and_as_if_applicable | needs resolution | Uses resolved return type and inherited-method information. | deferred |
| use_truncating_division | needs resolution | Requires operand static int types and core-library identity. | deferred |
| valid_regexps | needs resolution | Uses the resolved dart:core RegExp constructor identity. | deferred |
| var_with_no_type_annotation | AST-only | Checks var tokens and absent type annotations on formal parameters. | implemented |
| void_checks | needs resolution | Uses expression static void types, elements, and the type system. | deferred |
