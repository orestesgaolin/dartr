//! The node model as the AST builder and the resolver use it: nodes are made
//! with `Ast::add` on the tokens of the scanner, and changed like the Dart
//! resolver changes them (`AstRewriter`: a `MethodInvocation` becomes a
//! `FunctionExpressionInvocation` when the target is a local function).

use dartr_ast::*;
use dartr_syntax::{TokenId, scan_for_analyzer};

/// `void main() {f(x);}` built by hand, like the AST builder does.
struct Unit {
    ast: Ast,
    unit: Id<CompilationUnit>,
    main: Id<FunctionDeclaration>,
    statement: Id<ExpressionStatement>,
    invocation: Id<MethodInvocation>,
    f: Id<SimpleIdentifier>,
    x: Id<SimpleIdentifier>,
}

fn build() -> Unit {
    let scan = scan_for_analyzer("void main() {f(x);}");
    let first = scan.first;
    let toks: Vec<TokenId> = scan.tokens().iter_from(first).collect();
    let mut ast = Ast::new(scan.scan.tokens);
    let lexemes: Vec<&str> = toks.iter().map(|&t| ast.tokens.lexeme(t)).collect();
    assert_eq!(
        lexemes,
        [
            "void", "main", "(", ")", "{", "f", "(", "x", ")", ";", "}", ""
        ]
    );
    let t = |i: usize| toks[i];

    let f = ast.add(SimpleIdentifier { token: t(5) });
    let x = ast.add(SimpleIdentifier { token: t(7) });
    let arguments = ast.new_list([x.upcast::<Argument>()]);
    let argument_list = ast.add(ArgumentList {
        left_parenthesis: t(6),
        arguments,
        right_parenthesis: t(8),
    });
    let invocation = ast.add(MethodInvocation {
        target: None,
        operator: None,
        method_name: f,
        type_arguments: None,
        argument_list,
    });
    let statement = ast.add(ExpressionStatement {
        expression: invocation.upcast(),
        semicolon: Some(t(9)),
    });
    let statements = ast.new_list([statement.upcast::<Statement>()]);
    let block = ast.add(Block {
        left_bracket: t(4),
        statements,
        right_bracket: t(10),
    });
    let body = ast.add(BlockFunctionBody {
        keyword: None,
        star: None,
        block,
    });
    let parameters = ast.add(FormalParameterList {
        left_parenthesis: t(2),
        parameters: NodeList::EMPTY,
        left_delimiter: None,
        right_delimiter: None,
        right_parenthesis: t(3),
    });
    let function_expression = ast.add(FunctionExpression {
        type_parameters: None,
        parameters: Some(parameters),
        body: body.upcast(),
    });
    let return_type = ast.add(NamedType {
        import_prefix: None,
        name: t(0),
        type_arguments: None,
        question: None,
    });
    let main = ast.add(FunctionDeclaration {
        documentation_comment: None,
        metadata: NodeList::EMPTY,
        augment_keyword: None,
        external_keyword: None,
        return_type: Some(return_type.upcast()),
        property_keyword: None,
        name: t(1),
        function_expression,
    });
    let declarations = ast.new_list([main.upcast::<CompilationUnitMember>()]);
    let unit = ast.add(CompilationUnit {
        begin_token: first,
        script_tag: None,
        directives: NodeList::EMPTY,
        declarations,
        end_token: t(11),
    });
    Unit {
        ast,
        unit,
        main,
        statement,
        invocation,
        f,
        x,
    }
}

#[test]
fn built_unit_has_dart_offsets_parents_and_source() {
    let u = build();
    let ast = &u.ast;
    assert_eq!(to_source::to_source(ast, u.unit), "void main() {f(x);}");
    // Dart: offset = beginToken.offset, end = endToken.end; the unit starts at 0.
    assert_eq!((ast.offset(u.invocation), ast.end(u.invocation)), (13, 17));
    assert_eq!((ast.offset(u.main), ast.end(u.main)), (0, 19));
    assert_eq!((ast.offset(u.unit), ast.end(u.unit)), (0, 19));
    assert_eq!(ast.parent(u.f), Some(u.invocation.raw()));
    assert_eq!(
        ast.parent(u.statement).map(|p| ast.kind(p)),
        Some(NodeKind::Block)
    );
    assert_eq!(ast.root(u.x), u.unit.raw());
    assert!(ast.is::<Expression>(u.invocation));
    assert!(ast.is::<InvocationExpression>(u.invocation));
    assert!(!ast.is::<Statement>(u.invocation));
    assert!(ast.is_in_value_expression_slot(u.statement, u.invocation));
    assert_eq!(
        ast.this_or_ancestor_of_type::<FunctionDeclaration>(u.x),
        Some(u.main)
    );
    assert_eq!(
        dump::node_json(ast, u.invocation),
        concat!(
            r#"{"t":"MethodInvocation","o":13,"e":17,"c":["#,
            r#"{"t":"SimpleIdentifier","o":13,"e":14,"c":[{"k":"IDENTIFIER","o":13,"l":1,"x":"f"}]},"#,
            r#"{"t":"ArgumentList","o":14,"e":17,"c":[{"k":"OPEN_PAREN","o":14,"l":1,"x":"("},"#,
            r#"{"t":"SimpleIdentifier","o":15,"e":16,"c":[{"k":"IDENTIFIER","o":15,"l":1,"x":"x"}]},"#,
            r#"{"k":"CLOSE_PAREN","o":16,"l":1,"x":")"}]}]}"#
        )
    );
}

