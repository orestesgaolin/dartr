// Dart source: pkg/analysis_server/lib/src/services/correction/dart/flutter_move_up.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/flutter_move_down.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/flutter_convert_to_children.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/flutter_remove_widget.dart
// Dart source: pkg/analyzer/lib/src/utilities/extensions/flutter.dart (findInstanceCreationExpression, childArgument, childrenArgument, builderArgument, sliverArgument, sliversArgument)

//! Flutter assists that move, remove or convert widgets.

use dartr_ast::*;

use super::super::change::Position;
use super::super::change_builder::ChangeBuilder;
use super::super::fix_kind::FixKind;
use super::super::flutter::*;
use super::super::generated::assist_kinds as a;
use super::super::generated::fix_kinds as k;
use super::super::producer::*;
use super::simple::assist;

/// Swaps [widget] with its neighbor in a list literal (Dart `FlutterMoveUp`
/// and `FlutterMoveDown`).
fn move_widget(c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>, up: bool) {
    let ast = c.ast;
    let Some(widget) = find_widget_expression(c, c.node) else {
        return;
    };
    let Some(list) = ast.parent(widget).and_then(|p| ast.cast::<ListLiteral>(p)) else {
        return;
    };
    let elements = ast.list_raw(ast[list].elements).to_vec();
    let Some(index) = elements.iter().position(|e| *e == widget) else {
        return;
    };
    let other = if up {
        if index == 0 {
            return;
        }
        elements[index - 1]
    } else {
        if index == elements.len() - 1 {
            return;
        }
        elements[index + 1]
    };
    let other_range = c.range().node(other);
    let other_text = c.utils.get_range_text(other_range);
    let widget_range = c.range().node(widget);
    let widget_text = c.utils.get_range_text(widget_range);
    builder.add_dart_file_edit(c.path, |b| {
        b.add_simple_replacement(other_range.offset, other_range.length, &widget_text);
        b.add_simple_replacement(widget_range.offset, widget_range.length, &other_text);
    });
    let offset = if up {
        other_range.offset
    } else {
        (other_range.offset as i64 + other_range.length as i64 - widget_range.length as i64) as u32
    };
    builder.set_selection(Position {
        file: c.path.to_string(),
        offset,
    });
}

assist!(FlutterMoveUp, a::FLUTTER_MOVE_UP, |c, builder| {
    move_widget(c, builder, true);
});

assist!(FlutterMoveDown, a::FLUTTER_MOVE_DOWN, |c, builder| {
    move_widget(c, builder, false);
});

assist!(
    FlutterConvertToChildren,
    a::FLUTTER_CONVERT_TO_CHILDREN,
    |c, builder| {
        let ast = c.ast;
        let Some(named) = ast.cast::<NamedArgument>(c.node) else {
            return;
        };
        if c.lexeme(ast[named].name) != "child" {
            return;
        }
        if c.tables.param_element.get(named.raw()).is_none()
            && c.tables
                .param_element
                .get(ast[named].argument_expression.raw())
                .is_none()
        {
            return;
        }
        let child_arg = ast[named].argument_expression.raw();
        if !is_widget_expression(c, child_arg) {
            return;
        }
        let named_offset = ast.offset(named);
        let child_offset = ast.offset(child_arg);
        let child_length = ast.length(child_arg);
        let child_src = c.utils.get_node_text(child_arg);
        let eol = c.eol().to_string();
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_insertion(named_offset + "child".len() as u32, "ren");
            if !child_src.contains(&eol) {
                b.add_simple_insertion(child_offset, "[");
                b.add_simple_insertion(child_offset + child_length, "]");
                return;
            }
            let units: Vec<u16> = child_src.encode_utf16().collect();
            let eol_units: Vec<u16> = eol.encode_utf16().collect();
            let mut newline_loc = (0..units.len().saturating_sub(eol_units.len() - 1))
                .rev()
                .find(|&i| units[i..].starts_with(&eol_units))
                .unwrap_or(0) as u32;
            if newline_loc as usize == units.len() {
                newline_loc -= 1;
            }
            let indent_old = c
                .utils
                .get_line_prefix(child_offset + eol_units.len() as u32 + newline_loc);
            let indent_new = format!("{indent_old}{}", c.utils.one_indent());
            let separator = c.utils.get_text(named_offset, child_offset - named_offset);
            let prefix = if separator.contains(&eol) {
                String::new()
            } else {
                format!("{eol}{indent_new}")
            };
            if prefix.is_empty() {
                b.add_simple_insertion(named_offset + "child:".len() as u32, " [");
                b.add_deletion(child_offset - 2, 2);
            } else {
                b.add_simple_insertion(child_offset, "[");
            }
            let new_src =
                c.utils
                    .replace_source_indent(&child_src, &indent_old, &indent_new, false, false);
            let new_src = format!("{prefix}{new_src},{eol}{indent_old}]");
            b.add_simple_replacement(child_offset, child_length, &new_src);
        });
    }
);

