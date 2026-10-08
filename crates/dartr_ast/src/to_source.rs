// Dart source: pkg/analyzer/lib/src/dart/ast/to_source_visitor.dart

//! `toSource()`: a source representation of a node (Dart
//! `ToSourceVisitor`). Tokens are written with their lexemes, with single
//! spaces between them, without comments.

use dartr_syntax::TokenId;

use crate::arena::{Ast, Id, NodeId, NodeList};
use crate::generated::nodes::*;
use crate::generated::visitor::AstVisitor;

/// Dart `AstNode.toSource()`.
pub fn to_source(ast: &Ast, id: impl Into<NodeId>) -> String {
    let mut v = ToSourceVisitor::new();
    ast.accept(id, &mut v);
    v.sink
}

/// Dart `ToSourceVisitor`: writes the source representation to [sink].
pub struct ToSourceVisitor {
    pub sink: String,
}

impl ToSourceVisitor {
    pub fn new() -> Self {
        ToSourceVisitor {
            sink: String::new(),
        }
    }

    fn w(&mut self, s: &str) {
        self.sink.push_str(s);
    }

    fn lexeme(&mut self, ast: &Ast, t: TokenId) {
        self.sink.push_str(ast.tokens.lexeme(t));
    }

    /// Dart `_visitNode`.
    fn node<T: ?Sized>(&mut self, ast: &Ast, node: Option<Id<T>>, prefix: &str, suffix: &str) {
        if let Some(n) = node {
            self.w(prefix);
            ast.accept(n, self);
            self.w(suffix);
        }
    }

    fn n<T: ?Sized>(&mut self, ast: &Ast, node: Id<T>) {
        ast.accept(node, self);
    }

    fn opt<T: ?Sized>(&mut self, ast: &Ast, node: Option<Id<T>>) {
        self.node(ast, node, "", "");
    }

    /// Dart `_visitNodeList`.
    fn list<T: ?Sized>(
        &mut self,
        ast: &Ast,
        nodes: NodeList<T>,
        prefix: &str,
        separator: &str,
        suffix: &str,
    ) {
        let items = ast.list_raw(nodes);
        if !items.is_empty() {
            self.w(prefix);
            for (i, &n) in items.iter().enumerate() {
                if i > 0 {
                    self.w(separator);
                }
                ast.accept(n, self);
            }
            self.w(suffix);
        }
    }

    /// Dart `_visitToken`.
    fn token(&mut self, ast: &Ast, token: Option<TokenId>, prefix: &str, suffix: &str) {
        if let Some(t) = token {
            self.w(prefix);
            self.lexeme(ast, t);
            self.w(suffix);
        }
    }

    fn metadata(&mut self, ast: &Ast, metadata: NodeList<Annotation>) {
        self.list(ast, metadata, "", " ", " ");
    }

    /// Dart `_visitFormalParameterHeader`.
    fn formal_parameter_header(
        &mut self,
        ast: &Ast,
        metadata: NodeList<Annotation>,
        required_keyword: Option<TokenId>,
        covariant_keyword: Option<TokenId>,
        const_final_or_var_keyword: Option<TokenId>,
        type_: Option<Id<TypeAnnotation>>,
        has_name_or_suffix: bool,
    ) {
        self.metadata(ast, metadata);
        self.token(ast, required_keyword, "", " ");
        self.token(ast, covariant_keyword, "", " ");
        self.token(ast, const_final_or_var_keyword, "", " ");
        self.opt(ast, type_);
        if type_.is_some() && has_name_or_suffix {
            self.w(" ");
        }
    }

    /// Dart `_visitFunctionBody`.
    fn function_body(&mut self, ast: &Ast, body: Id<FunctionBody>) {
        if ast.kind(body) != NodeKind::EmptyFunctionBody {
            self.w(" ");
        }
        self.n(ast, body);
    }

    /// Dart `_writeOperand`.
    fn operand(&mut self, ast: &Ast, node: NodeId, operand: Id<Expression>) {
        let needs_parenthesis = ast.precedence(operand) < ast.precedence(node);
        if needs_parenthesis {
            self.w("(");
        }
        self.n(ast, operand);
        if needs_parenthesis {
            self.w(")");
        }
    }
}

impl Default for ToSourceVisitor {
    fn default() -> Self {
        Self::new()
    }
}

impl AstVisitor for ToSourceVisitor {
    fn visit_node(&mut self, _ast: &Ast, _node: NodeId) {}

    fn visit_adjacent_strings(&mut self, ast: &Ast, node: Id<AdjacentStrings>) {
        self.list(ast, ast[node].strings, "", " ", "");
    }

    fn visit_annotation(&mut self, ast: &Ast, node: Id<Annotation>) {
        let n = &ast[node];
        self.w("@");
        self.n(ast, n.name);
        self.opt(ast, n.type_arguments);
        self.node(ast, n.constructor_name, ".", "");
        self.opt(ast, n.arguments);
    }

    fn visit_anonymous_block_body(&mut self, ast: &Ast, node: Id<AnonymousBlockBody>) {
        self.n(ast, ast[node].block);
    }

    fn visit_anonymous_expression_body(&mut self, ast: &Ast, node: Id<AnonymousExpressionBody>) {
        let n = &ast[node];
        self.lexeme(ast, n.function_definition);
        self.w(" ");
        self.n(ast, n.expression);
    }

    fn visit_anonymous_method_invocation(
        &mut self,
        ast: &Ast,
        node: Id<AnonymousMethodInvocation>,
    ) {
        let n = &ast[node];
        self.opt(ast, n.target);
        self.token(ast, Some(n.operator), "", "");
        self.opt(ast, n.parameters);
        if n.parameters.is_some() {
            self.w(" ");
        }
        self.n(ast, n.body);
    }

    fn visit_argument_list(&mut self, ast: &Ast, node: Id<ArgumentList>) {
        self.w("(");
        self.list(ast, ast[node].arguments, "", ", ", "");
        self.w(")");
    }

    fn visit_as_expression(&mut self, ast: &Ast, node: Id<AsExpression>) {
        let n = &ast[node];
        self.n(ast, n.expression);
        self.w(" as ");
        self.n(ast, n.type_);
    }

    fn visit_assert_initializer(&mut self, ast: &Ast, node: Id<AssertInitializer>) {
        let n = &ast[node];
        self.w("assert (");
        self.n(ast, n.condition);
        if n.message.is_some() {
            self.w(", ");
            self.opt(ast, n.message);
        }
        self.w(")");
    }

    fn visit_assert_statement(&mut self, ast: &Ast, node: Id<AssertStatement>) {
        let n = &ast[node];
        self.w("assert (");
        self.n(ast, n.condition);
        if n.message.is_some() {
            self.w(", ");
            self.opt(ast, n.message);
        }
        self.w(");");
    }

    fn visit_assigned_variable_pattern(&mut self, ast: &Ast, node: Id<AssignedVariablePattern>) {
        self.lexeme(ast, ast[node].name);
    }

    fn visit_assignment_expression(&mut self, ast: &Ast, node: Id<AssignmentExpression>) {
        let n = &ast[node];
        self.n(ast, n.left_hand_side);
        self.w(" ");
        self.lexeme(ast, n.operator);
        self.w(" ");
        self.n(ast, n.right_hand_side);
    }

