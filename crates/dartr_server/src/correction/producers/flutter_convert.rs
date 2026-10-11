// Dart source: pkg/analysis_server/lib/src/services/correction/dart/flutter_convert_to_stateful_widget.dart

//! Dart `FlutterConvertToStatefulWidget`.

use std::collections::HashSet;

use dartr_ast::*;
use dartr_element::{ElemRef, Tag, TypeKind};
use dartr_syntax::TokenType;
use dartr_typesystem::type_ext::TypeExt;

use super::super::change::SourceEdit;
use super::super::flutter::*;
use super::super::generated::assist_kinds as a;
use super::super::producer::*;
use super::simple::assist;

fn is_static_member(c: &ProducerContext<'_>, member: NodeId) -> bool {
    let ast = c.ast;
    if let Some(f) = ast.cast::<FieldDeclaration>(member) {
        return ast[f].static_keyword.is_some();
    }
    if let Some(m) = ast.cast::<MethodDeclaration>(member) {
        return ast[m]
            .modifier_keyword
            .is_some_and(|k| c.lexeme(k) == "static");
    }
    false
}

/// Dart `_FieldFinder`: the names of the fields assigned in [constructor].
fn fields_assigned_in_constructor(
    c: &ProducerContext<'_>,
    constructor: NodeId,
    class: dartr_element::ElementId,
    out: &mut HashSet<String>,
) {
    let ast = c.ast;
    super::variables::visit(ast, constructor, &mut |n| {
        if let Some(p) = ast.cast::<FieldFormalParameter>(n) {
            out.insert(c.lexeme(ast[p].name).to_string());
        } else if let Some(id) = ast.cast::<SimpleIdentifier>(n) {
            let parent = ast.parent(n);
            if parent.is_some_and(|p| ast.is::<ConstructorFieldInitializer>(p)) {
                out.insert(c.lexeme(ast[id].token).to_string());
                return;
            }
            // Dart `inSetterContext`: an assigned setter of a field.
            let climbed = super::members::climb_property_access(ast, n);
            let assigned = ast
                .parent(climbed)
                .and_then(|p| ast.cast::<AssignmentExpression>(p))
                .is_some_and(|a| ast[a].left_hand_side.raw() == climbed);
            if assigned {
                if let Some(setter) =
                    dartr_resolver::error::support::write_element(c.ctx, c.tables, climbed)
                        .or_else(|| c.locator().write_or_read_element(id))
                {
                    if setter.tag() == Tag::Setter
                        && c.ctx.element_data(setter).and_then(|d| d.enclosing) == Some(class)
                    {
                        if let Some(name) = c.ctx.element_name(setter) {
                            out.insert(name.trim_end_matches('=').to_string());
                        }
                    }
                }
            }
        }
    });
}