/// Dart `AstNode.findInstanceCreationExpression`.
fn find_instance_creation(ast: &Ast, node: NodeId) -> Option<Id<InstanceCreationExpression>> {
    let mut node = node;
    if ast.is::<ImportPrefixReference>(node) {
        node = ast.parent(node)?;
    }
    if ast.is::<SimpleIdentifier>(node) {
        node = ast.parent(node)?;
    }
    if ast.is::<PrefixedIdentifier>(node) {
        node = ast.parent(node)?;
    }
    if ast.is::<NamedType>(node) {
        node = ast.parent(node)?;
    }
    if ast.is::<ConstructorName>(node) {
        node = ast.parent(node)?;
    }
    ast.cast::<InstanceCreationExpression>(node)
}

/// The named argument [name] of [creation].
fn named_argument(
    c: &ProducerContext<'_>,
    creation: Id<InstanceCreationExpression>,
    name: &str,
) -> Option<Id<NamedArgument>> {
    let ast = c.ast;
    let arguments = ast[ast[creation].argument_list].arguments;
    ast.list_raw(arguments)
        .iter()
        .filter_map(|a| ast.cast::<NamedArgument>(*a))
        .find(|a| c.lexeme(ast[*a].name) == name)
}

/// Dart `FlutterRemoveWidget`.
pub struct FlutterRemoveWidget;

impl FlutterRemoveWidget {
    fn remove_single(
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        creation: Id<InstanceCreationExpression>,
        expression: NodeId,
    ) {
        let ast = c.ast;
        let text = c.utils.get_node_text(expression);
        let indent_old = c.utils.get_line_prefix(ast.offset(expression));
        let indent_new = c.utils.get_line_prefix(ast.offset(creation));
        let text = c
            .utils
            .replace_source_indent(&text, &indent_old, &indent_new, false, false);
        let range = c.range().node(creation);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(range.offset, range.length, &text)
        });
    }

    fn remove_children(
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        creation: Id<InstanceCreationExpression>,
        children: &[NodeId],
    ) {
        let ast = c.ast;
        let parent_is_list = ast
            .parent(creation)
            .is_some_and(|p| ast.is::<ListLiteral>(p));
        if children.len() > 1 && !parent_is_list {
            return;
        }
        let (first, last) = (children[0], children[children.len() - 1]);
        let text = c.utils.get_range_text(c.range().start_end(first, last));
        let indent_old = c.utils.get_line_prefix(ast.offset(first));
        let indent_new = c.utils.get_line_prefix(ast.offset(creation));
        let text = c
            .utils
            .replace_source_indent(&text, &indent_old, &indent_new, false, false);
        let range = c.range().node(creation);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(range.offset, range.length, &text)
        });
    }

    fn remove_builder(
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        creation: Id<InstanceCreationExpression>,
        argument: Id<NamedArgument>,
    ) {
        let ast = c.ast;
        let Some(function) = ast.cast::<FunctionExpression>(ast[argument].argument_expression)
        else {
            return;
        };
        let Some(parameters) = ast[function].parameters else {
            return;
        };
        let Some(&first) = ast.list_raw(ast[parameters].parameters).first() else {
            return;
        };
        let Some(parameter) = c.locator().declared_element(first) else {
            return;
        };
        let body = ast[function].body.raw();
        let mut used = false;
        super::variables::visit(ast, body, &mut |n| {
            if let Some(id) = ast.cast::<SimpleIdentifier>(n) {
                if c.locator().write_or_read_element(id) == Some(parameter) {
                    used = true;
                }
            }
        });
        if used {
            return;
        }
        if let Some(b) = ast.cast::<BlockFunctionBody>(body) {
            let statements = ast.list(ast[ast[b].block].statements);
            if statements.len() != 1 {
                return;
            }
            let Some(r) = ast.cast::<ReturnStatement>(statements[0]) else {
                return;
            };
            let Some(expression) = ast[r].expression else {
                return;
            };
            Self::remove_single(c, builder, creation, expression.raw());
        } else if let Some(b) = ast.cast::<ExpressionFunctionBody>(body) {
            Self::remove_single(c, builder, creation, ast[b].expression.raw());
        }
    }
}

