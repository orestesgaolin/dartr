// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_class_to_mixin.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_class_to_enum.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/destructure_local_variable_assignment.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_async.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/add_diagnostic_property_reference.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/add_digit_separators.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_add_all_to_spread.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/inline_invocation.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_conditional_expression_to_if_element.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_documentation_into_line.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_into_for_index.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_into_is_not.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_into_is_not_empty.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_map_from_iterable_to_for_literal.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_part_of_to_uri.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_generic_function_syntax.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_null_aware.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_set_literal.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/join_variable_declaration.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_unnecessary_name.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/shadow_field.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/sort_child_property_last.dart

//! Class, variable, async, Flutter diagnostic, collection, and miscellaneous assists.

use std::collections::HashSet;

use dartr_ast::*;
use dartr_element::{
    ClassElement, ElemRef, ElementFlags, ElementId, FormalParameterElement, InterfaceElement, Tag,
    TypeId, TypeKind,
};
use dartr_syntax::{TokenId, TokenType};
use dartr_typesystem::type_ext::TypeExt;
use indexmap::IndexMap;

use super::super::change::LinkedEditSuggestionKind;
use super::super::change_builder::ChangeBuilder;
use super::super::dart_edit::{WriteType, get_camel_words};
use super::super::fix_kind::FixKind;
use super::super::generated::assist_kinds as a;
use super::super::generated::fix_kinds as k;
use super::super::producer::*;
use super::create::static_type;
use super::simple::{assist, producer};

fn unparenthesized(ast: &Ast, mut node: NodeId) -> NodeId {
    while let Some(pe) = ast.cast::<ParenthesizedExpression>(node) {
        node = ast[pe].expression.raw();
    }
    node
}

// ---------------------------------------------------------------------------
// ConvertClassToMixin
// ---------------------------------------------------------------------------

assist!(
    ConvertClassToMixin,
    a::CONVERT_CLASS_TO_MIXIN,
    |c, builder| {
        let ast = c.ast;
        let ctx = c.ctx;
        let Some(class_decl) = ast.this_or_ancestor_of_type::<ClassDeclaration>(c.node) else {
            return;
        };
        let name_part = ast[class_decl].name_part;
        if ast.is::<PrimaryConstructorDeclaration>(name_part) {
            return;
        }
        let type_name_tok = dartr_resolver::error::support::class_name_token(ast, name_part);
        let sel_end = c.selection_offset + c.selection_length;
        if c.selection_offset > c.token_end(type_name_tok)
            || sel_end < c.token_offset(ast[class_decl].class_keyword)
        {
            return;
        }
        if ast[class_decl].final_keyword.is_some()
            || ast[class_decl].interface_keyword.is_some()
            || ast[class_decl].sealed_keyword.is_some()
        {
            return;
        }
        let body = ast[class_decl].body.raw();
        if let Some(bcb) = ast.cast::<BlockClassBody>(body) {
            if ast
                .list_raw(ast[bcb].members)
                .iter()
                .any(|&m| ast.is::<ConstructorDeclaration>(m))
            {
                return;
            }
        }
        let Some(class_elem) = c
            .locator()
            .declared_element(class_decl)
            .and_then(|e| e.cast::<InterfaceElement>())
        else {
            return;
        };

        // _SuperclassReferenceFinder
        let mut referenced_classes: Vec<ElementId> = Vec::new();
        super::variables::visit(ast, class_decl.raw(), &mut |n| {
            if !ast.is::<SuperExpression>(n) {
                return;
            }
            let Some(parent) = ast.parent(n) else {
                return;
            };
            let elem = if ast.is::<BinaryExpression>(parent) || ast.is::<IndexExpression>(parent) {
                c.element_of(parent)
            } else if let Some(mi) = ast.cast::<MethodInvocation>(parent) {
                c.locator().element(ast[mi].method_name)
            } else if let Some(pi) = ast.cast::<PrefixedIdentifier>(parent) {
                c.locator().element(ast[pi].identifier)
            } else if let Some(pa) = ast.cast::<PropertyAccess>(parent) {
                c.locator().element(ast[pa].property_name)
            } else {
                None
            };
            if let Some(e) = elem {
                if matches!(
                    e.tag(),
                    Tag::Method | Tag::Getter | Tag::Setter | Tag::Constructor
                ) {
                    if let Some(enclosing) = ctx.element_data(e).and_then(|d| d.enclosing) {
                        if enclosing.tag() == Tag::Class {
                            referenced_classes.push(enclosing);
                        }
                    }
                }
            }
        });

        let mut superclass_constraints: Vec<TypeId> = Vec::new();
        let mut interfaces: Vec<TypeId> = Vec::new();
        let iface_data = ctx.interface(class_elem);
        for &mixin_ty in ctx.element_mixins(class_elem) {
            if let TypeKind::Interface { element, .. } = *ctx.ty(mixin_ty) {
                if referenced_classes.contains(&element.raw()) {
                    superclass_constraints.push(mixin_ty);
                } else {
                    interfaces.push(mixin_ty);
                }
            }
        }
        if ast[class_decl].extends_clause.is_some() {
            if let Some(super_ty) = iface_data.supertype.get() {
                if referenced_classes.len() > superclass_constraints.len() {
                    superclass_constraints.insert(0, super_ty);
                } else {
                    interfaces.insert(0, super_ty);
                }
            }
        }
        interfaces.extend_from_slice(ctx.element_interfaces(class_elem));

        let range_end = if let Some(bcb) = ast.cast::<BlockClassBody>(body) {
            ast[bcb].left_bracket
        } else if let Some(ecb) = ast.cast::<EmptyClassBody>(body) {
            ast[ecb].semicolon
        } else {
            return;
        };
        let start_tok = ast[class_decl]
            .abstract_keyword
            .unwrap_or(ast[class_decl].class_keyword);
        let r = c.range().token_start_start(start_tok, range_end);
        let type_name = c.lexeme(type_name_tok).to_string();
        let type_params = iface_data.type_params.clone();
        let is_block_body = ast.is::<BlockClassBody>(body);

        builder.add_dart_file_edit(c.path, |b| {
            b.add_replacement(r.offset, r.length, |e| {
                e.write("mixin ");
                e.write(&type_name);
                e.write_type_parameters(ctx, &type_params, &[]);
                if !superclass_constraints.is_empty() {
                    e.write(" on ");
                    for (i, &t) in superclass_constraints.iter().enumerate() {
                        if i > 0 {
                            e.write(", ");
                        }
                        e.write_type(Some(t), &WriteType::default());
                    }
                }
                if !interfaces.is_empty() {
                    e.write(" implements ");
                    for (i, &t) in interfaces.iter().enumerate() {
                        if i > 0 {
                            e.write(", ");
                        }
                        e.write_type(Some(t), &WriteType::default());
                    }
                }
                if is_block_body {
                    e.write(" ");
                }
            });
        });
    }
);

// ---------------------------------------------------------------------------
// DestructureLocalVariableAssignment
// ---------------------------------------------------------------------------

fn is_cascaded_invocation(ast: &Ast, mi: Id<MethodInvocation>) -> bool {
    ast[mi].operator.is_some_and(|op| {
        matches!(
            ast.tokens.ty(op),
            TokenType::PERIOD_PERIOD | TokenType::QUESTION_PERIOD_PERIOD
        )
    })
}

fn find_possible_local_variable_conflicts(c: &ProducerContext<'_>, offset: u32) -> HashSet<String> {
    let ast = c.ast;
    let Some(covering) = ast.node_covering(c.unit, offset, 0) else {
        return HashSet::new();
    };
    let Some(block) = ast.this_or_ancestor_of_type::<Block>(covering) else {
        return HashSet::new();
    };
    let mut names = HashSet::new();
    super::variables::visit(ast, block.raw(), &mut |n| {
        if let Some(nt) = ast.cast::<NamedType>(n) {
            if ast[nt].import_prefix.is_none() {
                names.insert(c.lexeme(ast[nt].name).to_string());
            }
        } else if let Some(id) = ast.cast::<SimpleIdentifier>(n) {
            let parent = ast.parent(id);
            let is_label = parent.is_some_and(|p| ast.is::<Label>(p));
            let is_prefixed = parent.is_some_and(|p| {
                if let Some(cn) = ast.cast::<ConstructorName>(p) {
                    ast[cn].name == Some(id)
                } else if let Some(mi) = ast.cast::<MethodInvocation>(p) {
                    ast[mi].method_name == id
                        && (ast[mi].target.is_some() || is_cascaded_invocation(ast, mi))
                } else if let Some(pi) = ast.cast::<PrefixedIdentifier>(p) {
                    ast[pi].identifier == id
                } else if let Some(pa) = ast.cast::<PropertyAccess>(p) {
                    ast[pa].target.map(|t| t.raw()) == Some(id.raw())
                } else {
                    false
                }
            });
            if !is_label && !is_prefixed {
                names.insert(c.lexeme(ast[id].token).to_string());
            }
        } else if let Some(vd) = ast.cast::<VariableDeclaration>(n) {
            names.insert(c.lexeme(ast[vd].name).to_string());
        }
    });
    names
}

fn variable_name_suggestions_for_text(text: &str, excluded: &HashSet<String>) -> Vec<String> {
    let parts = get_camel_words(Some(text));
    let mut res = Vec::new();
    for i in 0..parts.len() {
        let item = format!("{}{}", parts[i].to_lowercase(), parts[i + 1..].join(""));
        let mut suffix = 1;
        loop {
            let name = if suffix > 1 {
                format!("{item}{suffix}")
            } else {
                item.clone()
            };
            if !excluded.contains(&name) {
                if !res.contains(&name) {
                    res.push(name);
                }
                break;
            }
            suffix += 1;
        }
    }
    res
}

fn in_setter_context(ast: &Ast, node: NodeId) -> bool {
    let climbed = super::members::climb_property_access(ast, node);
    let Some(parent) = ast.parent(climbed) else {
        return false;
    };
    if let Some(ae) = ast.cast::<AssignmentExpression>(parent) {
        return ast[ae].left_hand_side.raw() == climbed;
    }
    if let Some(pe) = ast.cast::<PostfixExpression>(parent) {
        return matches!(
            ast.tokens.ty(ast[pe].operator),
            TokenType::PLUS_PLUS | TokenType::MINUS_MINUS
        );
    }
    if let Some(pe) = ast.cast::<PrefixExpression>(parent) {
        return matches!(
            ast.tokens.ty(ast[pe].operator),
            TokenType::PLUS_PLUS | TokenType::MINUS_MINUS
        );
    }
    false
}

