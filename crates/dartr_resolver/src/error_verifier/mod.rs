// Dart source: pkg/analyzer/lib/src/generated/error_verifier.dart
// (ErrorVerifier state, EnclosingExecutableContext, HiddenElements,
// LibraryVerificationContext, ThisContext, _NullAwareKind, the `_with*`
// helpers and the visitor dispatch)

//! `ErrorVerifier`: the visitor that runs after resolution and reports the
//! errors and warnings that the parser and the resolver do not report
//! (Dart `LibraryAnalyzer._computeVerifyErrors`).
//!
//! The Dart file is split by sections (design `docs/design/semantics.md`
//! wave D, D4–D7). Each section is an `impl ErrorVerifier` block in its own
//! module, with the `visitX` methods and the `_checkX` helpers of that
//! section (Dart names in snake case):
//!
//! - [`declarations`]: class, enum, mixin, extension, extension type and
//!   type alias declarations, directives, the compilation unit (D4).
//! - [`statements_expressions`]: statements and expressions (D5).
//! - [`constructors_fields`]: constructors, initializers, fields and
//!   variables (D6).
//! - [`type_arguments_misc`]: type arguments, type parameters, formal
//!   parameters, functions and methods, variance, operators (D7).
//!
//! This module has the state of the Dart class, the helper classes and the
//! [`AstVisitor`] dispatch: the `visit_x(ast, node)` method of the trait
//! calls the inherent `visit_x(node)` of the section. `super.visitX(node)`
//! is [`ErrorVerifier::visit_children`], `node.accept(this)` is
//! [`ErrorVerifier::accept`].

pub mod constructors_fields;
pub mod declarations;
pub mod statements_expressions;
pub mod type_arguments_misc;

use dartr_ast::{Ast, AstVisitor, Id, NodeId};
use dartr_diagnostics::{Diagnostic, LocatableDiagnostic, LocatedDiagnostic};
use dartr_element::{
    Ctx, EId, ElementId, ExtensionElement, FId, InterfaceElement, LibraryElement, LibraryFragment,
    ResolutionTables, TypeId,
};
use dartr_syntax::TokenId;
use dartr_typesystem::inheritance_manager3::InheritanceManager3;
use dartr_typesystem::{TypeExt, TypeSystem};
use indexmap::IndexSet;

use crate::error::constructor_fields_verifier::ConstructorFieldsVerifier;
use crate::error::duplicate_definition_verifier::DuplicationDefinitionContext;
use crate::options::AnalysisOptions;
use crate::resolver::UnitContext;
use crate::tables::ResolverTables;

/// Dart `EnclosingExecutableContext`.
#[derive(Clone, Debug, Default)]
pub struct EnclosingExecutableContext {
    /// Dart `element` (an executable element: constructor, method,
    /// accessor, top-level or local function).
    pub element: Option<ElementId>,
    pub is_asynchronous: bool,
    pub is_const_constructor: bool,
    pub is_generative_constructor: bool,
    pub is_generator: bool,
    pub in_factory_constructor: bool,
    pub in_static_method: bool,
    /// Dart `catchErrorOnErrorReturnType`.
    pub catch_error_on_error_return_type: Option<TypeId>,
    /// Dart `thenOnErrorReturnType`.
    pub then_on_error_return_type: Option<TypeId>,
    /// Dart `_returnsWith`: the return statements that have a value.
    pub returns_with: Vec<NodeId>,
    /// Dart `_returnsWithout`: the return statements without a value.
    pub returns_without: Vec<NodeId>,
    /// Dart `hasLegalReturnType`.
    pub has_legal_return_type: bool,
    /// Dart `catchClauseLevel`.
    pub catch_clause_level: u32,
}

impl EnclosingExecutableContext {
    /// Dart `EnclosingExecutableContext.empty()`.
    pub fn empty() -> EnclosingExecutableContext {
        EnclosingExecutableContext {
            has_legal_return_type: true,
            ..EnclosingExecutableContext::default()
        }
    }

    /// Dart `isSynchronous`.
    pub fn is_synchronous(&self) -> bool {
        !self.is_asynchronous
    }
}

