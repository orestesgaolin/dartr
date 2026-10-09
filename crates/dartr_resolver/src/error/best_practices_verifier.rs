// Dart source: pkg/analyzer/lib/src/error/best_practices_verifier.dart

//! `BestPracticesVerifier`: the warnings about Dart best practices
//! (unnecessary casts and type checks, null comparisons, `@doNotStore`,
//! `@nonVirtual`, `@sealed`, `@internal`, strict inference, ...), and the
//! verifiers it drives: annotations, deprecated and experimental element
//! usage, invalid access (`_InvalidAccessVerifier`: `@protected`,
//! `@visibleForTesting`, `@visibleForOverriding`, `@visibleForTemplate`,
//! `@internal`), `@mustCallSuper`, `@immutable`, null-safe APIs and widget
//! previews.
//!
//! Not wired here (other wave D groups port them): `ErrorHandlerVerifier`
//! (`verifyMethodInvocation`) and `DocCommentVerifier` (`docImport`,
//! `docDirective`); their Dart call sites are marked below.
//!
//! Differences that wait for the constant evaluation (D1–D2):
//! - `_checkForDuplications` compares `computeConstantValue()` of the keys
//!   and elements; here they are compared when they are literals (`null`,
//!   booleans, integers, doubles, strings without interpolation), which is
//!   the result of the constant evaluation for literals.
//! - `canBeConst` of `@literal` constructor invocations runs the constant
//!   verifier in Dart; here the arguments must be constant syntactically
//!   (see [`can_be_const`]).

use std::sync::Arc;

use dartr_ast::{
    Annotation, AsExpression, AssignmentExpression, Ast, AstVisitor, BinaryExpression,
    BlockFunctionBody, BooleanLiteral, CastPattern, CatchClause, ClassDeclaration, ClassTypeAlias,
    CommentReference, CompilationUnit, ConstantPattern, ConstructorDeclaration, ConstructorName,
    DotShorthandConstructorInvocation, DotShorthandInvocation, DotShorthandPropertyAccess,
    DoubleLiteral, EmptyFunctionBody, EnumDeclaration, ExportDirective, Expression,
    ExpressionFunctionBody, ExtensionDeclaration, ExtensionOverride, ExtensionTypeDeclaration,
    FieldDeclaration, FieldFormalParameter, FormalParameterList, FunctionDeclaration,
    FunctionDeclarationStatement, FunctionExpression, FunctionExpressionInvocation,
    FunctionTypeAlias, GenericFunctionType, GenericTypeAlias, Id, Identifier, ImportDirective,
    IndexExpression, InstanceCreationExpression, IntegerLiteral, IsExpression, MapLiteralEntry,
    MethodDeclaration, MethodInvocation, MixinDeclaration, NamedArgument, NamedType, NodeId,
    NullLiteral, PatternField, PostfixExpression, PrefixExpression, PrefixedIdentifier,
    PrimaryConstructorBody, PrimaryConstructorDeclaration, PropertyAccess,
    RedirectingConstructorInvocation, RegularFormalParameter, ReturnStatement, SetOrMapLiteral,
    SimpleIdentifier, SimpleStringLiteral, SuperConstructorInvocation, SuperExpression,
    SuperFormalParameter, TopLevelVariableDeclaration, VariableDeclarationList,
};
use dartr_diagnostics::{LocatableDiagnostic, diag};
use dartr_element::{
    ElementId, InterfaceElement, LibraryElement, NodeFlags, Tag, TypeId, TypeKind,
};
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_syntax::{TokenId, TokenType};
use dartr_typesystem::inheritance_manager3::{InheritanceManager3, Name};
use dartr_typesystem::{TypeExt, member};
use indexmap::IndexSet;

use super::annotation_verifier::AnnotationVerifier;
use super::element_usage_detector::UsageSet;
use super::element_usage_frontier_detector::ElementUsageFrontierDetector;
use super::support::{
    Range, class_name_token, corresponding_parameter, declared_element, display_name, element_of,
    enclosing_of, in_comment_reference, in_declaration_context, library_export, library_import,
    library_of, node_range, token_range, uri_library, write_or_read_element,
};
use super::{
    UnitVerifier, VerifierHost, deprecated_functionality_verifier as deprecated_functionality,
    immutable_verifier, must_call_super_verifier, null_safe_api_verifier, widget_preview_verifier,
};
use crate::ast_ext::formal_parameter_parts;
use crate::element_metadata::{
    UnitAst, WorkspacePackage, accessor_variable, element_has, flags, has_or_inherits_do_not_store,
    is_internal, is_protected, is_visible_for_testing,
};

/// Dart `unit.accept(BestPracticesVerifier(...))`.
pub fn verify(v: &mut UnitVerifier<'_>) {
    let ast = v.ast;
    let unit = v.unit;
    let mut verifier = BestPracticesVerifier::new(v);
    ast.accept(unit, &mut verifier);
}

/// Dart `BestPracticesVerifier` (with its `_InvalidAccessVerifier`).
struct BestPracticesVerifier<'v, 'a> {
    v: &'v mut UnitVerifier<'a>,
    /// Dart `_enclosingClass` (also `_InvalidAccessVerifier._enclosingClass`).
    enclosing_class: Option<ElementId>,
    /// Dart `_inDoNotStoreMember`.
    in_do_not_store_member: bool,
    annotation_verifier: AnnotationVerifier,
    element_usage_frontier_detector: ElementUsageFrontierDetector,
    /// Dart `_workspacePackage`.
    workspace_package: Option<Arc<WorkspacePackage>>,
    /// Dart `_strictInference`.
    strict_inference: bool,
    /// Dart `_inPackagePublicApi`.
    in_package_public_api: bool,
    /// Dart `_inPrimaryConstructorDeclaration`.
    in_primary_constructor_declaration: bool,
    /// Dart `_InvalidAccessVerifier._inTemplateSource`.
    in_template_source: bool,
    /// Dart `_InvalidAccessVerifier._inTestDirectory`.
    in_test_directory: bool,
}

impl<'v, 'a> BestPracticesVerifier<'v, 'a> {
    fn new(v: &'v mut UnitVerifier<'a>) -> Self {
        let library_path = {
            let library = v.ctx.get(v.library);
            v.ctx.fragment(library.first_fragment()).source.path.clone()
        };
        let workspace_package = WorkspacePackage::find_for(&library_path);
        // Dart `fileAnalysis.file.isInTestDirectory` (the file of the unit).
        let in_test_directory =
            WorkspacePackage::find_for(v.path).is_some_and(|p| p.is_in_test_directory(v.path));
        let in_package_public_api = workspace_package
            .as_ref()
            .is_some_and(|p| p.source_is_in_public_api(&library_path));
        let annotation_verifier = AnnotationVerifier::new(v, workspace_package.clone());
        let mut detector = ElementUsageFrontierDetector::new(
            workspace_package.clone(),
            vec![
                UsageSet::Deprecated,
                UsageSet::Experimental,
                UsageSet::DoNotSubmit,
            ],
        );
        detector.push_element(v, Some(v.library.raw()));
        let in_do_not_store_member =
            element_has(&v.ctx, v.library.raw(), flags::DO_NOT_STORE, None);
        BestPracticesVerifier {
            strict_inference: v.options.strict_inference,
            in_template_source: library_path.contains(".template"),
            v,
            enclosing_class: None,
            in_do_not_store_member,
            annotation_verifier,
            element_usage_frontier_detector: detector,
            workspace_package,
            in_package_public_api,
            in_primary_constructor_declaration: false,
            in_test_directory,
        }
    }

    fn unit_ast(&self) -> Option<UnitAst<'a>> {
        Some(UnitAst {
            ast: self.v.ast,
            tables: self.v.tables,
        })
    }

    fn report(&mut self, d: LocatableDiagnostic, range: Range) {
        self.v
            .report(d.at_offset(range.0 as usize, range.1 as usize));
    }

    fn report_node(&mut self, d: LocatableDiagnostic, node: impl Into<NodeId>) {
        let range = node_range(self.v.ast, node);
        self.report(d, range);
    }

    fn report_token(&mut self, d: LocatableDiagnostic, token: TokenId) {
        let range = token_range(self.v.ast, token);
        self.report(d, range);
    }

    fn declared(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        declared_element(&self.v.ctx, self.v.tables, node)
    }

    fn static_type(&self, node: impl Into<NodeId>) -> Option<TypeId> {
        self.v.tables.static_type.get(node.into()).copied()
    }

    /// Dart `typeAnnotation.type`.
    fn annotation_type(&self, node: impl Into<NodeId>) -> Option<TypeId> {
        self.v.tables.annotation_type.get(node.into()).copied()
    }

    fn has(&self, element: ElementId, flag: u32) -> bool {
        element_has(&self.v.ctx, element, flag, self.unit_ast())
    }

    fn push_element(&mut self, element: Option<ElementId>) {
        self.element_usage_frontier_detector
            .push_element(self.v, element);
    }

    fn pop_element(&mut self) {
        self.element_usage_frontier_detector.pop_element();
    }

    fn is_enabled(&self, flag: ExperimentalFlag) -> bool {
        self.v.features.is_experiment_enabled(flag)
    }

    /// Dart `firstVariableElement` of a field or top-level variable
    /// declaration.
    fn first_variable_element(&self, list: Id<VariableDeclarationList>) -> Option<ElementId> {
        let ast = self.v.ast;
        let first = *ast.list(ast[list].variables).first()?;
        self.declared(first)
    }
}

impl AstVisitor for BestPracticesVerifier<'_, '_> {
    fn visit_annotation(&mut self, ast: &Ast, node: Id<Annotation>) {
        self.annotation_verifier.check_annotation(self.v, node);
        widget_preview_verifier::check_annotation(self.v, node);
        self.element_usage_frontier_detector
            .annotation(self.v, node);
        ast.visit_children(node, self);
    }

    fn visit_as_expression(&mut self, ast: &Ast, node: Id<AsExpression>) {
        if self.is_unnecessary_cast(node) {
            self.report_node(diag::unnecessary_cast(), node);
        }
        if let Some(ty) = self.annotation_type(ast[node].type_)
            && self.v.type_system.is_non_nullable(ty)
            && self
                .static_type(ast[node].expression)
                .is_some_and(|t| self.v.ctx.is_dart_core_null(t))
        {
            self.report_node(diag::cast_from_null_always_fails(), node);
        }
        ast.visit_children(node, self);
    }

    fn visit_assignment_expression(&mut self, ast: &Ast, node: Id<AssignmentExpression>) {
        self.element_usage_frontier_detector
            .assignment_expression(self.v, node);
        ast.visit_children(node, self);
    }

