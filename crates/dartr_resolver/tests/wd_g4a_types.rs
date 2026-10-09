//! Integration tests of the wave D verifiers of group g4a: the type
//! arguments, literal element, return type, required parameters,
//! constructor fields, base-or-final type and super formal parameters
//! verifiers, and the assignment verifier.
//!
//! The cases in `wd_g4a/cases.rs` are extracted from the analyzer's
//! diagnostic tests (`pkg/analyzer/test/src/diagnostics/*_test.dart`,
//! single-file `resolveTestCodeWithDiagnostics` / `assertErrorsInCode` /
//! `assertNoErrorsInCode` cases) by `extract.py` (see the generated file
//! header); the expected diagnostics are filtered to the codes these
//! verifiers report.
//!
//! The error verifier, which calls most of these verifiers, is not ported
//! yet. [`ErrorVerifierStandIn`] calls them on the resolved unit like
//! `ErrorVerifier` does (`visitListLiteral`, `visitNamedType`,
//! `visitReturnStatement`, `_withEnclosingExecutable`, ...). The resolver
//! reports the diagnostics of the assignment, base-or-final and super
//! formal parameters verifiers itself.

mod support;

use dartr_ast::{
    Annotation, ClassDeclaration, ConstructorDeclaration, ConstructorReference,
    DotShorthandConstructorInvocation, DotShorthandInvocation, EnumConstantDeclaration,
    EnumDeclaration, ExpressionFunctionBody, ExtensionTypeDeclaration, FunctionDeclaration,
    FunctionExpressionInvocation, FunctionReference, Id, InstanceCreationExpression, ListLiteral,
    MethodDeclaration, MethodInvocation, NamedType, NodeId, NodeKind, PrimaryConstructorBody,
    RedirectingConstructorInvocation, ReturnStatement, SetOrMapLiteral, SuperConstructorInvocation,
    TypeAnnotation,
};
use dartr_diagnostics::Diagnostic;
use dartr_element::{ElemRef, ElementId, FragmentFlags, InterfaceElement, TypeId, TypeKind};
use dartr_resolver::element_ext::first_fragment_flags;
use dartr_resolver::error::UnitVerifier;
use dartr_resolver::error::constructor_fields_verifier::ConstructorFieldsVerifier;
use dartr_resolver::error::literal_element_verifier::LiteralElementVerifier;
use dartr_resolver::error::required_parameters_verifier as rp;
use dartr_resolver::error::return_type_verifier::{self as rt, EnclosingExecutableContext};
use dartr_resolver::error::type_arguments_verifier as ta;
use dartr_resolver::library_analyzer::ResolvedUnit;
use dartr_resolver::options::AnalysisOptions;
use dartr_resolver::scope::LibraryScopes;
use dartr_typesystem::TypeSystem;
use indexmap::IndexMap;

/// One analyzer test case.
pub struct Case {
    pub file: &'static str,
    pub name: &'static str,
    pub source: &'static str,
    /// `(camelCaseName, offset, length)` of the expected diagnostics.
    pub expected: &'static [(&'static str, usize, usize)],
}

include!("wd_g4a/cases.rs");