/// Dart `HiddenElements`: the elements that will be declared in a scope
/// (block) but are not declared yet. A stack of scopes; the outermost
/// scope is first.
#[derive(Clone, Debug, Default)]
pub struct HiddenElements {
    /// One set per scope (Dart `outerElements` chain).
    pub scopes: Vec<IndexSet<ElementId>>,
}

impl HiddenElements {
    /// Dart `contains(element)`.
    pub fn contains(&self, element: ElementId) -> bool {
        self.scopes.iter().any(|s| s.contains(&element))
    }

    /// Dart `declare(element)`: removes [element] from the innermost scope.
    pub fn declare(&mut self, element: ElementId) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.shift_remove(&element);
        }
    }
}

/// Dart `LibraryVerificationContext`: information shared by the units of a
/// library (one per library analysis).
#[derive(Default)]
pub struct LibraryVerificationContext {
    /// Dart `duplicationDefinitionContext`.
    pub duplication_definition_context: DuplicationDefinitionContext,
    /// Dart `constructorFieldsVerifier`.
    pub constructor_fields_verifier: ConstructorFieldsVerifier,
    /// Dart `libraryKind.libraryCycle.libraryUris` (for
    /// `libraryCycleContains`). The driver does not provide the cycle yet:
    /// empty means "only the library itself".
    pub library_cycle_uris: IndexSet<String>,
    /// Dart `_exportedElements`.
    pub exported_elements: indexmap::IndexMap<String, ElementId>,
    /// Dart `_setOfImplementsMap`.
    pub set_of_implements_map: indexmap::IndexMap<ElementId, IndexSet<ElementId>>,
    /// Dart `_setOfOnMaps`.
    pub set_of_on_maps: indexmap::IndexMap<ElementId, IndexSet<ElementId>>,
}

impl LibraryVerificationContext {
    /// Dart `libraryCycleContains(uri)`.
    pub fn library_cycle_contains(&self, uri: &str) -> bool {
        self.library_cycle_uris.contains(uri)
    }

    /// Dart `setOfImplements(declaration)`.
    pub fn set_of_implements(&mut self, declaration: ElementId) -> &mut IndexSet<ElementId> {
        self.set_of_implements_map.entry(declaration).or_default()
    }

    /// Dart `setOfOn(declaration)`.
    pub fn set_of_on(&mut self, declaration: ElementId) -> &mut IndexSet<ElementId> {
        self.set_of_on_maps.entry(declaration).or_default()
    }
}

/// Dart `ThisContext`: the semantic location related to explicit or
/// implicit `this` access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThisContext {
    ConstructorInitializers,
    FactoryConstructorBody,
    GenerativeConstructorBody,
    InstanceFieldDeclaration,
    InstanceMemberBody,
    LateInstanceFieldDeclaration,
    StaticFieldDeclaration,
    StaticMemberBody,
    TopLevel,
}

impl ThisContext {
    /// Dart `allowsThis`.
    pub fn allows_this(self) -> bool {
        matches!(
            self,
            ThisContext::GenerativeConstructorBody
                | ThisContext::InstanceMemberBody
                | ThisContext::LateInstanceFieldDeclaration
        )
    }
}

/// Dart `_NullAwareKind`: the kinds of null-aware accesses of
/// `_checkForUnnecessaryNullAware`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NullAwareKind {
    IndexExpression,
    Element,
    MapEntryKey,
    MapEntryValue,
    Access,
    Cascaded,
    Spread,
    NullCheck,
}

impl NullAwareKind {
    /// Dart `canParticipateInShortCircuiting`.
    pub fn can_participate_in_short_circuiting(self) -> bool {
        matches!(
            self,
            NullAwareKind::IndexExpression | NullAwareKind::Access | NullAwareKind::Cascaded
        )
    }

    /// Dart `locatableDiagnostic(becauseOfShortCircuiting:)`.
    pub fn locatable_diagnostic(self, because_of_short_circuiting: bool) -> LocatableDiagnostic {
        use dartr_diagnostics::diag;
        let (operator, replacement) = match self {
            NullAwareKind::Element => return diag::invalid_null_aware_element(),
            NullAwareKind::MapEntryKey => return diag::invalid_null_aware_map_entry_key(),
            NullAwareKind::MapEntryValue => return diag::invalid_null_aware_map_entry_value(),
            NullAwareKind::NullCheck => return diag::unnecessary_non_null_assertion(),
            NullAwareKind::IndexExpression => ("?[", "["),
            NullAwareKind::Access => ("?.", "."),
            NullAwareKind::Cascaded => ("?..", ".."),
            NullAwareKind::Spread => ("?...", "..."),
        };
        if because_of_short_circuiting {
            diag::invalid_null_aware_operator_after_short_circuit(operator, replacement)
        } else {
            diag::invalid_null_aware_operator(operator, replacement)
        }
    }
}