    fn visit_binary_expression(&mut self, ast: &Ast, node: Id<BinaryExpression>) {
        self.element_usage_frontier_detector
            .binary_expression(self.v, node);
        self.check_for_invariant_nan_comparison(node);
        self.check_for_invariant_null_comparison(node);
        self.verify_binary(node);
        ast.visit_children(node, self);
    }

    fn visit_cast_pattern(&mut self, ast: &Ast, node: Id<CastPattern>) {
        let ty = self.annotation_type(ast[node].type_);
        let matched_value_type = self
            .v
            .tables
            .pattern_info
            .get(node.raw())
            .and_then(|i| i.matched_value_type);
        if let Some(ty) = ty
            && self.v.type_system.is_non_nullable(ty)
            && let Some(m) = matched_value_type
            && self.v.ctx.is_dart_core_null(m)
        {
            self.report_node(diag::cast_from_null_always_fails(), node);
        }
        ast.visit_children(node, self);
    }

    fn visit_catch_clause(&mut self, ast: &Ast, node: Id<CatchClause>) {
        ast.visit_children(node, self);
        self.check_for_nullable_type_in_catch_clause(node);
    }

    fn visit_class_declaration(&mut self, ast: &Ast, node: Id<ClassDeclaration>) {
        let element = self.declared(node);
        self.enclosing_class = element;
        let was_in_do_not_store_member = self.in_do_not_store_member;
        deprecated_functionality::class_declaration(self.v, node);
        self.push_element(element);
        if element.is_some_and(|e| self.has(e, flags::DO_NOT_STORE)) {
            self.in_do_not_store_member = true;
        }
        let name_token = class_name_token(ast, ast[node].name_part);
        immutable_verifier::check_declaration(self.v, node.raw(), name_token);
        self.check_for_invalid_sealed_superclass(node.raw());
        ast.visit_children(node, self);
        self.enclosing_class = None;
        self.pop_element();
        self.in_do_not_store_member = was_in_do_not_store_member;
    }

    fn visit_class_type_alias(&mut self, ast: &Ast, node: Id<ClassTypeAlias>) {
        immutable_verifier::check_declaration(self.v, node.raw(), ast[node].name);
        self.check_for_invalid_sealed_superclass(node.raw());
        deprecated_functionality::class_type_alias(self.v, node);
        let element = self.declared(node);
        self.push_element(element);
        ast.visit_children(node, self);
        self.pop_element();
    }

    // Dart `visitComment`: `_docCommentVerifier.docImport` and
    // `.docDirective` (DocCommentVerifier, ported by another group).

    fn visit_comment_reference(&mut self, ast: &Ast, node: Id<CommentReference>) {
        if let Some(new_keyword) = ast[node].new_keyword
            && self.is_enabled(ExperimentalFlag::ConstructorTearoffs)
        {
            self.report_token(diag::deprecated_new_in_comment_reference(), new_keyword);
        }
        ast.visit_children(node, self);
    }

    fn visit_constant_pattern(&mut self, ast: &Ast, node: Id<ConstantPattern>) {
        if is_double_nan(ast, ast[node].expression.raw()) {
            self.report_node(diag::unnecessary_nan_comparison_false(), node);
        }
        ast.visit_children(node, self);
    }

    fn visit_constructor_declaration(&mut self, ast: &Ast, node: Id<ConstructorDeclaration>) {
        let initializers: Vec<NodeId> = ast
            .list(ast[node].initializers)
            .iter()
            .map(|i| i.raw())
            .collect();
        self.check_strict_inference_in_parameters(
            Some(ast[node].parameters),
            Some(&initializers),
            Some(ast[node].body.raw()),
        );
        let element = self.declared(node);
        self.push_element(element);
        self.element_usage_frontier_detector
            .constructor_declaration(self.v, node);
        deprecated_functionality::constructor_declaration(self.v, node);
        ast.visit_children(node, self);
        self.pop_element();
    }

    fn visit_constructor_name(&mut self, ast: &Ast, node: Id<ConstructorName>) {
        self.element_usage_frontier_detector
            .constructor_name(self.v, node);
        deprecated_functionality::constructor_name(self.v, node);
        ast.visit_children(node, self);
    }

