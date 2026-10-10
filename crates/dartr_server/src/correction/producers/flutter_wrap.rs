// Dart source: pkg/analysis_server/lib/src/services/correction/dart/flutter_wrap.dart (FlutterWrap, _WrapSingleWidget, _WrapMultipleWidgets)

//! Dart `FlutterWrap`: wrap a widget expression in `Center`, `Container`,
//! `Expanded`, `Flexible`, `Padding`, `SizedBox`, a generic widget, or the
//! selected widgets in `Column` or `Row`.

use dartr_ast::*;

use super::super::change_builder::{ChangeBuilder, ChangeWorkspace};
use super::super::fix_kind::FixKind;
use super::super::flutter::*;
use super::super::generated::assist_kinds as a;
use super::super::producer::*;

/// The wrapper of [WrapSingleWidget].
#[derive(Clone, Copy)]
enum Wrapper {
    Generic,
    Center,
    Container,
    Expanded,
    Flexible,
    Padding,
    SizedBox,
}

/// Dart `_WrapSingleWidget` and its subclasses.
pub struct WrapSingleWidget {
    widget: NodeId,
    wrapper: Wrapper,
}

/// Dart `Expression.inConstantContext` (the common cases).
pub fn in_constant_context(ast: &Ast, node: NodeId) -> bool {
    let mut child = node;
    let mut current = ast.parent(node);
    while let Some(n) = current {
        if let Some(i) = ast.cast::<InstanceCreationExpression>(n) {
            if ast[i]
                .keyword
                .is_some_and(|k| ast.tokens.lexeme(k) == "const")
            {
                return true;
            }
        } else if let Some(l) = ast.cast::<ListLiteral>(n) {
            if ast[l].const_keyword.is_some() {
                return true;
            }
        } else if let Some(l) = ast.cast::<SetOrMapLiteral>(n) {
            if ast[l].const_keyword.is_some() {
                return true;
            }
        } else if let Some(v) = ast.cast::<VariableDeclaration>(n) {
            if ast[v].initializer.map(|i| i.raw()) == Some(child) {
                let list = ast
                    .parent(v)
                    .and_then(|l| ast.cast::<VariableDeclarationList>(l));
                if list.is_some_and(|l| {
                    ast[l]
                        .keyword
                        .is_some_and(|k| ast.tokens.lexeme(k) == "const")
                }) {
                    return true;
                }
            }
        } else if ast.is::<Annotation>(n) {
            return true;
        } else if ast.is::<FunctionBody>(n) || ast.is::<CompilationUnitMember>(n) {
            return false;
        }
        child = n;
        current = ast.parent(n);
    }
    false
}

impl WrapSingleWidget {
    fn class(&self) -> Option<(&'static str, &'static str)> {
        Some(match self.wrapper {
            Wrapper::Generic => return None,
            Wrapper::Center => ("Center", WIDGETS_URI),
            Wrapper::Container => ("Container", WIDGETS_URI),
            Wrapper::Expanded => ("Expanded", WIDGETS_URI),
            Wrapper::Flexible => ("Flexible", WIDGETS_URI),
            Wrapper::Padding => ("Padding", WIDGETS_URI),
            Wrapper::SizedBox => ("SizedBox", WIDGETS_URI),
        })
    }

    fn leading_lines(&self, c: &ProducerContext<'_>) -> Vec<String> {
        if !matches!(self.wrapper, Wrapper::Padding) {
            return Vec::new();
        }
        let keyword = if in_constant_context(c.ast, self.widget) {
            ""
        } else {
            " const"
        };
        let padding = if c.is_lint_enabled("prefer_int_literals") {
            "8"
        } else {
            "8.0"
        };
        vec![format!("padding:{keyword} EdgeInsets.all({padding}),")]
    }
}