/// Dart `ErrorVerifier`.
pub struct ErrorVerifier<'a> {
    /// The lookup context, with the unit's local arena.
    pub ctx: Ctx<'a>,
    /// Dart `typeSystem`.
    pub type_system: TypeSystem<'a>,
    /// Dart `_inheritanceManager`.
    pub inheritance: InheritanceManager3<'a>,
    /// The resolved AST of the unit.
    pub ast: &'a Ast,
    /// The resolution results of the unit.
    pub tables: &'a ResolutionTables,
    /// The resolver-private node data of the unit.
    pub rt: &'a ResolverTables,
    /// Dart `diagnosticReporter`: the diagnostics of the unit.
    pub diagnostics: &'a mut Vec<Diagnostic>,
    /// Dart `_currentLibrary`, `_currentUnit`, `options`, the scopes and
    /// the features of the unit.
    pub unit: UnitContext<'a>,
    /// Dart `libraryContext`.
    pub library_context: &'a mut LibraryVerificationContext,
    /// Dart `_isInComment`.
    pub is_in_comment: bool,
    /// Dart `_isInLateLocalVariable`.
    pub is_in_late_local_variable: Vec<bool>,
    /// Dart `_isInSystemLibrary`.
    pub is_in_system_library: bool,
    /// Dart `_enclosingClass`.
    pub enclosing_class: Option<EId<InterfaceElement>>,
    /// Dart `_enclosingExtension`.
    pub enclosing_extension: Option<EId<ExtensionElement>>,
    /// Dart `_thisContextStack`.
    pub this_context_stack: Vec<ThisContext>,
    /// Dart `_enclosingExecutable` (also Dart
    /// `_returnTypeVerifier.enclosingExecutable`).
    pub enclosing_executable: EnclosingExecutableContext,
    /// Dart `_namesForReferenceToDeclaredVariableInInitializer`.
    pub names_for_reference_to_declared_variable_in_initializer: IndexSet<String>,
    /// Dart `_hiddenElements` (`None`: no scope).
    pub hidden_elements: Option<HiddenElements>,
}

