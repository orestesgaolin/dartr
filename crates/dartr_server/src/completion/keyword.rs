// Dart source: pkg/analysis_server/lib/src/services/completion/dart/keyword_helper.dart
// Dart source: pkg/analysis_server/lib/src/services/completion/dart/label_helper.dart
// Dart source: pkg/analysis_server/lib/src/services/completion/dart/identifier_helper.dart
// Dart source: pkg/analysis_server/lib/src/services/completion/dart/override_helper.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/name_suggestion.dart (getCamelWordCombinations)
// Dart source: pkg/analyzer_plugin/lib/src/utilities/string_utilities.dart (getCamelWords)

//! The keywords, labels, identifiers and overrides that completion
//! suggests.

#[allow(unused_imports)]
use dartr_typesystem::TypeExt;
use dartr_ast::*;
use dartr_element::{EId, ElemRef, ElementId, InterfaceElement, Tag};
use dartr_syntax::{TokenId, TokenType};
use dartr_typesystem::inheritance_manager3::{InheritanceManager3, Name};
use dartr_typesystem::member;

use super::candidate::{Candidate, Kind, display_name, utf16_len};
use super::target::TokenExt;
use super::{Out, Request, elem};

/// Dart `String.withoutCaret`.
fn without_caret(text: &str) -> (String, Option<usize>) {
    match text.find('^') {
        Some(i) => {
            let raw = format!("{}{}", &text[..i], &text[i + 1..]);
            (raw, Some(utf16_len(&text[..i])))
        }
        None => (text.to_string(), None),
    }
}

/// Dart `KeywordHelper`.
pub struct KeywordHelper;

impl KeywordHelper {
    /// Dart `_isAbsentOrIn`.
    fn absent_or_in(q: &Request<'_, '_>, token: Option<TokenId>) -> bool {
        match token {
            None => true,
            Some(t) => q.ast.t_offset(t) <= q.offset && q.offset <= q.ast.t_end(t),
        }
    }

    /// Dart `addKeyword`.
    pub fn add_keyword(out: &mut Out, keyword: &str) {
        let score = out.score(keyword);
        if score != -1.0 {
            out.add(Candidate::new(
                Kind::Keyword {
                    completion: keyword.to_string(),
                    selection_offset: utf16_len(keyword),
                },
                score,
            ));
        }
    }

    /// Dart `addKeywordAndText`.
    pub fn add_keyword_and_text(out: &mut Out, keyword: &str, annotated: &str) {
        let score = out.score(keyword);
        if score != -1.0 {
            let (raw, caret) = without_caret(annotated);
            let selection_offset = utf16_len(keyword) + caret.unwrap_or(utf16_len(&raw));
            out.add(Candidate::new(
                Kind::Keyword {
                    completion: format!("{keyword}{raw}"),
                    selection_offset,
                },
                score,
            ));
        }
    }

    /// Dart `addText`.
    pub fn add_text(out: &mut Out, annotated: &str) {
        let (raw, caret) = without_caret(annotated);
        let score = out.score(&raw);
        if score != -1.0 {
            let selection_offset = caret.unwrap_or(utf16_len(&raw));
            out.add(Candidate::new(
                Kind::Keyword {
                    completion: raw,
                    selection_offset,
                },
                score,
            ));
        }
    }

    /// Dart `addClassDeclarationKeywords`.
    pub fn add_class_declaration_keywords(q: &Request<'_, '_>, out: &mut Out, node: Id<ClassDeclaration>) {
        let ast = q.ast;
        if Self::absent_or_in(q, ast[node].extends_clause.map(|c| ast[c].extends_keyword)) {
            Self::add_keyword(out, "extends");
        }
        if Self::absent_or_in(q, ast[node].with_clause.map(|c| ast[c].with_keyword)) {
            Self::add_keyword(out, "with");
        }
        if Self::absent_or_in(q, ast[node].implements_clause.map(|c| ast[c].implements_keyword)) {
            Self::add_keyword(out, "implements");
        }
    }

    /// Dart `addClassMemberKeywords`.
    pub fn add_class_member_keywords(q: &Request<'_, '_>, out: &mut Out) {
        for k in ["const", "covariant", "dynamic", "factory", "final", "get"] {
            Self::add_keyword(out, k);
        }
        if q.feature_enabled(dartr_parser::experimental_flags::ExperimentalFlag::PrimaryConstructors) {
            let ast = q.ast;
            let parent = ast.this_or_ancestor_matching(q.covering, |a, n| {
                a.is::<ClassDeclaration>(n) || a.is::<EnumDeclaration>(n) || a.is::<ExtensionTypeDeclaration>(n)
            });
            if parent.is_some() {
                Self::add_keyword(out, "new");
            }
        }
        for k in ["operator", "set", "static", "var", "void", "late"] {
            Self::add_keyword(out, k);
        }
    }