/// The `camelCaseName`s of the diagnostics that the g4a verifiers report.
const CODES: &[&str] = &[
    "assignmentToConst",
    "assignmentToFinal",
    "assignmentToFinalNoSetter",
    "assignmentToFunction",
    "assignmentToMethod",
    "assignmentToType",
    "prefixIdentifierNotFollowedByDot",
    "baseClassImplementedOutsideOfLibrary",
    "baseMixinImplementedOutsideOfLibrary",
    "subtypeOfBaseIsNotBaseFinalOrSealed",
    "subtypeOfFinalIsNotBaseFinalOrSealed",
    "mixinSubtypeOfBaseIsNotBase",
    "mixinSubtypeOfFinalIsNotBase",
    "expectedOneListTypeArguments",
    "expectedOneSetTypeArguments",
    "expectedTwoMapTypeArguments",
    "expressionInMap",
    "mapEntryNotInMap",
    "fieldInitializedByMultipleInitializers",
    "fieldInitializedInDeclarationAndInitializerOfPrimaryConstructor",
    "fieldInitializedInDeclarationAndParameterOfPrimaryConstructor",
    "fieldInitializedInInitializerAndDeclaration",
    "fieldInitializedInParameterAndInitializer",
    "finalInitializedInDeclarationAndConstructor",
    "finalNotInitializedConstructor1",
    "finalNotInitializedConstructor2",
    "finalNotInitializedConstructor3Plus",
    "notInitializedNonNullableInstanceFieldConstructor",
    "genericFunctionTypeCannotBeTypeArgument",
    "illegalAsyncGeneratorReturnType",
    "illegalAsyncReturnType",
    "illegalSyncGeneratorReturnType",
    "invalidTypeArgumentInConstList",
    "invalidTypeArgumentInConstMap",
    "invalidTypeArgumentInConstSet",
    "listElementTypeNotAssignable",
    "listElementTypeNotAssignableNullability",
    "setElementTypeNotAssignable",
    "setElementTypeNotAssignableNullability",
    "mapKeyTypeNotAssignable",
    "mapKeyTypeNotAssignableNullability",
    "mapValueTypeNotAssignable",
    "mapValueTypeNotAssignableNullability",
    "missingRequiredArgument",
    "notIterableSpread",
    "notMapSpread",
    "notNullAwareNullSpread",
    "positionalSuperFormalParameterWithPositionalArgument",
    "recordLiteralOnePositionalNoTrailingCommaByType",
    "returnInGenerativeConstructor",
    "returnOfInvalidTypeFromClosure",
    "returnOfInvalidTypeFromConstructor",
    "returnOfInvalidTypeFromFunction",
    "returnOfInvalidTypeFromMethod",
    "returnWithoutValue",
    "typeArgumentNotMatchingBounds",
    "wrongNumberOfTypeArgumentsEnum",
];

/// Calls the g4a verifiers on the nodes of a resolved unit like the Dart
/// `ErrorVerifier` does.
struct ErrorVerifierStandIn<'a, 'v> {
    v: &'v mut UnitVerifier<'a>,
    constructor_fields: ConstructorFieldsVerifier,
    /// The enclosing executable of each function node (Dart
    /// `_withEnclosingExecutable`), created on first use.
    executables: IndexMap<NodeId, EnclosingExecutableContext>,
}

impl<'a, 'v> ErrorVerifierStandIn<'a, 'v> {
    fn declared_element(&self, node: NodeId) -> Option<ElementId> {
        let f = *self.v.tables.declared_fragment.get(node)?;
        self.v.ctx.fragment_data(f)?.element.try_get().copied()
    }

    /// The nearest enclosing executable node of [node] (a function or
    /// method declaration, a constructor, or a function expression that is
    /// not the body of a function declaration).
    fn enclosing_executable_node(&self, node: NodeId) -> Option<NodeId> {
        let ast = self.v.ast;
        let mut current = ast.parent(node);
        while let Some(n) = current {
            match ast.kind(n) {
                NodeKind::FunctionDeclaration
                | NodeKind::MethodDeclaration
                | NodeKind::ConstructorDeclaration => return Some(n),
                NodeKind::FunctionExpression
                    if !ast
                        .parent(n)
                        .is_some_and(|p| ast.is::<FunctionDeclaration>(p)) =>
                {
                    return Some(n);
                }
                NodeKind::PrimaryConstructorBody => return Some(n),
                _ => {}
            }
            current = ast.parent(n);
        }
        None
    }