impl<'a> ErrorVerifier<'a> {
    /// Dart `ErrorVerifier(...)`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ctx: Ctx<'a>,
        ast: &'a Ast,
        tables: &'a ResolutionTables,
        rt: &'a ResolverTables,
        diagnostics: &'a mut Vec<Diagnostic>,
        unit: UnitContext<'a>,
        library_context: &'a mut LibraryVerificationContext,
    ) -> ErrorVerifier<'a> {
        let is_in_system_library = ctx.library_uri(unit.library).starts_with("dart:");
        ErrorVerifier {
            ctx,
            type_system: TypeSystem::new(ctx),
            inheritance: InheritanceManager3::new(ctx.global()),
            ast,
            tables,
            rt,
            diagnostics,
            unit,
            library_context,
            is_in_comment: false,
            is_in_late_local_variable: vec![false],
            is_in_system_library,
            enclosing_class: None,
            enclosing_extension: None,
            this_context_stack: vec![ThisContext::TopLevel],
            enclosing_executable: EnclosingExecutableContext::empty(),
            names_for_reference_to_declared_variable_in_initializer: IndexSet::new(),
            hidden_elements: None,
        }
    }

    /// Dart `_currentLibrary`.
    pub fn current_library(&self) -> EId<LibraryElement> {
        self.unit.library
    }

    /// Dart `_currentUnit`.
    pub fn current_unit(&self) -> FId<LibraryFragment> {
        self.unit.fragment
    }

    /// Dart `options` (the resolver part).
    pub fn options(&self) -> AnalysisOptions {
        self.unit.options
    }

    /// Dart `strictCasts`.
    pub fn strict_casts(&self) -> bool {
        self.unit.options.strict_casts
    }

    /// Dart `_thisContext`.
    pub fn this_context(&self) -> ThisContext {
        *self.this_context_stack.last().expect("this context")
    }

    /// Dart `diagnosticReporter.report(diagnostic)`.
    pub fn report(&mut self, diagnostic: LocatedDiagnostic) {
        self.diagnostics.push(diagnostic.into_diagnostic());
    }

    /// Dart `diagnostic.at(node)`.
    pub fn at(
        &self,
        diagnostic: LocatableDiagnostic,
        node: impl Into<NodeId>,
    ) -> LocatedDiagnostic {
        let node = node.into();
        diagnostic.at_offset(
            self.ast.offset(node) as usize,
            self.ast.length(node) as usize,
        )
    }

    /// Dart `diagnostic.at(token)`.
    pub fn at_token(&self, diagnostic: LocatableDiagnostic, token: TokenId) -> LocatedDiagnostic {
        let t = self.ast.tokens.get(token);
        diagnostic.at_offset(t.offset as usize, (t.end() - t.offset) as usize)
    }

    /// Dart `diagnosticReporter.report(diagnostic.at(node))`.
    pub fn report_at(&mut self, diagnostic: LocatableDiagnostic, node: impl Into<NodeId>) {
        let d = self.at(diagnostic, node);
        self.report(d);
    }

    /// Dart `diagnosticReporter.report(diagnostic.at(token))`.
    pub fn report_at_token(&mut self, diagnostic: LocatableDiagnostic, token: TokenId) {
        let d = self.at_token(diagnostic, token);
        self.report(d);
    }

    /// Dart `node.staticType`.
    pub fn static_type(&self, node: impl Into<NodeId>) -> Option<TypeId> {
        self.tables.static_type.get(node).copied()
    }

    /// Dart `node.element` (`PrefixedIdentifier.element` is the element of
    /// its identifier).
    pub fn element(&self, node: impl Into<NodeId>) -> Option<dartr_element::ElemRef> {
        let node = node.into();
        if let Some(prefixed) = self.ast.cast::<dartr_ast::PrefixedIdentifier>(node) {
            return self
                .tables
                .element
                .get(self.ast[prefixed].identifier)
                .copied();
        }
        self.tables.element.get(node).copied()
    }

    /// Dart `super.visitX(node)` (`node.visitChildren(this)`).
    pub fn visit_children(&mut self, node: impl Into<NodeId>) {
        let ast = self.ast;
        ast.visit_children(node, self);
    }

    /// Dart `node.accept(this)`.
    pub fn accept(&mut self, node: impl Into<NodeId>) {
        let ast = self.ast;
        ast.accept(node, self);
    }

    /// Dart `node?.accept(this)`.
    pub fn accept_opt<T>(&mut self, node: Option<Id<T>>) {
        if let Some(node) = node {
            self.accept(node.raw());
        }
    }

    /// Dart `_withEnclosingExecutable(element, operation, isAsynchronous:,
    /// isGenerator:)`.
    pub fn with_enclosing_executable(
        &mut self,
        context: EnclosingExecutableContext,
        operation: impl FnOnce(&mut Self),
    ) {
        let current = std::mem::replace(&mut self.enclosing_executable, context);
        operation(self);
        self.enclosing_executable = current;
    }

    /// Dart `_withHiddenElements(hiddenElements, f)`: runs [f] with a new
    /// innermost scope of hidden [elements].
    pub fn with_hidden_elements(
        &mut self,
        elements: IndexSet<ElementId>,
        f: impl FnOnce(&mut Self),
    ) {
        let outer = self.hidden_elements.clone();
        let mut hidden = outer.clone().unwrap_or_default();
        hidden.scopes.push(elements);
        self.hidden_elements = Some(hidden);
        f(self);
        self.hidden_elements = outer;
    }

    /// Dart `_withThisContext(state, f)`.
    pub fn with_this_context(&mut self, state: ThisContext, f: impl FnOnce(&mut Self)) {
        self.this_context_stack.push(state);
        f(self);
        self.this_context_stack.pop();
    }
}