impl CorrectionProducer for FlutterRemoveWidget {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::REMOVE_UNNECESSARY_CONTAINER)
    }

    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::REMOVE_UNNECESSARY_CONTAINER_MULTI)
    }

    fn assist_kind(&self) -> Option<&'static FixKind> {
        Some(&a::FLUTTER_REMOVE_WIDGET)
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let Some(creation) = find_instance_creation(ast, c.node) else {
            return;
        };
        if !is_widget_creation(c, creation) {
            return;
        }
        let list_elements = |argument: Id<NamedArgument>| -> Option<Vec<NodeId>> {
            let list = ast.cast::<ListLiteral>(ast[argument].argument_expression)?;
            let elements = ast.list_raw(ast[list].elements).to_vec();
            (!elements.is_empty()).then_some(elements)
        };
        if let Some(children) = named_argument(c, creation, "children") {
            if let Some(elements) = list_elements(children) {
                Self::remove_children(c, builder, creation, &elements);
            }
        } else if let Some(child) = named_argument(c, creation, "child") {
            let expression = ast[child].argument_expression.raw();
            Self::remove_single(c, builder, creation, expression);
        } else if let Some(argument) = named_argument(c, creation, "builder") {
            Self::remove_builder(c, builder, creation, argument);
        } else if let Some(slivers) = named_argument(c, creation, "slivers") {
            if let Some(elements) = list_elements(slivers) {
                Self::remove_children(c, builder, creation, &elements);
            }
        } else if let Some(sliver) = named_argument(c, creation, "sliver") {
            let expression = ast[sliver].argument_expression.raw();
            Self::remove_single(c, builder, creation, expression);
        } else {
            let Some(list) = ast
                .parent(creation)
                .and_then(|p| ast.cast::<ListLiteral>(p))
            else {
                return;
            };
            let elements = ast.list_raw(ast[list].elements).to_vec();
            let range = c.range().node_in_list(&elements, creation.raw());
            builder.add_dart_file_edit(c.path, |b| b.add_deletion(range.offset, range.length));
        }
    }
}

/// Dart `FlutterParentAndChild._singleChildInChildren`.
fn single_child_in_children(
    c: &ProducerContext<'_>,
    parent: Id<InstanceCreationExpression>,
) -> Option<Id<InstanceCreationExpression>> {
    let ast = c.ast;
    let children = named_argument(c, parent, "children")?;
    let list = ast.cast::<ListLiteral>(ast[children].argument_expression)?;
    let elements = ast.list_raw(ast[list].elements);
    if elements.len() != 1 {
        return None;
    }
    ast.cast::<InstanceCreationExpression>(elements[0])
}