    /// Dart `_withEnclosingExecutable(element, ...)` and
    /// `_returnTypeVerifier.verifyReturnType(returnType)`.
    fn executable(&mut self, function: NodeId) -> &mut EnclosingExecutableContext {
        if !self.executables.contains_key(&function) {
            let ast = self.v.ast;
            let ctx = self.v.ctx;
            // A primary constructor body has no declared fragment; its
            // executable (the primary constructor) is not looked up here.
            let element = self.declared_element(function);
            let flags = element
                .map(|e| first_fragment_flags(&ctx, e))
                .unwrap_or_default();
            let mut context = EnclosingExecutableContext::new(
                &ctx,
                element.map(ElemRef::Base),
                flags.contains(FragmentFlags::EXECUTABLE_FRAGMENT_IS_ASYNCHRONOUS),
                flags.contains(FragmentFlags::EXECUTABLE_FRAGMENT_IS_GENERATOR),
                None,
                None,
            );
            let return_type: Option<Id<TypeAnnotation>> =
                if let Some(f) = ast.cast::<FunctionDeclaration>(function) {
                    ast[f].return_type
                } else if let Some(m) = ast.cast::<MethodDeclaration>(function) {
                    ast[m].return_type
                } else {
                    None
                };
            if ast.is::<FunctionDeclaration>(function) || ast.is::<MethodDeclaration>(function) {
                rt::verify_return_type(self.v, &mut context, return_type);
            }
            self.executables.insert(function, context);
        }
        &mut self.executables[&function]
    }

    fn run(&mut self) {
        let ast = self.v.ast;
        let root = self.v.unit.raw();
        let mut nodes: Vec<NodeId> = (0..ast.node_count())
            .map(NodeId::from_index)
            .filter(|&n| support::is_attached(ast, root, n))
            .collect();
        nodes.sort_by_key(|&n| (ast.offset(n), std::cmp::Reverse(ast.length(n))));
        for node in nodes {
            self.visit(node);
        }
    }