assist!(
    DestructureLocalVariableAssignment,
    a::DESTRUCTURE_LOCAL_VARIABLE_ASSIGNMENT,
    |c, builder| {
        let ast = c.ast;
        let ctx = c.ctx;
        let Some(var_decl) = ast.cast::<VariableDeclaration>(c.node) else {
            return;
        };
        let Some(var_elem) = c.locator().declared_element(var_decl) else {
            return;
        };
        let ty = dartr_resolver::element_ext::variable_type(ctx, var_elem);
        match *ctx.ty(ty) {
            TypeKind::Record {
                positional, named, ..
            } => {
                let offset = ast.offset(var_decl);
                let mut excluded: HashSet<String> =
                    super::assists2::scoped_names(c, var_decl.raw(), offset)
                        .into_iter()
                        .collect();
                enum RecField {
                    Positional(String),
                    Named {
                        field: String,
                        variable: Option<String>,
                    },
                }
                let mut fields = Vec::new();
                for i in 1..=ctx.list(positional).len() {
                    let mut var_name = format!("${i}");
                    if excluded.contains(&var_name) {
                        for ch in b'a'..b'z' {
                            let cand = format!("${i}{}", ch as char);
                            if !excluded.contains(&cand) {
                                var_name = cand;
                                break;
                            }
                        }
                    }
                    excluded.insert(var_name.clone());
                    fields.push(RecField::Positional(var_name));
                }
                for nf in ctx.list(named) {
                    let name = ctx.name_str(nf.name).to_string();
                    if !excluded.contains(&name) {
                        fields.push(RecField::Named {
                            field: name,
                            variable: None,
                        });
                    } else {
                        let suggestions = variable_name_suggestions_for_text(&name, &excluded);
                        let Some(first) = suggestions.first().cloned() else {
                            return;
                        };
                        fields.push(RecField::Named {
                            field: name,
                            variable: Some(first),
                        });
                    }
                }
                let name_range = c.range().token(ast[var_decl].name);
                builder.add_dart_file_edit(c.path, |b| {
                    b.add_replacement(name_range.offset, name_range.length, |e| {
                        e.write("(");
                        for (i, f) in fields.iter().enumerate() {
                            if i > 0 {
                                e.write(", ");
                            }
                            let group = format!("VAR_{i}");
                            match f {
                                RecField::Positional(v) => {
                                    let suggestions = vec![v.clone(), "_".to_string()];
                                    e.add_simple_linked_edit(
                                        &group,
                                        v,
                                        Some((LinkedEditSuggestionKind::Variable, &suggestions)),
                                    );
                                }
                                RecField::Named { field, variable } => match variable {
                                    None => {
                                        let v = format!(":{field}");
                                        let suggestions = vec![v.clone(), format!("{field}: _")];
                                        e.add_simple_linked_edit(
                                            &group,
                                            &v,
                                            Some((
                                                LinkedEditSuggestionKind::Variable,
                                                &suggestions,
                                            )),
                                        );
                                    }
                                    Some(v) => {
                                        e.write(&format!("{field}: "));
                                        let suggestions = vec![v.clone(), "_".to_string()];
                                        e.add_simple_linked_edit(
                                            &group,
                                            v,
                                            Some((
                                                LinkedEditSuggestionKind::Variable,
                                                &suggestions,
                                            )),
                                        );
                                    }
                                },
                            }
                        }
                        e.write(")");
                    });
                });
            }
            TypeKind::Interface { element, .. } => {
                if var_elem.tag() != Tag::LocalVariable {
                    return;
                }
                let Some(fn_body) = ast.this_or_ancestor_of_type::<FunctionBody>(var_decl) else {
                    return;
                };
                let mut object_refs = Vec::new();
                let mut prop_refs: IndexMap<String, Vec<NodeId>> = IndexMap::new();
                super::variables::visit(ast, fn_body.raw(), &mut |n| {
                    let Some(id) = ast.cast::<SimpleIdentifier>(n) else {
                        return;
                    };
                    if c.locator().element(id) != Some(var_elem) {
                        return;
                    }
                    let Some(parent) = ast.parent(id) else {
                        object_refs.push(n);
                        return;
                    };
                    if let Some(pi) = ast.cast::<PrefixedIdentifier>(parent) {
                        if ast[pi].prefix == id {
                            let pname = c.lexeme(ast[ast[pi].identifier].token).to_string();
                            prop_refs.entry(pname).or_default().push(parent);
                            return;
                        }
                    }
                    if let Some(pa) = ast.cast::<PropertyAccess>(parent) {
                        if ast[pa].target.map(|t| t.raw()) == Some(id.raw()) {
                            let pname = c.lexeme(ast[ast[pa].property_name].token).to_string();
                            prop_refs.entry(pname).or_default().push(parent);
                            return;
                        }
                    }
                    object_refs.push(n);
                });
                if !object_refs.is_empty() {
                    return;
                }
                let names_in_scope: HashSet<String> =
                    super::assists2::scoped_names(c, var_decl.raw(), ast.offset(var_decl))
                        .into_iter()
                        .collect();
                let mut var_map: Vec<(String, String, Vec<NodeId>)> = Vec::new();
                for (field_name, refs) in prop_refs {
                    let mut excludes =
                        find_possible_local_variable_conflicts(c, ast.offset(var_decl));
                    excludes.extend(names_in_scope.iter().cloned());
                    for &r in &refs {
                        if in_setter_context(ast, r) {
                            return;
                        }
                        excludes.extend(find_possible_local_variable_conflicts(c, ast.offset(r)));
                    }
                    let suggestions = variable_name_suggestions_for_text(&field_name, &excludes);
                    let Some(var_name) = suggestions.first().cloned() else {
                        return;
                    };
                    var_map.push((var_name, field_name, refs));
                }
                let type_name = ctx.element_name(element.raw()).unwrap_or("").to_string();
                let name_range = c.range().token(ast[var_decl].name);
                builder.add_dart_file_edit(c.path, |b| {
                    b.add_replacement(name_range.offset, name_range.length, |e| {
                        e.write(&format!("{type_name}("));
                        if var_map.is_empty() {
                            e.select_here();
                        } else {
                            for (i, (var_name, field_name, _)) in var_map.iter().enumerate() {
                                if i > 0 {
                                    e.write(", ");
                                }
                                if var_name == field_name {
                                    e.write(":");
                                } else {
                                    e.write(&format!("{field_name}: "));
                                }
                                let suggestions = vec![var_name.clone()];
                                e.add_simple_linked_edit(
                                    var_name,
                                    var_name,
                                    Some((LinkedEditSuggestionKind::Variable, &suggestions)),
                                );
                            }
                        }
                        e.write(")");
                    });
                    for (var_name, _, refs) in &var_map {
                        for &r in refs {
                            let rr = c.range().node(r);
                            b.add_replacement(rr.offset, rr.length, |e| {
                                e.add_linked_edit(var_name, |e| e.write(var_name));
                            });
                        }
                    }
                });
            }
            _ => {}
        }
    }
);

// ---------------------------------------------------------------------------
// RemoveAsync
// ---------------------------------------------------------------------------

struct RemoveAsyncVisitor<'a, 'b> {
    c: &'a ProducerContext<'b>,
    arg_type: TypeId,
    processing_future: bool,
    found_one_return: bool,
    found_await: bool,
    returns_are_assignable: bool,
}

impl<'a, 'b> RemoveAsyncVisitor<'a, 'b> {
    fn new(c: &'a ProducerContext<'b>, arg_type: TypeId) -> Self {
        Self {
            c,
            arg_type,
            processing_future: false,
            found_one_return: false,
            found_await: false,
            returns_are_assignable: true,
        }
    }

    fn is_assignable(&self, ty: TypeId) -> bool {
        let ctx = self.c.ctx;
        let ts = dartr_typesystem::type_system::TypeSystem::new(*ctx);
        let target = if self.processing_future {
            ctx.tp.future_or_type(ctx, self.arg_type)
        } else {
            self.arg_type
        };
        ts.is_assignable_to(ty, target, self.c.options.strict_casts)
    }

    fn visit(&mut self, node: NodeId) {
        let ast = self.c.ast;
        if ast.is::<AwaitExpression>(node) {
            self.found_await = true;
            return;
        }
        if let Some(efb) = ast.cast::<ExpressionFunctionBody>(node) {
            self.found_one_return = true;
            if let Some(ty) = static_type(self.c, ast[efb].expression.raw()) {
                self.returns_are_assignable &= self.is_assignable(ty);
            } else {
                self.returns_are_assignable = false;
            }
            for child in ast.children(node) {
                self.visit(child);
            }
            return;
        }
        if ast.is::<FunctionExpression>(node) {
            // Return statements within closures aren't counted.
            return;
        }
        if let Some(rs) = ast.cast::<ReturnStatement>(node) {
            self.found_one_return = true;
            if let Some(expr) = ast[rs].expression
                && let Some(ty) = static_type(self.c, expr.raw())
            {
                self.returns_are_assignable &= self.is_assignable(ty);
            } else {
                self.returns_are_assignable = false;
            }
            for child in ast.children(node) {
                self.visit(child);
            }
            return;
        }
        for child in ast.children(node) {
            self.visit(child);
        }
    }

    fn returns_will_be_assignable(&mut self, node: NodeId) -> bool {
        self.found_await = false;
        self.returns_are_assignable = true;
        self.found_one_return = false;
        self.processing_future = false;
        self.visit(node);
        (self.returns_are_assignable && self.found_one_return) || !self.found_one_return
    }

    fn returns_are_assignable(&mut self, node: NodeId) -> bool {
        self.found_await = false;
        self.returns_are_assignable = true;
        self.found_one_return = false;
        self.processing_future = true;
        self.visit(node);
        (self.returns_are_assignable && self.found_one_return) || !self.found_one_return
    }
}

assist!(RemoveAsync, a::REMOVE_ASYNC, |c, builder| {
    let ast = c.ast;
    let ctx = c.ctx;
    let mut update_return_type = true;
    let mut node = c.node;
    if ast.is::<Block>(node) {
        let Some(p) = ast.parent(node) else { return };
        node = p;
    }
    if ast.is::<BlockFunctionBody>(node) || ast.is::<ExpressionFunctionBody>(node) {
        let Some(p) = ast.parent(node) else { return };
        node = p;
    }
    if ast.is::<FormalParameterList>(node) {
        let Some(p) = ast.parent(node) else { return };
        node = p;
    }
    if ast.is::<NamedType>(node) {
        let Some(p) = ast.parent(node) else { return };
        node = p;
    }
    if ast.is::<FunctionExpression>(node)
        && ast
            .parent(node)
            .is_some_and(|p| ast.is::<FunctionDeclaration>(p))
    {
        node = ast.parent(node).unwrap();
    }
    let (body, return_type, return_type_node): (
        NodeId,
        Option<TypeId>,
        Option<Id<TypeAnnotation>>,
    ) = if let Some(fe) = ast.cast::<FunctionExpression>(node) {
        (ast[fe].body.raw(), None, None)
    } else if let Some(fd) = ast.cast::<FunctionDeclaration>(node) {
        let body = ast[ast[fd].function_expression].body.raw();
        let rt = c
            .locator()
            .declared_element(fd)
            .map(|e| dartr_typesystem::member::return_type(ctx, ElemRef::Base(e)));
        (body, rt, ast[fd].return_type)
    } else if let Some(md) = ast.cast::<MethodDeclaration>(node) {
        let body = ast[md].body.raw();
        let rt = c
            .locator()
            .declared_element(md)
            .map(|e| dartr_typesystem::member::return_type(ctx, ElemRef::Base(e)));
        (body, rt, ast[md].return_type)
    } else {
        return;
    };
    let async_kw = if let Some(bfb) = ast.cast::<BlockFunctionBody>(body) {
        if ast[bfb].star.is_some() {
            return;
        }
        let Some(kw) = ast[bfb].keyword.filter(|&k| c.lexeme(k) == "async") else {
            return;
        };
        kw
    } else if let Some(efb) = ast.cast::<ExpressionFunctionBody>(body) {
        if ast[efb].star.is_some() {
            return;
        }
        let Some(kw) = ast[efb].keyword.filter(|&k| c.lexeme(k) == "async") else {
            return;
        };
        kw
    } else {
        return;
    };

    if let Some(rt) = return_type
        && let TypeKind::Interface { element, args, .. } = *ctx.ty(rt)
        && (ctx.is_element(element.raw(), "dart.async", "Future")
            || ctx.is_element(element.raw(), "dart.async", "FutureOr"))
    {
        let new_return = ctx
            .list(args)
            .first()
            .copied()
            .unwrap_or_else(|| ctx.tp.dynamic_type());
        let mut visitor = RemoveAsyncVisitor::new(c, new_return);
        if !visitor.returns_will_be_assignable(body) {
            if visitor.returns_are_assignable(body) {
                update_return_type = false;
            } else {
                return;
            }
        }
        if visitor.found_await {
            return;
        }
    } else if let Some(rt) = return_type {
        let mut visitor = RemoveAsyncVisitor::new(c, rt);
        if !visitor.returns_are_assignable(body) {
            return;
        }
    } else {
        update_return_type = false;
    }

    let next_tok = ast.tokens.next(async_kw);
    let del_end = c.token_end(async_kw)
        + if c.token_offset(next_tok) == c.token_end(async_kw) {
            0
        } else {
            1
        };
    let del_start = c.token_offset(async_kw);
    let del_len = del_end - del_start;

    if update_return_type {
        builder.add_dart_file_edit(c.path, |b| {
            if !ast.is::<EmptyFunctionBody>(body) {
                b.add_deletion(del_start, del_len);
            }
            if let Some(rtn) = return_type_node
                && let Some(rt) = c.tables.annotation_type.get(rtn.raw()).copied()
                && let TypeKind::Interface { element, .. } = *ctx.ty(rt)
                && (ctx.is_element(element.raw(), "dart.async", "Future")
                    || ctx.is_element(element.raw(), "dart.async", "FutureOr"))
            {
                let ts = dartr_typesystem::type_system::TypeSystem::new(*ctx);
                let value_type = ts.flatten(rt);
                let rr = c.range().node(rtn);
                b.add_replacement(rr.offset, rr.length, |e| {
                    if !e.write_type(Some(value_type), &WriteType::default()) {
                        e.write("void");
                    }
                });
            }
        });
    } else {
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(del_start, del_len);
        });
    }
});

// ---------------------------------------------------------------------------
// AddDiagnosticPropertyReference
// ---------------------------------------------------------------------------