#[test]
fn replace_method_invocation_with_function_expression_invocation() {
    let mut u = build();
    let ast = &mut u.ast;
    let argument_list = ast[u.invocation].argument_list;
    // Dart `AstRewriter`: the new node takes the children of the old node.
    let fei = ast.add(FunctionExpressionInvocation {
        function: u.f.upcast(),
        type_arguments: None,
        argument_list,
    });
    ast.replace_with(u.invocation, fei);

    assert_eq!(ast[u.statement].expression.raw(), fei.raw());
    assert_eq!(ast.parent(fei), Some(u.statement.raw()));
    assert_eq!(ast.parent(u.f), Some(fei.raw()));
    assert_eq!(ast.parent(argument_list), Some(fei.raw()));
    assert_eq!(to_source::to_source(ast, u.unit), "void main() {f(x);}");
    assert!(dump::node_json(ast, u.statement).starts_with(r#"{"t":"ExpressionStatement","o":13,"e":18,"c":[{"t":"FunctionExpressionInvocation","o":13,"e":17,"#));
}

#[test]
fn replace_list_element_and_remove_children() {
    let mut u = build();
    let ast = &mut u.ast;
    let argument_list = ast[u.invocation].argument_list;
    let token = ast[u.f].token;
    let y = ast.add(SimpleIdentifier { token });
    ast.replace_child(argument_list, u.x, y);
    assert_eq!(ast.list(ast[argument_list].arguments)[0].raw(), y.raw());
    assert_eq!(ast.parent(y), Some(argument_list.raw()));
    assert_eq!(to_source::to_source(ast, u.unit), "void main() {f(f);}");

    // A nullable slot can be removed, a required one and a list element not.
    let return_type = ast[u.main].return_type.unwrap();
    ast.remove_from_parent(return_type).unwrap();
    assert_eq!(to_source::to_source(ast, u.unit), "main() {f(f);}");
    assert_eq!(ast.offset(u.main), 5);
    let fe = ast[u.main].function_expression;
    assert_eq!(
        ast.remove_child(u.main, fe).unwrap_err(),
        "Cannot remove required child 'functionExpression'."
    );
    assert_eq!(
        ast.remove_child(argument_list, y).unwrap_err(),
        "Cannot remove child 'arguments' because NodeList cannot be resized."
    );
}

#[test]
#[should_panic(expected = "is not a Expression")]
fn replace_child_checks_the_slot_type() {
    let mut u = build();
    let ast = &mut u.ast;
    let block = ast.parent(u.statement).unwrap();
    // Dart: `expression = newNode as ExpressionImpl` throws.
    ast.replace_child(u.statement, u.invocation, block);
}

#[derive(Default)]
struct Calls(Vec<String>);

impl GeneralizingAstVisitor for Calls {
    fn visit_node(&mut self, ast: &Ast, node: NodeId) {
        self.0.push(format!("node:{}", ast.kind(node).name()));
        ast.visit_children_generalizing(node, self);
    }

    fn visit_expression(&mut self, ast: &Ast, node: Id<Expression>) {
        self.0.push("expression".into());
        self.visit_node(ast, node.raw());
    }

    fn visit_invocation_expression(&mut self, ast: &Ast, node: Id<InvocationExpression>) {
        self.0.push("invocation".into());
        self.visit_expression(ast, node.upcast());
    }

    fn visit_compilation_unit_member(&mut self, ast: &Ast, node: Id<CompilationUnitMember>) {
        self.0.push("member".into());
        self.visit_declaration(ast, node.upcast());
    }
}

#[test]
fn generalizing_visitor_walks_the_class_hierarchy() {
    let u = build();
    let mut calls = Calls::default();
    u.ast.accept_generalizing(u.unit, &mut calls);
    assert_eq!(
        calls.0,
        [
            "node:CompilationUnit",
            "member",
            "node:FunctionDeclaration",
            "node:NamedType",
            "expression",
            "node:FunctionExpression",
            "node:FormalParameterList",
            "node:BlockFunctionBody",
            "node:Block",
            "node:ExpressionStatement",
            "invocation",
            "expression",
            "node:MethodInvocation",
            "expression",
            "node:SimpleIdentifier",
            "node:ArgumentList",
            "expression",
            "node:SimpleIdentifier",
        ]
    );
}

/// A resolver-like visitor that rewrites while it walks.
struct Rewriter;

impl AstVisitorMut for Rewriter {
    fn visit_method_invocation(&mut self, ast: &mut Ast, node: Id<MethodInvocation>) {
        let n = ast[node].clone();
        if n.target.is_none() {
            let fei = ast.add(FunctionExpressionInvocation {
                function: n.method_name.upcast(),
                type_arguments: n.type_arguments,
                argument_list: n.argument_list,
            });
            ast.replace_with(node, fei);
            ast.visit_children_mut(fei, self);
        } else {
            ast.visit_children_mut(node, self);
        }
    }
}

#[test]
fn mutable_visitor_rewrites_during_the_walk() {
    let mut u = build();
    u.ast.accept_mut(u.unit, &mut Rewriter);
    let e = u.ast[u.statement].expression;
    assert_eq!(u.ast.kind(e), NodeKind::FunctionExpressionInvocation);
    assert_eq!(u.ast.parent(u.f), Some(e.raw()));
    assert_eq!(to_source::to_source(&u.ast, u.unit), "void main() {f(x);}");
}

#[test]
fn side_table_holds_resolution_data() {
    let u = build();
    let mut static_types: NodeMap<&str> = NodeMap::new();
    static_types.insert(u.x, "int");
    assert_eq!(static_types.get(u.x), Some(&"int"));
    assert_eq!(static_types.get(u.f), None);
}