    fn visit(&mut self, node: NodeId) {
        let ast = self.v.ast;
        let ctx = self.v.ctx;
        match ast.kind(node) {
            NodeKind::FunctionDeclaration | NodeKind::MethodDeclaration => {
                self.executable(node);
            }
            NodeKind::ClassDeclaration
            | NodeKind::EnumDeclaration
            | NodeKind::ExtensionTypeDeclaration => {
                self.add_constructors(node);
            }
            NodeKind::ConstructorReference => {
                ta::check_constructor_reference(self.v, Id::<ConstructorReference>::from_raw(node));
            }
            NodeKind::EnumConstantDeclaration => {
                let n = Id::<EnumConstantDeclaration>::from_raw(node);
                rp::visit_enum_constant_declaration(self.v, n);
                ta::check_enum_constant_declaration(self.v, n);
            }
            NodeKind::FunctionExpressionInvocation => {
                let n = Id::<FunctionExpressionInvocation>::from_raw(node);
                ta::check_function_expression_invocation(self.v, n);
                rp::visit_function_expression_invocation(self.v, n);
            }
            NodeKind::FunctionReference => {
                ta::check_function_reference(self.v, Id::<FunctionReference>::from_raw(node));
            }
            NodeKind::ListLiteral => {
                let n = Id::<ListLiteral>::from_raw(node);
                ta::check_list_literal(self.v, n);
                // Dart `_checkForListElementTypeNotAssignable`.
                let element_type = interface_type_arguments(self.v, n.raw()).first().copied();
                let verifier = LiteralElementVerifier {
                    for_list: true,
                    element_type,
                    ..Default::default()
                };
                for element in ast.list(ast[n].elements).to_vec() {
                    verifier.verify(self.v, element);
                }
            }
            NodeKind::SetOrMapLiteral => {
                let n = Id::<SetOrMapLiteral>::from_raw(node);
                let ty = self.v.tables.static_type.get(n).copied();
                let element = ty.and_then(|t| match ctx.ty(t) {
                    TypeKind::Interface { element, .. } => Some(*element),
                    _ => None,
                });
                let is_map = element == Some(ctx.tp.map_element().upcast());
                let is_set = element == Some(ctx.tp.set_element().upcast());
                let type_arguments = interface_type_arguments(self.v, n.raw());
                let elements = ast.list(ast[n].elements).to_vec();
                if is_map {
                    ta::check_map_literal(self.v, n);
                    if type_arguments.len() == 2 {
                        let verifier = LiteralElementVerifier {
                            for_map: true,
                            map_key_type: Some(type_arguments[0]),
                            map_value_type: Some(type_arguments[1]),
                            ..Default::default()
                        };
                        for element in elements {
                            verifier.verify(self.v, element);
                        }
                    }
                } else if is_set {
                    ta::check_set_literal(self.v, n);
                    if type_arguments.len() == 1 {
                        let verifier = LiteralElementVerifier {
                            for_set: true,
                            element_type: Some(type_arguments[0]),
                            ..Default::default()
                        };
                        for element in elements {
                            verifier.verify(self.v, element);
                        }
                    }
                }
            }
            NodeKind::MethodInvocation => {
                let n = Id::<MethodInvocation>::from_raw(node);
                ta::check_method_invocation(self.v, n);
                rp::visit_method_invocation(self.v, n);
            }
            NodeKind::NamedType => {
                ta::check_named_type(self.v, Id::<NamedType>::from_raw(node));
            }
            NodeKind::Annotation => {
                rp::visit_annotation(self.v, Id::<Annotation>::from_raw(node));
            }
            NodeKind::DotShorthandConstructorInvocation => {
                rp::visit_dot_shorthand_constructor_invocation(
                    self.v,
                    Id::<DotShorthandConstructorInvocation>::from_raw(node),
                );
            }
            NodeKind::DotShorthandInvocation => {
                rp::visit_dot_shorthand_invocation(
                    self.v,
                    Id::<DotShorthandInvocation>::from_raw(node),
                );
            }
            NodeKind::InstanceCreationExpression => {
                rp::visit_instance_creation_expression(
                    self.v,
                    Id::<InstanceCreationExpression>::from_raw(node),
                );
            }
            NodeKind::RedirectingConstructorInvocation => {
                rp::visit_redirecting_constructor_invocation(
                    self.v,
                    Id::<RedirectingConstructorInvocation>::from_raw(node),
                );
            }
            NodeKind::SuperConstructorInvocation => {
                let enclosing = self
                    .enclosing_executable_node(node)
                    .filter(|&n| ast.is::<ConstructorDeclaration>(n))
                    .and_then(|n| self.declared_element(n))
                    .map(ElemRef::Base);
                rp::visit_super_constructor_invocation(
                    self.v,
                    Id::<SuperConstructorInvocation>::from_raw(node),
                    enclosing,
                );
            }
            NodeKind::ReturnStatement => {
                if let Some(function) = self.enclosing_executable_node(node) {
                    let context = self.executable(function).clone();
                    rt::verify_return_statement(
                        self.v,
                        &context,
                        Id::<ReturnStatement>::from_raw(node),
                    );
                }
            }
            NodeKind::ExpressionFunctionBody => {
                let parent = ast.parent(node);
                if parent.is_some_and(|p| ast.is::<PrimaryConstructorBody>(p)) {
                    return;
                }
                if let Some(function) = self.enclosing_executable_node(node) {
                    let context = self.executable(function).clone();
                    rt::verify_expression_function_body(
                        self.v,
                        &context,
                        Id::<ExpressionFunctionBody>::from_raw(node),
                    );
                }
            }
            _ => {}
        }
    }