producer!(
    AddDiagnosticPropertyReference,
    k::ADD_DIAGNOSTIC_PROPERTY_REFERENCE,
    Some(&k::ADD_DIAGNOSTIC_PROPERTY_REFERENCE_MULTI),
    Automatically,
    assist: Some(&a::ADD_DIAGNOSTIC_PROPERTY_REFERENCE),
    |c, builder| {
        let ast = c.ast;
        let ctx = c.ctx;
        let node = c.node;
        let (name_id, decl_ty, decl_type_ann) = if let Some(md) =
            ast.cast::<MethodDeclaration>(node)
        {
            let ty = c.locator().declared_element(md).and_then(|e| {
                (e.tag() == Tag::Getter)
                    .then(|| dartr_typesystem::member::return_type(ctx, ElemRef::Base(e)))
            });
            (c.lexeme(ast[md].name).to_string(), ty, ast[md].return_type)
        } else if let Some(vd) = ast.cast::<VariableDeclaration>(node) {
            let ty = c.locator().declared_element(vd).and_then(|e| {
                (e.tag() == Tag::Field).then(|| dartr_resolver::element_ext::variable_type(ctx, e))
            });
            let dta = ast
                .this_or_ancestor_of_type::<VariableDeclarationList>(vd)
                .and_then(|vdl| ast[vdl].type_);
            (c.lexeme(ast[vd].name).to_string(), ty, dta)
        } else {
            return;
        };
        let Some(class_decl) = ast.this_or_ancestor_of_type::<ClassDeclaration>(node) else {
            return;
        };
        let Some(class_elem) = c
            .locator()
            .declared_element(class_decl)
            .and_then(|e| e.cast::<InterfaceElement>())
        else {
            return;
        };
        // Class must subtype Diagnosticable.
        let is_diagnosticable = super::super::flutter::is_widget(ctx, class_elem.raw())
            || ctx.element_name(class_elem.raw()) == Some("Diagnosticable")
            || ctx.element_all_supertypes(class_elem).iter().any(|&st| {
                if let TypeKind::Interface { element, .. } = *ctx.ty(st) {
                    ctx.element_name(element.raw()) == Some("Diagnosticable")
                } else {
                    false
                }
            });
        if !is_diagnosticable {
            return;
        }
        let Some(ty) = decl_ty else {
            return;
        };
        let is_enum = matches!(ctx.ty(ty), TypeKind::Interface { element, .. } if element.raw().tag() == Tag::Enum);
        let is_iterable = ctx
            .as_instance_of(ty, ctx.tp.iterable_element().upcast())
            .is_some();
        let is_color = if let TypeKind::Interface { element, .. } = *ctx.ty(ty) {
            ctx.element_name(element.raw()) == Some("Color")
                || ctx.element_all_supertypes(element).iter().any(|&st| {
                    matches!(ctx.ty(st), TypeKind::Interface { element: se, .. } if ctx.element_name(se.raw()) == Some("Color"))
                })
        } else {
            false
        };
        let is_matrix4 = if let TypeKind::Interface { element, .. } = *ctx.ty(ty) {
            ctx.element_name(element.raw()) == Some("Matrix4")
                || ctx.element_all_supertypes(element).iter().any(|&st| {
                    matches!(ctx.ty(st), TypeKind::Interface { element: se, .. } if ctx.element_name(se.raw()) == Some("Matrix4"))
                })
        } else {
            false
        };

        let ctor_id: &'static str;
        let mut type_args: Option<Vec<TypeId>> = None;
        let mut ctor_name: &'static str = "";
        if matches!(ctx.ty(ty), TypeKind::Function(_)) {
            ctor_id = "ObjectFlagProperty";
            type_args = Some(vec![ty]);
            ctor_name = ".has";
        } else if ctx.is_dart_core_int(ty) {
            ctor_id = "IntProperty";
        } else if ctx.is_dart_core_double(ty) {
            ctor_id = "DoubleProperty";
        } else if ctx.is_dart_core_string(ty) {
            ctor_id = "StringProperty";
        } else if is_enum {
            ctor_id = "EnumProperty";
            type_args = Some(vec![ty]);
        } else if is_iterable {
            ctor_id = "IterableProperty";
            if let TypeKind::Interface { args, .. } = *ctx.ty(ty) {
                type_args = Some(ctx.list(args).to_vec());
            }
        } else if is_color {
            ctor_id = "ColorProperty";
        } else if is_matrix4 {
            ctor_id = "TransformProperty";
        } else {
            ctor_id = "DiagnosticsProperty";
            if !matches!(ctx.ty(ty), TypeKind::Dynamic | TypeKind::Invalid) {
                type_args = Some(vec![ty]);
            }
        }

        let Some(class_body) = ast.cast::<BlockClassBody>(ast[class_decl].body) else {
            return;
        };
        let debug_method = ast
            .list_raw(ast[class_body].members)
            .iter()
            .copied()
            .find_map(|m| {
                let md = ast.cast::<MethodDeclaration>(m)?;
                (c.lexeme(ast[md].name) == "debugFillProperties").then_some(md)
            });

        let write_prop_ref =
            |e: &mut super::super::change_builder::EditBuilder<'_, '_, '_>,
             prefix: &str,
             builder_name: &str| {
                e.write(&format!("{prefix}{builder_name}.add({ctor_id}"));
                if let Some(tas) = &type_args {
                    e.write("<");
                    for (i, &ta) in tas.iter().enumerate() {
                        if i > 0 {
                            e.write(", ");
                        }
                        e.write_type(
                            Some(ta),
                            &WriteType {
                                should_write_dynamic: true,
                                ..Default::default()
                            },
                        );
                    }
                    e.write(">");
                } else if matches!(ctx.ty(ty), TypeKind::Dynamic | TypeKind::Invalid)
                    && let Some(dt) = decl_type_ann
                {
                    let type_text = c.utils.get_node_text(dt.raw());
                    if type_text != "dynamic" {
                        e.write("<");
                        e.write(&type_text);
                        e.write(">");
                    }
                }
                e.writeln(&format!("{ctor_name}('{name_id}', {name_id}));"));
            };

        if let Some(dm) = debug_method {
            let Some(bfb) = ast.cast::<BlockFunctionBody>(ast[dm].body) else {
                return;
            };
            let Some(param_list) = ast[dm].parameters else {
                return;
            };
            let mut builder_name: Option<String> = None;
            for &p in ast.list_raw(ast[param_list].parameters) {
                if let Some(rfp) = ast.cast::<RegularFormalParameter>(p)
                    && ast[rfp].function_typed_suffix.is_none()
                    && let Some(t) = ast[rfp].type_
                    && let Some(nt) = ast.cast::<NamedType>(t)
                    && let Some(id_tok) = ast[rfp].name
                    && c.lexeme(ast[nt].name) == "DiagnosticPropertiesBuilder"
                {
                    builder_name = Some(c.lexeme(id_tok).to_string());
                    break;
                }
            }
            let Some(builder_name) = builder_name else {
                return;
            };
            let block = ast[bfb].block;
            let stmts = ast.list_raw(ast[block].statements);
            let (offset, prefix) = if stmts.is_empty() {
                let off = c.token_offset(ast[block].left_bracket);
                (off, format!("{}  ", c.utils.get_line_prefix(off)))
            } else {
                let off = c.token_offset(ast.end_token(*stmts.last().unwrap()));
                (off, c.utils.get_line_prefix(off))
            };
            let insert_offset = c.utils.get_line_next(offset);
            builder.add_dart_file_edit(c.path, |b| {
                b.add_insertion(insert_offset, |e| {
                    write_prop_ref(e, &prefix, &builder_name);
                });
            });
        } else {
            builder.add_dart_file_edit(c.path, |b| {
                b.insert_method(class_decl.raw(), |e| {
                    e.writeln("@override");
                    e.write_indent(1);
                    e.writeln("void debugFillProperties(DiagnosticPropertiesBuilder properties) {");
                    e.write_indent(2);
                    e.writeln("super.debugFillProperties(properties);");
                    write_prop_ref(e, "    ", "properties");
                    e.write_indent(1);
                    e.write("}");
                });
            });
        }
    }
);

// ---------------------------------------------------------------------------
// AddDigitSeparators
// ---------------------------------------------------------------------------

fn add_digit_separators(
    c: &ProducerContext<'_>,
    builder: &mut ChangeBuilder<'_>,
    group_size: usize,
) {
    if !c.ctx.features.is_enabled("digit-separators") {
        return;
    }
    let ast = c.ast;
    let tok = if let Some(il) = ast.cast::<IntegerLiteral>(c.node) {
        ast[il].literal
    } else if let Some(dl) = ast.cast::<DoubleLiteral>(c.node) {
        ast[dl].literal
    } else {
        return;
    };
    let lexeme = c.lexeme(tok);
    if lexeme.contains('_') {
        return;
    }
    let lower = lexeme.to_ascii_lowercase();
    if lower.starts_with("0x") || lower.starts_with("0b") {
        if group_size == 2 {
            return;
        }
        let prefix = &lexeme[..2];
        let digits = &lexeme[2..];
        let step = if lower.starts_with("0x") { 4 } else { 4 };
        if digits.len() <= step {
            return;
        }
        let mut parts = Vec::new();
        let mut end = digits.len();
        while end > step {
            parts.push(&digits[end - step..end]);
            end -= step;
        }
        parts.push(&digits[..end]);
        parts.reverse();
        let r = c.range().token(tok);
        let rep = format!("{prefix}{}", parts.join("_"));
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(r.offset, r.length, &rep);
        });
        return;
    }
    let (int_part, rest) = match lexeme.find(|ch: char| ch == '.' || ch == 'e' || ch == 'E') {
        Some(idx) => (&lexeme[..idx], &lexeme[idx..]),
        None => (lexeme, ""),
    };
    let min_digits = if group_size == 3 { 5 } else { 4 };
    if int_part.len() < min_digits {
        return;
    }
    let mut parts = Vec::new();
    let mut end = int_part.len();
    if group_size == 2 {
        // Indian numbering: 3 digits at right, then groups of 2.
        parts.push(&int_part[end - 3..end]);
        end -= 3;
        while end > 2 {
            parts.push(&int_part[end - 2..end]);
            end -= 2;
        }
        parts.push(&int_part[..end]);
    } else {
        while end > 3 {
            parts.push(&int_part[end - 3..end]);
            end -= 3;
        }
        parts.push(&int_part[..end]);
    }
    parts.reverse();
    let r = c.range().token(tok);
    let rep = format!("{}{rest}", parts.join("_"));
    builder.add_dart_file_edit(c.path, |b| {
        b.add_simple_replacement(r.offset, r.length, &rep);
    });
}

assist!(
    AddDigitSeparatorEveryThreeDigits,
    a::ADD_DIGIT_SEPARATORS,
    |c, builder| {
        add_digit_separators(c, builder, 3);
    }
);

assist!(
    AddDigitSeparatorEveryTwoDigits,
    a::ADD_DIGIT_SEPARATORS,
    |c, builder| {
        add_digit_separators(c, builder, 2);
    }
);

// ---------------------------------------------------------------------------
// ConvertAddAllToSpread & InlineInvocation
// ---------------------------------------------------------------------------

pub struct ConvertAddAllToSpread {
    is_inline_invocation: bool,
}

impl ConvertAddAllToSpread {
    pub fn new() -> Self {
        Self {
            is_inline_invocation: false,
        }
    }
}

impl CorrectionProducer for ConvertAddAllToSpread {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(if self.is_inline_invocation {
            &k::INLINE_INVOCATION
        } else {
            &k::CONVERT_TO_SPREAD
        })
    }

    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        Some(if self.is_inline_invocation {
            &k::INLINE_INVOCATION_MULTI
        } else {
            &k::CONVERT_TO_SPREAD_MULTI
        })
    }

    fn fix_arguments(&self) -> Vec<String> {
        if self.is_inline_invocation {
            vec!["addAll".to_string()]
        } else {
            Vec::new()
        }
    }

    fn assist_kind(&self) -> Option<&'static FixKind> {
        Some(if self.is_inline_invocation {
            &a::INLINE_INVOCATION
        } else {
            &a::CONVERT_TO_SPREAD
        })
    }

    fn assist_arguments(&self) -> Vec<String> {
        if self.is_inline_invocation {
            vec!["addAll".to_string()]
        } else {
            Vec::new()
        }
    }

    fn applicability(&self) -> Applicability {
        Applicability::Automatically
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let Some(id) = ast.cast::<SimpleIdentifier>(c.node) else {
            return;
        };
        if c.lexeme(ast[id].token) != "addAll" {
            return;
        }
        let Some(mi) = ast.parent(id).and_then(|p| ast.cast::<MethodInvocation>(p)) else {
            return;
        };
        if ast[mi].method_name != id || !is_cascaded_invocation(ast, mi) {
            return;
        }
        let Some(ce) = ast
            .parent(mi)
            .and_then(|p| ast.cast::<CascadeExpression>(p))
        else {
            return;
        };
        let sections = ast.list_raw(ast[ce].cascade_sections);
        if sections.first().copied() != Some(mi.raw()) {
            return;
        }
        let Some(target_list) = ast.cast::<ListLiteral>(ast[ce].target) else {
            return;
        };
        let args = ast.list_raw(ast[ast[mi].argument_list].arguments);
        if args.len() != 1 {
            return;
        }
        let arg = args[0];
        let element_text = if let Some(ll) = ast.cast::<ListLiteral>(arg) {
            self.is_inline_invocation = true;
            let elems = ast.list_raw(ast[ll].elements);
            if elems.is_empty() {
                return;
            }
            c.utils
                .get_range_text(c.range().start_end(elems[0], *elems.last().unwrap()))
        } else {
            format!("...{}", c.utils.get_node_text(arg))
        };
        let target_elems = ast.list_raw(ast[target_list].elements);
        let insert_offset = target_elems
            .last()
            .map(|&l| ast.end(l))
            .unwrap_or_else(|| c.token_end(ast[target_list].left_bracket));
        let prefix = if target_elems.is_empty() { "" } else { ", " };
        let del_range = if sections.len() == 1 {
            c.range().end_end(target_list, ce)
        } else {
            c.range().node(mi)
        };
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_insertion(insert_offset, &format!("{prefix}{element_text}"));
            b.add_deletion(del_range.offset, del_range.length);
        });
    }
}

pub struct InlineInvocation;