impl CorrectionProducer for WrapSingleWidget {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        None
    }

    fn assist_kind(&self) -> Option<&'static FixKind> {
        Some(match self.wrapper {
            Wrapper::Generic => &a::FLUTTER_WRAP_GENERIC,
            Wrapper::Center => &a::FLUTTER_WRAP_CENTER,
            Wrapper::Container => &a::FLUTTER_WRAP_CONTAINER,
            Wrapper::Expanded => &a::FLUTTER_WRAP_EXPANDED,
            Wrapper::Flexible => &a::FLUTTER_WRAP_FLEXIBLE,
            Wrapper::Padding => &a::FLUTTER_WRAP_PADDING,
            Wrapper::SizedBox => &a::FLUTTER_WRAP_SIZED_BOX,
        })
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let mut widget_src = c.utils.get_node_text(self.widget);
        let parent_class = match self.class() {
            Some((name, uri)) => match get_class(c, uri, name) {
                Some(e) => Some(e),
                None => return,
            },
            None => None,
        };
        let leading_lines = self.leading_lines(c);
        let range = c.range().node(self.widget);
        let widget_offset = ast.offset(self.widget);
        builder.add_dart_file_edit(c.path, |b| {
            let eol = b.eol();
            b.add_replacement(range.offset, range.length, |e| {
                match parent_class {
                    None => e.add_simple_linked_edit("WIDGET", "widget", None),
                    Some(class) => e.write_reference(class),
                }
                e.write("(");
                if parent_class.is_some() {
                    e.select_here();
                }
                if widget_src.contains(&eol) || !leading_lines.is_empty() {
                    let indent_old = c.utils.get_line_prefix(widget_offset);
                    let indent_new = format!("{indent_old}{}", c.utils.one_indent());
                    for line in &leading_lines {
                        e.writeln("");
                        e.write(&indent_new);
                        e.write(line);
                    }
                    e.writeln("");
                    e.write(&indent_new);
                    widget_src = c.utils.replace_source_indent(
                        &widget_src,
                        &indent_old,
                        &indent_new,
                        false,
                        false,
                    );
                    widget_src.push_str(&format!(",{eol}{indent_old}"));
                }
                if parent_class.is_none() {
                    e.add_simple_linked_edit("CHILD", "child", None);
                } else {
                    e.write("child");
                }
                e.write(": ");
                e.write(&widget_src);
                e.write(")");
            });
        });
    }
}

/// Dart `_WrapMultipleWidgets` (`Column` and `Row`).
pub struct WrapMultipleWidgets {
    first: NodeId,
    last: NodeId,
    row: bool,
}

impl CorrectionProducer for WrapMultipleWidgets {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        None
    }

    fn assist_kind(&self) -> Option<&'static FixKind> {
        Some(if self.row {
            &a::FLUTTER_WRAP_ROW
        } else {
            &a::FLUTTER_WRAP_COLUMN
        })
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let range = c.range().start_end(self.first, self.last);
        let src = c.utils.get_range_text(range);
        let name = if self.row { "Row" } else { "Column" };
        let Some(parent_class) = get_class(c, WIDGETS_URI, name) else {
            return;
        };
        if get_class(c, WIDGETS_URI, "Widget").is_none() {
            return;
        }
        let first_offset = ast.offset(self.first);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_replacement(range.offset, range.length, |e| {
                e.write_reference(parent_class);
                e.write("(");
                let indent_old = c.utils.get_line_prefix(first_offset);
                let indent_new1 = format!("{indent_old}{}", c.utils.one_indent());
                let indent_new2 = format!("{indent_old}{}", c.utils.two_indents());
                e.writeln("");
                e.write(&indent_new1);
                e.write("children: [");
                e.writeln("");
                let new_src =
                    c.utils
                        .replace_source_indent(&src, &indent_old, &indent_new2, false, false);
                e.write(&indent_new2);
                e.write(&new_src);
                e.write(",");
                e.writeln("");
                e.write(&indent_new1);
                e.write("],");
                e.writeln("");
                e.write(&indent_old);
                e.write(")");
            });
        });
    }
}

/// The parent `InstanceCreationExpression` of a widget argument (Dart
/// `_getParentInstanceCreationExpression`).
fn parent_instance_creation(ast: &Ast, node: NodeId) -> Option<Id<InstanceCreationExpression>> {
    let parent = ast.parent(node)?;
    let mut named: Option<NodeId> = None;
    if ast.is::<ListLiteral>(parent) {
        if let Some(p) = ast.parent(parent).filter(|p| ast.is::<NamedArgument>(*p)) {
            named = Some(p);
        }
    }
    let base = named.unwrap_or(parent);
    let creation = ast.parent(base).and_then(|p| ast.parent(p))?;
    ast.cast::<InstanceCreationExpression>(creation)
}

fn is_parent_widget(c: &ProducerContext<'_>, node: NodeId) -> bool {
    parent_instance_creation(c.ast, node).is_some_and(|p| is_widget_creation(c, p))
}

fn is_parent_flex_widget(c: &ProducerContext<'_>, node: NodeId) -> bool {
    let Some(parent) = parent_instance_creation(c.ast, node) else {
        return false;
    };
    if !is_widget_creation(c, parent) {
        return false;
    }
    let ty = super::create::static_type(c, parent.raw());
    ty.and_then(|t| super::super::dart_edit::interface_element(c.ctx, t))
        .is_some_and(|e| is_flex_widget(c.ctx, e.raw()))
}

