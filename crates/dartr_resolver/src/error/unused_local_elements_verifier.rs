// Dart source: pkg/analyzer/lib/src/error/unused_local_elements_verifier.dart

//! Unused private and local elements: [`gather_used_local_elements`] (Dart
//! `GatherUsedLocalElementsVisitor`) collects the used elements of each
//! unit, [`UsedLocalElements::merge`] joins the units of a library, and
//! [`verify`] (Dart `UnusedLocalElementsVerifier`) reports the unused
//! declarations of a unit.
//!
//! Element sets are keyed by [`ElementId`]: local elements are in the local
//! arena of their unit, and the ids of different arenas differ (the store
//! is part of the id), so the sets of the units of a library can be merged.
//! Members (Dart `SubstitutedExecutableElementImpl`) are stored as their
//! base elements.
//!
//! # Not ported exactly
//!
//! - `metadata.hasJS` / `hasPragmaVmEntryPoint`: the annotations are not
//!   resolved and not evaluated yet, so they are recognized from the syntax
//!   of the declaration's annotations (`@JS(...)`, `@pragma('vm:entry-point')`
//!   with a string literal), see [`AnnotationFacts`].

use dartr_ast::{
    Annotation, ArgumentList, AssignmentExpression, Ast, AstVisitor, BinaryExpression, CatchClause,
    CatchClauseParameter, ClassDeclaration, CommentReference, ConstructorDeclaration,
    ConstructorName, DeclaredIdentifier, DeclaredVariablePattern,
    DotShorthandConstructorInvocation, DotShorthandInvocation, DotShorthandPropertyAccess,
    EnumConstantDeclaration, EnumDeclaration, ExpressionStatement, ExtensionTypeDeclaration,
    FieldDeclaration, ForPartsWithDeclarations, FormalParameterList, FunctionDeclaration,
    FunctionExpression, FunctionExpressionInvocation, FunctionTypeAlias, GenericTypeAlias, Id,
    Identifier, IndexExpression, InstanceCreationExpression, IsExpression, MethodDeclaration,
    MethodInvocation, MixinDeclaration, NamedArgument, NamedType, NodeId, PatternField,
    PatternVariableDeclaration, PostfixExpression, PrefixExpression, PrefixedIdentifier,
    PrimaryConstructorDeclaration, PropertyAccess, RelationalPattern, SimpleIdentifier,
    StringLiteral, SuperConstructorInvocation, SuperFormalParameter, TopLevelVariableDeclaration,
    VariableDeclarationList, VariableDeclarationStatement,
};
use dartr_diagnostics::{LocatableDiagnostic, diag};
use dartr_element::{
    ConstructorElement, Ctx, EId, ElemRef, ElementId, ExecutableElement, FieldElement,
    FormalParameterElement, FragmentFlags, InstanceElement, InterfaceElement, LibraryElement,
    ParameterKind, Tag, TypeKind,
};
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_syntax::TokenType;
use dartr_typesystem::TypeExt;
use dartr_typesystem::inheritance_manager3::{InheritanceManager3, Name};
use dartr_typesystem::member;
use indexmap::IndexSet;

use super::{UnitVerifier, VerifierHost};
use crate::{ast_ext, element_ext};

// ============================================================ UsedLocalElements

/// Dart `UsedLocalElements`: the sets of used elements of a unit or a
/// library.
#[derive(Default)]
pub struct UsedLocalElements {
    /// Resolved, locally defined elements that are used or potentially can
    /// be used.
    pub elements: IndexSet<ElementId>,
    /// The local variables that are the exceptions of catch clauses.
    pub catch_exception_elements: IndexSet<ElementId>,
    /// The local variables that are the stack traces of catch clauses.
    pub catch_stack_trace_elements: IndexSet<ElementId>,
    /// Resolved class members that are referenced in the library.
    pub members: IndexSet<ElementId>,
    /// Resolved class members that are read in the library.
    pub read_members: IndexSet<ElementId>,
    /// Unresolved class members that are read in the library.
    pub unresolved_read_members: IndexSet<String>,
}

impl UsedLocalElements {
    /// Dart `UsedLocalElements.merge(parts)`.
    pub fn merge(parts: Vec<UsedLocalElements>) -> UsedLocalElements {
        let mut result = UsedLocalElements::default();
        for part in parts {
            result.elements.extend(part.elements);
            result
                .catch_exception_elements
                .extend(part.catch_exception_elements);
            result
                .catch_stack_trace_elements
                .extend(part.catch_stack_trace_elements);
            result.members.extend(part.members);
            result.read_members.extend(part.read_members);
            result
                .unresolved_read_members
                .extend(part.unresolved_read_members);
        }
        result
    }

    /// Dart `addCatchException(element)`.
    fn add_catch_exception(&mut self, element: Option<ElementId>) {
        if let Some(e) = element.filter(|&e| element_ext::is_local_variable(e)) {
            self.catch_exception_elements.insert(e);
        }
    }

    /// Dart `addCatchStackTrace(element)`.
    fn add_catch_stack_trace(&mut self, element: Option<ElementId>) {
        if let Some(e) = element.filter(|&e| element_ext::is_local_variable(e)) {
            self.catch_stack_trace_elements.insert(e);
        }
    }

    /// Dart `addElement(element)`.
    fn add_element(&mut self, ctx: &Ctx<'_>, element: Option<ElemRef>) {
        let Some(element) = element else {
            return;
        };
        let element = member::base_element(ctx, element);
        if element.tag() == Tag::JoinPatternVariable {
            // Dart `transitiveVariables`.
            let mut stack = vec![element];
            while let Some(e) = stack.pop() {
                if e.tag() == Tag::JoinPatternVariable {
                    let mut components = element_ext::join_pattern_variable_components(ctx, e);
                    components.reverse();
                    stack.extend(components);
                } else {
                    self.elements.insert(e);
                }
            }
            return;
        }
        self.elements.insert(element);
        if let Some(constructor) = element.cast::<ConstructorElement>()
            && let Some(enclosing) = ctx.element_data(element).and_then(|d| d.enclosing)
            && enclosing.tag() == Tag::Class
            && first_fragment_flags(ctx, enclosing)
                .contains(FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_APPLICATION)
        {
            let super_constructor = ctx.get(constructor).super_constructor.get();
            self.add_element(ctx, super_constructor);
        }
    }

    /// Dart `addMember(element)`.
    fn add_member(&mut self, ctx: &Ctx<'_>, element: Option<ElemRef>) {
        if let Some(e) = element {
            self.members.insert(member::base_element(ctx, e));
        }
    }

    /// Dart `addReadMember(element)`.
    fn add_read_member(&mut self, ctx: &Ctx<'_>, element: Option<ElemRef>) {
        if let Some(e) = element {
            self.read_members.insert(member::base_element(ctx, e));
        }
    }

    /// Dart `isCatchException(element)`.
    fn is_catch_exception(&self, element: ElementId) -> bool {
        self.catch_exception_elements.contains(&element)
    }

    /// Dart `isCatchStackTrace(element)`.
    fn is_catch_stack_trace(&self, element: ElementId) -> bool {
        self.catch_stack_trace_elements.contains(&element)
    }
}

// ============================================================ GatherUsedLocalElementsVisitor

/// Dart `unit.accept(GatherUsedLocalElementsVisitor(library))`.
pub fn gather_used_local_elements(v: &UnitVerifier<'_>) -> UsedLocalElements {
    let facts = AnnotationFacts::compute(v);
    let mut visitor = GatherUsedLocalElementsVisitor {
        v,
        facts: &facts,
        used_elements: UsedLocalElements::default(),
        enclosing_class: None,
        enclosing_exec: None,
        inside_is_expression: false,
    };
    v.ast.accept(v.unit, &mut visitor);
    visitor.used_elements
}