impl CorrectionProducer for InlineInvocation {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::INLINE_INVOCATION)
    }

    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::INLINE_INVOCATION_MULTI)
    }

    fn fix_arguments(&self) -> Vec<String> {
        vec!["add".to_string()]
    }

    fn assist_kind(&self) -> Option<&'static FixKind> {
        Some(&a::INLINE_INVOCATION)
    }

    fn assist_arguments(&self) -> Vec<String> {
        vec!["add".to_string()]
    }

    fn applicability(&self) -> Applicability {
        Applicability::Automatically
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let Some(id) = ast.cast::<SimpleIdentifier>(c.node) else {
            return;
        };
        if c.lexeme(ast[id].token) != "add" {
            return;
        }
        let Some(mi) = ast.parent(id).and_then(|p| ast.cast::<MethodInvocation>(p)) else {
            return;
        };
        if ast[mi].method_name != id || !is_cascaded_invocation(ast, mi) {
            return;
        }
        let Some(ce) = ast
            .parent(mi)
            .and_then(|p| ast.cast::<CascadeExpression>(p))
        else {
            return;
        };
        let sections = ast.list_raw(ast[ce].cascade_sections);
        if sections.first().copied() != Some(mi.raw()) {
            return;
        }
        let Some(target_list) = ast.cast::<ListLiteral>(ast[ce].target) else {
            return;
        };
        let args = ast.list_raw(ast[ast[mi].argument_list].arguments);
        if args.len() != 1 {
            return;
        }
        let arg_text = c.utils.get_node_text(args[0]);
        let target_elems = ast.list_raw(ast[target_list].elements);
        let insert_offset = target_elems
            .last()
            .map(|&l| ast.end(l))
            .unwrap_or_else(|| c.token_end(ast[target_list].left_bracket));
        let prefix = if target_elems.is_empty() { "" } else { ", " };
        let del_range = if sections.len() == 1 {
            c.range().end_end(target_list, ce)
        } else {
            c.range().node(mi)
        };
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_insertion(insert_offset, &format!("{prefix}{arg_text}"));
            b.add_deletion(del_range.offset, del_range.length);
        });
    }
}

// ---------------------------------------------------------------------------
// ConvertConditionalExpressionToIfElement
// ---------------------------------------------------------------------------

producer!(
    ConvertConditionalExpressionToIfElement,
    k::CONVERT_TO_IF_ELEMENT,
    Some(&k::CONVERT_TO_IF_ELEMENT_MULTI),
    Automatically,
    assist: Some(&a::CONVERT_TO_IF_ELEMENT),
    |c, builder| {
        let ast = c.ast;
        let mut node = c.node;
        while let Some(p) = ast.parent(node) {
            if ast.is::<ConditionalExpression>(node) {
                break;
            }
            node = p;
        }
        let Some(cond_expr) = ast.cast::<ConditionalExpression>(node) else {
            return;
        };
        let mut parent_to_replace = cond_expr.raw();
        while let Some(p) = ast.parent(parent_to_replace) {
            if ast.is::<ParenthesizedExpression>(p) {
                parent_to_replace = p;
            } else {
                break;
            }
        }
        let Some(container) = ast.parent(parent_to_replace) else {
            return;
        };
        if !ast.is::<ListLiteral>(container) && !ast.is::<SetOrMapLiteral>(container) {
            return;
        }
        let cond = c.utils.get_node_text(ast[cond_expr].condition);
        let then_e = c.utils.get_node_text(ast[cond_expr].then_expression);
        let else_e = c.utils.get_node_text(ast[cond_expr].else_expression);
        let r = c.range().node(parent_to_replace);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(
                r.offset,
                r.length,
                &format!("if ({cond}) {then_e} else {else_e}"),
            );
        });
    }
);

// ---------------------------------------------------------------------------
// ConvertDocumentationIntoLine
// ---------------------------------------------------------------------------

producer!(
    ConvertDocumentationIntoLine,
    k::CONVERT_TO_LINE_COMMENT,
    Some(&k::CONVERT_TO_LINE_COMMENT_MULTI),
    Automatically,
    assist: Some(&a::CONVERT_DOCUMENTATION_INTO_LINE),
    |c, builder| {
        let ast = c.ast;
        let Some(comment) = ast.this_or_ancestor_of_type::<Comment>(c.node) else {
            return;
        };
        let tokens = ast.token_list(ast[comment].tokens);
        if tokens.len() != 1 {
            return;
        }
        let first = tokens[0];
        let text = c.lexeme(first);
        if !text.starts_with("/**") {
            return;
        }
        let eol = c.eol();
        let lines: Vec<&str> = text.split(eol).collect();
        let prefix = c.utils.get_node_prefix(comment.raw());
        let mut new_lines = Vec::new();
        let mut first_line = true;
        let mut line_prefix = String::new();
        for mut line in lines {
            if first_line {
                first_line = false;
                let expected = &line[3..];
                if !expected.trim().is_empty() {
                    new_lines.push(format!("///{expected}"));
                }
                continue;
            }
            line = line.trim_start();
            if line.starts_with("*/") {
                break;
            }
            if line.starts_with('*') {
                line = &line[1..];
                if line.is_empty() {
                    new_lines.push(format!("{line_prefix}///"));
                } else {
                    new_lines.push(format!("{line_prefix}///{line}"));
                }
            } else {
                new_lines.push(format!("{line_prefix}/// {line}"));
            }
            line_prefix = prefix.clone();
        }
        let r = c.range().node(comment);
        let rep = new_lines.join(eol);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(r.offset, r.length, &rep);
        });
    }
);

// ---------------------------------------------------------------------------
// ConvertIntoForIndex
// ---------------------------------------------------------------------------

assist!(
    ConvertIntoForIndex,
    a::CONVERT_INTO_FOR_INDEX,
    |c, builder| {
        let ast = c.ast;
        let ctx = c.ctx;
        let Some(for_stmt) = ast.this_or_ancestor_of_type::<ForStatement>(c.node) else {
            return;
        };
        if c.selection_offset < ast.offset(for_stmt)
            || c.token_end(ast[for_stmt].right_parenthesis) < c.selection_offset
        {
            return;
        }
        let Some(parts) = ast.cast::<ForEachPartsWithDeclaration>(ast[for_stmt].for_loop_parts)
        else {
            return;
        };
        let Some(block) = ast.cast::<Block>(ast[for_stmt].body) else {
            return;
        };
        let iterable = ast[parts].iterable.raw();
        let Some(id) = ast.cast::<SimpleIdentifier>(iterable) else {
            return;
        };
        let Some(elem) = c.locator().element(id) else {
            return;
        };
        if !matches!(
            elem.tag(),
            Tag::LocalVariable | Tag::TopLevelVariable | Tag::Field | Tag::FormalParameter
        ) {
            return;
        }
        let list_name = c.lexeme(ast[id].token).to_string();
        let Some(iter_ty) = static_type(c, iterable) else {
            return;
        };
        let TypeKind::Interface { element, .. } = *ctx.ty(iter_ty) else {
            return;
        };
        if !ctx.is_element(element.raw(), "dart.core", "List") {
            return;
        }
        let excluded = super::assists2::scoped_names(c, for_stmt.raw(), ast.offset(for_stmt));
        let mut index_name = None;
        for ch in ['i', 'j', 'k'] {
            let s = ch.to_string();
            if !excluded.contains(&s) {
                index_name = Some(s);
                break;
            }
        }
        let Some(index_name) = index_name else {
            return;
        };
        let loop_var = ast[parts].loop_variable;
        let loop_var_text = c.utils.get_node_text(loop_var);
        let prefix = c.utils.get_node_prefix(for_stmt.raw());
        let indent = c.utils.one_indent();
        let eol = c.eol();
        let first_block_line = c
            .utils
            .get_line_content_end(c.token_end(ast[block].left_bracket));
        let header_range = c
            .range()
            .node_start_token_end(for_stmt, ast[for_stmt].right_parenthesis);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(
                header_range.offset,
                header_range.length,
                &format!(
                    "for (int {index_name} = 0; {index_name} < {list_name}.length; {index_name}++)"
                ),
            );
            b.add_simple_insertion(
                first_block_line,
                &format!("{prefix}{indent}{loop_var_text} = {list_name}[{index_name}];{eol}"),
            );
        });
    }
);

// ---------------------------------------------------------------------------
// ConvertIntoIsNot & ConvertIntoIsNotEmpty
// ---------------------------------------------------------------------------

producer!(
    ConvertIntoIsNot,
    k::CONVERT_TO_IS_NOT,
    Some(&k::CONVERT_TO_IS_NOT_MULTI),
    Automatically,
    assist: Some(&a::CONVERT_INTO_IS_NOT),
    |c, builder| {
        let ast = c.ast;
        let mut is_expr = ast.cast::<IsExpression>(c.node);
        if is_expr.is_none() {
            if let Some(pe) = ast.cast::<ParenthesizedExpression>(c.node) {
                is_expr = ast.cast::<IsExpression>(ast[pe].expression);
            } else if let Some(pref) = ast.cast::<PrefixExpression>(c.node) {
                if ast.tokens.ty(ast[pref].operator) == TokenType::BANG {
                    if let Some(pe) = ast.cast::<ParenthesizedExpression>(ast[pref].operand) {
                        is_expr = ast.cast::<IsExpression>(ast[pe].expression);
                    }
                }
            } else if let Some(p) = ast.parent(c.node) {
                is_expr = ast.cast::<IsExpression>(p);
            }
        }
        let Some(is_expr) = is_expr else {
            return;
        };
        if ast[is_expr].not_operator.is_some() {
            return;
        }
        let Some(parens) = ast
            .parent(is_expr)
            .and_then(|p| ast.cast::<ParenthesizedExpression>(p))
        else {
            return;
        };
        let Some(pref) = ast
            .parent(parens)
            .and_then(|p| ast.cast::<PrefixExpression>(p))
        else {
            return;
        };
        if ast.tokens.ty(ast[pref].operator) != TokenType::BANG {
            return;
        }
        let r1 = c
            .range()
            .token_start_end(ast[pref].operator, ast[parens].left_parenthesis);
        let r2 = c.range().token(ast[parens].right_parenthesis);
        let is_end = c.token_end(ast[is_expr].is_operator);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(r1.offset, r1.length);
            b.add_simple_insertion(is_end, "!");
            b.add_deletion(r2.offset, r2.length);
        });
    }
);

assist!(
    ConvertIntoIsNotEmpty,
    a::CONVERT_INTO_IS_NOT_EMPTY,
    |c, builder| {
        let ast = c.ast;
        let mut is_empty_id = None;
        let mut is_empty_access = None;
        if let Some(id) = ast.cast::<SimpleIdentifier>(c.node) {
            if let Some(p) = ast.parent(id) {
                if let Some(pi) = ast.cast::<PrefixedIdentifier>(p) {
                    if ast[pi].identifier == id {
                        is_empty_id = Some(id);
                        is_empty_access = Some(p);
                    } else {
                        is_empty_id = Some(ast[pi].identifier);
                        is_empty_access = Some(p);
                    }
                } else if let Some(pa) = ast.cast::<PropertyAccess>(p) {
                    is_empty_id = Some(ast[pa].property_name);
                    is_empty_access = Some(p);
                }
            }
        } else if let Some(pref) = ast.cast::<PrefixExpression>(c.node) {
            let op_node = ast[pref].operand.raw();
            if let Some(pi) = ast.cast::<PrefixedIdentifier>(op_node) {
                is_empty_id = Some(ast[pi].identifier);
                is_empty_access = Some(op_node);
            } else if let Some(pa) = ast.cast::<PropertyAccess>(op_node) {
                is_empty_id = Some(ast[pa].property_name);
                is_empty_access = Some(op_node);
            }
        }
        let (Some(id), Some(access)) = (is_empty_id, is_empty_access) else {
            return;
        };
        if c.lexeme(ast[id].token) != "isEmpty" {
            return;
        }
        let Some(pref) = ast
            .parent(access)
            .and_then(|p| ast.cast::<PrefixExpression>(p))
        else {
            return;
        };
        if ast.tokens.ty(ast[pref].operator) != TokenType::BANG {
            return;
        }
        let Some(prop_elem) = c.locator().element(id) else {
            return;
        };
        let Some(enclosing) = c.ctx.element_data(prop_elem).and_then(|d| d.enclosing) else {
            return;
        };
        let Some(iface) = enclosing.cast::<InterfaceElement>() else {
            return;
        };
        let has_is_not_empty = c
            .ctx
            .interface(iface)
            .getters
            .iter()
            .any(|&g| c.ctx.element_name(g.raw()) == Some("isNotEmpty"))
            || c.ctx
                .element_all_supertypes(iface)
                .iter()
                .any(|&st| match *c.ctx.ty(st) {
                    TypeKind::Interface { element: se, .. } => c
                        .ctx
                        .interface(se)
                        .getters
                        .iter()
                        .any(|&g| c.ctx.element_name(g.raw()) == Some("isNotEmpty")),
                    _ => false,
                });
        if !has_is_not_empty {
            return;
        }
        let bang_r = c.range().token(ast[pref].operator);
        let id_r = c.range().node(id);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(bang_r.offset, bang_r.length);
            b.add_simple_replacement(id_r.offset, id_r.length, "isNotEmpty");
        });
    }
);

// ---------------------------------------------------------------------------
// ConvertToGenericFunctionSyntax
// ---------------------------------------------------------------------------

