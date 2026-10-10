// Dart source: pkg/analysis_server/lib/src/services/correction/dart/replace_final_with_var.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_var.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_type_annotation.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_unnecessary_final.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/replace_with_var.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_non_null_assertion.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/replace_cascade_with_dot.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/replace_with_identifier.dart
// Dart source: pkg/analyzer/lib/src/utilities/dot_shorthands.dart (isDotShorthand, hasDependentDotShorthand: approximated)

//! Producers that add, remove or replace type annotations and keywords.

use dartr_ast::*;
use dartr_element::TypeKind;
use dartr_syntax::{TokenId, TokenType};
use dartr_typesystem::type_ext::TypeExt;

use super::super::change_builder::ChangeBuilder;
use super::super::fix_kind::FixKind;
use super::super::generated::assist_kinds as a;
use super::super::generated::fix_kinds as k;
use super::super::producer::*;
use super::super::utils::Range;
use super::create::static_type;
use super::simple::producer;

/// Dart `isDotShorthand` (the shorthand nodes).
pub fn is_dot_shorthand(ast: &Ast, node: NodeId) -> bool {
    ast.is::<DotShorthandConstructorInvocation>(node)
        || ast.is::<DotShorthandInvocation>(node)
        || ast.is::<DotShorthandPropertyAccess>(node)
}

/// Dart `hasDependentDotShorthand` (only the shorthand nodes and the
/// elements of collection literals without type arguments).
pub fn has_dependent_dot_shorthand(ast: &Ast, node: NodeId) -> bool {
    if is_dot_shorthand(ast, node) {
        return true;
    }
    let elements = if let Some(l) = ast.cast::<ListLiteral>(node) {
        if ast[l].type_arguments.is_some() {
            return false;
        }
        ast.list_raw(ast[l].elements).to_vec()
    } else if let Some(l) = ast.cast::<SetOrMapLiteral>(node) {
        if ast[l].type_arguments.is_some() {
            return false;
        }
        ast.list_raw(ast[l].elements).to_vec()
    } else {
        return false;
    };
    elements.iter().any(|&e| {
        if let Some(entry) = ast.cast::<MapLiteralEntry>(e) {
            has_dependent_dot_shorthand(ast, ast[entry].key.raw())
                || has_dependent_dot_shorthand(ast, ast[entry].value.raw())
        } else {
            has_dependent_dot_shorthand(ast, e)
        }
    })
}

fn is_keyword(c: &ProducerContext<'_>, t: Option<TokenId>, lexeme: &str) -> bool {
    t.is_some_and(|t| c.lexeme(t) == lexeme)
}

/// Dart `ReplaceFinalWithVar`.
pub struct ReplaceFinalWithVar {
    final_keyword: Option<TokenId>,
    remove_final: bool,
    can_be_bulk_applied: bool,
}

impl ReplaceFinalWithVar {
    pub fn new(c: &ProducerContext<'_>) -> Self {
        let ast = c.ast;
        let node = c.node;
        let (final_keyword, remove_final, can_be_bulk_applied) =
            if let Some(n) = ast.cast::<VariableDeclarationList>(node) {
                (ast[n].keyword, ast[n].type_.is_some(), true)
            } else if let Some(n) = ast.cast::<PatternVariableDeclaration>(node) {
                (Some(ast[n].keyword), false, true)
            } else if let Some(n) = ast.cast::<DeclaredIdentifier>(node) {
                (ast[n].keyword, ast[n].type_.is_some(), true)
            } else if let Some(n) = ast.cast::<DeclaredVariablePattern>(node) {
                (ast[n].keyword, ast[n].type_.is_some(), true)
            } else if let Some(n) = ast.cast::<ForEachPartsWithPattern>(node) {
                (Some(ast[n].keyword), false, true)
            } else if let Some(n) = ast.cast::<RegularFormalParameter>(node) {
                let keyword = ast[n]
                    .const_final_or_var_keyword
                    .filter(|t| c.lexeme(*t) == "final");
                (keyword, ast[n].type_.is_some(), false)
            } else {
                (None, true, false)
            };
        ReplaceFinalWithVar {
            final_keyword,
            remove_final,
            can_be_bulk_applied,
        }
    }
}