/// Dart `GatherUsedLocalElementsVisitor`: fills [`UsedLocalElements`].
struct GatherUsedLocalElementsVisitor<'v, 'a> {
    v: &'v UnitVerifier<'a>,
    facts: &'v AnnotationFacts,
    used_elements: UsedLocalElements,
    /// Dart `_enclosingClass`.
    enclosing_class: Option<ElementId>,
    /// Dart `_enclosingExec`.
    enclosing_exec: Option<ElementId>,
    /// Whether the visitor is inside the type of an `is` expression.
    inside_is_expression: bool,
}

impl GatherUsedLocalElementsVisitor<'_, '_> {
    /// `tables.element` of [node].
    fn element(&self, node: impl Into<NodeId>) -> Option<ElemRef> {
        self.v.tables.element.get(node.into()).copied()
    }

    /// The element of the declared fragment of [node].
    fn declared_element(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        declared_element(self.v, node.into())
    }

    fn add_element(&mut self, element: Option<ElemRef>) {
        let ctx = self.v.ctx;
        self.used_elements.add_element(&ctx, element);
    }

    fn add_member(&mut self, element: Option<ElemRef>) {
        let ctx = self.v.ctx;
        self.used_elements.add_member(&ctx, element);
    }

    fn add_read_member(&mut self, element: Option<ElemRef>) {
        let ctx = self.v.ctx;
        self.used_elements.add_read_member(&ctx, element);
    }

    /// Dart `_addMemberAndCorrespondingGetter(element)`.
    fn add_member_and_corresponding_getter(&mut self, element: ElementId) {
        let ctx = self.v.ctx;
        if element.tag() == Tag::Setter {
            let getter = member::corresponding_getter(&ctx, ElemRef::Base(element));
            self.add_member(getter);
            self.add_read_member(getter);
        } else {
            self.add_read_member(Some(ElemRef::Base(element)));
        }
    }

    /// Dart `_addParametersForArguments(argumentList)`.
    fn add_parameters_for_arguments(&mut self, argument_list: Id<ArgumentList>) {
        let ast = self.v.ast;
        for &argument in ast.list(ast[argument_list].arguments) {
            // Dart `argument.correspondingParameter`; the table is keyed by
            // the expression of a named argument.
            let expression: NodeId = match ast.cast::<NamedArgument>(argument) {
                Some(n) => ast[n].argument_expression.raw(),
                None => argument.raw(),
            };
            let parameter = self.v.tables.param_element.get(expression).copied();
            self.add_element(parameter);
        }
    }

    /// Dart `_useIdentifierElement(element)`: marks [element] as used in the
    /// library.
    fn use_identifier_element(&mut self, element: Option<ElemRef>) {
        let Some(element) = element else {
            return;
        };
        let ctx = self.v.ctx;
        let base = member::base_element(&ctx, element);
        // Check if [element] is a local element.
        if ctx.element_data(base).and_then(|d| d.library) != Some(self.v.library) {
            return;
        }
        // Ignore references to an element from itself.
        if Some(base) == self.enclosing_class || Some(base) == self.enclosing_exec {
            return;
        }
        // Ignore places where the element is not actually used.
        if base.tag() != Tag::TypeAlias && self.inside_is_expression {
            // An interface type found in an `is` expression is not used.
            return;
        }
        self.add_element(Some(element));
    }
}

/// Dart `_readElement(node)` / `_writeElement(node)` (`ast/extensions.dart`):
/// the read or write element of the compound assignment, prefix or
/// postfix expression whose operand is [node].
fn compound_element(v: &UnitVerifier<'_>, node: NodeId, read: bool) -> Option<ElemRef> {
    let ast = v.ast;
    let parent = ast.parent(node)?;
    let table = if read {
        &v.tables.read_element
    } else {
        &v.tables.write_element
    };
    if let Some(p) = ast.cast::<AssignmentExpression>(parent) {
        return (ast[p].left_hand_side.raw() == node)
            .then(|| table.get(p).copied())
            .flatten();
    }
    if let Some(p) = ast.cast::<PostfixExpression>(parent) {
        return (ast[p].operand.raw() == node)
            .then(|| table.get(p).copied())
            .flatten();
    }
    if let Some(p) = ast.cast::<PrefixExpression>(parent) {
        return (ast[p].operand.raw() == node)
            .then(|| table.get(p).copied())
            .flatten();
    }
    if let Some(p) = ast.cast::<PrefixedIdentifier>(parent) {
        if ast[p].identifier.raw() == node {
            return compound_element(v, parent, read);
        }
        return None;
    }
    if let Some(p) = ast.cast::<PropertyAccess>(parent) {
        if ast[p].property_name.raw() == node {
            return compound_element(v, parent, read);
        }
        return None;
    }
    None
}

/// Dart `node.writeOrReadElement` (`_writeElement(node) ?? node.element`).
fn write_or_read_element(v: &UnitVerifier<'_>, node: NodeId) -> Option<ElemRef> {
    compound_element(v, node, false).or_else(|| v.tables.element.get(node).copied())
}

/// Dart `GatherUsedLocalElementsVisitor._isReadIdentifier(node)`: whether
/// the value of [node] is _only_ being read at this position.
fn is_read_identifier(ast: &Ast, node: Id<SimpleIdentifier>) -> bool {
    // Not reading at all.
    if !ast_ext::simple_identifier_in_getter_context(ast, node) {
        return false;
    }
    // Check if useless reading.
    let Some(parent) = ast.parent(node) else {
        return true;
    };
    if ast
        .parent(parent)
        .is_some_and(|p| ast.is::<ExpressionStatement>(p))
    {
        if ast.is::<PrefixExpression>(parent) || ast.is::<PostfixExpression>(parent) {
            // v++;
            // ++v;
            return false;
        }
        if let Some(p) = ast.cast::<AssignmentExpression>(parent)
            && ast[p].left_hand_side.raw() == node.raw()
        {
            // v ??= doSomething();
            //   vs.
            // v += 2;
            return ast.tokens.get(ast[p].operator).ty == TokenType::QUESTION_QUESTION_EQ;
        }
    }
    // OK
    true
}

/// Dart `SimpleIdentifier.inCommentReference`.
fn in_comment_reference(ast: &Ast, node: NodeId) -> bool {
    let mut current = ast.parent(node);
    for _ in 0..3 {
        let Some(p) = current else {
            return false;
        };
        if ast.is::<CommentReference>(p) {
            return true;
        }
        current = ast.parent(p);
    }
    false
}

/// Dart `SimpleIdentifier.inDeclarationContext()`.
fn in_declaration_context(ast: &Ast, node: Id<SimpleIdentifier>) -> bool {
    let Some(parent) = ast.parent(node) else {
        return false;
    };
    if let Some(import) = ast.cast::<dartr_ast::ImportDirective>(parent) {
        return ast[import].prefix == Some(node);
    }
    if ast.is::<dartr_ast::Label>(parent) {
        let Some(parent2) = ast.parent(parent) else {
            return false;
        };
        return ast.is::<dartr_ast::Statement>(parent2)
            || ast.is::<dartr_ast::SwitchMember>(parent2);
    }
    false
}

impl AstVisitor for GatherUsedLocalElementsVisitor<'_, '_> {
    fn visit_annotation(&mut self, ast: &Ast, node: Id<Annotation>) {
        if let Some(arguments) = ast[node].arguments {
            self.add_parameters_for_arguments(arguments);
        }
        ast.visit_children(node, self);
    }