fn all_parameters_have_types(ast: &Ast, parameters: Id<FormalParameterList>) -> bool {
    for &param in ast.list_raw(ast[parameters].parameters) {
        if let Some(rfp) = ast.cast::<RegularFormalParameter>(param) {
            if ast[rfp].function_typed_suffix.is_none() && ast[rfp].type_.is_none() {
                return false;
            }
        } else if let Some(ffp) = ast.cast::<FieldFormalParameter>(param) {
            if ast[ffp].function_typed_suffix.is_none() && ast[ffp].type_.is_none() {
                return false;
            }
        } else if let Some(sfp) = ast.cast::<SuperFormalParameter>(param) {
            if ast[sfp].function_typed_suffix.is_none() && ast[sfp].type_.is_none() {
                return false;
            }
        }
    }
    true
}

producer!(
    ConvertToGenericFunctionSyntax,
    k::CONVERT_TO_GENERIC_FUNCTION_SYNTAX,
    Some(&k::CONVERT_TO_GENERIC_FUNCTION_SYNTAX_MULTI),
    Automatically,
    assist: Some(&a::CONVERT_INTO_GENERIC_FUNCTION_SYNTAX),
    |c, builder| {
        let ast = c.ast;
        let mut cur = Some(c.node);
        while let Some(n) = cur {
            if let Some(fta) = ast.cast::<FunctionTypeAlias>(n) {
                let params_node = ast[fta].parameters;
                if c.diagnostic.is_none() && c.selection_offset > ast.offset(params_node) {
                    return;
                }
                if !all_parameters_have_types(ast, params_node) {
                    return;
                }
                let return_type = ast[fta]
                    .return_type
                    .map(|t| format!("{} ", c.utils.get_node_text(t)))
                    .unwrap_or_default();
                let name = c.lexeme(ast[fta].name);
                let type_params = ast[fta]
                    .type_parameters
                    .map(|tp| c.utils.get_node_text(tp))
                    .unwrap_or_default();
                let params = c.utils.get_node_text(params_node);
                let r = c
                    .range()
                    .token_end_end(ast[fta].typedef_keyword, ast.end_token(params_node));
                let rep = format!(" {name}{type_params} = {return_type}Function{params}");
                builder.add_dart_file_edit(c.path, |b| {
                    b.add_simple_replacement(r.offset, r.length, &rep);
                });
                return;
            }
            if let Some(rfp) = ast.cast::<RegularFormalParameter>(n) {
                if let Some(fts) = ast[rfp].function_typed_suffix {
                    let fp_params = ast[fts].formal_parameters;
                    if c.diagnostic.is_none() && c.selection_offset > ast.offset(fp_params) {
                        return;
                    }
                    if !all_parameters_have_types(ast, fp_params) {
                        return;
                    }
                    let Some(name_tok) = ast[rfp].name else {
                        return;
                    };
                    let ret_ty = ast[rfp]
                        .type_
                        .map(|t| format!("{} ", c.utils.get_node_text(t)))
                        .unwrap_or_default();
                    let tp = ast[fts]
                        .type_parameters
                        .map(|t| c.utils.get_node_text(t))
                        .unwrap_or_default();
                    let params = c.utils.get_node_text(fp_params);
                    let q = ast[fts].question.map(|_| "?").unwrap_or_default();
                    let name = c.lexeme(name_tok);
                    let start = ast[rfp]
                        .type_
                        .map(|t| ast.offset(t))
                        .unwrap_or_else(|| c.token_offset(name_tok));
                    let end = ast.end(fts);
                    let rep = format!("{ret_ty}Function{tp}{params}{q} {name}");
                    builder.add_dart_file_edit(c.path, |b| {
                        b.add_simple_replacement(start, end - start, &rep);
                    });
                    return;
                }
            }
            if ast.is::<ArgumentList>(n) || (ast.is::<FormalParameterList>(n) && c.diagnostic.is_none()) {
                return;
            }
            cur = ast.parent(n);
        }
    }
);

// ---------------------------------------------------------------------------
// ConvertToSetLiteral
// ---------------------------------------------------------------------------

producer!(
    ConvertToSetLiteral,
    k::CONVERT_TO_SET_LITERAL,
    Some(&k::CONVERT_TO_SET_LITERAL_MULTI),
    Automatically,
    assist: Some(&a::CONVERT_TO_SET_LITERAL),
    |c, builder| {
        let ast = c.ast;
        let ctx = c.ctx;
        let Some(creation) = ast.this_or_ancestor_of_type::<InstanceCreationExpression>(c.node)
        else {
            return;
        };
        if c.selection_offset > ast.offset(ast[creation].argument_list) {
            return;
        }
        let Some(ty) = static_type(c, creation.raw()) else {
            return;
        };
        let TypeKind::Interface { element, .. } = *ctx.ty(ty) else {
            return;
        };
        if !ctx.is_element(element.raw(), "dart.core", "Set") {
            return;
        }
        let cn = ast[creation].constructor_name;
        let ctor_name = ast[cn].name.map(|n| c.lexeme(ast[n].token));
        let type_args = ast[ast[cn].type_]
            .type_arguments
            .map(|ta| c.utils.get_node_text(ta))
            .unwrap_or_default();
        let args = ast.list_raw(ast[ast[creation].argument_list].arguments);
        let r = c.range().node(creation);
        match ctor_name {
            None | Some("identity") => {
                if ctor_name == Some("identity") || !args.is_empty() {
                    return;
                }
                let rep = if type_args.is_empty() {
                    "<dynamic>{}".to_string()
                } else {
                    format!("{type_args}{{}}")
                };
                builder.add_dart_file_edit(c.path, |b| {
                    b.add_simple_replacement(r.offset, r.length, &rep);
                });
            }
            Some("from") | Some("of") => {
                if args.len() != 1 {
                    return;
                }
                let Some(ll) = ast.cast::<ListLiteral>(args[0]) else {
                    return;
                };
                let elems = ast.list_raw(ast[ll].elements);
                let elems_text = if elems.is_empty() {
                    String::new()
                } else {
                    c.utils
                        .get_range_text(c.range().start_end(elems[0], *elems.last().unwrap()))
                };
                let list_ta = ast[ll]
                    .type_arguments
                    .map(|ta| c.utils.get_node_text(ta))
                    .unwrap_or_default();
                let ta = if !type_args.is_empty() {
                    type_args
                } else if !list_ta.is_empty() {
                    list_ta
                } else if elems.is_empty() {
                    "<dynamic>".to_string()
                } else {
                    String::new()
                };
                let rep = format!("{ta}{{{elems_text}}}");
                builder.add_dart_file_edit(c.path, |b| {
                    b.add_simple_replacement(r.offset, r.length, &rep);
                });
            }
            _ => {}
        }
    }
);

// ---------------------------------------------------------------------------
// JoinVariableDeclaration
// ---------------------------------------------------------------------------

assist!(
    JoinVariableDeclaration,
    a::JOIN_VARIABLE_DECLARATION,
    |c, builder| {
        let ast = c.ast;
        if let Some(left) = ast.cast::<SimpleIdentifier>(c.node) {
            if let Some(assign) = ast
                .parent(left)
                .and_then(|p| ast.cast::<AssignmentExpression>(p))
                && ast[assign].left_hand_side.raw() == left.raw()
                && let Some(es) = ast
                    .parent(assign)
                    .and_then(|p| ast.cast::<ExpressionStatement>(p))
            {
                if ast.tokens.ty(ast[assign].operator) != TokenType::EQ {
                    return;
                }
                let Some(block) = ast.parent(es).and_then(|p| ast.cast::<Block>(p)) else {
                    return;
                };
                let stmts = ast.list_raw(ast[block].statements);
                let Some(idx) = stmts.iter().position(|&s| s == es.raw()) else {
                    return;
                };
                if idx < 1 {
                    return;
                }
                let Some(vds) = ast.cast::<VariableDeclarationStatement>(stmts[idx - 1]) else {
                    return;
                };
                let vars = ast.list(ast[ast[vds].variables].variables);
                if vars.len() != 1 {
                    return;
                }
                let decl_var = vars[0];
                if c.locator().declared_element(decl_var) != c.locator().write_or_read_element(left)
                    || ast[decl_var].initializer.is_some()
                {
                    return;
                }
                let r = c
                    .range()
                    .token_end_start(ast[decl_var].name, ast[assign].operator);
                builder.add_dart_file_edit(c.path, |b| {
                    b.add_simple_replacement(r.offset, r.length, " ");
                });
                return;
            }
        }
        let Some(decl_list) = ast.this_or_ancestor_of_type::<VariableDeclarationList>(c.node)
        else {
            return;
        };
        let vars = ast.list(ast[decl_list].variables);
        if vars.len() != 1 {
            return;
        }
        let decl_var = vars[0];
        if ast[decl_var].initializer.is_some() {
            return;
        }
        let Some(vds) = ast
            .parent(decl_list)
            .and_then(|p| ast.cast::<VariableDeclarationStatement>(p))
        else {
            return;
        };
        let Some(block) = ast.parent(vds).and_then(|p| ast.cast::<Block>(p)) else {
            return;
        };
        let stmts = ast.list_raw(ast[block].statements);
        let Some(idx) = stmts.iter().position(|&s| s == vds.raw()) else {
            return;
        };
        if idx + 1 >= stmts.len() {
            return;
        }
        let Some(es) = ast.cast::<ExpressionStatement>(stmts[idx + 1]) else {
            return;
        };
        let Some(assign) = ast.cast::<AssignmentExpression>(ast[es].expression) else {
            return;
        };
        let Some(lhs_id) = ast.cast::<SimpleIdentifier>(ast[assign].left_hand_side) else {
            return;
        };
        if c.locator().write_or_read_element(lhs_id) != c.locator().declared_element(decl_var) {
            return;
        }
        if ast.tokens.ty(ast[assign].operator) != TokenType::EQ {
            return;
        }
        let r = c
            .range()
            .token_end_start(ast[decl_var].name, ast[assign].operator);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(r.offset, r.length, " ");
        });
    }
);

// ---------------------------------------------------------------------------
// SortChildPropertyLast
// ---------------------------------------------------------------------------

producer!(
    SortChildPropertyLast,
    k::SORT_CHILD_PROPERTY_LAST,
    Some(&k::SORT_CHILD_PROPERTY_LAST_MULTI),
    Automatically,
    assist: Some(&a::SORT_CHILD_PROPERTY_LAST),
    |c, builder| {
        let ast = c.ast;
        let Some(named_arg) = ast.this_or_ancestor_of_type::<NamedArgument>(c.node) else {
            return;
        };
        let prop_name = c.lexeme(ast[named_arg].name);
        if prop_name != "child" && prop_name != "children" {
            return;
        }
        let Some(arg_list) = ast.parent(named_arg).and_then(|p| ast.cast::<ArgumentList>(p)) else {
            return;
        };
        let Some(creation) = ast
            .parent(arg_list)
            .and_then(|p| ast.cast::<InstanceCreationExpression>(p))
        else {
            return;
        };
        if !super::super::flutter::is_widget_creation(c, creation) {
            return;
        }
        let args = ast.list_raw(ast[arg_list].arguments);
        if args.last().copied() == Some(named_arg.raw()) {
            return;
        }
        // Check that named_arg is the only child/children property or move it to the end.
        let last_arg = *args.last().unwrap();
        let has_trailing_comma =
            ast.tokens.ty(ast.tokens.next(ast.end_token(last_arg))) == TokenType::COMMA;
        let del_range = c.range().node_in_list(args, named_arg.raw());
        let arg_text = c.utils.get_node_text(named_arg);
        let insert_offset = if has_trailing_comma {
            c.token_end(ast.tokens.next(ast.end_token(last_arg)))
        } else {
            ast.end(last_arg)
        };
        let replacement = if has_trailing_comma {
            let prefix = c.utils.get_node_prefix(last_arg);
            let eol = c.eol();
            format!("{eol}{prefix}{arg_text},")
        } else {
            format!(", {arg_text}")
        };
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(del_range.offset, del_range.length);
            b.add_simple_insertion(insert_offset, &replacement);
        });
    }
);

// ---------------------------------------------------------------------------
// ShadowField
// ---------------------------------------------------------------------------

