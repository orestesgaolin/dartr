// Dart source: pkg/analysis_server_plugin/lib/edit/correction_utils.dart (invertCondition, _InvertedCondition)
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/invert_if_statement.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/invert_conditional_expression.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_int_literal.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/replace_conditional_with_if_else.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/assign_to_local_variable.dart
// Dart source: pkg/analyzer/lib/src/dart/ast/utilities.dart (ScopedNameFinder)

//! More assists.

use dartr_ast::*;
use dartr_element::TypeKind;
use dartr_syntax::TokenType;
use dartr_typesystem::type_ext::TypeExt;

use super::super::change::LinkedEditSuggestionKind;
use super::super::dart_edit::WriteType;
use super::super::generated::assist_kinds as a;
use super::super::generated::fix_kinds as k;
use super::super::producer::*;
use super::create::static_type;
use super::simple::{assist, producer};

/// Dart `_InvertedCondition`: the precedence and the source.
struct Inverted(i64, String);

const SIMPLE: i64 = 2147483647;

fn parenthesize(e: &Inverted, precedence: i64) -> String {
    if e.0 < precedence {
        format!("({})", e.1)
    } else {
        e.1.clone()
    }
}

fn unparenthesized(ast: &Ast, mut node: NodeId) -> NodeId {
    while let Some(p) = ast.cast::<ParenthesizedExpression>(node) {
        node = ast[p].expression.raw();
    }
    node
}

/// Dart `CorrectionUtils._invertCondition0`.
fn invert(c: &ProducerContext<'_>, expression: NodeId) -> Inverted {
    let ast = c.ast;
    let text = |n: NodeId| c.utils.get_node_text(n);
    if let Some(b) = ast.cast::<BooleanLiteral>(expression) {
        let value = c.lexeme(ast[b].literal) == "true";
        return Inverted(SIMPLE, if value { "false" } else { "true" }.to_string());
    }
    if let Some(b) = ast.cast::<BinaryExpression>(expression) {
        let le = ast[b].left_operand.raw();
        let re = ast[b].right_operand.raw();
        let binary2 = |op: &str| Inverted(1 << 20, format!("{}{op}{}", text(le), text(re)));
        match ast.tokens.ty(ast[b].operator) {
            TokenType::LT => return binary2(" >= "),
            TokenType::GT => return binary2(" <= "),
            TokenType::LT_EQ => return binary2(" > "),
            TokenType::GT_EQ => return binary2(" < "),
            TokenType::EQ_EQ => return binary2(" != "),
            TokenType::BANG_EQ => return binary2(" == "),
            TokenType::AMPERSAND_AMPERSAND => {
                let (l, r) = (invert(c, le), invert(c, re));
                let p = 5;
                return Inverted(
                    p,
                    format!("{} || {}", parenthesize(&l, p), parenthesize(&r, p)),
                );
            }
            TokenType::BAR_BAR => {
                let (l, r) = (invert(c, le), invert(c, re));
                let p = 6;
                return Inverted(
                    p,
                    format!("{} && {}", parenthesize(&l, p), parenthesize(&r, p)),
                );
            }
            _ => {}
        }
    } else if let Some(i) = ast.cast::<IsExpression>(expression) {
        let e = text(ast[i].expression.raw());
        let t = text(ast[i].type_.raw());
        return Inverted(
            SIMPLE,
            if ast[i].not_operator.is_none() {
                format!("{e} is! {t}")
            } else {
                format!("{e} is {t}")
            },
        );
    } else if let Some(p) = ast.cast::<PrefixExpression>(expression) {
        if ast.tokens.ty(ast[p].operator) == TokenType::BANG {
            let operand = unparenthesized(ast, ast[p].operand.raw());
            return Inverted(SIMPLE, text(operand));
        }
    } else if ast.is::<ParenthesizedExpression>(expression) {
        return invert(c, unparenthesized(ast, expression));
    }
    let is_bool = static_type(c, expression).is_some_and(|t| c.ctx.is_dart_core_bool(t));
    if is_bool {
        return Inverted(SIMPLE, format!("!{}", text(expression)));
    }
    Inverted(SIMPLE, text(expression))
}

/// Dart `CorrectionUtils.invertCondition`.
pub fn invert_condition(c: &ProducerContext<'_>, expression: NodeId) -> String {
    invert(c, expression).1
}