/// Dart `LibraryAnalyzer._computeVerifyErrors` (the ErrorVerifier part):
/// runs the error verifier on one resolved unit.
#[allow(clippy::too_many_arguments)]
pub fn verify_unit(
    ctx: Ctx<'_>,
    ast: &Ast,
    root: Id<dartr_ast::CompilationUnit>,
    tables: &ResolutionTables,
    rt: &ResolverTables,
    diagnostics: &mut Vec<Diagnostic>,
    unit: UnitContext<'_>,
    library_context: &mut LibraryVerificationContext,
) {
    let mut verifier = ErrorVerifier::new(ctx, ast, tables, rt, diagnostics, unit, library_context);
    verifier.accept(root);
}

/// Dart `LibraryAnalyzer._computeVerifyErrors(fileAnalysis, ...)`: the
/// ErrorVerifier part for one resolved unit of the library. A unit whose
/// resolution panicked is skipped. A panic of the verifier drops the
/// diagnostics of the verifier for that unit (set `DARTR_DEBUG_VERIFY=1`
/// to print the panic to stderr).
pub fn compute_verify_errors(
    input: &crate::library_analyzer::LibraryAnalysisInput<'_>,
    scopes: &crate::scope::LibraryScopes,
    library_features: &dartr_element::FeatureSet,
    unit: &mut crate::library_analyzer::ResolvedUnit,
    library_context: &mut LibraryVerificationContext,
) {
    if unit.panic.is_some() {
        return;
    }
    let Some(unit_input) = input.units.iter().find(|u| u.fragment == unit.fragment) else {
        return;
    };
    let sink = dartr_element::NoopSink;
    let ctx = Ctx {
        world: input.world,
        current: None,
        local: Some(&unit.local),
        tp: input.type_provider,
        features: library_features,
        req: &sink,
    };
    let unit_ctx = UnitContext {
        library: input.library,
        fragment: unit.fragment,
        scopes,
        options: input.options,
        features: unit_input.parsed.feature_set,
    };
    let reported = unit.diagnostics.len();
    // Dart `RecordingDiagnosticListener` keeps a set: equal diagnostics
    // (the resolver and the verifier can report the same one) are kept
    // once, in report order.
    let dedupe = |diagnostics: &mut Vec<Diagnostic>| {
        let mut seen = IndexSet::new();
        diagnostics.retain(|d| {
            seen.insert((
                d.code as *const dartr_diagnostics::DiagnosticCode,
                d.offset,
                d.length,
                d.message.clone(),
            ))
        });
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        verify_unit(
            ctx,
            &unit.ast,
            unit.unit,
            &unit.tables,
            &unit.rt,
            &mut unit.diagnostics,
            unit_ctx,
            library_context,
        );
        // Dart: `unit.accept(FfiVerifier(...))` after the ErrorVerifier.
        crate::ffi_verifier::verify_unit(
            ctx,
            &unit.ast,
            unit.unit,
            &unit.tables,
            &unit.rt,
            &mut unit.diagnostics,
            unit_ctx,
        );
    }));
    if result.is_ok() {
        dedupe(&mut unit.diagnostics);
    }
    if let Err(e) = result {
        unit.diagnostics.truncate(reported);
        dedupe(&mut unit.diagnostics);
        if std::env::var_os("DARTR_DEBUG_VERIFY").is_some() {
            let message = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "panic".to_string());
            eprintln!("error verifier panic in {}: {message}", unit.path);
        }
    }
}

/// The [`AstVisitor`] dispatch: each Dart `visitX` override of
/// `ErrorVerifier` calls the inherent method of its section.
macro_rules! dispatch {
    ($($method:ident: $kind:ident),* $(,)?) => {
        impl AstVisitor for ErrorVerifier<'_> {
            $(
                fn $method(&mut self, _ast: &Ast, node: Id<dartr_ast::$kind>) {
                    ErrorVerifier::$method(self, node);
                }
            )*
        }
    };
}