    /// Dart `addClassModifiers`.
    pub fn add_class_modifiers(q: &Request<'_, '_>, out: &mut Out, node: Id<ClassDeclaration>) {
        use dartr_parser::experimental_flags::ExperimentalFlag as F;
        let ast = q.ast;
        let n = &ast[node];
        if Self::absent_or_in(q, n.abstract_keyword) && n.sealed_keyword.is_none() {
            Self::add_keyword(out, "abstract");
        }
        if q.feature_enabled(F::ClassModifiers) && q.feature_enabled(F::SealedClass) {
            if n.base_keyword.is_none()
                && n.final_keyword.is_none()
                && n.interface_keyword.is_none()
                && n.mixin_keyword.is_none()
                && n.sealed_keyword.is_none()
            {
                if n.abstract_keyword.is_none() {
                    Self::add_keyword(out, "sealed");
                } else {
                    for k in ["base", "final", "interface", "mixin"] {
                        Self::add_keyword(out, k);
                    }
                }
            }
            if n.base_keyword.is_some() && Self::absent_or_in(q, n.mixin_keyword) {
                Self::add_keyword(out, "mixin");
            }
            if n.mixin_keyword.is_some() && Self::absent_or_in(q, n.base_keyword) {
                Self::add_keyword(out, "base");
            }
        }
    }

    /// Dart `addCollectionElementKeywords`.
    pub fn add_collection_element_keywords(
        q: &Request<'_, '_>,
        out: &mut Out,
        literal: NodeId,
        elements: &[NodeId],
        must_be_const: bool,
        must_be_static: bool,
    ) {
        let ast = q.ast;
        Self::add_keyword(out, "for");
        Self::add_keyword(out, "if");
        if let Some(preceding) = element_before(ast, elements, q.offset) {
            let next = ast.t_next(ast.end_tok(preceding));
            if ast.t_synthetic(next) || q.offset <= ast.t_offset(next) {
                if could_have_trailing_else(ast, Some(preceding)) {
                    Self::add_keyword(out, "else");
                } else if let Some(index) = elements.iter().position(|e| *e == preceding) {
                    if index > 0 && could_have_trailing_else(ast, Some(elements[index - 1])) {
                        Self::add_keyword(out, "else");
                    }
                }
            }
        }
        Self::add_expression_keywords(q, out, Some(literal), true, true, true, must_be_const, must_be_static);
    }

    /// Dart `addCompilationUnitDeclarationKeywords`.
    pub fn add_compilation_unit_declaration_keywords(q: &Request<'_, '_>, out: &mut Out) {
        use dartr_parser::experimental_flags::ExperimentalFlag as F;
        for k in [
            "abstract", "class", "const", "covariant", "dynamic", "enum", "external", "final", "mixin",
            "typedef", "var", "void",
        ] {
            Self::add_keyword(out, k);
        }
        if q.feature_enabled(F::ExtensionMethods) {
            Self::add_keyword(out, "extension");
        }
        Self::add_keyword(out, "late");
        if q.feature_enabled(F::ClassModifiers) {
            Self::add_keyword(out, "base");
            Self::add_keyword(out, "interface");
        }
        if q.feature_enabled(F::SealedClass) {
            Self::add_keyword(out, "sealed");
        }
    }

    /// Dart `addConstantExpressionKeywords`.
    pub fn add_constant_expression_keywords(out: &mut Out, in_constant_context: bool) {
        Self::add_keyword(out, "false");
        Self::add_keyword(out, "null");
        Self::add_keyword(out, "true");
        if !in_constant_context {
            Self::add_keyword(out, "const");
        }
    }

    /// Dart `addConstructorInitializerKeywords`.
    pub fn add_constructor_initializer_keywords(
        q: &Request<'_, '_>,
        out: &mut Out,
        constructor: Id<ConstructorDeclaration>,
        initializer: Option<NodeId>,
    ) {
        let ast = q.ast;
        Self::add_keyword(out, "assert");
        let initializers = ast.list_raw(ast[constructor].initializers);
        if initializer.is_none() || initializers.last().copied() == initializer {
            let last_non_synthetic = initializers.last().map(|&last| {
                if ast.t_synthetic(ast.begin(last)) && initializers.len() > 1 {
                    initializers[initializers.len() - 2]
                } else {
                    last
                }
            });
            if last_non_synthetic == initializer
                || last_non_synthetic.is_none_or(|l| {
                    !ast.is::<SuperConstructorInvocation>(l) && !ast.is::<RedirectingConstructorInvocation>(l)
                })
            {
                let in_extension_type = ast
                    .parent(constructor)
                    .and_then(|p| ast.parent(p))
                    .is_some_and(|p| ast.is::<ExtensionTypeDeclaration>(p));
                if !in_extension_type {
                    Self::add_keyword(out, "super");
                }
                Self::add_keyword(out, "this");
            }
        } else if let Some(f) = initializer.and_then(|i| ast.cast::<ConstructorFieldInitializer>(i)) {
            let equals = ast[f].equals;
            if ast.t_end(equals) <= q.offset && q.offset <= ast.t_offset(ast.t_next(equals)) {
                Self::add_keyword(out, "this");
            }
        }
    }