    fn visit_assignment_expression(&mut self, ast: &Ast, node: Id<AssignmentExpression>) {
        let element = self.element(node);
        self.add_member(element);
        ast.visit_children(node, self);
    }

    fn visit_binary_expression(&mut self, ast: &Ast, node: Id<BinaryExpression>) {
        let element = self.element(node);
        self.add_member(element);
        ast.visit_children(node, self);
    }

    fn visit_catch_clause(&mut self, ast: &Ast, node: Id<CatchClause>) {
        let exception_parameter = ast[node].exception_parameter;
        let stack_trace_parameter = ast[node].stack_trace_parameter;
        if let Some(p) = exception_parameter {
            let element = self.declared_element(p);
            self.used_elements.add_catch_exception(element);
            if stack_trace_parameter.is_some() || ast[node].on_keyword.is_none() {
                self.add_element(element.map(ElemRef::Base));
            }
        }
        if let Some(p) = stack_trace_parameter {
            let element = self.declared_element(p);
            self.used_elements.add_catch_stack_trace(element);
        }
        ast.visit_children(node, self);
    }

    fn visit_class_declaration(&mut self, ast: &Ast, node: Id<ClassDeclaration>) {
        let element = self.declared_element(node);
        if let Some(element) = element
            && self.facts.has_js.contains(&element)
        {
            self.add_element(Some(ElemRef::Base(element)));
        }
        let enclosing_class_old = self.enclosing_class;
        self.enclosing_class = element;
        ast.visit_children(node, self);
        self.enclosing_class = enclosing_class_old;
    }