dispatch! {
    visit_annotation: Annotation,
    visit_anonymous_method_invocation: AnonymousMethodInvocation,
    visit_as_expression: AsExpression,
    visit_assigned_variable_pattern: AssignedVariablePattern,
    visit_assignment_expression: AssignmentExpression,
    visit_await_expression: AwaitExpression,
    visit_binary_expression: BinaryExpression,
    visit_block: Block,
    visit_break_statement: BreakStatement,
    visit_catch_clause: CatchClause,
    visit_class_declaration: ClassDeclaration,
    visit_class_type_alias: ClassTypeAlias,
    visit_comment: Comment,
    visit_compilation_unit: CompilationUnit,
    visit_constructor_declaration: ConstructorDeclaration,
    visit_constructor_field_initializer: ConstructorFieldInitializer,
    visit_constructor_reference: ConstructorReference,
    visit_dot_shorthand_constructor_invocation: DotShorthandConstructorInvocation,
    visit_dot_shorthand_invocation: DotShorthandInvocation,
    visit_enum_constant_declaration: EnumConstantDeclaration,
    visit_enum_declaration: EnumDeclaration,
    visit_export_directive: ExportDirective,
    visit_expression_function_body: ExpressionFunctionBody,
    visit_extension_declaration: ExtensionDeclaration,
    visit_extension_type_declaration: ExtensionTypeDeclaration,
    visit_field_declaration: FieldDeclaration,
    visit_field_formal_parameter: FieldFormalParameter,
    visit_for_each_parts_with_declaration: ForEachPartsWithDeclaration,
    visit_for_each_parts_with_identifier: ForEachPartsWithIdentifier,
    visit_for_element: ForElement,
    visit_formal_parameter_list: FormalParameterList,
    visit_for_parts_with_declarations: ForPartsWithDeclarations,
    visit_for_statement: ForStatement,
    visit_function_declaration: FunctionDeclaration,
    visit_function_expression: FunctionExpression,
    visit_function_expression_invocation: FunctionExpressionInvocation,
    visit_function_reference: FunctionReference,
    visit_function_type_alias: FunctionTypeAlias,
    visit_generic_type_alias: GenericTypeAlias,
    visit_guarded_pattern: GuardedPattern,
    visit_import_directive: ImportDirective,
    visit_import_prefix_reference: ImportPrefixReference,
    visit_index_expression: IndexExpression,
    visit_instance_creation_expression: InstanceCreationExpression,
    visit_integer_literal: IntegerLiteral,
    visit_interpolation_expression: InterpolationExpression,
    visit_is_expression: IsExpression,
    visit_list_literal: ListLiteral,
    visit_map_literal_entry: MapLiteralEntry,
    visit_method_declaration: MethodDeclaration,
    visit_method_invocation: MethodInvocation,
    visit_mixin_declaration: MixinDeclaration,
    visit_named_type: NamedType,
    visit_native_clause: NativeClause,
    visit_native_function_body: NativeFunctionBody,
    visit_null_aware_element: NullAwareElement,
    visit_pattern_variable_declaration_statement: PatternVariableDeclarationStatement,
    visit_postfix_expression: PostfixExpression,
    visit_prefixed_identifier: PrefixedIdentifier,
    visit_prefix_expression: PrefixExpression,
    visit_primary_constructor_body: PrimaryConstructorBody,
    visit_primary_constructor_declaration: PrimaryConstructorDeclaration,
    visit_property_access: PropertyAccess,
    visit_redirecting_constructor_invocation: RedirectingConstructorInvocation,
    visit_regular_formal_parameter: RegularFormalParameter,
    visit_rethrow_expression: RethrowExpression,
    visit_return_statement: ReturnStatement,
    visit_set_or_map_literal: SetOrMapLiteral,
    visit_simple_identifier: SimpleIdentifier,
    visit_spread_element: SpreadElement,
    visit_super_constructor_invocation: SuperConstructorInvocation,
    visit_super_formal_parameter: SuperFormalParameter,
    visit_switch_case: SwitchCase,
    visit_switch_default: SwitchDefault,
    visit_switch_expression: SwitchExpression,
    visit_switch_pattern_case: SwitchPatternCase,
    visit_switch_statement: SwitchStatement,
    visit_this_expression: ThisExpression,
    visit_throw_expression: ThrowExpression,
    visit_top_level_variable_declaration: TopLevelVariableDeclaration,
    visit_type_argument_list: TypeArgumentList,
    visit_type_parameter: TypeParameter,
    visit_type_parameter_list: TypeParameterList,
    visit_variable_declaration: VariableDeclaration,
    visit_variable_declaration_list: VariableDeclarationList,
    visit_variable_declaration_statement: VariableDeclarationStatement,
}