    /// Dart `addDirectiveKeywords`.
    pub fn add_directive_keywords(q: &Request<'_, '_>, out: &mut Out, unit: Id<CompilationUnit>, before: Option<NodeId>) {
        let ast = q.ast;
        let directives = ast.list_raw(ast[unit].directives);
        if before.is_none() && !directives.iter().any(|d| ast.is::<LibraryDirective>(*d)) {
            Self::add_keyword(out, "library");
        }
        Self::add_keyword_and_text(out, "import", " '^';");
        Self::add_keyword_and_text(out, "export", " '^';");
        Self::add_keyword_and_text(out, "part", " '^';");
        if directives.is_empty() {
            Self::add_text(out, "part of '^';");
        }
    }

    /// Dart `addEnumDeclarationKeywords`.
    pub fn add_enum_declaration_keywords(q: &Request<'_, '_>, out: &mut Out, node: Id<EnumDeclaration>) {
        let ast = q.ast;
        if Self::absent_or_in(q, ast[node].with_clause.map(|c| ast[c].with_keyword)) {
            Self::add_keyword(out, "with");
        }
        if Self::absent_or_in(q, ast[node].implements_clause.map(|c| ast[c].implements_keyword)) {
            Self::add_keyword(out, "implements");
        }
    }

    /// Dart `addEnumMemberKeywords`.
    pub fn add_enum_member_keywords(q: &Request<'_, '_>, out: &mut Out) {
        for k in ["const", "dynamic", "final", "get", "late"] {
            Self::add_keyword(out, k);
        }
        if q.feature_enabled(dartr_parser::experimental_flags::ExperimentalFlag::PrimaryConstructors) {
            Self::add_keyword(out, "new");
        }
        for k in ["operator", "set", "static", "var", "void"] {
            Self::add_keyword(out, k);
        }
    }

    /// Dart `addExpressionKeywords`.
    #[allow(clippy::too_many_arguments)]
    pub fn add_expression_keywords(
        q: &Request<'_, '_>,
        out: &mut Out,
        node: Option<NodeId>,
        can_be_bool: bool,
        can_be_null: bool,
        can_suggest_const: bool,
        must_be_constant: bool,
        must_be_static: bool,
    ) {
        let ast = q.ast;
        let patterns = q.feature_enabled(dartr_parser::experimental_flags::ExperimentalFlag::Patterns);
        if can_be_bool {
            Self::add_keyword(out, "false");
        }
        if can_be_null {
            Self::add_keyword(out, "null");
        }
        if can_be_bool {
            Self::add_keyword(out, "true");
        }
        let Some(node) = node else {
            if !must_be_constant && patterns {
                Self::add_keyword(out, "switch");
            }
            return;
        };
        let const_is_valid = |node: NodeId| -> bool {
            let mut node = node;
            if ast.is::<CollectionElement>(node) && !ast.is::<Expression>(node) {
                match ast.parent(node) {
                    Some(p) => node = p,
                    None => return false,
                }
            }
            if ast.is::<Expression>(node) {
                return !dartr_resolver::ast_ext::in_constant_context(ast, node);
            }
            match ast.kind(node) {
                NodeKind::Block
                | NodeKind::EmptyStatement
                | NodeKind::ExpressionStatement
                | NodeKind::IfStatement
                | NodeKind::PatternVariableDeclaration
                | NodeKind::SwitchPatternCase
                | NodeKind::SwitchStatement
                | NodeKind::WhenClause => true,
                NodeKind::RecordPattern => {
                    let r = ast.cast::<RecordPattern>(node).unwrap();
                    ast.list(ast[r].fields).is_empty()
                }
                NodeKind::VariableDeclaration => !variable_declaration_is_const(ast, node),
                NodeKind::VariableDeclarationStatement => {
                    let s = ast.cast::<VariableDeclarationStatement>(node).unwrap();
                    !variable_list_is_const(ast, ast[s].variables)
                }
                _ => false,
            }
        };
        let switch_is_valid = |node: NodeId| -> bool {
            let mut node = node;
            if ast.is::<SimpleIdentifier>(node)
                && ast
                    .parent(node)
                    .is_some_and(|p| ast.is::<FormalParameterDefaultClause>(p))
            {
                return false;
            }
            if ast.is::<CollectionElement>(node) && !ast.is::<Expression>(node) {
                match ast.parent(node) {
                    Some(p) => node = p,
                    None => return true,
                }
            }
            if let Some(c) = ast.cast::<SwitchPatternCase>(node) {
                if q.offset <= ast.t_offset(ast[c].colon) {
                    return false;
                }
            }
            true
        };
        if can_suggest_const && const_is_valid(node) {
            Self::add_keyword(out, "const");
        }
        if !must_be_constant && !must_be_static {
            Self::add_keyword(out, "super");
            Self::add_keyword(out, "this");
        }
        if in_async_method_or_function(ast, node) || in_async_star_or_sync_star(ast, node) {
            Self::add_keyword(out, "await");
        }
        if !must_be_constant && switch_is_valid(node) && patterns {
            Self::add_keyword(out, "switch");
        }
    }