assist!(ShadowField, a::SHADOW_FIELD, |c, builder| {
    let ast = c.ast;
    let Some(id) = ast.cast::<SimpleIdentifier>(c.node) else {
        return;
    };
    let Some(elem) = c.locator().write_or_read_element(id) else {
        return;
    };
    let Some(getter_id) = elem.cast::<dartr_element::GetterElement>() else {
        return;
    };
    if !c
        .ctx
        .element_data(elem)
        .and_then(|d| d.enclosing)
        .is_some_and(|e| e.cast::<InterfaceElement>().is_some())
    {
        return;
    }
    let enclosing_if = |expr: NodeId| {
        let mut p = ast.parent(expr);
        while let Some(pn) = p {
            if let Some(bin) = ast.cast::<BinaryExpression>(pn)
                && ast.tokens.ty(ast[bin].operator) == TokenType::AMPERSAND_AMPERSAND
            {
                p = ast.parent(pn);
            } else {
                break;
            }
        }
        p.and_then(|pn| ast.cast::<IfStatement>(pn))
    };
    let Some(parent) = ast.parent(id) else {
        return;
    };
    let stmt = if let Some(is_expr) = ast.cast::<IsExpression>(parent)
        && ast[is_expr].expression.raw() == id.raw()
    {
        enclosing_if(parent)
    } else if let Some(bin) = ast.cast::<BinaryExpression>(parent)
        && matches!(
            ast.tokens.ty(ast[bin].operator),
            TokenType::EQ_EQ | TokenType::BANG_EQ
        )
    {
        enclosing_if(parent)
    } else {
        None
    };
    let Some(stmt) = stmt else {
        return;
    };
    let Some(enclosing_block) = ast.parent(stmt).and_then(|p| ast.cast::<Block>(p)) else {
        return;
    };
    let Some(setter_el) = c
        .ctx
        .get(getter_id)
        .variable
        .get()
        .and_then(|v| c.ctx.property_inducing(v).setter)
        .map(|s| s.raw())
    else {
        return;
    };
    let mut is_written = false;
    super::variables::visit(ast, enclosing_block.raw(), &mut |n| {
        if let Some(sid) = ast.cast::<SimpleIdentifier>(n) {
            if c.locator().write_or_read_element(sid) == Some(setter_el) {
                is_written = true;
            }
        }
    });
    if is_written {
        return;
    }
    let field_name = c.lexeme(ast[id].token).to_string();
    let offset = ast.offset(stmt);
    let prefix = c.utils.get_line_prefix(offset);
    builder.add_dart_file_edit(c.path, |b| {
        b.add_insertion(offset, |e| {
            e.write("var ");
            e.write(&field_name);
            e.write(" = this.");
            e.write(&field_name);
            e.writeln(";");
            e.write(&prefix);
        });
    });
});

// ---------------------------------------------------------------------------
// RemoveUnnecessaryName
// ---------------------------------------------------------------------------

producer!(
    RemoveUnnecessaryName,
    k::REMOVE_UNNECESSARY_NAME,
    Some(&k::REMOVE_UNNECESSARY_NAME_MULTI),
    Automatically,
    assist: Some(&a::REMOVE_UNNECESSARY_NAME),
    |c, builder| {
        let ast = c.ast;
        let mut node = c.node;
        if let Some(p) = ast.parent(node) {
            if ast.is::<PatternField>(p) {
                node = p;
            }
        }
        let Some(pf) = ast.cast::<PatternField>(node) else {
            return;
        };
        let Some(dvp) = ast.cast::<DeclaredVariablePattern>(ast[pf].pattern) else {
            return;
        };
        let Some(name_node) = ast[pf].name else {
            return;
        };
        let Some(name_tok) = ast[name_node].name else {
            return;
        };
        if !ast.tokens.get(name_tok).is_synthetic()
            && c.lexeme(ast[dvp].name) == c.lexeme(name_tok)
        {
            let r = c.range().token(name_tok);
            builder.add_dart_file_edit(c.path, |b| {
                b.add_deletion(r.offset, r.length);
            });
        }
    }
);

// ---------------------------------------------------------------------------
// ConvertToNullAware
// ---------------------------------------------------------------------------

producer!(
    ConvertToNullAware,
    k::CONVERT_TO_NULL_AWARE,
    Some(&k::CONVERT_TO_NULL_AWARE_MULTI),
    Automatically,
    assist: Some(&a::CONVERT_TO_NULL_AWARE),
    |c, builder| {
        let ast = c.ast;
        let mut target_node = c.node;
        if let Some(p) = ast.parent(target_node) {
            if ast.is::<BinaryExpression>(p) {
                if let Some(gp) = ast.parent(p) {
                    if ast.is::<ConditionalExpression>(gp) {
                        target_node = gp;
                    }
                }
            }
        }
        let Some(cond_expr) = ast.cast::<ConditionalExpression>(target_node) else {
            return;
        };
        let cond = unparenthesized(ast, ast[cond_expr].condition.raw());
        let Some(bin) = ast.cast::<BinaryExpression>(cond) else {
            return;
        };
        let left = ast[bin].left_operand.raw();
        let right = ast[bin].right_operand.raw();
        let cond_text = if ast.is::<NullLiteral>(left) && !ast.is::<NullLiteral>(right) {
            c.utils.get_node_text(right)
        } else if ast.is::<NullLiteral>(right) && !ast.is::<NullLiteral>(left) {
            c.utils.get_node_text(left)
        } else {
            return;
        };
        let op_ty = ast.tokens.ty(ast[bin].operator);
        let (null_expr, non_null_expr) = if op_ty == TokenType::EQ_EQ {
            (
                ast[cond_expr].then_expression.raw(),
                ast[cond_expr].else_expression.raw(),
            )
        } else if op_ty == TokenType::BANG_EQ {
            (
                ast[cond_expr].else_expression.raw(),
                ast[cond_expr].then_expression.raw(),
            )
        } else {
            return;
        };
        if !ast.is::<NullLiteral>(unparenthesized(ast, null_expr)) {
            return;
        }
        let mut result_expr = unparenthesized(ast, non_null_expr);
        let mut operator: Option<TokenId> = None;
        loop {
            if let Some(pi) = ast.cast::<PrefixedIdentifier>(result_expr) {
                operator = Some(ast[pi].period);
                result_expr = ast[pi].prefix.raw();
            } else if let Some(mi) = ast.cast::<MethodInvocation>(result_expr) {
                operator = ast[mi].operator;
                let Some(t) = ast[mi].target else {
                    return;
                };
                result_expr = t.raw();
            } else if let Some(pe) = ast
                .cast::<PostfixExpression>(result_expr)
                .filter(|pe| ast.tokens.ty(ast[*pe].operator) == TokenType::BANG)
            {
                result_expr = ast[pe].operand.raw();
            } else if let Some(pa) = ast.cast::<PropertyAccess>(result_expr) {
                operator = Some(ast[pa].operator);
                let Some(t) = ast[pa].target else {
                    return;
                };
                result_expr = t.raw();
            } else {
                return;
            }
            if c.utils.get_node_text(result_expr) == cond_text {
                break;
            }
        }
        let mut optional_question_mark = "";
        let operator_range = if let Some(op) = operator {
            if ast.tokens.ty(op) == TokenType::PERIOD {
                optional_question_mark = "?";
            }
            c.range()
                .start_offset_end_offset(ast.end(result_expr), c.token_offset(op))
        } else if let Some(parent) = ast
            .parent(result_expr)
            .filter(|p| ast.is::<PostfixExpression>(*p))
        {
            c.range().end_end(result_expr, parent)
        } else {
            return;
        };
        let del1 = c.range().start_start(cond_expr, non_null_expr);
        let del2 = c.range().end_end(non_null_expr, cond_expr);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(del1.offset, del1.length);
            b.add_simple_replacement(
                operator_range.offset,
                operator_range.length,
                optional_question_mark,
            );
            b.add_deletion(del2.offset, del2.length);
        });
    }
);

// ---------------------------------------------------------------------------
// ConvertMapFromIterableToForLiteral
// ---------------------------------------------------------------------------

struct MapClosure {
    parameter: Id<RegularFormalParameter>,
    parameter_identifier: TokenId,
    body: NodeId,
}

fn extract_map_closure_body(ast: &Ast, expr: Id<FunctionExpression>) -> Option<NodeId> {
    let body = ast[expr].body.raw();
    if let Some(efb) = ast.cast::<ExpressionFunctionBody>(body) {
        return Some(ast[efb].expression.raw());
    }
    if let Some(bfb) = ast.cast::<BlockFunctionBody>(body) {
        let stmts = ast.list(ast[ast[bfb].block].statements);
        if stmts.len() == 1 {
            if let Some(ret) = ast.cast::<ReturnStatement>(stmts[0]) {
                return ast[ret].expression.map(|e| e.raw());
            }
        }
    }
    None
}

fn extract_map_closure(c: &ProducerContext<'_>, name: &str, arg: NodeId) -> Option<MapClosure> {
    let ast = c.ast;
    let na = ast.cast::<NamedArgument>(arg)?;
    if c.lexeme(ast[na].name) != name {
        return None;
    }
    let expr = unparenthesized(ast, ast[na].argument_expression.raw());
    let fe = ast.cast::<FunctionExpression>(expr)?;
    let param_list = ast[fe].parameters?;
    let params = ast.list_raw(ast[param_list].parameters);
    if params.len() != 1 {
        return None;
    }
    let param = ast.cast::<RegularFormalParameter>(params[0])?;
    if ast[param].kind != ParameterKind::Required {
        return None;
    }
    let param_id = ast[param].name?;
    let body = extract_map_closure_body(ast, fe)?;
    Some(MapClosure {
        parameter: param,
        parameter_identifier: param_id,
        body,
    })
}

fn find_param_references(
    c: &ProducerContext<'_>,
    param_elem: ElementId,
    body: NodeId,
) -> (Vec<Id<SimpleIdentifier>>, HashSet<String>) {
    let ast = c.ast;
    let mut refs = Vec::new();
    let mut other_names = HashSet::new();
    super::variables::visit(ast, body, &mut |n| {
        let Some(sid) = ast.cast::<SimpleIdentifier>(n) else {
            return;
        };
        if c.locator().element(sid) == Some(param_elem) {
            refs.push(sid);
        } else {
            let is_qualified = ast.parent(sid).is_some_and(|p| {
                if let Some(pi) = ast.cast::<PrefixedIdentifier>(p) {
                    ast[pi].identifier == sid
                } else if let Some(pa) = ast.cast::<PropertyAccess>(p) {
                    ast[pa].property_name == sid
                } else if let Some(mi) = ast.cast::<MethodInvocation>(p) {
                    ast[mi].method_name == sid && ast[mi].target.is_some()
                } else {
                    false
                }
            });
            if !is_qualified {
                other_names.insert(c.lexeme(ast[sid].token).to_string());
            }
        }
    });
    (refs, other_names)
}

fn replace_param_refs(
    ast: &Ast,
    refs: &[Id<SimpleIdentifier>],
    param_len: u32,
    mut source: String,
    new_name: &str,
    offset: u32,
) -> String {
    for &r in refs.iter().rev() {
        let old_offset = (ast.offset(r) - offset) as usize;
        let old_end = old_offset + (param_len as usize);
        if old_end <= source.len() {
            source.replace_range(old_offset..old_end, new_name);
        }
    }
    source
}

producer!(
    ConvertMapFromIterableToForLiteral,
    k::CONVERT_TO_FOR_ELEMENT,
    Some(&k::CONVERT_TO_FOR_ELEMENT_MULTI),
    Automatically,
    assist: Some(&a::CONVERT_TO_FOR_ELEMENT),
    |c, builder| {
        let ast = c.ast;
        let Some(creation) = ast.this_or_ancestor_of_type::<InstanceCreationExpression>(c.node)
        else {
            return;
        };
        let Some(elem) = c.locator().element(ast[creation].constructor_name) else {
            return;
        };
        if c.ctx.element_name(elem) != Some("fromIterable")
            || c.ctx.element_data(elem).and_then(|d| d.enclosing)
                != Some(c.ctx.tp.map_element.get().raw())
        {
            return;
        }
        let args = ast.list_raw(ast[ast[creation].argument_list].arguments);
        if args.len() != 3 {
            return;
        }
        let arg0 = if let Some(na) = ast.cast::<NamedArgument>(args[0]) {
            ast[na].argument_expression.raw()
        } else {
            args[0]
        };
        let iterator = unparenthesized(ast, arg0);
        let second_arg = args[1];
        let third_arg = args[2];
        let Some(key_closure) = extract_map_closure(c, "key", second_arg)
            .or_else(|| extract_map_closure(c, "key", third_arg))
        else {
            return;
        };
        let Some(value_closure) = extract_map_closure(c, "value", third_arg)
            .or_else(|| extract_map_closure(c, "value", second_arg))
        else {
            return;
        };
        let key_param_name = c.lexeme(key_closure.parameter_identifier).to_string();
        let value_param_name = c.lexeme(value_closure.parameter_identifier).to_string();
        let mut key_expr_text = c.utils.get_node_text(key_closure.body);
        let mut value_expr_text = c.utils.get_node_text(value_closure.body);
        let loop_var_name = if key_param_name == value_param_name {
            key_param_name
        } else {
            let (Some(key_elem), Some(value_elem)) = (
                c.locator().declared_element(key_closure.parameter),
                c.locator().declared_element(value_closure.parameter),
            ) else {
                return;
            };
            let (key_refs, key_others) = find_param_references(c, key_elem, key_closure.body);
            let (value_refs, value_others) =
                find_param_references(c, value_elem, value_closure.body);
            let unused_var_name = || -> String {
                let mut candidate = "e".to_string();
                let mut index = 1usize;
                while key_others.contains(&candidate) || value_others.contains(&candidate) {
                    candidate = format!("e{index}");
                    index += 1;
                }
                candidate
            };
            if value_refs.is_empty() {
                if value_others.contains(&key_param_name) {
                    let name = unused_var_name();
                    key_expr_text = replace_param_refs(
                        ast,
                        &key_refs,
                        key_param_name.len() as u32,
                        key_expr_text,
                        &name,
                        ast.offset(key_closure.body),
                    );
                    name
                } else {
                    key_param_name
                }
            } else if key_refs.is_empty() {
                if key_others.contains(&value_param_name) {
                    let name = unused_var_name();
                    value_expr_text = replace_param_refs(
                        ast,
                        &value_refs,
                        value_param_name.len() as u32,
                        value_expr_text,
                        &name,
                        ast.offset(value_closure.body),
                    );
                    name
                } else {
                    value_param_name
                }
            } else {
                let name = unused_var_name();
                key_expr_text = replace_param_refs(
                    ast,
                    &key_refs,
                    key_param_name.len() as u32,
                    key_expr_text,
                    &name,
                    ast.offset(key_closure.body),
                );
                value_expr_text = replace_param_refs(
                    ast,
                    &value_refs,
                    value_param_name.len() as u32,
                    value_expr_text,
                    &name,
                    ast.offset(value_closure.body),
                );
                name
            }
        };
        let iter_text = c.utils.get_node_text(iterator);
        let r = c.range().node(creation);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_replacement(r.offset, r.length, |e| {
                e.write("{ for (var ");
                e.write(&loop_var_name);
                e.write(" in ");
                e.write(&iter_text);
                e.write(") ");
                e.write(&key_expr_text);
                e.write(" : ");
                e.write(&value_expr_text);
                e.write(" }");
            });
        });
    }
);