assist!(InvertIfStatement, a::INVERT_IF_STATEMENT, |c, builder| {
    let ast = c.ast;
    let Some(i) = ast.cast::<IfStatement>(c.node) else {
        return;
    };
    if ast[i].case_clause.is_some() {
        return;
    }
    let then_statement = ast[i].then_statement.raw();
    let Some(else_statement) = ast[i].else_statement.map(|e| e.raw()) else {
        return;
    };
    if !ast.is::<Block>(then_statement) || !ast.is::<Block>(else_statement) {
        return;
    }
    let condition = ast[i].expression.raw();
    let inverted = invert_condition(c, condition);
    let then_code = c.utils.get_node_text(then_statement);
    let else_code = c.utils.get_node_text(else_statement);
    let (rc, rt, re) = (
        c.range().node(condition),
        c.range().node(then_statement),
        c.range().node(else_statement),
    );
    builder.add_dart_file_edit(c.path, |b| {
        b.add_simple_replacement(rc.offset, rc.length, &inverted);
        b.add_simple_replacement(rt.offset, rt.length, &else_code);
        b.add_simple_replacement(re.offset, re.length, &then_code);
    });
});

/// Dart `_thisOrParentOfType`.
fn this_or_parent<T: NodeType + ?Sized>(ast: &Ast, node: NodeId) -> Option<NodeId> {
    if ast.is::<T>(node) {
        return Some(node);
    }
    ast.parent(node).filter(|p| ast.is::<T>(*p))
}

assist!(
    InvertConditionalExpression,
    a::INVERT_CONDITIONAL_EXPRESSION,
    |c, builder| {
        let ast = c.ast;
        let mut node = c.node;
        node = this_or_parent::<MethodInvocation>(ast, node).unwrap_or(node);
        node = this_or_parent::<AwaitExpression>(ast, node).unwrap_or(node);
        let is_boolean = |n: NodeId| {
            ast.is::<IsExpression>(n)
                || ast.is::<BinaryExpression>(n)
                || ast.is::<BooleanLiteral>(n)
        };
        let boolean = if is_boolean(node) {
            Some(node)
        } else if ast.parent(node).is_some_and(is_boolean) {
            ast.parent(node)
        } else {
            this_or_parent::<PrefixExpression>(ast, node)
        };
        node = boolean.unwrap_or(node);
        node = this_or_parent::<ParenthesizedExpression>(ast, node).unwrap_or(node);
        node = this_or_parent::<AwaitExpression>(ast, node).unwrap_or(node);
        let Some(conditional) = this_or_parent::<ConditionalExpression>(ast, node)
            .and_then(|n| ast.cast::<ConditionalExpression>(n))
        else {
            return;
        };
        let condition = ast[conditional].condition.raw();
        let then_expression = ast[conditional].then_expression.raw();
        let else_expression = ast[conditional].else_expression.raw();
        let inverted = invert_condition(c, condition);
        let then_code = c.utils.get_node_text(then_expression);
        let else_code = c.utils.get_node_text(else_expression);
        let (rc, rt, re) = (
            c.range().node(condition),
            c.range().node(then_expression),
            c.range().node(else_expression),
        );
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(rc.offset, rc.length, &inverted);
            b.add_simple_replacement(rt.offset, rt.length, &else_code);
            b.add_simple_replacement(re.offset, re.length, &then_code);
        });
    }
);

producer!(
    ConvertToIntLiteral,
    k::CONVERT_TO_INT_LITERAL,
    Some(&k::CONVERT_TO_INT_LITERAL_MULTI),
    Automatically,
    assist: Some(&a::CONVERT_TO_INT_LITERAL),
    |c, builder| {
        let ast = c.ast;
        let Some(literal) = ast.cast::<DoubleLiteral>(c.node) else {
            return;
        };
        let value = ast[literal].value;
        if !value.is_finite() || value.trunc() != value || value.abs() >= 9.223372036854776e18 {
            return;
        }
        let int_value = value.trunc() as i64;
        let lexeme = c.lexeme(ast[literal].literal).to_string();
        let offset = ast.offset(literal);
        let length = ast.length(literal);
        builder.add_dart_file_edit(c.path, |b| {
            if !lexeme.to_lowercase().contains('e') {
                if let Some(index) = lexeme.find('.').filter(|i| *i > 0) {
                    let index = index as u32;
                    b.add_deletion(offset + index, length - index);
                    return;
                }
            }
            b.add_simple_replacement(offset, length, &int_value.to_string());
        });
    }
);