    fn visit_dot_shorthand_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<DotShorthandConstructorInvocation>,
    ) {
        deprecated_functionality::dot_shorthand_constructor_invocation(self.v, node);
        self.element_usage_frontier_detector
            .dot_shorthand_constructor_invocation(self.v, node);
        self.check_for_literal_constructor_use_in_dot_shorthand(node);
        ast.visit_children(node, self);
    }

    fn visit_dot_shorthand_invocation(&mut self, ast: &Ast, node: Id<DotShorthandInvocation>) {
        deprecated_functionality::dot_shorthand_invocation(self.v, node);
        self.element_usage_frontier_detector
            .dot_shorthand_invocation(self.v, node);
        ast.visit_children(node, self);
    }

    fn visit_dot_shorthand_property_access(
        &mut self,
        ast: &Ast,
        node: Id<DotShorthandPropertyAccess>,
    ) {
        self.element_usage_frontier_detector
            .dot_shorthand_property_access(self.v, node);
        ast.visit_children(node, self);
    }

    fn visit_enum_declaration(&mut self, ast: &Ast, node: Id<EnumDeclaration>) {
        let element = self.declared(node);
        self.push_element(element);
        deprecated_functionality::enum_declaration(self.v, node);
        ast.visit_children(node, self);
        self.pop_element();
    }

    fn visit_export_directive(&mut self, ast: &Ast, node: Id<ExportDirective>) {
        self.element_usage_frontier_detector
            .export_directive(self.v, node);
        self.check_for_internal_export(node);
        ast.visit_children(node, self);
    }

    fn visit_expression_function_body(&mut self, ast: &Ast, node: Id<ExpressionFunctionBody>) {
        if !self.in_test_directory {
            self.check_for_return_of_do_not_store(Some(ast[node].expression.raw()));
        }
        ast.visit_children(node, self);
    }

    fn visit_extension_declaration(&mut self, ast: &Ast, node: Id<ExtensionDeclaration>) {
        let element = self.declared(node);
        self.push_element(element);
        ast.visit_children(node, self);
        self.pop_element();
    }

    fn visit_extension_override(&mut self, ast: &Ast, node: Id<ExtensionOverride>) {
        self.element_usage_frontier_detector
            .extension_override(self.v, node);
        ast.visit_children(node, self);
    }

    fn visit_extension_type_declaration(&mut self, ast: &Ast, node: Id<ExtensionTypeDeclaration>) {
        let element = self.declared(node);
        self.push_element(element);
        ast.visit_children(node, self);
        self.pop_element();
    }

    fn visit_field_declaration(&mut self, ast: &Ast, node: Id<FieldDeclaration>) {
        let first = self.first_variable_element(ast[node].fields);
        self.push_element(first);
        ast.visit_children(node, self);
        let variables: Vec<_> = ast.list(ast[ast[node].fields].variables).to_vec();
        for field in variables {
            if !self.in_test_directory {
                self.check_for_assignment_of_do_not_store(ast[field].initializer.map(|i| i.raw()));
            }
            let Some(element) = self.declared(field) else {
                continue;
            };
            let ctx = self.v.ctx;
            let Some(enclosing) =
                enclosing_of(&ctx, element).and_then(|e| e.cast::<InterfaceElement>())
            else {
                continue;
            };
            let Some(name) = ctx.element_name(element) else {
                continue;
            };
            let name_obj = Name::for_library(&ctx, Some(self.v.library), name);
            let im = InheritanceManager3::new(ctx);
            let map = im.get_inherited_concrete_map(enclosing);
            let overridden = map
                .get(&name_obj)
                .or_else(|| map.get(&name_obj.for_setter(&ctx)))
                .map(|&e| member::base_element(&ctx, e));
            if let Some(overridden) = overridden
                && self.has_non_virtual_annotation(overridden)
            {
                let defining_class = enclosing_of(&ctx, overridden)
                    .map(|e| display_name(&ctx, e))
                    .unwrap_or_default();
                let field_name = ast.tokens.lexeme(ast[field].name);
                self.report_token(
                    diag::invalid_override_of_non_virtual_member(field_name, &defining_class),
                    ast[field].name,
                );
            }
        }
        self.pop_element();
    }

    fn visit_field_formal_parameter(&mut self, ast: &Ast, node: Id<FieldFormalParameter>) {
        self.check_for_colon_default_value(node.raw());
        let element = self.declared(node);
        self.push_element(element);
        self.element_usage_frontier_detector
            .formal_parameter(self.v, node.raw());
        self.check_final_parameter(node.raw());
        ast.visit_children(node, self);
        self.pop_element();
    }

    fn visit_function_declaration(&mut self, ast: &Ast, node: Id<FunctionDeclaration>) {
        let was_in_do_not_store_member = self.in_do_not_store_member;
        let element = self.declared(node);
        self.push_element(element);
        if element.is_some_and(|e| self.has(e, flags::DO_NOT_STORE)) {
            self.in_do_not_store_member = true;
        }
        // Return types are inferred only on non-recursive local functions.
        let is_setter = ast[node]
            .property_keyword
            .is_some_and(|k| ast.tokens.lexeme(k) == "set");
        if ast
            .parent(node)
            .is_some_and(|p| ast.is::<CompilationUnit>(p))
            && !is_setter
        {
            let name = ast.tokens.lexeme(ast[node].name).to_string();
            self.check_strict_inference_return_type(
                ast[node].return_type.map(|t| t.raw()),
                node.raw(),
                &name,
            );
        }
        let function = ast[node].function_expression;
        self.check_strict_inference_in_parameters(
            ast[function].parameters,
            None,
            Some(ast[function].body.raw()),
        );
        ast.visit_children(node, self);
        self.pop_element();
        self.in_do_not_store_member = was_in_do_not_store_member;
    }

    fn visit_function_declaration_statement(
        &mut self,
        ast: &Ast,
        node: Id<FunctionDeclarationStatement>,
    ) {
        // TODO(srawlins): Check strict-inference return type on recursive
        // local functions.
        ast.visit_children(node, self);
    }

    fn visit_function_expression(&mut self, ast: &Ast, node: Id<FunctionExpression>) {
        let body = ast[node].body;
        let was_function_type_supplied = self
            .v
            .tables
            .flags
            .get(node.raw())
            .is_some_and(|f| f.contains(NodeFlags::FUNCTION_TYPE_SUPPLIED));
        if !was_function_type_supplied {
            self.check_strict_inference_in_parameters(ast[node].parameters, None, Some(body.raw()));
        }
        self.check_for_unnecessary_set_literal(body.raw(), node);
        ast.visit_children(node, self);
    }

    fn visit_function_expression_invocation(
        &mut self,
        ast: &Ast,
        node: Id<FunctionExpressionInvocation>,
    ) {
        self.element_usage_frontier_detector
            .function_expression_invocation(self.v, node);
        ast.visit_children(node, self);
    }

    fn visit_function_type_alias(&mut self, ast: &Ast, node: Id<FunctionTypeAlias>) {
        let name = ast.tokens.lexeme(ast[node].name).to_string();
        self.check_strict_inference_return_type(
            ast[node].return_type.map(|t| t.raw()),
            node.raw(),
            &name,
        );
        self.check_strict_inference_in_parameters(Some(ast[node].parameters), None, None);
        let element = self.declared(node);
        self.push_element(element);
        ast.visit_children(node, self);
        self.pop_element();
    }

    fn visit_generic_function_type(&mut self, ast: &Ast, node: Id<GenericFunctionType>) {
        // GenericTypeAlias is handled in [visitGenericTypeAlias], where a
        // proper name can be reported in any message.
        if !ast
            .parent(node)
            .is_some_and(|p| ast.is::<GenericTypeAlias>(p))
        {
            let name = dartr_ast::to_source::to_source(ast, node.raw());
            self.check_strict_inference_return_type(
                ast[node].return_type.map(|t| t.raw()),
                node.raw(),
                &name,
            );
        }
        ast.visit_children(node, self);
    }

    fn visit_generic_type_alias(&mut self, ast: &Ast, node: Id<GenericTypeAlias>) {
        if let Some(function_type) = ast.cast::<GenericFunctionType>(ast[node].type_) {
            let name = ast.tokens.lexeme(ast[node].name).to_string();
            self.check_strict_inference_return_type(
                ast[function_type].return_type.map(|t| t.raw()),
                node.raw(),
                &name,
            );
        }
        let element = self.declared(node);
        self.push_element(element);
        ast.visit_children(node, self);
        self.pop_element();
    }

    fn visit_import_directive(&mut self, ast: &Ast, node: Id<ImportDirective>) {
        self.element_usage_frontier_detector
            .import_directive(self.v, node);
        if let Some(import) = library_import(self.v, node) {
            let is_deferred = import
                .prefix
                .is_some_and(|p| self.v.ctx.fragment(p).is_deferred);
            if is_deferred {
                self.check_for_load_library_function(node);
            }
        }
        self.verify_import(node);
        ast.visit_children(node, self);
    }

    fn visit_index_expression(&mut self, ast: &Ast, node: Id<IndexExpression>) {
        self.element_usage_frontier_detector
            .index_expression(self.v, node);
        ast.visit_children(node, self);
    }

    fn visit_instance_creation_expression(
        &mut self,
        ast: &Ast,
        node: Id<InstanceCreationExpression>,
    ) {
        self.element_usage_frontier_detector
            .instance_creation_expression(self.v, node);
        deprecated_functionality::instance_creation_expression(self.v, node);
        null_safe_api_verifier::instance_creation(self.v, node);
        self.check_for_literal_constructor_use(node);
        ast.visit_children(node, self);
    }

    fn visit_is_expression(&mut self, ast: &Ast, node: Id<IsExpression>) {
        self.check_all_type_checks(node);
        ast.visit_children(node, self);
    }

    fn visit_method_declaration(&mut self, ast: &Ast, node: Id<MethodDeclaration>) {
        let was_in_do_not_store_member = self.in_do_not_store_member;
        let element = self.declared(node);
        self.push_element(element);
        if element.is_some_and(|e| self.has(e, flags::DO_NOT_STORE)) {
            self.in_do_not_store_member = true;
        }
        self.visit_method_declaration_body(ast, node, element);
        self.pop_element();
        self.in_do_not_store_member = was_in_do_not_store_member;
    }

    fn visit_method_invocation(&mut self, ast: &Ast, node: Id<MethodInvocation>) {
        self.element_usage_frontier_detector
            .method_invocation(self.v, node);
        deprecated_functionality::method_invocation(self.v, node);
        // Dart `_errorHandlerVerifier.verifyMethodInvocation(node)`
        // (ErrorHandlerVerifier, ported by another group).
        null_safe_api_verifier::method_invocation(self.v, node);
        ast.visit_children(node, self);
    }

    fn visit_mixin_declaration(&mut self, ast: &Ast, node: Id<MixinDeclaration>) {
        let element = self.declared(node);
        self.enclosing_class = element;
        deprecated_functionality::mixin_declaration(self.v, node);
        self.push_element(element);
        immutable_verifier::check_declaration(self.v, node.raw(), ast[node].name);
        self.check_for_invalid_sealed_superclass(node.raw());
        ast.visit_children(node, self);
        self.enclosing_class = None;
        self.pop_element();
    }

    fn visit_named_argument(&mut self, ast: &Ast, node: Id<NamedArgument>) {
        self.verify_named_argument(node);
        ast.visit_children(node, self);
    }

    fn visit_named_type(&mut self, ast: &Ast, node: Id<NamedType>) {
        self.element_usage_frontier_detector
            .named_type(self.v, node);
        self.verify_named_type(node);
        if let Some(question) = ast[node].question
            && let Some(ty) = self.annotation_type(node)
        {
            let ctx = self.v.ctx;
            // Only report non-aliased, non-user-defined `Null?` and
            // `dynamic?`. Do not report synthetic `dynamic` in place of an
            // unresolved type.
            let report = match *ctx.ty(ty) {
                TypeKind::Interface { element, alias, .. } => {
                    alias.is_none() && ctx.is_dart_core_null_element(element)
                }
                TypeKind::Dynamic => ast.tokens.lexeme(ast[node].name) == "dynamic",
                _ => false,
            };
            if report {
                let name = ast.qualified_name(node);
                self.report_token(diag::unnecessary_question_mark(&name), question);
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_pattern_field(&mut self, ast: &Ast, node: Id<PatternField>) {
        self.element_usage_frontier_detector
            .pattern_field(self.v, node);
        self.verify_pattern_field(node);
        ast.visit_children(node, self);
    }

    fn visit_postfix_expression(&mut self, ast: &Ast, node: Id<PostfixExpression>) {
        self.element_usage_frontier_detector
            .postfix_expression(self.v, node);
        if ast.tokens.ty(ast[node].operator) == TokenType::BANG
            && self
                .static_type(ast[node].operand)
                .is_some_and(|t| self.v.ctx.is_dart_core_null(t))
        {
            self.report_node(diag::null_check_always_fails(), node);
        }
        ast.visit_children(node, self);
    }

    fn visit_prefix_expression(&mut self, ast: &Ast, node: Id<PrefixExpression>) {
        self.element_usage_frontier_detector
            .prefix_expression(self.v, node);
        ast.visit_children(node, self);
    }

    fn visit_primary_constructor_body(&mut self, ast: &Ast, node: Id<PrimaryConstructorBody>) {
        let element = super::element_usage_detector::primary_constructor_declaration(ast, node)
            .and_then(|d| self.declared(d));
        self.push_element(element);
        // TODO(srawlins): Account for @doNotStore, as in
        // `visitFunctionDeclaration`.
        ast.visit_children(node, self);
        self.pop_element();
    }

    fn visit_primary_constructor_declaration(
        &mut self,
        ast: &Ast,
        node: Id<PrimaryConstructorDeclaration>,
    ) {
        self.in_primary_constructor_declaration = true;
        self.check_strict_inference_in_parameters(Some(ast[node].formal_parameters), None, None);
        deprecated_functionality::primary_constructor_declaration(self.v, node);
        ast.visit_children(node, self);
        self.in_primary_constructor_declaration = false;
    }

    fn visit_redirecting_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<RedirectingConstructorInvocation>,
    ) {
        self.element_usage_frontier_detector
            .redirecting_constructor_invocation(self.v, node);
        ast.visit_children(node, self);
    }

    fn visit_regular_formal_parameter(&mut self, ast: &Ast, node: Id<RegularFormalParameter>) {
        self.check_for_colon_default_value(node.raw());
        if let Some(suffix) = ast[node].function_typed_suffix {
            let name = ast[node]
                .name
                .map(|n| ast.tokens.lexeme(n).to_string())
                .unwrap_or_default();
            self.check_strict_inference_return_type(
                ast[node].type_.map(|t| t.raw()),
                node.raw(),
                &name,
            );
            self.check_strict_inference_in_parameters(
                Some(ast[suffix].formal_parameters),
                None,
                None,
            );
        }
        let element = self.declared(node);
        self.push_element(element);
        self.element_usage_frontier_detector
            .formal_parameter(self.v, node.raw());
        ast.visit_children(node, self);
        self.pop_element();
    }

    fn visit_return_statement(&mut self, ast: &Ast, node: Id<ReturnStatement>) {
        if !self.in_test_directory {
            self.check_for_return_of_do_not_store(ast[node].expression.map(|e| e.raw()));
        }
        ast.visit_children(node, self);
    }

    fn visit_set_or_map_literal(&mut self, ast: &Ast, node: Id<SetOrMapLiteral>) {
        self.check_for_duplications(node);
        ast.visit_children(node, self);
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        self.element_usage_frontier_detector
            .simple_identifier(self.v, node);
        self.verify_identifier(node);
        ast.visit_children(node, self);
    }

    fn visit_super_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<SuperConstructorInvocation>,
    ) {
        self.element_usage_frontier_detector
            .super_constructor_invocation(self.v, node);
        self.verify_super_constructor_invocation(node);
        ast.visit_children(node, self);
    }

    fn visit_super_formal_parameter(&mut self, ast: &Ast, node: Id<SuperFormalParameter>) {
        self.check_for_colon_default_value(node.raw());
        let element = self.declared(node);
        self.push_element(element);
        self.element_usage_frontier_detector
            .super_formal_parameter(self.v, node);
        self.check_final_parameter(node.raw());
        ast.visit_children(node, self);
        self.pop_element();
    }

    fn visit_top_level_variable_declaration(
        &mut self,
        ast: &Ast,
        node: Id<TopLevelVariableDeclaration>,
    ) {
        let first = self.first_variable_element(ast[node].variables);
        self.push_element(first);
        if !self.in_test_directory {
            let variables: Vec<_> = ast.list(ast[ast[node].variables].variables).to_vec();
            for declaration in variables {
                self.check_for_assignment_of_do_not_store(
                    ast[declaration].initializer.map(|i| i.raw()),
                );
            }
        }
        ast.visit_children(node, self);
        self.pop_element();
    }
}