assist!(
    FlutterConvertToStatefulWidget,
    a::FLUTTER_CONVERT_TO_STATEFUL_WIDGET,
    |c, builder| {
        let ast = c.ast;
        let ctx = c.ctx;
        let Some(class) = ast.this_or_ancestor_of_type::<ClassDeclaration>(c.node) else {
            return;
        };
        let Some(extends) = ast[class].extends_clause else {
            return;
        };
        let superclass = ast[extends].superclass;
        let Some(body) = ast.cast::<BlockClassBody>(ast[class].body) else {
            return;
        };
        if c.selection_offset < c.token_offset(ast[class].class_keyword)
            || c.selection_offset > c.token_end(ast[body].left_bracket)
        {
            return;
        }
        let Some(element) = c
            .locator()
            .declared_element(class)
            .and_then(|e| e.cast::<dartr_element::InterfaceElement>())
        else {
            return;
        };
        let Some(supertype) = ctx.element_supertype(element) else {
            return;
        };
        if !is_exact_widget_type(ctx, Some(supertype), "StatelessWidget", URI_FRAMEWORK) {
            return;
        }
        let members: Vec<NodeId> = ast.list_raw(ast[body].members).to_vec();
        let build_method = members.iter().copied().find(|&m| {
            ast.cast::<MethodDeclaration>(m).is_some_and(|m| {
                c.lexeme(ast[m].name) == "build"
                    && ast[m]
                        .parameters
                        .is_some_and(|p| ast.list_raw(ast[p].parameters).len() == 1)
            })
        });
        let Some(build_method) = build_method else {
            return;
        };
        let widget_name = dartr_resolver::error::support::display_name(ctx, element.raw());
        let state_name = if widget_name.starts_with('_') {
            format!("{widget_name}State")
        } else {
            format!("_{widget_name}State")
        };
        let mut assigned: HashSet<String> = HashSet::new();
        for &member in &members {
            if ast.is::<ConstructorDeclaration>(member) {
                fields_assigned_in_constructor(c, member, element.raw(), &mut assigned);
            }
        }
        // The members to move and the names of their elements.
        let mut nodes_to_move: Vec<NodeId> = Vec::new();
        let mut names_to_move: HashSet<String> = HashSet::new();
        for &member in &members {
            if let Some(f) = ast.cast::<FieldDeclaration>(member) {
                if is_static_member(c, member) {
                    continue;
                }
                for &v in ast.list(ast[ast[f].fields].variables) {
                    let name = c.lexeme(ast[v].name).to_string();
                    if !assigned.contains(&name) {
                        if !nodes_to_move.contains(&member) {
                            nodes_to_move.push(member);
                        }
                        names_to_move.insert(name);
                    }
                }
            } else if let Some(m) = ast.cast::<MethodDeclaration>(member) {
                if is_static_member(c, member) {
                    continue;
                }
                nodes_to_move.push(member);
                names_to_move.insert(c.lexeme(ast[m].name).to_string());
            }
        }
        // Dart `rewriteWidgetMemberReferences`.
        let rewrite = |moved: NodeId| -> String {
            let lines = c.utils.get_lines_range(c.range().node(moved), false);
            let text = c.utils.get_range_text(lines);
            let mut edits: Vec<SourceEdit> = Vec::new();
            super::variables::visit(ast, moved, &mut |n| {
                let Some(id) = ast.cast::<SimpleIdentifier>(n) else {
                    return;
                };
                let Some(e) = c.locator().element(id) else {
                    return;
                };
                if !matches!(e.tag(), Tag::Getter | Tag::Setter | Tag::Method) {
                    return;
                }
                if ctx.element_data(e).and_then(|d| d.enclosing) != Some(element.raw()) {
                    return;
                }
                let name = ctx.element_name(e).unwrap_or("").trim_end_matches('=');
                if names_to_move.contains(name) {
                    return;
                }
                let offset = ast.offset(id) - lines.offset;
                let qualifier = if dartr_typesystem::member::is_static(ctx, ElemRef::Base(e)) {
                    widget_name.clone()
                } else {
                    "widget".to_string()
                };
                let interpolation_identifier = ast
                    .parent(id)
                    .and_then(|p| ast.cast::<InterpolationExpression>(p))
                    .is_some_and(|p| {
                        ast.tokens.ty(ast[p].left_bracket)
                            == TokenType::STRING_INTERPOLATION_IDENTIFIER
                    });
                if interpolation_identifier {
                    edits.push(SourceEdit {
                        description: None,
                        offset,
                        length: 0,
                        replacement: format!("{{{qualifier}."),
                        id: 0,
                    });
                    edits.push(SourceEdit {
                        description: None,
                        offset: offset + ast.length(id),
                        length: 0,
                        replacement: "}".to_string(),
                        id: 0,
                    });
                } else {
                    edits.push(SourceEdit {
                        description: None,
                        offset,
                        length: 0,
                        replacement: format!("{qualifier}."),
                        id: 0,
                    });
                }
            });
            edits.reverse();
            super::super::bulk_fix_processor::apply_edits(&text, &edits)
        };
        let (Some(stateful), Some(state)) = (
            get_class(c, WIDGETS_URI, "StatefulWidget"),
            get_class(c, WIDGETS_URI, "State"),
        ) else {
            return;
        };
        let name_part = ast[class].name_part;
        let type_name = c
            .lexeme(dartr_resolver::error::support::class_name_token(
                ast, name_part,
            ))
            .to_string();
        let type_parameters = ast
            .cast::<NameWithTypeParameters>(name_part)
            .and_then(|n| ast[n].type_parameters);
        let type_params = type_parameters
            .map(|t| c.utils.get_node_text(t))
            .unwrap_or_default();
        let superclass_range = c.range().node(superclass);
        // The replacements of the intervals of moved members:
        // (offset, length, has build method, empty line before, empty line
        // after, replace with empty line).
        let mut intervals: Vec<(u32, u32, bool, bool, bool, bool)> = Vec::new();
        let mut replace_offset: u32 = 0;
        let mut has_build_method = false;
        let mut last_to_remove_is_field = false;
        let mut end_of_last_node_to_keep: u32 = 0;
        for &member in &members {
            if nodes_to_move.contains(&member) {
                if replace_offset == 0 {
                    let begin = ast.begin_token(member);
                    let comments = ast.tokens.get(begin).preceding_comments;
                    let start = if comments.is_some() {
                        c.token_offset(comments)
                    } else {
                        ast.offset(member)
                    };
                    replace_offset = c.utils.get_line_content_start(start);
                }
                if member == build_method {
                    has_build_method = true;
                }
                last_to_remove_is_field = ast.is::<FieldDeclaration>(member);
            } else {
                let lines = c.utils.get_lines_range(c.range().node(member), false);
                end_of_last_node_to_keep = lines.end();
                if replace_offset != 0 {
                    intervals.push((
                        replace_offset,
                        lines.offset - replace_offset,
                        has_build_method,
                        false,
                        true,
                        last_to_remove_is_field && !ast.is::<FieldDeclaration>(member),
                    ));
                    has_build_method = false;
                    replace_offset = 0;
                }
            }
        }
        if replace_offset != 0 {
            if end_of_last_node_to_keep != 0 {
                replace_offset = end_of_last_node_to_keep;
            }
            let end = c.token_offset(ast[body].right_bracket);
            intervals.push((
                replace_offset,
                end - replace_offset,
                has_build_method,
                end_of_last_node_to_keep != 0,
                false,
                false,
            ));
        }
        let moved_texts: Vec<(Option<String>, String)> = nodes_to_move
            .iter()
            .map(|&member| {
                let begin = ast.begin_token(member);
                let comments = ast.tokens.get(begin).preceding_comments;
                let comment = if comments.is_some() {
                    let offset = c.utils.get_line_content_start(c.token_offset(comments));
                    let length = ast.tokens.get(comments).end() - offset;
                    Some(c.utils.get_text(offset, length))
                } else {
                    None
                };
                (comment, rewrite(member))
            })
            .collect();
        let class_end = ast.end(class);
        let type_parameter_names: Vec<String> = type_parameters
            .map(|t| {
                ast.list(ast[t].type_parameters)
                    .iter()
                    .map(|p| c.lexeme(ast[*p].name).to_string())
                    .collect()
            })
            .unwrap_or_default();
        builder.add_dart_file_edit(c.path, |b| {
            b.add_replacement(superclass_range.offset, superclass_range.length, |e| {
                e.write_reference(stateful);
            });
            for &(offset, length, has_build, before, after, empty_line) in &intervals {
                b.add_replacement(offset, length, |e| {
                    if has_build {
                        if before {
                            e.newline();
                        }
                        e.writeln("  @override");
                        e.write("  ");
                        e.write_reference(state);
                        e.write(&format!("<{type_name}{type_params}>"));
                        e.writeln(&format!(" createState() => {state_name}{type_params}();"));
                        if after {
                            e.newline();
                        }
                    } else if empty_line {
                        e.newline();
                    }
                });
            }
            b.add_insertion(class_end, |e| {
                e.newline();
                e.newline();
                e.write(&format!("class {state_name}{type_params} extends "));
                e.write_reference(state);
                e.write(&format!("<{type_name}"));
                if type_parameters.is_some() {
                    e.write("<");
                    // Dart writes `, ` only before the second name (`first` is
                    // never set to `false` after it).
                    let mut first = true;
                    for name in &type_parameter_names {
                        if !first {
                            e.write(", ");
                            first = false;
                        }
                        e.write(name);
                    }
                    e.write(">");
                }
                e.writeln("> {");
                let mut write_empty_line = false;
                for (comment, text) in &moved_texts {
                    if write_empty_line {
                        e.newline();
                    }
                    if let Some(comment) = comment {
                        e.writeln(comment);
                    }
                    e.write(text);
                    write_empty_line = true;
                }
                e.write("}");
            });
        });
    }
);