impl CorrectionProducer for ReplaceFinalWithVar {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(if self.remove_final {
            &k::REMOVE_UNNECESSARY_FINAL
        } else {
            &k::REPLACE_FINAL_WITH_VAR
        })
    }

    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        if !self.can_be_bulk_applied {
            return None;
        }
        Some(if self.remove_final {
            &k::REMOVE_UNNECESSARY_FINAL_MULTI
        } else {
            &k::REPLACE_FINAL_WITH_VAR_MULTI
        })
    }

    fn applicability(&self) -> Applicability {
        Applicability::Automatically
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        if c.applying_bulk_fixes && !self.can_be_bulk_applied {
            return;
        }
        let Some(keyword) = self.final_keyword else {
            return;
        };
        if self.remove_final {
            let next = c.ast.tokens.next(keyword);
            let range = c.range().token_start_start(keyword, next);
            builder.add_dart_file_edit(c.path, |b| b.add_deletion(range.offset, range.length));
        } else {
            let range = c.range().token(keyword);
            builder.add_dart_file_edit(c.path, |b| {
                b.add_simple_replacement(range.offset, range.length, "var")
            });
        }
    }
}

/// Dart `RemoveUnnecessaryFinal`.
pub struct RemoveUnnecessaryFinal;

impl CorrectionProducer for RemoveUnnecessaryFinal {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::REMOVE_UNNECESSARY_FINAL)
    }

    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::REMOVE_UNNECESSARY_FINAL_MULTI)
    }

    fn applicability(&self) -> Applicability {
        Applicability::Automatically
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let keyword = if let Some(p) = ast.cast::<FieldFormalParameter>(c.node) {
            ast[p].const_final_or_var_keyword
        } else if let Some(p) = ast.cast::<SuperFormalParameter>(c.node) {
            ast[p].const_final_or_var_keyword
        } else {
            None
        };
        let Some(keyword) = keyword else { return };
        let next = ast.tokens.next(keyword);
        let range = c.range().token_start_start(keyword, next);
        builder.add_dart_file_edit(c.path, |b| b.add_deletion(range.offset, range.length));
    }
}

/// Dart `RemoveVar`.
pub struct RemoveVar;

impl CorrectionProducer for RemoveVar {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::REMOVE_VAR)
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let Some(d) = c.diagnostic else { return };
        let (offset, length) = (d.offset as u32, d.length as u32);
        builder.add_dart_file_edit(c.path, |b| b.add_deletion(offset, length + 1));
    }
}

/// Dart `RemoveTypeAnnotation` (`.fixVarAndType` and `.other`).
pub struct RemoveTypeAnnotation {
    pub var_and_type: bool,
}

/// The text and offset to insert after the type is removed (Dart
/// `insertionText`, `insertionOffset`).
fn type_arguments_insertion(
    c: &ProducerContext<'_>,
    ty: Id<TypeAnnotation>,
    initializer: Id<Expression>,
) -> Option<Option<(String, u32)>> {
    let ast = c.ast;
    let mut insertion: Option<(String, u32)> = None;
    let mut initializer = initializer.raw();
    if is_dot_shorthand(ast, initializer) {
        insertion = Some((
            c.utils.get_node_text(ty),
            c.token_offset(ast.begin_token(initializer)),
        ));
    } else if let Some(named) = ast.cast::<NamedType>(ty) {
        if let Some(type_arguments) = ast[named].type_arguments {
            if let Some(cascade) = ast.cast::<CascadeExpression>(initializer) {
                initializer = ast[cascade].target.raw();
            }
            if let Some(l) = ast.cast::<ListLiteral>(initializer) {
                if ast[l].type_arguments.is_none() {
                    insertion = Some((
                        c.utils.get_node_text(type_arguments),
                        c.token_offset(ast[l].left_bracket),
                    ));
                }
            } else if let Some(l) = ast.cast::<SetOrMapLiteral>(initializer) {
                if ast[l].type_arguments.is_none() {
                    insertion = Some((
                        c.utils.get_node_text(type_arguments),
                        c.token_offset(ast[l].left_bracket),
                    ));
                }
            } else if let Some(creation) = ast.cast::<InstanceCreationExpression>(initializer) {
                let constructor_type = ast[ast[creation].constructor_name].type_;
                if ast[constructor_type].type_arguments.is_none() {
                    insertion = Some((
                        c.utils.get_node_text(type_arguments),
                        ast.end(constructor_type),
                    ));
                }
            }
        }
    }
    if let Some(l) = ast.cast::<SetOrMapLiteral>(initializer) {
        if ast[l].type_arguments.is_none() && insertion.is_none() {
            return None;
        }
    }
    Some(insertion)
}