    /// Dart `addExtensionDeclarationKeywords`.
    pub fn add_extension_declaration_keywords(q: &Request<'_, '_>, out: &mut Out, node: Id<ExtensionDeclaration>) {
        let ast = q.ast;
        let on = ast[node].on_clause;
        if on.is_none_or(|c| ast.t_synthetic(ast[c].on_keyword)) {
            Self::add_keyword(out, "on");
            if ast[node].name.is_none()
                && q.feature_enabled(dartr_parser::experimental_flags::ExperimentalFlag::InlineClass)
            {
                Self::add_text(out, "type");
            }
        }
    }

    /// Dart `addExtensionMemberKeywords`.
    pub fn add_extension_member_keywords(out: &mut Out, is_static: bool) {
        for k in ["const", "dynamic", "final", "get"] {
            Self::add_keyword(out, k);
        }
        if !is_static {
            Self::add_keyword(out, "operator");
        }
        Self::add_keyword(out, "set");
        if !is_static {
            Self::add_keyword(out, "static");
        }
        Self::add_keyword(out, "var");
        Self::add_keyword(out, "void");
    }

    /// Dart `addExtensionTypeMemberKeywords`.
    pub fn add_extension_type_member_keywords(q: &Request<'_, '_>, out: &mut Out, is_static: bool) {
        for k in ["const", "dynamic", "final", "get"] {
            Self::add_keyword(out, k);
        }
        if q.feature_enabled(dartr_parser::experimental_flags::ExperimentalFlag::PrimaryConstructors) {
            Self::add_keyword(out, "new");
        }
        if !is_static {
            Self::add_keyword(out, "operator");
        }
        Self::add_keyword(out, "set");
        if !is_static {
            Self::add_keyword(out, "static");
        }
        Self::add_keyword(out, "var");
        Self::add_keyword(out, "void");
    }

    /// Dart `addFieldDeclarationKeywords`.
    pub fn add_field_declaration_keywords(
        q: &Request<'_, '_>,
        out: &mut Out,
        node: Id<FieldDeclaration>,
        keyword: Option<&str>,
    ) {
        let ast = q.ast;
        let n = &ast[node];
        if Self::absent_or_in(q, n.external_keyword) && keyword != Some("external") {
            Self::add_keyword(out, "external");
        }
        let fields = n.fields;
        if ast[fields].type_.is_none() {
            Self::add_keyword(out, "dynamic");
            Self::add_keyword(out, "void");
        }
        let is_static = n.static_keyword.is_some();
        if !is_static && keyword != Some("static") {
            if Self::absent_or_in(q, n.abstract_keyword) && keyword != Some("abstract") {
                Self::add_keyword(out, "abstract");
            }
            if Self::absent_or_in(q, n.covariant_keyword) && keyword != Some("covariant") {
                Self::add_keyword(out, "covariant");
            }
            if Self::absent_or_in(q, ast[fields].late_keyword) && keyword != Some("late") {
                Self::add_keyword(out, "late");
            }
            Self::add_keyword(out, "static");
        }
        if !ast.list(ast[fields].variables).is_empty() {
            let is_const_or_final = ast[fields]
                .keyword
                .is_some_and(|k| matches!(ast.t_lexeme(k), "const" | "final"));
            if !is_const_or_final && !matches!(keyword, Some("const" | "final" | "var")) {
                Self::add_keyword(out, "const");
                Self::add_keyword(out, "final");
                if ast[fields].type_.is_none() {
                    Self::add_keyword(out, "var");
                }
            }
        }
    }