    fn visit_await_expression(&mut self, ast: &Ast, node: Id<AwaitExpression>) {
        self.w("await ");
        self.n(ast, ast[node].expression);
    }

    fn visit_binary_expression(&mut self, ast: &Ast, node: Id<BinaryExpression>) {
        let n = &ast[node];
        self.operand(ast, node.raw(), n.left_operand);
        self.w(" ");
        self.lexeme(ast, n.operator);
        self.w(" ");
        self.operand(ast, node.raw(), n.right_operand);
    }

    fn visit_block(&mut self, ast: &Ast, node: Id<Block>) {
        self.w("{");
        self.list(ast, ast[node].statements, "", " ", "");
        self.w("}");
    }

    fn visit_block_class_body(&mut self, ast: &Ast, node: Id<BlockClassBody>) {
        self.w(" {");
        self.list(ast, ast[node].members, "", " ", "");
        self.w("}");
    }

    fn visit_block_enum_body(&mut self, ast: &Ast, node: Id<BlockEnumBody>) {
        let n = &ast[node];
        self.w(" {");
        self.list(ast, n.constants, "", ", ", "");
        self.token(ast, n.semicolon, "", "");
        self.list(ast, n.members, " ", " ", "");
        self.w("}");
    }

    fn visit_block_function_body(&mut self, ast: &Ast, node: Id<BlockFunctionBody>) {
        let n = &ast[node];
        if let Some(keyword) = n.keyword {
            self.lexeme(ast, keyword);
            if n.star.is_some() {
                self.w("*");
            }
            self.w(" ");
        }
        self.n(ast, n.block);
    }

    fn visit_boolean_literal(&mut self, ast: &Ast, node: Id<BooleanLiteral>) {
        self.lexeme(ast, ast[node].literal);
    }

    fn visit_break_statement(&mut self, ast: &Ast, node: Id<BreakStatement>) {
        self.w("break");
        self.node(ast, ast[node].label, " ", "");
        self.w(";");
    }

    fn visit_cascade_expression(&mut self, ast: &Ast, node: Id<CascadeExpression>) {
        let n = &ast[node];
        self.n(ast, n.target);
        self.list(ast, n.cascade_sections, "", "", "");
    }

    fn visit_case_clause(&mut self, ast: &Ast, node: Id<CaseClause>) {
        self.w("case ");
        self.n(ast, ast[node].guarded_pattern);
    }

    fn visit_cast_pattern(&mut self, ast: &Ast, node: Id<CastPattern>) {
        let n = &ast[node];
        self.n(ast, n.pattern);
        self.w(" as ");
        self.n(ast, n.type_);
    }

    fn visit_catch_clause(&mut self, ast: &Ast, node: Id<CatchClause>) {
        let n = &ast[node];
        self.node(ast, n.exception_type, "on ", "");
        if n.catch_keyword.is_some() {
            if n.exception_type.is_some() {
                self.w(" ");
            }
            self.w("catch (");
            self.opt(ast, n.exception_parameter);
            self.node(ast, n.stack_trace_parameter, ", ", "");
            self.w(") ");
        } else {
            self.w(" ");
        }
        self.n(ast, n.body);
    }

    fn visit_catch_clause_parameter(&mut self, ast: &Ast, node: Id<CatchClauseParameter>) {
        self.lexeme(ast, ast[node].name);
    }

    fn visit_class_declaration(&mut self, ast: &Ast, node: Id<ClassDeclaration>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.token(ast, n.augment_keyword, "", " ");
        self.token(ast, n.abstract_keyword, "", " ");
        self.token(ast, n.sealed_keyword, "", " ");
        self.token(ast, n.base_keyword, "", " ");
        self.token(ast, n.interface_keyword, "", " ");
        self.token(ast, n.final_keyword, "", " ");
        self.token(ast, n.mixin_keyword, "", " ");
        self.w("class ");
        self.n(ast, n.name_part);
        self.node(ast, n.extends_clause, " ", "");
        self.node(ast, n.with_clause, " ", "");
        self.node(ast, n.implements_clause, " ", "");
        self.n(ast, n.body);
    }

    fn visit_class_type_alias(&mut self, ast: &Ast, node: Id<ClassTypeAlias>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.token(ast, n.augment_keyword, "", " ");
        self.token(ast, n.abstract_keyword, "", " ");
        self.token(ast, n.sealed_keyword, "", " ");
        self.token(ast, n.base_keyword, "", " ");
        self.token(ast, n.interface_keyword, "", " ");
        self.token(ast, n.final_keyword, "", " ");
        self.token(ast, n.mixin_keyword, "", " ");
        self.w("class ");
        self.lexeme(ast, n.name);
        self.opt(ast, n.type_parameters);
        self.w(" = ");
        self.n(ast, n.superclass);
        self.node(ast, Some(n.with_clause), " ", "");
        self.node(ast, n.implements_clause, " ", "");
        self.w(";");
    }

    fn visit_comment(&mut self, _ast: &Ast, _node: Id<Comment>) {}

    fn visit_comment_reference(&mut self, ast: &Ast, node: Id<CommentReference>) {
        let n = &ast[node];
        self.token(ast, n.new_keyword, "", "");
        self.node(ast, Some(n.expression), "[", "]");
    }

    fn visit_compilation_unit(&mut self, ast: &Ast, node: Id<CompilationUnit>) {
        let n = &ast[node];
        let script_tag = n.script_tag;
        self.opt(ast, script_tag);
        let prefix = if script_tag.is_none() { "" } else { " " };
        self.list(ast, n.directives, prefix, " ", "");
        let prefix = if script_tag.is_none() && n.directives.is_empty() {
            ""
        } else {
            " "
        };
        self.list(ast, n.declarations, prefix, " ", "");
    }

    fn visit_conditional_expression(&mut self, ast: &Ast, node: Id<ConditionalExpression>) {
        let n = &ast[node];
        self.n(ast, n.condition);
        self.w(" ? ");
        self.n(ast, n.then_expression);
        self.w(" : ");
        self.n(ast, n.else_expression);
    }

    fn visit_configuration(&mut self, ast: &Ast, node: Id<Configuration>) {
        let n = &ast[node];
        self.w("if (");
        self.n(ast, n.name);
        self.node(ast, n.value, " == ", "");
        self.w(") ");
        self.n(ast, n.uri);
    }

    fn visit_constant_pattern(&mut self, ast: &Ast, node: Id<ConstantPattern>) {
        let n = &ast[node];
        self.token(ast, n.const_keyword, "", " ");
        self.n(ast, n.expression);
    }

    fn visit_constructor_declaration(&mut self, ast: &Ast, node: Id<ConstructorDeclaration>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.token(ast, n.augment_keyword, "", " ");
        self.token(ast, n.external_keyword, "", " ");
        self.token(ast, n.const_keyword, "", " ");
        self.token(ast, n.factory_keyword, "", " ");
        self.token(ast, n.new_keyword, "", " ");
        if n.type_name.is_some() {
            self.opt(ast, n.type_name);
            self.token(ast, n.name, ".", "");
        } else {
            self.token(ast, n.name, "", "");
        }
        self.n(ast, n.parameters);
        self.list(ast, n.initializers, " : ", ", ", "");
        self.node(ast, n.redirected_constructor, " = ", "");
        self.function_body(ast, n.body);
    }