/// The selected nodes (Dart `SelectionAnalyzer.selectedNodes`): the
/// children of the covering node that are inside the selection.
fn selected_nodes(c: &ProducerContext<'_>) -> Vec<NodeId> {
    if c.selection_length == 0 {
        return Vec::new();
    }
    let ast = c.ast;
    let start = c.selection_offset;
    let end = start + c.selection_length;
    let covering = c.node;
    if ast.offset(covering) >= start && ast.end(covering) <= end {
        return vec![covering];
    }
    ast.children(covering)
        .into_iter()
        .filter(|&n| ast.offset(n) >= start && ast.end(n) <= end)
        .collect()
}

/// Dart `FlutterWrap.producers`.
pub fn flutter_wrap_producers(
    c: &ProducerContext<'_>,
    _workspace: &mut dyn ChangeWorkspace,
) -> Vec<Box<dyn CorrectionProducer>> {
    let ast = c.ast;
    let ctx = c.ctx;
    let mut producers: Vec<Box<dyn CorrectionProducer>> = Vec::new();
    if let Some(widget) = find_widget_expression(c, c.node) {
        let ty = super::create::static_type(c, widget);
        let single = |wrapper| -> Box<dyn CorrectionProducer> {
            Box::new(WrapSingleWidget { widget, wrapper })
        };
        producers.push(single(Wrapper::Generic));
        if !is_exact_widget_type(ctx, ty, "Center", URI_BASIC) {
            producers.push(single(Wrapper::Center));
        }
        if !is_exact_widget_type(ctx, ty, "Container", URI_CONTAINER) {
            producers.push(single(Wrapper::Container));
        }
        let flex_parent = is_parent_flex_widget(c, widget) || !is_parent_widget(c, widget);
        if !is_exact_widget_type(ctx, ty, "Expanded", URI_BASIC) && flex_parent {
            producers.push(single(Wrapper::Expanded));
        }
        if !is_exact_widget_type(ctx, ty, "Flexible", URI_BASIC) && flex_parent {
            producers.push(single(Wrapper::Flexible));
        }
        if !is_exact_widget_type(ctx, ty, "Padding", URI_BASIC) {
            producers.push(single(Wrapper::Padding));
        }
        if !is_exact_widget_type(ctx, ty, "SizedBox", URI_BASIC) {
            producers.push(single(Wrapper::SizedBox));
        }
    }
    // Dart `_wrapMultipleWidgets`.
    let mut widgets: Vec<NodeId> = Vec::new();
    let selected = selected_nodes(c);
    if !selected.is_empty() {
        for mut node in selected {
            if ast.is::<ConstructorName>(node) {
                if let Some(p) = ast
                    .parent(node)
                    .filter(|p| ast.is::<InstanceCreationExpression>(*p))
                {
                    node = p;
                }
            }
            if !ast.is::<Expression>(node) || !is_widget_expression(c, node) {
                return producers;
            }
            widgets.push(node);
        }
    } else {
        let mut covering = c.node;
        if ast.is::<ArgumentList>(covering) && ast.offset(covering) == c.selection_offset {
            if let Some(p) = ast.parent(covering) {
                covering = p;
            }
        }
        if let Some(w) = find_widget_expression(c, covering) {
            widgets.push(w);
        }
    }
    let (Some(&first), Some(&last)) = (widgets.first(), widgets.last()) else {
        return producers;
    };
    producers.push(Box::new(WrapMultipleWidgets {
        first,
        last,
        row: false,
    }));
    producers.push(Box::new(WrapMultipleWidgets {
        first,
        last,
        row: true,
    }));
    producers
}

/// Dart `_FlutterBaseWrapBuilder` and its subclasses.
pub struct WrapBuilder {
    builder_name: &'static str,
    extra_named_params: &'static [&'static str],
    extra_builder_params: &'static [&'static str],
    kind: &'static FixKind,
}

