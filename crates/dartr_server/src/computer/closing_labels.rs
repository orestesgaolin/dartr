// Dart source: pkg/analysis_server/lib/src/computer/computer_closing_labels.dart

//! Closing labels (Dart `DartUnitClosingLabelsComputer`). The Dart computer
//! runs on the resolved unit; the parts that need resolution use
//! [`super::heuristics`].

use std::collections::HashSet;

use dartr_ast::to_source::to_source;
use dartr_ast::*;
use dartr_syntax::LineInfo;

use super::heuristics;

/// Dart `ClosingLabel`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ClosingLabel {
    pub offset: u32,
    pub length: u32,
    pub label: String,
}

/// Dart `DartUnitClosingLabelsComputer(lineInfo, unit).compute()`.
pub fn compute_closing_labels(
    ast: &Ast,
    unit: Id<CompilationUnit>,
    line_info: &LineInfo,
) -> Vec<ClosingLabel> {
    let mut v = Visitor {
        line_info,
        labels: Vec::new(),
        has_nesting: HashSet::new(),
        single_line: HashSet::new(),
        interpolated_strings_entered: 0,
        stack: Vec::new(),
        has_test_import: heuristics::imports_test_framework(ast, unit),
    };
    ast.accept(unit, &mut v);
    let Visitor {
        labels,
        has_nesting,
        single_line,
        ..
    } = v;
    labels
        .into_iter()
        .filter(|l| has_nesting.contains(l) && !single_line.contains(l))
        .collect()
}

struct Visitor<'a> {
    line_info: &'a LineInfo,
    labels: Vec<ClosingLabel>,
    has_nesting: HashSet<ClosingLabel>,
    single_line: HashSet<ClosingLabel>,
    interpolated_strings_entered: u32,
    stack: Vec<ClosingLabel>,
    has_test_import: bool,
}

impl Visitor<'_> {
    /// Dart `_addLabel`.
    fn add_label(
        &mut self,
        ast: &Ast,
        node: NodeId,
        label: String,
        check_lines_using: Option<NodeId>,
    ) -> Option<ClosingLabel> {
        if self.interpolated_strings_entered > 0 {
            return None;
        }
        let check = check_lines_using.unwrap_or(node);
        let start = self.line_info.get_location(ast.offset(check));
        let end = self
            .line_info
            .get_location(ast.end(check).saturating_sub(1));
        let closing_label = ClosingLabel {
            offset: ast.offset(node),
            length: ast.length(node),
            label,
        };
        if (end.line_number as i64) - (start.line_number as i64) < 1 {
            self.single_line.insert(closing_label.clone());
        }
        if let Some(parent) = self.stack.last() {
            self.has_nesting.insert(parent.clone());
            self.has_nesting.insert(closing_label.clone());
        }
        self.labels.push(closing_label.clone());
        Some(closing_label)
    }

    fn with_label(&mut self, ast: &Ast, node: NodeId, label: Option<ClosingLabel>) {
        let pushed = label.is_some();
        if let Some(l) = label {
            self.stack.push(l);
        }
        self.visit_node(ast, node);
        if pushed {
            self.stack.pop();
        }
    }
}

impl AstVisitor for Visitor<'_> {
    fn visit_instance_creation_expression(
        &mut self,
        ast: &Ast,
        node: Id<InstanceCreationExpression>,
    ) {
        let n = &ast[node];
        let constructor_name = &ast[n.constructor_name];
        let mut text = ast.qualified_name(constructor_name.type_);
        if let Some(name) = constructor_name.name {
            text.push('.');
            text.push_str(ast.tokens.lexeme(ast[name].token));
        }
        let label = self.add_label(ast, node.raw(), text, Some(n.argument_list.raw()));
        self.with_label(ast, node.raw(), label);
    }

    fn visit_list_literal(&mut self, ast: &Ast, node: Id<ListLiteral>) {
        let label = match ast[node].type_arguments {
            Some(args) => match ast.list(ast[args].arguments).first() {
                Some(&first) => {
                    let text = format!("<{}>[]", to_source(ast, first));
                    self.add_label(ast, node.raw(), text, None)
                }
                None => None,
            },
            None => None,
        };
        self.with_label(ast, node.raw(), label);
    }

    fn visit_method_invocation(&mut self, ast: &Ast, node: Id<MethodInvocation>) {
        let n = &ast[node];
        // Resolved as an `InstanceCreationExpression` by the Dart server.
        if let Some(creation) = heuristics::implicit_creation(ast, node) {
            let mut text = creation.type_name;
            if let Some(name) = creation.constructor_name {
                text.push('.');
                text.push_str(&name);
            }
            let label = self.add_label(ast, node.raw(), text, Some(n.argument_list.raw()));
            self.with_label(ast, node.raw(), label);
            return;
        }
        let mut label = None;
        if heuristics::test_call(ast, node, self.has_test_import).is_some() {
            let function_name = ast.tokens.lexeme(ast[n.method_name].token);
            if let Some(&first) = ast.list(ast[n.argument_list].arguments).first() {
                let expression = match ast.cast::<NamedArgument>(first) {
                    Some(named) => ast[named].argument_expression.raw(),
                    None => first.raw(),
                };
                let text = format!("{function_name}({})", to_source(ast, expression));
                label = self.add_label(ast, node.raw(), text, None);
            }
        }
        self.with_label(ast, node.raw(), label);
    }

    fn visit_string_interpolation(&mut self, ast: &Ast, node: Id<StringInterpolation>) {
        self.interpolated_strings_entered += 1;
        self.visit_node(ast, node.raw());
        self.interpolated_strings_entered -= 1;
    }
}