fn is_state(
    ctx: &dartr_element::Ctx<'_>,
    widget_element: dartr_element::ElementId,
    ty: Option<dartr_element::TypeId>,
) -> bool {
    let Some(ty) = ty else {
        return false;
    };
    let TypeKind::Interface { args, .. } = *ctx.ty(ty) else {
        return false;
    };
    let type_args = ctx.list(args);
    if type_args.len() != 1 {
        return false;
    }
    let TypeKind::Interface {
        element: first_elem,
        ..
    } = *ctx.ty(type_args[0])
    else {
        return false;
    };
    if first_elem.raw() != widget_element {
        return false;
    }
    is_exact_widget_type(ctx, Some(ty), "State", URI_FRAMEWORK)
}

fn is_same_type_parameters(
    c: &ProducerContext<'_>,
    widget_class: Id<ClassDeclaration>,
    state_class: Id<ClassDeclaration>,
) -> bool {
    let ast = c.ast;
    let params = |cls: Id<ClassDeclaration>| -> Option<&[Id<TypeParameter>]> {
        let np = ast.cast::<NameWithTypeParameters>(ast[cls].name_part)?;
        let tp = ast[np].type_parameters?;
        Some(ast.list(ast[tp].type_parameters))
    };
    let widget_params = params(widget_class);
    let state_params = params(state_class);
    match (widget_params, state_params) {
        (None, None) => true,
        (None, Some(_)) | (Some(_), None) => false,
        (Some(wp), Some(sp)) => {
            if wp.len() < sp.len() {
                return false;
            }
            'outer: for &s in sp {
                let s_name = c.lexeme(ast[s].name);
                let s_bound = ast[s]
                    .bound
                    .and_then(|b| c.tables.annotation_type.get(b.raw()).copied());
                for &w in wp {
                    let w_name = c.lexeme(ast[w].name);
                    let w_bound = ast[w]
                        .bound
                        .and_then(|b| c.tables.annotation_type.get(b.raw()).copied());
                    if s_name == w_name && s_bound == w_bound {
                        continue 'outer;
                    }
                }
                return false;
            }
            true
        }
    }
}