impl RemoveTypeAnnotation {
    fn remove_from_declaration_list(
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        list: Id<VariableDeclarationList>,
    ) {
        let ast = c.ast;
        let Some(ty) = ast[list].type_ else { return };
        let variables = ast.list(ast[list].variables);
        let Some(&first) = variables.first() else {
            return;
        };
        if variables.len() == 1 && ast.tokens.get(ast[first].name).is_synthetic() {
            return;
        }
        if c.selection_offset > c.token_end(ast[first].name) {
            return;
        }
        let Some(initializer) = ast[first].initializer else {
            return;
        };
        let Some(insertion) = type_arguments_insertion(c, ty, initializer) else {
            return;
        };
        let keyword = ast[list].keyword;
        let range = c.range().start_start(ty, first);
        builder.add_dart_file_edit(c.path, |b| {
            if keyword.is_some_and(|k| c.lexeme(k) != "var") {
                b.add_simple_replacement(range.offset, range.length, "");
            } else {
                b.add_simple_replacement(range.offset, range.length, "var ");
            }
            if let Some((text, offset)) = &insertion {
                b.add_simple_insertion(*offset, text);
            }
        });
    }

    fn remove_from_declared_identifier(
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        declaration: Id<DeclaredIdentifier>,
    ) {
        let ast = c.ast;
        let Some(ty) = ast[declaration].type_ else {
            return;
        };
        let mut insertion: Option<(String, u32)> = None;
        if let Some(parts) = ast
            .parent(declaration)
            .and_then(|p| ast.cast::<ForEachPartsWithDeclaration>(p))
        {
            let iterable = ast[parts].iterable.raw();
            if has_dependent_dot_shorthand(ast, iterable)
                && (ast.is::<ListLiteral>(iterable) || ast.is::<SetOrMapLiteral>(iterable))
            {
                insertion = Some((
                    format!("<{}>", c.utils.get_node_text(ty)),
                    c.token_offset(ast.begin_token(iterable)),
                ));
            }
        }
        let keyword = ast[declaration].keyword;
        let name = ast[declaration].name;
        let range = c.range().node_start_token_start(ty, name);
        builder.add_dart_file_edit(c.path, |b| {
            if keyword.is_some_and(|k| c.lexeme(k) != "var") {
                b.add_simple_replacement(range.offset, range.length, "");
            } else {
                b.add_simple_replacement(range.offset, range.length, "var ");
            }
            if let Some((text, offset)) = &insertion {
                b.add_simple_insertion(*offset, text);
            }
        });
    }

    fn remove_type_annotation(
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        ty: Option<Id<TypeAnnotation>>,
        parameters: Option<Id<FormalParameterList>>,
    ) {
        let ast = c.ast;
        let Some(ty) = ty else { return };
        let next = ast.tokens.next(ast.end_token(ty));
        let range = c.range().node_start_token_start(ty, next);
        let parameters_range: Option<Range> = parameters.map(|p| c.range().deletion_range(p));
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(range.offset, range.length);
            if let Some(r) = parameters_range {
                b.add_deletion(r.offset, r.length);
            }
        });
    }

    fn var_and_type(c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let node = c.node;
        let (keyword, ty) = if let Some(p) = ast.cast::<DeclaredVariablePattern>(node) {
            (ast[p].keyword, ast[p].type_)
        } else if let Some(l) = ast.cast::<VariableDeclarationList>(node) {
            (ast[l].keyword, ast[l].type_)
        } else {
            return;
        };
        let (Some(keyword), Some(ty)) = (keyword, ty) else {
            return;
        };
        if c.lexeme(keyword) != "var" {
            return;
        }
        let start = c.token_end(keyword);
        let end = ast.end(ty);
        builder.add_dart_file_edit(c.path, |b| b.add_deletion(start, end - start));
    }
}