    /// Dart `addFormalParameterKeywords`.
    #[allow(clippy::too_many_arguments)]
    pub fn add_formal_parameter_keywords(
        q: &Request<'_, '_>,
        out: &mut Out,
        list: Option<Id<FormalParameterList>>,
        suggest_required: bool,
        suggest_variable_name: bool,
        suggest_covariant: bool,
        suggest_final_or_var: bool,
        suggest_this: bool,
    ) {
        use dartr_parser::experimental_flags::ExperimentalFlag as F;
        let ast = q.ast;
        if suggest_covariant {
            Self::add_keyword(out, "covariant");
        }
        if suggest_required && list.is_some_and(|l| in_named_group(ast, l, q.offset)) {
            Self::add_keyword(out, "required");
        }
        if !suggest_variable_name {
            return;
        }
        let Some(list) = list else {
            return;
        };
        let parent = ast.parent(list);
        if parent.is_some_and(|p| ast.is::<ConstructorDeclaration>(p)) {
            if q.feature_enabled(F::SuperParameters) {
                Self::add_keyword(out, "super");
            }
            if suggest_this {
                Self::add_keyword(out, "this");
            }
        } else if parent.is_some_and(|p| ast.is::<PrimaryConstructorDeclaration>(p)) && suggest_final_or_var {
            if q.feature_enabled(F::SuperParameters) {
                Self::add_keyword(out, "super");
            }
            Self::add_keyword(out, "final");
            Self::add_keyword(out, "this");
            Self::add_keyword(out, "var");
        }
    }

    /// Dart `addFunctionBodyModifiers`.
    pub fn add_function_body_modifiers(q: &Request<'_, '_>, out: &mut Out, body: Option<NodeId>) {
        let ast = q.ast;
        let keyword = body.and_then(|b| function_body_keyword(ast, b));
        if Self::absent_or_in(q, keyword) {
            Self::add_keyword(out, "async");
            if !body.is_some_and(|b| ast.is::<ExpressionFunctionBody>(b)) {
                Self::add_keyword_and_text(out, "async", "*");
                Self::add_keyword_and_text(out, "sync", "*");
            }
        }
    }

    /// Dart `addImportDirectiveKeywords`.
    pub fn add_import_directive_keywords(q: &Request<'_, '_>, out: &mut Out, node: Id<ImportDirective>) {
        let ast = q.ast;
        let n = &ast[node];
        let first_combinator = ast.list_raw(n.combinators).first().copied();
        if first_combinator.is_none_or(|c| q.offset < ast.offset(c)) {
            match n.deferred_keyword {
                None => match n.as_keyword {
                    None => {
                        Self::add_keyword_and_text(out, "deferred", " as");
                        Self::add_keyword(out, "as");
                        Self::add_keyword(out, "hide");
                        Self::add_keyword(out, "show");
                    }
                    Some(as_keyword) if q.offset < ast.t_offset(as_keyword) => {
                        Self::add_keyword(out, "deferred");
                    }
                    Some(_) => {
                        if n.prefix.is_some_and(|p| q.offset > ast.end(p)) {
                            Self::add_keyword(out, "hide");
                            Self::add_keyword(out, "show");
                        }
                    }
                },
                Some(deferred) if q.offset > ast.t_end(deferred) && n.as_keyword.is_none() => {
                    Self::add_keyword(out, "as");
                }
                Some(_) => {
                    Self::add_keyword(out, "hide");
                    Self::add_keyword(out, "show");
                }
            }
        } else {
            Self::add_keyword(out, "hide");
            Self::add_keyword(out, "show");
        }
    }

    /// Dart `addMixinDeclarationKeywords`.
    pub fn add_mixin_declaration_keywords(q: &Request<'_, '_>, out: &mut Out, node: Id<MixinDeclaration>) {
        let ast = q.ast;
        if Self::absent_or_in(q, ast[node].on_clause.map(|c| ast[c].on_keyword)) {
            Self::add_keyword(out, "on");
        }
        if Self::absent_or_in(q, ast[node].implements_clause.map(|c| ast[c].implements_keyword)) {
            Self::add_keyword(out, "implements");
        }
    }

    /// Dart `addMixinMemberKeywords`.
    pub fn add_mixin_member_keywords(out: &mut Out) {
        for k in [
            "const", "covariant", "dynamic", "final", "get", "operator", "set", "static", "var", "void", "late",
        ] {
            Self::add_keyword(out, k);
        }
    }

    /// Dart `addMixinModifiers`.
    pub fn add_mixin_modifiers(q: &Request<'_, '_>, out: &mut Out, node: Id<MixinDeclaration>) {
        if Self::absent_or_in(q, q.ast[node].base_keyword) {
            Self::add_keyword(out, "base");
        }
    }

    /// Dart `addPatternKeywords`.
    pub fn add_pattern_keywords(out: &mut Out) {
        Self::add_constant_expression_keywords(out, false);
        Self::add_variable_pattern_keywords(out);
    }

