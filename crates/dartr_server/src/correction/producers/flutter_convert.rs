// Dart source: pkg/analysis_server/lib/src/services/correction/dart/flutter_convert_to_stateful_widget.dart

//! Dart `FlutterConvertToStatefulWidget`.

use std::collections::HashSet;

use dartr_ast::*;
use dartr_element::{ElemRef, Tag};
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
                        offset,
                        length: 0,
                        replacement: format!("{{{qualifier}."),
                        id: 0,
                    });
                    edits.push(SourceEdit {
                        offset: offset + ast.length(id),
                        length: 0,
                        replacement: "}".to_string(),
                        id: 0,
                    });
                } else {
                    edits.push(SourceEdit {
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