impl BestPracticesVerifier<'_, '_> {
    /// The body of Dart `visitMethodDeclaration` inside its `try`.
    fn visit_method_declaration_body(
        &mut self,
        ast: &Ast,
        node: Id<MethodDeclaration>,
        element: Option<ElementId>,
    ) {
        let name_token = ast[node].name;
        must_call_super_verifier::check_method_declaration(self.v, node);
        self.check_for_unnecessary_no_such_method(node);
        self.check_for_nullable_equals_parameter_type(node);

        let ctx = self.v.ctx;
        let Some(element) = element else {
            return;
        };
        let Some(name) = Name::for_element(&ctx, element.into()) else {
            return;
        };
        let enclosing = enclosing_of(&ctx, element).and_then(|e| e.cast::<InterfaceElement>());
        let im = InheritanceManager3::new(ctx);

        let mut element_is_override = false;
        if let Some(enclosing) = enclosing
            && matches!(element.tag(), Tag::Method | Tag::Getter | Tag::Setter)
        {
            element_is_override = im.get_overridden(enclosing, name).is_some();
        }

        let is_setter = ast[node]
            .property_keyword
            .is_some_and(|k| ast.tokens.lexeme(k) == "set");
        if !is_setter && !element_is_override {
            let name_text = ast.tokens.lexeme(name_token).to_string();
            self.check_strict_inference_return_type(
                ast[node].return_type.map(|t| t.raw()),
                node.raw(),
                &name_text,
            );
        }
        if !element_is_override {
            self.check_strict_inference_in_parameters(
                ast[node].parameters,
                None,
                Some(ast[node].body.raw()),
            );
        }

        let overridden = enclosing.and_then(|enclosing| {
            im.get_inherited_concrete_map(enclosing)
                .get(&name)
                .map(|&e| member::base_element(&ctx, e))
        });
        if let Some(overridden) = overridden
            && self.has_non_virtual_annotation(overridden)
        {
            let defining_class = enclosing_of(&ctx, overridden)
                .map(|e| display_name(&ctx, e))
                .unwrap_or_default();
            let member_name = ast.tokens.lexeme(name_token);
            self.report_token(
                diag::invalid_override_of_non_virtual_member(member_name, &defining_class),
                name_token,
            );
        }

        if !self.is_enabled(ExperimentalFlag::PrimaryConstructors)
            && self.is_ambiguous_factory_method(node)
        {
            self.report_token(diag::deprecated_factory_method(), name_token);
        }

        ast.visit_children(node, self);
    }

    /// Dart `isAmbiguousFactoryMethod()` of `visitMethodDeclaration`.
    fn is_ambiguous_factory_method(&self, node: Id<MethodDeclaration>) -> bool {
        let ast = self.v.ast;
        let name_token = ast[node].name;
        if ast.tokens.lexeme(name_token) != "factory" {
            return false;
        }
        let first_token = ast[node].first_token_after_comment_and_metadata(ast);
        if first_token == name_token {
            return true;
        }
        let second_token = ast.tokens.next(first_token);
        if ast.tokens.lexeme(first_token) == "external" {
            return second_token == name_token;
        }
        if self.is_enabled(ExperimentalFlag::Augmentations)
            && ast.tokens.lexeme(first_token) == "augment"
        {
            return second_token == name_token
                || (ast.tokens.lexeme(second_token) == "external"
                    && ast.tokens.next(second_token) == name_token);
        }
        false
    }

    /// Dart `_checkAllTypeChecks(node)`.
    fn check_all_type_checks(&mut self, node: Id<IsExpression>) -> bool {
        let ast = self.v.ast;
        let ctx = self.v.ctx;
        let left_node = ast[node].expression;
        let Some(left_type) = self.static_type(left_node) else {
            return false;
        };
        let Some(right_type) = self.annotation_type(ast[node].type_) else {
            return false;
        };
        let true_false = || {
            if ast[node].not_operator.is_none() {
                diag::unnecessary_type_check_true()
            } else {
                diag::unnecessary_type_check_false()
            }
        };

        // `cannotResolve is X` or `cannotResolve is! X`
        if matches!(ctx.ty(left_type), TypeKind::Invalid) {
            return false;
        }
        // `is dynamic` or `is! dynamic`
        if matches!(ctx.ty(right_type), TypeKind::Dynamic) {
            self.report_node(true_false(), node);
            return true;
        }
        // `is CannotResolveType` or `is! CannotResolveType`
        if matches!(ctx.ty(right_type), TypeKind::Invalid) {
            return false;
        }
        // `is Null` or `is! Null`
        if ctx.is_dart_core_null(right_type) {
            if ast.is::<NullLiteral>(left_node) {
                self.report_node(true_false(), node);
            } else {
                let d = if ast[node].not_operator.is_none() {
                    diag::type_check_is_null()
                } else {
                    diag::type_check_is_not_null()
                };
                self.report_node(d, node);
            }
            return true;
        }
        if self.v.type_system.is_subtype_of(left_type, right_type) {
            self.report_node(true_false(), node);
            return true;
        }
        false
    }

    /// Dart `_checkFinalParameter(node)`.
    fn check_final_parameter(&mut self, node: NodeId) {
        let ast = self.v.ast;
        let parts = formal_parameter_parts(ast, node);
        if let Some(keyword) = parts.const_final_or_var_keyword
            && ast.tokens.lexeme(keyword) == "final"
            && !self.in_primary_constructor_declaration
        {
            // If we have the erroneous case of `class C(final this.x);` we
            // model the as an initializing formal instead of a declaring
            // parameter, but don't want to report the warning here.
            self.report_token(diag::unnecessary_final(), keyword);
        }
    }

    /// Dart `_checkForAssignmentOfDoNotStore(expression)`.
    fn check_for_assignment_of_do_not_store(&mut self, expression: Option<NodeId>) {
        let expressions = self.get_sub_expressions_marked_do_not_store(expression);
        for (node, element) in expressions {
            let name = self.v.ctx.element_name(element).unwrap_or("").to_string();
            self.report_node(diag::assignment_of_do_not_store(&name), node);
        }
    }

    /// Dart `_checkForColonDefaultValue(node)`.
    fn check_for_colon_default_value(&mut self, node: NodeId) {
        let ast = self.v.ast;
        let parts = formal_parameter_parts(ast, node);
        let Some(default_clause) = parts.default_clause else {
            return;
        };
        let separator = ast[default_clause].separator;
        let is_named = matches!(
            parts.kind,
            dartr_ast::ParameterKind::Named | dartr_ast::ParameterKind::NamedRequired
        );
        if is_named && ast.tokens.ty(separator) == TokenType::COLON {
            // This is a warning in code whose language version is < 3.0, but
            // an error in code whose language version is >= 3.0.
            let major = self
                .v
                .ctx
                .get(self.v.library)
                .language_version
                .effective()
                .major;
            if major < 3 {
                self.report_token(diag::deprecated_colon_for_default_value(), separator);
            } else {
                self.report_token(diag::obsolete_colon_for_default_value(), separator);
            }
        }
    }

    /// Dart `_checkForDuplications(node)`.
    fn check_for_duplications(&mut self, node: Id<SetOrMapLiteral>) {
        let ast = self.v.ast;
        // This only checks for top-level elements. If, for, and spread
        // elements that contribute duplicate values are not detected.
        if ast[node].const_keyword.is_some() || crate::ast_ext::in_constant_context(ast, node.raw())
        {
            // This case is covered by the DiagnosticVerifier.
            return;
        }
        let is_set = self.is_set_literal(node);
        let expressions: Vec<NodeId> = ast
            .list(ast[node].elements)
            .iter()
            .filter_map(|&e| {
                if is_set {
                    ast.is::<Expression>(e).then_some(e.raw())
                } else {
                    ast.cast::<MapLiteralEntry>(e).map(|m| ast[m].key.raw())
                }
            })
            .collect();
        let mut already_seen: IndexSet<LiteralValue> = IndexSet::new();
        for expression in expressions {
            if let Some(value) = literal_value(ast, expression)
                && !already_seen.insert(value)
            {
                let d = if is_set {
                    diag::equal_elements_in_set()
                } else {
                    diag::equal_keys_in_map()
                };
                self.report_node(d, expression);
            }
        }
    }

    /// Dart `SetOrMapLiteral.isSet` (from the static type).
    fn is_set_literal(&self, node: Id<SetOrMapLiteral>) -> bool {
        self.static_type(node)
            .is_some_and(|t| self.v.ctx.is_dart_core_set(t))
    }

    /// Dart `_checkForInternalExport(node)`.
    fn check_for_internal_export(&mut self, node: Id<ExportDirective>) {
        if !self.in_package_public_api {
            return;
        }
        let Some(export) = library_export(self.v, node) else {
            return;
        };
        let Some(library) = uri_library(&export.directive.uri) else {
            return;
        };
        let ctx = self.v.ctx;
        let unit = self.unit_ast();
        if element_has(&ctx, library.raw(), flags::INTERNAL, unit) {
            let name = display_name(&ctx, library.raw());
            self.report_node(diag::invalid_export_of_internal_element(&name), node);
        }
        let Some(namespace) = ctx.get(library).export_namespace.try_get() else {
            return;
        };
        for (&name, &element) in &namespace.defined_names {
            let name = ctx.name_str(name);
            if !combinators_allow(&ctx, &export.combinators, name) {
                continue;
            }
            if is_internal(&ctx, element, unit) {
                let display = display_name(&ctx, element);
                self.report_node(diag::invalid_export_of_internal_element(&display), node);
                continue;
            }
            if matches!(
                element.tag(),
                Tag::Method | Tag::Getter | Tag::Setter | Tag::TopLevelFunction | Tag::Constructor
            ) {
                let mut signature_types: Vec<TypeId> =
                    member::formal_parameters(&ctx, element.into())
                        .into_iter()
                        .map(|p| member::type_(&ctx, p))
                        .collect();
                signature_types.push(member::return_type(&ctx, element.into()));
                for tp in member::type_parameters(&ctx, element.into()) {
                    if let Some(bound) = ctx.type_parameter_bound(tp) {
                        signature_types.push(bound);
                    }
                }
                for ty in signature_types {
                    let Some(alias) = ctx.type_alias(ty) else {
                        continue;
                    };
                    let alias_element = ctx.alias(alias).element.raw();
                    if element_has(&ctx, alias_element, flags::INTERNAL, unit) {
                        let internal_name =
                            ctx.element_name(alias_element).unwrap_or("").to_string();
                        let exported = display_name(&ctx, element);
                        self.report_node(
                            diag::invalid_export_of_internal_element_indirectly(
                                &internal_name,
                                &exported,
                            ),
                            node,
                        );
                    }
                }
            }
        }
    }

