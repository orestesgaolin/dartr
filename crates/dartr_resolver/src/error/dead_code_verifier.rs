// Dart source: pkg/analyzer/lib/src/error/dead_code_verifier.dart

//! Dead code: [`verify`] (Dart `DeadCodeVerifier`, a library-wide step:
//! unused labels, undefined names in combinators, wildcard local functions
//! and late wildcard variables) and [`NullSafetyDeadCodeVerifier`] (the
//! unreachable code that flow analysis finds; the resolver drives it).
//!
//! # The resolver part
//!
//! Dart `ResolverVisitor.nullSafetyDeadCodeVerifier` is the field
//! `ResolverVisitor::null_safety_dead_code_verifier` (the state). The Dart
//! methods are the free functions of this module that take the resolver
//! ([`flow_end`], [`visit_node`], [`try_statement_enter`], ...), because
//! they read the AST and the flow analysis and report through the resolver.

use dartr_ast::{
    AnonymousMethodInvocation, Assertion, AstVisitor, BinaryExpression, Block, BlockFunctionBody,
    BreakStatement, CascadeExpression, CatchClause, Combinator, Comment, ConstructorDeclaration,
    ConstructorInitializer, ContinueStatement, DoStatement, EmptyFunctionBody, ExportDirective,
    Expression, ForParts, ForPartsWithDeclarations, ForPartsWithExpression, ForPartsWithPattern,
    FunctionDeclaration, FunctionExpression, HideCombinator, Id, ImportDirective, IndexExpression,
    Label, LabeledStatement, LogicalOrPattern, MethodDeclaration, MethodInvocation, NodeId,
    NodeList, PropertyAccess, ShowCombinator, SimpleIdentifier, SwitchMember, SwitchStatement,
    TryStatement, VariableDeclaration, VariableDeclarationList,
};
use dartr_diagnostics::{LocatableDiagnostic, diag};
use dartr_element::diagnostics::type_arg;
use dartr_element::{
    Ctx, DirectiveUri, EId, ElemRef, ElementId, FragmentFlags, LibraryElement, Nullability,
    PromotableElement, Tag, TypeId,
};
use dartr_flow::flow_analysis::FlowAnalysis;
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_syntax::{TokenId, TokenType};
use dartr_typesystem::TypeExt;

use super::{UnitVerifier, VerifierHost};
use crate::ast_ext;
use crate::resolver::ResolverVisitor;

// ============================================================ DeadCodeVerifier

/// Dart `unit.accept(DeadCodeVerifier(diagnosticReporter, library))`.
pub fn verify(v: &mut UnitVerifier<'_>) {
    let wild_card_variables_enabled = crate::scope::library_feature_enabled(
        &v.ctx,
        v.library,
        ExperimentalFlag::WildcardVariables,
    );
    let ast = v.ast;
    let unit = v.unit;
    let mut verifier = DeadCodeVerifier {
        v,
        label_trackers: Vec::new(),
        wild_card_variables_enabled,
        import_index: 0,
        export_index: 0,
    };
    ast.accept(unit, &mut verifier);
}

/// Dart `DeadCodeVerifier`: a visitor that finds dead code, other than
/// unreachable code that is handled in [`NullSafetyDeadCodeVerifier`].
struct DeadCodeVerifier<'v, 'a> {
    v: &'v mut UnitVerifier<'a>,
    /// Dart `_labelTracker` and its `outerTracker` chain (the last one is
    /// the innermost).
    label_trackers: Vec<LabelTracker>,
    /// Dart `_wildCardVariablesEnabled`.
    wild_card_variables_enabled: bool,
    /// The index of the next import directive in `library_imports` (Dart
    /// `node.libraryImport`, matched by position).
    import_index: usize,
    /// The index of the next export directive in `library_exports`.
    export_index: usize,
}

impl DeadCodeVerifier<'_, '_> {
    /// Dart `_labelTracker?.recordUsage(labelName)`.
    fn record_usage(&mut self, label_name: Option<&str>) {
        let Some(label_name) = label_name else {
            return;
        };
        for tracker in self.label_trackers.iter_mut().rev() {
            if let Some(&index) = tracker.label_map.get(label_name) {
                tracker.used[index] = true;
                return;
            }
        }
    }