    fn visit_constructor_field_initializer(
        &mut self,
        ast: &Ast,
        node: Id<ConstructorFieldInitializer>,
    ) {
        let n = &ast[node];
        self.token(ast, n.this_keyword, "", ".");
        self.n(ast, n.field_name);
        self.w(" = ");
        self.n(ast, n.expression);
    }

    fn visit_constructor_name(&mut self, ast: &Ast, node: Id<ConstructorName>) {
        let n = &ast[node];
        self.n(ast, n.type_);
        self.node(ast, n.name, ".", "");
    }

    fn visit_constructor_reference(&mut self, ast: &Ast, node: Id<ConstructorReference>) {
        self.n(ast, ast[node].constructor_name);
    }

    fn visit_constructor_selector(&mut self, ast: &Ast, node: Id<ConstructorSelector>) {
        let n = &ast[node];
        self.lexeme(ast, n.period);
        self.n(ast, n.name);
    }

    fn visit_continue_statement(&mut self, ast: &Ast, node: Id<ContinueStatement>) {
        self.w("continue");
        self.node(ast, ast[node].label, " ", "");
        self.w(";");
    }

    fn visit_declared_identifier(&mut self, ast: &Ast, node: Id<DeclaredIdentifier>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.token(ast, n.keyword, "", " ");
        self.node(ast, n.type_, "", " ");
        self.lexeme(ast, n.name);
    }

    fn visit_declared_variable_pattern(&mut self, ast: &Ast, node: Id<DeclaredVariablePattern>) {
        let n = &ast[node];
        self.token(ast, n.keyword, "", " ");
        self.node(ast, n.type_, "", " ");
        self.lexeme(ast, n.name);
    }

    fn visit_do_statement(&mut self, ast: &Ast, node: Id<DoStatement>) {
        let n = &ast[node];
        self.w("do ");
        self.n(ast, n.body);
        self.w(" while (");
        self.n(ast, n.condition);
        self.w(");");
    }