    /// Dart `_checkForInvalidSealedSuperclass(node)`.
    fn check_for_invalid_sealed_superclass(&mut self, node: NodeId) {
        let ctx = self.v.ctx;
        let Some(element) = self
            .declared(node)
            .and_then(|e| e.cast::<InterfaceElement>())
        else {
            return;
        };
        let constraints: Vec<TypeId> = if element.raw().tag() == Tag::Mixin {
            ctx.element_superclass_constraints(element).to_vec()
        } else {
            Vec::new()
        };
        for supertype in ctx.element_all_supertypes(element).iter().copied() {
            let Some(superclass) = ctx.interface_element(supertype) else {
                continue;
            };
            if !self.has(superclass.raw(), flags::SEALED) {
                continue;
            }
            if self.is_library_in_workspace_package(library_of(&ctx, superclass.raw())) {
                continue;
            }
            let name = ctx
                .element_name(superclass.raw())
                .unwrap_or("null")
                .to_string();
            if element.raw().tag() == Tag::Mixin && constraints.contains(&supertype) {
                // This is a special violation of the sealed class contract,
                // requiring specific messaging.
                self.report_node(diag::mixin_on_sealed_class(&name), node);
            } else {
                // This is a regular violation of the sealed class contract.
                self.report_node(diag::subtype_of_sealed_class(&name), node);
            }
        }
    }

    /// Dart `_checkForInvariantNanComparison(node)`.
    fn check_for_invariant_nan_comparison(&mut self, node: Id<BinaryExpression>) {
        let ast = self.v.ast;
        let operator = ast[node].operator;
        let d = match ast.tokens.ty(operator) {
            TokenType::BANG_EQ => diag::unnecessary_nan_comparison_true(),
            TokenType::EQ_EQ => diag::unnecessary_nan_comparison_false(),
            _ => return,
        };
        let left = ast[node].left_operand.raw();
        let right = ast[node].right_operand.raw();
        let operator_end = crate::ast_ext::token_end(ast, operator);
        if is_double_nan(ast, left) {
            let offset = ast.offset(left);
            self.report(d, (offset, operator_end - offset));
        } else if is_double_nan(ast, right) {
            let offset = ast.tokens.offset(operator);
            self.report(d, (offset, ast.end(right) - offset));
        }
    }

    /// Dart `_checkForInvariantNullComparison(node)`.
    fn check_for_invariant_null_comparison(&mut self, node: Id<BinaryExpression>) {
        let ast = self.v.ast;
        let operator = ast[node].operator;
        let make = match ast.tokens.ty(operator) {
            TokenType::BANG_EQ => diag::unnecessary_null_comparison_never_null_true,
            TokenType::EQ_EQ => diag::unnecessary_null_comparison_never_null_false,
            _ => return,
        };
        let left = ast[node].left_operand.raw();
        let right = ast[node].right_operand.raw();
        if ast.is::<NullLiteral>(left)
            && let Some(right_type) = self.static_type(right)
            && self.v.type_system.is_strictly_non_nullable(right_type)
        {
            let offset = ast.offset(left);
            let end = crate::ast_ext::token_end(ast, operator);
            self.report(make(), (offset, end - offset));
        }
        if ast.is::<NullLiteral>(right)
            && let Some(left_type) = self.static_type(left)
            && self.v.type_system.is_strictly_non_nullable(left_type)
        {
            let offset = ast.tokens.offset(operator);
            self.report(make(), (offset, ast.end(right) - offset));
        }
    }

    /// Dart `_checkForLiteralConstructorUse(node)`.
    fn check_for_literal_constructor_use(&mut self, node: Id<InstanceCreationExpression>) {
        let ast = self.v.ast;
        let constructor_name = ast[node].constructor_name;
        let Some(constructor) = element_of(&self.v.ctx, self.v.tables, constructor_name) else {
            return;
        };
        if crate::ast_ext::instance_creation_is_const(ast, node)
            || !self.has(constructor, flags::LITERAL)
            || !can_be_const(self.v, constructor, ast[node].argument_list)
        {
            return;
        }
        let mut full_name = ast.qualified_name(ast[constructor_name].type_);
        if let Some(name) = ast[constructor_name].name {
            full_name = format!("{full_name}.{}", ast.tokens.lexeme(ast[name].token));
        }
        let is_new = ast[node]
            .keyword
            .is_some_and(|k| ast.tokens.lexeme(k) == "new");
        let d = if is_new {
            diag::non_const_call_to_literal_constructor_using_new(&full_name)
        } else {
            diag::non_const_call_to_literal_constructor(&full_name)
        };
        self.report_node(d, node);
    }

    /// Dart `_checkForLiteralConstructorUseInDotShorthand(node)`.
    fn check_for_literal_constructor_use_in_dot_shorthand(
        &mut self,
        node: Id<DotShorthandConstructorInvocation>,
    ) {
        let ast = self.v.ast;
        let Some(constructor) = element_of(&self.v.ctx, self.v.tables, ast[node].constructor_name)
            .or_else(|| element_of(&self.v.ctx, self.v.tables, node))
        else {
            return;
        };
        if constructor.tag() != Tag::Constructor {
            return;
        }
        let is_const = ast[node].const_keyword.is_some()
            || crate::ast_ext::in_constant_context(ast, node.raw());
        if is_const
            || !self.has(constructor, flags::LITERAL)
            || !can_be_const(self.v, constructor, ast[node].argument_list)
        {
            return;
        }
        let name = display_name(&self.v.ctx, constructor);
        self.report_node(diag::non_const_call_to_literal_constructor(&name), node);
    }

    /// Dart `_checkForLoadLibraryFunction(node, importElement)`.
    fn check_for_load_library_function(&mut self, node: Id<ImportDirective>) -> bool {
        let Some(import) = library_import(self.v, node) else {
            return false;
        };
        let Some(imported_library) = uri_library(&import.directive.uri) else {
            return false;
        };
        let ctx = self.v.ctx;
        let has_prefix_name = import
            .prefix
            .and_then(|p| ctx.fragment(p).element.try_get())
            .and_then(|&p| ctx.element_name(p))
            .is_some();
        if !has_prefix_name {
            return false;
        }
        let Some(namespace) = ctx.get(imported_library).export_namespace.try_get() else {
            return false;
        };
        let found = namespace
            .defined_names
            .contains_key(&ctx.name("loadLibrary"))
            && combinators_allow(&ctx, &import.combinators, "loadLibrary");
        if found {
            self.report_node(diag::import_deferred_library_with_load_function(), node);
            return true;
        }
        false
    }

    /// Dart `_checkForNullableEqualsParameterType(node)`.
    fn check_for_nullable_equals_parameter_type(&mut self, node: Id<MethodDeclaration>) {
        let ast = self.v.ast;
        if ast.tokens.ty(ast[node].name) != TokenType::EQ_EQ {
            return;
        }
        let Some(parameters) = ast[node].parameters else {
            return;
        };
        let list = ast.list(ast[parameters].parameters);
        if list.len() != 1 {
            return;
        }
        let Some(parameter) = self.declared(list[0]) else {
            return;
        };
        let ctx = self.v.ctx;
        let ty = member::type_(&ctx, parameter.into());
        if !ctx.is_dart_core_object(ty) && !matches!(ctx.ty(ty), TypeKind::Dynamic) {
            // There is no legal way to define a nullable parameter type,
            // which is not `dynamic` or `Object?`, so avoid double reporting
            // here.
            return;
        }
        if self.v.type_system.is_nullable(ty) {
            self.report_token(diag::non_nullable_equals_parameter(), ast[node].name);
        }
    }

    /// Dart `_checkForNullableTypeInCatchClause(node)`.
    fn check_for_nullable_type_in_catch_clause(&mut self, node: Id<CatchClause>) {
        let ast = self.v.ast;
        let Some(type_node) = ast[node].exception_type else {
            return;
        };
        let Some(ty) = self.annotation_type(type_node) else {
            return;
        };
        if matches!(self.v.ctx.ty(ty), TypeKind::Invalid) {
            return;
        }
        if self.v.type_system.is_potentially_nullable(ty) {
            self.report_node(diag::nullable_type_in_catch_clause(), type_node);
        }
    }

    /// Dart `_checkForReturnOfDoNotStore(expression)`.
    fn check_for_return_of_do_not_store(&mut self, expression: Option<NodeId>) {
        if self.in_do_not_store_member {
            return;
        }
        let expressions = self.get_sub_expressions_marked_do_not_store(expression);
        if expressions.is_empty() {
            return;
        }
        let ast = self.v.ast;
        let Some(expression) = expression else {
            return;
        };
        let mut parent = Some(expression);
        while let Some(p) = parent {
            if ast.is::<FunctionDeclaration>(p) || ast.is::<MethodDeclaration>(p) {
                break;
            }
            parent = ast.parent(p);
        }
        let Some(parent) = parent else {
            return;
        };
        let ctx = self.v.ctx;
        let returning = self
            .declared(parent)
            .map(|e| display_name(&ctx, e))
            .unwrap_or_default();
        for (node, element) in expressions {
            let invoked = ctx.element_name(element).unwrap_or("").to_string();
            self.report_node(diag::return_of_do_not_store(&invoked, &returning), node);
        }
    }

    /// Dart `_checkForUnnecessaryNoSuchMethod(node)`.
    fn check_for_unnecessary_no_such_method(&mut self, node: Id<MethodDeclaration>) -> bool {
        let ast = self.v.ast;
        if ast.tokens.lexeme(ast[node].name) != "noSuchMethod" {
            return false;
        }
        let body = ast[node].body.raw();
        let report = if let Some(b) = ast.cast::<ExpressionFunctionBody>(body) {
            self.is_non_object_no_such_method_invocation(Some(ast[b].expression.raw()))
        } else if let Some(b) = ast.cast::<BlockFunctionBody>(body) {
            let statements = ast.list(ast[ast[b].block].statements);
            statements.len() == 1
                && ast.cast::<ReturnStatement>(statements[0]).is_some_and(|r| {
                    self.is_non_object_no_such_method_invocation(ast[r].expression.map(|e| e.raw()))
                })
        } else {
            false
        };
        if report {
            self.report_token(diag::unnecessary_no_such_method(), ast[node].name);
        }
        report
    }