// ---------------------------------------------------------------------------
// ConvertPartOfToUri
// ---------------------------------------------------------------------------

assist!(
    ConvertPartOfToUri,
    a::CONVERT_PART_OF_TO_URI,
    |c, builder| {
        let ast = c.ast;
        let Some(directive) = ast.this_or_ancestor_of_type::<PartOfDirective>(c.node) else {
            return;
        };
        let Some(library_name) = ast[directive].library_name else {
            return;
        };
        let Some(first_unit) = c.resolved.library.library.units.first() else {
            return;
        };
        let library_path = &*first_unit.path;
        let part_path = c.path;
        let from = match part_path.rfind('/') {
            Some(i) => &part_path[..i],
            None => ".",
        };
        let to: Vec<&str> = library_path.split('/').filter(|s| !s.is_empty()).collect();
        let from_parts: Vec<&str> = from.split('/').filter(|s| !s.is_empty()).collect();
        let common = to
            .iter()
            .zip(&from_parts)
            .take_while(|(a, b)| a == b)
            .count();
        let mut parts: Vec<&str> = vec![".."; from_parts.len() - common];
        parts.extend(&to[common..]);
        let relative = if parts.is_empty() {
            ".".to_string()
        } else {
            parts.join("/")
        };
        let r = c.range().node(library_name);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(r.offset, r.length, &format!("'{relative}'"));
        });
    }
);

// ---------------------------------------------------------------------------
// ConvertClassToEnum
// ---------------------------------------------------------------------------

struct EnumConstantField {
    declaration: Id<VariableDeclaration>,
    field_decl: Id<FieldDeclaration>,
    instance_creation: Id<InstanceCreationExpression>,
    constructor_element: ElementId,
    index_value: i64,
}

enum EnumIndexField {
    InVariableDeclaration {
        element: ElementId,
        declaration: Id<VariableDeclaration>,
        field_decl: Id<FieldDeclaration>,
    },
    InPrimaryConstructor {
        element: ElementId,
        parameter: NodeId,
    },
}

impl EnumIndexField {
    fn element(&self) -> ElementId {
        match self {
            EnumIndexField::InVariableDeclaration { element, .. }
            | EnumIndexField::InPrimaryConstructor { element, .. } => *element,
        }
    }
}

fn nodes_in_list_ranges(
    c: &ProducerContext<'_>,
    list: &[NodeId],
    indexes: &[usize],
) -> Vec<super::super::utils::Range> {
    if indexes.is_empty() {
        return Vec::new();
    }
    let mut index_ranges: Vec<(usize, usize)> = Vec::new();
    let mut lower = indexes[0];
    let mut upper = indexes[0];
    for &idx in &indexes[1..] {
        if idx == upper + 1 {
            upper = idx;
        } else {
            index_ranges.push((lower, upper));
            lower = idx;
            upper = idx;
        }
    }
    index_ranges.push((lower, upper));
    if index_ranges.len() == 1 && index_ranges[0] == (0, list.len() - 1) {
        return vec![c.range().start_end(list[0], list[list.len() - 1])];
    }
    let mut ranges = Vec::new();
    for (lo, hi) in index_ranges {
        if lo == 0 {
            ranges.add_or_push(c.range().start_start(list[lo], list[hi + 1]));
        } else {
            ranges.add_or_push(c.range().end_end(list[lo - 1], list[hi]));
        }
    }
    ranges
}

trait VecPushExt<T> {
    fn add_or_push(&mut self, item: T);
}
impl<T> VecPushExt<T> for Vec<T> {
    fn add_or_push(&mut self, item: T) {
        self.push(item);
    }
}

fn has_first_fragment_flag(
    ctx: &dartr_element::Ctx<'_>,
    elem: ElementId,
    flag: dartr_element::FragmentFlags,
) -> bool {
    ctx.element_data(elem)
        .and_then(|d| ctx.fragment_data(d.first_fragment))
        .is_some_and(|fd| fd.flags.has(flag))
}