impl CorrectionProducer for WrapBuilder {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        None
    }

    fn assist_kind(&self) -> Option<&'static FixKind> {
        Some(self.kind)
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let Some(widget) = find_widget_expression(c, c.node) else {
            return;
        };
        let ty = super::create::static_type(c, widget);
        if is_exact_widget_type(c.ctx, ty, "Builder", URI_BASIC) {
            return;
        }
        let widget_src = c.utils.get_node_text(widget);
        let Some(builder_element) = get_class(c, WIDGETS_URI, self.builder_name) else {
            return;
        };
        let mut params = vec!["context"];
        params.extend_from_slice(self.extra_builder_params);
        let range = c.range().node(widget);
        let widget_offset = ast.offset(widget);
        let named_params = self.extra_named_params.join(", ");
        let trailing_commas = c.code_style().add_trailing_commas();
        builder.add_dart_file_edit(c.path, |b| {
            b.add_replacement(range.offset, range.length, |e| {
                e.write_reference(builder_element);
                e.writeln("(");
                let indent_old = c.utils.get_line_prefix(widget_offset);
                let indent_new1 = format!("{indent_old}{}", c.utils.one_indent());
                let indent_new2 = format!("{indent_old}{}", c.utils.two_indents());
                if !named_params.is_empty() {
                    e.write(&indent_new1);
                    e.write(&format!("{named_params}: "));
                    e.add_simple_linked_edit("variable", &named_params, None);
                    e.writeln(",");
                }
                e.write(&indent_new1);
                e.writeln(&format!("builder: ({}) {{", params.join(", ")));
                let src = c.utils.replace_source_indent(
                    &widget_src,
                    &indent_old,
                    &indent_new2,
                    false,
                    false,
                );
                e.write(&indent_new2);
                e.write(&format!("return {src}"));
                e.writeln(";");
                e.write(&indent_new1);
                e.writeln(&format!("}}{}", if trailing_commas { "," } else { "" }));
                e.write(&indent_old);
                e.write(")");
            });
        });
    }
}

/// Dart `FlutterWrapBuilders.producers`.
pub fn flutter_wrap_builders_producers(
    c: &ProducerContext<'_>,
    _workspace: &mut dyn ChangeWorkspace,
) -> Vec<Box<dyn CorrectionProducer>> {
    if find_widget_expression(c, c.node).is_none() {
        return Vec::new();
    }
    let make = |builder_name,
                extra_named_params,
                extra_builder_params,
                kind|
     -> Box<dyn CorrectionProducer> {
        Box::new(WrapBuilder {
            builder_name,
            extra_named_params,
            extra_builder_params,
            kind,
        })
    };
    vec![
        make("Builder", &[], &[], &a::FLUTTER_WRAP_BUILDER),
        make(
            "ValueListenableBuilder",
            &["valueListenable"],
            &["value", "child"],
            &a::FLUTTER_WRAP_VALUE_LISTENABLE_BUILDER,
        ),
        make(
            "StreamBuilder",
            &["stream"],
            &["asyncSnapshot"],
            &a::FLUTTER_WRAP_STREAM_BUILDER,
        ),
        make(
            "FutureBuilder",
            &["future"],
            &["asyncSnapshot"],
            &a::FLUTTER_WRAP_FUTURE_BUILDER,
        ),
    ]
}

super::simple::assist!(FlutterWrapGeneric, a::FLUTTER_WRAP_GENERIC, |c, builder| {
    let ast = c.ast;
    let Some(list) = ast.cast::<ListLiteral>(c.node) else {
        return;
    };
    let elements = ast.list_raw(ast[list].elements);
    if elements.iter().any(|&el| {
        ast.cast::<InstanceCreationExpression>(el)
            .is_none_or(|ic| !is_widget_creation(c, ic))
    }) {
        return;
    }
    let eol = c.eol();
    let literal_src = c.utils.get_node_text(list);
    let Some(newline_idx) = literal_src.rfind(eol) else {
        return;
    };
    if newline_idx == literal_src.len() - 1 {
        return;
    }
    let indent_old = c
        .utils
        .get_line_prefix(ast.offset(list) + eol.len() as u32 + newline_idx as u32);
    let indent_arg = format!("{indent_old}{}", c.utils.one_indent());
    let indent_list = format!("{indent_old}{}", c.utils.two_indents());
    let replaced =
        c.utils
            .replace_source_indent(&literal_src, &indent_old, &indent_list, false, false);
    let r = c.range().node(list);
    builder.add_dart_file_edit(c.path, |b| {
        b.add_replacement(r.offset, r.length, |e| {
            e.write("[");
            e.write(eol);
            e.write(&indent_arg);
            e.add_simple_linked_edit("WIDGET", "widget", None);
            e.write("(");
            e.write(eol);
            e.write(&indent_list);
            e.write("children: ");
            e.write(&replaced);
            e.write(",");
            e.write(eol);
            e.write(&indent_arg);
            e.write("),");
            e.write(eol);
            e.write(&indent_old);
            e.write("]");
        });
    });
});