    /// Dart `isNonObjectNoSuchMethodInvocation(invocation)`.
    fn is_non_object_no_such_method_invocation(&self, invocation: Option<NodeId>) -> bool {
        let ast = self.v.ast;
        let Some(invocation) = invocation.and_then(|i| ast.cast::<MethodInvocation>(i)) else {
            return false;
        };
        let target_is_super = ast[invocation]
            .target
            .is_some_and(|t| ast.is::<SuperExpression>(t));
        if !target_is_super || ast.list(ast[ast[invocation].argument_list].arguments).len() != 1 {
            return false;
        }
        let name = ast[invocation].method_name;
        if ast.tokens.lexeme(ast[name].token) != "noSuchMethod" {
            return false;
        }
        let ctx = self.v.ctx;
        let Some(method) = element_of(&ctx, self.v.tables, name) else {
            return false;
        };
        let class = enclosing_of(&ctx, method);
        method.tag() == Tag::Method
            && class.is_some_and(|c| {
                c.tag() == Tag::Class && !member::is_dart_core_object_element(&ctx, c)
            })
    }

    /// Dart `_checkForUnnecessarySetLiteral(body, node)`.
    fn check_for_unnecessary_set_literal(&mut self, body: NodeId, node: Id<FunctionExpression>) {
        let ast = self.v.ast;
        let Some(body) = ast.cast::<ExpressionFunctionBody>(body) else {
            return;
        };
        let ctx = self.v.ctx;
        let parameter_type = corresponding_parameter(&ctx, ast, self.v.tables, node.raw())
            .map(|p| member::type_(&ctx, p.into()));
        let return_type = match parameter_type.map(|t| ctx.ty(t)) {
            Some(TypeKind::Function(f)) => Some(f.ret),
            _ => {
                let Some(parent) = ast
                    .parent(node)
                    .and_then(|p| ast.cast::<FunctionDeclaration>(p))
                else {
                    return;
                };
                ast[parent]
                    .return_type
                    .and_then(|t| self.annotation_type(t))
            }
        };
        let Some(return_type) = return_type else {
            return;
        };
        let is_return_void = if matches!(ctx.ty(return_type), TypeKind::Void) {
            true
        } else if ctx.is_dart_async_future(return_type) || ctx.is_dart_async_future_or(return_type)
        {
            let args = ctx.type_arguments(return_type);
            args.len() == 1 && matches!(ctx.ty(args[0]), TypeKind::Void)
        } else {
            false
        };
        if is_return_void {
            let expression = ast[body].expression;
            if let Some(literal) = ast.cast::<SetOrMapLiteral>(expression)
                && self.is_set_literal(literal)
            {
                self.report_node(diag::unnecessary_set_literal(), literal);
            }
        }
    }

    /// Dart `_checkStrictInferenceInParameters(parameterList, initializers:,
    /// body:)`.
    fn check_strict_inference_in_parameters(
        &mut self,
        parameter_list: Option<Id<FormalParameterList>>,
        initializers: Option<&[NodeId]>,
        body: Option<NodeId>,
    ) {
        if !self.strict_inference {
            return;
        }
        let Some(parameter_list) = parameter_list else {
            return;
        };
        let ast = self.v.ast;
        let mut implicitly_typed: Vec<Id<RegularFormalParameter>> = ast
            .list(ast[parameter_list].parameters)
            .iter()
            .filter_map(|&p| ast.cast::<RegularFormalParameter>(p))
            .filter(|&p| ast[p].function_typed_suffix.is_none() && ast[p].type_.is_none())
            .collect();
        if implicitly_typed.is_empty() {
            return;
        }
        // Whether the parameters are in a typedef, or function that is
        // abstract, external, etc.
        let parameter_reference_is_unknown =
            body.is_none_or(|b| ast.is::<EmptyFunctionBody>(b)) && initializers.is_none();
        if !parameter_reference_is_unknown {
            let parameters: Vec<ElementId> = implicitly_typed
                .iter()
                .filter_map(|&p| self.declared(p))
                .collect();
            let mut used = UsedParameterVisitor {
                ctx: self.v.ctx,
                tables: self.v.tables,
                parameters,
                used: Vec::new(),
            };
            if let Some(body) = body {
                ast.accept(body, &mut used);
            }
            for &initializer in initializers.unwrap_or_default() {
                ast.accept(initializer, &mut used);
            }
            implicitly_typed.retain(|&p| self.declared(p).is_some_and(|e| used.used.contains(&e)));
        }
        for parameter in implicitly_typed {
            let Some(element) = self.declared(parameter) else {
                continue;
            };
            let name = display_name(&self.v.ctx, element);
            let d = diag::inference_failure_on_untyped_parameter(&name);
            match ast[parameter].name {
                Some(n) => self.report_token(d, n),
                None => self.report_node(d, parameter),
            }
        }
    }

    /// Dart `_checkStrictInferenceReturnType(returnType, reportNode,
    /// displayName)`.
    fn check_strict_inference_return_type(
        &mut self,
        return_type: Option<NodeId>,
        report_node: NodeId,
        display_name: &str,
    ) {
        if !self.strict_inference || return_type.is_some() {
            return;
        }
        let ast = self.v.ast;
        let d = diag::inference_failure_on_function_return_type(display_name);
        if let Some(m) = ast.cast::<MethodDeclaration>(report_node) {
            self.report_token(d, ast[m].name);
        } else if let Some(f) = ast.cast::<FunctionDeclaration>(report_node) {
            self.report_token(d, ast[f].name);
        } else {
            self.report_node(d, report_node);
        }
    }

    /// Dart `_getSubExpressionsMarkedDoNotStore(expression)`: the
    /// subexpressions whose element is marked `@doNotStore`, in the order
    /// of the Dart map.
    fn get_sub_expressions_marked_do_not_store(
        &self,
        expression: Option<NodeId>,
    ) -> Vec<(NodeId, ElementId)> {
        let mut expressions = Vec::new();
        self.collect_do_not_store(expression, &mut expressions);
        expressions
    }

    fn collect_do_not_store(
        &self,
        expression: Option<NodeId>,
        expressions: &mut Vec<(NodeId, ElementId)>,
    ) {
        let Some(expression) = expression else {
            return;
        };
        let ast = self.v.ast;
        let ctx = self.v.ctx;
        let tables = self.v.tables;
        let is_tear_off = |e: ElementId| {
            matches!(
                e.tag(),
                Tag::LocalFunction | Tag::TopLevelFunction | Tag::Method
            )
        };
        let mut element: Option<ElementId> = None;
        if let Some(p) = ast.cast::<PropertyAccess>(expression) {
            element = element_of(&ctx, tables, ast[p].property_name).filter(|&e| !is_tear_off(e));
        } else if let Some(m) = ast.cast::<MethodInvocation>(expression) {
            element = element_of(&ctx, tables, ast[m].method_name);
        } else if ast.is::<Identifier>(expression) {
            element = element_of(&ctx, tables, expression)
                .or_else(|| {
                    let p = ast.cast::<PrefixedIdentifier>(expression)?;
                    element_of(&ctx, tables, ast[p].identifier)
                })
                .filter(|&e| !is_tear_off(e));
        } else if let Some(c) = ast.cast::<dartr_ast::ConditionalExpression>(expression) {
            self.collect_do_not_store(Some(ast[c].else_expression.raw()), expressions);
            self.collect_do_not_store(Some(ast[c].then_expression.raw()), expressions);
        } else if let Some(b) = ast.cast::<BinaryExpression>(expression) {
            self.collect_do_not_store(Some(ast[b].left_operand.raw()), expressions);
            self.collect_do_not_store(Some(ast[b].right_operand.raw()), expressions);
        } else if let Some(f) = ast.cast::<FunctionExpression>(expression)
            && let Some(body) = ast.cast::<ExpressionFunctionBody>(ast[f].body)
        {
            self.collect_do_not_store(Some(ast[body].expression.raw()), expressions);
        }
        if let Some(e) = element
            && let Some(variable) = accessor_variable(&ctx, e)
        {
            element = Some(variable);
        }
        if let Some(e) = element
            && has_or_inherits_do_not_store(&ctx, e, self.unit_ast())
        {
            match expressions.iter_mut().find(|(n, _)| *n == expression) {
                Some(entry) => entry.1 = e,
                None => expressions.push((expression, e)),
            }
        }
    }

    /// Dart `_isLibraryInWorkspacePackage(library)`.
    fn is_library_in_workspace_package(
        &self,
        library: Option<dartr_element::EId<LibraryElement>>,
    ) -> bool {
        match (&self.workspace_package, library) {
            (Some(p), Some(library)) => p.contains_library(&self.v.ctx, library),
            // Better to not make a big claim that they _are_ in the same
            // package, if we were unable to determine what package
            // [_currentLibrary] is in.
            _ => false,
        }
    }

    /// Dart `_hasNonVirtualAnnotation(element)`.
    fn has_non_virtual_annotation(&self, element: ElementId) -> bool {
        if let Some(variable) = accessor_variable(&self.v.ctx, element)
            && self.has(variable, flags::NON_VIRTUAL)
        {
            return true;
        }
        self.has(element, flags::NON_VIRTUAL)
    }

    /// Dart `_isUnnecessaryCast(node, typeSystem)`.
    fn is_unnecessary_cast(&self, node: Id<AsExpression>) -> bool {
        let ast = self.v.ast;
        let ctx = self.v.ctx;
        let (Some(left_type), Some(right_type)) = (
            self.static_type(ast[node].expression),
            self.annotation_type(ast[node].type_),
        ) else {
            return false;
        };
        // `cannotResolve is SomeType` is already reported.
        if matches!(ctx.ty(left_type), TypeKind::Invalid) {
            return false;
        }
        // `x as Unresolved` is already reported as an error.
        if matches!(ctx.ty(right_type), TypeKind::Invalid) {
            return false;
        }
        // The cast is necessary.
        if !ctx.dart_eq(left_type, right_type) {
            return false;
        }
        // `x as dynamic` is a valid use case.
        if matches!(ctx.ty(right_type), TypeKind::Dynamic) {
            return false;
        }
        // `x as Function` is a valid use case.
        if ctx.is_dart_core_function(right_type) {
            return false;
        }
        true
    }
}

// ------------------------------------------------------------------ _InvalidAccessVerifier