impl CorrectionProducer for RemoveTypeAnnotation {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::REMOVE_TYPE_ANNOTATION)
    }

    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::REMOVE_TYPE_ANNOTATION_MULTI)
    }

    fn assist_kind(&self) -> Option<&'static FixKind> {
        Some(&a::REMOVE_TYPE_ANNOTATION)
    }

    fn applicability(&self) -> Applicability {
        Applicability::Automatically
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        if self.var_and_type {
            Self::var_and_type(c, builder);
            return;
        }
        let ast = c.ast;
        let mut current = Some(c.node);
        while let Some(node) = current {
            if let Some(d) = ast.cast::<DeclaredIdentifier>(node) {
                Self::remove_from_declared_identifier(c, builder, d);
                return;
            }
            if let Some(p) = ast.cast::<RegularFormalParameter>(node) {
                Self::remove_type_annotation(c, builder, ast[p].type_, None);
                return;
            }
            if let Some(p) = ast.cast::<SuperFormalParameter>(node) {
                let parameters = ast[p]
                    .function_typed_suffix
                    .map(|s| ast[s].formal_parameters);
                Self::remove_type_annotation(c, builder, ast[p].type_, parameters);
                return;
            }
            if c.diagnostic.is_some() {
                if let Some(ty) = ast.cast::<TypeAnnotation>(node) {
                    let parent = ast.parent(node);
                    if let Some(l) = parent.and_then(|p| ast.cast::<VariableDeclarationList>(p)) {
                        Self::remove_from_declaration_list(c, builder, l);
                    } else if let Some(d) = parent.and_then(|p| ast.cast::<DeclaredIdentifier>(p)) {
                        Self::remove_from_declared_identifier(c, builder, d);
                    } else {
                        Self::remove_type_annotation(c, builder, Some(ty), None);
                    }
                    return;
                }
            }
            if let Some(l) = ast.cast::<VariableDeclarationList>(node) {
                Self::remove_from_declaration_list(c, builder, l);
                return;
            }
            current = ast.parent(node);
        }
    }
}

/// Dart `ReplaceWithVar`.
pub struct ReplaceWithVar;

impl ReplaceWithVar {
    /// Dart `_canConvertVariableDeclarationList`.
    fn can_convert_list(c: &ProducerContext<'_>, list: Id<VariableDeclarationList>) -> bool {
        let ast = c.ast;
        let ctx = c.ctx;
        let Some(ty) = ast[list].type_ else {
            return false;
        };
        let Some(static_type) = c.tables.annotation_type.get(ty.raw()).copied() else {
            return false;
        };
        if matches!(ctx.ty(static_type), TypeKind::Dynamic) {
            return false;
        }
        ast.list(ast[list].variables).iter().all(|v| {
            ast[*v]
                .initializer
                .is_some_and(|i| static_type_of(c, i.raw()) == Some(static_type))
        })
    }

    /// Dart `_canReplaceWithVar`.
    fn can_replace_with_var(c: &ProducerContext<'_>) -> bool {
        let ast = c.ast;
        let ctx = c.ctx;
        let mut parent = ast.parent(c.node);
        while let Some(p) = parent {
            if let Some(s) = ast.cast::<VariableDeclarationStatement>(p) {
                return Self::can_convert_list(c, ast[s].variables);
            } else if let Some(f) = ast.cast::<ForPartsWithDeclarations>(p) {
                return Self::can_convert_list(c, ast[f].variables);
            } else if let Some(f) = ast.cast::<ForEachPartsWithDeclaration>(p) {
                let Some(ty) = ast[ast[f].loop_variable].type_ else {
                    return false;
                };
                let Some(static_type) = c.tables.annotation_type.get(ty.raw()).copied() else {
                    return false;
                };
                if matches!(ctx.ty(static_type), TypeKind::Dynamic) {
                    return false;
                }
                let Some(iterable_type) = static_type_of(c, ast[f].iterable.raw()) else {
                    return false;
                };
                let iterable = ctx.tp.iterable_element().upcast();
                let Some(instance) = ctx.as_instance_of(iterable_type, iterable) else {
                    return false;
                };
                let TypeKind::Interface { args, .. } = *ctx.ty(instance) else {
                    return false;
                };
                return ctx.list(args).first() == Some(&static_type);
            } else if let Some(t) = ast.cast::<TopLevelVariableDeclaration>(p) {
                return Self::can_convert_list(c, ast[t].variables);
            } else if let Some(f) = ast.cast::<FieldDeclaration>(p) {
                return Self::can_convert_list(c, ast[f].fields);
            }
            parent = ast.parent(p);
        }
        false
    }
}