    /// Dart `libraryContext.constructorFieldsVerifier.addConstructors(...)`
    /// (visitClassDeclaration without a native clause,
    /// visitEnumDeclaration, visitExtensionTypeDeclaration).
    fn add_constructors(&mut self, node: NodeId) {
        let ast = self.v.ast;
        let Some(element) = self
            .declared_element(node)
            .and_then(|e| e.cast::<InterfaceElement>())
        else {
            return;
        };
        let (name_part, body) = if let Some(c) = ast.cast::<ClassDeclaration>(node) {
            if ast[c].native_clause.is_some() {
                return;
            }
            (ast[c].name_part, ast[c].body.raw())
        } else if let Some(e) = ast.cast::<EnumDeclaration>(node) {
            (ast[e].name_part, ast[e].body.raw())
        } else if let Some(e) = ast.cast::<ExtensionTypeDeclaration>(node) {
            (ast[e].name_part, ast[e].body.raw())
        } else {
            return;
        };
        let members = if let Some(b) = ast.cast::<dartr_ast::BlockClassBody>(body) {
            ast.list(ast[b].members).to_vec()
        } else if let Some(b) = ast.cast::<dartr_ast::BlockEnumBody>(body) {
            ast.list(ast[b].members).to_vec()
        } else {
            Vec::new()
        };
        let unit = self.v.index;
        self.constructor_fields
            .add_constructors(self.v, unit, element, &members, name_part);
    }
}

/// The type arguments of the static type of [node] (an interface type).
fn interface_type_arguments(v: &UnitVerifier<'_>, node: NodeId) -> Vec<TypeId> {
    let Some(&t) = v.tables.static_type.get(node) else {
        return Vec::new();
    };
    match v.ctx.ty(t) {
        TypeKind::Interface { args, .. } => v.ctx.list(*args).to_vec(),
        _ => Vec::new(),
    }
}

/// The diagnostics of the g4a codes of [case], as `(code, offset,
/// length)` sorted; `None` without an SDK.
fn run_case(case: &Case) -> Option<Vec<(String, usize, usize)>> {
    let a = support::analyze(&[("main.dart", case.source)])?;
    let unit: &ResolvedUnit = a.unit();
    if let Some(panic) = &unit.panic {
        return Some(vec![(format!("panic: {panic}"), 0, 0)]);
    }
    let ctx = a.ctx(unit);
    let parsed = dartr_ast_builder::parse_string(case.source, &unit.path);
    let scopes = LibraryScopes::build(&ctx, a.library.library);
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    {
        let mut v = UnitVerifier {
            ctx,
            type_system: TypeSystem::new(ctx),
            index: 0,
            path: &unit.path,
            uri: &unit.uri,
            parsed: &parsed,
            ast: &unit.ast,
            unit: unit.unit,
            tables: &unit.tables,
            rt: &unit.rt,
            library: a.library.library,
            fragment: unit.fragment,
            scopes: &scopes,
            options: AnalysisOptions::default(),
            features: parsed.feature_set,
            diagnostics: &mut diagnostics,
        };
        let mut stand_in = ErrorVerifierStandIn {
            v: &mut v,
            constructor_fields: ConstructorFieldsVerifier::default(),
            executables: IndexMap::new(),
        };
        stand_in.run();
        let mut constructor_fields = std::mem::take(&mut stand_in.constructor_fields);
        constructor_fields.report(std::slice::from_mut(&mut v));
    }
    // Dart `RecordingDiagnosticListener` keeps a set of diagnostics: equal
    // diagnostics (code, range and message) are reported once.
    let mut seen = std::collections::HashSet::new();
    let mut actual: Vec<(String, usize, usize)> = unit
        .diagnostics
        .iter()
        .chain(&diagnostics)
        .filter(|d| CODES.contains(&d.code.camel_case_name))
        .filter(|d| seen.insert((d.code.unique_name, d.offset, d.length, d.message.clone())))
        .map(|d| (d.code.camel_case_name.to_string(), d.offset, d.length))
        .collect();
    actual.sort();
    Some(actual)
}

