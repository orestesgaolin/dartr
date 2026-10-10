// Dart source: pkg/analysis_server/lib/src/services/completion/dart/in_scope_completion_pass.dart

//! The first completion pass (Dart `InScopeCompletionPass`): a visitor that
//! starts at the node at the completion offset and adds the suggestions
//! that are valid there.

use dartr_ast::*;
use dartr_element::{ElemRef, ElementId, InterfaceElement, Nullability, Tag, TypeId, TypeKind};
use dartr_parser::experimental_flags::ExperimentalFlag as F;
use dartr_syntax::{TokenId, TokenType};
use dartr_typesystem::{TypeExt, TypeSystem, member};

use super::candidate::{Candidate, Kind};
use super::declaration::{DeclConfig, DeclarationHelper};
use super::keyword::{
    IdentifierHelper, KeywordHelper as K, add_labels, compute_overrides_for, element_before,
    variable_list_is_const, variable_list_is_final,
};
use super::relevance::{ContextInput, compute_context_type};
use super::target::TokenExt;
use super::{Out, Request, elem};

/// The options of [`Pass::for_expression`].
#[derive(Clone, Copy)]
struct ExprOpts {
    must_be_assignable: bool,
    must_be_non_void: bool,
    can_be_bool: bool,
    can_be_null: bool,
    can_suggest_const: bool,
    prefer_non_invocation: bool,
    include_trailing_comma_after_closure: bool,
}

impl Default for ExprOpts {
    fn default() -> Self {
        ExprOpts {
            must_be_assignable: false,
            must_be_non_void: false,
            can_be_bool: true,
            can_be_null: true,
            can_suggest_const: true,
            prefer_non_invocation: false,
            include_trailing_comma_after_closure: false,
        }
    }
}

/// The options of [`Pass::for_type_annotation`].
#[derive(Clone, Default)]
struct TypeOpts {
    must_be_extensible: bool,
    must_be_implementable: bool,
    must_be_mixable: bool,
    must_be_non_void: bool,
    exclude_type_names: bool,
    excluded_nodes: Vec<NodeId>,
    is_in_declaration: bool,
    no_void: bool,
    no_dynamic: bool,
}

/// Dart `InScopeCompletionPass`.
pub struct Pass<'q, 'r, 'a> {
    q: &'q Request<'r, 'a>,
    pub out: Out,
    pub decl: Option<DeclarationHelper>,
    ident: Option<IdentifierHelper>,
    skip_imports: bool,
    suggest_overrides: bool,
    suggest_uris: bool,
}

/// Dart `inStaticContext`.
fn in_static_context(ast: &Ast, node: NodeId) -> bool {
    let mut enclosing = ast.parent(node);
    while let Some(e) = enclosing {
        if let Some(m) = ast.cast::<MethodDeclaration>(e) {
            return ast[m]
                .modifier_keyword
                .is_some_and(|k| ast.t_lexeme(k) == "static");
        }
        if ast.is::<FunctionBody>(e) {
            if let Some(c) = ast.parent(e).and_then(|p| ast.cast::<ConstructorDeclaration>(p)) {
                return ast[c].factory_keyword.is_some();
            }
        }
        if let Some(l) = ast.cast::<VariableDeclarationList>(e) {
            if ast.parent(e).is_some_and(|p| ast.is::<FieldDeclaration>(p)) {
                return ast[l].late_keyword.is_none();
            }
        }
        enclosing = ast.parent(e);
    }
    true
}

/// Dart `lastNonSynthetic`.
fn last_non_synthetic(ast: &Ast, node: NodeId) -> Option<TokenId> {
    let mut current = ast.end_tok(node);
    let stop = ast.t_prev(ast.begin(node));
    loop {
        if Some(current) == stop {
            return None;
        }
        if !ast.t_synthetic(current) {
            return Some(current);
        }
        current = ast.t_prev(current)?;
    }
}

/// Dart `Token.nextNonSynthetic`.
fn next_non_synthetic(ast: &Ast, token: TokenId) -> TokenId {
    let mut candidate = ast.t_next(token);
    while ast.t_synthetic(candidate) && !ast.t_is_eof(candidate) {
        let next = ast.t_next(candidate);
        if next == candidate {
            break;
        }
        candidate = next;
    }
    candidate
}

fn list_contains(ast: &Ast, list: &[NodeId], child: NodeId) -> bool {
    let _ = ast;
    list.contains(&child)
}

/// Dart `isChildInList`.
fn is_child_in_list(ast: &Ast, parent: NodeId, child: NodeId) -> bool {
    macro_rules! l {
        ($t:ty, $($f:ident),+) => {{
            let n = ast.cast::<$t>(parent).unwrap();
            false $(|| list_contains(ast, ast.list_raw(ast[n].$f), child))+
        }};
    }
    match ast.kind(parent) {
        NodeKind::AdjacentStrings => l!(AdjacentStrings, strings),
        NodeKind::ArgumentList => l!(ArgumentList, arguments),
        NodeKind::Block => l!(Block, statements),
        NodeKind::BlockClassBody => l!(BlockClassBody, members),
        NodeKind::BlockEnumBody => l!(BlockEnumBody, constants, members),
        NodeKind::CascadeExpression => l!(CascadeExpression, cascade_sections),
        NodeKind::ClassDeclaration => l!(ClassDeclaration, metadata),
        NodeKind::ClassTypeAlias => l!(ClassTypeAlias, metadata),
        NodeKind::Comment => l!(Comment, references),
        NodeKind::CompilationUnit => l!(CompilationUnit, directives, declarations),
        NodeKind::ConstructorDeclaration => l!(ConstructorDeclaration, initializers, metadata),
        NodeKind::DeclaredIdentifier => l!(DeclaredIdentifier, metadata),
        NodeKind::EnumConstantDeclaration => l!(EnumConstantDeclaration, metadata),
        NodeKind::EnumDeclaration => l!(EnumDeclaration, metadata),
        NodeKind::ExportDirective => l!(ExportDirective, combinators, configurations, metadata),
        NodeKind::ExtensionDeclaration => l!(ExtensionDeclaration, metadata),
        NodeKind::ExtensionTypeDeclaration => l!(ExtensionTypeDeclaration, metadata),
        NodeKind::FieldDeclaration => l!(FieldDeclaration, metadata),
        NodeKind::ForEachPartsWithPattern => l!(ForEachPartsWithPattern, metadata),
        NodeKind::RegularFormalParameter => l!(RegularFormalParameter, metadata),
        NodeKind::FieldFormalParameter => l!(FieldFormalParameter, metadata),
        NodeKind::SuperFormalParameter => l!(SuperFormalParameter, metadata),
        NodeKind::FormalParameterList => l!(FormalParameterList, parameters),
        NodeKind::ForPartsWithDeclarations => l!(ForPartsWithDeclarations, updaters),
        NodeKind::ForPartsWithExpression => l!(ForPartsWithExpression, updaters),
        NodeKind::ForPartsWithPattern => l!(ForPartsWithPattern, updaters),
        NodeKind::FunctionDeclaration => l!(FunctionDeclaration, metadata),
        NodeKind::FunctionTypeAlias => l!(FunctionTypeAlias, metadata),
        NodeKind::GenericTypeAlias => l!(GenericTypeAlias, metadata),
        NodeKind::HideCombinator => l!(HideCombinator, hidden_names),
        NodeKind::ImplementsClause => l!(ImplementsClause, interfaces),
        NodeKind::ImportDirective => l!(ImportDirective, combinators, configurations, metadata),
        NodeKind::LabeledStatement => l!(LabeledStatement, labels),
        NodeKind::LibraryDirective => l!(LibraryDirective, metadata),
        NodeKind::ListLiteral => l!(ListLiteral, elements),
        NodeKind::ListPattern => l!(ListPattern, elements),
        NodeKind::MapPattern => l!(MapPattern, elements),
        NodeKind::MethodDeclaration => l!(MethodDeclaration, metadata),
        NodeKind::MixinDeclaration => l!(MixinDeclaration, metadata),
        NodeKind::MixinOnClause => l!(MixinOnClause, superclass_constraints),
        NodeKind::ObjectPattern => l!(ObjectPattern, fields),
        NodeKind::PartDirective => l!(PartDirective, metadata),
        NodeKind::PartOfDirective => l!(PartOfDirective, metadata),
        NodeKind::PatternVariableDeclaration => l!(PatternVariableDeclaration, metadata),
        NodeKind::RecordLiteral => l!(RecordLiteral, fields),
        NodeKind::RecordPattern => l!(RecordPattern, fields),
        NodeKind::RecordTypeAnnotation => l!(RecordTypeAnnotation, positional_fields),
        NodeKind::RecordTypeAnnotationNamedField => l!(RecordTypeAnnotationNamedField, metadata),
        NodeKind::RecordTypeAnnotationPositionalField => {
            l!(RecordTypeAnnotationPositionalField, metadata)
        }
        NodeKind::RecordTypeAnnotationNamedFields => l!(RecordTypeAnnotationNamedFields, fields),
        NodeKind::SetOrMapLiteral => l!(SetOrMapLiteral, elements),
        NodeKind::ShowCombinator => l!(ShowCombinator, shown_names),
        NodeKind::SwitchExpression => l!(SwitchExpression, cases),
        NodeKind::SwitchCase => l!(SwitchCase, labels, statements),
        NodeKind::SwitchDefault => l!(SwitchDefault, labels, statements),
        NodeKind::SwitchPatternCase => l!(SwitchPatternCase, labels, statements),
        NodeKind::SwitchStatement => l!(SwitchStatement, members),
        NodeKind::TopLevelVariableDeclaration => l!(TopLevelVariableDeclaration, metadata),
        NodeKind::TryStatement => l!(TryStatement, catch_clauses),
        NodeKind::TypeArgumentList => l!(TypeArgumentList, arguments),
        NodeKind::TypeParameter => l!(TypeParameter, metadata),
        NodeKind::TypeParameterList => l!(TypeParameterList, type_parameters),
        NodeKind::VariableDeclaration => l!(VariableDeclaration, metadata),
        NodeKind::VariableDeclarationList => l!(VariableDeclarationList, metadata, variables),
        NodeKind::WithClause => l!(WithClause, mixin_types),
        _ => false,
    }
}

/// Dart `CompilationUnit.sortedDirectivesAndDeclarations`.
fn sorted_members(ast: &Ast, unit: Id<CompilationUnit>) -> Vec<NodeId> {
    let mut members: Vec<NodeId> = ast
        .list_raw(ast[unit].directives)
        .iter()
        .chain(ast.list_raw(ast[unit].declarations))
        .copied()
        .collect();
    dartr_ast::sort::dart_sort(&mut members, |a, b| ast.offset(*a) as i64 - ast.offset(*b) as i64);
    members
}

/// Dart `membersBeforeAndAfterMember`.
fn members_around_member(ast: &Ast, unit: Id<CompilationUnit>, member: NodeId) -> (Option<NodeId>, Option<NodeId>) {
    let members = sorted_members(ast, unit);
    let Some(index) = members.iter().position(|m| *m == member) else {
        return (None, members.first().copied().filter(|_| false));
    };
    let before = if index > 0 { Some(members[index - 1]) } else { None };
    let after = members.get(index + 1).copied();
    (before, after)
}

/// Dart `membersBeforeAndAfterOffset`.
fn members_around_offset(ast: &Ast, unit: Id<CompilationUnit>, offset: u32) -> (Option<NodeId>, Option<NodeId>) {
    let mut previous = None;
    for m in sorted_members(ast, unit) {
        if offset < ast.offset(m) {
            return (previous, Some(m));
        }
        previous = Some(m);
    }
    (previous, None)
}

/// Dart `Statement.precedingStatement`.
fn preceding_statement(ast: &Ast, statement: NodeId) -> Option<NodeId> {
    let block = ast.parent(statement).and_then(|p| ast.cast::<Block>(p))?;
    let statements = ast.list_raw(ast[block].statements);
    let index = statements.iter().position(|s| *s == statement)?;
    if index == 0 {
        return None;
    }
    Some(statements[index - 1])
}

/// Dart `ClassMember.precedingMember`.
fn preceding_member(ast: &Ast, member: NodeId) -> Option<NodeId> {
    let body = ast.parent(member)?;
    let members = if let Some(b) = ast.cast::<BlockClassBody>(body) {
        ast.list_raw(ast[b].members)
    } else if let Some(b) = ast.cast::<BlockEnumBody>(body) {
        ast.list_raw(ast[b].members)
    } else {
        return None;
    };
    let index = members.iter().position(|m| *m == member)?;
    if index == 0 {
        return None;
    }
    Some(members[index - 1])
}

/// Dart `ExpressionStatement.isSingleIdentifier` / `FieldDeclaration`
/// variant: `first.isKeywordOrIdentifier && last.isSynthetic &&
/// first.next == last`.
fn is_single_identifier_tokens(ast: &Ast, first: TokenId, last: TokenId) -> bool {
    ast.t_kw_or_ident(first) && ast.t_synthetic(last) && ast.t_next(first) == last
}

/// Dart `TypeAnnotation?.isSingleIdentifier`.
fn type_is_single_identifier(ast: &Ast, ty: Option<NodeId>) -> bool {
    let Some(t) = ty.and_then(|t| ast.cast::<NamedType>(t)) else {
        return false;
    };
    ast[t].question.is_none() && ast[t].type_arguments.is_none() && ast[t].import_prefix.is_none()
}

/// Dart `GuardedPattern.hasWhen`.
fn has_when(ast: &Ast, g: Id<GuardedPattern>) -> bool {
    if ast[g].when_clause.is_some() {
        return true;
    }
    if let Some(p) = ast.cast::<DeclaredVariablePattern>(ast[g].pattern.raw()) {
        if ast.t_lexeme(ast[p].name) == "when" {
            if let Some(t) = ast[p].type_.and_then(|t| ast.cast::<NamedType>(t.raw())) {
                if ast[t].type_arguments.is_none() {
                    return true;
                }
            }
        }
    }
    false
}