fn static_type_of(c: &ProducerContext<'_>, node: NodeId) -> Option<dartr_element::TypeId> {
    static_type(c, node)
}

impl CorrectionProducer for ReplaceWithVar {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::REPLACE_WITH_VAR)
    }

    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::REPLACE_WITH_VAR_MULTI)
    }

    fn assist_kind(&self) -> Option<&'static FixKind> {
        Some(&a::REPLACE_WITH_VAR)
    }

    fn applicability(&self) -> Applicability {
        Applicability::Automatically
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let ty = if let Some(l) = ast.cast::<VariableDeclarationList>(c.node) {
            ast[l].type_
        } else {
            ast.this_or_ancestor_of_type::<TypeAnnotation>(c.node)
        };
        let Some(ty) = ty else { return };
        if !Self::can_replace_with_var(c) {
            return;
        }
        let parent = ast.parent(ty);
        let grandparent = parent.and_then(|p| ast.parent(p));
        if let Some(list) = parent.and_then(|p| ast.cast::<VariableDeclarationList>(p)) {
            let Some(g) = grandparent else { return };
            if !(ast.is::<VariableDeclarationStatement>(g)
                || ast.is::<ForPartsWithDeclarations>(g)
                || ast.is::<TopLevelVariableDeclaration>(g)
                || ast.is::<FieldDeclaration>(g))
            {
                return;
            }
            let keyword = ast[list].keyword;
            if is_keyword(c, keyword, "const") || is_keyword(c, keyword, "final") {
                return;
            }
            let variables = ast.list(ast[list].variables);
            if variables.len() != 1 {
                return;
            }
            let Some(initializer) = ast[variables[0]].initializer else {
                return;
            };
            let Some(insertion) = type_arguments_insertion(c, ty, initializer) else {
                return;
            };
            let range = c.range().node(ty);
            builder.add_dart_file_edit(c.path, |b| {
                b.add_simple_replacement(range.offset, range.length, "var");
                if let Some((text, offset)) = &insertion {
                    b.add_simple_insertion(*offset, text);
                }
            });
        } else if let Some(declared) = parent.and_then(|p| ast.cast::<DeclaredIdentifier>(p)) {
            let Some(parts) = grandparent.and_then(|g| ast.cast::<ForEachPartsWithDeclaration>(g))
            else {
                return;
            };
            let keyword = ast[declared].keyword;
            if is_keyword(c, keyword, "const") || is_keyword(c, keyword, "final") {
                return;
            }
            let iterable = ast[parts].iterable.raw();
            let mut insertion: Option<(String, u32)> = None;
            let typed_literal =
                ast.is::<ListLiteral>(iterable) || ast.is::<SetOrMapLiteral>(iterable);
            if has_dependent_dot_shorthand(ast, iterable) && typed_literal {
                insertion = Some((
                    format!("<{}>", c.utils.get_node_text(ty)),
                    c.token_offset(ast.begin_token(iterable)),
                ));
            } else if let Some(named) = ast.cast::<NamedType>(ty) {
                if let Some(type_arguments) = ast[named].type_arguments {
                    let literal_type_arguments = if let Some(l) = ast.cast::<ListLiteral>(iterable)
                    {
                        Some(ast[l].type_arguments)
                    } else {
                        ast.cast::<SetOrMapLiteral>(iterable)
                            .map(|l| ast[l].type_arguments)
                    };
                    if literal_type_arguments == Some(None) {
                        insertion =
                            Some((c.utils.get_node_text(type_arguments), ast.offset(iterable)));
                    }
                }
            }
            let range = c.range().node(ty);
            builder.add_dart_file_edit(c.path, |b| {
                b.add_simple_replacement(range.offset, range.length, "var");
                if let Some((text, offset)) = &insertion {
                    b.add_simple_insertion(*offset, text);
                }
            });
        }
    }
}