    /// Dart `addStatementKeywords`.
    pub fn add_statement_keywords(q: &Request<'_, '_>, out: &mut Out, node: NodeId) {
        let ast = q.ast;
        let in_loop = ast.this_or_ancestor_of_type::<DoStatement>(node).is_some()
            || ast.this_or_ancestor_of_type::<ForStatement>(node).is_some()
            || ast.this_or_ancestor_of_type::<WhileStatement>(node).is_some();
        if in_loop {
            Self::add_keyword(out, "break");
            Self::add_keyword(out, "continue");
        }
        if ast.this_or_ancestor_of_type::<SwitchStatement>(node).is_some() {
            Self::add_keyword(out, "break");
        }
        for k in ["assert", "do", "dynamic", "final", "for", "if"] {
            Self::add_keyword(out, k);
        }
        if ast.this_or_ancestor_of_type::<CatchClause>(node).is_some() {
            Self::add_keyword(out, "rethrow");
        }
        Self::add_keyword(out, "return");
        if !q.feature_enabled(dartr_parser::experimental_flags::ExperimentalFlag::Patterns) {
            Self::add_keyword(out, "switch");
        }
        for k in ["throw", "try", "var", "void", "while"] {
            Self::add_keyword(out, k);
        }
        if in_async_star_or_sync_star(ast, node) {
            Self::add_keyword(out, "yield");
            Self::add_keyword_and_text(out, "yield", "*");
        }
        Self::add_keyword(out, "late");
    }

    /// Dart `addTryClauseKeywords`.
    pub fn add_try_clause_keywords(out: &mut Out, can_have_finally: bool) {
        Self::add_keyword(out, "catch");
        if can_have_finally {
            Self::add_keyword(out, "finally");
        }
        Self::add_keyword(out, "on");
    }

    /// Dart `addVariablePatternKeywords`.
    pub fn add_variable_pattern_keywords(out: &mut Out) {
        Self::add_keyword(out, "final");
        Self::add_keyword(out, "var");
    }
}

/// The `async`/`sync` keyword of a function body.
pub(crate) fn function_body_keyword(ast: &Ast, body: NodeId) -> Option<TokenId> {
    if let Some(b) = ast.cast::<BlockFunctionBody>(body) {
        return ast[b].keyword;
    }
    if let Some(b) = ast.cast::<ExpressionFunctionBody>(body) {
        return ast[b].keyword;
    }
    None
}

fn function_body_star(ast: &Ast, body: NodeId) -> Option<TokenId> {
    if let Some(b) = ast.cast::<BlockFunctionBody>(body) {
        return ast[b].star;
    }
    if let Some(b) = ast.cast::<ExpressionFunctionBody>(body) {
        return ast[b].star;
    }
    None
}

/// Dart `inAsyncMethodOrFunction`.
pub(crate) fn in_async_method_or_function(ast: &Ast, node: NodeId) -> bool {
    let Some(body) = ast.this_or_ancestor_of_type::<FunctionBody>(node) else {
        return false;
    };
    dartr_resolver::ast_ext::function_body_is_asynchronous(ast, body.raw())
        && function_body_star(ast, body.raw()).is_none()
}

/// Dart `inAsyncStarOrSyncStarMethodOrFunction`.
pub(crate) fn in_async_star_or_sync_star(ast: &Ast, node: NodeId) -> bool {
    let Some(body) = ast.this_or_ancestor_of_type::<FunctionBody>(node) else {
        return false;
    };
    function_body_keyword(ast, body.raw()).is_some() && function_body_star(ast, body.raw()).is_some()
}

/// Dart `NodeList.elementBefore(offset)`.
pub(crate) fn element_before(ast: &Ast, list: &[NodeId], offset: u32) -> Option<NodeId> {
    list.iter().rev().copied().find(|e| ast.end(*e) <= offset)
}

/// Dart `couldHaveTrailingElse`.
fn could_have_trailing_else(ast: &Ast, element: Option<NodeId>) -> bool {
    let mut current = element;
    while let Some(e) = current {
        if let Some(i) = ast.cast::<IfElement>(e) {
            match ast[i].else_element {
                None => break,
                Some(x) => current = Some(x.raw()),
            }
        } else if let Some(f) = ast.cast::<ForElement>(e) {
            current = Some(ast[f].body.raw());
        } else {
            break;
        }
    }
    let Some(e) = current else {
        return false;
    };
    let Some(i) = ast.cast::<IfElement>(e) else {
        return false;
    };
    ast[i].else_keyword.is_none() && !ast.n_synthetic(ast[i].then_element.raw())
}

/// Dart `FormalParameterList.inNamedGroup(offset)`.
fn in_named_group(ast: &Ast, list: Id<FormalParameterList>, offset: u32) -> bool {
    let Some(left) = ast[list].left_delimiter else {
        return false;
    };
    if ast.t_ty(left) != TokenType::OPEN_CURLY_BRACKET {
        return false;
    }
    let l = ast.t_end(left);
    let r = match ast[list].right_delimiter {
        Some(r) => ast.t_offset(r),
        None => ast.t_offset(ast[list].right_parenthesis),
    };
    l <= offset && offset <= r
}