/// Dart `FlutterParentAndChild.swapParentAndChild`.
fn swap_parent_and_child(
    c: &ProducerContext<'_>,
    builder: &mut ChangeBuilder<'_>,
    parent: Id<InstanceCreationExpression>,
    child: Id<InstanceCreationExpression>,
    parent_had_single_child: bool,
) {
    let ast = c.ast;
    let parent_args = ast[parent].argument_list;
    let child_args = ast[child].argument_list;
    let child_argument_in_parent = ast
        .list_raw(ast[parent_args].arguments)
        .iter()
        .filter_map(|a| ast.cast::<NamedArgument>(*a))
        .find(|a| ast[*a].argument_expression.raw() == child.raw());
    let stable_child: NodeId = if let Some(first) = single_child_in_children(c, child) {
        first.raw()
    } else if let Some(argument) = named_argument(c, child, "child") {
        argument.raw()
    } else {
        return;
    };
    let child_text = c
        .utils
        .get_range_text(c.range().start_start(child, child_args));
    let parent_text = c
        .utils
        .get_range_text(c.range().start_start(parent, parent_args));
    let parent_indent = c.utils.get_line_prefix(ast.offset(parent));
    let child_indent = format!("{parent_indent}  ");
    let is_children_argument = |a: NodeId| {
        ast.cast::<NamedArgument>(a)
            .is_some_and(|a| c.lexeme(ast[a].name) == "children")
    };
    let range = c.range().node(parent);
    builder.add_dart_file_edit(c.path, |b| {
        b.add_replacement(range.offset, range.length, |e| {
            e.write(&child_text);
            e.writeln("(");
            for &argument in ast.list_raw(ast[child_args].arguments) {
                if argument != stable_child {
                    let text = c.utils.get_node_text(argument);
                    let text = c.utils.replace_source_indent(
                        &text,
                        &child_indent,
                        &parent_indent,
                        false,
                        false,
                    );
                    e.write(&parent_indent);
                    e.write("  ");
                    e.write(&text);
                    e.writeln(",");
                }
            }
            e.write(&parent_indent);
            e.write("  ");
            e.write("child: ");
            e.write(&parent_text);
            e.writeln("(");
            for &argument in ast.list_raw(ast[parent_args].arguments) {
                if Some(argument) != child_argument_in_parent.map(|a| a.raw())
                    && !is_children_argument(argument)
                {
                    let text = c.utils.get_node_text(argument);
                    let text = c.utils.replace_source_indent(
                        &text,
                        &parent_indent,
                        &child_indent,
                        false,
                        false,
                    );
                    e.write(&child_indent);
                    e.write("  ");
                    e.write(&text);
                    e.writeln(",");
                }
            }
            {
                let mut text = c.utils.get_node_text(stable_child);
                if text.trim().starts_with("child: ") {
                    text = text["child: ".len()..].to_string();
                } else if text.trim().starts_with("children: ") {
                    text = text["children: ".len()..].to_string();
                }
                e.write(&child_indent);
                e.write("  ");
                if parent_had_single_child {
                    match child_argument_in_parent {
                        Some(a) => {
                            e.write(c.lexeme(ast[a].name));
                            e.write(": ");
                        }
                        None => e.write("child: "),
                    }
                } else {
                    e.write("children: [");
                    e.newline();
                    e.write(&child_indent);
                    e.write("    ");
                }
                e.write(&text);
                e.writeln(",");
            }
            e.write(&child_indent);
            if !parent_had_single_child {
                e.write("  ");
                e.write("]");
                e.writeln(",");
                e.write(&child_indent);
            }
            e.writeln("),");
            e.write(&parent_indent);
            e.write(")");
        });
    });
}

assist!(
    FlutterSwapWithParent,
    a::FLUTTER_SWAP_WITH_PARENT,
    |c, builder| {
        let ast = c.ast;
        let Some(child) = find_instance_creation(ast, c.node) else {
            return;
        };
        if !is_widget_creation(c, child) {
            return;
        }
        let mut parent_had_single_child = true;
        let mut named: Option<NodeId> = None;
        if let Some(list) = ast.parent(child).and_then(|p| ast.cast::<ListLiteral>(p)) {
            if ast.list_raw(ast[list].elements).len() != 1 {
                return;
            }
            if let Some(p) = ast.parent(list).filter(|p| ast.is::<NamedArgument>(*p)) {
                named = Some(p);
                parent_had_single_child = false;
            }
        }
        let base = named.or_else(|| ast.parent(child));
        let Some(expr) = base
            .and_then(|n| ast.parent(n))
            .and_then(|n| ast.parent(n))
            .and_then(|n| ast.cast::<InstanceCreationExpression>(n))
        else {
            return;
        };
        swap_parent_and_child(c, builder, expr, child, parent_had_single_child);
    }
);

assist!(
    FlutterSwapWithChild,
    a::FLUTTER_SWAP_WITH_CHILD,
    |c, builder| {
        let ast = c.ast;
        let Some(parent) = find_instance_creation(ast, c.node) else {
            return;
        };
        if !is_widget_creation(c, parent) {
            return;
        }
        let mut parent_has_single_child = true;
        let mut child: Option<NodeId> = None;
        if let Some(first) = single_child_in_children(c, parent) {
            child = Some(first.raw());
            parent_has_single_child = false;
        }
        let child = child.or_else(|| {
            named_argument(c, parent, "child").map(|a| ast[a].argument_expression.raw())
        });
        let Some(child) = child.and_then(|n| ast.cast::<InstanceCreationExpression>(n)) else {
            return;
        };
        if !is_widget_creation(c, child) {
            return;
        }
        swap_parent_and_child(c, builder, parent, child, parent_has_single_child);
    }
);