producer!(
    ConvertClassToEnum,
    k::CONVERT_CLASS_TO_ENUM,
    Some(&k::CONVERT_CLASS_TO_ENUM_MULTI),
    Automatically,
    assist: Some(&a::CONVERT_CLASS_TO_ENUM),
    |c, builder| {
        let ast = c.ast;
        let ctx = c.ctx;
        let Some(class_decl) = ast.parent(c.node).and_then(|p| ast.cast::<ClassDeclaration>(p)).or_else(|| ast.cast::<ClassDeclaration>(c.node)) else {
            return;
        };
        let type_name_tok = dartr_resolver::error::support::class_name_token(ast, ast[class_decl].name_part);
        if type_name_tok != c.token {
            return;
        }
        if !ctx.features.is_enabled("enhanced-enums") || c.resolved.library.library.units.len() > 1 {
            return;
        }
        let Some(class_elem) = c
            .locator()
            .declared_element(class_decl)
            .and_then(|e| e.cast::<ClassElement>())
        else {
            return;
        };
        if ctx
            .get(class_elem)
            .flags
            .has(ElementFlags::CLASS_ELEMENT_IS_ABSTRACT)
        {
            return;
        }
        if let Some(ec) = ast[class_decl].extends_clause {
            if let Some(st) = c.tables.annotation_type.get(ast[ec].superclass.raw()).copied() {
                if !ctx.is_dart_core_object(st) {
                    return;
                }
            }
        }
        let Some(class_body) = ast.cast::<BlockClassBody>(ast[class_decl].body) else {
            return;
        };
        let members = ast.list_raw(ast[class_body].members);
        // Validate constructors.
        let class_is_public = !c.lexeme(type_name_tok).starts_with('_');
        let iface_elem: dartr_element::EId<InterfaceElement> = class_elem.upcast();
        for &ctor_el in &ctx.interface(iface_elem).constructors {
            let ctor_name = ctx.element_name(ctor_el.raw()).unwrap_or("");
            let ctor_is_public = !ctor_name.starts_with('_');
            if ctor_is_public && class_is_public {
                return;
            }
            let is_factory = has_first_fragment_flag(
                ctx,
                ctor_el.raw(),
                dartr_element::FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY,
            );
            let is_const = has_first_fragment_flag(
                ctx,
                ctor_el.raw(),
                dartr_element::FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST,
            );
            if !is_factory && !is_const {
                return;
            }
        }
        let mut constructors_map: IndexMap<ElementId, (NodeId, Id<FormalParameterList>)> =
            IndexMap::new();
        for &m in members {
            if let Some(cd) = ast.cast::<ConstructorDeclaration>(m) {
                let Some(ce) = c.locator().declared_element(cd) else {
                    return;
                };
                constructors_map.insert(ce, (cd.raw(), ast[cd].parameters));
            } else if let Some(md) = ast.cast::<MethodDeclaration>(m) {
                let mname = c.lexeme(ast[md].name);
                if mname == "==" || mname == "hashCode" {
                    return;
                }
            }
        }
        if let Some(pcd) = ast.cast::<PrimaryConstructorDeclaration>(ast[class_decl].name_part) {
            let Some(ce) = c.locator().declared_element(pcd) else {
                return;
            };
            constructors_map.insert(ce, (pcd.raw(), ast[pcd].formal_parameters));
        }

        // Validate fields.
        let mut fields_to_convert: Vec<EnumConstantField> = Vec::new();
        let mut index_field: Option<EnumIndexField> = None;
        for &m in members {
            let Some(fd) = ast.cast::<FieldDeclaration>(m) else {
                continue;
            };
            let vl = ast[fd].fields;
            let vars = ast.list(ast[vl].variables);
            let is_const = ast[vl].keyword.is_some_and(|k| c.lexeme(k) == "const");
            let is_final = ast[vl].keyword.is_some_and(|k| c.lexeme(k) == "final");
            if ast[fd].static_keyword.is_some() {
                for &vd in vars {
                    let Some(fe) = c.locator().declared_element(vd) else {
                        continue;
                    };
                    let fty = dartr_resolver::element_ext::variable_type(ctx, fe);
                    if is_const
                        && matches!(*ctx.ty(fty), TypeKind::Interface { element, .. } if element == iface_elem)
                    {
                        let Some(init) = ast[vd].initializer else {
                            continue;
                        };
                        let Some(ice) = ast.cast::<InstanceCreationExpression>(init) else {
                            continue;
                        };
                        let Some(ctor_el) = c.locator().element(ast[ice].constructor_name) else {
                            continue;
                        };
                        if ctor_el.is::<dartr_element::ConstructorElement>() {
                            let is_factory = has_first_fragment_flag(
                                ctx,
                                ctor_el,
                                dartr_element::FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY,
                            );
                            if !is_factory
                                && ctx.element_data(ctor_el).and_then(|d| d.enclosing)
                                    == Some(class_elem.raw())
                            {
                                if vars.len() != 1 {
                                    return;
                                }
                                fields_to_convert.push(EnumConstantField {
                                    declaration: vd,
                                    field_decl: fd,
                                    instance_creation: ice,
                                    constructor_element: ctor_el,
                                    index_value: -1,
                                });
                            }
                        }
                    }
                }
            } else {
                for &vd in vars {
                    if !is_final {
                        return;
                    }
                    let Some(fe) = c.locator().declared_element(vd) else {
                        continue;
                    };
                    let fty = dartr_resolver::element_ext::variable_type(ctx, fe);
                    if c.lexeme(ast[vd].name) == "index" && ctx.is_dart_core_int(fty) {
                        index_field = Some(EnumIndexField::InVariableDeclaration {
                            element: fe,
                            declaration: vd,
                            field_decl: fd,
                        });
                    }
                }
            }
        }
        if let Some(pcd) = ast.cast::<PrimaryConstructorDeclaration>(ast[class_decl].name_part) {
            for &param in ast.list_raw(ast[ast[pcd].formal_parameters].parameters) {
                let Some(pe) = c
                    .locator()
                    .declared_element(param)
                    .and_then(|e| e.cast::<FormalParameterElement>())
                else {
                    continue;
                };
                if !has_first_fragment_flag(
                    ctx,
                    pe.raw(),
                    dartr_element::FragmentFlags::FIELD_FORMAL_PARAMETER_FRAGMENT_IS_DECLARING,
                ) {
                    continue;
                }
                if let Some(fe) = ctx.get(pe).field.get() {
                    if !has_first_fragment_flag(
                        ctx,
                        fe.raw(),
                        dartr_element::FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL,
                    ) {
                        return;
                    }
                    let fty = dartr_resolver::element_ext::variable_type(ctx, fe.raw());
                    if ctx.element_name(fe.raw()) == Some("index") && ctx.is_dart_core_int(fty) {
                        index_field = Some(EnumIndexField::InPrimaryConstructor {
                            element: fe.raw(),
                            parameter: param,
                        });
                    }
                }
            }
        }
        if fields_to_convert.is_empty() {
            return;
        }

        // Check _EnumVisitor and _NonEnumVisitor.
        let invokes_generative = |ice: Id<InstanceCreationExpression>| -> bool {
            let Some(ce) = c
                .locator()
                .element(ast[ice].constructor_name)
                .and_then(|e| e.cast::<dartr_element::ConstructorElement>())
            else {
                return false;
            };
            let is_factory = has_first_fragment_flag(
                ctx,
                ce.raw(),
                dartr_element::FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY,
            );
            !is_factory
                && ctx.element_data(ce.raw()).and_then(|d| d.enclosing) == Some(class_elem.raw())
        };
        let allowed_creations: HashSet<NodeId> = fields_to_convert
            .iter()
            .map(|f| f.instance_creation.raw())
            .collect();
        let mut cannot_convert = false;
        super::variables::visit(ast, c.unit.raw(), &mut |n| {
            if cannot_convert {
                return;
            }
            if let Some(ice) = ast.cast::<InstanceCreationExpression>(n) {
                if !allowed_creations.contains(&n) && invokes_generative(ice) {
                    cannot_convert = true;
                }
            } else if let Some(cd) = ast.cast::<ClassDeclaration>(n) {
                if cd != class_decl {
                    let Some(other_el) = c
                        .locator()
                        .declared_element(cd)
                        .and_then(|e| e.cast::<InterfaceElement>())
                    else {
                        cannot_convert = true;
                        return;
                    };
                    let iface = ctx.interface(other_el);
                    let uses_class = iface
                        .supertype
                        .get()
                        .is_some_and(|t| matches!(*ctx.ty(t), TypeKind::Interface { element, .. } if element == iface_elem))
                        || ctx
                            .element_interfaces(other_el)
                            .iter()
                            .chain(ctx.element_mixins(other_el).iter())
                            .any(|&t| matches!(*ctx.ty(t), TypeKind::Interface { element, .. } if element == iface_elem));
                    if uses_class {
                        cannot_convert = true;
                    }
                }
            }
        });
        if cannot_convert {
            return;
        }

        // Compute usedConstructors and indexFieldData.
        let mut used_constructors: IndexMap<ElementId, (NodeId, Id<FormalParameterList>)> =
            IndexMap::new();
        for f in &fields_to_convert {
            if let Some(&data) = constructors_map.get(&f.constructor_element) {
                used_constructors.insert(f.constructor_element, data);
            }
        }
        let mut ctor_index_param: Option<IndexMap<ElementId, (usize, ElementId)>> = None;
        if let Some(ref idx_f) = index_field {
            let idx_elem = idx_f.element();
            let mut map = IndexMap::new();
            for (&ctor_el, &(_, param_list)) in &used_constructors {
                let params = ast.list_raw(ast[param_list].parameters);
                let mut found = None;
                for (i, &p) in params.iter().enumerate() {
                    if let Some(pe) = c
                        .locator()
                        .declared_element(p)
                        .and_then(|e| e.cast::<FormalParameterElement>())
                    {
                        if ctx.get(pe).field.get().map(|f| f.raw()) == Some(idx_elem) {
                            found = Some((i, pe.raw()));
                            break;
                        }
                    }
                }
                let Some(f_param) = found else {
                    return;
                };
                map.insert(ctor_el, f_param);
            }
            let mut values = HashSet::new();
            for f in &mut fields_to_convert {
                let Some(&(_, param_el)) = map.get(&f.constructor_element) else {
                    return;
                };
                let args = ast.list_raw(ast[ast[f.instance_creation].argument_list].arguments);
                let mut arg_expr = None;
                for &arg in args {
                    let corr = c
                        .tables
                        .param_element
                        .get(arg)
                        .map(|r| dartr_typesystem::member::base_element(ctx, *r));
                    if corr == Some(param_el) {
                        arg_expr = Some(if let Some(na) = ast.cast::<NamedArgument>(arg) {
                            ast[na].argument_expression.raw()
                        } else {
                            arg
                        });
                        break;
                    }
                }
                let Some(ae) = arg_expr.and_then(|a| ast.cast::<IntegerLiteral>(a)) else {
                    return;
                };
                let Some(val) = ast[ae].value else {
                    return;
                };
                if !values.insert(val) {
                    return;
                }
                f.index_value = val;
            }
            let mut sorted_vals: Vec<i64> = values.into_iter().collect();
            sorted_vals.sort();
            if sorted_vals.len() != fields_to_convert.len()
                || sorted_vals.first().copied() != Some(0)
                || sorted_vals.last().copied() != Some(fields_to_convert.len() as i64 - 1)
            {
                return;
            }
            ctor_index_param = Some(map);
        } else {
            // Check no two fields evaluate to identical constant values when there is no index field.
            let mut seen_creations = HashSet::new();
            for f in &fields_to_convert {
                let args_text = c
                    .utils
                    .get_node_text(ast[f.instance_creation].argument_list);
                if !seen_creations.insert((f.constructor_element, args_text)) {
                    return;
                }
            }
        }

        // Apply changes.
        let mut members_to_delete: Vec<usize> = Vec::new();
        let mut parameters_to_delete: Vec<usize> = Vec::new();
        let delete_field = |f: &EnumIndexField,
                            members_to_delete: &mut Vec<usize>,
                            parameters_to_delete: &mut Vec<usize>,
                            b: &mut super::super::change_builder::FileEditBuilder<'_, '_>| {
            match f {
                EnumIndexField::InVariableDeclaration {
                    declaration,
                    field_decl,
                    ..
                } => {
                    let vl = ast[*field_decl].fields;
                    let vars = ast.list_raw(ast[vl].variables);
                    if vars.len() == 1 {
                        if let Some(pos) = members.iter().position(|&m| m == field_decl.raw()) {
                            members_to_delete.push(pos);
                        }
                    } else {
                        let r = c.range().node_in_list(vars, declaration.raw());
                        b.add_deletion(r.offset, r.length);
                    }
                }
                EnumIndexField::InPrimaryConstructor { parameter, .. } => {
                    if let Some(pcd) =
                        ast.cast::<PrimaryConstructorDeclaration>(ast[class_decl].name_part)
                    {
                        let params = ast.list_raw(ast[ast[pcd].formal_parameters].parameters);
                        if let Some(pos) = params.iter().position(|&p| p == *parameter) {
                            parameters_to_delete.push(pos);
                        }
                    }
                }
            }
        };

        fields_to_convert.sort_by_key(|f| f.index_value);
        let indent = c.utils.one_indent();
        let eol = c.eol();
        let mut constants_buf = String::new();

        builder.add_dart_file_edit(c.path, |b| {
            let kw_r = c.range().token_start_end(
                super::super::utils::first_token_after_comment_and_metadata(ast, class_decl.raw()),
                ast[class_decl].class_keyword,
            );
            b.add_simple_replacement(kw_r.offset, kw_r.length, "enum");

            if let Some(pcd) = ast.cast::<PrimaryConstructorDeclaration>(ast[class_decl].name_part)
            {
                if let Some(ck) = ast[pcd].const_keyword {
                    let r = c.range().token_start_start(ck, ast.tokens.next(ck));
                    b.add_deletion(r.offset, r.length);
                }
            }
            if let Some(ec) = ast[class_decl].extends_clause {
                let next_tok = ast.tokens.next(ast.end_token(ec));
                let r = c.range().node_start_token_start(ec, next_tok);
                b.add_deletion(r.offset, r.length);
            }

            for field in &fields_to_convert {
                let doc = ast[field.field_decl].documentation_comment;
                if !constants_buf.is_empty() {
                    constants_buf.push_str(&format!(",{eol}"));
                    if doc.is_some() {
                        constants_buf.push_str(eol);
                    }
                    constants_buf.push_str(indent);
                }
                if let Some(dc) = doc {
                    constants_buf.push_str(&c.utils.get_node_text(dc));
                    constants_buf.push_str(&format!("{eol}{indent}"));
                }
                constants_buf.push_str(c.lexeme(ast[field.declaration].name));
                let inv = field.instance_creation;
                let cn = ast[inv].constructor_name;
                let param_data = ctor_index_param
                    .as_ref()
                    .and_then(|m| m.get(&field.constructor_element))
                    .copied();
                let type_args = ast[ast[cn].type_].type_arguments;
                if let Some(ta) = type_args {
                    constants_buf.push_str(&c.utils.get_node_text(ta));
                }
                let cname = ast[cn].name.map(|n| c.lexeme(ast[n].token));
                if let Some(cn_str) = cname {
                    constants_buf.push_str(&format!(".{cn_str}"));
                }
                let arg_list = ast[inv].argument_list;
                let args = ast.list_raw(ast[arg_list].arguments);
                let arg_count = args.len() - usize::from(param_data.is_some());
                if arg_count == 0 {
                    if type_args.is_some() || cname.is_some() {
                        constants_buf.push_str("()");
                    }
                } else if let Some((index, _)) = param_data {
                    constants_buf.push('(');
                    let last = args.len() - 1;
                    if index == 0 {
                        let off = ast.offset(args[1]);
                        let len = ast.end(args[last]) - off;
                        constants_buf.push_str(&c.utils.get_text(off, len));
                    } else if index == last {
                        let off = ast.offset(args[0]);
                        let len = if ast.tokens.ty(ast.tokens.next(ast.end_token(args[last])))
                            == TokenType::COMMA
                        {
                            ast.offset(args[last]) - off
                        } else {
                            ast.end(args[last - 1]) - off
                        };
                        constants_buf.push_str(&c.utils.get_text(off, len));
                    } else {
                        let off1 = ast.offset(args[0]);
                        let len1 = ast.offset(args[index]) - off1;
                        constants_buf.push_str(&c.utils.get_text(off1, len1));
                        let off2 = ast.offset(args[index + 1]);
                        let len2 = c.token_offset(ast.end_token(arg_list)) - off2;
                        constants_buf.push_str(&c.utils.get_text(off2, len2));
                    }
                    constants_buf.push(')');
                } else {
                    constants_buf.push_str(&c.utils.get_node_text(arg_list));
                }
                delete_field(
                    &EnumIndexField::InVariableDeclaration {
                        element: field.constructor_element,
                        declaration: field.declaration,
                        field_decl: field.field_decl,
                    },
                    &mut members_to_delete,
                    &mut parameters_to_delete,
                    b,
                );
            }

            if let Some(ref idx_f) = index_field {
                delete_field(idx_f, &mut members_to_delete, &mut parameters_to_delete, b);
            }

            // _transformConstructors & _removeUnnamedConstructor
            let body_ctors: Vec<Id<ConstructorDeclaration>> = members
                .iter()
                .copied()
                .filter_map(|m| ast.cast::<ConstructorDeclaration>(m))
                .collect();
            let primary_ctor = ast.cast::<PrimaryConstructorDeclaration>(ast[class_decl].name_part);
            let mut primary_ctor_to_delete: Option<Id<PrimaryConstructorDeclaration>> = None;
            let removed_ctor: Option<NodeId> = if let Some(pcd) = primary_ctor {
                let cname = ast[pcd].constructor_name.map(|cn| c.lexeme(ast[cn].name));
                let primary_body = members
                    .iter()
                    .find_map(|&m| ast.cast::<PrimaryConstructorBody>(m));
                let body_ok = primary_body.is_none_or(|body| {
                    ast[body].documentation_comment.is_none()
                        && ast.list_raw(ast[body].metadata).is_empty()
                        && ast.list_raw(ast[body].initializers).is_empty()
                });
                let params = ast.list_raw(ast[ast[pcd].formal_parameters].parameters);
                let has_idx = ctor_index_param
                    .as_ref()
                    .and_then(|m| m.values().next())
                    .is_some();
                let updated_count = params.len().saturating_sub(usize::from(has_idx));
                if body_ctors.is_empty()
                    && cname.is_none_or(|n| n == "new")
                    && body_ok
                    && updated_count == 0
                {
                    primary_ctor_to_delete = Some(pcd);
                    Some(pcd.raw())
                } else {
                    None
                }
            } else if body_ctors.len() == 1 {
                let ctor = body_ctors[0];
                let cname = ast[ctor].name.map(|n| c.lexeme(n));
                let params = ast.list_raw(ast[ast[ctor].parameters].parameters);
                let has_idx = ctor_index_param
                    .as_ref()
                    .and_then(|m| m.values().next())
                    .is_some();
                let updated_count = params.len().saturating_sub(usize::from(has_idx));
                if cname.is_none_or(|n| n == "new")
                    && ast[ctor].documentation_comment.is_none()
                    && ast[ctor].metadata.is_empty()
                    && ast[ctor].initializers.is_empty()
                    && updated_count == 0
                {
                    if let Some(pos) = members.iter().position(|&m| m == ctor.raw()) {
                        members_to_delete.push(pos);
                    }
                    Some(ctor.raw())
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(ref map) = ctor_index_param {
                for (&ctor_el, &(idx, _)) in map {
                    if let Some(&(decl_node, param_list)) = used_constructors.get(&ctor_el) {
                        if Some(decl_node) == removed_ctor {
                            continue;
                        }
                        let params = ast.list_raw(ast[param_list].parameters);
                        let r = c.range().node_in_list(params, params[idx]);
                        b.add_deletion(r.offset, r.length);
                    }
                }
            }

            if let Some(pcd) = primary_ctor_to_delete {
                let r = if let Some(cn) = ast[pcd].constructor_name {
                    c.range().start_end(cn, pcd)
                } else {
                    c.range().node(ast[pcd].formal_parameters)
                };
                b.add_deletion(r.offset, r.length);
            }

            if members_to_delete.len() == members.len() && !members.is_empty() {
                let r = c.range().start_end(members[0], *members.last().unwrap());
                b.add_simple_replacement(r.offset, r.length, &constants_buf);
                return;
            }

            let body_offset = c.token_end(ast[class_body].left_bracket);
            b.add_simple_insertion(
                body_offset,
                &format!("{eol}{indent}{constants_buf};{eol}"),
            );

            members_to_delete.sort();
            members_to_delete.dedup();
            for r in nodes_in_list_ranges(c, members, &members_to_delete) {
                b.add_deletion(r.offset, r.length);
            }

            if let Some(pcd) = primary_ctor {
                let params = ast.list_raw(ast[ast[pcd].formal_parameters].parameters);
                parameters_to_delete.sort();
                parameters_to_delete.dedup();
                for r in nodes_in_list_ranges(c, params, &parameters_to_delete) {
                    b.add_deletion(r.offset, r.length);
                }
            }
        });
    }
);