assist!(
    ReplaceConditionalWithIfElse,
    a::REPLACE_CONDITIONAL_WITH_IF_ELSE,
    |c, builder| {
        let ast = c.ast;
        let Some(statement) = ast.this_or_ancestor_of_type::<Statement>(c.node) else {
            return;
        };
        let statement = statement.raw();
        let indent = c.utils.one_indent();
        let prefix = c.utils.get_node_prefix(statement);
        let eol = c.eol().to_string();
        let parts = |cond: Id<ConditionalExpression>| {
            (
                c.utils.get_node_text(ast[cond].condition),
                c.utils.get_node_text(ast[cond].then_expression),
                c.utils.get_node_text(ast[cond].else_expression),
            )
        };
        if let Some(s) = ast.cast::<VariableDeclarationStatement>(statement) {
            let list = ast[s].variables;
            for &variable in ast.list(ast[list].variables) {
                let Some(conditional) = ast[variable]
                    .initializer
                    .and_then(|i| ast.cast::<ConditionalExpression>(i))
                else {
                    continue;
                };
                let ty = if ast[list].type_.is_none() {
                    c.locator()
                        .declared_element(variable)
                        .map(|e| dartr_resolver::element_ext::variable_type(c.ctx, e))
                } else {
                    None
                };
                let keyword = ast[list].keyword;
                let name = c.lexeme(ast[variable].name).to_string();
                let name_offset = c.token_offset(ast[variable].name);
                let deletion = c
                    .range()
                    .token_end_node_end(ast[variable].name, conditional);
                let (cond, then, els) = parts(conditional);
                let end = ast.end(statement);
                let src = format!(
                    "{eol}{prefix}if ({cond}) {{{eol}{prefix}{indent}{name} = {then};{eol}{prefix}}} else {{{eol}{prefix}{indent}{name} = {els};{eol}{prefix}}}"
                );
                builder.add_dart_file_edit(c.path, |b| {
                    if let Some(ty) = ty {
                        match keyword.filter(|k| c.lexeme(*k) == "var") {
                            Some(k) => {
                                let r = c.range().token(k);
                                b.add_replacement(r.offset, r.length, |e| {
                                    e.write_type(Some(ty), &WriteType::default());
                                });
                            }
                            None => b.add_insertion(name_offset, |e| {
                                e.write_type(Some(ty), &WriteType::default());
                                e.write(" ");
                            }),
                        }
                    }
                    b.add_deletion(deletion.offset, deletion.length);
                    b.add_simple_replacement(end, 0, &src);
                });
            }
            return;
        }
        if let Some(s) = ast.cast::<ExpressionStatement>(statement) {
            if let Some(assignment) = ast.cast::<AssignmentExpression>(ast[s].expression) {
                let Some(conditional) =
                    ast.cast::<ConditionalExpression>(ast[assignment].right_hand_side)
                else {
                    return;
                };
                if c.lexeme(ast[assignment].operator) != "=" {
                    return;
                }
                let name = c.utils.get_node_text(ast[assignment].left_hand_side);
                let (cond, then, els) = parts(conditional);
                let src = format!(
                    "if ({cond}) {{{eol}{prefix}{indent}{name} = {then};{eol}{prefix}}} else {{{eol}{prefix}{indent}{name} = {els};{eol}{prefix}}}"
                );
                let r = c.range().node(statement);
                builder.add_dart_file_edit(c.path, |b| {
                    b.add_simple_replacement(r.offset, r.length, &src)
                });
            }
            return;
        }
        if let Some(r) = ast.cast::<ReturnStatement>(statement) {
            let Some(conditional) = ast[r]
                .expression
                .and_then(|e| ast.cast::<ConditionalExpression>(e))
            else {
                return;
            };
            let (cond, then, els) = parts(conditional);
            let src = format!(
                "if ({cond}) {{{eol}{prefix}{indent}return {then};{eol}{prefix}}} else {{{eol}{prefix}{indent}return {els};{eol}{prefix}}}"
            );
            let r = c.range().node(statement);
            builder.add_dart_file_edit(c.path, |b| {
                b.add_simple_replacement(r.offset, r.length, &src)
            });
        }
    }
);

