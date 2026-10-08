// Dart source: pkg/analyzer/lib/src/dart/ast/utilities.dart (NodeLocator2)

//! Node locators.

use crate::arena::{Ast, Id, NodeId};
use crate::generated::nodes::*;
use crate::generated::visitor::AstVisitor;

/// Dart `NodeLocator2`: finds the deepest node for which
/// `node.offset <= start` and `end < node.end`.
pub struct NodeLocator2 {
    start: u32,
    end: u32,
    found: Option<NodeId>,
}

impl NodeLocator2 {
    /// [end] defaults to [start].
    pub fn new(start: u32, end: Option<u32>) -> Self {
        NodeLocator2 {
            start,
            end: end.unwrap_or(start),
            found: None,
        }
    }

    /// Dart `searchWithin`.
    pub fn search_within(mut self, ast: &Ast, node: Option<NodeId>) -> Option<NodeId> {
        ast.accept(node?, &mut self);
        self.found
    }

    fn at_name_end(&self, end: Option<u32>) -> bool {
        self.start == self.end && end == Some(self.start)
    }
}

impl AstVisitor for NodeLocator2 {
    fn visit_class_declaration(&mut self, ast: &Ast, node: Id<ClassDeclaration>) {
        // Names do not have nodes, but an offset at the end of the name is
        // part of the declaration (not of the parameter list).
        let name_part = ast[node].name_part;
        let type_name = match ast.kind(name_part) {
            NodeKind::NameWithTypeParameters => {
                ast[Id::<NameWithTypeParameters>::from_raw(name_part.raw())].type_name
            }
            NodeKind::PrimaryConstructorDeclaration => {
                ast[Id::<PrimaryConstructorDeclaration>::from_raw(name_part.raw())].type_name
            }
            k => unreachable!("ClassNamePart {k:?}"),
        };
        if self.at_name_end(Some(ast.tokens.get(type_name).end())) {
            self.found = Some(node.raw());
            return;
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_constructor_declaration(&mut self, ast: &Ast, node: Id<ConstructorDeclaration>) {
        let n = &ast[node];
        let end = match (n.name, n.type_name) {
            (Some(name), _) => Some(ast.tokens.get(name).end()),
            (None, Some(t)) => Some(ast.end(t)),
            (None, None) => None,
        };
        if self.at_name_end(end) {
            self.found = Some(node.raw());
            return;
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_function_declaration(&mut self, ast: &Ast, node: Id<FunctionDeclaration>) {
        if self.at_name_end(Some(ast.tokens.get(ast[node].name).end())) {
            self.found = Some(node.raw());
            return;
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_method_declaration(&mut self, ast: &Ast, node: Id<MethodDeclaration>) {
        if self.at_name_end(Some(ast.tokens.get(ast[node].name).end())) {
            self.found = Some(node.raw());
            return;
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_node(&mut self, ast: &Ast, node: NodeId) {
        // Don't visit a new tree if the result has been already found.
        if self.found.is_some() {
            return;
        }
        // Check whether the current node covers the selection.
        let begin_token = ast.begin_token(node);
        let mut end_token = ast.end_token(node);
        // Don't include synthetic tokens (the scanner gives an unterminated
        // string a synthetic token with a length, so check the length).
        while end_token != begin_token {
            let t = ast.tokens.get(end_token);
            if t.is_eof() || t.length > 0 {
                break;
            }
            end_token = t.previous;
        }
        let end = ast.tokens.get(end_token).end();
        let start = ast.offset(node);
        if end <= self.start || start > self.end {
            return;
        }
        // Check children.
        ast.visit_children(node, self);
        // Found a child.
        if self.found.is_some() {
            return;
        }
        // Check this node.
        if start <= self.start && self.end < end {
            self.found = Some(node);
        }
    }
}