    fn visit_constructor_declaration(&mut self, ast: &Ast, node: Id<ConstructorDeclaration>) {
        let ctx = self.v.ctx;
        if let Some(element) = self.declared_element(node)
            && let Some(redirected) = ast[node].redirected_constructor
            && let Some(redirected_element) = self.element(redirected)
        {
            let first = formal_parameters(&ctx, element);
            let second: Vec<ElementId> = member::formal_parameters(&ctx, redirected_element)
                .into_iter()
                .map(|p| member::base_element(&ctx, p))
                .collect();
            let mut matched = Vec::new();
            match_parameters(&ctx, &first, &second, |_, second| matched.push(second));
            for second in matched {
                self.add_element(Some(ElemRef::Base(second)));
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_dot_shorthand_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<DotShorthandConstructorInvocation>,
    ) {
        let ctx = self.v.ctx;
        let element = self
            .element(node)
            .or_else(|| self.element(ast[node].constructor_name));
        let enclosing = element.and_then(|e| member::enclosing_element(&ctx, e));
        self.add_element(enclosing.map(ElemRef::Base));
        self.add_parameters_for_arguments(ast[node].argument_list);
        ast.visit_children(node, self);
    }

    fn visit_dot_shorthand_invocation(&mut self, ast: &Ast, node: Id<DotShorthandInvocation>) {
        let ctx = self.v.ctx;
        let element = self.element(ast[node].member_name);
        let enclosing = element.and_then(|e| member::enclosing_element(&ctx, e));
        self.add_element(enclosing.map(ElemRef::Base));
        self.add_parameters_for_arguments(ast[node].argument_list);
        ast.visit_children(node, self);
    }

    fn visit_dot_shorthand_property_access(
        &mut self,
        ast: &Ast,
        node: Id<DotShorthandPropertyAccess>,
    ) {
        let ctx = self.v.ctx;
        let element = self.element(ast[node].property_name);
        let enclosing = element.and_then(|e| member::enclosing_element(&ctx, e));
        self.add_element(enclosing.map(ElemRef::Base));
        ast.visit_children(node, self);
    }

    fn visit_enum_constant_declaration(&mut self, ast: &Ast, node: Id<EnumConstantDeclaration>) {
        let ctx = self.v.ctx;
        // Dart `node.constructorElement?.baseElement`.
        let constructor = self
            .element(node)
            .map(|e| ElemRef::Base(member::base_element(&ctx, e)));
        self.add_element(constructor);
        if let Some(arguments) = ast[node].arguments {
            self.add_parameters_for_arguments(ast[arguments].argument_list);
        }
        ast.visit_children(node, self);
    }

    fn visit_function_declaration(&mut self, ast: &Ast, node: Id<FunctionDeclaration>) {
        let enclosing_exec_old = self.enclosing_exec;
        self.enclosing_exec = self.declared_element(node);
        ast.visit_children(node, self);
        self.enclosing_exec = enclosing_exec_old;
    }

    fn visit_function_expression(&mut self, ast: &Ast, node: Id<FunctionExpression>) {
        let parent = ast.parent(node);
        if !parent.is_some_and(|p| ast.is::<FunctionDeclaration>(p)) {
            let element = self.declared_element(node);
            self.add_element(element.map(ElemRef::Base));
        }
        ast.visit_children(node, self);
    }

    fn visit_function_expression_invocation(
        &mut self,
        ast: &Ast,
        node: Id<FunctionExpressionInvocation>,
    ) {
        let element = self.element(node);
        self.add_element(element);
        ast.visit_children(node, self);
    }

    fn visit_generic_type_alias(&mut self, ast: &Ast, node: Id<GenericTypeAlias>) {
        let ctx = self.v.ctx;
        if !is_private_name(ast.tokens.lexeme(ast[node].name)) {
            let ty = self.v.tables.annotation_type.get(ast[node].type_).copied();
            if let Some(ty) = ty
                && let TypeKind::Interface { element, .. } = *ctx.ty(ty)
            {
                for &constructor in &ctx.interface(element).constructors {
                    let name = ctx.element_name(constructor.raw()).unwrap_or("");
                    if !is_private_name(name) {
                        self.add_element(Some(ElemRef::Base(constructor.raw())));
                    }
                }
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_index_expression(&mut self, ast: &Ast, node: Id<IndexExpression>) {
        let element = write_or_read_element(self.v, node.raw());
        self.add_member(element);
        ast.visit_children(node, self);
    }

    fn visit_instance_creation_expression(
        &mut self,
        ast: &Ast,
        node: Id<InstanceCreationExpression>,
    ) {
        self.add_parameters_for_arguments(ast[node].argument_list);
        ast.visit_children(node, self);
    }

    fn visit_is_expression(&mut self, ast: &Ast, node: Id<IsExpression>) {
        let inside_is_expression_old = self.inside_is_expression;
        ast.accept(ast[node].expression, self);
        self.inside_is_expression = true;
        ast.accept(ast[node].type_, self);
        self.inside_is_expression = inside_is_expression_old;
    }

    fn visit_method_declaration(&mut self, ast: &Ast, node: Id<MethodDeclaration>) {
        let enclosing_exec_old = self.enclosing_exec;
        self.enclosing_exec = self.declared_element(node);
        ast.visit_children(node, self);
        self.enclosing_exec = enclosing_exec_old;
    }

    fn visit_method_invocation(&mut self, ast: &Ast, node: Id<MethodInvocation>) {
        let ctx = self.v.ctx;
        let function = self
            .element(ast[node].method_name)
            .map(|e| member::base_element(&ctx, e));
        if function.is_some_and(|f| {
            matches!(
                f.tag(),
                Tag::LocalFunction | Tag::Method | Tag::TopLevelFunction
            )
        }) {
            self.add_parameters_for_arguments(ast[node].argument_list);
        }
        ast.visit_children(node, self);
    }

    fn visit_named_type(&mut self, ast: &Ast, node: Id<NamedType>) {
        let element = self.element(node);
        self.use_identifier_element(element);
        ast.visit_children(node, self);
    }

    fn visit_pattern_field(&mut self, ast: &Ast, node: Id<PatternField>) {
        let element = self.element(node);
        self.add_member(element);
        self.add_read_member(element);
        ast.visit_children(node, self);
    }

    fn visit_postfix_expression(&mut self, ast: &Ast, node: Id<PostfixExpression>) {
        let element = self.element(node);
        self.add_member(element);
        ast.visit_children(node, self);
    }

    fn visit_prefix_expression(&mut self, ast: &Ast, node: Id<PrefixExpression>) {
        let element = self.element(node);
        self.add_member(element);
        ast.visit_children(node, self);
    }

    fn visit_relational_pattern(&mut self, ast: &Ast, node: Id<RelationalPattern>) {
        let element = self.element(node);
        self.add_member(element);
        self.add_read_member(element);
        ast.visit_children(node, self);
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        if in_declaration_context(ast, node) {
            return;
        }
        if in_comment_reference(ast, node.raw()) {
            return;
        }
        let ctx = self.v.ctx;
        // Store un-parameterized members.
        let base = write_or_read_element(self.v, node.raw()).map(|e| member::base_element(&ctx, e));
        let is_accessor = base.is_some_and(|e| matches!(e.tag(), Tag::Getter | Tag::Setter));
        let variable = base
            .filter(|_| is_accessor)
            .and_then(|e| member::variable(&ctx, ElemRef::Base(e)))
            .map(|v| member::base_element(&ctx, v));
        let is_identifier_read = is_read_identifier(ast, node);
        if let Some(element) = base
            && is_accessor
            && is_identifier_read
            && let Some(variable) = variable.filter(|v| v.tag() == Tag::TopLevelVariable)
        {
            if first_fragment_flags(&ctx, element)
                .contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE)
            {
                self.add_element(Some(ElemRef::Base(variable)));
            } else {
                self.used_elements.members.insert(element);
                self.add_member_and_corresponding_getter(element);
            }
        } else if base.is_some_and(element_ext::is_local_variable) {
            if is_identifier_read {
                self.add_element(base.map(ElemRef::Base));
            }
        } else {
            let Some(parent) = ast.parent(node) else {
                return;
            };
            let read_element = compound_element(self.v, node.raw(), true);
            self.use_identifier_element(read_element);
            let write_element = compound_element(self.v, node.raw(), false);
            self.use_identifier_element(write_element);
            let node_element = self.element(node);
            self.use_identifier_element(node_element);
            let grandparent = ast.parent(parent);
            let is_executable = base.is_some_and(|e| e.is::<ExecutableElement>());
            // If [node] is a tear-off, assume all parameters are used.
            let function_reference_is_call = (is_executable && ast.is::<MethodInvocation>(parent))
                // named constructor
                || (base.is_some_and(|e| e.tag() == Tag::Constructor)
                    && ast.is::<ConstructorName>(parent)
                    && grandparent.is_some_and(|g| ast.is::<InstanceCreationExpression>(g)))
                // unnamed constructor
                || (base.is_some_and(|e| e.is::<InterfaceElement>())
                    && grandparent.is_some_and(|g| ast.is::<ConstructorName>(g))
                    && grandparent
                        .and_then(|g| ast.parent(g))
                        .is_some_and(|g| ast.is::<InstanceCreationExpression>(g)));
            if let Some(element) = base
                && is_executable
                && is_identifier_read
                && !function_reference_is_call
            {
                for parameter in formal_parameters(&ctx, element) {
                    self.add_element(Some(ElemRef::Base(parameter)));
                }
            }
            match base {
                None => {
                    if is_identifier_read {
                        let name = ast_ext::identifier_name(ast, node).to_string();
                        self.used_elements.unresolved_read_members.insert(name);
                    }
                }
                Some(element) => {
                    let enclosing_element = ctx.element_data(element).and_then(|d| d.enclosing);
                    if let Some(enclosing) = enclosing_element
                        && enclosing.tag() == Tag::Enum
                        && ctx.element_name(element) == Some("values")
                    {
                        // If the 'values' static accessor of the enum is
                        // accessed, then all of the enum values have been
                        // read.
                        let instance = ctx.instance(EId::<InstanceElement>::from_raw(enclosing));
                        for &field in &instance.fields {
                            if element_ext::is_enum_constant(&ctx, field.raw())
                                && let Some(getter) = ctx.get(field).getter
                            {
                                self.used_elements.read_members.insert(getter.raw());
                            }
                        }
                    } else if enclosing_element.is_some_and(|e| e.is::<InstanceElement>())
                        && Some(element) != self.enclosing_exec
                    {
                        self.used_elements.members.insert(element);
                        if is_identifier_read {
                            self.add_member_and_corresponding_getter(element);
                        }
                    }
                }
            }
        }
    }

    fn visit_super_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<SuperConstructorInvocation>,
    ) {
        self.add_parameters_for_arguments(ast[node].argument_list);
        ast.visit_children(node, self);
    }

    fn visit_super_formal_parameter(&mut self, ast: &Ast, node: Id<SuperFormalParameter>) {
        let ctx = self.v.ctx;
        if let Some(element) = self.declared_element(node)
            && element.tag() == Tag::SuperFormalParameter
            && let Some(p) = super_constructor_parameter(&ctx, element)
        {
            self.add_element(Some(p));
        }
        ast.visit_children(node, self);
    }

    fn visit_variable_declaration_list(&mut self, ast: &Ast, node: Id<VariableDeclarationList>) {
        for &annotation in ast.list(ast[node].metadata) {
            ast.accept(annotation, self);
        }
        if let Some(t) = ast[node].type_ {
            ast.accept(t, self);
        }
        for &variable in ast.list(ast[node].variables) {
            ast.accept(variable, self);
        }
    }
}

/// Dart `GatherUsedLocalElementsVisitor._matchParameters(first, second, f)`:
/// calls [f] for the corresponding positional and named parameters.
fn match_parameters(
    ctx: &Ctx<'_>,
    first_list: &[ElementId],
    second_list: &[ElementId],
    mut f: impl FnMut(ElementId, ElementId),
) {
    let segregate = |elements: &[ElementId]| {
        let mut positional = Vec::new();
        let mut named: indexmap::IndexMap<String, ElementId> = indexmap::IndexMap::new();
        for &element in elements {
            if parameter_kind(ctx, element).is_named() {
                if let Some(name) = ctx.element_name(element) {
                    named.insert(name.to_string(), element);
                }
            } else {
                positional.push(element);
            }
        }
        (positional, named)
    };
    let (first_positional, first_named) = segregate(first_list);
    let (second_positional, second_named) = segregate(second_list);
    for (&first, &second) in first_positional.iter().zip(&second_positional) {
        f(first, second);
    }
    for (name, &first) in &first_named {
        if let Some(&second) = second_named.get(name) {
            f(first, second);
        }
    }
}

// ============================================================ UnusedLocalElementsVerifier

/// Dart `unit.accept(UnusedLocalElementsVerifier(reporter, usedElements,
/// library))`.
pub fn verify(v: &mut UnitVerifier<'_>, used: &UsedLocalElements) {
    let facts = AnnotationFacts::compute(v);
    let wild_card_variables_enabled = crate::scope::library_feature_enabled(
        &v.ctx,
        v.library,
        ExperimentalFlag::WildcardVariables,
    );
    let ast = v.ast;
    let unit = v.unit;
    let mut verifier = UnusedLocalElementsVerifier {
        v,
        used_elements: used,
        facts,
        wild_card_variables_enabled,
        pattern_variable_elements: None,
    };
    ast.accept(unit, &mut verifier);
}

/// Dart `UnusedLocalElementsVerifier`: reports `unused_element`,
/// `unused_field`, `unused_local_variable`, ...
struct UnusedLocalElementsVerifier<'v, 'a> {
    v: &'v mut UnitVerifier<'a>,
    /// The elements known to be used.
    used_elements: &'v UsedLocalElements,
    facts: AnnotationFacts,
    /// Whether the `wildcard_variables` feature is enabled.
    wild_card_variables_enabled: bool,
    /// The pattern variable elements of the current
    /// `PatternVariableDeclaration`.
    pattern_variable_elements: Option<Vec<ElementId>>,
}

impl UnusedLocalElementsVerifier<'_, '_> {
    fn declared_element(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        declared_element(self.v, node.into())
    }

    /// Dart `_getCorrespondingParameter(parameter, overridden,
    /// enclosingElement)`.
    fn get_corresponding_parameter(
        &self,
        parameter: ElementId,
        overridden: ElementId,
        enclosing_element: ElementId,
    ) -> Option<ElementId> {
        let ctx = self.v.ctx;
        let overridden_parameters = formal_parameters(&ctx, overridden);
        if parameter_kind(&ctx, parameter).is_named() {
            let name = ctx.element_name(parameter);
            overridden_parameters
                .into_iter()
                .find(|&p| ctx.element_name(p) == name)
        } else {
            let enclosing_parameters = formal_parameters(&ctx, enclosing_element);
            let parameter_index = enclosing_parameters
                .iter()
                .position(|&p| p == parameter)
                .unwrap_or(enclosing_parameters.len());
            // Something is wrong with the overridden element when it has
            // fewer parameters. Ignore it.
            overridden_parameters.get(parameter_index).copied()
        }
    }

    /// Dart `_isNamedWildcard(element)`.
    fn is_named_wildcard(&self, element: ElementId) -> bool {
        let ctx = self.v.ctx;
        if self.wild_card_variables_enabled {
            is_wildcard_variable(&ctx, element)
        } else {
            match ctx.element_name(element) {
                None => false,
                Some(name) => name.chars().all(|c| c == '_'),
            }
        }
    }

    /// Dart `_isPubliclyAccessible(element)`: whether [element] is
    /// accessible outside of the library in which it is declared.
    fn is_publicly_accessible(&self, element: ElementId) -> bool {
        let ctx = self.v.ctx;
        if is_private(&ctx, element) {
            return false;
        }
        if let Some(enclosing) = ctx.element_data(element).and_then(|d| d.enclosing) {
            if enclosing.tag() == Tag::Enum
                && element.tag() == Tag::Constructor
                && is_generative(&ctx, element)
            {
                return false;
            }
            if enclosing.is::<InterfaceElement>()
                && is_private(&ctx, enclosing)
                && (member::is_static(&ctx, ElemRef::Base(element))
                    || element.tag() == Tag::Constructor)
            {
                return false;
            }
            if enclosing.tag() == Tag::Extension {
                return !is_private(&ctx, enclosing);
            }
        }
        true
    }

    /// Dart `_isReadMember(element)`: whether [element] is a private
    /// element which is read somewhere in the library.
    fn is_read_member(&self, element: ElementId) -> bool {
        let ctx = self.v.ctx;
        let element_is_static_variable = is_static_variable(&ctx, element);
        if !is_private(&ctx, element) {
            let enclosing = ctx.element_data(element).and_then(|d| d.enclosing);
            let private_class_or_extension = enclosing.is_some_and(|e| {
                (e.is::<InterfaceElement>() || e.tag() == Tag::Extension) && is_private(&ctx, e)
            });
            if !(private_class_or_extension && element_is_static_variable) {
                return true;
            }
            // Public static fields of private classes, mixins, and
            // extensions are inaccessible from outside the library in which
            // they are declared.
        }
        let mut element = element;
        if element.tag() == Tag::Field {
            match ctx.get(EId::<FieldElement>::from_raw(element)).getter {
                None => return false,
                Some(g) => element = g.raw(),
            }
        }
        let name = ctx.element_name(element);
        if self.used_elements.read_members.contains(&element)
            || name.is_some_and(|n| self.used_elements.unresolved_read_members.contains(n))
        {
            return true;
        }
        if element_is_static_variable {
            return false;
        }
        self.overrides_used_element(element)
    }

    /// Dart `_isUsedElement(element)`.
    fn is_used_element(&self, element: ElementId) -> bool {
        let ctx = self.v.ctx;
        if element_ext::is_local_variable(element)
            || (element.tag() == Tag::LocalFunction
                && !member::is_static(&ctx, ElemRef::Base(element)))
        {
            // local variable or function
        } else if element.is::<FormalParameterElement>() {
            // Only report unused parameters of constructors, methods, and
            // top-level functions.
            let Some(enclosing) = ctx
                .element_data(element)
                .and_then(|d| d.enclosing)
                .filter(|e| {
                    matches!(
                        e.tag(),
                        Tag::Constructor | Tag::Method | Tag::TopLevelFunction
                    )
                })
            else {
                return true;
            };
            let kind = parameter_kind(&ctx, element);
            if !matches!(kind, ParameterKind::Positional | ParameterKind::Named) {
                return true;
            }
            if enclosing.tag() == Tag::Constructor {
                let class = ctx.element_data(enclosing).and_then(|d| d.enclosing);
                if class.is_some_and(|c| {
                    c.is::<InstanceElement>()
                        && !ctx
                            .instance(EId::<InstanceElement>::from_raw(c))
                            .type_params
                            .is_empty()
                }) {
                    // There is an issue matching arguments of instance
                    // creation expressions for generic classes with
                    // parameters, so for now, consider every parameter of a
                    // constructor of a generic class "used". See
                    // https://github.com/dart-lang/sdk/issues/47839.
                    return true;
                }
                let super_constructor = ctx
                    .get(EId::<ConstructorElement>::from_raw(enclosing))
                    .super_constructor
                    .get()
                    .map(|s| member::base_element(&ctx, s));
                if let Some(super_constructor) = super_constructor
                    && let Some(corresponding) =
                        self.get_corresponding_parameter(element, super_constructor, enclosing)
                    && matches!(
                        parameter_kind(&ctx, corresponding),
                        ParameterKind::NamedRequired | ParameterKind::Required
                    )
                {
                    return true;
                }
            }
            if !ctx
                .executable(EId::<ExecutableElement>::from_raw(enclosing))
                .type_params
                .is_empty()
            {
                // There is an issue matching arguments of generic function
                // invocations with parameters, so for now, consider every
                // parameter of a generic function "used". See
                // https://github.com/dart-lang/sdk/issues/47839.
                return true;
            }
            if self.is_publicly_accessible(enclosing) {
                return true;
            }
            if self.overrides_used_parameter(element, enclosing) {
                return true;
            }
        } else if !is_private(&ctx, element) {
            return true;
        }
        if self.facts.has_pragma_vm_entry_point.contains(&element) {
            return true;
        }
        self.used_elements.elements.contains(&element)
    }

    /// Dart `_isUsedMember(element)`.
    fn is_used_member(&self, element: ElementId) -> bool {
        if self.is_publicly_accessible(element) {
            return true;
        }
        if self.facts.has_pragma_vm_entry_point.contains(&element) {
            return true;
        }
        if self.used_elements.members.contains(&element) {
            return true;
        }
        if self.used_elements.elements.contains(&element) {
            return true;
        }
        self.overrides_used_element(element)
    }

    /// Dart `_overriddenElements(element)`.
    fn overridden_elements(&self, element: ElementId) -> Vec<ElementId> {
        let ctx = self.v.ctx;
        let Some(enclosing) = ctx
            .element_data(element)
            .and_then(|d| d.enclosing)
            .and_then(|e| e.cast::<InterfaceElement>())
        else {
            return Vec::new();
        };
        let Some(element_name) = ctx.element_name(element) else {
            return Vec::new();
        };
        let element_name = if element.tag() == Tag::Setter {
            format!("{element_name}=")
        } else {
            element_name.to_string()
        };
        let name = Name::new(&ctx, Some(self.v.library), &element_name);
        match InheritanceManager3::new(ctx).get_overridden(enclosing, name) {
            None => Vec::new(),
            Some(overridden) => overridden
                .into_iter()
                .map(|e| member::base_element(&ctx, e))
                .collect(),
        }
    }

    /// Dart `_overridesUsedElement(element)`.
    fn overrides_used_element(&self, element: ElementId) -> bool {
        self.overridden_elements(element).into_iter().any(|e| {
            self.used_elements.members.contains(&e)
                || self.used_elements.elements.contains(&e)
                || self.overrides_used_element(e)
        })
    }

    /// Dart `_overridesUsedParameter(element, enclosingElement)`: whether
    /// [element] is a parameter of a method which overrides a super class's
    /// method in which the corresponding parameter is used.
    fn overrides_used_parameter(&self, element: ElementId, enclosing_element: ElementId) -> bool {
        let ctx = self.v.ctx;
        for overridden in self.overridden_elements(enclosing_element) {
            let Some(corresponding) =
                self.get_corresponding_parameter(element, overridden, enclosing_element)
            else {
                // The parameter was added in the override.
                continue;
            };
            // The parameter was made optional in the override.
            if matches!(
                parameter_kind(&ctx, corresponding),
                ParameterKind::NamedRequired | ParameterKind::Required
            ) {
                return true;
            }
            if self.used_elements.elements.contains(&corresponding) {
                return true;
            }
        }
        false
    }

    /// Dart `_reportDiagnosticForElement(diagnostic, element)`.
    fn report_diagnostic_for_element(
        &mut self,
        diagnostic: LocatableDiagnostic,
        element: ElementId,
    ) {
        let ctx = self.v.ctx;
        let Some(data) = ctx.element_data(element) else {
            return;
        };
        let Some(fragment) = ctx.fragment_data(data.first_fragment) else {
            return;
        };
        let offset = fragment
            .name_offset
            .or_else(|| {
                fragment
                    .enclosing_fragment
                    .and_then(|f| ctx.fragment_data(f))
                    .and_then(|f| f.name_offset)
            })
            .unwrap_or(0);
        let length = fragment
            .name
            .map(|n| ctx.name_str(n).encode_utf16().count())
            .unwrap_or(0);
        self.v.report(diagnostic.at_offset(offset as usize, length));
    }

    /// Reports [diagnostic] of an unused element ([`diag::unused_element`]).
    fn report_unused_element(&mut self, element: ElementId) {
        let d = diag::unused_element(&display_name(&self.v.ctx, element));
        self.report_diagnostic_for_element(d, element);
    }

    /// Dart `_visitClassElement`, `_visitTopLevelFunctionElement`,
    /// `_visitTopLevelVariableElement`, `_visitTypeAliasElement`.
    fn visit_used_element(&mut self, element: ElementId) {
        if !self.is_used_element(element) {
            self.report_unused_element(element);
        }
    }

    /// Dart `_visitConstructorElement(element)`.
    fn visit_constructor_element(&mut self, element: ElementId) {
        let ctx = self.v.ctx;
        // Only complain about an unused constructor if it is not the only
        // constructor in the class. A single unused, private constructor may
        // serve the purpose of preventing the class from being extended. In
        // serving this purpose, the constructor is "used."
        let constructor_count = ctx
            .element_data(element)
            .and_then(|d| d.enclosing)
            .and_then(|e| e.cast::<InterfaceElement>())
            .map(|e| ctx.interface(e).constructors.len())
            .unwrap_or(0);
        if constructor_count > 1 && !self.is_used_member(element) {
            self.report_unused_element(element);
        }
    }

    /// Dart `_visitFieldElement(element)`.
    fn visit_field_element(&mut self, element: ElementId) {
        if !self.is_read_member(element) {
            let d = diag::unused_field(&display_name(&self.v.ctx, element));
            self.report_diagnostic_for_element(d, element);
        }
    }

    /// Dart `_visitLocalFunctionElement(element)`.
    fn visit_local_function_element(&mut self, element: ElementId) {
        if !self.is_used_element(element) {
            if self.wild_card_variables_enabled && self.is_named_wildcard(element) {
                return;
            }
            self.report_unused_element(element);
        }
    }

    /// Dart `_visitLocalVariableElement(element)`.
    fn visit_local_variable_element(&mut self, element: ElementId) {
        if self.is_used_element(element) {
            return;
        }
        let name = display_name(&self.v.ctx, element);
        if self.used_elements.is_catch_exception(element)
            // TODO(srawlins): Report a wildcard catch clause exception
            // variable.
            && !self.is_named_wildcard(element)
        {
            self.report_diagnostic_for_element(diag::unused_catch_clause(&name), element);
        } else if self.used_elements.is_catch_stack_trace(element) {
            self.report_diagnostic_for_element(diag::unused_catch_stack(&name), element);
        } else if !self.is_named_wildcard(element) {
            self.report_diagnostic_for_element(diag::unused_local_variable(&name), element);
        }
    }

    /// Dart `_visitMethodElement`, `_visitPropertyAccessorElement`.
    fn visit_member_element(&mut self, element: ElementId) {
        if !self.is_used_member(element) {
            self.report_unused_element(element);
        }
    }

    /// The elements of the variables of [list].
    fn variable_elements(&self, list: Id<VariableDeclarationList>) -> Vec<ElementId> {
        let ast = self.v.ast;
        ast.list(ast[list].variables)
            .iter()
            .filter_map(|&variable| self.declared_element(variable))
            .collect()
    }
}

impl AstVisitor for UnusedLocalElementsVerifier<'_, '_> {
    fn visit_catch_clause_parameter(&mut self, ast: &Ast, node: Id<CatchClauseParameter>) {
        if let Some(element) = self.declared_element(node) {
            self.visit_local_variable_element(element);
        }
        ast.visit_children(node, self);
    }

    fn visit_class_declaration(&mut self, ast: &Ast, node: Id<ClassDeclaration>) {
        if let Some(element) = self.declared_element(node) {
            self.visit_used_element(element);
        }
        ast.visit_children(node, self);
    }

    fn visit_constructor_declaration(&mut self, ast: &Ast, node: Id<ConstructorDeclaration>) {
        if ast[node].name.is_some()
            && let Some(element) = self.declared_element(node)
        {
            self.visit_constructor_element(element);
        }
        ast.visit_children(node, self);
    }

    fn visit_declared_identifier(&mut self, ast: &Ast, node: Id<DeclaredIdentifier>) {
        if let Some(element) = self.declared_element(node) {
            self.visit_local_variable_element(element);
        }
        ast.visit_children(node, self);
    }

    fn visit_declared_variable_pattern(&mut self, ast: &Ast, node: Id<DeclaredVariablePattern>) {
        let ctx = self.v.ctx;
        if let Some(element) = self.declared_element(node) {
            let is_duplicate = element_ext::local_variable_fragment(&ctx, element)
                .is_some_and(|f| f.pattern.is_duplicate.get());
            if !is_duplicate {
                if let Some(elements) = self.pattern_variable_elements.as_mut() {
                    elements.push(element);
                } else {
                    self.visit_local_variable_element(element);
                }
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_enum_constant_declaration(&mut self, ast: &Ast, node: Id<EnumConstantDeclaration>) {
        if let Some(element) = self.declared_element(node) {
            self.visit_field_element(element);
        }
        ast.visit_children(node, self);
    }

    fn visit_enum_declaration(&mut self, ast: &Ast, node: Id<EnumDeclaration>) {
        if let Some(element) = self.declared_element(node) {
            self.visit_used_element(element);
        }
        ast.visit_children(node, self);
    }

    fn visit_extension_type_declaration(&mut self, ast: &Ast, node: Id<ExtensionTypeDeclaration>) {
        if let Some(element) = self.declared_element(node) {
            self.visit_used_element(element);
        }
        ast.visit_children(node, self);
    }

    fn visit_field_declaration(&mut self, ast: &Ast, node: Id<FieldDeclaration>) {
        for element in self.variable_elements(ast[node].fields) {
            if element.tag() == Tag::Field {
                self.visit_field_element(element);
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_formal_parameter_list(&mut self, ast: &Ast, node: Id<FormalParameterList>) {
        for &parameter in ast.list(ast[node].parameters) {
            let Some(element) = self.declared_element(parameter) else {
                continue;
            };
            if !self.is_used_element(element) {
                let d = diag::unused_element_parameter(&display_name(&self.v.ctx, element));
                self.report_diagnostic_for_element(d, element);
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_for_parts_with_declarations(&mut self, ast: &Ast, node: Id<ForPartsWithDeclarations>) {
        for element in self.variable_elements(ast[node].variables) {
            self.visit_local_variable_element(element);
        }
        ast.visit_children(node, self);
    }

    fn visit_function_declaration(&mut self, ast: &Ast, node: Id<FunctionDeclaration>) {
        if let Some(element) = self.declared_element(node) {
            match element.tag() {
                Tag::LocalFunction => self.visit_local_function_element(element),
                Tag::Getter | Tag::Setter => self.visit_member_element(element),
                Tag::TopLevelFunction => self.visit_used_element(element),
                _ => {}
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_function_type_alias(&mut self, ast: &Ast, node: Id<FunctionTypeAlias>) {
        if let Some(element) = self.declared_element(node) {
            self.visit_used_element(element);
        }
        ast.visit_children(node, self);
    }

    fn visit_generic_type_alias(&mut self, ast: &Ast, node: Id<GenericTypeAlias>) {
        if let Some(element) = self.declared_element(node) {
            self.visit_used_element(element);
        }
        ast.visit_children(node, self);
    }

    fn visit_method_declaration(&mut self, ast: &Ast, node: Id<MethodDeclaration>) {
        if let Some(element) = self.declared_element(node)
            && matches!(element.tag(), Tag::Method | Tag::Getter | Tag::Setter)
        {
            self.visit_member_element(element);
        }
        ast.visit_children(node, self);
    }

    fn visit_mixin_declaration(&mut self, ast: &Ast, node: Id<MixinDeclaration>) {
        if let Some(element) = self.declared_element(node) {
            self.visit_used_element(element);
        }
        ast.visit_children(node, self);
    }

    fn visit_pattern_variable_declaration(
        &mut self,
        ast: &Ast,
        node: Id<PatternVariableDeclaration>,
    ) {
        let outer_pattern_variable_elements = self.pattern_variable_elements.replace(Vec::new());
        ast.visit_children(node, self);
        let pattern_variable_elements = self.pattern_variable_elements.take().unwrap_or_default();
        self.pattern_variable_elements = outer_pattern_variable_elements;
        let mut elements_to_report = Vec::new();
        for element in pattern_variable_elements {
            // Don't report any of the declared variables as unused, if any of
            // them are used. This allows for a consistent set of patterns to
            // be used, in a case where some declared variables are used, and
            // some are just present to help match, for example, a record
            // shape, or a list, etc.
            if self.used_elements.elements.contains(&element) {
                return;
            }
            if !self.is_named_wildcard(element) {
                elements_to_report.push(element);
            }
        }
        for element in elements_to_report {
            let d = diag::unused_local_variable(&display_name(&self.v.ctx, element));
            self.report_diagnostic_for_element(d, element);
        }
    }

    fn visit_primary_constructor_declaration(
        &mut self,
        ast: &Ast,
        node: Id<PrimaryConstructorDeclaration>,
    ) {
        let ctx = self.v.ctx;
        let parent = ast.parent(node);
        // Do not report a field declared in a primary constructor which is
        // declared in an extension type, since it cannot be removed, and
        // renaming it to be public is not an improvement.
        let reports_fields =
            parent.is_some_and(|p| ast.is::<ClassDeclaration>(p) || ast.is::<EnumDeclaration>(p));
        if reports_fields {
            let parameters = ast[node].formal_parameters;
            for &parameter in ast.list(ast[parameters].parameters) {
                let Some(element) = self.declared_element(parameter) else {
                    continue;
                };
                if element.tag() != Tag::FieldFormalParameter
                    || !first_fragment_flags(&ctx, element)
                        .contains(FragmentFlags::FIELD_FORMAL_PARAMETER_FRAGMENT_IS_DECLARING)
                {
                    continue;
                }
                let field = ctx
                    .get(EId::<FormalParameterElement>::from_raw(element))
                    .field
                    .get();
                if let Some(field) = field
                    && !self.is_read_member(field.raw())
                    && let Some((keyword, name)) = declaring_parameter_tokens(ast, parameter.raw())
                {
                    let d = diag::unused_field_from_primary_constructor(
                        &display_name(&ctx, field.raw()),
                        ast.tokens.lexeme(keyword),
                    );
                    let d = self.v.at_token(d, name);
                    self.v.report(d);
                }
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_top_level_variable_declaration(
        &mut self,
        ast: &Ast,
        node: Id<TopLevelVariableDeclaration>,
    ) {
        for element in self.variable_elements(ast[node].variables) {
            self.visit_used_element(element);
        }
        ast.visit_children(node, self);
    }

    fn visit_variable_declaration_statement(
        &mut self,
        ast: &Ast,
        node: Id<VariableDeclarationStatement>,
    ) {
        for element in self.variable_elements(ast[node].variables) {
            self.visit_local_variable_element(element);
        }
        ast.visit_children(node, self);
    }
}

/// Dart `parameter.finalOrVarKeyword!` and `parameter.name!` of a declaring
/// parameter of a primary constructor.
fn declaring_parameter_tokens(
    ast: &Ast,
    parameter: NodeId,
) -> Option<(dartr_syntax::TokenId, dartr_syntax::TokenId)> {
    if let Some(p) = ast.cast::<dartr_ast::FieldFormalParameter>(parameter) {
        return Some((ast[p].const_final_or_var_keyword?, ast[p].name));
    }
    if let Some(p) = ast.cast::<dartr_ast::RegularFormalParameter>(parameter) {
        return Some((ast[p].const_final_or_var_keyword?, ast[p].name?));
    }
    None
}

// ============================================================ annotations

/// The facts of the annotations of the declarations of a unit that the
/// verifiers need (Dart `metadata.hasJS`, `metadata.hasPragmaVmEntryPoint`).
///
/// The annotations are not resolved or evaluated yet, so this reads the
/// syntax: `@JS(...)` / `@prefix.JS(...)`, and `@pragma('vm:entry-point')`
/// whose first argument is a simple string literal.
#[derive(Default)]
struct AnnotationFacts {
    has_js: IndexSet<ElementId>,
    has_pragma_vm_entry_point: IndexSet<ElementId>,
}

impl AnnotationFacts {
    fn compute(v: &UnitVerifier<'_>) -> AnnotationFacts {
        struct Collector<'v, 'a> {
            v: &'v UnitVerifier<'a>,
            facts: AnnotationFacts,
        }
        impl AstVisitor for Collector<'_, '_> {
            fn visit_annotation(&mut self, ast: &Ast, node: Id<Annotation>) {
                let name = annotation_name(ast, ast[node].name);
                let is_js = name == "JS";
                let is_pragma = name == "pragma" && is_vm_entry_point_pragma(ast, node);
                if (is_js || is_pragma)
                    && let Some(parent) = ast.parent(node)
                {
                    for element in annotated_elements(self.v, parent) {
                        if is_js {
                            self.facts.has_js.insert(element);
                        } else {
                            self.facts.has_pragma_vm_entry_point.insert(element);
                        }
                    }
                }
                ast.visit_children(node, self);
            }
        }
        let mut collector = Collector {
            v,
            facts: AnnotationFacts::default(),
        };
        v.ast.accept(v.unit, &mut collector);
        collector.facts
    }
}

/// The last identifier of the name of an annotation.
fn annotation_name(ast: &Ast, name: Id<Identifier>) -> &str {
    if let Some(p) = ast.cast::<PrefixedIdentifier>(name) {
        return ast_ext::identifier_name(ast, ast[p].identifier);
    }
    match ast.cast::<SimpleIdentifier>(name) {
        Some(s) => ast_ext::identifier_name(ast, s),
        None => "",
    }
}

/// Whether the first argument of the annotation [node] is the string
/// literal `'vm:entry-point'`.
fn is_vm_entry_point_pragma(ast: &Ast, node: Id<Annotation>) -> bool {
    let Some(arguments) = ast[node].arguments else {
        return false;
    };
    let Some(&first) = ast.list(ast[arguments].arguments).first() else {
        return false;
    };
    if ast.cast::<StringLiteral>(first).is_none() {
        return false;
    }
    let begin = ast.begin_token(first.raw());
    let text = ast.tokens.lexeme(begin);
    text.trim_matches(|c| c == '\'' || c == '"') == "vm:entry-point"
}

/// The elements whose metadata has the annotations of [parent] (Dart: the
/// metadata of a field or variable declaration list belongs to each
/// variable).
fn annotated_elements(v: &UnitVerifier<'_>, parent: NodeId) -> Vec<ElementId> {
    let ast = v.ast;
    let list = if let Some(n) = ast.cast::<FieldDeclaration>(parent) {
        Some(ast[n].fields)
    } else if let Some(n) = ast.cast::<TopLevelVariableDeclaration>(parent) {
        Some(ast[n].variables)
    } else {
        ast.cast::<VariableDeclarationList>(parent)
    };
    match list {
        Some(list) => ast
            .list(ast[list].variables)
            .iter()
            .filter_map(|&variable| declared_element(v, variable.raw()))
            .collect(),
        None => declared_element(v, parent).into_iter().collect(),
    }
}

// ============================================================ element helpers

/// The element of the declared fragment of [node].
fn declared_element(v: &UnitVerifier<'_>, node: NodeId) -> Option<ElementId> {
    let fragment = *v.tables.declared_fragment.get(node)?;
    v.ctx.fragment_data(fragment)?.element.try_get().copied()
}

/// The flags of the first fragment of [e].
fn first_fragment_flags(ctx: &Ctx<'_>, e: ElementId) -> FragmentFlags {
    element_ext::first_fragment_flags(ctx, e)
}

/// Dart `Identifier.isPrivateName(name)`.
fn is_private_name(name: &str) -> bool {
    name.starts_with('_')
}

/// Dart `element.isPrivate`: `ElementImpl.isPrivate` (no name: private),
/// `InterfaceElementImpl` (`_firstFragment.isPrivate`; no name: public).
fn is_private(ctx: &Ctx<'_>, element: ElementId) -> bool {
    match ctx.element_name(element) {
        Some(name) => is_private_name(name),
        None => !element.is::<InterfaceElement>(),
    }
}

/// Dart `element is VariableElement && element.isStatic` (a top-level
/// variable is static).
fn is_static_variable(ctx: &Ctx<'_>, element: ElementId) -> bool {
    match element.tag() {
        Tag::TopLevelVariable => true,
        Tag::Field => member::is_static(ctx, ElemRef::Base(element)),
        _ => false,
    }
}

/// Dart `ConstructorElement.isGenerative`.
fn is_generative(ctx: &Ctx<'_>, element: ElementId) -> bool {
    !first_fragment_flags(ctx, element).contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY)
}

/// Dart `FormalParameterElement.parameterKind`.
fn parameter_kind(ctx: &Ctx<'_>, element: ElementId) -> ParameterKind {
    match element.cast::<FormalParameterElement>() {
        Some(p) => ctx.get(p).kind,
        None => ParameterKind::Required,
    }
}

/// Dart `element.formalParameters` of an executable element.
fn formal_parameters(ctx: &Ctx<'_>, element: ElementId) -> Vec<ElementId> {
    match element.cast::<ExecutableElement>() {
        Some(e) => ctx
            .executable(e)
            .formal_params
            .iter()
            .map(|p| p.raw())
            .collect(),
        None => Vec::new(),
    }
}

/// Dart `SuperFormalParameterElementImpl.superConstructorParameter`.
fn super_constructor_parameter(ctx: &Ctx<'_>, element: ElementId) -> Option<ElemRef> {
    let enclosing = ctx.element_data(element)?.enclosing?;
    let constructor = enclosing.cast::<ConstructorElement>()?;
    let super_constructor = ctx.get(constructor).super_constructor.get()?;
    let super_parameters = member::formal_parameters(ctx, super_constructor);
    if parameter_kind(ctx, element).is_named() {
        let name = ctx.element_name(element);
        return super_parameters.into_iter().find(|&p| {
            let base = member::base_element(ctx, p);
            parameter_kind(ctx, base).is_named() && ctx.element_name(base) == name
        });
    }
    // Dart `indexIn(enclosingElement)`.
    let index = formal_parameters(ctx, enclosing)
        .into_iter()
        .filter(|&p| p.tag() == Tag::SuperFormalParameter && parameter_kind(ctx, p).is_positional())
        .position(|p| p == element)?;
    super_parameters
        .into_iter()
        .filter(|&p| parameter_kind(ctx, member::base_element(ctx, p)).is_positional())
        .nth(index)
}

/// Dart `element.isWildcardVariable`.
fn is_wildcard_variable(ctx: &Ctx<'_>, element: ElementId) -> bool {
    if ctx.element_name(element) != Some("_") {
        return false;
    }
    let kind_ok = matches!(
        element.tag(),
        Tag::LocalFunction
            | Tag::LocalVariable
            | Tag::PatternVariable
            | Tag::BindPatternVariable
            | Tag::JoinPatternVariable
            | Tag::Prefix
            | Tag::TypeParameter
            | Tag::FormalParameter
    );
    let library: Option<EId<LibraryElement>> = ctx.element_data(element).and_then(|d| d.library);
    kind_ok
        && library.is_some_and(|l| {
            crate::scope::library_feature_enabled(ctx, l, ExperimentalFlag::WildcardVariables)
        })
}

/// Dart `element.displayName`.
fn display_name(ctx: &Ctx<'_>, element: ElementId) -> String {
    let name = ctx.element_name(element);
    if element.tag() == Tag::Constructor {
        let class_name = ctx
            .element_data(element)
            .and_then(|d| d.enclosing)
            .and_then(|e| ctx.element_name(e))
            .unwrap_or("<null>");
        let name = name.unwrap_or("<null>");
        return if name != "new" {
            format!("{class_name}.{name}")
        } else {
            class_name.to_string()
        };
    }
    // Dart `ElementImpl.displayName` (`name ?? '<unnamed>'`);
    // `InstanceElementImpl` uses the fragment (`name ?? ''`).
    match name {
        Some(name) => name.to_string(),
        None if element.is::<InstanceElement>() => String::new(),
        None => "<unnamed>".to_string(),
    }
}