impl BestPracticesVerifier<'_, '_> {
    /// Dart `_InvalidAccessVerifier.verify(identifier)`.
    fn verify_identifier(&mut self, identifier: Id<SimpleIdentifier>) {
        let ast = self.v.ast;
        let ctx = self.v.ctx;
        if in_declaration_context(ast, identifier) || in_comment_reference(ast, identifier.raw()) {
            return;
        }
        // This is the same logic used in
        // [checkForDeprecatedMemberUseAtIdentifier] to avoid reporting an
        // error twice for named constructors.
        let parent = ast.parent(identifier);
        if let Some(c) = parent.and_then(|p| ast.cast::<ConstructorName>(p))
            && ast[c].name == Some(identifier)
        {
            return;
        }
        let grandparent = parent.and_then(|p| ast.parent(p));
        let element = match grandparent.and_then(|g| ast.cast::<ConstructorName>(g)) {
            Some(g) => element_of(&ctx, self.v.tables, g),
            None => write_or_read_element(&ctx, ast, self.v.tables, identifier),
        };
        let Some(element) = element else {
            return;
        };
        if self.in_current_library(element) {
            return;
        }
        if parent.is_some_and(|p| ast.is::<dartr_ast::HideCombinator>(p)) {
            return;
        }
        let name_token = ast[identifier].token;
        self.check_for_invalid_internal_access(parent, name_token, element);
        self.check_for_other_invalid_access(identifier.raw(), element);
    }

    /// Dart `_InvalidAccessVerifier.verifyBinary(node)`.
    fn verify_binary(&mut self, node: Id<BinaryExpression>) {
        let ast = self.v.ast;
        let Some(element) = element_of(&self.v.ctx, self.v.tables, node) else {
            return;
        };
        if !self.has_visible_for_overriding(element) {
            return;
        }
        let operator = ast[node].operator;
        if ast.is::<SuperExpression>(ast[node].left_operand) {
            let mut current = ast.parent(node);
            while let Some(c) = current {
                if let Some(m) = ast.cast::<MethodDeclaration>(c) {
                    if ast.tokens.lexeme(ast[m].name) == ast.tokens.lexeme(operator) {
                        return;
                    }
                    break;
                }
                current = ast.parent(c);
            }
        }
        let name = ast.tokens.lexeme(operator).to_string();
        self.report_token(
            diag::invalid_use_of_visible_for_overriding_member(&name),
            operator,
        );
    }

    /// Dart `_InvalidAccessVerifier.verifyImport(node)`.
    fn verify_import(&mut self, node: Id<ImportDirective>) {
        let Some(import) = library_import(self.v, node) else {
            return;
        };
        let Some(imported_library) = uri_library(&import.directive.uri) else {
            return;
        };
        if is_internal(&self.v.ctx, imported_library.raw(), self.unit_ast())
            && !self.is_library_in_workspace_package(Some(imported_library))
        {
            let ast = self.v.ast;
            let uri =
                crate::element_metadata::string_value(ast, ast[node].uri.raw()).unwrap_or_default();
            self.report_node(diag::invalid_use_of_internal_member(&uri), node);
        }
    }

    /// Dart `_InvalidAccessVerifier.verifyNamedArgument(node)`.
    fn verify_named_argument(&mut self, node: Id<NamedArgument>) {
        let ast = self.v.ast;
        let Some(element) = corresponding_parameter(&self.v.ctx, ast, self.v.tables, node.raw())
        else {
            return;
        };
        if self.in_current_library(element) {
            return;
        }
        self.check_for_invalid_internal_access(Some(node.raw()), ast[node].name, element);
        self.check_for_other_invalid_access(node.raw(), element);
    }

    /// Dart `_InvalidAccessVerifier.verifyNamedType(node)`.
    fn verify_named_type(&mut self, node: Id<NamedType>) {
        let ast = self.v.ast;
        let ctx = self.v.ctx;
        let mut element = element_of(&ctx, self.v.tables, node);
        if let Some(parent) = ast
            .parent(node)
            .and_then(|p| ast.cast::<ConstructorName>(p))
        {
            element = element_of(&ctx, self.v.tables, parent);
        }
        let Some(element) = element else {
            return;
        };
        if self.in_current_library(element) {
            return;
        }
        self.check_for_invalid_internal_access(Some(node.raw()), ast[node].name, element);
        self.check_for_other_invalid_access(node.raw(), element);
    }

    /// Dart `_InvalidAccessVerifier.verifyPatternField(node)`.
    fn verify_pattern_field(&mut self, node: Id<PatternField>) {
        let ast = self.v.ast;
        let ctx = self.v.ctx;
        let Some(element) = element_of(&ctx, self.v.tables, node) else {
            return;
        };
        if self.in_current_library(element) {
            return;
        }
        if is_internal(&ctx, element, self.unit_ast())
            && !self.is_library_in_workspace_package(library_of(&ctx, element))
        {
            if ast[node].name.is_none() {
                return;
            }
            let name = display_name(&ctx, element);
            let range = pattern_field_error_entity(ast, node);
            self.report(diag::invalid_use_of_internal_member(&name), range);
        }
        self.check_for_other_invalid_access(node.raw(), element);
    }

    /// Dart `_InvalidAccessVerifier.verifySuperConstructorInvocation(node)`.
    fn verify_super_constructor_invocation(&mut self, node: Id<SuperConstructorInvocation>) {
        let ast = self.v.ast;
        if ast[node].constructor_name.is_some() {
            // Named constructor calls are handled by [verify].
            return;
        }
        let ctx = self.v.ctx;
        let Some(element) = element_of(&ctx, self.v.tables, node) else {
            return;
        };
        if is_internal(&ctx, element, self.unit_ast())
            && !self.is_library_in_workspace_package(library_of(&ctx, element))
        {
            let name = ctx.element_name(element).unwrap_or("").to_string();
            self.report_node(diag::invalid_use_of_internal_member(&name), node);
        }
    }

    /// Dart `_checkForInvalidInternalAccess(parent:, nameToken:, element:)`.
    fn check_for_invalid_internal_access(
        &mut self,
        parent: Option<NodeId>,
        name_token: TokenId,
        element: ElementId,
    ) {
        let ctx = self.v.ctx;
        if !is_internal(&ctx, element, self.unit_ast())
            || self.is_library_in_workspace_package(library_of(&ctx, element))
        {
            return;
        }
        let ast = self.v.ast;
        let grandparent = parent.and_then(|p| ast.parent(p));
        let (name, range) = match grandparent.and_then(|g| ast.cast::<ConstructorName>(g)) {
            Some(g) => (
                dartr_ast::to_source::to_source(ast, g.raw()),
                node_range(ast, g),
            ),
            None => (
                ast.tokens.lexeme(name_token).to_string(),
                token_range(ast, name_token),
            ),
        };
        self.report(diag::invalid_use_of_internal_member(&name), range);
    }

    /// Dart `_checkForOtherInvalidAccess(node, element)`.
    fn check_for_other_invalid_access(&mut self, node: NodeId, element: ElementId) {
        let ctx = self.v.ctx;
        let ast = self.v.ast;
        let unit = self.unit_ast();
        let has_protected = is_protected(&ctx, element, unit);
        if has_protected
            && let Some(defining_class) =
                enclosing_of(&ctx, element).and_then(|e| e.cast::<InterfaceElement>())
            && self.has_type_or_super_type(self.enclosing_class, defining_class)
        {
            return;
        }

        let is_visible_for_template_applied = self.is_visible_for_template_applied(element);
        if is_visible_for_template_applied
            && (self.in_template_source || in_export_directive(ast, node))
        {
            return;
        }

        let has_visible_for_testing = is_visible_for_testing(&ctx, element, unit);
        if has_visible_for_testing && (self.in_test_directory || in_export_directive(ast, node)) {
            return;
        }

        let (name, error_entity) = identifier_name_and_error_entity(&ctx, ast, node, element);

        let has_visible_for_overriding = self.has_visible_for_overriding(element);
        if has_visible_for_overriding && let Some(parent) = ast.parent(node) {
            let super_target = if let Some(m) = ast.cast::<MethodInvocation>(parent) {
                ast[m].target.is_some_and(|t| ast.is::<SuperExpression>(t))
            } else if let Some(p) = ast.cast::<PropertyAccess>(parent) {
                ast[p].target.is_some_and(|t| ast.is::<SuperExpression>(t))
            } else {
                false
            };
            if super_target {
                let mut current = ast.parent(parent);
                while let Some(c) = current {
                    if let Some(m) = ast.cast::<MethodDeclaration>(c) {
                        if ast.tokens.lexeme(ast[m].name) == name {
                            return;
                        }
                        break;
                    }
                    current = ast.parent(c);
                }
            }
        }

        // At this point, [identifier] was not cleared as protected access,
        // nor cleared as access for templates or testing. Report a violation
        // for each annotation present.
        let Some(defining_class) = enclosing_of(&ctx, element) else {
            return;
        };
        let defining_uri = library_of(&ctx, defining_class)
            .map(|l| ctx.library_uri(l).to_string())
            .unwrap_or_default();

        if has_protected {
            let class_name = display_name(&ctx, defining_class);
            self.report(
                diag::invalid_use_of_protected_member(&name, &class_name),
                error_entity,
            );
        }
        if is_visible_for_template_applied {
            self.report(
                diag::invalid_use_of_visible_for_template_member(&name, &defining_uri),
                error_entity,
            );
        }
        if has_visible_for_testing {
            self.report(
                diag::invalid_use_of_visible_for_testing_member(&name, &defining_uri),
                error_entity,
            );
        }
        if has_visible_for_overriding {
            self.report(
                diag::invalid_use_of_visible_for_overriding_member(&name),
                error_entity,
            );
        }
    }

    /// Dart `_hasTypeOrSuperType(element, superElement)`.
    fn has_type_or_super_type(
        &self,
        element: Option<ElementId>,
        super_element: dartr_element::EId<InterfaceElement>,
    ) -> bool {
        let Some(element) = element.and_then(|e| e.cast::<InterfaceElement>()) else {
            return false;
        };
        let ctx = self.v.ctx;
        let this_type = ctx.interface_this_type(element);
        ctx.as_instance_of(this_type, super_element).is_some()
    }

    /// Dart `_hasVisibleForOverriding(element)`.
    fn has_visible_for_overriding(&self, element: ElementId) -> bool {
        if self.has(element, flags::VISIBLE_FOR_OVERRIDING) {
            return true;
        }
        if let Some(variable) = crate::element_metadata::accessor_variable_any(&self.v.ctx, element)
        {
            return self.has(variable, flags::VISIBLE_FOR_OVERRIDING);
        }
        false
    }