producer!(
    RemoveNonNullAssertion,
    k::REMOVE_NON_NULL_ASSERTION,
    Some(&k::REMOVE_NON_NULL_ASSERTION_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        if let Some(p) = ast.cast::<PostfixExpression>(c.node) {
            let bang = ast[p].operator;
            if ast.tokens.ty(bang) == TokenType::BANG {
                let range = c.range().token(bang);
                builder.add_dart_file_edit(c.path, |b| b.add_deletion(range.offset, range.length));
            }
        }
        if let Some(p) = ast.cast::<NullAssertPattern>(c.node) {
            let bang = ast[p].operator;
            let next = ast.tokens.next(bang);
            let range = c.range().token_start_start(bang, next);
            builder.add_dart_file_edit(c.path, |b| b.add_deletion(range.offset, range.length));
        }
    }
);

/// Dart `ReplaceCascadeWithDot._replaceFor`: the token and its replacement.
fn cascade_replacement(
    c: &ProducerContext<'_>,
    section: Option<NodeId>,
) -> Option<(TokenId, &'static str)> {
    let ast = c.ast;
    let section = section?;
    if let Some(a) = ast.cast::<AssignmentExpression>(section) {
        return cascade_replacement(c, Some(ast[a].left_hand_side.raw()));
    }
    let index = |t: TokenId| match c.lexeme(t) {
        ".." => Some((t, "")),
        "?.." => Some((t, "?")),
        _ => None,
    };
    let property = |t: TokenId| match c.lexeme(t) {
        ".." => Some((t, ".")),
        "?.." => Some((t, "?.")),
        _ => None,
    };
    if let Some(i) = ast.cast::<IndexExpression>(section) {
        if let Some(period) = ast[i].period {
            return index(period);
        }
        return cascade_replacement(c, ast[i].target.map(|t| t.raw()));
    }
    if let Some(m) = ast.cast::<MethodInvocation>(section) {
        if let Some(operator) = ast[m].operator {
            return property(operator);
        }
    }
    if let Some(p) = ast.cast::<PropertyAccess>(section) {
        return property(ast[p].operator);
    }
    None
}

producer!(
    ReplaceCascadeWithDot,
    k::REPLACE_CASCADE_WITH_DOT,
    Some(&k::REPLACE_CASCADE_WITH_DOT_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let mut node = Some(c.node);
        if ast.is::<ExtensionOverride>(c.node) {
            node = ast.parent(c.node);
        }
        let Some(cascade) = node.and_then(|n| ast.cast::<CascadeExpression>(n)) else {
            return;
        };
        let sections = ast.list_raw(ast[cascade].cascade_sections).to_vec();
        if sections.len() != 1 {
            return;
        }
        if let Some((token, replacement)) = cascade_replacement(c, Some(sections[0])) {
            let range = c.range().token(token);
            builder.add_dart_file_edit(c.path, |b| {
                b.add_simple_replacement(range.offset, range.length, replacement)
            });
        }
    }
);

producer!(
    ReplaceWithIdentifier,
    k::REPLACE_WITH_IDENTIFIER,
    Some(&k::REPLACE_WITH_IDENTIFIER_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let node = c.node;
        let (suffix, name) = if let Some(p) = ast.cast::<RegularFormalParameter>(node) {
            (ast[p].function_typed_suffix, ast[p].name)
        } else if let Some(p) = ast.cast::<FieldFormalParameter>(node) {
            (ast[p].function_typed_suffix, Some(ast[p].name))
        } else if let Some(p) = ast.cast::<SuperFormalParameter>(node) {
            (ast[p].function_typed_suffix, Some(ast[p].name))
        } else {
            return;
        };
        if suffix.is_none() {
            return;
        }
        let Some(name) = name else { return };
        let text = c.lexeme(name).to_string();
        let range = c.range().node(node);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(range.offset, range.length, &text)
        });
    }
);