fn is_default_override(c: &ProducerContext<'_>, method: Option<Id<MethodDeclaration>>) -> bool {
    let ast = c.ast;
    let Some(method) = method else {
        return false;
    };
    let body = ast[method].body.raw();
    let expr = if let Some(b) = ast.cast::<BlockFunctionBody>(body) {
        let stmts = ast.list(ast[ast[b].block].statements);
        if stmts.is_empty() {
            return true;
        }
        if stmts.len() > 1 {
            return false;
        }
        let Some(es) = ast.cast::<ExpressionStatement>(stmts[0]) else {
            return false;
        };
        ast[es].expression.raw()
    } else if let Some(e) = ast.cast::<ExpressionFunctionBody>(body) {
        ast[e].expression.raw()
    } else {
        return false;
    };
    if let Some(mi) = ast.cast::<MethodInvocation>(expr) {
        if ast[mi].target.is_some_and(|t| ast.is::<SuperExpression>(t))
            && c.lexeme(ast[method].name) == c.lexeme(ast[ast[mi].method_name].token)
        {
            return true;
        }
    }
    false
}

assist!(
    FlutterConvertToStatelessWidget,
    a::FLUTTER_CONVERT_TO_STATELESS_WIDGET,
    |c, builder| {
        let ast = c.ast;
        let ctx = c.ctx;
        let Some(widget_class) = ast.this_or_ancestor_of_type::<ClassDeclaration>(c.node) else {
            return;
        };
        let Some(extends) = ast[widget_class].extends_clause else {
            return;
        };
        let superclass = ast[extends].superclass;
        let Some(widget_body) = ast.cast::<BlockClassBody>(ast[widget_class].body) else {
            return;
        };
        if c.selection_offset < c.token_offset(ast[widget_class].class_keyword)
            || c.selection_offset > c.token_end(ast[widget_body].left_bracket)
        {
            return;
        }
        let Some(widget_element) = c
            .locator()
            .declared_element(widget_class)
            .and_then(|e| e.cast::<dartr_element::InterfaceElement>())
        else {
            return;
        };
        let Some(supertype) = ctx.element_supertype(widget_element) else {
            return;
        };
        if !is_exact_widget_type(ctx, Some(supertype), "StatefulWidget", URI_FRAMEWORK) {
            return;
        }
        let widget_members = ast.list_raw(ast[widget_body].members);
        let mut create_state_method: Option<Id<MethodDeclaration>> = None;
        for &m in widget_members {
            if let Some(md) = ast.cast::<MethodDeclaration>(m) {
                if c.lexeme(ast[md].name) == "createState" {
                    if ast[md]
                        .parameters
                        .is_some_and(|p| ast.list_raw(ast[p].parameters).is_empty())
                    {
                        create_state_method = Some(md);
                    }
                    break;
                }
            }
        }
        let Some(create_state_method) = create_state_method else {
            return;
        };
        let mut state_class: Option<Id<ClassDeclaration>> = None;
        for &decl in ast.list_raw(ast[c.unit].declarations) {
            if let Some(cd) = ast.cast::<ClassDeclaration>(decl) {
                if let Some(ext) = ast[cd].extends_clause {
                    let ty = c
                        .tables
                        .annotation_type
                        .get(ast[ext].superclass.raw())
                        .copied();
                    if is_state(ctx, widget_element.raw(), ty) {
                        state_class = Some(cd);
                        break;
                    }
                }
            }
        }
        let Some(state_class) = state_class else {
            return;
        };
        let Some(state_element) = c
            .locator()
            .declared_element(state_class)
            .and_then(|e| e.cast::<dartr_element::InterfaceElement>())
        else {
            return;
        };
        let state_name_tok =
            dartr_resolver::error::support::class_name_token(ast, ast[state_class].name_part);
        if !c.lexeme(state_name_tok).starts_with('_')
            || !is_same_type_parameters(c, widget_class, state_class)
        {
            return;
        }
        let Some(state_body) = ast.cast::<BlockClassBody>(ast[state_class].body) else {
            return;
        };
        let state_members = ast.list_raw(ast[state_body].members);
        let mut assigned: HashSet<String> = HashSet::new();
        for &member in state_members {
            if ast.is::<ConstructorDeclaration>(member) {
                fields_assigned_in_constructor(c, member, state_element.raw(), &mut assigned);
            } else if ast.is::<MethodDeclaration>(member) {
                let mut can_be_stateless = true;
                super::variables::visit(ast, member, &mut |n| {
                    if !can_be_stateless {
                        return;
                    }
                    let Some(mi) = ast.cast::<MethodInvocation>(n) else {
                        return;
                    };
                    let Some(me) = c.locator().element(ast[mi].method_name) else {
                        return;
                    };
                    let Some(enclosing) = ctx.element_data(me).and_then(|d| d.enclosing) else {
                        return;
                    };
                    let Some(cls_elem) = enclosing.cast::<dartr_element::InterfaceElement>() else {
                        return;
                    };
                    if is_exactly(ctx, cls_elem.raw(), "State", URI_FRAMEWORK)
                        && !is_default_override(
                            c,
                            ast.this_or_ancestor_of_type::<MethodDeclaration>(n),
                        )
                    {
                        can_be_stateless = false;
                    }
                });
                if !can_be_stateless {
                    return;
                }
            }
        }
        // _StateUsageVisitor
        let mut used = false;
        super::variables::visit(ast, c.unit.raw(), &mut |n| {
            if used {
                return;
            }
            if let Some(ic) = ast.cast::<InstanceCreationExpression>(n) {
                let Some(ty) = super::create::static_type(c, ic.raw()) else {
                    return;
                };
                if !matches!(*ctx.ty(ty), TypeKind::Interface { element, .. } if element == state_element)
                {
                    return;
                }
                let md = ast.this_or_ancestor_of_type::<MethodDeclaration>(n);
                let cd = md.and_then(|m| ast.this_or_ancestor_of_type::<ClassDeclaration>(m));
                let in_create_state = md.is_some_and(|m| c.lexeme(ast[m].name) == "createState")
                    && cd.and_then(|d| c.locator().declared_element(d))
                        == Some(widget_element.raw());
                if !in_create_state {
                    used = true;
                }
            } else if let Some(mi) = ast.cast::<MethodInvocation>(n) {
                if c.lexeme(ast[ast[mi].method_name].token) != "createState" {
                    return;
                }
                let ty = super::create::static_type(c, mi.raw());
                let is_state_elem = ty.is_some_and(|t| {
                    matches!(*ctx.ty(t), TypeKind::Interface { element, .. } if element == state_element)
                });
                if is_state(ctx, widget_element.raw(), ty) || is_state_elem {
                    used = true;
                }
            }
        });
        if used {
            return;
        }
        let mut nodes_to_move: Vec<NodeId> = Vec::new();
        let mut names_to_move: HashSet<String> = HashSet::new();
        for &member in state_members {
            if let Some(f) = ast.cast::<FieldDeclaration>(member) {
                if is_static_member(c, member) {
                    return;
                }
                for &v in ast.list(ast[ast[f].fields].variables) {
                    let name = c.lexeme(ast[v].name).to_string();
                    if !assigned.contains(&name) {
                        nodes_to_move.push(member);
                        names_to_move.insert(name);
                    }
                }
            } else if let Some(m) = ast.cast::<MethodDeclaration>(member) {
                if is_static_member(c, member) {
                    return;
                }
                if !is_default_override(c, Some(m)) {
                    nodes_to_move.push(member);
                    names_to_move.insert(c.lexeme(ast[m].name).to_string());
                }
            }
        }
        let rewrite = |moved: NodeId| -> String {
            let lines = c.utils.get_lines_range(c.range().node(moved), false);
            let text = c.utils.get_range_text(lines);
            let mut edits: Vec<SourceEdit> = Vec::new();
            super::variables::visit(ast, moved, &mut |n| {
                let Some(id) = ast.cast::<SimpleIdentifier>(n) else {
                    return;
                };
                let Some(e) = c.locator().element(id) else {
                    return;
                };
                if !matches!(e.tag(), Tag::Getter | Tag::Setter | Tag::Method) {
                    return;
                }
                if ctx.element_data(e).and_then(|d| d.enclosing) != Some(widget_element.raw()) {
                    return;
                }
                let name = ctx.element_name(e).unwrap_or("").trim_end_matches('=');
                if names_to_move.contains(name) {
                    return;
                }
                let Some(parent) = ast.parent(id) else {
                    return;
                };
                if let Some(pi) = ast.cast::<PrefixedIdentifier>(parent) {
                    let mut right_bracket_edit: Option<SourceEdit> = None;
                    if !c.lexeme(ast[id].token).contains('$') {
                        if let Some(gp) = ast
                            .parent(pi)
                            .and_then(|p| ast.cast::<InterpolationExpression>(p))
                        {
                            if ast.tokens.ty(ast[gp].left_bracket)
                                == TokenType::STRING_INTERPOLATION_EXPRESSION
                            {
                                edits.push(SourceEdit {
                                    description: None,
                                    offset: c.token_end(ast[gp].left_bracket) - 1 - lines.offset,
                                    length: 1,
                                    replacement: String::new(),
                                    id: 0,
                                });
                                if let Some(rb) = ast[gp].right_bracket {
                                    right_bracket_edit = Some(SourceEdit {
                                        description: None,
                                        offset: c.token_offset(rb) - lines.offset,
                                        length: 1,
                                        replacement: String::new(),
                                        id: 0,
                                    });
                                }
                            }
                        }
                    }
                    let offset = ast.offset(ast[pi].prefix);
                    let length = c.token_end(ast[pi].period) - offset;
                    edits.push(SourceEdit {
                        description: None,
                        offset: offset - lines.offset,
                        length,
                        replacement: String::new(),
                        id: 0,
                    });
                    if let Some(rb_edit) = right_bracket_edit {
                        edits.push(rb_edit);
                    }
                } else if let Some(mi) = ast.cast::<MethodInvocation>(parent) {
                    if let (Some(target), Some(operator)) = (ast[mi].target, ast[mi].operator) {
                        let offset = ast.offset(target);
                        let length = c.token_end(operator) - offset;
                        edits.push(SourceEdit {
                            description: None,
                            offset: offset - lines.offset,
                            length,
                            replacement: String::new(),
                            id: 0,
                        });
                    }
                } else if let Some(pa) = ast.cast::<PropertyAccess>(parent) {
                    if let Some(target) = ast[pa].target {
                        let offset = ast.offset(target);
                        let length = c.token_end(ast[pa].operator) - offset;
                        edits.push(SourceEdit {
                            description: None,
                            offset: offset - lines.offset,
                            length,
                            replacement: String::new(),
                            id: 0,
                        });
                    }
                }
            });
            edits.reverse();
            super::super::bulk_fix_processor::apply_edits(&text, &edits)
        };
        let Some(stateless) = get_class(c, WIDGETS_URI, "StatelessWidget") else {
            return;
        };
        let superclass_range = c.range().node(superclass);
        let deletion_range = c.range().deletion_range(state_class);
        let next_tok = ast.tokens.next(ast.end_token(create_state_method));
        let next_tok = ast
            .tokens
            .get(next_tok)
            .preceding_comments
            .get()
            .unwrap_or(next_tok);
        let create_state_range = c.range().start_offset_end_offset(
            c.utils
                .get_line_content_start(ast.offset(create_state_method)),
            c.utils.get_line_content_start(c.token_offset(next_tok)),
        );
        let new_line = ast.tokens.ty(next_tok) != TokenType::CLOSE_CURLY_BRACKET;
        let moved_texts: Vec<(Option<String>, String)> = nodes_to_move
            .iter()
            .map(|&member| {
                let begin = ast.begin_token(member);
                let comments = ast.tokens.get(begin).preceding_comments;
                let comment = if comments.is_some() {
                    let offset = c.utils.get_line_content_start(c.token_offset(comments));
                    let length = ast.tokens.get(comments).end() - offset;
                    Some(c.utils.get_text(offset, length))
                } else {
                    None
                };
                (comment, rewrite(member))
            })
            .collect();
        builder.add_dart_file_edit(c.path, |b| {
            b.add_replacement(superclass_range.offset, superclass_range.length, |e| {
                e.write_reference(stateless);
            });
            b.add_deletion(deletion_range.offset, deletion_range.length);
            b.add_replacement(create_state_range.offset, create_state_range.length, |e| {
                for (i, (comment, text)) in moved_texts.iter().enumerate() {
                    if let Some(comment) = comment {
                        e.writeln(comment);
                    }
                    e.write(text);
                    if new_line || i + 1 < moved_texts.len() {
                        e.newline();
                    }
                }
            });
        });
    }
);