/// Dart `ScopedNameFinder(offset)` from [node]: the local names visible
/// at [node].
pub(crate) fn scoped_names(c: &ProducerContext<'_>, node: NodeId, position: u32) -> Vec<String> {
    let ast = c.ast;
    let mut locals: Vec<String> = Vec::new();
    let add = |t: Option<dartr_syntax::TokenId>, locals: &mut Vec<String>| {
        if let Some(t) = t {
            if c.token_end(t) < position {
                locals.push(c.lexeme(t).to_string());
            }
        }
    };
    let parameter_names = |list: Id<FormalParameterList>| -> Vec<dartr_syntax::TokenId> {
        ast.list_raw(ast[list].parameters)
            .iter()
            .filter_map(|&p| {
                if let Some(r) = ast.cast::<RegularFormalParameter>(p) {
                    ast[r].name
                } else if let Some(f) = ast.cast::<FieldFormalParameter>(p) {
                    Some(ast[f].name)
                } else {
                    ast.cast::<SuperFormalParameter>(p).map(|s| ast[s].name)
                }
            })
            .collect()
    };
    let mut in_local_function = false;
    let mut child = node;
    let mut current = ast.parent(node);
    while let Some(n) = current {
        let statements: Option<Vec<NodeId>> = if let Some(b) = ast.cast::<Block>(n) {
            Some(ast.list_raw(ast[b].statements).to_vec())
        } else if let Some(m) = ast.cast::<SwitchCase>(n) {
            Some(ast.list_raw(ast[m].statements).to_vec())
        } else if let Some(m) = ast.cast::<SwitchDefault>(n) {
            Some(ast.list_raw(ast[m].statements).to_vec())
        } else {
            ast.cast::<SwitchPatternCase>(n)
                .map(|m| ast.list_raw(ast[m].statements).to_vec())
        };
        if let Some(statements) = statements {
            for s in statements {
                if s == child {
                    break;
                }
                if let Some(v) = ast.cast::<VariableDeclarationStatement>(s) {
                    for &var in ast.list(ast[ast[v].variables].variables) {
                        add(Some(ast[var].name), &mut locals);
                    }
                } else if let Some(f) = ast.cast::<FunctionDeclarationStatement>(s) {
                    if !in_local_function {
                        add(Some(ast[ast[f].function_declaration].name), &mut locals);
                    }
                }
            }
        }
        if let Some(cc) = ast.cast::<CatchClause>(n) {
            for p in [ast[cc].exception_parameter, ast[cc].stack_trace_parameter]
                .into_iter()
                .flatten()
            {
                add(Some(ast[p].name), &mut locals);
            }
        } else if let Some(f) = ast.cast::<ForEachPartsWithDeclaration>(n) {
            add(Some(ast[ast[f].loop_variable].name), &mut locals);
        } else if let Some(f) = ast.cast::<ForPartsWithDeclarations>(n) {
            for &var in ast.list(ast[ast[f].variables].variables) {
                add(Some(ast[var].name), &mut locals);
            }
        } else if let Some(f) = ast.cast::<FunctionExpression>(n) {
            if let Some(list) = ast[f].parameters.filter(|l| l.raw() != child) {
                for t in parameter_names(list) {
                    add(Some(t), &mut locals);
                }
            }
        } else if ast.is::<FunctionDeclarationStatement>(n) {
            in_local_function = true;
        } else if let Some(m) = ast.cast::<MethodDeclaration>(n) {
            if let Some(list) = ast[m].parameters.filter(|l| l.raw() != child) {
                for t in parameter_names(list) {
                    add(Some(t), &mut locals);
                }
            }
            break;
        } else if let Some(cd) = ast.cast::<ConstructorDeclaration>(n) {
            let list = ast[cd].parameters;
            if list.raw() != child {
                for t in parameter_names(list) {
                    add(Some(t), &mut locals);
                }
            }
            break;
        } else if let Some(fd) = ast.cast::<FunctionDeclaration>(n) {
            if !ast
                .parent(fd)
                .is_some_and(|p| ast.is::<FunctionDeclarationStatement>(p))
            {
                break;
            }
        } else if ast.is::<FieldDeclaration>(n)
            || ast.is::<TopLevelVariableDeclaration>(n)
            || ast.is::<TypeAlias>(n)
        {
            break;
        }
        child = n;
        current = ast.parent(n);
    }
    locals
}