/// Dart `VariableDeclaration.isConst`.
pub(crate) fn variable_declaration_is_const(ast: &Ast, node: NodeId) -> bool {
    ast.parent(node)
        .and_then(|p| ast.cast::<VariableDeclarationList>(p))
        .is_some_and(|l| variable_list_is_const(ast, l))
}

/// Dart `VariableDeclarationList.isConst`.
pub(crate) fn variable_list_is_const(ast: &Ast, list: Id<VariableDeclarationList>) -> bool {
    ast[list].keyword.is_some_and(|k| ast.t_lexeme(k) == "const")
}

/// Dart `VariableDeclarationList.isFinal`.
pub(crate) fn variable_list_is_final(ast: &Ast, list: Id<VariableDeclarationList>) -> bool {
    ast[list].keyword.is_some_and(|k| ast.t_lexeme(k) == "final")
}

// ---------------------------------------------------------------------------
// Labels

/// Dart `LabelHelper.addLabels`.
pub fn add_labels(q: &Request<'_, '_>, out: &mut Out, statement: NodeId) {
    let ast = q.ast;
    let include_case_labels = ast.is::<ContinueStatement>(statement);
    let mut current = Some(statement);
    let visit = |labels: &[NodeId], out: &mut Out| {
        for &l in labels {
            let label = ast.cast::<Label>(l).unwrap();
            let name = ast.t_lexeme(ast[label].name).to_string();
            let score = out.score(&name);
            if score != -1.0 {
                out.add(Candidate::new(Kind::Label(name), score));
            }
        }
    };
    while let Some(c) = current {
        if let Some(s) = ast.cast::<SwitchStatement>(c) {
            if include_case_labels {
                for &m in ast.list(ast[s].members) {
                    let labels = dartr_resolver::ast_ext::switch_member_labels(ast, m);
                    visit(ast.list_raw(labels), out);
                }
            }
        } else if ast.is::<FunctionBody>(c) {
            return;
        } else if let Some(l) = ast.cast::<LabeledStatement>(c) {
            visit(ast.list_raw(ast[l].labels), out);
        }
        current = ast.parent(c);
    }
}

// ---------------------------------------------------------------------------
// Identifiers

/// Dart `getCamelWords`.
pub fn camel_words(s: &str) -> Vec<String> {
    if s.is_empty() {
        return Vec::new();
    }
    let units: Vec<u16> = s.encode_utf16().collect();
    let is_lower = |c: u16| (b'a' as u16..=b'z' as u16).contains(&c);
    let is_upper = |c: u16| (b'A' as u16..=b'Z' as u16).contains(&c);
    let mut parts = Vec::new();
    let mut was_lower = false;
    let mut was_upper = false;
    let mut start = 0;
    for i in 0..units.len() {
        let c = units[i];
        let new_lower = is_lower(c);
        let new_upper = is_upper(c);
        if was_lower && new_upper {
            parts.push(String::from_utf16_lossy(&units[start..i]));
            start = i;
        }
        if was_upper && new_upper && i + 1 < units.len() && is_lower(units[i + 1]) {
            parts.push(String::from_utf16_lossy(&units[start..i]));
            start = i;
        }
        was_lower = new_lower;
        was_upper = new_upper;
    }
    parts.push(String::from_utf16_lossy(&units[start..]));
    parts
}

/// Dart `getCamelWordCombinations`.
pub fn camel_word_combinations(name: &str) -> Vec<String> {
    let parts = camel_words(name);
    (0..parts.len())
        .map(|i| format!("{}{}", parts[i].to_lowercase(), parts[i + 1..].join("")))
        .collect()
}

/// Dart `IdentifierHelper`.
pub struct IdentifierHelper {
    pub include_private: bool,
}

impl IdentifierHelper {
    /// Dart `addSuggestionsFromTypeName`.
    pub fn add_suggestions_from_type_name(&self, out: &mut Out, type_name: &str) {
        let mut names = camel_word_combinations(type_name);
        if let Some(i) = names.iter().position(|n| n == type_name) {
            names.remove(i);
        }
        for name in names {
            Self::create_name_suggestion(out, &name);
            if self.include_private {
                Self::create_name_suggestion(out, &format!("_{name}"));
            }
        }
    }