    /// Dart `_withLabelTracker(labels, f)`.
    fn with_label_tracker(&mut self, labels: Vec<Id<Label>>, f: impl FnOnce(&mut Self)) {
        let ast = self.v.ast;
        let tracker = LabelTracker::new(ast, labels);
        self.label_trackers.push(tracker);
        f(self);
        let tracker = self.label_trackers.pop().expect("label tracker");
        for label in tracker.unused_labels() {
            let name = ast.tokens.lexeme(ast[label].name);
            let d = self.v.at(diag::unused_label(name), label);
            self.v.report(d);
        }
    }

    /// Dart `_checkCombinator(library, combinator)`: resolves the names in
    /// [combinator] in the export namespace of [library].
    fn check_combinator(&mut self, library: EId<LibraryElement>, combinator: Id<Combinator>) {
        let ast = self.v.ast;
        let ctx = self.v.ctx;
        let namespace = ctx.get(library).export_namespace.try_get().cloned();
        let (names, hide) = if let Some(c) = ast.cast::<HideCombinator>(combinator) {
            (ast[c].hidden_names, true)
        } else if let Some(c) = ast.cast::<ShowCombinator>(combinator) {
            (ast[c].shown_names, false)
        } else {
            return;
        };
        let library_uri = library_uri(&ctx, library);
        for &name in ast.list(names) {
            let name_str = ast_ext::identifier_name(ast, name);
            let found = namespace.as_ref().is_some_and(|namespace| {
                namespace.defined_names.contains_key(&ctx.name(name_str))
                    || namespace
                        .defined_names
                        .contains_key(&ctx.name(&format!("{name_str}=")))
            });
            if !found {
                let d = if hide {
                    diag::undefined_hidden_name(&library_uri, name_str)
                } else {
                    diag::undefined_shown_name(&library_uri, name_str)
                };
                let d = self.v.at(d, name);
                self.v.report(d);
            }
        }
    }

    /// The element of the declaration [node] (Dart
    /// `node.declaredFragment!.element`).
    fn declared_element(&self, node: NodeId) -> Option<ElementId> {
        let fragment = *self.v.tables.declared_fragment.get(node)?;
        self.v
            .ctx
            .fragment_data(fragment)?
            .element
            .try_get()
            .copied()
    }

    /// Whether [element] is named `_`.
    fn is_wildcard_name(&self, element: ElementId) -> bool {
        let ctx = self.v.ctx;
        ctx.element_data(element)
            .and_then(|d| d.name)
            .is_some_and(|n| ctx.name_str(n) == "_")
    }
}

impl AstVisitor for DeadCodeVerifier<'_, '_> {
    fn visit_break_statement(&mut self, ast: &dartr_ast::Ast, node: Id<BreakStatement>) {
        let label = ast[node].label.map(|l| ast.tokens.lexeme(ast[l].name));
        self.record_usage(label);
    }

    fn visit_continue_statement(&mut self, ast: &dartr_ast::Ast, node: Id<ContinueStatement>) {
        let label = ast[node].label.map(|l| ast.tokens.lexeme(ast[l].name));
        self.record_usage(label);
    }