assist!(
    AssignToLocalVariable,
    a::ASSIGN_TO_LOCAL_VARIABLE,
    |c, builder| {
        let ast = c.ast;
        let mut statement: Option<Id<ExpressionStatement>> = None;
        let mut current = Some(c.node);
        while let Some(n) = current {
            if let Some(s) = ast.cast::<ExpressionStatement>(n) {
                statement = Some(s);
                break;
            }
            if ast.is::<ArgumentList>(n)
                || ast.is::<AssignmentExpression>(n)
                || ast.is::<Statement>(n)
                || ast.is::<ThrowExpression>(n)
            {
                return;
            }
            current = ast.parent(n);
        }
        let Some(statement) = statement else { return };
        // Dart `_hasPrecedingStatementRecovery`.
        if let Some(block) = ast.parent(statement).and_then(|p| ast.cast::<Block>(p)) {
            let statements = ast.list_raw(ast[block].statements);
            if let Some(index) = statements.iter().position(|s| *s == statement.raw()) {
                if index > 0 {
                    let preceding = statements[index - 1];
                    if let Some(e) = ast.cast::<ExpressionStatement>(preceding) {
                        if ast[e]
                            .semicolon
                            .is_some_and(|t| ast.tokens.get(t).is_synthetic())
                        {
                            return;
                        }
                    } else if let Some(v) = ast.cast::<VariableDeclarationStatement>(preceding) {
                        if ast.tokens.get(ast[v].semicolon).is_synthetic() {
                            return;
                        }
                    }
                }
            }
        }
        let expression = ast[statement].expression.raw();
        let offset = ast.offset(expression);
        let Some(ty) = static_type(c, expression) else {
            return;
        };
        if matches!(c.ctx.ty(ty), TypeKind::Void) {
            return;
        }
        let excluded = scoped_names(c, expression, offset);
        let suggestions = super::create::variable_name_suggestions(c, ty, expression, &excluded);
        if suggestions.is_empty() {
            return;
        }
        let keyword = if c.code_style().make_locals_final() {
            "final"
        } else {
            "var"
        };
        builder.add_dart_file_edit(c.path, |b| {
            b.add_insertion(offset, |e| {
                e.write(&format!("{keyword} "));
                e.add_simple_linked_edit(
                    "NAME",
                    &suggestions[0],
                    Some((LinkedEditSuggestionKind::Variable, &suggestions)),
                );
                e.write(" = ");
            });
        });
    }
);

assist!(
    ConvertIntoFinalField,
    a::CONVERT_INTO_FINAL_FIELD,
    |c, builder| {
        let ast = c.ast;
        let ctx = c.ctx;
        let mut getter: Option<Id<MethodDeclaration>> = None;
        let mut current = Some(c.node);
        while let Some(n) = current {
            if let Some(m) = ast.cast::<MethodDeclaration>(n) {
                getter = Some(m);
                break;
            }
            if ast.is::<SimpleIdentifier>(n)
                || ast.is::<TypeAnnotation>(n)
                || ast.is::<TypeArgumentList>(n)
            {
                current = ast.parent(n);
                continue;
            }
            break;
        }
        let Some(getter) = getter else { return };
        let Some(get_keyword) = ast[getter]
            .property_keyword
            .filter(|k| c.lexeme(*k) == "get")
        else {
            return;
        };
        let Some(element) = c.locator().declared_element(getter) else {
            return;
        };
        if element.tag() != dartr_element::Tag::Getter {
            return;
        }
        let is_static =
            dartr_typesystem::member::is_static(ctx, dartr_element::ElemRef::Base(element));
        if !is_static {
            let enclosing = ctx.element_data(element).and_then(|d| d.enclosing);
            if enclosing.is_some_and(|e| {
                matches!(
                    e.tag(),
                    dartr_element::Tag::Extension | dartr_element::Tag::ExtensionType
                )
            }) {
                return;
            }
        }
        if dartr_typesystem::member::corresponding_setter(
            ctx,
            dartr_element::ElemRef::Base(element),
        )
        .is_some()
        {
            return;
        }
        let body = ast[getter].body.raw();
        let expression = if let Some(b) = ast.cast::<ExpressionFunctionBody>(body) {
            Some(ast[b].expression.raw())
        } else if let Some(b) = ast.cast::<BlockFunctionBody>(body) {
            let statements = ast.list(ast[ast[b].block].statements);
            if statements.len() == 1 {
                ast.cast::<ReturnStatement>(statements[0])
                    .and_then(|r| ast[r].expression)
                    .map(|e| e.raw())
            } else {
                None
            }
        } else {
            None
        };
        let Some(expression) = expression else { return };
        let return_type = ast[getter].return_type;
        let mut code = "final".to_string();
        if let Some(t) = return_type {
            code.push(' ');
            code.push_str(&c.utils.get_node_text(t));
        }
        code.push(' ');
        code.push_str(c.lexeme(ast[getter].name));
        if !ast.is::<NullLiteral>(expression) {
            code.push_str(" = ");
            code.push_str(&c.utils.get_node_text(expression));
        }
        code.push(';');
        let range = match return_type {
            Some(t) => c.range().start_end(t, getter),
            None => c.range().token_start_node_end(get_keyword, getter),
        };
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(range.offset, range.length, &code)
        });
    }
);