    /// Dart `addTopLevelName`.
    pub fn add_top_level_name(&self, q: &Request<'_, '_>, out: &mut Out, include_body: bool) {
        let ctx = q.ctx;
        let base = std::path::Path::new(q.path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let Some(candidate) = to_upper_camel_case(base) else {
            return;
        };
        // Dart `libraryElement.children`.
        let l = ctx.get(q.library);
        let children: Vec<ElementId> = l
            .classes
            .iter()
            .map(|e| e.raw())
            .chain(l.enums.iter().map(|e| e.raw()))
            .chain(l.extensions.iter().map(|e| e.raw()))
            .chain(l.extension_types.iter().map(|e| e.raw()))
            .chain(l.getters.iter().map(|e| e.raw()))
            .chain(l.mixins.iter().map(|e| e.raw()))
            .chain(l.setters.iter().map(|e| e.raw()))
            .chain(l.top_level_functions.iter().map(|e| e.raw()))
            .chain(l.top_level_variables.iter().map(|e| e.raw()))
            .chain(l.type_aliases.iter().map(|e| e.raw()))
            .collect();
        if children.iter().any(|c| ctx.element_name(*c) == Some(candidate.as_str())) {
            return;
        }
        let score = out.score(&candidate);
        if score != -1.0 {
            out.add(Candidate::new(
                Kind::Identifier {
                    identifier: candidate,
                    include_body,
                },
                score,
            ));
        }
    }

    /// Dart `addVariable`.
    pub fn add_variable(&self, q: &Request<'_, '_>, out: &mut Out, ty: Option<NodeId>) {
        let ast = q.ast;
        if let Some(t) = ty.and_then(|t| ast.cast::<NamedType>(t)) {
            let name = ast.t_lexeme(ast[t].name).to_string();
            self.add_suggestions_from_type_name(out, &name);
        }
    }

    fn create_name_suggestion(out: &mut Out, name: &str) {
        if !name.is_empty() {
            let score = out.score(name);
            if score != -1.0 {
                out.add(Candidate::new(
                    Kind::Identifier {
                        identifier: name.to_string(),
                        include_body: false,
                    },
                    score,
                ));
            }
        }
    }
}

/// Dart `String.toUpperCamelCase`.
fn to_upper_camel_case(s: &str) -> Option<String> {
    let capitalized = |w: &str| {
        let mut c = w.chars();
        match c.next() {
            Some(f) => format!("{}{}", f.to_uppercase(), c.as_str()),
            None => String::new(),
        }
    };
    let words: Vec<&str> = s.split('_').collect();
    if words.len() < 2 {
        let first = words.first().copied().unwrap_or("");
        if !first.is_empty() {
            return Some(capitalized(first));
        }
        return None;
    }
    let mut buffer = String::new();
    for w in words {
        if w.is_empty() {
            return None;
        }
        buffer.push_str(&capitalized(w));
    }
    Some(buffer)
}

// ---------------------------------------------------------------------------
// Overrides

/// Dart `OverrideHelper.computeOverridesFor`.
pub fn compute_overrides_for(
    q: &Request<'_, '_>,
    out: &mut Out,
    interface: EId<InterfaceElement>,
    replacement: (u32, u32),
    skip_at: bool,
) {
    let ctx = q.ctx;
    let im = InheritanceManager3::new(*ctx);
    let iface = im.get_interface(interface);
    let library = elem::library_of(ctx, interface.raw());
    let data = ctx.instance(interface.raw().cast().unwrap());
    let declared_name = |text: &str| -> bool {
        let getter = data.getters.iter().any(|g| ctx.element_name(g.raw()) == Some(text));
        let method = data.methods.iter().any(|m| ctx.element_name(m.raw()) == Some(text));
        let setter_name = text.strip_suffix('=').unwrap_or(text);
        let setter = data.setters.iter().any(|s| ctx.element_name(s.raw()) == Some(setter_name));
        getter || method || setter
    };
    let mut names: Vec<Name> = Vec::new();
    for name in iface.map.keys() {
        if library.is_some_and(|l| name.is_accessible_for(ctx, l)) {
            let text = name.text(ctx);
            if !declared_name(text) {
                names.push(*name);
            }
        }
    }
    for name in names {
        let Some(&element) = iface.map.get(&name) else {
            continue;
        };
        if has_non_virtual_annotation(ctx, element) {
            continue;
        }
        let base = member::base_element(ctx, element);
        let score = out
            .score("override")
            .max(out.score("operator"))
            .max(out.score(&display_name(ctx, base)));
        let invoke_super = im.get_inherited_concrete_map(interface).contains_key(&name);
        if score != -1.0 {
            out.add(Candidate::new(
                Kind::Override {
                    element,
                    should_invoke_super: invoke_super,
                    skip_at,
                    replacement,
                    data: None,
                },
                score,
            ));
        }
    }
}

/// Dart `_hasNonVirtualAnnotation`.
fn has_non_virtual_annotation(ctx: &dartr_element::Ctx<'_>, element: ElemRef) -> bool {
    let base = member::base_element(ctx, element);
    let flag = dartr_resolver::element_metadata::flags::NON_VIRTUAL;
    if base.tag() == Tag::Getter && elem::is_origin_variable(ctx, base) {
        if let Some(v) = elem::accessor_variable(ctx, base) {
            if elem::metadata_has(ctx, v, flag) {
                return true;
            }
        }
    }
    elem::metadata_has(ctx, base, flag)
}