    fn visit_dot_shorthand_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<DotShorthandConstructorInvocation>,
    ) {
        let n = &ast[node];
        self.token(ast, n.const_keyword, "", " ");
        self.lexeme(ast, n.period);
        self.n(ast, n.constructor_name);
        self.opt(ast, n.type_arguments);
        self.n(ast, n.argument_list);
    }

    fn visit_dot_shorthand_invocation(&mut self, ast: &Ast, node: Id<DotShorthandInvocation>) {
        let n = &ast[node];
        self.lexeme(ast, n.period);
        self.n(ast, n.member_name);
        self.opt(ast, n.type_arguments);
        self.n(ast, n.argument_list);
    }

    fn visit_dot_shorthand_property_access(
        &mut self,
        ast: &Ast,
        node: Id<DotShorthandPropertyAccess>,
    ) {
        let n = &ast[node];
        self.lexeme(ast, n.period);
        self.n(ast, n.property_name);
    }

    fn visit_dotted_name(&mut self, ast: &Ast, node: Id<DottedName>) {
        for &t in ast.token_list(ast[node].tokens) {
            self.lexeme(ast, t);
        }
    }

    fn visit_double_literal(&mut self, ast: &Ast, node: Id<DoubleLiteral>) {
        self.lexeme(ast, ast[node].literal);
    }

    fn visit_empty_class_body(&mut self, _ast: &Ast, _node: Id<EmptyClassBody>) {
        self.w(";");
    }

    fn visit_empty_enum_body(&mut self, _ast: &Ast, _node: Id<EmptyEnumBody>) {
        self.w(";");
    }

    fn visit_empty_function_body(&mut self, _ast: &Ast, _node: Id<EmptyFunctionBody>) {
        self.w(";");
    }

    fn visit_empty_statement(&mut self, _ast: &Ast, _node: Id<EmptyStatement>) {
        self.w(";");
    }

    fn visit_enum_constant_arguments(&mut self, ast: &Ast, node: Id<EnumConstantArguments>) {
        let n = &ast[node];
        self.opt(ast, n.type_arguments);
        self.opt(ast, n.constructor_selector);
        self.n(ast, n.argument_list);
    }

    fn visit_enum_constant_declaration(&mut self, ast: &Ast, node: Id<EnumConstantDeclaration>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.token(ast, n.augment_keyword, "", " ");
        self.lexeme(ast, n.name);
        self.opt(ast, n.arguments);
    }

    fn visit_enum_declaration(&mut self, ast: &Ast, node: Id<EnumDeclaration>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.token(ast, n.augment_keyword, "", " ");
        self.w("enum ");
        self.n(ast, n.name_part);
        self.node(ast, n.with_clause, " ", "");
        self.node(ast, n.implements_clause, " ", "");
        self.n(ast, n.body);
    }

    fn visit_export_directive(&mut self, ast: &Ast, node: Id<ExportDirective>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.w("export ");
        self.n(ast, n.uri);
        self.list(ast, n.configurations, " ", " ", "");
        self.list(ast, n.combinators, " ", " ", "");
        self.w(";");
    }

    fn visit_expression_function_body(&mut self, ast: &Ast, node: Id<ExpressionFunctionBody>) {
        let n = &ast[node];
        if let Some(keyword) = n.keyword {
            self.lexeme(ast, keyword);
            if n.star.is_some() {
                self.w("*");
            }
            self.w(" ");
        }
        self.lexeme(ast, n.function_definition);
        self.w(" ");
        self.n(ast, n.expression);
        if n.semicolon.is_some() {
            self.w(";");
        }
    }

    fn visit_expression_statement(&mut self, ast: &Ast, node: Id<ExpressionStatement>) {
        self.n(ast, ast[node].expression);
        self.w(";");
    }

    fn visit_extends_clause(&mut self, ast: &Ast, node: Id<ExtendsClause>) {
        self.w("extends ");
        self.n(ast, ast[node].superclass);
    }

    fn visit_extension_declaration(&mut self, ast: &Ast, node: Id<ExtensionDeclaration>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.token(ast, n.augment_keyword, "", " ");
        self.token(ast, Some(n.extension_keyword), "", " ");
        self.token(ast, n.type_keyword, "", " ");
        self.token(ast, n.name, "", "");
        self.opt(ast, n.type_parameters);
        self.node(ast, n.on_clause, " ", "");
        self.n(ast, n.body);
    }

    fn visit_extension_on_clause(&mut self, ast: &Ast, node: Id<ExtensionOnClause>) {
        self.w("on ");
        self.n(ast, ast[node].extended_type);
    }

    fn visit_extension_override(&mut self, ast: &Ast, node: Id<ExtensionOverride>) {
        let n = &ast[node];
        self.opt(ast, n.import_prefix);
        self.lexeme(ast, n.name);
        self.opt(ast, n.type_arguments);
        self.n(ast, n.argument_list);
    }

    fn visit_extension_type_declaration(&mut self, ast: &Ast, node: Id<ExtensionTypeDeclaration>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.token(ast, n.augment_keyword, "", " ");
        self.token(ast, Some(n.extension_keyword), "", " ");
        self.token(ast, Some(n.type_keyword), "", " ");
        self.n(ast, n.name_part);
        self.node(ast, n.implements_clause, " ", "");
        self.n(ast, n.body);
    }

    fn visit_field_declaration(&mut self, ast: &Ast, node: Id<FieldDeclaration>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.token(ast, n.augment_keyword, "", " ");
        self.token(ast, n.external_keyword, "", " ");
        self.token(ast, n.static_keyword, "", " ");
        self.token(ast, n.abstract_keyword, "", " ");
        self.token(ast, n.covariant_keyword, "", " ");
        self.n(ast, n.fields);
        self.w(";");
    }

    fn visit_field_formal_parameter(&mut self, ast: &Ast, node: Id<FieldFormalParameter>) {
        let n = &ast[node];
        self.formal_parameter_header(
            ast,
            n.metadata,
            n.required_keyword,
            n.covariant_keyword,
            n.const_final_or_var_keyword,
            n.type_,
            true,
        );
        self.w("this.");
        self.lexeme(ast, n.name);
        self.opt(ast, n.function_typed_suffix);
        self.opt(ast, n.default_clause);
    }

    fn visit_for_each_parts_with_declaration(
        &mut self,
        ast: &Ast,
        node: Id<ForEachPartsWithDeclaration>,
    ) {
        let n = &ast[node];
        self.n(ast, n.loop_variable);
        self.w(" in ");
        self.n(ast, n.iterable);
    }

    fn visit_for_each_parts_with_identifier(
        &mut self,
        ast: &Ast,
        node: Id<ForEachPartsWithIdentifier>,
    ) {
        let n = &ast[node];
        self.n(ast, n.identifier);
        self.w(" in ");
        self.n(ast, n.iterable);
    }

    fn visit_for_each_parts_with_pattern(&mut self, ast: &Ast, node: Id<ForEachPartsWithPattern>) {
        let n = &ast[node];
        self.list(ast, n.metadata, "", " ", " ");
        self.token(ast, Some(n.keyword), "", " ");
        self.n(ast, n.pattern);
        self.w(" in ");
        self.n(ast, n.iterable);
    }

    fn visit_for_element(&mut self, ast: &Ast, node: Id<ForElement>) {
        let n = &ast[node];
        self.token(ast, n.await_keyword, "", " ");
        self.w("for (");
        self.n(ast, n.for_loop_parts);
        self.w(") ");
        self.n(ast, n.body);
    }

    fn visit_formal_parameter_default_clause(
        &mut self,
        ast: &Ast,
        node: Id<FormalParameterDefaultClause>,
    ) {
        let n = &ast[node];
        if ast.tokens.lexeme(n.separator) != ":" {
            self.w(" ");
        }
        self.lexeme(ast, n.separator);
        self.node(ast, Some(n.value), " ", "");
    }

    fn visit_formal_parameter_list(&mut self, ast: &Ast, node: Id<FormalParameterList>) {
        let n = &ast[node];
        let mut group_end: Option<TokenId> = None;
        self.w("(");
        for (i, &parameter) in ast.list_raw(n.parameters).iter().enumerate() {
            if i > 0 {
                self.w(", ");
            }
            if let (None, Some(left_delimiter)) = (group_end, n.left_delimiter) {
                if !is_required_positional(ast, parameter) {
                    group_end = Some(n.right_delimiter.expect("rightDelimiter"));
                    self.lexeme(ast, left_delimiter);
                }
            }
            ast.accept(parameter, self);
        }
        if let Some(g) = group_end {
            self.lexeme(ast, g);
        }
        self.w(")");
    }

    fn visit_for_parts_with_declarations(&mut self, ast: &Ast, node: Id<ForPartsWithDeclarations>) {
        let n = &ast[node];
        self.n(ast, n.variables);
        self.w(";");
        self.node(ast, n.condition, " ", "");
        self.w(";");
        self.list(ast, n.updaters, " ", ", ", "");
    }

    fn visit_for_parts_with_expression(&mut self, ast: &Ast, node: Id<ForPartsWithExpression>) {
        let n = &ast[node];
        self.opt(ast, n.initialization);
        self.w(";");
        self.node(ast, n.condition, " ", "");
        self.w(";");
        self.list(ast, n.updaters, " ", ", ", "");
    }

    fn visit_for_parts_with_pattern(&mut self, ast: &Ast, node: Id<ForPartsWithPattern>) {
        let n = &ast[node];
        self.n(ast, n.variables);
        self.w("; ");
        self.opt(ast, n.condition);
        self.w("; ");
        self.list(ast, n.updaters, "", ", ", "");
    }

    fn visit_for_statement(&mut self, ast: &Ast, node: Id<ForStatement>) {
        let n = &ast[node];
        if n.await_keyword.is_some() {
            self.w("await ");
        }
        self.w("for (");
        self.n(ast, n.for_loop_parts);
        self.w(") ");
        self.n(ast, n.body);
    }

    fn visit_function_declaration(&mut self, ast: &Ast, node: Id<FunctionDeclaration>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.token(ast, n.augment_keyword, "", " ");
        self.token(ast, n.external_keyword, "", " ");
        self.node(ast, n.return_type, "", " ");
        self.token(ast, n.property_keyword, "", " ");
        self.lexeme(ast, n.name);
        self.n(ast, n.function_expression);
    }

    fn visit_function_declaration_statement(
        &mut self,
        ast: &Ast,
        node: Id<FunctionDeclarationStatement>,
    ) {
        self.n(ast, ast[node].function_declaration);
    }

    fn visit_function_expression(&mut self, ast: &Ast, node: Id<FunctionExpression>) {
        let n = &ast[node];
        self.opt(ast, n.type_parameters);
        self.opt(ast, n.parameters);
        self.function_body(ast, n.body);
    }

    fn visit_function_expression_invocation(
        &mut self,
        ast: &Ast,
        node: Id<FunctionExpressionInvocation>,
    ) {
        let n = &ast[node];
        self.n(ast, n.function);
        self.opt(ast, n.type_arguments);
        self.n(ast, n.argument_list);
    }

    fn visit_function_reference(&mut self, ast: &Ast, node: Id<FunctionReference>) {
        let n = &ast[node];
        self.n(ast, n.function);
        self.opt(ast, n.type_arguments);
    }

    fn visit_function_type_alias(&mut self, ast: &Ast, node: Id<FunctionTypeAlias>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.token(ast, n.augment_keyword, "", " ");
        self.w("typedef ");
        self.node(ast, n.return_type, "", " ");
        self.lexeme(ast, n.name);
        self.opt(ast, n.type_parameters);
        self.n(ast, n.parameters);
        self.w(";");
    }

    fn visit_function_typed_formal_parameter_suffix(
        &mut self,
        ast: &Ast,
        node: Id<FunctionTypedFormalParameterSuffix>,
    ) {
        let n = &ast[node];
        self.opt(ast, n.type_parameters);
        self.n(ast, n.formal_parameters);
        if n.question.is_some() {
            self.w("?");
        }
    }

    fn visit_generic_function_type(&mut self, ast: &Ast, node: Id<GenericFunctionType>) {
        let n = &ast[node];
        self.opt(ast, n.return_type);
        self.w(" Function");
        self.opt(ast, n.type_parameters);
        self.n(ast, n.parameters);
        if n.question.is_some() {
            self.w("?");
        }
    }

    fn visit_generic_type_alias(&mut self, ast: &Ast, node: Id<GenericTypeAlias>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.token(ast, n.augment_keyword, "", " ");
        self.w("typedef ");
        self.lexeme(ast, n.name);
        self.opt(ast, n.type_parameters);
        self.w(" = ");
        self.n(ast, n.type_);
        self.w(";");
    }

    fn visit_guarded_pattern(&mut self, ast: &Ast, node: Id<GuardedPattern>) {
        let n = &ast[node];
        self.n(ast, n.pattern);
        self.node(ast, n.when_clause, " ", "");
    }

    fn visit_hide_combinator(&mut self, ast: &Ast, node: Id<HideCombinator>) {
        self.w("hide ");
        self.list(ast, ast[node].hidden_names, "", ", ", "");
    }

    fn visit_if_element(&mut self, ast: &Ast, node: Id<IfElement>) {
        let n = &ast[node];
        self.w("if (");
        self.n(ast, n.expression);
        self.node(ast, n.case_clause, " ", "");
        self.w(") ");
        self.n(ast, n.then_element);
        self.node(ast, n.else_element, " else ", "");
    }

    fn visit_if_statement(&mut self, ast: &Ast, node: Id<IfStatement>) {
        let n = &ast[node];
        self.w("if (");
        self.n(ast, n.expression);
        self.node(ast, n.case_clause, " ", "");
        self.w(") ");
        self.n(ast, n.then_statement);
        self.node(ast, n.else_statement, " else ", "");
    }

    fn visit_implements_clause(&mut self, ast: &Ast, node: Id<ImplementsClause>) {
        self.w("implements ");
        self.list(ast, ast[node].interfaces, "", ", ", "");
    }

    fn visit_implicit_call_reference(&mut self, ast: &Ast, node: Id<ImplicitCallReference>) {
        let n = &ast[node];
        self.n(ast, n.expression);
        self.opt(ast, n.type_arguments);
    }

    fn visit_import_directive(&mut self, ast: &Ast, node: Id<ImportDirective>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.w("import ");
        self.n(ast, n.uri);
        self.list(ast, n.configurations, " ", " ", "");
        if n.deferred_keyword.is_some() {
            self.w(" deferred");
        }
        self.node(ast, n.prefix, " as ", "");
        self.list(ast, n.combinators, " ", " ", "");
        self.w(";");
    }

    fn visit_import_prefix_reference(&mut self, ast: &Ast, node: Id<ImportPrefixReference>) {
        self.lexeme(ast, ast[node].name);
        self.w(".");
    }

    fn visit_index_expression(&mut self, ast: &Ast, node: Id<IndexExpression>) {
        let n = &ast[node];
        // Dart `isCascaded`: `period != null`.
        if n.period.is_some() {
            self.token(ast, n.period, "", "");
        } else {
            self.opt(ast, n.target);
        }
        self.token(ast, n.question, "", "");
        self.lexeme(ast, n.left_bracket);
        self.n(ast, n.index);
        self.lexeme(ast, n.right_bracket);
    }

    fn visit_instance_creation_expression(
        &mut self,
        ast: &Ast,
        node: Id<InstanceCreationExpression>,
    ) {
        let n = &ast[node];
        self.token(ast, n.keyword, "", " ");
        self.n(ast, n.constructor_name);
        self.n(ast, n.argument_list);
    }

    fn visit_integer_literal(&mut self, ast: &Ast, node: Id<IntegerLiteral>) {
        self.lexeme(ast, ast[node].literal);
    }

    fn visit_interpolation_expression(&mut self, ast: &Ast, node: Id<InterpolationExpression>) {
        let n = &ast[node];
        if n.right_bracket.is_some() {
            self.w("${");
            self.n(ast, n.expression);
            self.w("}");
        } else {
            self.w("$");
            self.n(ast, n.expression);
        }
    }

    fn visit_interpolation_string(&mut self, ast: &Ast, node: Id<InterpolationString>) {
        self.lexeme(ast, ast[node].contents);
    }

    fn visit_is_expression(&mut self, ast: &Ast, node: Id<IsExpression>) {
        let n = &ast[node];
        self.n(ast, n.expression);
        if n.not_operator.is_none() {
            self.w(" is ");
        } else {
            self.w(" is! ");
        }
        self.n(ast, n.type_);
    }

    fn visit_label(&mut self, ast: &Ast, node: Id<Label>) {
        self.lexeme(ast, ast[node].name);
        self.w(":");
    }

    fn visit_labeled_statement(&mut self, ast: &Ast, node: Id<LabeledStatement>) {
        let n = &ast[node];
        self.list(ast, n.labels, "", " ", " ");
        self.n(ast, n.statement);
    }

    fn visit_label_reference(&mut self, ast: &Ast, node: Id<LabelReference>) {
        self.lexeme(ast, ast[node].name);
    }

    fn visit_library_directive(&mut self, ast: &Ast, node: Id<LibraryDirective>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.w("library ");
        self.opt(ast, n.name);
        self.w(";");
    }

    fn visit_list_literal(&mut self, ast: &Ast, node: Id<ListLiteral>) {
        let n = &ast[node];
        self.token(ast, n.const_keyword, "", " ");
        self.opt(ast, n.type_arguments);
        self.w("[");
        self.list(ast, n.elements, "", ", ", "");
        self.w("]");
    }

    fn visit_list_pattern(&mut self, ast: &Ast, node: Id<ListPattern>) {
        let n = &ast[node];
        self.opt(ast, n.type_arguments);
        self.w("[");
        self.list(ast, n.elements, "", ", ", "");
        self.w("]");
    }

    fn visit_logical_and_pattern(&mut self, ast: &Ast, node: Id<LogicalAndPattern>) {
        let n = &ast[node];
        self.n(ast, n.left_operand);
        self.w(" ");
        self.lexeme(ast, n.operator);
        self.w(" ");
        self.n(ast, n.right_operand);
    }

    fn visit_logical_or_pattern(&mut self, ast: &Ast, node: Id<LogicalOrPattern>) {
        let n = &ast[node];
        self.n(ast, n.left_operand);
        self.w(" ");
        self.lexeme(ast, n.operator);
        self.w(" ");
        self.n(ast, n.right_operand);
    }

    fn visit_map_literal_entry(&mut self, ast: &Ast, node: Id<MapLiteralEntry>) {
        let n = &ast[node];
        self.n(ast, n.key);
        self.w(" : ");
        self.n(ast, n.value);
    }

    fn visit_map_pattern(&mut self, ast: &Ast, node: Id<MapPattern>) {
        let n = &ast[node];
        self.opt(ast, n.type_arguments);
        self.w("{");
        self.list(ast, n.elements, "", ", ", "");
        self.w("}");
    }

    fn visit_map_pattern_entry(&mut self, ast: &Ast, node: Id<MapPatternEntry>) {
        let n = &ast[node];
        self.n(ast, n.key);
        self.w(": ");
        self.n(ast, n.value);
    }

    fn visit_method_declaration(&mut self, ast: &Ast, node: Id<MethodDeclaration>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.token(ast, n.augment_keyword, "", " ");
        self.token(ast, n.external_keyword, "", " ");
        self.token(ast, n.modifier_keyword, "", " ");
        self.node(ast, n.return_type, "", " ");
        self.token(ast, n.property_keyword, "", " ");
        self.token(ast, n.operator_keyword, "", " ");
        self.lexeme(ast, n.name);
        // Dart `isGetter`: `propertyKeyword?.keyword == Keyword.GET`.
        let is_getter = n
            .property_keyword
            .is_some_and(|t| ast.tokens.lexeme(t) == "get");
        if !is_getter {
            self.opt(ast, n.type_parameters);
            self.opt(ast, n.parameters);
        }
        self.function_body(ast, n.body);
    }

    fn visit_method_invocation(&mut self, ast: &Ast, node: Id<MethodInvocation>) {
        let n = &ast[node];
        self.opt(ast, n.target);
        self.token(ast, n.operator, "", "");
        self.n(ast, n.method_name);
        self.opt(ast, n.type_arguments);
        self.n(ast, n.argument_list);
    }

    fn visit_mixin_declaration(&mut self, ast: &Ast, node: Id<MixinDeclaration>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.token(ast, n.augment_keyword, "", " ");
        self.token(ast, n.base_keyword, "", " ");
        self.w("mixin ");
        self.lexeme(ast, n.name);
        self.opt(ast, n.type_parameters);
        self.node(ast, n.on_clause, " ", "");
        self.node(ast, n.implements_clause, " ", "");
        self.n(ast, n.body);
    }

    fn visit_mixin_on_clause(&mut self, ast: &Ast, node: Id<MixinOnClause>) {
        self.w("on ");
        self.list(ast, ast[node].superclass_constraints, "", ", ", "");
    }

    fn visit_named_argument(&mut self, ast: &Ast, node: Id<NamedArgument>) {
        let n = &ast[node];
        self.lexeme(ast, n.name);
        self.lexeme(ast, n.colon);
        self.node(ast, Some(n.argument_expression), " ", "");
    }

    fn visit_named_type(&mut self, ast: &Ast, node: Id<NamedType>) {
        let n = &ast[node];
        self.opt(ast, n.import_prefix);
        self.lexeme(ast, n.name);
        self.opt(ast, n.type_arguments);
        if n.question.is_some() {
            self.w("?");
        }
    }

    fn visit_name_with_type_parameters(&mut self, ast: &Ast, node: Id<NameWithTypeParameters>) {
        let n = &ast[node];
        self.lexeme(ast, n.type_name);
        self.opt(ast, n.type_parameters);
    }

    fn visit_native_clause(&mut self, ast: &Ast, node: Id<NativeClause>) {
        self.w("native ");
        self.opt(ast, ast[node].name);
    }

    fn visit_native_function_body(&mut self, ast: &Ast, node: Id<NativeFunctionBody>) {
        self.w("native ");
        self.opt(ast, ast[node].string_literal);
        self.w(";");
    }

    fn visit_null_assert_pattern(&mut self, ast: &Ast, node: Id<NullAssertPattern>) {
        let n = &ast[node];
        self.n(ast, n.pattern);
        self.lexeme(ast, n.operator);
    }

    fn visit_null_aware_element(&mut self, ast: &Ast, node: Id<NullAwareElement>) {
        let n = &ast[node];
        self.lexeme(ast, n.question);
        self.n(ast, n.value);
    }

    fn visit_null_check_pattern(&mut self, ast: &Ast, node: Id<NullCheckPattern>) {
        let n = &ast[node];
        self.n(ast, n.pattern);
        self.lexeme(ast, n.operator);
    }

    fn visit_null_literal(&mut self, _ast: &Ast, _node: Id<NullLiteral>) {
        self.w("null");
    }

    fn visit_object_pattern(&mut self, ast: &Ast, node: Id<ObjectPattern>) {
        let n = &ast[node];
        self.n(ast, n.type_);
        self.w("(");
        self.list(ast, n.fields, "", ", ", "");
        self.w(")");
    }

    fn visit_parenthesized_expression(&mut self, ast: &Ast, node: Id<ParenthesizedExpression>) {
        self.w("(");
        self.n(ast, ast[node].expression);
        self.w(")");
    }

    fn visit_parenthesized_pattern(&mut self, ast: &Ast, node: Id<ParenthesizedPattern>) {
        self.w("(");
        self.n(ast, ast[node].pattern);
        self.w(")");
    }

    fn visit_part_directive(&mut self, ast: &Ast, node: Id<PartDirective>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.w("part ");
        self.n(ast, n.uri);
        self.w(";");
    }

    fn visit_part_of_directive(&mut self, ast: &Ast, node: Id<PartOfDirective>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.w("part of ");
        self.opt(ast, n.library_name);
        self.opt(ast, n.uri);
        self.w(";");
    }

    fn visit_pattern_assignment(&mut self, ast: &Ast, node: Id<PatternAssignment>) {
        let n = &ast[node];
        self.n(ast, n.pattern);
        self.w(" = ");
        self.n(ast, n.expression);
    }

    fn visit_pattern_field(&mut self, ast: &Ast, node: Id<PatternField>) {
        let n = &ast[node];
        self.node(ast, n.name, "", " ");
        self.n(ast, n.pattern);
    }

    fn visit_pattern_field_name(&mut self, ast: &Ast, node: Id<PatternFieldName>) {
        self.token(ast, ast[node].name, "", "");
        self.w(":");
    }

    fn visit_pattern_variable_declaration(
        &mut self,
        ast: &Ast,
        node: Id<PatternVariableDeclaration>,
    ) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.lexeme(ast, n.keyword);
        self.w(" ");
        self.n(ast, n.pattern);
        self.w(" = ");
        self.n(ast, n.expression);
    }

    fn visit_pattern_variable_declaration_statement(
        &mut self,
        ast: &Ast,
        node: Id<PatternVariableDeclarationStatement>,
    ) {
        self.n(ast, ast[node].declaration);
        self.w(";");
    }

    fn visit_postfix_expression(&mut self, ast: &Ast, node: Id<PostfixExpression>) {
        let n = &ast[node];
        self.operand(ast, node.raw(), n.operand);
        self.lexeme(ast, n.operator);
    }

    fn visit_prefixed_identifier(&mut self, ast: &Ast, node: Id<PrefixedIdentifier>) {
        let n = &ast[node];
        self.n(ast, n.prefix);
        self.w(".");
        self.n(ast, n.identifier);
    }

    fn visit_prefix_expression(&mut self, ast: &Ast, node: Id<PrefixExpression>) {
        let n = &ast[node];
        self.lexeme(ast, n.operator);
        self.operand(ast, node.raw(), n.operand);
    }

    fn visit_primary_constructor_body(&mut self, ast: &Ast, node: Id<PrimaryConstructorBody>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.lexeme(ast, n.this_keyword);
        if !n.initializers.is_empty() {
            self.token(ast, n.colon, " ", " ");
            self.list(ast, n.initializers, "", ", ", "");
        }
        self.function_body(ast, n.body);
    }

    fn visit_primary_constructor_declaration(
        &mut self,
        ast: &Ast,
        node: Id<PrimaryConstructorDeclaration>,
    ) {
        let n = &ast[node];
        self.token(ast, n.const_keyword, "", " ");
        self.lexeme(ast, n.type_name);
        self.opt(ast, n.type_parameters);
        self.opt(ast, n.constructor_name);
        self.n(ast, n.formal_parameters);
    }

    fn visit_primary_constructor_name(&mut self, ast: &Ast, node: Id<PrimaryConstructorName>) {
        let n = &ast[node];
        self.lexeme(ast, n.period);
        self.lexeme(ast, n.name);
    }

    fn visit_property_access(&mut self, ast: &Ast, node: Id<PropertyAccess>) {
        let n = &ast[node];
        // Dart `isCascaded`: the operator is `..` or `?..`.
        let op = ast.tokens.lexeme(n.operator);
        if op == ".." || op == "?.." {
            self.lexeme(ast, n.operator);
        } else {
            self.opt(ast, n.target);
            self.lexeme(ast, n.operator);
        }
        self.n(ast, n.property_name);
    }

    fn visit_record_literal(&mut self, ast: &Ast, node: Id<RecordLiteral>) {
        let n = &ast[node];
        self.lexeme(ast, n.left_parenthesis);
        self.list(ast, n.fields, "", ", ", "");
        self.lexeme(ast, n.right_parenthesis);
    }

    fn visit_record_literal_named_field(&mut self, ast: &Ast, node: Id<RecordLiteralNamedField>) {
        let n = &ast[node];
        self.lexeme(ast, n.name);
        self.lexeme(ast, n.colon);
        self.node(ast, Some(n.field_expression), " ", "");
    }

    fn visit_record_pattern(&mut self, ast: &Ast, node: Id<RecordPattern>) {
        let fields = ast[node].fields;
        self.w("(");
        self.list(ast, fields, "", ", ", "");
        if fields.len() == 1 {
            self.w(",");
        }
        self.w(")");
    }

    fn visit_record_type_annotation(&mut self, ast: &Ast, node: Id<RecordTypeAnnotation>) {
        let n = &ast[node];
        self.w("(");
        if !n.positional_fields.is_empty() {
            self.list(ast, n.positional_fields, "", ", ", "");
            if n.named_fields.is_some() {
                self.w(", ");
            }
        }
        self.opt(ast, n.named_fields);
        self.w(")");
        if n.question.is_some() {
            self.w("?");
        }
    }

    fn visit_record_type_annotation_named_field(
        &mut self,
        ast: &Ast,
        node: Id<RecordTypeAnnotationNamedField>,
    ) {
        let n = &ast[node];
        self.n(ast, n.type_);
        self.w(" ");
        self.lexeme(ast, n.name);
    }

    fn visit_record_type_annotation_named_fields(
        &mut self,
        ast: &Ast,
        node: Id<RecordTypeAnnotationNamedFields>,
    ) {
        self.w("{");
        self.list(ast, ast[node].fields, "", ", ", "");
        self.w("}");
    }

    fn visit_record_type_annotation_positional_field(
        &mut self,
        ast: &Ast,
        node: Id<RecordTypeAnnotationPositionalField>,
    ) {
        let n = &ast[node];
        self.n(ast, n.type_);
        if let Some(name) = n.name {
            self.w(" ");
            self.lexeme(ast, name);
        }
    }

    fn visit_redirecting_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<RedirectingConstructorInvocation>,
    ) {
        let n = &ast[node];
        self.w("this");
        self.node(ast, n.constructor_name, ".", "");
        self.n(ast, n.argument_list);
    }

    fn visit_regular_formal_parameter(&mut self, ast: &Ast, node: Id<RegularFormalParameter>) {
        let n = &ast[node];
        self.formal_parameter_header(
            ast,
            n.metadata,
            n.required_keyword,
            n.covariant_keyword,
            n.const_final_or_var_keyword,
            n.type_,
            n.name.is_some() || n.function_typed_suffix.is_some(),
        );
        self.token(ast, n.name, "", "");
        self.opt(ast, n.function_typed_suffix);
        self.opt(ast, n.default_clause);
    }

    fn visit_relational_pattern(&mut self, ast: &Ast, node: Id<RelationalPattern>) {
        let n = &ast[node];
        self.lexeme(ast, n.operator);
        self.w(" ");
        self.n(ast, n.operand);
    }

    fn visit_rest_pattern_element(&mut self, ast: &Ast, node: Id<RestPatternElement>) {
        let n = &ast[node];
        self.lexeme(ast, n.operator);
        self.opt(ast, n.pattern);
    }

    fn visit_rethrow_expression(&mut self, _ast: &Ast, _node: Id<RethrowExpression>) {
        self.w("rethrow");
    }

    fn visit_return_statement(&mut self, ast: &Ast, node: Id<ReturnStatement>) {
        match ast[node].expression {
            None => self.w("return;"),
            Some(e) => {
                self.w("return ");
                self.n(ast, e);
                self.w(";");
            }
        }
    }

    fn visit_script_tag(&mut self, ast: &Ast, node: Id<ScriptTag>) {
        self.lexeme(ast, ast[node].script_tag);
    }

    fn visit_set_or_map_literal(&mut self, ast: &Ast, node: Id<SetOrMapLiteral>) {
        let n = &ast[node];
        self.token(ast, n.const_keyword, "", " ");
        self.opt(ast, n.type_arguments);
        self.w("{");
        self.list(ast, n.elements, "", ", ", "");
        self.w("}");
    }

    fn visit_show_combinator(&mut self, ast: &Ast, node: Id<ShowCombinator>) {
        self.w("show ");
        self.list(ast, ast[node].shown_names, "", ", ", "");
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        self.lexeme(ast, ast[node].token);
    }

    fn visit_simple_string_literal(&mut self, ast: &Ast, node: Id<SimpleStringLiteral>) {
        self.lexeme(ast, ast[node].literal);
    }

    fn visit_spread_element(&mut self, ast: &Ast, node: Id<SpreadElement>) {
        let n = &ast[node];
        self.lexeme(ast, n.spread_operator);
        self.n(ast, n.expression);
    }

    fn visit_string_interpolation(&mut self, ast: &Ast, node: Id<StringInterpolation>) {
        self.list(ast, ast[node].elements, "", "", "");
    }

    fn visit_super_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<SuperConstructorInvocation>,
    ) {
        let n = &ast[node];
        self.w("super");
        self.node(ast, n.constructor_name, ".", "");
        self.n(ast, n.argument_list);
    }

    fn visit_super_expression(&mut self, _ast: &Ast, _node: Id<SuperExpression>) {
        self.w("super");
    }

    fn visit_super_formal_parameter(&mut self, ast: &Ast, node: Id<SuperFormalParameter>) {
        let n = &ast[node];
        self.formal_parameter_header(
            ast,
            n.metadata,
            n.required_keyword,
            n.covariant_keyword,
            n.const_final_or_var_keyword,
            n.type_,
            true,
        );
        self.w("super.");
        self.lexeme(ast, n.name);
        self.opt(ast, n.function_typed_suffix);
        self.opt(ast, n.default_clause);
    }

    fn visit_switch_case(&mut self, ast: &Ast, node: Id<SwitchCase>) {
        let n = &ast[node];
        self.list(ast, n.labels, "", " ", " ");
        self.w("case ");
        self.n(ast, n.expression);
        self.w(": ");
        self.list(ast, n.statements, "", " ", "");
    }

    fn visit_switch_default(&mut self, ast: &Ast, node: Id<SwitchDefault>) {
        let n = &ast[node];
        self.list(ast, n.labels, "", " ", " ");
        self.w("default: ");
        self.list(ast, n.statements, "", " ", "");
    }

    fn visit_switch_expression(&mut self, ast: &Ast, node: Id<SwitchExpression>) {
        let n = &ast[node];
        self.w("switch (");
        self.n(ast, n.expression);
        self.w(") {");
        self.list(ast, n.cases, "", ", ", "");
        self.w("}");
    }

    fn visit_switch_expression_case(&mut self, ast: &Ast, node: Id<SwitchExpressionCase>) {
        let n = &ast[node];
        self.n(ast, n.guarded_pattern);
        self.w(" => ");
        self.n(ast, n.expression);
    }

    fn visit_switch_pattern_case(&mut self, ast: &Ast, node: Id<SwitchPatternCase>) {
        let n = &ast[node];
        self.list(ast, n.labels, "", " ", " ");
        self.w("case ");
        self.n(ast, n.guarded_pattern);
        self.w(": ");
        self.list(ast, n.statements, "", " ", "");
    }

    fn visit_switch_statement(&mut self, ast: &Ast, node: Id<SwitchStatement>) {
        let n = &ast[node];
        self.w("switch (");
        self.n(ast, n.expression);
        self.w(") {");
        self.list(ast, n.members, "", " ", "");
        self.w("}");
    }

    fn visit_symbol_literal(&mut self, ast: &Ast, node: Id<SymbolLiteral>) {
        self.w("#");
        for (i, &t) in ast.token_list(ast[node].components).iter().enumerate() {
            if i > 0 {
                self.w(".");
            }
            self.lexeme(ast, t);
        }
    }

    fn visit_this_expression(&mut self, _ast: &Ast, _node: Id<ThisExpression>) {
        self.w("this");
    }

    fn visit_throw_expression(&mut self, ast: &Ast, node: Id<ThrowExpression>) {
        self.w("throw ");
        self.n(ast, ast[node].expression);
    }

    fn visit_top_level_variable_declaration(
        &mut self,
        ast: &Ast,
        node: Id<TopLevelVariableDeclaration>,
    ) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.token(ast, n.augment_keyword, "", " ");
        self.token(ast, n.external_keyword, "", " ");
        self.token(ast, n.abstract_keyword, "", " ");
        self.node(ast, Some(n.variables), "", ";");
    }

    fn visit_try_statement(&mut self, ast: &Ast, node: Id<TryStatement>) {
        let n = &ast[node];
        self.w("try ");
        self.n(ast, n.body);
        self.list(ast, n.catch_clauses, " ", " ", "");
        self.node(ast, n.finally_block, " finally ", "");
    }

    fn visit_type_argument_list(&mut self, ast: &Ast, node: Id<TypeArgumentList>) {
        self.w("<");
        self.list(ast, ast[node].arguments, "", ", ", "");
        self.w(">");
    }

    fn visit_type_literal(&mut self, ast: &Ast, node: Id<TypeLiteral>) {
        self.n(ast, ast[node].type_);
    }

    fn visit_type_parameter(&mut self, ast: &Ast, node: Id<TypeParameter>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        if let Some(v) = n.variance_keyword {
            self.lexeme(ast, v);
            self.w(" ");
        }
        self.lexeme(ast, n.name);
        self.node(ast, n.bound, " extends ", "");
    }

    fn visit_type_parameter_list(&mut self, ast: &Ast, node: Id<TypeParameterList>) {
        self.w("<");
        self.list(ast, ast[node].type_parameters, "", ", ", "");
        self.w(">");
    }

    fn visit_variable_declaration(&mut self, ast: &Ast, node: Id<VariableDeclaration>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.lexeme(ast, n.name);
        self.node(ast, n.initializer, " = ", "");
    }

    fn visit_variable_declaration_list(&mut self, ast: &Ast, node: Id<VariableDeclarationList>) {
        let n = &ast[node];
        self.metadata(ast, n.metadata);
        self.token(ast, n.late_keyword, "", " ");
        self.token(ast, n.keyword, "", " ");
        self.node(ast, n.type_, "", " ");
        self.list(ast, n.variables, "", ", ", "");
    }

    fn visit_variable_declaration_statement(
        &mut self,
        ast: &Ast,
        node: Id<VariableDeclarationStatement>,
    ) {
        self.n(ast, ast[node].variables);
        self.w(";");
    }

    fn visit_when_clause(&mut self, ast: &Ast, node: Id<WhenClause>) {
        self.w("when ");
        self.n(ast, ast[node].expression);
    }

    fn visit_while_statement(&mut self, ast: &Ast, node: Id<WhileStatement>) {
        let n = &ast[node];
        self.w("while (");
        self.n(ast, n.condition);
        self.w(") ");
        self.n(ast, n.body);
    }

    fn visit_wildcard_pattern(&mut self, ast: &Ast, node: Id<WildcardPattern>) {
        let n = &ast[node];
        self.token(ast, n.keyword, "", " ");
        self.node(ast, n.type_, "", " ");
        self.lexeme(ast, n.name);
    }

    fn visit_with_clause(&mut self, ast: &Ast, node: Id<WithClause>) {
        self.w("with ");
        self.list(ast, ast[node].mixin_types, "", ", ", "");
    }

    fn visit_yield_statement(&mut self, ast: &Ast, node: Id<YieldStatement>) {
        let n = &ast[node];
        if n.star.is_some() {
            self.w("yield* ");
        } else {
            self.w("yield ");
        }
        self.n(ast, n.expression);
        self.w(";");
    }
}

/// Dart `FormalParameter.isRequiredPositional`.
fn is_required_positional(ast: &Ast, parameter: NodeId) -> bool {
    let kind = match ast.kind(parameter) {
        NodeKind::RegularFormalParameter => {
            ast[Id::<RegularFormalParameter>::from_raw(parameter)].kind
        }
        NodeKind::FieldFormalParameter => ast[Id::<FieldFormalParameter>::from_raw(parameter)].kind,
        NodeKind::SuperFormalParameter => ast[Id::<SuperFormalParameter>::from_raw(parameter)].kind,
        _ => return true,
    };
    kind.is_required_positional()
}