    fn visit_export_directive(&mut self, ast: &dartr_ast::Ast, node: Id<ExportDirective>) {
        let index = self.export_index;
        self.export_index += 1;
        let ctx = self.v.ctx;
        let library = ctx
            .fragment(self.v.fragment)
            .library_exports
            .get(index)
            .and_then(|e| directive_library(&e.directive.uri));
        // The element is null when the URI is invalid.
        if let Some(library) = library
            && !is_origin_not_existing_file(&ctx, library)
        {
            for &combinator in ast.list(ast[node].combinators) {
                self.check_combinator(library, combinator);
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_function_declaration(&mut self, ast: &dartr_ast::Ast, node: Id<FunctionDeclaration>) {
        if let Some(element) = self.declared_element(node.raw())
            && self.wild_card_variables_enabled
            && element.tag() == Tag::LocalFunction
            && self.is_wildcard_name(element)
        {
            let d = self.v.at(diag::dead_code(), node);
            self.v.report(d);
        }
        ast.visit_children(node, self);
    }

    fn visit_import_directive(&mut self, ast: &dartr_ast::Ast, node: Id<ImportDirective>) {
        let index = self.import_index;
        self.import_index += 1;
        let ctx = self.v.ctx;
        let library = ctx
            .fragment(self.v.fragment)
            .library_imports
            .get(index)
            .and_then(|i| directive_library(&i.directive.uri));
        // The element is null when the URI is invalid, but not when the URI
        // is valid but refers to a nonexistent file.
        if let Some(library) = library
            && !is_origin_not_existing_file(&ctx, library)
        {
            for &combinator in ast.list(ast[node].combinators) {
                self.check_combinator(library, combinator);
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_labeled_statement(&mut self, ast: &dartr_ast::Ast, node: Id<LabeledStatement>) {
        let labels = ast.list(ast[node].labels).to_vec();
        self.with_label_tracker(labels, |this| ast.visit_children(node, this));
    }

    fn visit_switch_statement(&mut self, ast: &dartr_ast::Ast, node: Id<SwitchStatement>) {
        let mut labels = Vec::new();
        for &member in ast.list(ast[node].members) {
            labels.extend_from_slice(ast.list(ast_ext::switch_member_labels(ast, member)));
        }
        self.with_label_tracker(labels, |this| ast.visit_children(node, this));
    }

    fn visit_variable_declaration(&mut self, ast: &dartr_ast::Ast, node: Id<VariableDeclaration>) {
        let is_late = ast
            .parent(node)
            .and_then(|p| ast.cast::<VariableDeclarationList>(p))
            .is_some_and(|list| ast[list].late_keyword.is_some());
        if let Some(initializer) = ast[node].initializer
            && is_late
            && let Some(element) = self.declared_element(node.raw())
            && self.wild_card_variables_enabled
            && is_local_variable_element(element)
            && self.is_wildcard_name(element)
        {
            let d = self.v.at(
                diag::dead_code_late_wildcard_variable_initializer(),
                initializer,
            );
            self.v.report(d);
        }
        ast.visit_children(node, self);
    }
}

/// Dart `element is LocalVariableElement`.
fn is_local_variable_element(element: ElementId) -> bool {
    matches!(
        element.tag(),
        Tag::LocalVariable
            | Tag::PatternVariable
            | Tag::BindPatternVariable
            | Tag::JoinPatternVariable
    )
}

/// Dart `libraryImport.importedLibrary` / `libraryExport.exportedLibrary`.
pub(crate) fn directive_library(uri: &DirectiveUri) -> Option<EId<LibraryElement>> {
    match uri {
        DirectiveUri::Library { library, .. } => Some(*library),
        _ => None,
    }
}

/// Dart `library.isOriginNotExistingFile`.
pub(crate) fn is_origin_not_existing_file(ctx: &Ctx<'_>, library: EId<LibraryElement>) -> bool {
    let first = ctx.get(library).first_fragment();
    ctx.fragment(first)
        .flags
        .has(FragmentFlags::LIBRARY_FRAGMENT_IS_ORIGIN_NOT_EXISTING_FILE)
}

/// Dart `'${library.uri}'`.
pub(crate) fn library_uri(ctx: &Ctx<'_>, library: EId<LibraryElement>) -> String {
    let first = ctx.get(library).first_fragment();
    ctx.fragment(first).source.uri.to_string()
}

/// Dart `_LabelTracker`: the usage of the labels of one label scope.
struct LabelTracker {
    /// The labels whose usage is being tracked.
    labels: Vec<Id<Label>>,
    /// Whether the label at the same index of [labels] has been used.
    used: Vec<bool>,
    /// The names of the labels to the index of the label in [labels].
    label_map: indexmap::IndexMap<String, usize>,
}

impl LabelTracker {
    fn new(ast: &dartr_ast::Ast, labels: Vec<Id<Label>>) -> LabelTracker {
        let mut label_map = indexmap::IndexMap::new();
        for (i, &label) in labels.iter().enumerate() {
            label_map.insert(ast.tokens.lexeme(ast[label].name).to_string(), i);
        }
        LabelTracker {
            used: vec![false; labels.len()],
            labels,
            label_map,
        }
    }

    /// Dart `unusedLabels()`.
    fn unused_labels(&self) -> Vec<Id<Label>> {
        self.labels
            .iter()
            .zip(&self.used)
            .filter(|(_, used)| !**used)
            .map(|(&label, _)| label)
            .collect()
    }
}

// ============================================================ NullSafetyDeadCodeVerifier

/// Dart `DeadCodeForPartsState`: the state captured by
/// [`for_condition_end`] for [`for_updater_begin`].
pub struct DeadCodeForPartsState {
    /// The value of `first_dead_node` at the time of the call to
    /// [`for_condition_end`].
    first_dead_node_as_of_condition_end: Option<NodeId>,
}

/// Dart `NullSafetyDeadCodeVerifier`: the state of the tracking of dead
/// code (catch clauses and unreachable code).
///
/// Catch clauses are checked separately: as the resolver visits the AST,
/// it may mark some of them as dead, and record
/// `dead_catch_clause_ranges`.
///
/// When an unreachable node is found and `first_dead_node` is `None`, the
/// node starts a new dead code interval. The interval ends when
/// [`flow_end`] is called with a node that is the start node or contains
/// it, so at the end of the covering control flow.
#[derive(Default)]
pub struct NullSafetyDeadCodeVerifier {
    /// The stack of verifiers of (potentially nested) try statements.
    catch_clauses_verifiers: Vec<CatchClausesVerifier>,
    /// When a sequence of catch clauses is found to be dead, no additional
    /// dead code is reported inside of it: `(offset, length)`.
    dead_catch_clause_ranges: Vec<(u32, u32)>,
    /// `None` in reachable code; the first unreachable node otherwise.
    /// While it is set, a new unreachable node continues the same range.
    first_dead_node: Option<NodeId>,
}

/// Dart `nullSafetyDeadCodeVerifier.flowEnd(node)`: [node] ends a basic
/// block in the control flow. If the first dead node is covered by [node],
/// the current dead code interval ends.
pub fn flow_end(rv: &mut ResolverVisitor<'_>, node: impl Into<NodeId>) {
    let mut node: NodeId = node.into();
    let Some(first_dead_node) = rv.null_safety_dead_code_verifier.first_dead_node else {
        return;
    };
    if !contains_first_dead_node(rv, node) {
        return;
    }
    let ast = &*rv.ast;

    if let Some(member) = ast.cast::<SwitchMember>(node)
        && node == first_dead_node
    {
        let keyword = switch_member_keyword(ast, member);
        let d = rv.at_token(diag::dead_code(), keyword);
        rv.report(d);
        rv.null_safety_dead_code_verifier.first_dead_node = None;
        return;
    }

    let parent = ast.parent(first_dead_node);
    let mut reports: Vec<(u32, u32)> = Vec::new();
    let is_assert_message = parent
        .and_then(|p| ast.cast::<Assertion>(p))
        .is_some_and(|p| assertion_message(ast, p.raw()) == Some(first_dead_node));
    let parent_is_constructor = parent.is_some_and(|p| ast.is::<ConstructorDeclaration>(p));
    if is_assert_message {
        // Don't report "dead code" for the message part of an assert
        // statement, because this causes nuisance warnings for redundant
        // `!= null` asserts.
    } else if parent_is_constructor && ast.is::<EmptyFunctionBody>(first_dead_node) {
        // Don't report "dead code" for an unreachable, but syntactically
        // required, semicolon that follows one or more constructor
        // initializers.
    } else if parent_is_constructor
        && ast
            .cast::<BlockFunctionBody>(first_dead_node)
            .is_some_and(|b| ast.list(ast[ast[b].block].statements).is_empty())
    {
        // Don't report "dead code" for an unreachable, but empty block body
        // that follows one or more constructor initializers.
    } else {
        let mut offset = ast.offset(first_dead_node);
        // We know that [node] is the first dead node, or contains it. So,
        // technically the code interval ends at the end of [node]. But we
        // trim it to the last statement for presentation purposes.
        if node != first_dead_node {
            if let Some(n) = ast.cast::<FunctionDeclaration>(node) {
                node = ast[ast[n].function_expression].body.raw();
            }
            if let Some(n) = ast.cast::<FunctionExpression>(node) {
                node = ast[n].body.raw();
            }
            if let Some(n) = ast.cast::<MethodDeclaration>(node) {
                node = ast[n].body.raw();
            }
            if let Some(n) = ast.cast::<BlockFunctionBody>(node) {
                node = ast[n].block.raw();
            }
            if let Some(n) = ast.cast::<Block>(node)
                && let Some(&last) = ast.list(ast[n].statements).last()
            {
                node = last.raw();
            }
            if let Some(n) = ast.cast::<SwitchMember>(node)
                && let Some(&last) = ast.list(ast_ext::switch_member_statements(ast, n)).last()
            {
                node = last.raw();
            }
        } else if let Some(p) = parent.and_then(|p| ast.cast::<BinaryExpression>(p)) {
            offset = token_offset(rv, ast[p].operator);
        }
        if let Some(p) = parent.filter(|&p| ast.is::<ConstructorInitializer>(p)) {
            reports.push((ast.offset(p), ast.end(p) - ast.offset(p)));
            offset = ast.end(node);
        } else if let Some(p) = parent.and_then(|p| ast.cast::<DoStatement>(p)) {
            let mut while_offset = token_offset(rv, ast[p].while_keyword);
            let while_end = ast.tokens.get(ast[p].semicolon).end();
            let body = ast[p].body;
            if let Some(body) = ast.cast::<Block>(body) {
                while_offset = token_offset(rv, ast[body].right_bracket);
            }
            reports.push((while_offset, while_end.saturating_sub(while_offset)));
            let next = ast.tokens.next(ast[p].semicolon);
            offset = ast.tokens.get(next).offset;
            if do_statement_has_break_statement(rv, p) {
                offset = ast.end(node);
            }
        } else if let Some(p) = parent.filter(|&p| ast.is::<ForParts>(p)) {
            if let Some(&last) = ast.list(for_parts_updaters(ast, p)).last() {
                node = last.raw();
            }
        } else if let Some(p) = parent.and_then(|p| ast.cast::<BinaryExpression>(p)) {
            offset = token_offset(rv, ast[p].operator);
            node = ast[p].right_operand.raw();
        } else if let Some(p) = parent.and_then(|p| ast.cast::<LogicalOrPattern>(p))
            && first_dead_node == ast[p].right_operand.raw()
        {
            offset = token_offset(rv, ast[p].operator);
        } else if let Some(p) = parent.and_then(|p| ast.cast::<AnonymousMethodInvocation>(p)) {
            offset = token_offset(rv, ast[p].operator);
        }
        let end = ast.end(node);
        if end > offset {
            reports.push((offset, end - offset));
        }
    }
    for (offset, length) in reports {
        rv.report(diag::dead_code().at_offset(offset as usize, length as usize));
    }
    rv.null_safety_dead_code_verifier.first_dead_node = None;
}

/// Dart `for_conditionEnd()`: the dead code analysis at the end of the
/// `condition` part of `ForParts`. The result goes to
/// [`for_updater_begin`] after the body of the loop.
pub fn for_condition_end(rv: &ResolverVisitor<'_>) -> DeadCodeForPartsState {
    DeadCodeForPartsState {
        first_dead_node_as_of_condition_end: rv.null_safety_dead_code_verifier.first_dead_node,
    }
}

/// Dart `for_updaterBegin(updaters, state)`: the dead code analysis at the
/// beginning of the `updaters` part of `ForParts`.
pub fn for_updater_begin(
    rv: &mut ResolverVisitor<'_>,
    updaters: NodeList<Expression>,
    state: DeadCodeForPartsState,
) {
    let is_reachable = rv
        .flow_analysis
        .flow
        .as_ref()
        .is_none_or(|flow| flow.is_reachable());
    if !is_reachable && state.first_dead_node_as_of_condition_end.is_none() {
        // A dead code range started either at the beginning of the loop body
        // or somewhere inside it, and so the updaters are dead. Since the
        // updaters appear textually before the loop body, they need their
        // own dead code warning.
        let begin_token = rv.ast.list_begin_token(updaters);
        let end_token = rv.ast.list_end_token(updaters);
        if let (Some(begin_token), Some(end_token)) = (begin_token, end_token) {
            let offset = rv.ast.tokens.get(begin_token).offset;
            let end = rv.ast.tokens.get(end_token).end();
            let length = end.saturating_sub(offset);
            rv.report(diag::dead_code().at_offset(offset as usize, length as usize));
        }
    }
}

/// Dart `maybeRewriteFirstDeadNode(oldNode, newNode)`: [old] is being
/// rewritten into [new] in the syntax tree.
pub fn maybe_rewrite_first_dead_node(rv: &mut ResolverVisitor<'_>, old: NodeId, new: NodeId) {
    let verifier = &mut rv.null_safety_dead_code_verifier;
    if verifier.first_dead_node == Some(old) {
        verifier.first_dead_node = Some(new);
    }
}

/// Dart `tryStatementEnter(node)`.
pub fn try_statement_enter(rv: &mut ResolverVisitor<'_>, node: Id<TryStatement>) {
    let catch_clauses = rv.ast.list(rv.ast[node].catch_clauses).to_vec();
    rv.null_safety_dead_code_verifier
        .catch_clauses_verifiers
        .push(CatchClausesVerifier {
            catch_clauses,
            done: false,
            visited_types: Vec::new(),
        });
}

/// Dart `tryStatementExit(node)`.
pub fn try_statement_exit(rv: &mut ResolverVisitor<'_>, node: Id<TryStatement>) {
    let _ = node;
    rv.null_safety_dead_code_verifier
        .catch_clauses_verifiers
        .pop();
}

/// Dart `verifyCascadeExpression(node)`.
pub fn verify_cascade_expression(rv: &mut ResolverVisitor<'_>, node: Id<CascadeExpression>) {
    let ast = &*rv.ast;
    let target = Some(ast[node].target);
    let Some(&first) = ast.list(ast[node].cascade_sections).first() else {
        return;
    };
    if let Some(first) = ast.cast::<PropertyAccess>(first) {
        let operator = ast[first].operator;
        verify_unassigned_simple_identifier(rv, node.raw(), target, Some(operator));
    } else if let Some(first) = ast.cast::<MethodInvocation>(first) {
        let operator = ast[first].operator;
        verify_unassigned_simple_identifier(rv, node.raw(), target, operator);
    } else if let Some(first) = ast.cast::<IndexExpression>(first) {
        let period = ast[first].period;
        verify_unassigned_simple_identifier(rv, node.raw(), target, period);
    }
}

/// Dart `verifyCatchClause(node)`.
pub fn verify_catch_clause(rv: &mut ResolverVisitor<'_>, node: Id<CatchClause>) {
    let Some(verifier) = rv
        .null_safety_dead_code_verifier
        .catch_clauses_verifiers
        .last()
    else {
        return;
    };
    if verifier.done {
        return;
    }
    next_catch_clause(rv, node);
}

/// Dart `verifyIndexExpression(node)`.
pub fn verify_index_expression(rv: &mut ResolverVisitor<'_>, node: Id<IndexExpression>) {
    let target = rv.ast[node].target;
    let question = rv.ast[node].question;
    verify_unassigned_simple_identifier(rv, node.raw(), target, question);
}

/// Dart `verifyMethodInvocation(node)`.
pub fn verify_method_invocation(rv: &mut ResolverVisitor<'_>, node: Id<MethodInvocation>) {
    let target = rv.ast[node].target;
    let operator = rv.ast[node].operator;
    verify_unassigned_simple_identifier(rv, node.raw(), target, operator);
}

/// Dart `verifyPropertyAccess(node)`.
pub fn verify_property_access(rv: &mut ResolverVisitor<'_>, node: Id<PropertyAccess>) {
    let target = rv.ast[node].target;
    let operator = rv.ast[node].operator;
    verify_unassigned_simple_identifier(rv, node.raw(), target, Some(operator));
}

/// Dart `nullSafetyDeadCodeVerifier.visitNode(node)` (also Dart
/// `ResolverVisitor.checkUnreachableNode`).
pub fn visit_node(rv: &mut ResolverVisitor<'_>, node: impl Into<NodeId>) {
    let node: NodeId = node.into();
    // Comments are visited after bodies of functions. So, they look
    // unreachable, but this does not make sense.
    if rv.ast.is::<Comment>(node) {
        return;
    }
    rv.flow_analysis.check_unreachable_node(node);

    // If the first dead node is not `None`, even if this new node is
    // unreachable, we can ignore it as it is part of the same dead code
    // range anyway.
    if rv.null_safety_dead_code_verifier.first_dead_node.is_some() {
        return;
    }
    let Some(flow) = rv.flow_analysis.flow.as_ref() else {
        return;
    };
    if flow.is_reachable() {
        return;
    }
    // If in a dead `CatchClause`, no need to report dead code.
    let offset = rv.ast.offset(node);
    for &(start, length) in &rv.null_safety_dead_code_verifier.dead_catch_clause_ranges {
        // Dart `SourceRange.contains`: `offset <= x && x < offset + length`.
        if start <= offset && offset < start + length {
            return;
        }
    }
    rv.null_safety_dead_code_verifier.first_dead_node = Some(node);
}

/// Dart `_containsFirstDeadNode(parent)`.
fn contains_first_dead_node(rv: &ResolverVisitor<'_>, parent: NodeId) -> bool {
    let mut node = rv.null_safety_dead_code_verifier.first_dead_node;
    while let Some(n) = node {
        if n == parent {
            return true;
        }
        node = rv.ast.parent(n);
    }
    false
}

/// Dart `_verifyUnassignedSimpleIdentifier(node, target, operator)`.
fn verify_unassigned_simple_identifier(
    rv: &mut ResolverVisitor<'_>,
    mut node: NodeId,
    target: Option<Id<Expression>>,
    operator: Option<TokenId>,
) {
    if rv.flow_analysis.flow.is_none() {
        // `isDefinitelyUnassigned` needs a flow.
        return;
    }
    let Some(operator) = operator else {
        return;
    };
    let operator_type = rv.ast.tokens.get(operator).ty;
    if operator_type != TokenType::QUESTION
        && operator_type != TokenType::QUESTION_PERIOD
        && operator_type != TokenType::QUESTION_PERIOD_PERIOD
    {
        return;
    }
    let Some(target) = target else {
        return;
    };
    let Some(target_type) = rv.static_type(target) else {
        return;
    };
    if rv.ctx.nullability_suffix(target_type) != Nullability::Question {
        return;
    }
    let target = ast_ext::un_parenthesized(rv.ast, target);
    let Some(target) = rv.ast.cast::<SimpleIdentifier>(target) else {
        return;
    };
    let element = match rv.element(target) {
        Some(ElemRef::Base(e)) => e.cast::<PromotableElement>(),
        _ => None,
    };
    let Some(element) = element else {
        return;
    };
    if !rv.flow_analysis.is_definitely_unassigned(element) {
        return;
    }
    let mut parent = rv.ast.parent(node);
    while let Some(p) = parent
        && (rv.ast.is::<MethodInvocation>(p)
            || rv.ast.is::<PropertyAccess>(p)
            || rv.ast.is::<IndexExpression>(p))
    {
        node = p;
        parent = rv.ast.parent(node);
    }
    let offset = token_offset(rv, operator);
    let end = rv.ast.end(node);
    rv.report(diag::dead_code().at_offset(offset as usize, end.saturating_sub(offset) as usize));
}

/// Dart `_CatchClausesVerifier.nextCatchClause(catchClause)` on the
/// innermost verifier.
fn next_catch_clause(rv: &mut ResolverVisitor<'_>, catch_clause: Id<CatchClause>) {
    let current_type: Option<TypeId> = rv.ast[catch_clause]
        .exception_type
        .and_then(|t| rv.tables.annotation_type.get(t).copied());
    let Some(verifier) = rv
        .null_safety_dead_code_verifier
        .catch_clauses_verifiers
        .last()
    else {
        return;
    };
    let catch_clauses = verifier.catch_clauses.clone();
    let visited_types = verifier.visited_types.clone();
    let Some(&last) = catch_clauses.last() else {
        return;
    };

    // Found catch clause that doesn't have an exception type. Generate a
    // diagnostic on any following catch clauses.
    let current_type = match current_type {
        Some(t) if !rv.ctx.is_dart_core_object(t) => t,
        _ => {
            if catch_clause != last
                && let Some(index) = catch_clauses.iter().position(|&c| c == catch_clause)
            {
                report_dead_catch_clauses(
                    rv,
                    catch_clauses[index + 1],
                    last,
                    diag::dead_code_catch_following_catch(),
                );
                set_catch_clauses_done(rv);
            }
            return;
        }
    };

    // An on-catch clause was found; verify that the exception type is not a
    // subtype of a previous on-catch exception type.
    for type_ in visited_types {
        if rv.type_system.is_subtype_of(current_type, type_) {
            let d = diag::dead_code_on_catch_subtype(
                type_arg(&rv.ctx, current_type),
                type_arg(&rv.ctx, type_),
            );
            report_dead_catch_clauses(rv, catch_clause, last, d);
            set_catch_clauses_done(rv);
            return;
        }
    }
    if let Some(v) = rv
        .null_safety_dead_code_verifier
        .catch_clauses_verifiers
        .last_mut()
    {
        v.visited_types.push(current_type);
    }
}

/// Dart `_CatchClausesVerifier._done = true`.
fn set_catch_clauses_done(rv: &mut ResolverVisitor<'_>) {
    if let Some(v) = rv
        .null_safety_dead_code_verifier
        .catch_clauses_verifiers
        .last_mut()
    {
        v.done = true;
    }
}

/// The report function of Dart `tryStatementEnter`: reports [diagnostic]
/// from [first] to the end of [last], and records the dead range.
fn report_dead_catch_clauses(
    rv: &mut ResolverVisitor<'_>,
    first: Id<CatchClause>,
    last: Id<CatchClause>,
    diagnostic: LocatableDiagnostic,
) {
    let offset = rv.ast.offset(first);
    let length = rv.ast.end(last).saturating_sub(offset);
    rv.report(diagnostic.at_offset(offset as usize, length as usize));
    rv.null_safety_dead_code_verifier
        .dead_catch_clause_ranges
        .push((offset, length));
}

/// Dart `_CatchClausesVerifier`.
struct CatchClausesVerifier {
    catch_clauses: Vec<Id<CatchClause>>,
    done: bool,
    visited_types: Vec<TypeId>,
}

/// Dart `DoStatementExtension.hasBreakStatement`: whether the body of [node]
/// has a `break` that targets [node] (Dart `_BreakDoStatementVisitor`).
fn do_statement_has_break_statement(rv: &ResolverVisitor<'_>, node: Id<DoStatement>) -> bool {
    struct BreakDoStatementVisitor<'r, 'a> {
        rv: &'r ResolverVisitor<'a>,
        do_statement: NodeId,
        has_break_statement: bool,
    }
    impl AstVisitor for BreakDoStatementVisitor<'_, '_> {
        fn visit_break_statement(&mut self, ast: &dartr_ast::Ast, node: Id<BreakStatement>) {
            let element = ast[node]
                .label
                .and_then(|l| match self.rv.tables.element.get(l) {
                    Some(ElemRef::Base(e)) => Some(*e),
                    _ => None,
                });
            let target = crate::flow_analysis_visitor::get_label_target(
                ast,
                self.rv.tables,
                &self.rv.ctx,
                node.raw(),
                element,
                true,
            );
            if target.map(|t| t.raw()) == Some(self.do_statement) {
                self.has_break_statement = true;
            }
        }
    }
    let mut visitor = BreakDoStatementVisitor {
        rv,
        do_statement: node.raw(),
        has_break_statement: false,
    };
    let body = rv.ast[node].body;
    rv.ast.visit_children(body, &mut visitor);
    visitor.has_break_statement
}

/// Dart `SwitchMember.keyword`.
fn switch_member_keyword(ast: &dartr_ast::Ast, member: Id<SwitchMember>) -> TokenId {
    if let Some(m) = ast.cast::<dartr_ast::SwitchCase>(member) {
        ast[m].keyword
    } else if let Some(m) = ast.cast::<dartr_ast::SwitchDefault>(member) {
        ast[m].keyword
    } else {
        let m = ast
            .cast::<dartr_ast::SwitchPatternCase>(member)
            .expect("switch member");
        ast[m].keyword
    }
}

/// Dart `Assertion.message`.
fn assertion_message(ast: &dartr_ast::Ast, node: NodeId) -> Option<NodeId> {
    if let Some(n) = ast.cast::<dartr_ast::AssertStatement>(node) {
        ast[n].message.map(|m| m.raw())
    } else if let Some(n) = ast.cast::<dartr_ast::AssertInitializer>(node) {
        ast[n].message.map(|m| m.raw())
    } else {
        None
    }
}

/// Dart `ForParts.updaters`.
fn for_parts_updaters(ast: &dartr_ast::Ast, node: NodeId) -> NodeList<Expression> {
    if let Some(n) = ast.cast::<ForPartsWithDeclarations>(node) {
        ast[n].updaters
    } else if let Some(n) = ast.cast::<ForPartsWithExpression>(node) {
        ast[n].updaters
    } else {
        let n = ast.cast::<ForPartsWithPattern>(node).expect("ForParts");
        ast[n].updaters
    }
}

/// Dart `token.offset`.
fn token_offset(rv: &ResolverVisitor<'_>, token: TokenId) -> u32 {
    rv.ast.tokens.get(token).offset
}