/// Dart `PatternField.effectiveName` of each field.
fn field_names(ast: &Ast, fields: NodeList<PatternField>) -> Vec<String> {
    let mut out = Vec::new();
    for &f in ast.list(fields) {
        if let Some(name) = pattern_field_effective_name(ast, f) {
            if !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

/// Dart `PatternField.effectiveName`.
fn pattern_field_effective_name(ast: &Ast, f: Id<PatternField>) -> Option<String> {
    let name = ast[f].name?;
    if let Some(n) = ast[name].name {
        return Some(ast.t_lexeme(n).to_string());
    }
    // Dart `variablePattern?.name.lexeme`.
    let mut p = ast[f].pattern.raw();
    loop {
        if let Some(c) = ast.cast::<CastPattern>(p) {
            p = ast[c].pattern.raw();
        } else if let Some(n) = ast.cast::<NullCheckPattern>(p) {
            p = ast[n].pattern.raw();
        } else if let Some(n) = ast.cast::<NullAssertPattern>(p) {
            p = ast[n].pattern.raw();
        } else {
            break;
        }
    }
    if let Some(d) = ast.cast::<DeclaredVariablePattern>(p) {
        return Some(ast.t_lexeme(ast[d].name).to_string());
    }
    if let Some(a) = ast.cast::<AssignedVariablePattern>(p) {
        return Some(ast.t_lexeme(ast[a].name).to_string());
    }
    None
}

/// The name token of a formal parameter.
fn parameter_name(ast: &Ast, p: NodeId) -> Option<TokenId> {
    if let Some(r) = ast.cast::<RegularFormalParameter>(p) {
        return ast[r].name;
    }
    if let Some(r) = ast.cast::<FieldFormalParameter>(p) {
        return Some(ast[r].name);
    }
    if let Some(r) = ast.cast::<SuperFormalParameter>(p) {
        return Some(ast[r].name);
    }
    None
}

/// The default clause of a formal parameter.
fn parameter_default(ast: &Ast, p: NodeId) -> Option<Id<FormalParameterDefaultClause>> {
    if let Some(r) = ast.cast::<RegularFormalParameter>(p) {
        return ast[r].default_clause;
    }
    if let Some(r) = ast.cast::<FieldFormalParameter>(p) {
        return ast[r].default_clause;
    }
    if let Some(r) = ast.cast::<SuperFormalParameter>(p) {
        return ast[r].default_clause;
    }
    None
}

/// Dart `FormalParameter.isIncomplete`.
fn parameter_is_incomplete(ast: &Ast, p: NodeId) -> bool {
    let Some(name) = parameter_name(ast, p) else {
        return true;
    };
    if ast.t_is_keyword(name) {
        return true;
    }
    if ast.t_synthetic(name) {
        let next = ast.t_next(name);
        if ast.t_is_keyword(next) {
            return true;
        }
    }
    if let Some(d) = parameter_default(ast, p) {
        if ast.n_synthetic(ast[d].value.raw()) {
            return true;
        }
    }
    false
}

/// The formal parameter list of a parameter (Dart
/// `parentFormalParameterList`).
fn parent_parameter_list(ast: &Ast, p: NodeId) -> Option<Id<FormalParameterList>> {
    let parent = ast.parent(p)?;
    ast.cast::<FormalParameterList>(parent)
}

/// Dart `InvocationExpression.argumentList` element: the invoked element of
/// an argument list (Dart `ArgumentList.invokedElement`).
fn invoked_element(q: &Request<'_, '_>, list: Id<ArgumentList>) -> Option<ElemRef> {
    let ast = q.ast;
    let tables = q.tables;
    let parent = ast.parent(list)?;
    match ast.kind(parent) {
        NodeKind::Annotation | NodeKind::DotShorthandConstructorInvocation => {
            tables.element.get(parent).copied().or_else(|| {
                let d = ast.cast::<DotShorthandConstructorInvocation>(parent)?;
                tables.element.get(ast[d].constructor_name.raw()).copied()
            })
        }
        NodeKind::DotShorthandInvocation => {
            let d = ast.cast::<DotShorthandInvocation>(parent)?;
            tables.element.get(ast[d].member_name.raw()).copied()
        }
        NodeKind::EnumConstantArguments => {
            let gp = ast.parent(parent)?;
            tables.element.get(gp).copied()
        }
        NodeKind::FunctionExpressionInvocation => {
            let f = ast.cast::<FunctionExpressionInvocation>(parent)?;
            tables.element.get(parent).copied().or_else(|| {
                let function = dartr_resolver::ast_ext::un_parenthesized(ast, ast[f].function);
                ast.cast::<SimpleIdentifier>(function)
                    .and_then(|s| tables.element.get(s.raw()).copied())
            })
        }
        NodeKind::InstanceCreationExpression => {
            let i = ast.cast::<InstanceCreationExpression>(parent)?;
            tables.element.get(ast[i].constructor_name.raw()).copied()
        }
        NodeKind::MethodInvocation => {
            let m = ast.cast::<MethodInvocation>(parent)?;
            tables.element.get(ast[m].method_name.raw()).copied()
        }
        NodeKind::SuperConstructorInvocation | NodeKind::RedirectingConstructorInvocation => {
            tables.element.get(parent).copied()
        }
        _ => None,
    }
}

/// A parameter of the invoked function: (name, kind, type, element).
#[derive(Clone)]
struct InvokedParameter {
    name: Option<String>,
    kind: dartr_element::ParameterKind,
    ty: TypeId,
    element: Option<ElemRef>,
}

/// Dart `ArgumentList.invokedFormalParameters`.
fn invoked_formal_parameters(q: &Request<'_, '_>, list: Id<ArgumentList>) -> Option<Vec<InvokedParameter>> {
    let ast = q.ast;
    let ctx = q.ctx;
    let from_function_type = |t: TypeId| -> Option<Vec<InvokedParameter>> {
        let TypeKind::Function(f) = ctx.ty(t) else {
            return None;
        };
        Some(
            ctx.list(f.params)
                .iter()
                .map(|p| InvokedParameter {
                    name: p.name.map(|n| ctx.name_str(n).to_string()),
                    kind: p.kind,
                    ty: p.ty,
                    element: p.element,
                })
                .collect(),
        )
    };
    if let Some(e) = invoked_element(q, list) {
        let base = member::base_element(ctx, e);
        match base.tag() {
            Tag::Getter => {
                let t = member::return_type(ctx, e);
                return from_function_type(t);
            }
            Tag::Method | Tag::Constructor | Tag::TopLevelFunction | Tag::LocalFunction | Tag::Setter => {
                return Some(
                    member::formal_parameters(ctx, e)
                        .into_iter()
                        .map(|p| {
                            let b = member::base_element(ctx, p);
                            InvokedParameter {
                                name: ctx.element_name(b).map(str::to_string),
                                kind: elem::parameter_kind(ctx, p),
                                ty: member::type_(ctx, p),
                                element: Some(p),
                            }
                        })
                        .collect(),
                );
            }
            Tag::LocalVariable
            | Tag::TopLevelVariable
            | Tag::Field
            | Tag::FormalParameter
            | Tag::FieldFormalParameter
            | Tag::SuperFormalParameter
            | Tag::PatternVariable
            | Tag::BindPatternVariable
            | Tag::JoinPatternVariable => {
                let t = member::type_(ctx, e);
                if let Some(r) = from_function_type(t) {
                    return Some(r);
                }
            }
            _ => {}
        }
    }
    let parent = ast.parent(list)?;
    if let Some(f) = ast.cast::<FunctionExpressionInvocation>(parent) {
        let t = q.tables.static_type.get(ast[f].function.raw()).copied()?;
        return from_function_type(t);
    }
    None
}

impl<'q, 'r, 'a> Pass<'q, 'r, 'a> {
    pub fn new(q: &'q Request<'r, 'a>, out: Out, skip_imports: bool, suggest_overrides: bool, suggest_uris: bool) -> Self {
        Pass {
            q,
            out,
            decl: None,
            ident: None,
            skip_imports,
            suggest_overrides,
            suggest_uris,
        }
    }

    fn ast(&self) -> &'r Ast {
        self.q.ast
    }

    fn offset(&self) -> u32 {
        self.q.offset
    }

    fn feature(&self, f: F) -> bool {
        self.q.feature_enabled(f)
    }

    fn location(&mut self, location: &str) {
        self.out.collector.completion_location = Some(location.to_string());
    }

    fn kw(&mut self, k: &str) {
        K::add_keyword(&mut self.out, k);
    }

    fn kw_text(&mut self, k: &str, text: &str) {
        K::add_keyword_and_text(&mut self.out, k, text);
    }

    fn ci(&self) -> ContextInput<'r, 'a> {
        self.q.ci()
    }

    /// Dart `declarationHelper(...)`: creates the helper once.
    fn decl(&mut self, mut cfg: DeclConfig) -> &mut DeclarationHelper {
        let ctx = self.q.ctx;
        let mut context = self.q.context_type;
        if context.is_some_and(|c| ctx.is_dart_core_function(c)) {
            context = Some(self.void_function_no_parameters());
        }
        if let Some(c) = context {
            if let TypeKind::Function(f) = ctx.ty(c) {
                if matches!(ctx.ty(f.ret), TypeKind::Void) {
                    cfg.must_be_non_void = false;
                    cfg.prefer_non_invocation = true;
                }
            }
        }
        cfg.skip_imports = self.skip_imports;
        self.decl.get_or_insert_with(|| DeclarationHelper::new(cfg))
    }

    fn add_lexical(&mut self, cfg: DeclConfig, node: NodeId) {
        let q = self.q;
        self.decl(cfg);
        let d = self.decl.as_mut().unwrap();
        d.add_lexical_declarations(q, &mut self.out, node);
    }

    fn ident(&mut self, include_private: bool) -> IdentifierHelper {
        let helper = self.ident.get_or_insert(IdentifierHelper { include_private });
        IdentifierHelper {
            include_private: helper.include_private,
        }
    }

    fn void_function_no_parameters(&self) -> TypeId {
        let ctx = self.q.ctx;
        ctx.function_type(&[], &[], ctx.tp.void_type(), Nullability::None, None)
    }

    fn context_type_of(&self, node: NodeId) -> Option<TypeId> {
        let ast = self.ast();
        let parent = ast.parent(node)?;
        compute_context_type(&self.ci(), parent, ast.offset(node))
    }

    /// Dart `_completionNode`.
    fn completion_node(&self) -> NodeId {
        let ast = self.ast();
        let covering = self.q.covering;
        let begin = ast.begin(covering);
        if !ast.t_kw_or_ident(begin) && !ast.is::<SimpleIdentifier>(covering) {
            return covering;
        }
        let o = self.offset();
        if !(ast.t_offset(begin) <= o && o <= ast.t_end(begin)) {
            return covering;
        }
        let mut child = covering;
        let mut parent = ast.parent(child);
        while let Some(p) = parent {
            if ast.begin(p) == begin && !(!ast.is::<SimpleIdentifier>(child) && is_child_in_list(ast, p, child)) {
                child = p;
                parent = ast.parent(child);
            } else {
                break;
            }
        }
        if let Some(p) = parent {
            if !(!ast.is::<SimpleIdentifier>(child) && is_child_in_list(ast, p, child)) {
                return p;
            }
        }
        child
    }

    /// Dart `computeSuggestions`.
    pub fn compute_suggestions(&mut self) {
        let ast = self.ast();
        let mut node = self.completion_node();
        if ast.is::<BlockClassBody>(node) || ast.is::<EnumBody>(node) {
            node = ast.parent(node).unwrap_or(node);
        }
        self.visit(node);
    }

    fn visit_parent(&mut self, node: NodeId) {
        if let Some(p) = self.ast().parent(node) {
            self.visit(p);
        }
    }

    fn visit_parent_if_at_or_before(&mut self, node: NodeId) {
        if self.offset() <= self.ast().offset(node) {
            self.visit_parent(node);
        }
    }

    // ------------------------------------------------------------- dispatch

    pub fn visit(&mut self, node: NodeId) {
        let ast = self.ast();
        match ast.kind(node) {
            NodeKind::AdjacentStrings
            | NodeKind::DoubleLiteral
            | NodeKind::IntegerLiteral
            | NodeKind::StringInterpolation => self.visit_parent_if_at_or_before(node),
            NodeKind::Annotation => self.visit_annotation(node),
            NodeKind::ArgumentList => self.visit_argument_list(node),
            NodeKind::AsExpression => self.visit_as_expression(node),
            NodeKind::AssertInitializer => self.visit_assert_initializer(node),
            NodeKind::AssertStatement => self.visit_assert_statement(node),
            NodeKind::AssignmentExpression => {
                self.location("AssignmentExpression_rightHandSide");
                self.for_expression(node, ExprOpts { must_be_non_void: true, ..Default::default() });
            }
            NodeKind::AwaitExpression => {
                self.location("AwaitExpression_expression");
                self.for_expression(node, ExprOpts { must_be_non_void: true, ..Default::default() });
            }
            NodeKind::BinaryExpression => {
                let b = ast.cast::<BinaryExpression>(node).unwrap();
                let op = ast.t_lexeme(ast[b].operator).to_string();
                self.location(&format!("BinaryExpression_{op}_rightOperand"));
                self.for_expression(node, ExprOpts { must_be_non_void: true, ..Default::default() });
            }
            NodeKind::Block => self.visit_block(node),
            NodeKind::BlockEnumBody => self.visit_parent(node),
            NodeKind::BooleanLiteral
            | NodeKind::ConstructorReference
            | NodeKind::ExtensionOverride
            | NodeKind::FunctionExpressionInvocation
            | NodeKind::FunctionReference
            | NodeKind::NullLiteral
            | NodeKind::SymbolLiteral
            | NodeKind::ThisExpression
            | NodeKind::TypeLiteral => self.for_expression(node, ExprOpts::default()),
            NodeKind::BreakStatement => {
                let s = ast.cast::<BreakStatement>(node).unwrap();
                let end = ast.t_end(ast[s].break_keyword);
                if self.offset() <= end {
                    self.location("Block_statement");
                    self.kw("break");
                } else if end < self.offset() && self.offset() <= ast.t_offset(ast[s].semicolon) {
                    add_labels(self.q, &mut self.out, node);
                }
            }
            NodeKind::CascadeExpression => {
                self.location("CascadeExpression_cascadeSection");
                self.for_expression(node, ExprOpts::default());
            }
            NodeKind::CaseClause => {
                self.location("CaseClause_pattern");
                self.for_pattern(node, true);
            }
            NodeKind::CastPattern => {
                let p = ast.cast::<CastPattern>(node).unwrap();
                if ast.t_covers(Some(ast[p].as_token), self.offset()) {
                    self.kw("as");
                } else {
                    self.location("CastPattern_type");
                    self.for_type_annotation(node, TypeOpts { must_be_non_void: true, ..Default::default() });
                }
            }
            NodeKind::CatchClause => self.visit_catch_clause(node),
            NodeKind::ClassDeclaration => self.visit_class_declaration(node),
            NodeKind::Comment => {}
            NodeKind::CommentReference => {
                self.location("CommentReference_identifier");
                self.add_lexical(DeclConfig { prefer_non_invocation: true, ..Default::default() }, node);
            }
            NodeKind::CompilationUnit => self.visit_compilation_unit(node),
            NodeKind::ConditionalExpression => {
                let c = ast.cast::<ConditionalExpression>(node).unwrap();
                let o = self.offset();
                if o >= ast.t_end(ast[c].question) && o <= ast.t_offset(ast[c].colon) {
                    self.location("ConditionalExpression_thenExpression");
                } else if o >= ast.t_end(ast[c].colon) {
                    self.location("ConditionalExpression_elseExpression");
                }
                self.for_expression(node, ExprOpts::default());
            }
            NodeKind::ConstantPattern => {
                let p = ast.cast::<ConstantPattern>(node).unwrap();
                if ast.is::<SimpleIdentifier>(ast[p].expression.raw()) {
                    self.visit_parent(node);
                }
            }
            NodeKind::ConstructorDeclaration => self.visit_constructor_declaration(node),
            NodeKind::ConstructorFieldInitializer => self.visit_constructor_field_initializer(node),
            NodeKind::ConstructorName => self.visit_constructor_name(node),
            NodeKind::ConstructorSelector => self.visit_constructor_selector(node),
            NodeKind::ContinueStatement => {
                let s = ast.cast::<ContinueStatement>(node).unwrap();
                let end = ast.t_end(ast[s].continue_keyword);
                if self.offset() <= end {
                    self.location("Block_statement");
                    self.kw("continue");
                } else if end < self.offset() && self.offset() <= ast.t_offset(ast[s].semicolon) {
                    add_labels(self.q, &mut self.out, node);
                }
            }
            NodeKind::DeclaredIdentifier => self.visit_parent(node),
            NodeKind::DeclaredVariablePattern => self.visit_declared_variable_pattern(node),
            NodeKind::DoStatement => self.visit_do_statement(node),
            NodeKind::DotShorthandConstructorInvocation => self.visit_dot_shorthand_constructor_invocation(node),
            NodeKind::DotShorthandInvocation => self.visit_dot_shorthand_invocation(node),
            NodeKind::DotShorthandPropertyAccess => self.visit_dot_shorthand_property_access(node),
            NodeKind::EmptyStatement => self.visit_empty_statement(node),
            NodeKind::EnumDeclaration => self.visit_enum_declaration(node),
            NodeKind::ExportDirective => {
                let d = ast.cast::<ExportDirective>(node).unwrap();
                if self.offset() <= ast.t_end(ast[d].export_keyword) {
                    self.for_compilation_unit_member_before(node);
                }
            }
            NodeKind::ExpressionFunctionBody => {
                let b = ast.cast::<ExpressionFunctionBody>(node).unwrap();
                let e = ast[b].expression.raw();
                if self.offset() >= ast.t_end(ast[b].function_definition) && self.offset() <= ast.end(e) {
                    self.location("ExpressionFunctionBody_expression");
                    self.for_expression(e, ExprOpts::default());
                }
            }
            NodeKind::ExpressionStatement => self.visit_expression_statement(node),
            NodeKind::ExtendsClause => {
                let c = ast.cast::<ExtendsClause>(node).unwrap();
                let superclass = ast[c].superclass;
                if self.offset() <= ast.t_end(ast[c].extends_keyword) {
                    self.kw("extends");
                } else if ast.fully_synthetic(superclass.raw())
                    || ast.t_covers(Some(ast[superclass].name), self.offset())
                {
                    self.location("ExtendsClause_superclass");
                    self.for_type_annotation(node, TypeOpts { must_be_extensible: true, ..Default::default() });
                }
            }
            NodeKind::ExtensionDeclaration => self.visit_extension_declaration(node),
            NodeKind::ExtensionOnClause => {
                let c = ast.cast::<ExtensionOnClause>(node).unwrap();
                if self.offset() <= ast.t_end(ast[c].on_keyword) {
                    self.kw("on");
                    return;
                }
                self.location("ExtensionOnClause_extendedType");
                self.for_type_annotation(node, TypeOpts::default());
            }
            NodeKind::ExtensionTypeDeclaration => self.visit_extension_type_declaration(node),
            NodeKind::FieldDeclaration => self.visit_field_declaration(node),
            NodeKind::FieldFormalParameter => self.visit_field_formal_parameter(node),
            NodeKind::ForEachPartsWithDeclaration | NodeKind::ForEachPartsWithIdentifier => {
                self.location("ForEachPartsWithDeclaration_iterable");
                self.visit_for_each_parts(node);
            }
            NodeKind::ForEachPartsWithPattern => {
                self.location("visitForEachPartsWithPattern_iterable");
                self.visit_for_each_parts(node);
            }
            NodeKind::ForElement => {
                self.location("ForElement_body");
                self.for_enclosing_collection(node);
            }
            NodeKind::FormalParameterDefaultClause => {
                let c = ast.cast::<FormalParameterDefaultClause>(node).unwrap();
                let value = ast[c].value.raw();
                if ast.n_covers(Some(value), self.offset()) {
                    self.location("DefaultFormalParameter_defaultValue");
                    self.for_expression(value, ExprOpts { must_be_non_void: true, ..Default::default() });
                }
            }
            NodeKind::FormalParameterList => self.visit_formal_parameter_list(node),
            NodeKind::ForPartsWithDeclarations => self.visit_for_parts_with_declarations(node),
            NodeKind::ForPartsWithExpression => {
                if ast.fully_synthetic(node) {
                    self.visit_parent(node);
                }
            }
            NodeKind::ForStatement => self.visit_for_statement(node),
            NodeKind::FunctionDeclaration => self.visit_function_declaration(node),
            NodeKind::FunctionExpression => self.visit_function_expression(node),
            NodeKind::FunctionTypeAlias => {
                let a = ast.cast::<FunctionTypeAlias>(node).unwrap();
                let typedef = ast[a].typedef_keyword;
                if self.offset() == ast.offset(node) {
                    self.for_compilation_unit_member_before(node);
                } else if self.offset() <= ast.t_end(typedef) {
                    self.location("CompilationUnit_declaration");
                    self.kw("typedef");
                } else if self.offset() <= ast.t_end(ast.t_next(typedef)) {
                    self.add_lexical(DeclConfig { must_be_type: true, ..Default::default() }, node);
                }
            }
            NodeKind::FunctionTypedFormalParameterSuffix => self.visit_function_typed_suffix(node),
            NodeKind::GenericTypeAlias => {
                let a = ast.cast::<GenericTypeAlias>(node).unwrap();
                if self.offset() == ast.offset(node) {
                    self.for_compilation_unit_member_before(node);
                } else if ast.t_covers(Some(ast[a].typedef_keyword), self.offset()) {
                    self.kw("typedef");
                } else if self.offset() >= ast.t_end(ast[a].equals)
                    && self.offset() <= ast.t_offset(ast[a].semicolon)
                {
                    self.location("GenericTypeAlias_type");
                    self.for_type_annotation(node, TypeOpts::default());
                }
            }
            NodeKind::HideCombinator => {
                let c = ast.cast::<HideCombinator>(node).unwrap();
                self.location("HideCombinator_hiddenName");
                let names = ast.list_raw(ast[c].hidden_names).to_vec();
                self.for_combinator(node, &names);
            }
            NodeKind::IfElement => self.visit_if_element(node),
            NodeKind::IfStatement => self.visit_if_statement(node),
            NodeKind::ImplementsClause => {
                let c = ast.cast::<ImplementsClause>(node).unwrap();
                if self.offset() <= ast.t_end(ast[c].implements_keyword) {
                    self.kw("implements");
                } else {
                    self.location("ImplementsClause_interface");
                    self.for_type_annotation(node, TypeOpts { must_be_implementable: true, ..Default::default() });
                }
            }
            NodeKind::ImportDirective => self.visit_import_directive(node),
            NodeKind::ImportPrefixReference => self.visit_import_prefix_reference(node),
            NodeKind::IndexExpression => {
                let i = ast.cast::<IndexExpression>(node).unwrap();
                let begin = ast.begin(node);
                if self.offset() <= ast.t_end(begin) && ast.t_kw_or_ident(begin) {
                    self.visit_parent(node);
                    return;
                }
                if self.offset() >= ast.t_end(ast[i].left_bracket) && self.offset() <= ast.t_offset(ast[i].right_bracket) {
                    self.location("IndexExpression_index");
                    self.for_expression(node, ExprOpts { must_be_non_void: true, ..Default::default() });
                }
            }
            NodeKind::InstanceCreationExpression => self.visit_instance_creation(node),
            NodeKind::InterpolationExpression => {
                self.location("InterpolationExpression_expression");
                let cfg = DeclConfig {
                    must_be_static: in_static_context(ast, node),
                    must_be_non_void: true,
                    ..Default::default()
                };
                self.add_lexical(cfg, node);
            }
            NodeKind::IsExpression => self.visit_is_expression(node),
            NodeKind::Label => {
                let l = ast.cast::<Label>(node).unwrap();
                let name = ast[l].name;
                if !ast.t_synthetic(name) && self.offset() >= ast.t_end(name) {
                    if let Some(p) = ast.parent(node).filter(|p| ast.is::<NamedArgument>(*p)) {
                        self.visit(p);
                    }
                }
            }
            NodeKind::LibraryDirective => {
                if self.offset() >= ast.end(node) {
                    if let Some(unit) = ast.parent(node).and_then(|p| ast.cast::<CompilationUnit>(p)) {
                        self.for_directive(unit, Some(node));
                        let (_, after) = members_around_member(ast, unit, node);
                        if after.is_none_or(|a| ast.is::<CompilationUnitMember>(a)) {
                            self.for_compilation_unit_declaration(unit);
                        }
                    }
                }
            }
            NodeKind::ListLiteral => {
                let l = ast.cast::<ListLiteral>(node).unwrap();
                if self.offset() >= ast.t_end(ast[l].left_bracket) && self.offset() <= ast.t_offset(ast[l].right_bracket) {
                    self.location("ListLiteral_element");
                    let elements = ast.list_raw(ast[l].elements).to_vec();
                    self.for_collection_element(node, &elements);
                }
            }
            NodeKind::ListPattern => {
                self.location("ListPattern_element");
                self.for_pattern(node, true);
            }
            NodeKind::LogicalAndPattern => {
                self.location("LogicalAndPattern_rightOperand");
                self.for_pattern(node, true);
            }
            NodeKind::LogicalOrPattern => {
                self.location("LogicalOrPattern_rightOperand");
                self.for_pattern(node, true);
            }
            NodeKind::MapLiteralEntry => self.visit_map_literal_entry(node),
            NodeKind::MapPattern => {
                self.location("MapPatternEntry_key");
                self.for_constant_expression(node);
            }
            NodeKind::MapPatternEntry => {
                let e = ast.cast::<MapPatternEntry>(node).unwrap();
                let separator = ast[e].separator;
                if ast.t_synthetic(separator) || self.offset() <= ast.t_offset(separator) {
                    self.visit_parent(node);
                    return;
                }
                self.location("MapPatternEntry_value");
                self.for_pattern(node, false);
            }
            NodeKind::MethodDeclaration => self.visit_method_declaration(node),
            NodeKind::MethodInvocation => self.visit_method_invocation(node),
            NodeKind::MixinDeclaration => self.visit_mixin_declaration(node),
            NodeKind::MixinOnClause => {
                let c = ast.cast::<MixinOnClause>(node).unwrap();
                if self.offset() <= ast.t_end(ast[c].on_keyword) {
                    self.kw("on");
                } else {
                    self.for_type_annotation(node, TypeOpts::default());
                }
            }
            NodeKind::NamedArgument => self.visit_named_argument(node),
            NodeKind::NamedType => self.visit_named_type(node),
            NodeKind::NullAwareElement => {
                self.location("NullAwareElement_value");
                self.for_expression(node, ExprOpts::default());
            }
            NodeKind::ObjectPattern => {
                let p = ast.cast::<ObjectPattern>(node).unwrap();
                if ast.t_end(ast[p].left_parenthesis) <= self.offset()
                    && self.offset() <= ast.t_offset(ast[p].right_parenthesis)
                {
                    self.location("ObjectPattern_fieldName");
                    if let Some(t) = self.q.tables.annotation_type.get(ast[p].type_.raw()).copied() {
                        let excluded = field_names(ast, ast[p].fields);
                        let q = self.q;
                        self.decl(DeclConfig {
                            must_be_non_void: true,
                            prefer_non_invocation: true,
                            ..Default::default()
                        });
                        self.decl.as_mut().unwrap().add_getters(q, &mut self.out, t, &excluded, false, false);
                    }
                }
            }
            NodeKind::ParenthesizedExpression => self.visit_parenthesized_expression(node),
            NodeKind::ParenthesizedPattern => {
                self.location("ParenthesizedPattern_expression");
                self.for_pattern(node, true);
            }
            NodeKind::PartDirective => {
                let d = ast.cast::<PartDirective>(node).unwrap();
                if self.offset() <= ast.t_end(ast[d].part_keyword) {
                    self.location("CompilationUnit_directive");
                    self.for_compilation_unit_member_before(node);
                }
            }
            NodeKind::PartOfDirective => {
                let d = ast.cast::<PartOfDirective>(node).unwrap();
                if self.offset() <= ast.t_end(ast[d].part_keyword) {
                    self.location("CompilationUnit_directive");
                    self.for_compilation_unit_member_before(node);
                }
            }
            NodeKind::PatternAssignment => {
                self.location("PatternAssignment_expression");
                self.for_expression(node, ExprOpts::default());
            }
            NodeKind::PatternField => self.visit_pattern_field(node),
            NodeKind::PatternFieldName => {
                let n = ast.cast::<PatternFieldName>(node).unwrap();
                if self.offset() <= ast.t_offset(ast[n].colon) {
                    let parent = ast.parent(node).and_then(|p| ast.parent(p));
                    if parent.is_some_and(|p| ast.is::<ObjectPattern>(p)) {
                        self.location("ObjectPattern_fieldName");
                    } else {
                        self.location("PatternField_pattern");
                    }
                    self.for_pattern_field_name(node, false, false);
                    return;
                }
                self.visit_parent(node);
            }
            NodeKind::PatternVariableDeclaration => {
                self.location("PatternVariableDeclaration_expression");
                self.for_expression(node, ExprOpts::default());
            }
            NodeKind::PostfixExpression => {
                let p = ast.cast::<PostfixExpression>(node).unwrap();
                let ty = ast.t_ty(ast[p].operator);
                self.location(&format!("PrefixExpression_{}_operand", ty.lexeme()));
                self.for_expression(
                    node,
                    ExprOpts {
                        must_be_assignable: ty == TokenType::PLUS_PLUS || ty == TokenType::MINUS_MINUS,
                        ..Default::default()
                    },
                );
            }
            NodeKind::PrefixedIdentifier => self.visit_prefixed_identifier(node),
            NodeKind::PrefixExpression => {
                let p = ast.cast::<PrefixExpression>(node).unwrap();
                self.location("PropertyAccess_propertyName");
                let ty = ast.t_ty(ast[p].operator);
                self.for_expression(
                    node,
                    ExprOpts {
                        must_be_assignable: ty == TokenType::PLUS_PLUS || ty == TokenType::MINUS_MINUS,
                        ..Default::default()
                    },
                );
            }
            NodeKind::PrimaryConstructorDeclaration => self.visit_primary_constructor(node),
            NodeKind::PropertyAccess => self.visit_property_access(node),
            NodeKind::RecordLiteral => {
                self.location("RecordLiteral_fields");
                let context = self.context_type_of(node);
                self.suggest_record_literal_named_fields(context, node, Some(node), true);
                self.for_expression(node, ExprOpts::default());
            }
            NodeKind::RecordLiteralNamedField => {
                let f = ast.cast::<RecordLiteralNamedField>(node).unwrap();
                if self.offset() <= ast.t_end(ast[f].name) {
                    if let Some(r) = ast.parent(node).filter(|p| ast.is::<RecordLiteral>(*p)) {
                        self.location("RecordLiteral_fields");
                        let context = self.context_type_of(r);
                        self.suggest_record_literal_named_fields(context, node, Some(r), false);
                    }
                } else if self.offset() >= ast.t_end(ast[f].colon) {
                    self.location("RecordLiteral_fields");
                    self.for_expression(ast[f].field_expression.raw(), ExprOpts::default());
                }
            }
            NodeKind::RecordPattern => self.visit_record_pattern(node),
            NodeKind::RecordTypeAnnotation => {
                let r = ast.cast::<RecordTypeAnnotation>(node).unwrap();
                if self.offset() <= ast.offset(node) {
                    if let Some(p) = ast.parent(node).filter(|p| ast.is::<FormalParameter>(*p)) {
                        if self.offset() <= ast.offset(p) {
                            self.location("FormalParameterList_parameter");
                            self.for_type_annotation(node, TypeOpts::default());
                        }
                    }
                } else if self.offset() <= ast.t_offset(ast[r].right_parenthesis) {
                    self.location("RecordTypeAnnotation_positionalFields");
                    self.for_type_annotation(node, TypeOpts::default());
                }
            }
            NodeKind::RecordTypeAnnotationNamedField => {
                let f = ast.cast::<RecordTypeAnnotationNamedField>(node).unwrap();
                if ast.n_covers(Some(ast[f].type_.raw()), self.offset()) {
                    self.location("RecordTypeAnnotationNamedFields_fields");
                    self.for_type_annotation(node, TypeOpts::default());
                } else if ast.t_covers(Some(ast[f].name), self.offset()) {
                    self.location("RecordTypeAnnotationNamedField_name");
                    let helper = self.ident(false);
                    helper.add_variable(self.q, &mut self.out, Some(ast[f].type_.raw()));
                }
            }
            NodeKind::RecordTypeAnnotationNamedFields => {
                self.location("RecordTypeAnnotationNamedFields_fields");
                self.for_type_annotation(node, TypeOpts::default());
            }
            NodeKind::RecordTypeAnnotationPositionalField => {
                let f = ast.cast::<RecordTypeAnnotationPositionalField>(node).unwrap();
                if ast.n_covers(Some(ast[f].type_.raw()), self.offset()) {
                    self.location("RecordTypeAnnotation_positionalFields");
                    self.for_type_annotation(node, TypeOpts::default());
                }
            }
            NodeKind::RedirectingConstructorInvocation => self.visit_redirecting_constructor_invocation(node),
            NodeKind::RegularFormalParameter => self.visit_regular_formal_parameter(node),
            NodeKind::RelationalPattern => {
                let p = ast.cast::<RelationalPattern>(node).unwrap();
                let operand = ast[p].operand.raw();
                if ast.t_ty(ast[p].operator) == TokenType::LT
                    && ast.n_synthetic(operand)
                    && ast.t_ty(next_non_synthetic(ast, ast.begin(operand))) == TokenType::GT
                {
                    self.location("TypeArgumentList_argument");
                    self.for_type_annotation(node, TypeOpts::default());
                } else if ast.is::<SimpleIdentifier>(operand)
                    && self.offset() >= ast.t_end(ast[p].operator)
                    && self.offset() <= ast.end(operand)
                {
                    self.location("RelationalPattern_operand");
                    self.for_expression(node, ExprOpts::default());
                }
            }
            NodeKind::RestPatternElement => {
                self.location("RestPatternElement_pattern");
                self.for_pattern(node, true);
            }
            NodeKind::ReturnStatement => {
                let r = ast.cast::<ReturnStatement>(node).unwrap();
                if self.offset() <= ast.t_end(ast[r].return_keyword) {
                    self.location("Block_statement");
                    self.for_statement(node);
                } else {
                    self.location("ReturnStatement_expression");
                    let target = ast[r].expression.map(|e| e.raw()).unwrap_or(node);
                    self.for_expression(target, ExprOpts::default());
                }
            }
            NodeKind::SetOrMapLiteral => {
                let l = ast.cast::<SetOrMapLiteral>(node).unwrap();
                if self.offset() >= ast.t_end(ast[l].left_bracket) && self.offset() <= ast.t_offset(ast[l].right_bracket) {
                    self.location("SetOrMapLiteral_element");
                    let elements = ast.list_raw(ast[l].elements).to_vec();
                    self.for_collection_element(node, &elements);
                }
            }
            NodeKind::ShowCombinator => {
                let c = ast.cast::<ShowCombinator>(node).unwrap();
                self.location("ShowCombinator_shownName");
                let names = ast.list_raw(ast[c].shown_names).to_vec();
                self.for_combinator(node, &names);
            }
            NodeKind::SimpleStringLiteral => self.visit_simple_string_literal(node),
            NodeKind::SpreadElement => {
                self.location("SpreadElement_expression");
                self.for_expression(node, ExprOpts::default());
            }
            NodeKind::SuperConstructorInvocation => self.visit_super_constructor_invocation(node),
            NodeKind::SuperFormalParameter => {
                let p = ast.cast::<SuperFormalParameter>(node).unwrap();
                let q = self.q;
                self.decl(DeclConfig::default());
                self.decl.as_mut().unwrap().add_parameters_from_super_constructor(q, &mut self.out, p);
            }
            NodeKind::SwitchCase => {
                self.location("SwitchMember_statement");
                self.for_statement(node);
            }
            NodeKind::SwitchDefault => {
                let d = ast.cast::<SwitchDefault>(node).unwrap();
                let keyword = ast[d].keyword;
                if self.offset() <= ast.t_offset(keyword) {
                    self.location("SwitchMember_statement");
                    self.kw("case");
                    self.kw_text("default", ":");
                } else if self.offset() <= ast.t_end(keyword) {
                    if ast.t_synthetic(ast[d].colon) {
                        self.kw_text("default", ":");
                    } else {
                        self.kw("default");
                    }
                }
            }
            NodeKind::SwitchExpression => {
                let s = ast.cast::<SwitchExpression>(node).unwrap();
                let o = self.offset();
                if o >= ast.t_end(ast[s].left_parenthesis) && o <= ast.t_offset(ast[s].right_parenthesis) {
                    self.location("SwitchExpression_expression");
                    self.for_expression(node, ExprOpts::default());
                } else if o >= ast.t_end(ast[s].left_bracket) && o <= ast.t_offset(ast[s].right_bracket) {
                    self.location("SwitchExpression_body");
                    self.for_pattern(node, true);
                }
            }
            NodeKind::SwitchExpressionCase => self.visit_switch_expression_case(node),
            NodeKind::SwitchPatternCase => self.visit_switch_pattern_case(node),
            NodeKind::SwitchStatement => self.visit_switch_statement(node),
            NodeKind::ThrowExpression => {
                self.location("ThrowExpression_expression");
                self.for_expression(node, ExprOpts::default());
            }
            NodeKind::TopLevelVariableDeclaration => self.visit_top_level_variable_declaration(node),
            NodeKind::TryStatement => self.visit_try_statement(node),
            NodeKind::TypeArgumentList => self.for_type_annotation(node, TypeOpts::default()),
            NodeKind::TypeParameter => self.visit_type_parameter(node),
            NodeKind::VariableDeclaration => self.visit_variable_declaration(node),
            NodeKind::VariableDeclarationList => self.visit_variable_declaration_list(node),
            NodeKind::VariableDeclarationStatement => {
                self.location("Block_statement");
                if self.for_incomplete_preceding_statement(node) {
                    return;
                }
                if self.offset() <= ast.t_end(ast.begin(node)) {
                    self.for_statement(node);
                } else if self.offset() >= ast.end(node) {
                    if let Some(p) = ast.parent(node) {
                        self.for_statement(p);
                    }
                }
            }
            NodeKind::WhenClause => {
                let w = ast.cast::<WhenClause>(node).unwrap();
                let when = ast[w].when_keyword;
                if !ast.t_synthetic(when) && self.offset() > ast.t_end(when) {
                    self.location("WhenClause_expression");
                    self.for_expression(node, ExprOpts::default());
                }
            }
            NodeKind::WhileStatement => {
                let w = ast.cast::<WhileStatement>(node).unwrap();
                let o = self.offset();
                if o <= ast.t_end(ast[w].while_keyword) {
                    self.location("Block_statement");
                    self.for_statement(node);
                } else if ast.t_end(ast[w].left_parenthesis) <= o && o <= ast.t_offset(ast[w].right_parenthesis) {
                    let condition = ast[w].condition.raw();
                    if ast.n_synthetic(condition) || o <= ast.offset(condition) || o == ast.end(condition) {
                        self.location("WhileStatement_condition");
                        self.for_expression(node, ExprOpts { must_be_non_void: true, ..Default::default() });
                    }
                }
            }
            NodeKind::WildcardPattern => {
                let p = ast.cast::<WildcardPattern>(node).unwrap();
                if let Some(t) = ast[p].type_ {
                    if ast.n_covers(Some(t.raw()), self.offset()) {
                        self.add_lexical(DeclConfig { must_be_type: true, ..Default::default() }, node);
                    }
                }
            }
            NodeKind::WithClause => {
                let w = ast.cast::<WithClause>(node).unwrap();
                let parent = ast.parent(node);
                let with_keyword = ast[w].with_keyword;
                if self.offset() <= ast.t_offset(with_keyword) {
                    if let Some(c) = parent.and_then(|p| ast.cast::<ClassDeclaration>(p)) {
                        K::add_class_declaration_keywords(self.q, &mut self.out, c);
                        return;
                    }
                }
                if self.offset() <= ast.t_end(with_keyword) {
                    self.kw("with");
                } else {
                    self.location("WithClause_mixinType");
                    self.for_type_annotation(node, TypeOpts { must_be_mixable: true, ..Default::default() });
                }
            }
            NodeKind::YieldStatement => {
                let y = ast.cast::<YieldStatement>(node).unwrap();
                if self.offset() <= ast.t_end(ast[y].yield_keyword) {
                    self.location("Block_statement");
                    self.kw("yield");
                } else if ast.t_synthetic(ast[y].semicolon) || self.offset() <= ast.t_end(ast[y].semicolon) {
                    self.location("YieldStatement_expression");
                    self.for_expression(node, ExprOpts::default());
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------- visitors

    fn visit_annotation(&mut self, node: NodeId) {
        let ast = self.ast();
        let a = ast.cast::<Annotation>(node).unwrap();
        self.location("Annotation_name");
        self.for_annotation(node);
        if ast[a].constructor_name.is_none() {
            if let Some(name) = ast.cast::<SimpleIdentifier>(ast[a].name.raw()) {
                let class_node = ast.parent(node).and_then(|p| ast.parent(p)).and_then(|p| ast.parent(p));
                if let Some(c) = class_node.filter(|c| ast.is::<Declaration>(*c)) {
                    let line_info = self.q.line_info;
                    let name_token = ast[name].token;
                    let name_line = line_info.get_location(ast.t_offset(name_token)).line_number;
                    let next = ast.t_next(name_token);
                    let next_line = line_info.get_location(ast.t_offset(next)).line_number;
                    if next_line > name_line + 1 {
                        self.try_override_annotation(name_token, c);
                    }
                }
            }
        }
    }

    fn visit_argument_list(&mut self, node: NodeId) {
        let ast = self.ast();
        let list = ast.cast::<ArgumentList>(node).unwrap();
        let o = self.offset();
        if o <= ast.t_offset(ast[list].left_parenthesis) {
            self.visit_parent(node);
            return;
        }
        if o > ast.t_offset(ast[list].right_parenthesis) {
            self.location("ExpressionStatement_expression");
            return;
        }
        let Some(parent) = ast.parent(node) else {
            return;
        };
        let location = self.location_for(list, false);
        self.location(&location);
        let arguments = ast.list_raw(ast[list].arguments).to_vec();
        let (before, after) = arguments_before_and_after(ast, list, o);
        let mut argument_index = 0;
        if let Some(b) = before {
            if self.handled_possible_closure(b) {
                self.for_expression(b, ExprOpts { must_be_non_void: true, ..Default::default() });
                return;
            }
            argument_index = arguments
                .iter()
                .position(|a| *a == b || argument_expression(ast, *a) == b)
                .unwrap_or(0);
            if o > ast.end(b) {
                argument_index += 1;
            }
        }
        let (positional_count, used_names) = argument_context(ast, list, argument_index as i64);
        let parameters = invoked_formal_parameters(self.q, list);
        if let Some(parameters) = parameters {
            let mut positional_parameter_count = 0;
            let mut available = Vec::new();
            for p in &parameters {
                if p.kind.is_named() {
                    if !p.name.as_ref().is_some_and(|n| used_names.contains(n)) {
                        available.push(p.clone());
                    }
                } else {
                    positional_parameter_count += 1;
                }
            }
            if positional_count < positional_parameter_count {
                let parameter = &parameters[positional_count];
                let ctx = self.q.ctx;
                let mut ty = parameter.ty;
                while let TypeKind::TypeParameter { .. } = ctx.ty(ty) {
                    let bound = ctx.type_parameter_type_bound(ty);
                    if bound == ty {
                        break;
                    }
                    ty = bound;
                }
                if ctx.is_dart_core_function(ty) {
                    ty = self.void_function_no_parameters();
                }
                let is_function = matches!(ctx.ty(ty), TypeKind::Function(_));
                let argument = if is_function && argument_index < arguments.len() {
                    Some(argument_expression(ast, arguments[argument_index]))
                } else {
                    None
                };
                let can_be_null = self.can_be_null(ty);
                let can_be_bool = self.can_be_bool(ty);
                self.for_expression(
                    parent,
                    ExprOpts {
                        must_be_non_void: true,
                        can_be_null,
                        can_be_bool,
                        include_trailing_comma_after_closure: argument
                            .is_none_or(|a| !is_followed_by_comma(ast, a)),
                        can_suggest_const: !is_function,
                        ..Default::default()
                    },
                );
            } else {
                let location = self.location_for(list, true);
                self.location(&location);
            }
            let mut append_comma = false;
            let comma_after_before = before.map(|b| ast.t_next(ast.end_tok(b)));
            if before.is_some_and(|b| o <= ast.end(b))
                && comma_after_before
                    .is_some_and(|c| ast.t_ty(c) == TokenType::COMMA && !ast.t_synthetic(c))
            {
                append_comma = false;
            } else if let Some(a) = after {
                let mut possible = ast.t_prev(ast.begin(a));
                if ast.n_synthetic(a) {
                    possible = Some(ast.t_next(ast.end_tok(a)));
                }
                match possible.filter(|p| ast.t_ty(*p) == TokenType::COMMA) {
                    Some(comma) => {
                        if ast.t_synthetic(comma) {
                            let after_is_named = arguments
                                .iter()
                                .any(|arg| ast.is::<NamedArgument>(*arg) && argument_expression(ast, *arg) == a);
                            if after_is_named
                                || !before.is_some_and(|b| ast.is::<SimpleIdentifier>(b))
                                || o > before.map(|b| ast.end(b)).unwrap_or(0)
                            {
                                append_comma = true;
                            }
                        } else if o >= ast.t_end(comma) {
                            append_comma = true;
                        }
                    }
                    None => append_comma = true,
                }
            } else if let Some(i) = ast.cast::<InstanceCreationExpression>(parent) {
                if self.is_widget_creation(i) {
                    append_comma = true;
                }
            }
            let mut replacement_length = None;
            if before.is_some_and(|b| o == ast.offset(b)) {
                replacement_length = Some(0);
                append_comma = false;
            }
            for p in available {
                let Some(element) = p.element else {
                    continue;
                };
                let ctx = self.q.ctx;
                let name = super::candidate::display_name(ctx, member::base_element(ctx, element));
                let score = self.out.score(&name);
                if score != -1.0 {
                    let is_widget = is_flutter_widget_parameter(ctx, element);
                    self.out.add(Candidate::new(
                        Kind::NamedArgument {
                            parameter: element,
                            append_colon: true,
                            append_comma,
                            replacement_length,
                            is_widget,
                            quote: self.q.style.quote,
                        },
                        score,
                    ));
                }
            }
        } else if ast.is::<Expression>(parent) {
            self.for_expression(parent, ExprOpts { must_be_non_void: true, ..Default::default() });
        }
    }

    fn visit_as_expression(&mut self, node: NodeId) {
        let ast = self.ast();
        let a = ast.cast::<AsExpression>(node).unwrap();
        let o = self.offset();
        if o <= ast.end(ast[a].expression) {
            let cfg = DeclConfig {
                must_be_non_void: true,
                must_be_static: in_static_context(ast, node),
                ..Default::default()
            };
            self.add_lexical(cfg, node);
            return;
        }
        if ast.t_covers(Some(ast[a].as_operator), o) {
            if ast.is::<ParenthesizedExpression>(ast[a].expression.raw()) {
                K::add_function_body_modifiers(self.q, &mut self.out, None);
            } else {
                self.kw("as");
            }
            return;
        }
        let ty = ast[a].type_.raw();
        if ast.fully_synthetic(ty) || ast.t_covers(Some(ast.begin(ty)), o) {
            self.location("AsExpression_type");
            self.for_type_annotation(node, TypeOpts { must_be_non_void: true, ..Default::default() });
        }
    }

    fn visit_assert_initializer(&mut self, node: NodeId) {
        let ast = self.ast();
        let a = ast.cast::<AssertInitializer>(node).unwrap();
        if ast.t_end(ast[a].left_parenthesis) <= self.offset() {
            let comma = ast[a].comma;
            if comma.is_none_or(|c| self.offset() <= ast.t_offset(c)) {
                self.location("AssertInitializer_condition");
            } else {
                self.location("AssertInitializer_message");
            }
            self.for_expression(ast[a].condition.raw(), ExprOpts::default());
            return;
        }
        self.location("ConstructorDeclaration_initializer");
        if let Some(c) = ast.parent(node).and_then(|p| ast.cast::<ConstructorDeclaration>(p)) {
            K::add_constructor_initializer_keywords(self.q, &mut self.out, c, Some(node));
        }
    }

    fn visit_assert_statement(&mut self, node: NodeId) {
        let ast = self.ast();
        let a = ast.cast::<AssertStatement>(node).unwrap();
        let left = ast[a].left_parenthesis;
        if !ast.t_synthetic(left) && ast.t_end(left) <= self.offset() {
            let comma = ast[a].comma;
            if comma.is_none_or(|c| self.offset() <= ast.t_offset(c)) {
                self.location("AssertStatement_condition");
            } else {
                self.location("AssertStatement_message");
            }
            self.for_expression(ast[a].condition.raw(), ExprOpts::default());
            return;
        }
        if self.offset() <= ast.t_end(ast[a].assert_keyword) {
            self.location("Block_statement");
            self.for_statement(node);
        }
    }

    fn visit_block(&mut self, node: NodeId) {
        let ast = self.ast();
        let b = ast.cast::<Block>(node).unwrap();
        if ast.t_synthetic(ast[b].left_bracket) && ast.t_synthetic(ast[b].right_bracket) {
            self.visit_parent(node);
        }
        if self.offset() <= ast.t_offset(ast[b].left_bracket) {
            if let Some(p) = ast.parent(node).filter(|p| ast.is::<BlockFunctionBody>(*p)) {
                self.visit_parent(p);
            }
            return;
        }
        self.location("Block_statement");
        let statements = ast.list_raw(ast[b].statements).to_vec();
        let previous = element_before(ast, &statements, self.offset());
        if let Some(t) = previous.and_then(|p| ast.cast::<TryStatement>(p)) {
            if ast[t].finally_block.is_none() {
                self.kw("on");
                self.kw("catch");
                self.kw("finally");
                if ast.list(ast[t].catch_clauses).is_empty() {
                    return;
                }
            }
        } else if let Some(i) = previous.and_then(|p| ast.cast::<IfStatement>(p)) {
            if ast[i].else_keyword.is_none() {
                self.kw("else");
            }
        }
        self.for_statement(node);
    }

    fn visit_catch_clause(&mut self, node: NodeId) {
        let ast = self.ast();
        let c = ast.cast::<CatchClause>(node).unwrap();
        let o = self.offset();
        let on = ast[c].on_keyword;
        let catch = ast[c].catch_keyword;
        if let Some(on) = on {
            if o <= ast.t_end(on) {
                self.kw("on");
            } else if (catch.is_none() && o <= ast.offset(ast[c].body))
                || catch.is_some_and(|k| o < ast.t_offset(k))
            {
                self.location("CatchClause_exceptionType");
                self.for_type_annotation(node, TypeOpts::default());
            }
        }
        if let Some(k) = catch {
            if o >= ast.t_offset(k) && o <= ast.t_end(k) {
                self.kw("catch");
            }
        }
    }

    fn visit_class_declaration(&mut self, node: NodeId) {
        let ast = self.ast();
        let c = ast.cast::<ClassDeclaration>(node).unwrap();
        let Some(body) = ast.cast::<BlockClassBody>(ast[c].body.raw()) else {
            return;
        };
        let o = self.offset();
        if let Some(dropped) = self.q.target.dropped_token {
            if ast.t_end(dropped) == o && ast.t_is_keyword(dropped) {
                for &m in ast.list_raw(ast[body].members) {
                    let Some(fd) = ast.cast::<FieldDeclaration>(m) else {
                        continue;
                    };
                    let fields = ast[fd].fields;
                    if ast[fields].type_.is_some() {
                        continue;
                    }
                    let variables = ast.list(ast[fields].variables);
                    if variables.len() == 1 {
                        let should_be_type_name = ast[variables[0]].name;
                        let semicolon = ast.t_next(should_be_type_name);
                        if ast.t_ty(semicolon) == TokenType::SEMICOLON && ast.t_next(semicolon) == dropped {
                            self.location("ClassDeclaration_member");
                            let lexeme = ast.t_lexeme(should_be_type_name).to_string();
                            self.ident(false).add_suggestions_from_type_name(&mut self.out, &lexeme);
                            return;
                        }
                    }
                }
            }
        }
        let name_part = ast[c].name_part;
        let type_name = class_name_token(ast, name_part.raw());
        if o == ast.offset(node) {
            self.for_compilation_unit_member_before(node);
        } else if o < ast.t_offset(ast[c].class_keyword) {
            K::add_class_modifiers(self.q, &mut self.out, c);
        } else if o <= ast.t_end(ast[c].class_keyword) {
            self.kw("class");
        } else if o <= ast.t_end(type_name) {
            if o < ast.t_offset(type_name)
                && self.feature(F::PrimaryConstructors)
                && !name_part_has_const(ast, name_part.raw())
            {
                self.kw("const");
            }
            let has_synthetic_body =
                ast.t_synthetic(ast[body].left_bracket) && ast.t_synthetic(ast[body].right_bracket);
            self.ident(false).add_top_level_name(self.q, &mut self.out, has_synthetic_body);
        } else if o <= ast.t_offset(ast[body].left_bracket) {
            K::add_class_declaration_keywords(self.q, &mut self.out, c);
        } else if o >= ast.t_end(ast[body].left_bracket) && o <= ast.t_offset(ast[body].right_bracket) {
            if self.try_annotation_at_end_of_class_body(node) {
                return;
            }
            self.location("ClassDeclaration_member");
            let members = ast.list_raw(ast[body].members).to_vec();
            let preceding = element_before(ast, &members, o);
            let token = preceding
                .map(|p| ast.begin(p))
                .unwrap_or_else(|| ast.t_next(ast[body].left_bracket));
            if ast.t_lexeme(token) == "final" && ast.t_is_keyword(token) {
                self.for_type_annotation(node, TypeOpts::default());
                return;
            }
            self.for_class_member(c);
            if let Some(m) = element_before(ast, &members, o).and_then(|m| ast.cast::<MethodDeclaration>(m)) {
                let body = ast[m].body.raw();
                if function_body_is_empty(ast, body) {
                    K::add_function_body_modifiers(self.q, &mut self.out, Some(body));
                }
            }
        } else {
            self.visit_parent(node);
        }
    }

    fn visit_compilation_unit(&mut self, node: NodeId) {
        let ast = self.ast();
        let unit = ast.cast::<CompilationUnit>(node).unwrap();
        let (before, after) = members_around_offset(ast, unit, self.offset());
        if let Some(b) = before {
            if self.handled_incomplete_preceding_unit_member(unit, b) {
                return;
            }
        }
        self.for_compilation_unit_member(unit, before, after);
    }

    fn visit_constructor_declaration(&mut self, node: NodeId) {
        let ast = self.ast();
        let c = ast.cast::<ConstructorDeclaration>(node).unwrap();
        let o = self.offset();
        let limit = ast[c]
            .type_name
            .map(|t| ast.end(t))
            .or(ast[c].new_keyword.map(|t| ast.t_end(t)))
            .or(ast[c].factory_keyword.map(|t| ast.t_end(t)))
            .unwrap_or_else(|| ast.t_offset(ast.begin(node)));
        if o <= limit {
            self.location("ClassDeclaration_member");
            if let Some(p) = ast.parent(node).and_then(|p| ast.parent(p)) {
                self.for_class_like_member(p);
            }
            return;
        }
        let Some(separator) = ast[c].separator else {
            return;
        };
        let ty = ast.t_ty(separator);
        if ty == TokenType::COLON {
            if o >= ast.t_end(separator) && o <= ast.offset(ast[c].body) {
                self.location("ConstructorDeclaration_initializer");
                self.for_constructor_initializer(c, None);
            }
        } else if ty == TokenType::EQ {
            let Some(e) = self.q.declared_element(node) else {
                return;
            };
            let is_const = elem::is_const_constructor(self.q.ctx, e);
            let q = self.q;
            self.decl(DeclConfig { must_be_constant: is_const, ..Default::default() });
            self.decl
                .as_mut()
                .unwrap()
                .add_possible_redirections_in_library(q, &mut self.out, e, q.library);
        }
    }

    fn visit_constructor_field_initializer(&mut self, node: NodeId) {
        let ast = self.ast();
        let f = ast.cast::<ConstructorFieldInitializer>(node).unwrap();
        let Some(constructor) = ast.parent(node).and_then(|p| ast.cast::<ConstructorDeclaration>(p)) else {
            return;
        };
        if self.offset() <= ast.t_offset(ast[f].equals) {
            self.location("ConstructorDeclaration_initializer");
            self.for_constructor_initializer(constructor, Some(f));
        } else {
            self.location("ConstructorFieldInitializer_expression");
            if ast.n_synthetic(ast[f].field_name.raw()) && ast.t_synthetic(ast[f].equals) {
                if let Some(p) = ast.cast::<PropertyAccess>(ast[f].expression.raw()) {
                    if ast[p].target.is_some_and(|t| ast.is::<ThisExpression>(t.raw())) {
                        if ast.t_synthetic(ast[p].operator) {
                            self.for_constructor_initializer(constructor, Some(f));
                        } else {
                            self.for_redirecting_constructor_invocation(constructor);
                        }
                        return;
                    }
                }
            }
            self.for_expression(node, ExprOpts { must_be_non_void: true, ..Default::default() });
        }
    }

    fn visit_constructor_name(&mut self, node: NodeId) {
        let ast = self.ast();
        let n = ast.cast::<ConstructorName>(node).unwrap();
        let ctx = self.q.ctx;
        let parent = ast.parent(node);
        if parent.is_some_and(|p| ast.is::<ConstructorReference>(p)) {
            if let Some(e) = self.q.element(ast[n].type_.raw()) {
                if e.is::<InterfaceElement>() {
                    let q = self.q;
                    self.decl(DeclConfig { prefer_non_invocation: true, ..Default::default() });
                    self.decl.as_mut().unwrap().add_static_members_of_element(q, &mut self.out, e, false);
                }
            }
            return;
        }
        let ty = self.q.tables.annotation_type.get(ast[n].type_.raw()).copied();
        let Some(ty) = ty.filter(|t| matches!(ctx.ty(*t), TypeKind::Interface { .. })) else {
            return;
        };
        if let Some(fc) = parent.and_then(|p| ast.cast::<ConstructorDeclaration>(p)) {
            if ast[fc].factory_keyword.is_some() && ast[fc].redirected_constructor.map(|r| r.raw()) == Some(node) {
                let exclude = ast[fc].name.map(|n| ast.t_lexeme(n).to_string());
                let q = self.q;
                self.decl(DeclConfig {
                    must_be_constant: ast[fc].const_keyword.is_some(),
                    prefer_non_invocation: true,
                    ..Default::default()
                });
                self.decl
                    .as_mut()
                    .unwrap()
                    .add_constructor_names_for_type(q, &mut self.out, ty, exclude.as_deref());
                return;
            }
        }
        let element = ctx.interface_element(ty).unwrap().raw();
        let q = self.q;
        self.decl(DeclConfig::default());
        self.decl.as_mut().unwrap().add_static_members_of_element(q, &mut self.out, element, true);
    }

    fn visit_constructor_selector(&mut self, node: NodeId) {
        let ast = self.ast();
        self.location("ConstructorSelector_name");
        if !self.feature(F::EnhancedEnums) {
            return;
        }
        let Some(arguments) = ast.parent(node).filter(|p| ast.is::<EnumConstantArguments>(*p)) else {
            return;
        };
        let Some(constant) = ast.parent(arguments).filter(|p| ast.is::<EnumConstantDeclaration>(*p)) else {
            return;
        };
        let Some(declaration) = ast
            .parent(constant)
            .and_then(|p| ast.parent(p))
            .filter(|p| ast.is::<EnumDeclaration>(*p))
        else {
            return;
        };
        let Some(e) = self.q.declared_element(declaration).and_then(|e| e.cast::<InterfaceElement>()) else {
            return;
        };
        let q = self.q;
        self.decl(DeclConfig { suggest_unnamed_as_new: true, ..Default::default() });
        self.decl.as_mut().unwrap().add_constructor_names_for_element(q, &mut self.out, e);
    }

    fn visit_declared_variable_pattern(&mut self, node: NodeId) {
        let ast = self.ast();
        let p = ast.cast::<DeclaredVariablePattern>(node).unwrap();
        let name = ast[p].name;
        let o = self.offset();
        if ast.t_synthetic(name) {
            if ast[p].type_.is_none() && ast[p].keyword.is_none() {
                self.for_type_annotation(node, TypeOpts { must_be_non_void: true, ..Default::default() });
                return;
            }
            self.for_name_in_declared_variable_pattern(node);
            return;
        } else if ast.t_covers(Some(name), o) {
            self.for_name_in_declared_variable_pattern(node);
            return;
        }
        if ast[p].keyword.is_some() {
            let ty = ast[p].type_.map(|t| t.raw());
            if ty.is_none() && o < ast.t_offset(name) {
                self.add_lexical(DeclConfig { must_be_type: true, ..Default::default() }, node);
                return;
            }
            if !type_is_single_identifier(ast, ty) && ast.t_covers(Some(name), o) {
                return;
            }
        }
        let parent = ast.parent(node);
        let parent_has_when = parent
            .and_then(|p| ast.cast::<GuardedPattern>(p))
            .is_some_and(|g| has_when(ast, g));
        if !parent_has_when {
            self.kw("when");
        }
    }

    fn visit_do_statement(&mut self, node: NodeId) {
        let ast = self.ast();
        let d = ast.cast::<DoStatement>(node).unwrap();
        let o = self.offset();
        if o <= ast.t_end(ast[d].do_keyword) {
            self.location("Block_statement");
            self.for_statement(node);
        } else if ast.t_end(ast[d].left_parenthesis) <= o && o <= ast.t_offset(ast[d].right_parenthesis) {
            let condition = ast[d].condition.raw();
            if ast.n_synthetic(condition) || o <= ast.offset(condition) || o == ast.end(condition) {
                self.location("DoStatement_condition");
                self.for_expression(node, ExprOpts { must_be_non_void: true, ..Default::default() });
            }
        }
    }

    /// Dart `_resolveFutureOrType`.
    fn resolve_future_or(&self, t: Option<TypeId>) -> Option<TypeId> {
        let ctx = self.q.ctx;
        let t = t?;
        if ctx.is_dart_async_future_or(t) {
            return ctx.type_arguments(t).first().copied();
        }
        Some(t)
    }

    fn visit_dot_shorthand_constructor_invocation(&mut self, node: NodeId) {
        let ast = self.ast();
        let n = ast.cast::<DotShorthandConstructorInvocation>(node).unwrap();
        let o = self.offset();
        if o >= ast.t_end(ast[n].period) && o <= ast.end(ast[n].constructor_name) {
            let context = self.resolve_future_or(self.context_type_of(node));
            let ctx = self.q.ctx;
            let Some(t) = context.filter(|t| matches!(ctx.ty(*t), TypeKind::Interface { .. })) else {
                return;
            };
            let element = ctx.interface_element(t).unwrap().raw();
            if elem::is_accessible_in(ctx, element, self.q.library) {
                let is_const = dartr_resolver::ast_ext::dot_shorthand_constructor_invocation_is_const(ast, n);
                let unnamed_as_new = self.feature(F::DotShorthands) || self.q.is_replacing_keyword_or_identifier();
                let q = self.q;
                self.decl(DeclConfig {
                    must_be_constant: is_const,
                    suggesting_dot_shorthand: true,
                    suggest_unnamed_as_new: unnamed_as_new,
                    ..Default::default()
                });
                self.decl.as_mut().unwrap().add_constructor_names_for_type(q, &mut self.out, t, None);
            }
        }
    }

    fn visit_dot_shorthand_invocation(&mut self, node: NodeId) {
        let ast = self.ast();
        let n = ast.cast::<DotShorthandInvocation>(node).unwrap();
        let o = self.offset();
        if o >= ast.t_end(ast[n].period) && o <= ast.end(ast[n].member_name) {
            let context = self.resolve_future_or(self.context_type_of(node));
            let ctx = self.q.ctx;
            let Some(element) = context.and_then(|t| ctx.type_element(t)) else {
                return;
            };
            if elem::is_accessible_in(ctx, element, self.q.library) {
                let unnamed_as_new = self.feature(F::DotShorthands) || self.q.is_replacing_keyword_or_identifier();
                let q = self.q;
                self.decl(DeclConfig {
                    must_be_constant: dartr_resolver::ast_ext::in_constant_context(ast, node),
                    suggesting_dot_shorthand: true,
                    suggest_unnamed_as_new: unnamed_as_new,
                    ..Default::default()
                });
                self.decl.as_mut().unwrap().add_static_members_of_element(q, &mut self.out, element, false);
            }
        }
    }

    fn visit_dot_shorthand_property_access(&mut self, node: NodeId) {
        let ast = self.ast();
        let context = self.resolve_future_or(self.context_type_of(node));
        let ctx = self.q.ctx;
        let Some(element) = context.and_then(|t| ctx.type_element(t)) else {
            return;
        };
        if elem::is_accessible_in(ctx, element, self.q.library) {
            let unnamed_as_new = self.feature(F::DotShorthands) || self.q.is_replacing_keyword_or_identifier();
            let prefer_non_invocation = element.is::<InterfaceElement>() && self.q.should_suggest_tear_off(element);
            let q = self.q;
            self.decl(DeclConfig {
                suggesting_dot_shorthand: true,
                must_be_constant: dartr_resolver::ast_ext::in_constant_context(ast, node),
                prefer_non_invocation,
                suggest_unnamed_as_new: unnamed_as_new,
                ..Default::default()
            });
            self.decl.as_mut().unwrap().add_static_members_of_element(q, &mut self.out, element, false);
        }
    }

    fn visit_empty_statement(&mut self, node: NodeId) {
        let ast = self.ast();
        let s = ast.cast::<EmptyStatement>(node).unwrap();
        let parent = ast.parent(node);
        if let Some(b) = parent.and_then(|p| ast.cast::<Block>(p)) {
            self.location("Block_statement");
            let statements = ast.list_raw(ast[b].statements);
            if let Some(index) = statements.iter().position(|x| *x == node) {
                if index > 0 {
                    if let Some(t) = ast.cast::<TryStatement>(statements[index - 1]) {
                        if ast[t].finally_block.is_none() {
                            K::add_try_clause_keywords(&mut self.out, true);
                            if ast.list(ast[t].catch_clauses).is_empty() {
                                return;
                            }
                        }
                    }
                }
            }
        } else if let Some(i) = parent.and_then(|p| ast.cast::<IfStatement>(p)) {
            if ast[i].then_statement.raw() == node {
                self.location("IfStatement_thenStatement");
            } else {
                self.location("IfStatement_elseStatement");
            }
        }
        if self.offset() <= ast.t_offset(ast[s].semicolon) {
            self.for_statement(node);
        }
    }

    fn visit_enum_declaration(&mut self, node: NodeId) {
        let ast = self.ast();
        let e = ast.cast::<EnumDeclaration>(node).unwrap();
        if !self.feature(F::EnhancedEnums) {
            return;
        }
        let o = self.offset();
        if o < ast.t_offset(ast[e].enum_keyword) {
            return;
        }
        if o <= ast.t_end(ast[e].enum_keyword) {
            self.kw("enum");
            return;
        }
        let Some(body) = ast.cast::<BlockEnumBody>(ast[e].body.raw()) else {
            return;
        };
        let type_name = class_name_token(ast, ast[e].name_part.raw());
        if o <= ast.t_end(type_name) {
            let include_body = ast.t_synthetic(ast[body].left_bracket) && ast.t_synthetic(ast[body].right_bracket);
            self.ident(false).add_top_level_name(self.q, &mut self.out, include_body);
            return;
        }
        if o <= ast.t_offset(ast[body].left_bracket) {
            K::add_enum_declaration_keywords(self.q, &mut self.out, e);
            return;
        }
        let right = ast[body].right_bracket;
        if !ast.t_synthetic(right) && o >= ast.t_end(right) {
            return;
        }
        if let Some(semicolon) = ast[body].semicolon {
            if o >= ast.t_end(semicolon) {
                self.location("EnumDeclaration_member");
                self.for_enum_member(node);
            }
        }
    }

    fn visit_expression_statement(&mut self, node: NodeId) {
        let ast = self.ast();
        let s = ast.cast::<ExpressionStatement>(node).unwrap();
        let o = self.offset();
        let parent = ast.parent(node);
        if parent.is_some_and(|p| ast.is::<SwitchPatternCase>(p) || ast.is::<SwitchCase>(p)) {
            self.kw("case");
            self.kw_text("default", ":");
            self.location("SwitchMember_statement");
        } else {
            self.location("Block_statement");
        }
        if self.for_incomplete_preceding_statement(node) {
            let single = is_single_identifier_tokens(ast, ast.begin(node), ast.end_tok(node));
            if single && preceding_statement(ast, node).is_some_and(|p| ast.is::<TryStatement>(p)) {
                return;
            }
        }
        if let Some(semicolon) = ast[s].semicolon {
            if !ast.t_synthetic(semicolon) && o >= ast.t_end(semicolon) {
                self.for_statement(node);
                return;
            }
        }
        let expression = ast[s].expression.raw();
        match ast.kind(expression) {
            NodeKind::AsExpression | NodeKind::IsExpression => self.visit(expression),
            NodeKind::AssignmentExpression => {
                let a = ast.cast::<AssignmentExpression>(expression).unwrap();
                let lhs = ast[a].left_hand_side.raw();
                if o <= ast.end(lhs) {
                    if ast.is::<PrefixedIdentifier>(lhs) {
                        self.visit(lhs);
                    } else if ast.is::<SimpleIdentifier>(lhs) {
                        self.for_statement(node);
                    }
                }
            }
            NodeKind::CascadeExpression => {
                let c = ast.cast::<CascadeExpression>(expression).unwrap();
                if o <= ast.end(ast[c].target) {
                    let cfg = DeclConfig {
                        must_be_non_void: true,
                        must_be_static: in_static_context(ast, node),
                        ..Default::default()
                    };
                    self.add_lexical(cfg, node);
                }
            }
            NodeKind::InstanceCreationExpression | NodeKind::MethodInvocation => {
                if o <= ast.t_end(ast.begin(expression)) {
                    self.for_statement(node);
                }
            }
            NodeKind::FunctionReference => {
                if o > ast.end(expression) {
                    let f = ast.cast::<FunctionReference>(expression).unwrap();
                    if let Some(i) = ast.cast::<SimpleIdentifier>(ast[f].function.raw()) {
                        let name = ast.t_lexeme(ast[i].token).to_string();
                        self.ident(false).add_suggestions_from_type_name(&mut self.out, &name);
                    }
                }
            }
            NodeKind::TypeLiteral => {
                if o <= ast.end(expression) {
                    self.for_statement(node);
                } else {
                    let t = ast.cast::<TypeLiteral>(expression).unwrap();
                    let name = ast.t_lexeme(ast[ast[t].type_].name).to_string();
                    self.ident(false).add_suggestions_from_type_name(&mut self.out, &name);
                }
            }
            NodeKind::PrefixedIdentifier => {
                let p = ast.cast::<PrefixedIdentifier>(expression).unwrap();
                if o <= ast.end(ast[p].prefix) {
                    let cfg = DeclConfig {
                        must_be_non_void: true,
                        must_be_static: in_static_context(ast, node),
                        ..Default::default()
                    };
                    self.add_lexical(cfg, node);
                } else if o <= ast.end(ast[p].identifier) {
                } else {
                    let name = ast.t_lexeme(ast[ast[p].identifier].token).to_string();
                    self.ident(false).add_suggestions_from_type_name(&mut self.out, &name);
                }
            }
            NodeKind::SimpleIdentifier => {
                if o <= ast.end(expression) {
                    self.for_statement(node);
                } else {
                    let i = ast.cast::<SimpleIdentifier>(expression).unwrap();
                    let name = ast.t_lexeme(ast[i].token).to_string();
                    self.ident(false).add_suggestions_from_type_name(&mut self.out, &name);
                }
            }
            _ => self.for_expression(node, ExprOpts::default()),
        }
    }

    fn visit_extension_declaration(&mut self, node: NodeId) {
        let ast = self.ast();
        let e = ast.cast::<ExtensionDeclaration>(node).unwrap();
        let o = self.offset();
        if o == ast.offset(node) {
            self.for_compilation_unit_member_before(node);
            return;
        }
        let doc = ast.children(node).into_iter().find(|c| ast.is::<Comment>(*c));
        if ast.n_covers(doc, o) {
            return;
        }
        if o < ast.t_offset(ast[e].extension_keyword) {
            return;
        }
        if o <= ast.t_end(ast[e].extension_keyword) {
            self.kw("extension");
            return;
        }
        if let Some(name) = ast[e].name {
            if o <= ast.t_end(name) {
                self.kw("on");
                if self.feature(F::InlineClass) {
                    K::add_text(&mut self.out, "type");
                }
                self.ident(false).add_top_level_name(self.q, &mut self.out, false);
                return;
            }
        }
        self.ident(false).add_top_level_name(self.q, &mut self.out, false);
        if let Some(body) = ast.cast::<BlockClassBody>(ast[e].body.raw()) {
            if o <= ast.t_offset(ast[body].left_bracket) {
                self.location("ExtensionDeclaration_onClause");
                if let Some(on) = ast[e].on_clause {
                    if ast.t_synthetic(ast[on].on_keyword) {
                        K::add_extension_declaration_keywords(self.q, &mut self.out, e);
                    }
                }
                return;
            }
            if o >= ast.t_end(ast[body].left_bracket) && o <= ast.t_offset(ast[body].right_bracket) {
                self.location("ExtensionDeclaration_member");
                self.for_extension_member(node);
            }
        }
    }

    fn visit_extension_type_declaration(&mut self, node: NodeId) {
        let ast = self.ast();
        let e = ast.cast::<ExtensionTypeDeclaration>(node).unwrap();
        let Some(body) = ast.cast::<BlockClassBody>(ast[e].body.raw()) else {
            return;
        };
        let o = self.offset();
        let name_part = ast[e].name_part.raw();
        let type_name = class_name_token(ast, name_part);
        if o == ast.offset(node) {
            self.for_compilation_unit_member_before(node);
        } else if o <= ast.t_end(type_name) {
            if o < ast.t_offset(type_name)
                && self.feature(F::PrimaryConstructors)
                && ast.is::<PrimaryConstructorDeclaration>(name_part)
                && !name_part_has_const(ast, name_part)
            {
                self.kw("const");
            }
            let include_body = ast.t_synthetic(ast[body].left_bracket) && ast.t_synthetic(ast[body].right_bracket);
            self.ident(false).add_top_level_name(self.q, &mut self.out, include_body);
        } else if o >= ast.end(name_part)
            && (o <= ast.t_offset(ast[body].left_bracket) || ast.t_synthetic(ast[body].left_bracket))
        {
            self.kw("implements");
        } else if o >= ast.t_end(ast[body].left_bracket) && o <= ast.t_offset(ast[body].right_bracket) {
            self.location("ExtensionTypeDeclaration_member");
            self.for_extension_type_member(node);
        }
    }

    fn visit_field_declaration(&mut self, node: NodeId) {
        let ast = self.ast();
        let f = ast.cast::<FieldDeclaration>(node).unwrap();
        let o = self.offset();
        self.for_incomplete_preceding_class_member(node);
        let first_token = ast[f].first_token_after_comment_and_metadata(ast);
        if o <= ast.t_end(first_token) {
            self.location("ClassDeclaration_member");
        }
        let fields = ast[f].fields;
        match ast[fields].type_ {
            None => {
                let variables = ast.list(ast[fields].variables).to_vec();
                if let Some(&first) = variables.first() {
                    self.location("FieldDeclaration_fields");
                    let name = ast[first].name;
                    if variables.len() == 1 && ast.t_is_keyword(name) && o > ast.t_end(name) {
                        let keyword = ast.t_lexeme(name).to_string();
                        K::add_field_declaration_keywords(self.q, &mut self.out, f, Some(&keyword));
                        self.add_lexical(DeclConfig { must_be_type: true, ..Default::default() }, node);
                    } else if o < ast.t_offset(name) {
                        K::add_field_declaration_keywords(self.q, &mut self.out, f, None);
                        self.for_type_annotation(
                            node,
                            TypeOpts {
                                must_be_non_void: ast[first].equals.is_some(),
                                is_in_declaration: true,
                                ..Default::default()
                            },
                        );
                    } else if o <= ast.t_end(name) {
                        K::add_field_declaration_keywords(self.q, &mut self.out, f, None);
                    }
                }
            }
            Some(ty) => {
                let preceding = preceding_member(ast, node);
                if o <= ast.offset(ty) && preceding.is_none_or(|p| o >= ast.end(p)) {
                    if let Some(p) = ast.parent(node).and_then(|p| ast.parent(p)) {
                        self.for_class_like_member(p);
                    }
                } else if o <= ast.end(ty) {
                    let single = is_single_identifier_tokens(ast, first_token, ast.end_tok(node));
                    if single {
                        if let Some(p) = ast.parent(node) {
                            let p = if ast.is::<BlockClassBody>(p) || ast.is::<BlockEnumBody>(p) {
                                ast.parent(p).unwrap_or(p)
                            } else {
                                p
                            };
                            self.for_class_like_member(p);
                        }
                    } else {
                        K::add_field_declaration_keywords(self.q, &mut self.out, f, None);
                        self.location("ClassDeclaration_member");
                        self.kw("var");
                        self.add_lexical(DeclConfig { must_be_type: true, ..Default::default() }, node);
                    }
                }
            }
        }
    }

    fn visit_field_formal_parameter(&mut self, node: NodeId) {
        let ast = self.ast();
        let p = ast.cast::<FieldFormalParameter>(node).unwrap();
        let mut constructor = ast.parent(node).and_then(|n| ast.parent(n));
        if constructor.is_some_and(|c| ast.is::<FormalParameterList>(c)) {
            constructor = constructor.and_then(|c| ast.parent(c));
        }
        let Some(constructor) = constructor.and_then(|c| ast.cast::<ConstructorDeclaration>(c)) else {
            return;
        };
        let field = self
            .q
            .declared_element(node)
            .and_then(|e| e.cast::<dartr_element::FormalParameterElement>())
            .and_then(|fp| self.q.ctx.get(fp).field.get())
            .map(|f| f.raw());
        let o = self.offset();
        let this = ast[p].this_keyword;
        if ast.t_offset(this) >= o && ast[p].required_keyword.is_none() {
            self.kw("required");
        }
        if ast.t_offset(this) >= o && ast[p].required_keyword.is_none_or(|r| ast.t_end(r) <= o) {
            self.add_lexical(DeclConfig { must_be_type: true, ..Default::default() }, node);
        }
        if ast.t_end(ast[p].period) <= o {
            let q = self.q;
            self.decl(DeclConfig::default());
            self.decl.as_mut().unwrap().add_fields_for_initializers(q, &mut self.out, constructor, field);
        }
    }

    fn visit_formal_parameter_list(&mut self, node: NodeId) {
        let ast = self.ast();
        let list = ast.cast::<FormalParameterList>(node).unwrap();
        if let Some(p) = ast.parent(node).filter(|p| ast.is::<PrimaryConstructorDeclaration>(*p)) {
            self.visit(p);
            return;
        }
        if self.offset() >= ast.end(node) {
            if let Some(f) = ast.parent(node).filter(|p| ast.is::<FunctionExpression>(*p)) {
                self.visit_function_expression(f);
                return;
            }
        }
        self.location("FormalParameterList_parameter");
        let parameters = ast.list_raw(ast[list].parameters).to_vec();
        if let Some(preceding) = element_before(ast, &parameters, self.offset()) {
            if parameter_is_incomplete(ast, preceding) {
                self.visit(preceding);
                return;
            }
            if let Some(r) = ast.cast::<RegularFormalParameter>(preceding) {
                if ast[r].type_.is_none() && ast[r].function_typed_suffix.is_none() && self.offset() > ast.end(preceding) {
                    if let Some(name) = ast[r].name {
                        let name = ast.t_lexeme(name).to_string();
                        self.ident(false).add_suggestions_from_type_name(&mut self.out, &name);
                    }
                }
            }
        }
        K::add_formal_parameter_keywords(self.q, &mut self.out, Some(list), true, true, true, true, true);
        self.for_type_annotation(node, TypeOpts::default());
    }

    fn visit_for_parts_with_declarations(&mut self, node: NodeId) {
        let ast = self.ast();
        let p = ast.cast::<ForPartsWithDeclarations>(node).unwrap();
        let o = self.offset();
        let left = ast[p].left_separator;
        let right = ast[p].right_separator;
        if o >= ast.t_end(left) && o <= ast.t_offset(right) {
            self.location("ForParts_condition");
            if ast[p].condition.is_some_and(|c| ast.is::<SimpleIdentifier>(c.raw()))
                && ast.t_synthetic(left)
                && ast.t_synthetic(right)
            {
                self.kw("in");
                return;
            }
            self.for_expression(node, ExprOpts::default());
        } else if o >= ast.t_end(right) {
            self.location("ForParts_updater");
            self.for_expression(node, ExprOpts::default());
        }
    }

    fn visit_for_statement(&mut self, node: NodeId) {
        let ast = self.ast();
        let f = ast.cast::<ForStatement>(node).unwrap();
        let o = self.offset();
        if o <= ast.t_end(ast[f].for_keyword) {
            self.location("Block_statement");
            self.for_statement(node);
        } else if o >= ast.t_end(ast[f].left_parenthesis) && o <= ast.t_offset(ast[f].right_parenthesis) {
            self.location("ForStatement_forLoopParts");
            let parts = ast[f].for_loop_parts.raw();
            match ast.kind(parts) {
                NodeKind::ForEachPartsWithDeclaration => {
                    let p = ast.cast::<ForEachPartsWithDeclaration>(parts).unwrap();
                    let variable = ast[p].loop_variable;
                    if o < ast.t_offset(ast[variable].name) {
                        let ty = ast[variable].type_;
                        let ok = match ty {
                            None => true,
                            Some(t) => ast
                                .cast::<NamedType>(t.raw())
                                .is_some_and(|n| o <= ast.t_end(ast[n].name)),
                        };
                        if ok {
                            self.for_type_annotation(node, TypeOpts::default());
                        }
                    }
                }
                NodeKind::ForEachPartsWithIdentifier => {
                    let p = ast.cast::<ForEachPartsWithIdentifier>(parts).unwrap();
                    if o < ast.offset(ast[p].identifier) {
                        self.for_type_annotation(node, TypeOpts::default());
                    }
                }
                NodeKind::ForPartsWithDeclarations => {
                    let p = ast.cast::<ForPartsWithDeclarations>(parts).unwrap();
                    let variables = ast[p].variables;
                    let vars = ast.list(ast[variables].variables);
                    if let Some(keyword) = ast[variables].keyword {
                        if vars.len() == 1 && ast.t_synthetic(ast[vars[0]].name) && ast.t_synthetic(ast[p].left_separator) {
                            let after = ast.t_next(keyword);
                            if ast.t_ty(after) == TokenType::OPEN_PAREN {
                                let end_group = ast.tokens.get(after).end_group;
                                if let Some(end) = end_group.get() {
                                    if o >= ast.t_end(end) {
                                        self.kw("in");
                                    }
                                }
                            }
                        }
                    }
                    if let Some(t) = ast[variables].type_ {
                        if ast.n_covers(Some(t.raw()), o) {
                            self.visit(t.raw());
                        }
                    }
                }
                NodeKind::ForPartsWithExpression => {
                    let p = ast.cast::<ForPartsWithExpression>(parts).unwrap();
                    if ast.t_synthetic(ast[p].left_separator)
                        && ast[p].initialization.is_some_and(|i| ast.is::<SimpleIdentifier>(i.raw()))
                    {
                        self.kw("final");
                        self.kw("var");
                        self.for_type_annotation(node, TypeOpts::default());
                    }
                }
                _ => {}
            }
        } else {
            self.location("ForStatement_body");
            self.for_statement(node);
        }
    }

    fn visit_function_declaration(&mut self, node: NodeId) {
        let ast = self.ast();
        let f = ast.cast::<FunctionDeclaration>(node).unwrap();
        if self.offset() == ast.offset(node) {
            self.for_compilation_unit_member_before(node);
        }
        let return_type = ast[f].return_type.map(|r| r.raw());
        let simple = return_type.is_none_or(|r| ast.begin(r) == ast.end_tok(r));
        if simple && self.offset() <= ast.t_offset(ast[f].name) {
            self.location("FunctionDeclaration_returnType");
            self.for_type_annotation(node, TypeOpts::default());
        }
    }

    fn visit_function_expression(&mut self, node: NodeId) {
        let ast = self.ast();
        let f = ast.cast::<FunctionExpression>(node).unwrap();
        let start = ast[f]
            .parameters
            .map(|p| ast.end(p))
            .or(ast[f].type_parameters.map(|p| ast.end(p)))
            .unwrap_or_else(|| ast.offset(node));
        let body = ast[f].body.raw();
        if self.offset() >= start && self.offset() <= ast.offset(body) {
            K::add_function_body_modifiers(self.q, &mut self.out, Some(body));
            let grand_parent = ast.parent(node);
            let unit = grand_parent.and_then(|g| ast.parent(g));
            if ast.is::<EmptyFunctionBody>(body)
                && grand_parent.is_some_and(|g| ast.is::<FunctionDeclaration>(g))
            {
                if let Some(u) = unit.and_then(|u| ast.cast::<CompilationUnit>(u)) {
                    self.for_compilation_unit_declaration(u);
                }
            }
        }
    }

    fn visit_function_typed_suffix(&mut self, node: NodeId) {
        let ast = self.ast();
        self.location("FormalParameterList_parameter");
        let Some(parent) = ast.parent(node).filter(|p| ast.is::<FormalParameter>(*p)) else {
            return;
        };
        let (return_type, required, name) = if let Some(r) = ast.cast::<RegularFormalParameter>(parent) {
            (ast[r].type_.map(|t| t.raw()), ast[r].required_keyword, ast[r].name)
        } else if let Some(r) = ast.cast::<FieldFormalParameter>(parent) {
            (ast[r].type_.map(|t| t.raw()), ast[r].required_keyword, Some(ast[r].name))
        } else if let Some(r) = ast.cast::<SuperFormalParameter>(parent) {
            (ast[r].type_.map(|t| t.raw()), ast[r].required_keyword, Some(ast[r].name))
        } else {
            return;
        };
        match return_type {
            Some(rt) if self.offset() <= ast.end(rt) => {
                let suggest_variable_name = name.is_some_and(|n| ast.t_lexeme(n).is_empty());
                K::add_formal_parameter_keywords(
                    self.q,
                    &mut self.out,
                    parent_parameter_list(ast, parent),
                    required.is_none(),
                    suggest_variable_name,
                    true,
                    true,
                    true,
                );
                self.for_type_annotation(parent, TypeOpts::default());
            }
            None => {
                if let Some(n) = name {
                    if self.offset() < ast.t_offset(n) {
                        self.for_type_annotation(parent, TypeOpts::default());
                    }
                }
            }
            _ => {}
        }
    }

    fn visit_if_element(&mut self, node: NodeId) {
        let ast = self.ast();
        let i = ast.cast::<IfElement>(node).unwrap();
        let o = self.offset();
        let expression = ast[i].expression.raw();
        let right = ast[i].right_parenthesis;
        if o > ast.end(expression) && o <= ast.t_offset(right) {
            match ast[i].case_clause {
                None => {
                    self.kw("case");
                    self.kw("is");
                }
                Some(c) => {
                    let g = ast[c].guarded_pattern;
                    if has_when(ast, g) {
                        let has_expression = ast[g].when_clause.is_some();
                        if !has_expression {
                            let must_be_static = in_static_context(ast, node);
                            K::add_expression_keywords(self.q, &mut self.out, Some(node), true, true, true, false, must_be_static);
                        }
                    } else {
                        self.kw("when");
                    }
                }
            }
        } else if o >= ast.t_end(ast[i].left_parenthesis) && o <= ast.t_offset(right) {
            self.location("IfElement_condition");
            self.for_expression(node, ExprOpts { must_be_non_void: true, ..Default::default() });
        } else if o >= ast.t_end(right) {
            match ast[i].else_keyword {
                Some(e) if o > ast.t_offset(e) => self.location("IfElement_elseElement"),
                _ => self.location("IfElement_thenElement"),
            }
            self.for_enclosing_collection(node);
        }
    }

    fn visit_if_statement(&mut self, node: NodeId) {
        let ast = self.ast();
        let i = ast.cast::<IfStatement>(node).unwrap();
        let o = self.offset();
        if ast.t_synthetic(ast[i].right_parenthesis) && !ast.t_synthetic(ast[i].left_parenthesis) {
            self.kw("is");
            return;
        }
        let expression = ast[i].expression.raw();
        let right = ast[i].right_parenthesis;
        if o <= ast.t_end(ast[i].if_keyword) {
            self.location("Block_statement");
            self.for_statement(node);
        } else if o > ast.end(expression) && o <= ast.t_offset(right) {
            self.location("IfStatement_condition");
            match ast[i].case_clause {
                None => {
                    self.kw("case");
                    self.kw("is");
                }
                Some(c) => {
                    let g = ast[c].guarded_pattern;
                    if has_when(ast, g) {
                        if ast[g].when_clause.is_none() {
                            self.for_expression(node, ExprOpts::default());
                        }
                    } else {
                        self.kw("when");
                        let pattern = ast[g].pattern.raw();
                        if let Some(cp) = ast.cast::<ConstantPattern>(pattern) {
                            if let Some(tl) = ast.cast::<TypeLiteral>(ast[cp].expression.raw()) {
                                let nt = ast[tl].type_;
                                if ast.end(nt) < o {
                                    let name = ast.t_lexeme(ast[nt].name).to_string();
                                    self.ident(false).add_suggestions_from_type_name(&mut self.out, &name);
                                }
                            }
                        }
                    }
                }
            }
        } else if o >= ast.t_end(ast[i].left_parenthesis) && o <= ast.t_offset(right) {
            self.location("IfStatement_condition");
            self.for_expression(node, ExprOpts { must_be_non_void: true, ..Default::default() });
        } else if o >= ast.t_end(right) {
            match ast[i].else_keyword {
                Some(e) if o > ast.t_offset(e) => self.location("IfStatement_elseStatement"),
                _ => self.location("IfStatement_thenStatement"),
            }
            self.for_statement(node);
        }
    }

    fn visit_import_directive(&mut self, node: NodeId) {
        let ast = self.ast();
        let d = ast.cast::<ImportDirective>(node).unwrap();
        let o = self.offset();
        if o <= ast.t_end(ast[d].import_keyword) {
            self.location("CompilationUnit_directive");
            self.for_compilation_unit_member_before(node);
        } else if o <= ast.offset(ast[d].uri) {
        } else if o >= ast.end(ast[d].uri) {
            self.location("CompilationUnit_directive");
            K::add_import_directive_keywords(self.q, &mut self.out, d);
        }
    }

    fn visit_import_prefix_reference(&mut self, node: NodeId) {
        let ast = self.ast();
        let ctx = self.q.ctx;
        let Some(parent) = ast.parent(node).and_then(|p| ast.cast::<NamedType>(p)) else {
            return;
        };
        if self.offset() > ast.t_offset(ast[parent].name) {
            return;
        }
        let element = self.q.tables.element.get(node).copied();
        self.location("PropertyAccess_propertyName");
        let Some(element) = element else {
            return;
        };
        let base = member::base_element(ctx, element);
        let ty = match base.tag() {
            Tag::Getter => Some(member::return_type(ctx, element)),
            Tag::Method | Tag::TopLevelFunction | Tag::Constructor | Tag::LocalFunction | Tag::Setter => {
                Some(member::type_(ctx, element))
            }
            Tag::Prefix => {
                let is_instance_creation = ast
                    .parent(parent)
                    .and_then(|p| ast.parent(p))
                    .is_some_and(|p| ast.is::<InstanceCreationExpression>(p));
                let q = self.q;
                self.decl(DeclConfig {
                    exclude_type_names: is_instance_creation,
                    must_be_type: !is_instance_creation,
                    must_be_non_void: is_instance_creation,
                    ..Default::default()
                });
                self.decl.as_mut().unwrap().add_declarations_through_import_prefix(q, &mut self.out, base);
                return;
            }
            _ if base.is::<dartr_element::VariableElement>() => Some(member::type_(ctx, element)),
            _ => {
                if base.is::<InterfaceElement>() || base.tag() == Tag::Extension {
                    let q = self.q;
                    self.decl(DeclConfig::default());
                    self.decl.as_mut().unwrap().add_static_members_of_element(q, &mut self.out, base, false);
                }
                return;
            }
        };
        if let Some(t) = ty {
            let q = self.q;
            self.decl(DeclConfig::default());
            self.decl.as_mut().unwrap().add_instance_members_of_type(q, &mut self.out, t, false);
        }
    }

    fn visit_instance_creation(&mut self, node: NodeId) {
        let ast = self.ast();
        let i = ast.cast::<InstanceCreationExpression>(node).unwrap();
        match ast[i].keyword {
            Some(k) if self.offset() > ast.t_end(k) => {
                let name = ast[i].constructor_name.raw();
                if ast.n_synthetic(name) || self.offset() < ast.offset(name) || ast.n_covers(Some(name), self.offset()) {
                    self.location("InstanceCreationExpression_constructorName");
                    let q = self.q;
                    self.decl(DeclConfig::default());
                    let d = self.decl.as_mut().unwrap();
                    d.add_constructor_invocations(q, &mut self.out);
                    d.add_import_prefixes(q, &mut self.out);
                }
            }
            _ => self.for_expression(node, ExprOpts::default()),
        }
    }

    fn visit_is_expression(&mut self, node: NodeId) {
        let ast = self.ast();
        let e = ast.cast::<IsExpression>(node).unwrap();
        let o = self.offset();
        let op = ast[e].is_operator;
        if ast.n_synthetic(ast[e].expression.raw()) && ast.n_synthetic(ast[e].type_.raw()) && ast.t_end(op) == o {
            let cfg = DeclConfig { must_be_static: in_static_context(ast, node), ..Default::default() };
            self.add_lexical(cfg, node);
            return;
        }
        if ast.t_covers(Some(op), o) {
            self.kw("is");
        } else if o < ast.t_offset(op) {
            self.for_expression(node, ExprOpts::default());
        } else if o > ast.t_end(op) {
            self.location("IsExpression_type");
            self.add_lexical(DeclConfig { must_be_type: true, ..Default::default() }, node);
        }
    }

    fn visit_map_literal_entry(&mut self, node: NodeId) {
        let ast = self.ast();
        let e = ast.cast::<MapLiteralEntry>(node).unwrap();
        let o = self.offset();
        if o == ast.offset(node) {
            self.visit_parent(node);
        } else if ast[e].key_question.is_some() && o == ast.offset(ast[e].key) {
            self.visit_parent(node);
            self.location("NullAwareElement_value");
        } else if ast[e].value_question.is_some_and(|vq| {
            o == ast.t_end(ast[e].separator) || o == ast.t_offset(vq) || o == ast.offset(ast[e].value)
        }) {
            self.visit_parent(node);
            self.location("NullAwareElement_value");
        } else if o >= ast.t_end(ast[e].separator) {
            self.location("MapLiteralEntry_value");
            self.for_expression(node, ExprOpts { must_be_non_void: true, ..Default::default() });
        }
    }

    fn visit_method_declaration(&mut self, node: NodeId) {
        let ast = self.ast();
        let m = ast.cast::<MethodDeclaration>(node).unwrap();
        let o = self.offset();
        self.location("ClassDeclaration_member");
        let first = ast[m].first_token_after_comment_and_metadata(ast);
        let before_first = ast.t_prev(first).map(|t| ast.t_offset(t)).unwrap_or(0);
        if o >= before_first && o <= ast.t_end(ast[m].name) {
            self.for_type_annotation(node, TypeOpts { is_in_declaration: true, ..Default::default() });
            K::add_class_member_keywords(self.q, &mut self.out);
        }
        let body = ast[m].body.raw();
        let Some(token_before_body) = ast.t_prev(ast.begin(body)) else {
            return;
        };
        if o >= ast.t_end(token_before_body) && o <= ast.offset(body) {
            if super::keyword::function_body_keyword(ast, body).is_none() {
                K::add_function_body_modifiers(self.q, &mut self.out, Some(body));
            }
            if function_body_is_empty(ast, body) {
                K::add_class_member_keywords(self.q, &mut self.out);
            }
        }
    }

    fn visit_method_invocation(&mut self, node: NodeId) {
        let ast = self.ast();
        let m = ast.cast::<MethodInvocation>(node).unwrap();
        let o = self.offset();
        let begin = ast.begin(node);
        if o <= ast.t_end(begin) && ast.t_kw_or_ident(begin) {
            self.visit_parent(node);
            return;
        }
        let Some(operator) = ast[m].operator else {
            if ast.n_covers(Some(node), o) {
                let parent = ast.parent(node);
                let mut must_be_non_void = false;
                if let Some(l) = parent.and_then(|p| ast.cast::<ArgumentList>(p)) {
                    let location = self.location_for(l, false);
                    self.location(&location);
                    must_be_non_void = true;
                } else if let Some(p) = parent.filter(|p| ast.is::<NamedArgument>(*p)) {
                    if let Some(l) = ast.parent(p).and_then(|g| ast.cast::<ArgumentList>(g)) {
                        let location = self.location_for(l, true);
                        self.location(&location);
                    }
                    must_be_non_void = true;
                } else if parent.is_some_and(|p| ast.is::<RecordLiteral>(p)) {
                    self.location("RecordLiteral_fields");
                    must_be_non_void = true;
                }
                self.for_expression(node, ExprOpts { must_be_non_void, ..Default::default() });
            }
            return;
        };
        let is_cascaded = dartr_resolver::ast_ext::method_invocation_is_cascaded(ast, m);
        if (is_cascaded && o + 1 == ast.t_end(operator))
            || (o >= ast.t_end(operator) && o <= ast.end(ast[m].method_name))
        {
            let target = super::target::real_target_of_method_invocation(ast, m);
            let ty = target.and_then(|t| self.q.tables.static_type.get(t).copied());
            if let Some(t) = ty {
                self.for_member_access(node, t, false);
            }
            let element = target.and_then(|t| self.static_member_target_element(t));
            let ctx = self.q.ctx;
            let type_ok = ty.is_none_or(|t| matches!(ctx.ty(t), TypeKind::Invalid) || ctx.is_dart_core_type(t));
            if type_ok && element.is_some() && (!is_cascaded || o + 1 == ast.t_end(operator)) {
                let element = element.unwrap();
                if element.is::<InterfaceElement>() {
                    let q = self.q;
                    self.decl(DeclConfig::default());
                    self.decl.as_mut().unwrap().add_static_members_of_element(q, &mut self.out, element, false);
                }
                if element.tag() == Tag::Prefix {
                    let q = self.q;
                    self.decl(DeclConfig::default());
                    self.decl.as_mut().unwrap().add_declarations_through_import_prefix(q, &mut self.out, element);
                }
            }
        }
    }

    fn visit_mixin_declaration(&mut self, node: NodeId) {
        let ast = self.ast();
        let m = ast.cast::<MixinDeclaration>(node).unwrap();
        let o = self.offset();
        if o == ast.offset(node) {
            self.for_compilation_unit_member_before(node);
            return;
        }
        if o < ast.t_offset(ast[m].mixin_keyword) {
            K::add_mixin_modifiers(self.q, &mut self.out, m);
            return;
        }
        if o <= ast.t_end(ast[m].mixin_keyword) {
            self.kw("mixin");
            return;
        }
        let Some(body) = ast.cast::<BlockClassBody>(ast[m].body.raw()) else {
            return;
        };
        if o <= ast.t_end(ast[m].name) {
            let include_body = ast.t_synthetic(ast[body].left_bracket) && ast.t_synthetic(ast[body].right_bracket);
            self.ident(false).add_top_level_name(self.q, &mut self.out, include_body);
            return;
        }
        if o <= ast.t_offset(ast[body].left_bracket) {
            K::add_mixin_declaration_keywords(self.q, &mut self.out, m);
            return;
        }
        if o >= ast.t_end(ast[body].left_bracket) && o <= ast.t_offset(ast[body].right_bracket) {
            self.location("MixinDeclaration_member");
            if self.try_annotation_at_end_of_class_body(node) {
                return;
            }
            self.for_mixin_member(m);
            let members = ast.list_raw(ast[body].members).to_vec();
            if let Some(md) = element_before(ast, &members, o).and_then(|e| ast.cast::<MethodDeclaration>(e)) {
                let b = ast[md].body.raw();
                if function_body_is_empty(ast, b) {
                    K::add_function_body_modifiers(self.q, &mut self.out, Some(b));
                }
            }
        }
    }

    fn visit_named_argument(&mut self, node: NodeId) {
        let ast = self.ast();
        let n = ast.cast::<NamedArgument>(node).unwrap();
        let o = self.offset();
        if o <= ast.t_end(ast[n].name) {
            let parent = ast.parent(node);
            if let Some(list) = parent.and_then(|p| ast.cast::<ArgumentList>(p)) {
                let location = self.location_for(list, true);
                self.location(&location);
                if let Some(parameters) = invoked_formal_parameters(self.q, list) {
                    let (_, mut used) = argument_context(ast, list, -1);
                    let own = ast.t_lexeme(ast[n].name).to_string();
                    used.retain(|u| *u != own);
                    let append_colon = ast.t_synthetic(ast[n].colon);
                    for p in parameters {
                        if !p.kind.is_named() || p.name.as_ref().is_some_and(|nm| used.contains(nm)) {
                            continue;
                        }
                        let Some(element) = p.element else {
                            continue;
                        };
                        let ctx = self.q.ctx;
                        let name = super::candidate::display_name(ctx, member::base_element(ctx, element));
                        let score = self.out.score(&name);
                        if score != -1.0 {
                            let is_widget = is_flutter_widget_parameter(ctx, element);
                            self.out.add(Candidate::new(
                                Kind::NamedArgument {
                                    parameter: element,
                                    append_colon,
                                    append_comma: false,
                                    replacement_length: None,
                                    is_widget,
                                    quote: self.q.style.quote,
                                },
                                score,
                            ));
                        }
                    }
                }
            } else if let Some(r) = parent.filter(|p| ast.is::<RecordLiteral>(*p)) {
                self.location("RecordLiteral_fields");
                let context = self.context_type_of(r);
                self.suggest_record_literal_named_fields(context, node, Some(r), false);
            }
        } else if o >= ast.t_end(ast[n].colon) {
            let in_argument_list = ast.parent(node).is_some_and(|p| ast.is::<ArgumentList>(p));
            if in_argument_list {
                self.location("ArgumentList_method_named");
            }
            let ctx = self.q.ctx;
            let parameter = dartr_resolver::error::support::corresponding_parameter(ctx, ast, self.q.tables, node);
            let param_ref = self
                .q
                .tables
                .param_element
                .get(ast[n].argument_expression.raw())
                .or_else(|| self.q.tables.param_element.get(node))
                .copied();
            let mut ty = match (param_ref, parameter) {
                (Some(r), _) => member::type_(ctx, r),
                _ => TypeId::DYNAMIC,
            };
            while let TypeKind::TypeParameter { .. } = ctx.ty(ty) {
                let bound = ctx.type_parameter_type_bound(ty);
                if bound == ty {
                    break;
                }
                ty = bound;
            }
            if ctx.is_dart_core_function(ty) {
                ty = self.void_function_no_parameters();
            }
            let is_function = matches!(ctx.ty(ty), TypeKind::Function(_));
            let can_be_null = self.can_be_null(ty);
            let can_be_bool = self.can_be_bool(ty);
            self.for_expression(
                ast[n].argument_expression.raw(),
                ExprOpts {
                    must_be_non_void: in_argument_list,
                    can_be_null,
                    can_be_bool,
                    include_trailing_comma_after_closure: !is_followed_by_comma(ast, node),
                    can_suggest_const: !is_function,
                    ..Default::default()
                },
            );
        }
    }

    fn visit_named_type(&mut self, node: NodeId) {
        let ast = self.ast();
        let n = ast.cast::<NamedType>(node).unwrap();
        let prefix_element = ast[n]
            .import_prefix
            .and_then(|p| self.q.element(p.raw()))
            .filter(|e| e.tag() == Tag::Prefix);
        if let Some(prefix) = prefix_element {
            if let Some(list) = ast.parent(node).filter(|p| ast.is::<VariableDeclarationList>(*p)) {
                if let Some(s) = ast.parent(list).and_then(|p| ast.cast::<VariableDeclarationStatement>(p)) {
                    if ast.t_synthetic(ast[s].semicolon) {
                        let q = self.q;
                        self.decl(DeclConfig::default());
                        self.decl.as_mut().unwrap().add_declarations_through_import_prefix(q, &mut self.out, prefix);
                        return;
                    }
                }
            }
        }
        let parent = ast.parent(node);
        if parent.is_some_and(|p| ast.is::<ImplementsClause>(p)) {
            self.location("ImplementsClause_interface");
        } else if parent.is_some_and(|p| ast.is::<TypeArgumentList>(p)) {
            self.location("TypeArgumentList_argument");
        } else if parent.is_some_and(|p| ast.is::<WithClause>(p)) {
            self.location("WithClause_mixinType");
        }
        let exclude = parent
            .and_then(|p| ast.parent(p))
            .is_some_and(|g| ast.is::<InstanceCreationExpression>(g));
        self.for_type_annotation(node, TypeOpts { exclude_type_names: exclude, ..Default::default() });
    }

    fn visit_parenthesized_expression(&mut self, node: NodeId) {
        let ast = self.ast();
        let p = ast.cast::<ParenthesizedExpression>(node).unwrap();
        let expression = ast[p].expression.raw();
        if (ast.is::<Identifier>(expression) || ast.is::<PropertyAccess>(expression))
            && self.offset() == ast.t_offset(ast[p].right_parenthesis)
        {
            let next = ast.t_next(ast.end_tok(expression));
            if ast.t_ty(next) == TokenType::IDENTIFIER {
                self.kw("is");
                return;
            }
        }
        self.location("ParenthesizedExpression_expression");
        if ast.is::<SimpleIdentifier>(expression) {
            let context = self.context_type_of(node);
            self.suggest_record_literal_named_fields(context, node, None, true);
        }
        self.for_expression(node, ExprOpts::default());
    }

    fn visit_pattern_field(&mut self, node: NodeId) {
        let ast = self.ast();
        let f = ast.cast::<PatternField>(node).unwrap();
        let o = self.offset();
        let name = ast[f].name;
        if let Some(name) = name {
            if o <= ast.t_offset(ast[name].colon) {
                if ast.parent(node).is_some_and(|p| ast.is::<ObjectPattern>(p)) {
                    self.location("ObjectPattern_fieldName");
                } else {
                    self.location("PatternField_pattern");
                }
                self.for_pattern_field_name(name.raw(), false, false);
                return;
            }
        }
        match name {
            None => {
                let parent = ast.parent(node);
                if let Some(op) = parent.and_then(|p| ast.cast::<ObjectPattern>(p)) {
                    self.location("ObjectPattern_fieldName");
                    if let Some(t) = self.q.tables.annotation_type.get(ast[op].type_.raw()).copied() {
                        let excluded = field_names(ast, ast[op].fields);
                        let q = self.q;
                        self.decl(DeclConfig { must_be_non_void: true, ..Default::default() });
                        self.decl.as_mut().unwrap().add_getters(q, &mut self.out, t, &excluded, false, false);
                    }
                } else if parent.is_some_and(|p| ast.is::<RecordPattern>(p)) {
                    self.location("PatternField_pattern");
                    self.for_pattern(node, true);
                }
            }
            Some(n) if ast[n].name.is_none() => {
                self.location("PatternField_pattern");
                if ast.this_or_ancestor_of_type::<PatternVariableDeclaration>(node).is_none() {
                    K::add_variable_pattern_keywords(&mut self.out);
                }
                let is_keyword_needed = ast.this_or_ancestor_of_type::<GuardedPattern>(node).is_some();
                self.for_pattern_field_name(n.raw(), is_keyword_needed, true);
            }
            Some(_) => {
                self.location("PatternField_pattern");
                self.for_pattern(node, false);
            }
        }
    }

    fn visit_prefixed_identifier(&mut self, node: NodeId) {
        let ast = self.ast();
        let p = ast.cast::<PrefixedIdentifier>(node).unwrap();
        if self.offset() <= ast.t_offset(ast[p].period) {
            self.for_expression(node, ExprOpts::default());
            return;
        }
        self.location("PropertyAccess_propertyName");
        let target = ast[p].prefix.raw();
        let ctx = self.q.ctx;
        let ty = self.q.tables.static_type.get(target).copied();
        if let Some(t) = ty.filter(|t| !matches!(ctx.ty(*t), TypeKind::Invalid)) {
            let only_super = ast.is::<SuperExpression>(target);
            self.for_member_access(node, t, only_super);
            return;
        }
        let Some(element) = self.q.element(target) else {
            return;
        };
        let parent = ast.parent(node);
        let must_be_assignable = parent
            .and_then(|pa| ast.cast::<AssignmentExpression>(pa))
            .is_some_and(|a| ast[a].left_hand_side.raw() == node);
        if element.tag() == Tag::Prefix {
            let q = self.q;
            self.decl(DeclConfig { must_be_assignable, ..Default::default() });
            self.decl.as_mut().unwrap().add_declarations_through_import_prefix(q, &mut self.out, element);
            return;
        }
        let in_comment = parent.is_some_and(|pa| ast.is::<CommentReference>(pa));
        let prefer_non_invocation =
            in_comment || (element.is::<InterfaceElement>() && self.q.should_suggest_tear_off(element));
        let q = self.q;
        self.decl(DeclConfig { must_be_assignable, prefer_non_invocation, ..Default::default() });
        let d = self.decl.as_mut().unwrap();
        if in_comment {
            if let Some(i) = element.cast::<InterfaceElement>() {
                let t = ctx.interface_this_type(i);
                d.add_instance_members_of_type(q, &mut self.out, t, false);
            } else if let Some(e) = element.cast::<dartr_element::ExtensionElement>() {
                d.add_members_from_extension_element(q, &mut self.out, e, None, &[], true, true);
            }
        }
        d.add_static_members_of_element(q, &mut self.out, element, false);
    }

    fn visit_primary_constructor(&mut self, node: NodeId) {
        let ast = self.ast();
        let p = ast.cast::<PrimaryConstructorDeclaration>(node).unwrap();
        let o = self.offset();
        let formal = ast[p].formal_parameters;
        let Some(&parameter) = ast.list_raw(ast[formal].parameters).first() else {
            self.location("PrimaryConstructorDeclaration_fieldType");
            K::add_formal_parameter_keywords(self.q, &mut self.out, Some(formal), true, true, true, true, true);
            self.for_type_annotation(node, TypeOpts::default());
            return;
        };
        let Some(name_token) = parameter_name(ast, parameter) else {
            return;
        };
        let last = last_non_synthetic(ast, parameter);
        let last_kw = last.filter(|t| ast.t_is_keyword(*t) && o >= ast.t_end(*t)).map(|t| ast.t_lexeme(t));
        let after_comma = {
            let next = ast.t_next(ast.end_tok(parameter));
            ast.t_ty(next) == TokenType::COMMA && o >= ast.t_end(next)
        };
        match last_kw {
            Some("covariant") => {
                self.location("PrimaryConstructorDeclaration_fieldName");
                K::add_formal_parameter_keywords(self.q, &mut self.out, Some(formal), true, true, false, true, true);
                self.for_type_annotation(node, TypeOpts::default());
                return;
            }
            Some("required") => {
                self.location("PrimaryConstructorDeclaration_fieldName");
                K::add_formal_parameter_keywords(self.q, &mut self.out, Some(formal), false, true, false, true, true);
                self.for_type_annotation(node, TypeOpts::default());
                return;
            }
            Some("final") | Some("var") => {
                self.location("PrimaryConstructorDeclaration_fieldName");
                K::add_formal_parameter_keywords(self.q, &mut self.out, Some(formal), true, true, false, false, true);
                self.for_type_annotation(node, TypeOpts::default());
                return;
            }
            _ => {}
        }
        if o <= ast.t_end(name_token) {
            let parameter_type = ast
                .cast::<RegularFormalParameter>(parameter)
                .and_then(|r| ast[r].type_)
                .map(|t| t.raw());
            if parameter_type.is_none_or(|t| ast.fully_synthetic(t)) {
                let next = ast.t_next(ast[formal].left_parenthesis);
                let has_incomplete_annotation = ast.t_ty(next) == TokenType::AT
                    || (ast.t_synthetic(next) && ast.t_ty(ast.t_next(next)) == TokenType::AT);
                if ast.t_synthetic(name_token) && has_incomplete_annotation {
                    self.location("Annotation_name");
                    self.for_annotation(parameter);
                } else {
                    self.location("PrimaryConstructorDeclaration_fieldType");
                    self.add_lexical(DeclConfig { must_be_type: true, ..Default::default() }, parameter);
                }
            } else {
                self.location("PrimaryConstructorDeclaration_fieldName");
                self.ident(true).add_variable(self.q, &mut self.out, parameter_type);
            }
        } else if after_comma {
            self.location("PrimaryConstructorDeclaration_fieldType");
            K::add_formal_parameter_keywords(self.q, &mut self.out, Some(formal), true, true, true, true, true);
            self.for_type_annotation(node, TypeOpts::default());
        } else {
            self.location("PrimaryConstructorDeclaration_fieldName");
            let name = ast.t_lexeme(name_token).to_string();
            self.ident(true).add_suggestions_from_type_name(&mut self.out, &name);
        }
    }

    fn visit_property_access(&mut self, node: NodeId) {
        let ast = self.ast();
        let p = ast.cast::<PropertyAccess>(node).unwrap();
        let o = self.offset();
        let operator = ast[p].operator;
        if o <= ast.t_offset(operator) {
            self.for_expression(node, ExprOpts::default());
            return;
        }
        self.location("PropertyAccess_propertyName");
        let Some(target) = super::target::real_target_of_property_access(ast, p) else {
            return;
        };
        let parent = ast.parent(node);
        if ast.is::<ThisExpression>(target) && parent.is_some_and(|pa| ast.is::<ConstructorFieldInitializer>(pa)) {
            self.visit_parent(node);
            return;
        }
        let ctx = self.q.ctx;
        let ty = self.q.tables.static_type.get(target).copied();
        if let Some(t) = ty {
            self.for_member_access(node, t, ast.is::<SuperExpression>(target));
        }
        let element = self.static_member_target_element(target);
        let is_cascaded = dartr_resolver::ast_ext::property_access_is_cascaded(ast, p);
        let type_ok = ty.is_none_or(|t| matches!(ctx.ty(t), TypeKind::Invalid) || ctx.is_dart_core_type(t));
        if type_ok && element.is_some() && (!is_cascaded || o + 1 == ast.t_end(operator)) {
            let element = element.unwrap();
            if element.is::<InterfaceElement>() {
                let q = self.q;
                self.decl(DeclConfig::default());
                self.decl.as_mut().unwrap().add_static_members_of_element(q, &mut self.out, element, false);
            }
            if element.tag() == Tag::Prefix {
                let q = self.q;
                self.decl(DeclConfig::default());
                self.decl.as_mut().unwrap().add_declarations_through_import_prefix(q, &mut self.out, element);
            }
        }
        if ty.is_none() && ast.is::<ExtensionOverride>(target) {
            if let Some(e) = self.q.element(target).and_then(|e| e.cast::<dartr_element::ExtensionElement>()) {
                let q = self.q;
                self.decl(DeclConfig::default());
                self.decl
                    .as_mut()
                    .unwrap()
                    .add_members_from_extension_element(q, &mut self.out, e, None, &[], true, true);
            }
        }
    }

    fn visit_record_pattern(&mut self, node: NodeId) {
        let ast = self.ast();
        let r = ast.cast::<RecordPattern>(node).unwrap();
        let o = self.offset();
        if o == ast.t_offset(ast[r].left_parenthesis) {
            self.location("ObjectPattern_type");
            self.add_lexical(DeclConfig { must_be_type: true, must_be_non_void: true, ..Default::default() }, node);
            return;
        }
        if ast.t_end(ast[r].left_parenthesis) <= o && o <= ast.t_offset(ast[r].right_parenthesis) {
            self.location("PatternField_pattern");
            self.kw("dynamic");
            self.for_expression(node, ExprOpts::default());
            let target = ast.list(ast[r].fields).iter().copied().find(|f| ast.end(*f) >= o);
            if let Some(t) = target {
                if let Some(name) = ast[t].name {
                    if o <= ast.t_offset(ast[name].colon) {
                        let matched = self.q.tables.pattern_info.get(node).and_then(|i| i.matched_value_type);
                        if let Some(m) = matched {
                            let excluded = field_names(ast, ast[r].fields);
                            let q = self.q;
                            self.decl(DeclConfig { must_be_non_void: true, ..Default::default() });
                            self.decl.as_mut().unwrap().add_getters(q, &mut self.out, m, &excluded, false, false);
                        }
                    }
                }
            }
        }
    }

    fn visit_redirecting_constructor_invocation(&mut self, node: NodeId) {
        let ast = self.ast();
        let r = ast.cast::<RedirectingConstructorInvocation>(node).unwrap();
        let Some(constructor) = ast.parent(node).and_then(|p| ast.cast::<ConstructorDeclaration>(p)) else {
            return;
        };
        self.location("ConstructorDeclaration_initializer");
        let o = self.offset();
        if o <= ast.t_end(ast[r].this_keyword) && ast.fully_synthetic(ast[r].argument_list.raw()) {
            K::add_constructor_initializer_keywords(self.q, &mut self.out, constructor, Some(node));
            return;
        }
        if let Some(period) = ast[r].period {
            if o >= ast.t_end(period) && o <= ast.offset(ast[r].argument_list) {
                self.for_redirecting_constructor_invocation(constructor);
            }
        }
    }

    fn visit_regular_formal_parameter(&mut self, node: NodeId) {
        let ast = self.ast();
        let p = ast.cast::<RegularFormalParameter>(node).unwrap();
        if let Some(primary) = ast
            .parent(node)
            .and_then(|l| ast.parent(l))
            .filter(|g| ast.is::<PrimaryConstructorDeclaration>(*g))
        {
            self.visit(primary);
            return;
        }
        let o = self.offset();
        let name = ast[p].name;
        let no_required = ast[p].required_keyword.is_none();
        let mut suggest_covariant = true;
        let mut suggest_this = true;
        let mut suggest_void = true;
        let mut suggest_dynamic = true;
        let mut suggest_required = true;
        let list = parent_parameter_list(ast, node);
        let single = ast.begin(node) == ast.end_tok(node) && ast.t_kw_or_ident(ast.begin(node));
        if let Some(n) = name {
            if single {
                self.location("FormalParameterList_parameter");
                let lexeme = ast.t_lexeme(n);
                let is_kw = ast.t_is_keyword(n);
                K::add_formal_parameter_keywords(
                    self.q,
                    &mut self.out,
                    list,
                    suggest_required && !(is_kw && lexeme == "required"),
                    true,
                    suggest_covariant && !(is_kw && lexeme == "covariant"),
                    true,
                    true,
                );
                suggest_covariant = false;
                suggest_this = false;
                suggest_required = false;
                self.for_type_annotation(
                    node,
                    TypeOpts { no_dynamic: !suggest_dynamic, no_void: !suggest_void, ..Default::default() },
                );
                suggest_dynamic = false;
                suggest_void = false;
                if is_kw {
                    return;
                }
            }
        }
        if let Some(ty) = ast[p].type_ {
            let ty = ty.raw();
            self.location("FormalParameterList_parameter");
            if let Some(nt) = ast.cast::<NamedType>(ty) {
                if let Some(ip) = ast[nt].import_prefix {
                    if let Some(prefix) = self.q.element(ip.raw()).filter(|e| e.tag() == Tag::Prefix) {
                        if ast.t_covers(Some(ast[nt].name), o) {
                            let q = self.q;
                            self.decl(DeclConfig { must_be_type: true, ..Default::default() });
                            self.decl.as_mut().unwrap().add_declarations_through_import_prefix(q, &mut self.out, prefix);
                        }
                    }
                }
            }
            if ast.t_covers(Some(ast.begin(ty)), o) {
                let suggest_variable_name = name.is_none() || ast.t_offset(ast.begin(ty)) == o;
                K::add_formal_parameter_keywords(
                    self.q,
                    &mut self.out,
                    list,
                    no_required && suggest_required,
                    suggest_variable_name,
                    true,
                    true,
                    true,
                );
                self.for_type_annotation(node, TypeOpts::default());
            } else if let Some(g) = ast.cast::<GenericFunctionType>(ty) {
                if o < ast.t_offset(ast[g].function_keyword) && ast[g].return_type.is_none() {
                    self.for_type_annotation(node, TypeOpts::default());
                }
            }
        } else {
            let keyword = ast[p].const_final_or_var_keyword;
            if keyword.is_none_or(|k| o <= ast.t_end(k)) {
                self.location("FormalParameterList_parameter");
                if let Some(list) = list {
                    K::add_formal_parameter_keywords(
                        self.q,
                        &mut self.out,
                        Some(list),
                        no_required && suggest_required,
                        ast.t_covers(name, o),
                        suggest_covariant,
                        true,
                        suggest_this,
                    );
                }
                self.for_type_annotation(
                    node,
                    TypeOpts { no_void: !suggest_void, no_dynamic: !suggest_dynamic, ..Default::default() },
                );
            }
        }
    }

    fn visit_simple_string_literal(&mut self, node: NodeId) {
        let ast = self.ast();
        if self.suggest_uris {
            if let Some(p) = ast.parent(node) {
                if ast.is::<Configuration>(p) || ast.is::<PartOfDirective>(p) || ast.is::<UriBasedDirective>(p) {
                    super::uri::add_uri_suggestions(self.q, &mut self.out, node);
                    return;
                }
            }
        }
        self.visit_parent_if_at_or_before(node);
    }

    fn visit_super_constructor_invocation(&mut self, node: NodeId) {
        let ast = self.ast();
        let s = ast.cast::<SuperConstructorInvocation>(node).unwrap();
        let Some(constructor) = ast.parent(node).and_then(|p| ast.cast::<ConstructorDeclaration>(p)) else {
            return;
        };
        self.location("ConstructorDeclaration_initializer");
        let o = self.offset();
        if o <= ast.t_end(ast[s].super_keyword) && ast.fully_synthetic(ast[s].argument_list.raw()) {
            K::add_constructor_initializer_keywords(self.q, &mut self.out, constructor, Some(node));
            return;
        }
        if let Some(period) = ast[s].period {
            if o >= ast.t_end(period) && o <= ast.offset(ast[s].argument_list) {
                let container = ast.parent(constructor).and_then(|p| ast.parent(p));
                let ctx = self.q.ctx;
                let super_type = container
                    .filter(|c| ast.is::<ClassDeclaration>(*c) || ast.is::<EnumDeclaration>(*c))
                    .and_then(|c| self.q.declared_element(c))
                    .and_then(|e| e.cast::<InterfaceElement>())
                    .and_then(|e| ctx.element_supertype(e));
                if let Some(t) = super_type {
                    let q = self.q;
                    self.decl(DeclConfig { must_be_constant: ast[constructor].const_keyword.is_some(), ..Default::default() });
                    self.decl.as_mut().unwrap().add_constructor_names_for_type(q, &mut self.out, t, None);
                }
            }
        }
    }

    fn visit_switch_expression_case(&mut self, node: NodeId) {
        let ast = self.ast();
        let c = ast.cast::<SwitchExpressionCase>(node).unwrap();
        let arrow = ast[c].arrow;
        if ast.t_synthetic(arrow) || ast.t_offset(arrow) >= self.offset() {
            self.location("SwitchExpression_body");
            self.for_pattern(node, true);
            return;
        }
        let expression = ast[c].expression.raw();
        let end = ast.end_tok(expression);
        if end == ast.begin(expression) || ast.t_synthetic(end) {
            self.location("SwitchExpressionCase_expression");
            let ty = self.context_type_of(expression).unwrap_or(TypeId::DYNAMIC);
            let ctx = self.q.ctx;
            let can_be_bool = self.can_be_bool(ty);
            let can_be_null = self.can_be_null(ty);
            let can_suggest_const = !ctx.is_dart_core_function(ty) && !matches!(ctx.ty(ty), TypeKind::Function(_));
            self.for_expression(
                expression,
                ExprOpts { can_be_bool, can_be_null, can_suggest_const, ..Default::default() },
            );
        }
    }

    fn visit_switch_pattern_case(&mut self, node: NodeId) {
        let ast = self.ast();
        let c = ast.cast::<SwitchPatternCase>(node).unwrap();
        let covering = self.q.covering;
        let o = self.offset();
        if o <= ast.t_end(ast[c].keyword) {
            self.kw("case");
            return;
        }
        if o <= ast.t_offset(ast[c].colon) {
            self.location("SwitchPatternCase_pattern");
            if let Some(nt) = ast.cast::<NamedType>(covering) {
                if ast.parent(nt).is_some_and(|p| ast.is::<ObjectPattern>(p)) {
                    self.location("ObjectPattern_type");
                    self.visit(covering);
                    return;
                }
            }
            let pattern = ast[ast[c].guarded_pattern].pattern.raw();
            if let Some(cp) = ast.cast::<ConstantPattern>(pattern) {
                if let Some(i) = ast.cast::<SimpleIdentifier>(ast[cp].expression.raw()) {
                    if !ast.n_synthetic(i.raw()) && o < ast.offset(i) {
                        self.add_lexical(DeclConfig { must_be_type: true, ..Default::default() }, node);
                        return;
                    }
                }
            }
            if let Some(w) = ast.cast::<WildcardPattern>(pattern) {
                if o < ast.t_offset(ast[w].name) {
                    self.add_lexical(DeclConfig { must_be_type: true, ..Default::default() }, node);
                    return;
                }
            }
            if let Some(cp) = ast.cast::<ConstantPattern>(pattern) {
                if let Some(tl) = ast.cast::<TypeLiteral>(ast[cp].expression.raw()) {
                    let nt = ast[tl].type_;
                    if ast.end(nt) < o {
                        let name = ast.t_lexeme(ast[nt].name).to_string();
                        self.ident(false).add_suggestions_from_type_name(&mut self.out, &name);
                        return;
                    }
                }
            }
            if ast.is::<NamedType>(covering) {
                if let Some(p) = ast.parent(covering) {
                    match ast.kind(p) {
                        NodeKind::DeclaredVariablePattern => {
                            self.visit(covering);
                            return;
                        }
                        NodeKind::ObjectPattern => {
                            self.location("ObjectPattern_type");
                            self.visit(covering);
                            return;
                        }
                        NodeKind::WildcardPattern => {
                            self.location("WildcardPattern_type");
                            self.visit(covering);
                            return;
                        }
                        _ => {}
                    }
                }
            }
            let Some(previous) = ast.t_prev(ast[c].colon) else {
                return;
            };
            let previous_keyword = ast.t_is_keyword(previous).then(|| ast.t_lexeme(previous));
            match previous_keyword {
                None => {
                    if ast.t_synthetic(previous) || ast.t_covers(Some(previous), o) {
                        self.kw("final");
                        self.kw("var");
                        if let Some(cp) = ast.cast::<ConstantPattern>(pattern) {
                            self.for_expression(ast[cp].expression.raw(), ExprOpts { must_be_non_void: true, ..Default::default() });
                        } else {
                            self.for_expression(pattern, ExprOpts { must_be_non_void: true, ..Default::default() });
                        }
                    } else {
                        self.kw("as");
                        self.kw("when");
                    }
                }
                Some("as") => self.kw("dynamic"),
                Some("when") => {}
                Some(_) => {
                    self.kw("as");
                    self.kw("when");
                }
            }
        } else {
            self.location("SwitchMember_statement");
            let statements = ast.list_raw(ast[c].statements);
            if statements.is_empty() || o <= ast.offset(statements[0]) {
                self.kw("case");
                self.kw_text("default", ":");
            }
            self.for_statement(node);
        }
    }

    fn visit_switch_statement(&mut self, node: NodeId) {
        let ast = self.ast();
        let s = ast.cast::<SwitchStatement>(node).unwrap();
        let o = self.offset();
        if o <= ast.t_end(ast[s].switch_keyword) {
            self.location("Block_statement");
            self.for_statement(node);
        } else if o >= ast.t_end(ast[s].left_parenthesis) && o <= ast.t_offset(ast[s].right_parenthesis) {
            self.location("SwitchStatement_expression");
            self.for_expression(node, ExprOpts { must_be_non_void: true, ..Default::default() });
        } else if o >= ast.t_end(ast[s].left_bracket) && o <= ast.t_offset(ast[s].right_bracket) {
            self.location("SwitchMember_statement");
            self.kw("case");
            self.kw_text("default", ":");
            let members = ast.list_raw(ast[s].members).to_vec();
            if let Some(e) = element_before(ast, &members, o) {
                self.for_statement(e);
            }
        }
    }

    fn visit_top_level_variable_declaration(&mut self, node: NodeId) {
        let ast = self.ast();
        let d = ast.cast::<TopLevelVariableDeclaration>(node).unwrap();
        if self.handled_recovery(node) {
            return;
        }
        let o = self.offset();
        if o == ast.offset(node) {
            self.for_compilation_unit_member_before(node);
            return;
        }
        let list = ast[d].variables;
        let variables = ast.list(ast[list].variables).to_vec();
        let Some(&first) = variables.first() else {
            return;
        };
        if o > ast.t_end(ast.begin(first.raw())) {
            if ast[list].type_.is_none() {
                let name = ast.t_lexeme(ast[first].name).to_string();
                self.ident(true).add_suggestions_from_type_name(&mut self.out, &name);
            }
            return;
        }
        if ast[d].external_keyword.is_none() {
            self.kw("external");
        }
        if ast[list].late_keyword.is_none() {
            self.kw("late");
        }
        if !variable_list_is_const(ast, list) {
            self.kw("const");
        }
        if !variable_list_is_final(ast, list) {
            self.kw("final");
        }
        if let Some(unit) = ast.parent(node).and_then(|p| ast.cast::<CompilationUnit>(p)) {
            let (before, _) = members_around_member(ast, unit, node);
            if before.is_none_or(|b| ast.is::<Directive>(b)) {
                self.location("CompilationUnit_directive");
            } else {
                self.location("CompilationUnit_declaration");
            }
        }
        self.add_lexical(DeclConfig { must_be_type: true, ..Default::default() }, node);
    }

    fn visit_try_statement(&mut self, node: NodeId) {
        let ast = self.ast();
        let t = ast.cast::<TryStatement>(node).unwrap();
        let o = self.offset();
        if o <= ast.t_end(ast[t].try_keyword) {
            self.location("Block_statement");
            self.for_statement(node);
        } else if o >= ast.end(ast[t].body) {
            match ast[t].finally_keyword {
                None => {
                    let clauses = ast.list(ast[t].catch_clauses);
                    match clauses.last() {
                        None => K::add_try_clause_keywords(&mut self.out, true),
                        Some(&last) => K::add_try_clause_keywords(&mut self.out, o >= ast.end(last)),
                    }
                }
                Some(f) if o < ast.t_offset(f) => K::add_try_clause_keywords(&mut self.out, false),
                _ => {}
            }
        }
    }

    fn visit_type_parameter(&mut self, node: NodeId) {
        let ast = self.ast();
        let p = ast.cast::<TypeParameter>(node).unwrap();
        let o = self.offset();
        if let Some(list) = ast.parent(node).and_then(|l| ast.cast::<TypeParameterList>(l)) {
            let left = ast.t_next(ast[list].right_bracket);
            let right = ast.t_next(left);
            if ast.t_ty(left) == TokenType::OPEN_PAREN
                && ast.t_synthetic(left)
                && ast.t_ty(right) == TokenType::CLOSE_PAREN
                && ast.t_synthetic(right)
            {
                self.location("TypeParameter_bound");
                self.for_type_annotation(node, TypeOpts { excluded_nodes: vec![list.raw()], ..Default::default() });
                return;
            }
        }
        if o <= ast.t_end(ast[p].name) {
            return;
        }
        match ast[p].extends_keyword {
            Some(e) if o > ast.t_end(e) => {
                self.location("TypeParameter_bound");
                self.for_type_annotation(node, TypeOpts { must_be_non_void: true, ..Default::default() });
            }
            _ => self.kw("extends"),
        }
    }

    fn visit_variable_declaration(&mut self, node: NodeId) {
        let ast = self.ast();
        let v = ast.cast::<VariableDeclaration>(node).unwrap();
        let o = self.offset();
        let Some(list) = ast.parent(node).and_then(|p| ast.cast::<VariableDeclarationList>(p)) else {
            return;
        };
        let grandparent = ast.parent(list);
        if let Some(fd) = grandparent.filter(|g| ast.is::<FieldDeclaration>(*g)) {
            let incomplete = self.for_incomplete_preceding_class_member(fd);
            let f = ast.cast::<FieldDeclaration>(fd).unwrap();
            let first = ast[f].first_token_after_comment_and_metadata(ast);
            if incomplete && is_single_identifier_tokens(ast, first, ast.end_tok(fd)) {
                return;
            }
        } else if grandparent.is_some_and(|g| ast.is::<ForPartsWithDeclarations>(g)) {
            if ast[v].equals.is_none()
                && ast.list(ast[list].variables).len() == 1
                && ast[list].type_.is_some_and(|t| ast.is::<RecordTypeAnnotation>(t.raw()))
            {
                self.kw("in");
            }
        } else if let Some(g) = grandparent.filter(|g| ast.is::<TopLevelVariableDeclaration>(*g)) {
            if self.handled_recovery(g) {
                return;
            }
        }
        if o <= ast.t_end(ast[v].name) {
            let mut const_added = false;
            let mut container = grandparent.and_then(|g| ast.parent(g));
            if container.is_some_and(|c| ast.is::<BlockClassBody>(c)) {
                container = container.and_then(|c| ast.parent(c));
            }
            let keyword = ast[list].keyword;
            match ast[list].type_ {
                None => {
                    self.location("VariableDeclarationList_type");
                    if keyword.is_none() {
                        self.kw("const");
                        self.kw("final");
                        self.kw("var");
                        const_added = true;
                    }
                    if let Some(t) = grandparent.and_then(|g| ast.cast::<TopLevelVariableDeclaration>(g)) {
                        if ast[t].abstract_keyword.is_some() {
                            for k in ["base", "class", "final", "interface", "mixin"] {
                                self.kw(k);
                            }
                        }
                    }
                    if keyword.is_none_or(|k| ast.t_lexeme(k) != "var") {
                        self.for_type_annotation(node, TypeOpts::default());
                    }
                }
                Some(t) => {
                    let can_be_private = grandparent
                        .is_some_and(|g| ast.is::<FieldDeclaration>(g) || ast.is::<TopLevelVariableDeclaration>(g));
                    self.ident(can_be_private).add_variable(self.q, &mut self.out, Some(t.raw()));
                }
            }
            if let Some(fd) = grandparent.and_then(|g| ast.cast::<FieldDeclaration>(g)) {
                self.location("FieldDeclaration_fields");
                if ast[fd].external_keyword.is_none() {
                    self.kw("external");
                }
                if ast[fd].static_keyword.is_none() {
                    self.kw("static");
                    if container.is_some_and(|c| ast.is::<ClassDeclaration>(c) || ast.is::<MixinDeclaration>(c)) {
                        if ast[fd].abstract_keyword.is_none() {
                            self.kw("abstract");
                        }
                        if ast[fd].covariant_keyword.is_none() {
                            self.kw("covariant");
                        }
                    }
                    if ast[list].late_keyword.is_none() && !container.is_some_and(|c| ast.is::<ExtensionDeclaration>(c)) {
                        self.kw("late");
                    }
                }
                let first = ast[fd].first_token_after_comment_and_metadata(ast);
                if ast[v].name == first {
                    if !const_added {
                        self.kw("const");
                    }
                    if container.is_some_and(|c| ast.is::<ClassDeclaration>(c)) {
                        self.kw("factory");
                    }
                    self.kw("get");
                    self.kw("operator");
                    self.kw("set");
                }
                if is_single_identifier_tokens(ast, first, ast.end_tok(fd.raw())) {
                    let c = container;
                    if c.is_some_and(|c| ast.is::<ClassDeclaration>(c)) {
                        self.location("ClassDeclaration_member");
                    } else if c.is_some_and(|c| ast.is::<EnumDeclaration>(c)) {
                        self.location("EnumDeclaration_member");
                    } else if c.is_some_and(|c| ast.is::<ExtensionDeclaration>(c)) {
                        self.location("ExtensionDeclaration_member");
                    } else if c.is_some_and(|c| ast.is::<MixinDeclaration>(c)) {
                        self.location("MixinDeclaration_member");
                    }
                    let element = c
                        .filter(|c| ast.is::<ClassDeclaration>(*c) || ast.is::<MixinDeclaration>(*c))
                        .and_then(|c| self.q.declared_element(c))
                        .and_then(|e| e.cast::<InterfaceElement>());
                    self.suggest_overrides_for(element, false);
                }
            } else if let Some(t) = grandparent.and_then(|g| ast.cast::<TopLevelVariableDeclaration>(g)) {
                if ast[t].external_keyword.is_none() {
                    self.kw("external");
                }
                if ast[list].late_keyword.is_none() && !container.is_some_and(|c| ast.is::<ExtensionDeclaration>(c)) {
                    self.kw("late");
                }
            }
            return;
        }
        if let Some(equals) = ast[v].equals {
            if o >= ast.t_end(equals) {
                self.location("VariableDeclaration_initializer");
                self.for_expression(node, ExprOpts { must_be_non_void: true, ..Default::default() });
            }
        }
    }

    fn visit_variable_declaration_list(&mut self, node: NodeId) {
        let ast = self.ast();
        let l = ast.cast::<VariableDeclarationList>(node).unwrap();
        let variables = ast.list(ast[l].variables);
        if let Some(&first) = variables.first() {
            if self.offset() <= ast.t_end(ast[first].name) {
                let ty = ast[l].type_.map(|t| t.raw());
                let keyword_is_var = ast[l].keyword.is_some_and(|k| ast.t_lexeme(k) == "var");
                if ty.is_none_or(|t| ast.n_covers(Some(t), self.offset())) && !keyword_is_var {
                    self.location("VariableDeclarationList_type");
                    self.for_type_annotation(node, TypeOpts::default());
                } else if ty.is_some_and(|t| ast.is::<RecordTypeAnnotation>(t)) {
                    self.kw("in");
                }
            }
        }
    }

    // --------------------------------------------------------------- helpers

    fn can_be_bool(&self, ty: TypeId) -> bool {
        let ctx = self.q.ctx;
        TypeSystem::new(*ctx).is_subtype_of(ctx.tp.bool_type(), ty)
    }

    fn can_be_null(&self, ty: TypeId) -> bool {
        let ctx = self.q.ctx;
        ctx.nullability_suffix(ty) != Nullability::None
            || ctx.is_dart_core_null(ty)
            || matches!(ctx.ty(ty), TypeKind::Dynamic)
    }

    fn is_widget_creation(&self, i: Id<InstanceCreationExpression>) -> bool {
        let ctx = self.q.ctx;
        self.q
            .tables
            .static_type
            .get(i.raw())
            .is_some_and(|t| is_widget_type(ctx, *t))
    }

    fn for_annotation(&mut self, node: NodeId) {
        self.add_lexical(DeclConfig { must_be_constant: true, ..Default::default() }, node);
    }

    fn for_class_like_member(&mut self, node: NodeId) {
        let ast = self.ast();
        match ast.kind(node) {
            NodeKind::ClassDeclaration => self.for_class_member(ast.cast::<ClassDeclaration>(node).unwrap()),
            NodeKind::EnumDeclaration => self.for_enum_member(node),
            NodeKind::ExtensionDeclaration => self.for_extension_member(node),
            NodeKind::ExtensionTypeDeclaration => self.for_extension_type_member(node),
            NodeKind::MixinDeclaration => self.for_mixin_member(ast.cast::<MixinDeclaration>(node).unwrap()),
            _ => {}
        }
    }

    fn for_class_member(&mut self, node: Id<ClassDeclaration>) {
        K::add_class_member_keywords(self.q, &mut self.out);
        self.add_lexical(DeclConfig { must_be_type: true, ..Default::default() }, node.raw());
        let element = self.q.declared_element(node.raw()).and_then(|e| e.cast::<InterfaceElement>());
        self.suggest_overrides_for(element, false);
    }

    fn for_enclosing_collection(&mut self, node: NodeId) {
        let ast = self.ast();
        let literal = ast.this_or_ancestor_matching(node, |a, n| a.is::<ListLiteral>(n) || a.is::<SetOrMapLiteral>(n));
        if let Some(l) = literal {
            let elements = if let Some(ll) = ast.cast::<ListLiteral>(l) {
                ast.list_raw(ast[ll].elements).to_vec()
            } else {
                let s = ast.cast::<SetOrMapLiteral>(l).unwrap();
                ast.list_raw(ast[s].elements).to_vec()
            };
            self.for_collection_element(l, &elements);
        }
    }

    fn for_collection_element(&mut self, literal: NodeId, elements: &[NodeId]) {
        let ast = self.ast();
        let must_be_static = in_static_context(ast, literal);
        let must_be_const = dartr_resolver::ast_ext::in_constant_context(ast, literal);
        K::add_collection_element_keywords(self.q, &mut self.out, literal, elements, must_be_const, must_be_static);
        let preceding = element_before(ast, elements, self.offset());
        let cfg = DeclConfig { must_be_static, must_be_constant: must_be_const, ..Default::default() };
        self.add_lexical(cfg, preceding.unwrap_or(literal));
    }

    fn for_combinator(&mut self, combinator: NodeId, existing: &[NodeId]) {
        let ast = self.ast();
        let Some(directive) = ast.parent(combinator) else {
            return;
        };
        if !ast.is::<ImportDirective>(directive) && !ast.is::<ExportDirective>(directive) {
            return;
        }
        let u = self.q.unit();
        let Some(library) = u.directive_library(directive).and_then(|l| l.cast::<dartr_element::LibraryElement>()) else {
            return;
        };
        let covering = self.q.covering;
        let excluded_name = existing.iter().copied().find(|e| *e == covering);
        let excluded: Vec<String> = existing
            .iter()
            .filter(|e| Some(**e) != excluded_name)
            .filter_map(|e| ast.cast::<SimpleIdentifier>(*e))
            .map(|s| ast.t_lexeme(ast[s].token).to_string())
            .collect();
        let q = self.q;
        self.decl(DeclConfig { prefer_non_invocation: true, ..Default::default() });
        self.decl.as_mut().unwrap().add_from_library(q, &mut self.out, library, &excluded);
    }

    fn for_compilation_unit_declaration(&mut self, unit: Id<CompilationUnit>) {
        K::add_compilation_unit_declaration_keywords(self.q, &mut self.out);
        self.add_lexical(DeclConfig { must_be_type: true, ..Default::default() }, unit.raw());
    }

    fn for_compilation_unit_member(&mut self, unit: Id<CompilationUnit>, before: Option<NodeId>, after: Option<NodeId>) {
        let ast = self.ast();
        if before.is_none_or(|b| ast.is::<Directive>(b)) {
            self.location("CompilationUnit_directive");
            self.for_directive(unit, before);
        }
        if after.is_none_or(|a| ast.is::<CompilationUnitMember>(a)) {
            if self.out.collector.completion_location.is_none() {
                self.location("CompilationUnit_declaration");
            }
            self.for_compilation_unit_declaration(unit);
        }
    }

    fn for_compilation_unit_member_before(&mut self, member: NodeId) {
        let ast = self.ast();
        if let Some(unit) = ast.parent(member).and_then(|p| ast.cast::<CompilationUnit>(p)) {
            let (before, after) = members_around_member(ast, unit, member);
            self.for_compilation_unit_member(unit, before, after);
        }
    }

    fn for_constant_expression(&mut self, node: NodeId) {
        let ast = self.ast();
        let in_constant_context = ast.is::<Expression>(node) && dartr_resolver::ast_ext::in_constant_context(ast, node);
        K::add_constant_expression_keywords(&mut self.out, in_constant_context);
        let cfg = DeclConfig { must_be_constant: true, must_be_static: in_static_context(ast, node), ..Default::default() };
        self.add_lexical(cfg, node);
    }

    fn for_constructor_initializer(&mut self, constructor: Id<ConstructorDeclaration>, initializer: Option<Id<ConstructorFieldInitializer>>) {
        let ast = self.ast();
        let field = initializer
            .and_then(|i| self.q.element(ast[i].field_name.raw()))
            .filter(|e| e.tag() == Tag::Field);
        K::add_constructor_initializer_keywords(self.q, &mut self.out, constructor, initializer.map(|i| i.raw()));
        let q = self.q;
        self.decl(DeclConfig::default());
        self.decl.as_mut().unwrap().add_fields_for_initializers(q, &mut self.out, constructor, field);
    }

    fn for_directive(&mut self, unit: Id<CompilationUnit>, before: Option<NodeId>) {
        K::add_directive_keywords(self.q, &mut self.out, unit, before);
    }

    fn for_enum_member(&mut self, node: NodeId) {
        K::add_enum_member_keywords(self.q, &mut self.out);
        self.add_lexical(DeclConfig { must_be_type: true, ..Default::default() }, node);
    }

    /// Dart `_forExpression`.
    fn for_expression(&mut self, node: NodeId, opts: ExprOpts) {
        let ast = self.ast();
        let must_be_constant = ast.is::<Expression>(node)
            && (constant_context_including_self(ast, node)
                || ast.parent(node).is_some_and(|p| ast.is::<FormalParameterDefaultClause>(p)));
        let must_be_static = in_static_context(ast, node);
        K::add_expression_keywords(
            self.q,
            &mut self.out,
            Some(node),
            opts.can_be_bool,
            opts.can_be_null,
            opts.can_suggest_const,
            must_be_constant,
            must_be_static,
        );
        let cfg = DeclConfig {
            must_be_assignable: opts.must_be_assignable,
            must_be_constant,
            must_be_non_void: opts.must_be_non_void,
            must_be_static,
            prefer_non_invocation: opts.prefer_non_invocation,
            ..Default::default()
        };
        self.add_lexical(cfg, node);
        let ctx = self.q.ctx;
        let mut ty = self.q.context_type;
        if ty.is_some_and(|t| ctx.is_dart_core_function(t)) {
            ty = Some(self.void_function_no_parameters());
        }
        if let Some(t) = ty {
            if matches!(ctx.ty(t), TypeKind::Function(_)) && !must_be_constant {
                self.add_closure_suggestion(t, opts.include_trailing_comma_after_closure);
            }
        }
    }

    fn add_closure_suggestion(&mut self, function_type: TypeId, include_trailing_comma: bool) {
        let include_types = self.q.style.specify_types;
        let indent = self.q.indent();
        let end_of_line = self.q.end_of_line();
        for use_block_statement in [true, false] {
            self.out.add(Candidate::new(
                Kind::Closure {
                    function_type,
                    include_trailing_comma,
                    use_block_statement,
                    include_types,
                    indent: indent.clone(),
                    end_of_line: end_of_line.clone(),
                },
                0.0,
            ));
        }
    }

    fn for_extension_member(&mut self, node: NodeId) {
        K::add_extension_member_keywords(&mut self.out, false);
        self.add_lexical(DeclConfig { must_be_type: true, ..Default::default() }, node);
    }

    fn for_extension_type_member(&mut self, node: NodeId) {
        K::add_extension_type_member_keywords(self.q, &mut self.out, false);
        self.add_lexical(DeclConfig { must_be_type: true, ..Default::default() }, node);
    }

    /// Dart `_forIncompletePrecedingClassMember`.
    fn for_incomplete_preceding_class_member(&mut self, member: NodeId) -> bool {
        let ast = self.ast();
        if self.offset() <= ast.t_end(ast.begin(member)) {
            if let Some(m) = preceding_member(ast, member).and_then(|p| ast.cast::<MethodDeclaration>(p)) {
                let body = ast[m].body.raw();
                if ast.fully_synthetic(body) {
                    self.location("ClassDeclaration_member");
                    K::add_function_body_modifiers(self.q, &mut self.out, Some(body));
                    return true;
                }
            }
        }
        false
    }

    /// Dart `_forIncompletePrecedingStatement`.
    fn for_incomplete_preceding_statement(&mut self, statement: NodeId) -> bool {
        let ast = self.ast();
        if self.offset() <= ast.t_end(ast.begin(statement)) {
            let Some(preceding) = preceding_statement(ast, statement) else {
                return false;
            };
            if let Some(i) = ast.cast::<IfStatement>(preceding) {
                if ast[i].else_keyword.is_none() {
                    self.kw("else");
                    return false;
                }
            } else if let Some(t) = ast.cast::<TryStatement>(preceding) {
                if ast[t].finally_block.is_none() {
                    self.visit_try_statement(preceding);
                    return ast.list(ast[t].catch_clauses).is_empty();
                }
            }
        }
        false
    }

    /// Dart `_forMemberAccess`.
    fn for_member_access(&mut self, node: NodeId, ty: TypeId, only_super: bool) {
        let ast = self.ast();
        let parent = ast.parent(node);
        let must_be_assignable = self.compute_must_be_assignable(node);
        let cfg = DeclConfig {
            must_be_assignable,
            must_be_constant: dartr_resolver::ast_ext::in_constant_context(ast, node),
            must_be_non_void: parent.is_some_and(|p| ast.is::<ArgumentList>(p)),
            ..Default::default()
        };
        let q = self.q;
        self.decl(cfg);
        self.decl.as_mut().unwrap().add_instance_members_of_type(q, &mut self.out, ty, only_super);
    }

    /// Dart `_computeMustBeAssignable`.
    fn compute_must_be_assignable(&self, node: NodeId) -> bool {
        let ast = self.ast();
        if let Some(a) = ast.parent(node).and_then(|p| ast.cast::<AssignmentExpression>(p)) {
            if ast[a].left_hand_side.raw() == node {
                let li = self.q.line_info;
                return li.get_location(self.offset()).line_number
                    == li.get_location(ast.t_offset(ast[a].operator)).line_number;
            }
        }
        false
    }

    fn for_mixin_member(&mut self, node: Id<MixinDeclaration>) {
        K::add_mixin_member_keywords(&mut self.out);
        self.add_lexical(DeclConfig { must_be_type: true, ..Default::default() }, node.raw());
        let element = self.q.declared_element(node.raw()).and_then(|e| e.cast::<InterfaceElement>());
        self.suggest_overrides_for(element, false);
    }

    fn for_name_in_declared_variable_pattern(&mut self, node: NodeId) {
        let ast = self.ast();
        let p = ast.cast::<DeclaredVariablePattern>(node).unwrap();
        let parent = ast.parent(node);
        if parent.is_some_and(|pa| ast.is::<GuardedPattern>(pa)) {
            if !ast.t_synthetic(ast[p].name) {
                self.kw("when");
                if let Some(nt) = ast[p].type_.and_then(|t| ast.cast::<NamedType>(t.raw())) {
                    let name = ast.t_lexeme(ast[nt].name).to_string();
                    self.ident(false).add_suggestions_from_type_name(&mut self.out, &name);
                }
            }
        } else if let Some(field) = parent.filter(|pa| ast.is::<PatternField>(*pa)) {
            if let Some(outer) = ast.parent(field).filter(|o| ast.is::<DartPattern>(*o)) {
                self.location("PatternField_pattern");
                let is_keyword_needed = ast[p].type_.is_none() && ast[p].keyword.is_none();
                self.for_pattern_field_name_in_pattern(outer, is_keyword_needed, false);
            }
        }
    }

    /// Dart `_forPattern`.
    fn for_pattern(&mut self, node: NodeId, must_be_const: bool) {
        let ast = self.ast();
        let covering = self.q.covering;
        let o = self.offset();
        if let Some(c) = ast.cast::<CaseClause>(node) {
            let pattern = ast[ast[c].guarded_pattern].pattern.raw();
            if let Some(cp) = ast.cast::<ConstantPattern>(pattern) {
                if let Some(i) = ast.cast::<SimpleIdentifier>(ast[cp].expression.raw()) {
                    if !ast.n_synthetic(i.raw()) && o < ast.offset(i) {
                        self.add_lexical(DeclConfig { must_be_type: true, ..Default::default() }, node);
                        return;
                    }
                }
            }
            if let Some(w) = ast.cast::<WildcardPattern>(pattern) {
                if o < ast.t_offset(ast[w].name) {
                    self.add_lexical(DeclConfig { must_be_type: true, ..Default::default() }, node);
                    return;
                }
            }
        }
        if let Some(i) = ast.cast::<SimpleIdentifier>(covering) {
            if ast.n_synthetic(i.raw()) {
                if ast.parent(covering).and_then(|p| ast.parent(p)).is_some_and(|g| ast.is::<PatternField>(g)) {
                    K::add_variable_pattern_keywords(&mut self.out);
                    self.out.collector.prefer_constants = true;
                    let cfg = DeclConfig {
                        must_be_constant: must_be_const,
                        must_be_static: in_static_context(ast, node),
                        object_pattern_allowed: true,
                        ..Default::default()
                    };
                    self.add_lexical(cfg, node);
                    return;
                }
            } else if ast.parent(covering).is_some_and(|p| ast.is::<ConstantPattern>(p)) {
                K::add_pattern_keywords(&mut self.out);
                self.out.collector.prefer_constants = true;
                self.add_lexical(DeclConfig { must_be_non_void: true, ..Default::default() }, node);
                return;
            }
        }
        if ast.is::<NamedType>(covering) {
            if let Some(p) = ast.parent(covering) {
                match ast.kind(p) {
                    NodeKind::DeclaredVariablePattern => {
                        self.location("DeclaredVariablePattern_type");
                        self.visit(covering);
                        return;
                    }
                    NodeKind::ObjectPattern => {
                        self.location("ObjectPattern_type");
                        self.visit(covering);
                        return;
                    }
                    NodeKind::WildcardPattern => {
                        self.location("WildcardPattern_type");
                        self.visit(covering);
                        return;
                    }
                    _ => {}
                }
            }
        }
        K::add_pattern_keywords(&mut self.out);
        self.out.collector.prefer_constants = true;
        let cfg = DeclConfig {
            must_be_constant: must_be_const,
            must_be_static: in_static_context(ast, node),
            object_pattern_allowed: true,
            ..Default::default()
        };
        self.add_lexical(cfg, node);
    }

    fn for_pattern_field_name(&mut self, node: NodeId, is_keyword_needed: bool, is_type_needed: bool) {
        let ast = self.ast();
        if let Some(pattern) = ast.parent(node).and_then(|p| ast.parent(p)).filter(|p| ast.is::<DartPattern>(*p)) {
            self.for_pattern_field_name_in_pattern(pattern, is_keyword_needed, is_type_needed);
        }
    }

    fn for_pattern_field_name_in_pattern(&mut self, pattern: NodeId, is_keyword_needed: bool, is_type_needed: bool) {
        let ast = self.ast();
        let (ty, fields) = if let Some(o) = ast.cast::<ObjectPattern>(pattern) {
            (self.q.tables.annotation_type.get(ast[o].type_.raw()).copied(), ast[o].fields)
        } else if let Some(r) = ast.cast::<RecordPattern>(pattern) {
            (
                self.q.tables.pattern_info.get(pattern).and_then(|i| i.matched_value_type),
                ast[r].fields,
            )
        } else {
            return;
        };
        let Some(ty) = ty else {
            return;
        };
        let excluded = field_names(ast, fields);
        let q = self.q;
        self.decl(DeclConfig { must_be_non_void: true, prefer_non_invocation: true, ..Default::default() });
        self.decl
            .as_mut()
            .unwrap()
            .add_getters(q, &mut self.out, ty, &excluded, is_keyword_needed, is_type_needed);
    }

    fn for_redirecting_constructor_invocation(&mut self, constructor: Id<ConstructorDeclaration>) {
        let ast = self.ast();
        let container = ast.parent(constructor).and_then(|p| ast.parent(p));
        let ctx = self.q.ctx;
        let this_type = container
            .filter(|c| {
                ast.is::<ClassDeclaration>(*c) || ast.is::<EnumDeclaration>(*c) || ast.is::<ExtensionTypeDeclaration>(*c)
            })
            .and_then(|c| self.q.declared_element(c))
            .and_then(|e| e.cast::<InterfaceElement>())
            .map(|e| ctx.interface_this_type(e));
        if let Some(t) = this_type {
            let name = ast[constructor].name.map(|n| ast.t_lexeme(n).to_string());
            let q = self.q;
            self.decl(DeclConfig { must_be_constant: ast[constructor].const_keyword.is_some(), ..Default::default() });
            self.decl.as_mut().unwrap().add_constructor_names_for_type(q, &mut self.out, t, name.as_deref());
        }
    }

    fn for_statement(&mut self, node: NodeId) {
        self.for_expression(node, ExprOpts::default());
        K::add_statement_keywords(self.q, &mut self.out, node);
    }

    /// Dart `_forTypeAnnotation`.
    fn for_type_annotation(&mut self, node: NodeId, opts: TypeOpts) {
        let ast = self.ast();
        if !opts.no_dynamic
            && !(opts.must_be_extensible || opts.must_be_implementable || opts.must_be_mixable || opts.is_in_declaration)
        {
            self.kw("dynamic");
            if !opts.no_void && !opts.must_be_non_void {
                self.kw("void");
            }
        }
        if let Some(nt) = ast.cast::<NamedType>(node) {
            if let Some(ip) = ast[nt].import_prefix {
                if let Some(prefix) = self.q.element(ip.raw()).filter(|e| e.tag() == Tag::Prefix) {
                    let q = self.q;
                    self.decl(DeclConfig {
                        must_be_extendable: opts.must_be_extensible,
                        must_be_implementable: opts.must_be_implementable,
                        must_be_mixable: opts.must_be_mixable,
                        must_be_non_void: opts.must_be_non_void,
                        excluded_nodes: opts.excluded_nodes.clone(),
                        exclude_type_names: opts.exclude_type_names,
                        ..Default::default()
                    });
                    self.decl.as_mut().unwrap().add_declarations_through_import_prefix(q, &mut self.out, prefix);
                }
                return;
            }
        }
        let cfg = DeclConfig {
            must_be_extendable: opts.must_be_extensible,
            must_be_implementable: opts.must_be_implementable,
            must_be_mixable: opts.must_be_mixable,
            must_be_type: true,
            must_be_non_void: opts.must_be_non_void,
            excluded_nodes: opts.excluded_nodes,
            ..Default::default()
        };
        self.add_lexical(cfg, node);
    }

    fn handled_incomplete_preceding_unit_member(&mut self, unit: Id<CompilationUnit>, preceding: NodeId) -> bool {
        let ast = self.ast();
        let _ = unit;
        match ast.kind(preceding) {
            NodeKind::ClassDeclaration => {
                let c = ast.cast::<ClassDeclaration>(preceding).unwrap();
                let no_body = match ast.cast::<BlockClassBody>(ast[c].body.raw()) {
                    Some(b) => ast.t_synthetic(ast[b].left_bracket) && ast.t_synthetic(ast[b].right_bracket),
                    None => true,
                };
                if no_body {
                    self.location("CompilationUnit_declaration");
                    K::add_class_declaration_keywords(self.q, &mut self.out, c);
                    return true;
                }
            }
            NodeKind::ExtensionTypeDeclaration => {
                let e = ast.cast::<ExtensionTypeDeclaration>(preceding).unwrap();
                let no_body = match ast.cast::<BlockClassBody>(ast[e].body.raw()) {
                    Some(b) => ast.t_synthetic(ast[b].left_bracket) && ast.t_synthetic(ast[b].right_bracket),
                    None => true,
                };
                if no_body {
                    self.location("CompilationUnit_declaration");
                    self.visit_extension_type_declaration(preceding);
                    return true;
                }
            }
            NodeKind::FunctionDeclaration => {
                let f = ast.cast::<FunctionDeclaration>(preceding).unwrap();
                self.location("CompilationUnit_declaration");
                let body = ast[ast[f].function_expression].body.raw();
                if function_body_is_empty(ast, body) {
                    K::add_function_body_modifiers(self.q, &mut self.out, Some(body));
                }
            }
            NodeKind::ImportDirective => {
                let d = ast.cast::<ImportDirective>(preceding).unwrap();
                if ast.t_synthetic(ast[d].semicolon) {
                    self.visit_import_directive(preceding);
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    /// Dart `_handledPossibleClosure`.
    fn handled_possible_closure(&mut self, expression: NodeId) -> bool {
        let ast = self.ast();
        let next = next_non_synthetic(ast, ast.end_tok(expression));
        if self.offset() > ast.t_offset(next) {
            return false;
        }
        if let Some(p) = ast.cast::<ParenthesizedExpression>(expression) {
            if ast.is::<SimpleIdentifier>(ast[p].expression.raw()) {
                K::add_function_body_modifiers(self.q, &mut self.out, None);
                return true;
            }
        } else if let Some(r) = ast.cast::<RecordLiteral>(expression) {
            for &f in ast.list_raw(ast[r].fields) {
                if !ast.is::<SimpleIdentifier>(f) {
                    return false;
                }
            }
            K::add_function_body_modifiers(self.q, &mut self.out, None);
            return true;
        }
        false
    }

    /// Dart `_handledRecovery`.
    fn handled_recovery(&mut self, declaration: NodeId) -> bool {
        let ast = self.ast();
        let Some(unit) = ast.parent(declaration).and_then(|p| ast.cast::<CompilationUnit>(p)) else {
            return false;
        };
        let o = self.offset();
        let begin = ast.begin(declaration);
        if o <= ast.t_end(begin) {
            let (before, _) = members_around_member(ast, unit, declaration);
            if let Some(b) = before {
                if self.handled_incomplete_preceding_unit_member(unit, b) {
                    return true;
                }
            }
        }
        let next = ast.t_next(begin);
        let last = ast.end_tok(declaration);
        let single = ast.t_kw_or_ident(begin)
            && ast.t_synthetic(last)
            && (next == last || (ast.t_synthetic(next) && ast.t_next(next) == last));
        if single && o <= ast.t_end(begin) {
            let (before, after) = members_around_member(ast, unit, declaration);
            self.for_compilation_unit_member(unit, before, after);
            return true;
        }
        false
    }

    /// Dart `_locationFor`.
    fn location_for(&self, list: Id<ArgumentList>, is_named: bool) -> String {
        let ast = self.ast();
        let context = match ast.parent(list).map(|p| ast.kind(p)) {
            Some(NodeKind::Annotation) => "annotation",
            Some(NodeKind::EnumConstantArguments) => "enumConstantArguments",
            Some(NodeKind::ExtensionOverride) => "extensionOverride",
            Some(NodeKind::FunctionExpressionInvocation) => "function",
            Some(NodeKind::InstanceCreationExpression) => "constructor",
            Some(NodeKind::MethodInvocation) => "method",
            Some(NodeKind::RedirectingConstructorInvocation) | Some(NodeKind::SuperConstructorInvocation) => {
                "constructorRedirect"
            }
            _ => "",
        };
        let kind = if is_named { "named" } else { "unnamed" };
        format!("ArgumentList_{context}_{kind}")
    }

    /// Dart `_staticMemberTargetElement`.
    fn static_member_target_element(&self, target: NodeId) -> Option<ElementId> {
        let ast = self.ast();
        if ast.is::<Identifier>(target) {
            return self.q.element(target);
        }
        if let Some(t) = ast.cast::<TypeLiteral>(target) {
            return self.q.element(ast[t].type_.raw());
        }
        None
    }

    fn suggest_overrides_for(&mut self, element: Option<dartr_element::EId<InterfaceElement>>, skip_at: bool) {
        if self.out.budget_is_empty() {
            self.out.collector.is_incomplete = true;
            return;
        }
        if self.suggest_overrides {
            if let Some(e) = element {
                let offset = self.offset();
                compute_overrides_for(self.q, &mut self.out, e, (offset, 0), skip_at);
            }
        }
    }

    fn suggest_record_literal_named_fields(
        &mut self,
        context: Option<TypeId>,
        container: NodeId,
        record_literal: Option<NodeId>,
        is_new_field: bool,
    ) {
        let ast = self.ast();
        let ctx = self.q.ctx;
        let context = self.resolve_future_or(context);
        let Some(t) = context else {
            return;
        };
        let TypeKind::Record { named, .. } = *ctx.ty(t) else {
            return;
        };
        let o = self.offset();
        let displaced = {
            let mut found = None;
            let mut token = ast.begin(container);
            let end = ast.end_tok(container);
            loop {
                if !ast.t_synthetic(token) && o <= ast.t_offset(token) {
                    found = Some(token);
                    break;
                }
                if token == end || ast.t_is_eof(token) {
                    break;
                }
                token = ast.t_next(token);
            }
            found
        };
        let Some(displaced) = displaced else {
            return;
        };
        let included: Vec<String> = match record_literal.and_then(|r| ast.cast::<RecordLiteral>(r)) {
            Some(r) => ast
                .list_raw(ast[r].fields)
                .iter()
                .filter_map(|f| ast.cast::<RecordLiteralNamedField>(*f))
                .map(|f| ast.t_lexeme(ast[f].name).to_string())
                .collect(),
            None => Vec::new(),
        };
        for field in ctx.list(named) {
            let name = ctx.name_str(field.name).to_string();
            if included.contains(&name) {
                continue;
            }
            let score = self.out.score(&name);
            if score == -1.0 {
                continue;
            }
            let kind = if is_new_field {
                let ty = ast.t_ty(displaced);
                Kind::RecordLiteralNamedField {
                    name,
                    field_type: field.ty,
                    append_colon: true,
                    append_comma: ty != TokenType::COMMA && ty != TokenType::CLOSE_PAREN,
                }
            } else {
                Kind::RecordLiteralNamedField {
                    name,
                    field_type: field.ty,
                    append_colon: false,
                    append_comma: false,
                }
            };
            self.out.add(Candidate::new(kind, score));
        }
    }

    fn try_annotation_at_end_of_class_body(&mut self, node: NodeId) -> bool {
        let ast = self.ast();
        if let Some(displaced) = self.q.target.entity_token() {
            if ast.t_ty(displaced) == TokenType::CLOSE_CURLY_BRACKET {
                if let Some(identifier) = ast.t_prev(displaced) {
                    if ast.t_ty(identifier) == TokenType::IDENTIFIER
                        && ast.t_prev(identifier).is_some_and(|a| ast.t_ty(a) == TokenType::AT)
                    {
                        self.location("Annotation_name");
                        self.for_annotation(node);
                        self.try_override_annotation(identifier, node);
                        return true;
                    }
                }
            }
        }
        false
    }

    fn try_override_annotation(&mut self, identifier: TokenId, node: NodeId) {
        let lexeme = self.ast().t_lexeme(identifier).to_string();
        if !lexeme.is_empty() && "override".starts_with(&lexeme) {
            let element = self.q.declared_element(node).and_then(|e| e.cast::<InterfaceElement>());
            if element.is_some() {
                self.suggest_overrides_for(element, true);
            }
        }
    }

    fn visit_for_each_parts(&mut self, node: NodeId) {
        let ast = self.ast();
        let in_keyword = if let Some(p) = ast.cast::<ForEachPartsWithDeclaration>(node) {
            ast[p].in_keyword
        } else if let Some(p) = ast.cast::<ForEachPartsWithIdentifier>(node) {
            ast[p].in_keyword
        } else if let Some(p) = ast.cast::<ForEachPartsWithPattern>(node) {
            ast[p].in_keyword
        } else {
            return;
        };
        if ast.t_covers(Some(in_keyword), self.offset()) {
            let mut previous = ast.find_previous(in_keyword);
            if let Some(p) = previous {
                if ast.t_synthetic(p) && ast.t_lexeme(p) == "in" {
                    previous = ast.find_previous(p);
                }
            }
            if previous.is_some_and(|p| ast.t_ty(p) == TokenType::EQ) {
                for k in ["const", "false", "null", "true"] {
                    self.kw(k);
                }
            } else {
                self.kw("in");
            }
        } else if !ast.t_synthetic(in_keyword) {
            self.kw("await");
            let cfg = DeclConfig { must_be_static: in_static_context(ast, node), must_be_non_void: true, ..Default::default() };
            self.add_lexical(cfg, node);
        }
    }
}

/// Dart `Argument.argumentExpression`.
fn argument_expression(ast: &Ast, argument: NodeId) -> NodeId {
    if let Some(n) = ast.cast::<NamedArgument>(argument) {
        return ast[n].argument_expression.raw();
    }
    argument
}

/// Dart `argumentsBeforeAndAfterOffset`.
fn arguments_before_and_after(ast: &Ast, list: Id<ArgumentList>, offset: u32) -> (Option<NodeId>, Option<NodeId>) {
    let mut previous: Option<NodeId> = None;
    for &argument in ast.list_raw(ast[list].arguments) {
        let expression = argument_expression(ast, argument);
        if offset < ast.offset(argument) {
            return (previous, Some(expression));
        } else if offset == ast.offset(argument) && previous.is_some_and(|p| offset == ast.end(p)) {
            return (previous, Some(expression));
        }
        previous = Some(expression);
    }
    (previous, None)
}

/// Dart `argumentContext(argumentIndex)`.
fn argument_context(ast: &Ast, list: Id<ArgumentList>, argument_index: i64) -> (usize, Vec<String>) {
    let mut positional = 0;
    let mut used = Vec::new();
    for (i, &argument) in ast.list_raw(ast[list].arguments).iter().enumerate() {
        if let Some(n) = ast.cast::<NamedArgument>(argument) {
            used.push(ast.t_lexeme(ast[n].name).to_string());
        } else if (i as i64) < argument_index {
            positional += 1;
        }
    }
    if let Some(s) = ast.parent(list).and_then(|p| ast.cast::<SuperConstructorInvocation>(p)) {
        if let Some(c) = ast.parent(s).and_then(|p| ast.cast::<ConstructorDeclaration>(p)) {
            let parameters = ast[c].parameters;
            for &p in ast.list_raw(ast[parameters].parameters) {
                if let Some(sp) = ast.cast::<SuperFormalParameter>(p) {
                    if ast[sp].kind.is_named() {
                        used.push(ast.t_lexeme(ast[sp].name).to_string());
                    }
                }
            }
        }
    }
    (positional, used)
}

/// Dart `isFollowedByComma` of a node.
fn is_followed_by_comma(ast: &Ast, node: NodeId) -> bool {
    let next = ast.t_next(ast.end_tok(node));
    ast.t_ty(next) == TokenType::COMMA && !ast.t_synthetic(next)
}

/// The type name token of a class name part.
pub(crate) fn class_name_token(ast: &Ast, part: NodeId) -> TokenId {
    if let Some(n) = ast.cast::<NameWithTypeParameters>(part) {
        return ast[n].type_name;
    }
    if let Some(p) = ast.cast::<PrimaryConstructorDeclaration>(part) {
        return ast[p].type_name;
    }
    ast.begin(part)
}

/// Dart `ClassNamePart.hasConst`.
fn name_part_has_const(ast: &Ast, part: NodeId) -> bool {
    ast.cast::<PrimaryConstructorDeclaration>(part)
        .is_some_and(|p| ast[p].const_keyword.is_some())
}

/// Dart `FunctionBody.isEmpty` (an `EmptyFunctionBody`).
fn function_body_is_empty(ast: &Ast, body: NodeId) -> bool {
    ast.is::<EmptyFunctionBody>(body)
}

/// Dart `isFlutterWidgetParameter`.
fn is_flutter_widget_parameter(ctx: &dartr_element::Ctx<'_>, parameter: ElemRef) -> bool {
    let base = member::base_element(ctx, parameter);
    let Some(enclosing) = elem::enclosing(ctx, base) else {
        return false;
    };
    if enclosing.tag() != Tag::Constructor {
        return false;
    }
    let Some(class) = elem::enclosing(ctx, enclosing).and_then(|c| c.cast::<InterfaceElement>()) else {
        return false;
    };
    is_widget_type(ctx, ctx.interface_this_type(class))
}

/// Dart `isWidget` of an interface type (Flutter `Widget` subtype).
fn is_widget_type(ctx: &dartr_element::Ctx<'_>, t: TypeId) -> bool {
    let Some(e) = ctx.interface_element(t) else {
        return false;
    };
    let is_widget = |e: dartr_element::EId<InterfaceElement>| {
        ctx.element_name(e.raw()) == Some("Widget")
            && elem::library_of(ctx, e.raw())
                .is_some_and(|l| elem::library_uri(ctx, l) == "package:flutter/src/widgets/framework.dart")
    };
    if is_widget(e) {
        return true;
    }
    ctx.element_all_supertypes(e)
        .iter()
        .filter_map(|s| ctx.interface_element(*s))
        .any(is_widget)
}

/// Dart `AstNodeImpl.constantContext(includeSelf: true) != null`.
fn constant_context_including_self(ast: &Ast, node: NodeId) -> bool {
    use dartr_resolver::ast_ext::{has_const_keyword, in_constant_context};
    let is_const = |t: Option<dartr_syntax::TokenId>| t.is_some_and(|t| ast.t_lexeme(t) == "const");
    match ast.kind(node) {
        NodeKind::Annotation | NodeKind::EnumConstantArguments | NodeKind::SwitchCase => return true,
        NodeKind::ConstantPattern => {
            return ast.cast::<ConstantPattern>(node).is_some_and(|c| ast[c].const_keyword.is_some());
        }
        NodeKind::VariableDeclarationList => {
            return ast.cast::<VariableDeclarationList>(node).is_some_and(|l| is_const(ast[l].keyword));
        }
        NodeKind::InstanceCreationExpression => {
            if ast.cast::<InstanceCreationExpression>(node).is_some_and(|i| is_const(ast[i].keyword)) {
                return true;
            }
        }
        NodeKind::DotShorthandConstructorInvocation => {
            if let Some(d) = ast.cast::<DotShorthandConstructorInvocation>(node) {
                if has_const_keyword(ast, node, ast[d].const_keyword) {
                    return true;
                }
            }
        }
        NodeKind::RecordLiteral => {
            if let Some(r) = ast.cast::<RecordLiteral>(node) {
                if has_const_keyword(ast, node, ast[r].const_keyword) {
                    return true;
                }
            }
        }
        NodeKind::ListLiteral => {
            if let Some(l) = ast.cast::<ListLiteral>(node) {
                if has_const_keyword(ast, node, ast[l].const_keyword) {
                    return true;
                }
            }
        }
        NodeKind::SetOrMapLiteral => {
            if let Some(l) = ast.cast::<SetOrMapLiteral>(node) {
                if has_const_keyword(ast, node, ast[l].const_keyword) {
                    return true;
                }
            }
        }
        _ => {}
    }
    in_constant_context(ast, node)
}