/// Dart `ConvertIntoGetter` (`.new` and `.this_`).
pub struct ConvertIntoGetter {
    pub this_: bool,
    member_name: String,
}

impl ConvertIntoGetter {
    pub fn new(this_: bool) -> Self {
        ConvertIntoGetter {
            this_,
            member_name: String::new(),
        }
    }
}

impl CorrectionProducer for ConvertIntoGetter {
    fn fix_kind(&self) -> Option<&'static super::super::fix_kind::FixKind> {
        Some(&k::CONVERT_INTO_GETTER)
    }

    fn fix_arguments(&self) -> Vec<String> {
        vec![self.member_name.clone()]
    }

    fn assist_kind(&self) -> Option<&'static super::super::fix_kind::FixKind> {
        Some(&a::CONVERT_INTO_GETTER)
    }

    fn assist_arguments(&self) -> Vec<String> {
        vec![self.member_name.clone()]
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(
        &mut self,
        c: &ProducerContext<'_>,
        builder: &mut super::super::change_builder::ChangeBuilder<'_>,
    ) {
        let ast = c.ast;
        let ctx = c.ctx;
        let declaration = if self.this_ {
            ast.this_or_ancestor_of_type::<FieldDeclaration>(c.node)
        } else {
            let mut found = None;
            let mut current = Some(c.node);
            while let Some(n) = current {
                if let Some(f) = ast.cast::<FieldDeclaration>(n) {
                    found = Some(f);
                    break;
                }
                if ast.is::<SimpleIdentifier>(n)
                    || ast.is::<VariableDeclaration>(n)
                    || ast.is::<VariableDeclarationList>(n)
                    || ast.is::<TypeAnnotation>(n)
                    || ast.is::<TypeArgumentList>(n)
                {
                    current = ast.parent(n);
                    continue;
                }
                break;
            }
            found
        };
        let Some(declaration) = declaration else {
            return;
        };
        let list = ast[declaration].fields;
        let variables = ast.list(ast[list].variables);
        if variables.len() != 1 {
            return;
        }
        let field = variables[0];
        self.member_name = c.lexeme(ast[field].name).to_string();
        if self.member_name.is_empty() {
            return;
        }
        let initializer = ast[field].initializer.map(|i| i.raw());
        let code = format!("get {} => ", self.member_name);
        let start = ast[list]
            .late_keyword
            .or(ast[list].keyword)
            .map(|t| c.token_offset(t))
            .or_else(|| ast[list].type_.map(|t| ast.offset(t)))
            .unwrap_or_else(|| c.token_offset(ast[field].name));
        let end = ast.end(declaration);
        let write_type = c.code_style().specify_return_types();
        let declared = ast[list]
            .type_
            .map(|t| c.tables.annotation_type.get(t.raw()).copied());
        let initializer_text = initializer.map(|i| c.utils.get_node_text(i));
        // Dart: `if (fieldList.type?.type case var type)` always matches.
        let mut ty = declared.flatten();
        if ty.is_none_or(|t| matches!(ctx.ty(t), TypeKind::Dynamic)) {
            ty = initializer.and_then(|i| static_type(c, i));
        }
        if ty.is_some_and(|t| matches!(ctx.ty(t), TypeKind::Invalid)) {
            ty = Some(ctx.tp.dynamic_type());
        }
        builder.add_dart_file_edit(c.path, |b| {
            b.add_replacement(start, end - start, |e| {
                let not_dynamic = ty.is_some_and(|t| !matches!(ctx.ty(t), TypeKind::Dynamic));
                if not_dynamic || write_type {
                    e.write_type(
                        ty,
                        &WriteType {
                            should_write_dynamic: write_type,
                            required: write_type,
                            ..Default::default()
                        },
                    );
                    e.write(" ");
                }
                e.write(&code);
                match &initializer_text {
                    None => {
                        e.add_simple_linked_edit("initializer", "null", None);
                        e.write(";");
                    }
                    Some(t) => e.write(&format!("{t};")),
                }
            });
        });
    }
}