/// Analyzer test cases that fail for reasons outside of the g4a
/// verifiers (`file::name`); see the comment of each.
const KNOWN_FAILURES: &[&str] = &[
    // Primary constructor bodies (`this : ...`) and the super invocation
    // of a primary constructor are not resolved by the resolver core.
    "positional_super_formal_parameter_with_positional_argument_test.dart::test_primaryConstructor_reported",
    "variable_not_initialized_test.dart::test_class_instanceField1_notFinal_typeInt_hasInitializer_primaryConstructor_fieldFormalParameter_constructorInitializer",
    "variable_not_initialized_test.dart::test_class_instanceField1_notFinal_typeInt_noInitializer_primaryConstructor_constructorInitializer2",
    "variable_not_initialized_test.dart::test_class_instanceField1_notFinal_typeInt_noInitializer_primaryConstructor_constructorInitializer3",
    "variable_not_initialized_test.dart::test_class_instanceField2_notFinal_typeInt_noInitializer_primaryConstructor_constructorInitializer2",
    "invalid_reference_to_this_test.dart::test_extensionType_primaryConstructor_fieldInitializer",
    // Reported by the error verifier itself (ErrorVerifier
    // `returnInGenerativeConstructor` at an expression body,
    // `checkForRecordLiteralOnePositional...` of arguments and variables),
    // or by the constant verifier (const sets).
    "record_literal_one_positional_no_trailing_comma_test.dart::test_argument_parenthesized",
    "record_literal_one_positional_no_trailing_comma_test.dart::test_declaration",
    "return_in_generative_constructor_test.dart::test_expressionFunctionBody",
    "constructor_body_test.dart::test_class_secondaryConstructor_constGenerative_expressionBody",
    "constructor_body_test.dart::test_class_secondaryConstructor_constGenerative_external_expressionBody",
    "constructor_body_test.dart::test_class_secondaryConstructor_constGenerativeRedirecting_expressionBody",
    "constructor_body_test.dart::test_class_secondaryConstructor_generative_external_expressionBody",
    "constructor_body_test.dart::test_enum_secondaryConstructor_constGenerative_expressionBody",
    "constructor_body_test.dart::test_enum_secondaryConstructor_constGenerative_external_expressionBody",
    "constructor_body_test.dart::test_enum_secondaryConstructor_constGenerativeRedirecting_expressionBody",
    "constructor_body_test.dart::test_extensionType_secondaryConstructor_constGenerative_expressionBody",
    "constructor_body_test.dart::test_extensionType_secondaryConstructor_constGenerative_external_expressionBody",
    "constructor_body_test.dart::test_extensionType_secondaryConstructor_constGenerativeRedirecting_expressionBody",
    "constructor_body_test.dart::test_extensionType_secondaryConstructor_generative_external_expressionBody",
    // The element of an augmented class has the augmentation as its
    // first fragment in the linked element model.
    "subtype_of_final_is_not_base_final_or_sealed_test.dart::test_class_extends_inAugmentation",
];

#[test]
fn g4a_verifiers_report_the_diagnostics_of_the_analyzer_tests() {
    let mut passed = 0;
    let mut failures = Vec::new();
    let mut unexpected_passes = Vec::new();
    for case in CASES {
        let Some(actual) = run_case(case) else {
            eprintln!("skipped: no Dart SDK on PATH");
            return;
        };
        let mut expected: Vec<(String, usize, usize)> = case
            .expected
            .iter()
            .map(|&(c, o, l)| (c.to_string(), o, l))
            .collect();
        expected.sort();
        let id = format!("{}::{}", case.file, case.name);
        let known = KNOWN_FAILURES.contains(&id.as_str());
        if actual == expected {
            passed += 1;
            if known {
                unexpected_passes.push(id);
            }
        } else if !known {
            failures.push(format!(
                "{id}\n  expected: {expected:?}\n  actual:   {actual:?}"
            ));
        }
    }
    eprintln!(
        "g4a analyzer cases: {passed}/{} pass ({} known failures)",
        CASES.len(),
        KNOWN_FAILURES.len()
    );
    assert!(
        failures.is_empty() && unexpected_passes.is_empty(),
        "{} failures:\n{}\nunexpected passes: {unexpected_passes:?}",
        failures.len(),
        failures.join("\n")
    );
}