    /// Dart `_hasVisibleForTemplate(element)`.
    fn has_visible_for_template(&self, element: Option<ElementId>) -> bool {
        let Some(element) = element else {
            return false;
        };
        if self.has(element, flags::VISIBLE_FOR_TEMPLATE) {
            return true;
        }
        if let Some(variable) = crate::element_metadata::accessor_variable_any(&self.v.ctx, element)
            && self.has(variable, flags::VISIBLE_FOR_TEMPLATE)
        {
            return true;
        }
        self.has_visible_for_template(enclosing_of(&self.v.ctx, element))
    }

    /// Dart `_hasVisibleOutsideTemplate(element)`.
    fn has_visible_outside_template(&self, element: ElementId) -> bool {
        if self.has(element, flags::VISIBLE_OUTSIDE_TEMPLATE) {
            return true;
        }
        if let Some(variable) = crate::element_metadata::accessor_variable_any(&self.v.ctx, element)
            && self.has(variable, flags::VISIBLE_OUTSIDE_TEMPLATE)
        {
            return true;
        }
        enclosing_of(&self.v.ctx, element).is_some_and(|e| self.has_visible_outside_template(e))
    }

    /// Dart `_inCurrentLibrary(element)`.
    fn in_current_library(&self, element: ElementId) -> bool {
        library_of(&self.v.ctx, element) == Some(self.v.library)
    }

    /// Dart `_isVisibleForTemplateApplied(element)`.
    fn is_visible_for_template_applied(&self, element: ElementId) -> bool {
        if matches!(element.tag(), Tag::Class | Tag::Enum | Tag::Mixin) {
            false
        } else {
            self.has_visible_for_template(Some(element))
                && !self.has_visible_outside_template(element)
        }
    }
}

/// Dart `_inExportDirective(node)`.
fn in_export_directive(ast: &Ast, node: NodeId) -> bool {
    let Some(parent) = ast.parent(node) else {
        return false;
    };
    ast.is::<dartr_ast::Combinator>(parent)
        && ast
            .parent(parent)
            .is_some_and(|g| ast.is::<ExportDirective>(g))
}

/// Dart `PatternFieldImpl.errorEntity`: the name of the field, else the
/// variable pattern, else the field.
fn pattern_field_error_entity(ast: &Ast, node: Id<PatternField>) -> Range {
    if let Some(field_name) = ast[node].name {
        if let Some(name) = ast[field_name].name {
            return token_range(ast, name);
        }
        if let Some(name) = super::support::variable_pattern_name(ast, ast[node].pattern.raw()) {
            return token_range(ast, name);
        }
    }
    node_range(ast, node)
}

/// Dart `_getIdentifierNameAndErrorEntity(node, element)`.
fn identifier_name_and_error_entity(
    ctx: &dartr_element::Ctx<'_>,
    ast: &Ast,
    node: NodeId,
    element: ElementId,
) -> (String, Range) {
    let parent = ast.parent(node);
    let grandparent = parent.and_then(|p| ast.parent(p));
    if ast.is::<Identifier>(node) {
        if let Some(g) = grandparent.and_then(|g| ast.cast::<ConstructorName>(g)) {
            return (
                dartr_ast::to_source::to_source(ast, g.raw()),
                node_range(ast, g),
            );
        }
        let name = if let Some(s) = ast.cast::<SimpleIdentifier>(node) {
            ast.tokens.lexeme(ast[s].token).to_string()
        } else {
            dartr_ast::to_source::to_source(ast, node)
        };
        return (name, node_range(ast, node));
    }
    if let Some(n) = ast.cast::<NamedType>(node) {
        if let Some(p) = parent.and_then(|p| ast.cast::<ConstructorName>(p)) {
            return (
                dartr_ast::to_source::to_source(ast, p.raw()),
                node_range(ast, p),
            );
        }
        return (
            ast.tokens.lexeme(ast[n].name).to_string(),
            node_range(ast, node),
        );
    }
    if let Some(n) = ast.cast::<NamedArgument>(node) {
        return (
            ast.tokens.lexeme(ast[n].name).to_string(),
            token_range(ast, ast[n].name),
        );
    }
    if let Some(n) = ast.cast::<PatternField>(node) {
        return (
            display_name(ctx, element),
            pattern_field_error_entity(ast, n),
        );
    }
    (String::new(), node_range(ast, node))
}

fn combinators_allow(
    ctx: &dartr_element::Ctx<'_>,
    combinators: &[dartr_element::NamespaceCombinator],
    name: &str,
) -> bool {
    let name = name.strip_suffix('=').unwrap_or(name);
    let matches = |names: &[dartr_element::Name]| names.iter().any(|&n| ctx.name_str(n) == name);
    for c in combinators {
        match c {
            dartr_element::NamespaceCombinator::Show { shown_names, .. } => {
                if !matches(shown_names) {
                    return false;
                }
            }
            dartr_element::NamespaceCombinator::Hide { hidden_names, .. } => {
                if matches(hidden_names) {
                    return false;
                }
            }
        }
    }
    true
}

/// Dart `Expression.isDoubleNan`: the prefixed identifier `double.nan`.
fn is_double_nan(ast: &Ast, node: NodeId) -> bool {
    let Some(p) = ast.cast::<PrefixedIdentifier>(node) else {
        return false;
    };
    ast.tokens.lexeme(ast[ast[p].prefix].token) == "double"
        && ast.tokens.lexeme(ast[ast[p].identifier].token) == "nan"
}

/// The constant value of a literal (see the module documentation).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum LiteralValue {
    Null,
    Bool(bool),
    Int(i64),
    Double(u64),
    String(String),
}

fn literal_value(ast: &Ast, node: NodeId) -> Option<LiteralValue> {
    if ast.is::<NullLiteral>(node) {
        return Some(LiteralValue::Null);
    }
    if let Some(b) = ast.cast::<BooleanLiteral>(node) {
        return Some(LiteralValue::Bool(ast[b].value));
    }
    if let Some(i) = ast.cast::<IntegerLiteral>(node) {
        return ast[i].value.map(LiteralValue::Int);
    }
    if let Some(d) = ast.cast::<DoubleLiteral>(node) {
        let value = ast[d].value;
        // Dart `==` of doubles: `NaN != NaN`, `0.0 == -0.0`.
        if value.is_nan() {
            return None;
        }
        let value = if value == 0.0 { 0.0 } else { value };
        return Some(LiteralValue::Double(value.to_bits()));
    }
    if ast.is::<SimpleStringLiteral>(node) || ast.is::<dartr_ast::AdjacentStrings>(node) {
        return crate::element_metadata::string_value(ast, node).map(LiteralValue::String);
    }
    None
}

/// Dart `InstanceCreationExpression.canBeConst` (and of a dot shorthand
/// constructor invocation): the constructor is const and the invocation
/// with `const` would not produce a constant verifier error. Dart runs the
/// constant verifier; until the constant evaluation (D1–D2) is available
/// here, the arguments must be constant syntactically: literals, `const`
/// and const-able creations, `const` collection literals, type literals,
/// and references to constant variables, types and static functions.
fn can_be_const(
    v: &UnitVerifier<'_>,
    constructor: ElementId,
    arguments: Id<dartr_ast::ArgumentList>,
) -> bool {
    let Some(c) = constructor.cast::<dartr_element::ConstructorElement>() else {
        return false;
    };
    let ctx = v.ctx;
    let is_const = ctx
        .fragment_data(ctx.get(c).first_fragment().raw())
        .is_some_and(|f| {
            f.flags
                .has(dartr_element::FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST)
        });
    if !is_const {
        return false;
    }
    let ast = v.ast;
    ast.list(ast[arguments].arguments).iter().all(|&a| {
        let e = match ast.cast::<NamedArgument>(a) {
            Some(n) => ast[n].argument_expression.raw(),
            None => a.raw(),
        };
        is_constant_expression(v, e)
    })
}

fn is_constant_expression(v: &UnitVerifier<'_>, node: NodeId) -> bool {
    let ast = v.ast;
    let ctx = v.ctx;
    if literal_value(ast, node).is_some()
        || ast.is::<DoubleLiteral>(node)
        || ast.is::<dartr_ast::SymbolLiteral>(node)
        || ast.is::<dartr_ast::TypeLiteral>(node)
    {
        return true;
    }
    if let Some(p) = ast.cast::<dartr_ast::ParenthesizedExpression>(node) {
        return is_constant_expression(v, ast[p].expression.raw());
    }
    if let Some(i) = ast.cast::<InstanceCreationExpression>(node) {
        if crate::ast_ext::instance_creation_is_const(ast, i) {
            return true;
        }
        return element_of(&ctx, v.tables, ast[i].constructor_name)
            .is_some_and(|c| can_be_const(v, c, ast[i].argument_list));
    }
    if let Some(l) = ast.cast::<dartr_ast::ListLiteral>(node) {
        return ast[l].const_keyword.is_some();
    }
    if let Some(l) = ast.cast::<SetOrMapLiteral>(node) {
        return ast[l].const_keyword.is_some();
    }
    if let Some(p) = ast.cast::<PrefixExpression>(node) {
        return is_constant_expression(v, ast[p].operand.raw());
    }
    if ast.is::<Identifier>(node) || ast.is::<PropertyAccess>(node) {
        let element = if let Some(p) = ast.cast::<PropertyAccess>(node) {
            element_of(&ctx, v.tables, ast[p].property_name)
        } else {
            element_of(&ctx, v.tables, node)
        };
        let Some(element) = element else {
            return false;
        };
        let variable = accessor_variable(&ctx, element).unwrap_or(element);
        return match variable.tag() {
            Tag::TopLevelVariable | Tag::Field | Tag::LocalVariable => {
                crate::element_ext::is_const(&ctx, variable)
            }
            Tag::Class
            | Tag::Enum
            | Tag::Mixin
            | Tag::TypeAlias
            | Tag::ExtensionType
            | Tag::TopLevelFunction => true,
            Tag::Method => member::is_static(&ctx, variable.into()),
            _ => false,
        };
    }
    false
}

/// Dart `_UsedParameterVisitor`.
struct UsedParameterVisitor<'c, 't> {
    ctx: dartr_element::Ctx<'c>,
    tables: &'t dartr_element::ResolutionTables,
    parameters: Vec<ElementId>,
    used: Vec<ElementId>,
}

impl AstVisitor for UsedParameterVisitor<'_, '_> {
    fn visit_simple_identifier(&mut self, _ast: &Ast, node: Id<SimpleIdentifier>) {
        if let Some(element) = element_of(&self.ctx, self.tables, node)
            && self.parameters.contains(&element)
            && !self.used.contains(&element)
        {
            self.used.push(element);
        }
    }
}
