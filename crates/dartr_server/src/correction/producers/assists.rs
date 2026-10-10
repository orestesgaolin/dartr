// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_expression_function_body.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_into_block_body.dart (missingBody)
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/exchange_operands.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/split_variable_declaration.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_package_import.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_relative_import.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_digit_separators.dart
// Dart source: pkg/analysis_server_plugin/lib/edit/dart/correction_producer.dart (getEnclosingFunctionBody, isOperatorSelected)

//! Producers that are assists (some of them are fixes too).

use dartr_ast::*;
use dartr_element::{DirectiveUri, ElemRef, ElementId, TypeKind};
use dartr_syntax::TokenId;
use dartr_typesystem::type_ext::TypeExt;

use super::super::change_builder::ChangeBuilder;
use super::super::dart_edit::WriteType;
use super::super::fix_kind::FixKind;
use super::super::generated::assist_kinds as a;
use super::super::generated::fix_kinds as k;
use super::super::producer::*;
use super::create::static_type;
use super::simple::{assist, producer};

/// Dart `getEnclosingFunctionBody`.
pub fn enclosing_function_body(c: &ProducerContext<'_>) -> Option<Id<FunctionBody>> {
    let ast = c.ast;
    let node = c.node;
    if let Some(f) = ast.this_or_ancestor_of_type::<FunctionExpression>(node) {
        return Some(ast[f].body);
    }
    if let Some(f) = ast.this_or_ancestor_of_type::<FunctionDeclaration>(node) {
        return Some(ast[ast[f].function_expression].body);
    }
    if let Some(f) = ast.this_or_ancestor_of_type::<ConstructorDeclaration>(node) {
        return Some(ast[f].body);
    }
    if let Some(f) = ast.this_or_ancestor_of_type::<MethodDeclaration>(node) {
        return Some(ast[f].body);
    }
    if let Some(f) = ast.this_or_ancestor_of_type::<PrimaryConstructorBody>(node) {
        return Some(ast[f].body);
    }
    None
}

/// The `async`/`sync` keyword and the star of a function body.
fn body_modifiers(ast: &Ast, body: NodeId) -> (Option<TokenId>, Option<TokenId>) {
    if let Some(b) = ast.cast::<BlockFunctionBody>(body) {
        (ast[b].keyword, ast[b].star)
    } else if let Some(b) = ast.cast::<ExpressionFunctionBody>(body) {
        (ast[b].keyword, ast[b].star)
    } else {
        (None, None)
    }
}

/// Dart `FunctionBody.isAsynchronous`.
pub fn is_asynchronous(ast: &Ast, body: NodeId) -> bool {
    body_modifiers(ast, body)
        .0
        .is_some_and(|k| ast.tokens.lexeme(k) == "async")
}

/// Dart `FunctionBody.isGenerator`.
pub fn is_generator(ast: &Ast, body: NodeId) -> bool {
    body_modifiers(ast, body).1.is_some()
}

fn has_preceding_comments(c: &ProducerContext<'_>, t: TokenId) -> bool {
    c.ast.tokens.get(t).preceding_comments.is_some()
}

/// Dart `isOperatorSelected`.
fn is_operator_selected(c: &ProducerContext<'_>, binary: Id<BinaryExpression>) -> bool {
    let ast = c.ast;
    let left = ast[binary].left_operand;
    let right = ast[binary].right_operand;
    let start = c.selection_offset;
    let end = c.selection_offset + c.selection_length;
    if start >= ast.end(left) && end <= ast.offset(right) {
        return true;
    }
    if start == ast.offset(left) && end == ast.end(right) {
        return !(ast.is::<BinaryExpression>(left) || ast.is::<BinaryExpression>(right));
    }
    false
}

producer!(
    ConvertToExpressionFunctionBody,
    k::CONVERT_INTO_EXPRESSION_BODY,
    Some(&k::CONVERT_INTO_EXPRESSION_BODY_MULTI),
    Automatically,
    assist: Some(&a::CONVERT_INTO_EXPRESSION_BODY),
    |c, builder| {
        let ast = c.ast;
        let Some(body) = enclosing_function_body(c) else {
            return;
        };
        let Some(block_body) = ast.cast::<BlockFunctionBody>(body) else {
            return;
        };
        if is_generator(ast, body.raw()) {
            return;
        }
        let block = ast[block_body].block;
        if ast[block_body]
            .keyword
            .is_some_and(|k| has_preceding_comments(c, k))
            || has_preceding_comments(c, ast[block].left_bracket)
            || has_preceding_comments(c, ast[block].right_bracket)
        {
            return;
        }
        let parent = ast.parent(body);
        if let Some(constructor) = parent.and_then(|p| ast.cast::<ConstructorDeclaration>(p)) {
            if ast[constructor].factory_keyword.is_none() {
                return;
            }
        }
        let statements = ast.list(ast[block].statements);
        if statements.len() != 1 {
            return;
        }
        let only = statements[0];
        let return_expression = if let Some(r) = ast.cast::<ReturnStatement>(only) {
            let Some(expression) = ast[r].expression else {
                return;
            };
            if has_preceding_comments(c, ast[r].return_keyword)
                || has_preceding_comments(c, ast[r].semicolon)
            {
                return;
            }
            expression
        } else if let Some(s) = ast.cast::<ExpressionStatement>(only) {
            if ast[s].semicolon.is_some_and(|t| has_preceding_comments(c, t)) {
                return;
            }
            ast[s].expression
        } else {
            return;
        };
        if c.selection_offset >= ast.offset(return_expression) {
            return;
        }
        let range = c.range().node(body);
        let asynchronous = is_asynchronous(ast, body.raw());
        let text = c.utils.get_node_text(return_expression);
        let semicolon = match parent.and_then(|p| ast.cast::<FunctionExpression>(p)) {
            None => true,
            Some(f) => ast
                .parent(f)
                .is_some_and(|p| ast.is::<FunctionDeclaration>(p)),
        };
        builder.add_dart_file_edit(c.path, |b| {
            b.add_replacement(range.offset, range.length, |e| {
                if asynchronous {
                    e.write("async ");
                }
                e.write("=> ");
                e.write(&text);
                if semicolon {
                    e.write(";");
                }
            });
        });
    }
);

/// Dart `ConvertIntoBlockBody._getFunctionElement`.
fn function_element(c: &ProducerContext<'_>, node: Option<NodeId>) -> Option<ElementId> {
    let ast = c.ast;
    let node = node?;
    if ast.is::<MethodDeclaration>(node) || ast.is::<ConstructorDeclaration>(node) {
        return c.locator().declared_element(node);
    }
    if let Some(f) = ast.cast::<FunctionExpression>(node) {
        if let Some(e) = c.locator().declared_element(f) {
            return Some(e);
        }
        let parent = ast.parent(f)?;
        if ast.is::<FunctionDeclaration>(parent) {
            return c.locator().declared_element(parent);
        }
        return None;
    }
    if let Some(b) = ast.cast::<PrimaryConstructorBody>(node) {
        // Dart `PrimaryConstructorBody.declaration`.
        let mut p = ast.parent(b);
        while let Some(n) = p {
            if let Some(d) = ast.cast::<PrimaryConstructorDeclaration>(n) {
                return c.locator().declared_element(d);
            }
            p = ast.parent(n);
        }
    }
    None
}

fn element_return_type(c: &ProducerContext<'_>, element: ElementId) -> dartr_element::TypeId {
    dartr_typesystem::member::return_type(c.ctx, ElemRef::Base(element))
}

/// Dart `ConvertIntoBlockBody.missingBody`.
pub struct ConvertIntoBlockBody;

impl ConvertIntoBlockBody {
    fn code_for_function_body(
        c: &ProducerContext<'_>,
        body: Id<ExpressionFunctionBody>,
    ) -> Option<Vec<String>> {
        let ast = c.ast;
        let ctx = c.ctx;
        let return_value = ast[body].expression;
        if c.selection_offset >= ast.offset(return_value) {
            return None;
        }
        let element = function_element(c, ast.parent(body))?;
        let return_value_type = static_type(c, return_value.raw())?;
        let code = c.utils.get_node_text(return_value);
        let mut return_code = String::new();
        if !matches!(ctx.ty(return_value_type), TypeKind::Void)
            && !ctx.is_bottom(return_value_type)
            && !matches!(ctx.ty(element_return_type(c, element)), TypeKind::Void)
        {
            return_code.push_str("return ");
        }
        return_code.push_str(&code);
        return_code.push(';');
        Some(vec![return_code])
    }

    fn code_for_empty_body(
        c: &ProducerContext<'_>,
        body: Id<EmptyFunctionBody>,
    ) -> Option<Vec<String>> {
        let ast = c.ast;
        let element = function_element(c, ast.parent(body))?;
        let name = dartr_resolver::error::support::display_name(c.ctx, element);
        let mut lines = vec![format!("// TODO: implement {name}")];
        if !matches!(c.ctx.ty(element_return_type(c, element)), TypeKind::Void) {
            lines.push("throw UnimplementedError();".to_string());
        }
        Some(lines)
    }

    fn missing_function_body(
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        body: Id<FunctionBody>,
    ) {
        let ast = c.ast;
        let lines = if let Some(b) = ast.cast::<ExpressionFunctionBody>(body) {
            Self::code_for_function_body(c, b)
        } else if let Some(b) = ast.cast::<EmptyFunctionBody>(body) {
            Self::code_for_empty_body(c, b)
        } else {
            None
        };
        let Some(lines) = lines else { return };
        let Some(parent) = ast.parent(body) else {
            return;
        };
        let prefix = c.utils.get_node_prefix(parent);
        let indent = c.utils.one_indent();
        let previous = ast.tokens.previous(ast.begin_token(body));
        let start = c.token_end(previous);
        let end = ast.end(body);
        let asynchronous = is_asynchronous(ast, body.raw());
        let eol = c.eol().to_string();
        builder.add_dart_file_edit(c.path, |b| {
            b.add_replacement(start, end - start, |e| {
                e.write(" ");
                if asynchronous {
                    e.write("async ");
                }
                e.write("{");
                for line in &lines {
                    e.write(&format!("{eol}{prefix}{indent}"));
                    e.write(line);
                }
                e.select_here();
                e.write(&format!("{eol}{prefix}}}"));
            });
        });
    }

    fn missing_container_body(
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        container: NodeId,
    ) {
        let ast = c.ast;
        let body: Option<NodeId> = if let Some(d) = ast.cast::<ClassDeclaration>(container) {
            Some(ast[d].body.raw())
        } else if let Some(d) = ast.cast::<EnumDeclaration>(container) {
            Some(ast[d].body.raw())
        } else if let Some(d) = ast.cast::<ExtensionDeclaration>(container) {
            Some(ast[d].body.raw())
        } else if let Some(d) = ast.cast::<ExtensionTypeDeclaration>(container) {
            Some(ast[d].body.raw())
        } else if let Some(d) = ast.cast::<MixinDeclaration>(container) {
            Some(ast[d].body.raw())
        } else {
            None
        };
        let Some(body) = body else { return };
        if ast.is::<EmptyClassBody>(body) || ast.is::<EmptyEnumBody>(body) {
            let range = c.range().node(body);
            builder.add_dart_file_edit(c.path, |b| {
                b.add_simple_replacement(range.offset, range.length, " {}")
            });
        }
    }
}

impl CorrectionProducer for ConvertIntoBlockBody {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::CONVERT_INTO_BLOCK_BODY)
    }

    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::CONVERT_INTO_BLOCK_BODY_MULTI)
    }

    fn assist_kind(&self) -> Option<&'static FixKind> {
        Some(&a::CONVERT_INTO_BLOCK_BODY)
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        if let Some(body) = enclosing_function_body(c) {
            if !is_generator(ast, body.raw()) {
                Self::missing_function_body(c, builder, body);
            }
            return;
        }
        if let Some(container) = ast.this_or_ancestor_of_type::<CompilationUnitMember>(c.node) {
            Self::missing_container_body(c, builder, container.raw());
        }
    }
}

assist!(ExchangeOperands, a::EXCHANGE_OPERANDS, |c, builder| {
    let ast = c.ast;
    let Some(mut binary) = ast.cast::<BinaryExpression>(c.node) else {
        return;
    };
    if !is_operator_selected(c, binary) {
        return;
    }
    let left = ast[binary].left_operand;
    let right = ast[binary].right_operand;
    let operator_type = |b: Id<BinaryExpression>| ast.tokens.ty(ast[b].operator);
    while let Some(parent) = ast
        .parent(binary)
        .and_then(|p| ast.cast::<BinaryExpression>(p))
    {
        if operator_type(parent) != operator_type(binary) {
            break;
        }
        binary = parent;
    }
    let left_range = c.range().start_end(binary, left);
    let right_range = c.range().start_end(right, binary);
    let operator = ast[binary].operator;
    let new_operator = match c.lexeme(operator) {
        "<" => Some(">"),
        "<=" => Some(">="),
        ">" => Some("<"),
        ">=" => Some("<="),
        _ => None,
    };
    let left_text = c.utils.get_range_text(left_range);
    let right_text = c.utils.get_range_text(right_range);
    let operator_range = c.range().token(operator);
    builder.add_dart_file_edit(c.path, |b| {
        b.add_simple_replacement(left_range.offset, left_range.length, &right_text);
        b.add_simple_replacement(right_range.offset, right_range.length, &left_text);
        if let Some(op) = new_operator {
            b.add_simple_replacement(operator_range.offset, operator_range.length, op);
        }
    });
});

assist!(
    SplitVariableDeclaration,
    a::SPLIT_VARIABLE_DECLARATION,
    |c, builder| {
        let ast = c.ast;
        let ctx = c.ctx;
        let Some(list) = ast.this_or_ancestor_of_type::<VariableDeclarationList>(c.node) else {
            return;
        };
        let Some(statement) = ast
            .parent(list)
            .and_then(|p| ast.cast::<VariableDeclarationStatement>(p))
        else {
            return;
        };
        let keyword = ast[list].keyword;
        if keyword.is_some_and(|k| matches!(c.lexeme(k), "const" | "final")) {
            return;
        }
        let variables = ast.list(ast[list].variables);
        if variables.len() != 1 {
            return;
        }
        let variable = variables[0];
        let name = ast[variable].name;
        let range = c.range().node_start_token_end(statement, name);
        if !range.contains(c.selection_offset) {
            return;
        }
        if ast[variable].initializer.is_none() {
            return;
        }
        let ty = if ast[list].type_.is_none() {
            c.locator()
                .declared_element(variable)
                .map(|e| dartr_resolver::element_ext::variable_type(ctx, e))
        } else {
            None
        };
        let indent = c.utils.get_node_prefix(statement.raw());
        let name_text = c.lexeme(name).to_string();
        let name_end = c.token_end(name);
        builder.add_dart_file_edit(c.path, |b| {
            let eol = b.eol();
            if let (Some(ty), Some(keyword)) = (ty, keyword) {
                if !matches!(ctx.ty(ty), TypeKind::Dynamic) {
                    let offset = c.token_offset(keyword);
                    if b.can_write_type_at(offset, ty) {
                        let length = c.token_end(keyword) - offset;
                        b.add_replacement(offset, length, |e| {
                            e.write_type(Some(ty), &WriteType::default());
                        });
                    }
                }
            }
            b.add_simple_insertion(name_end, &format!(";{eol}{indent}{name_text}"));
        });
    }
);

/// The URI of the import directive [node] in the element model (Dart
/// `ImportDirective.libraryImport?.uri`).
fn import_uri(c: &ProducerContext<'_>, node: Id<ImportDirective>) -> Option<DirectiveUri> {
    let ast = c.ast;
    let unit = ast.cast::<CompilationUnit>(ast.parent(node)?)?;
    let fragment = c
        .tables
        .declared_fragment
        .get(unit.raw())?
        .cast::<dartr_element::LibraryFragment>()?;
    let data = c.ctx.fragment(fragment);
    let index = ast
        .list_raw(ast[unit].directives)
        .iter()
        .filter(|d| ast.is::<ImportDirective>(**d))
        .position(|d| *d == node.raw())?;
    Some(
        data.library_imports
            .iter()
            .filter(|i| !i.is_synthetic)
            .nth(index)?
            .directive
            .uri
            .clone(),
    )
}

/// The import directive of the selection (Dart: the node, or the parent of
/// a string literal).
fn selected_import(c: &ProducerContext<'_>) -> Option<Id<ImportDirective>> {
    let ast = c.ast;
    let mut target = c.node;
    if ast.is::<StringLiteral>(target) {
        target = ast.parent(target)?;
    }
    ast.cast::<ImportDirective>(target)
}

/// The `(relative uri string, relative uri, source uri)` of a
/// `DirectiveUriWithSource`.
fn with_source(uri: &DirectiveUri) -> Option<(String, String, String)> {
    match uri {
        DirectiveUri::Source {
            relative_uri_string,
            relative_uri,
            source,
        }
        | DirectiveUri::Library {
            relative_uri_string,
            relative_uri,
            source,
            ..
        } => Some((
            relative_uri_string.to_string(),
            relative_uri.to_string(),
            source.uri.to_string(),
        )),
        _ => None,
    }
}

producer!(
    ConvertToPackageImport,
    k::CONVERT_TO_PACKAGE_IMPORT,
    Some(&k::CONVERT_TO_PACKAGE_IMPORT_MULTI),
    Automatically,
    assist: Some(&a::CONVERT_TO_PACKAGE_IMPORT),
    |c, builder| {
        let ast = c.ast;
        let Some(import) = selected_import(c) else {
            return;
        };
        let Some((relative_uri_string, _, source_uri)) =
            import_uri(c, import).as_ref().and_then(with_source)
        else {
            return;
        };
        if !source_uri.starts_with("package:") {
            return;
        }
        if relative_uri_string.starts_with("package:") {
            return;
        }
        let range = c.range().node(ast[import].uri);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(range.offset, range.length, &format!("'{source_uri}'"))
        });
    }
);

/// Dart `path.posix.relative(to, from: from)` for absolute paths.
fn posix_relative(to: &str, from: &str) -> String {
    let to: Vec<&str> = to.split('/').filter(|s| !s.is_empty()).collect();
    let from: Vec<&str> = from.split('/').filter(|s| !s.is_empty()).collect();
    let common = to.iter().zip(&from).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<&str> = vec![".."; from.len() - common];
    parts.extend(&to[common..]);
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

producer!(
    ConvertToRelativeImport,
    k::CONVERT_TO_RELATIVE_IMPORT,
    Some(&k::CONVERT_TO_RELATIVE_IMPORT_MULTI),
    Automatically,
    assist: Some(&a::CONVERT_TO_RELATIVE_IMPORT),
    |c, builder| {
        let ast = c.ast;
        let Some(import) = selected_import(c) else {
            return;
        };
        let Some((_, import_uri, _)) = import_uri(c, import).as_ref().and_then(with_source) else {
            return;
        };
        let source_uri = c.resolved.unit().uri.to_string();
        let (Some(source_path), Some(import_path)) = (
            source_uri.strip_prefix("package:"),
            import_uri.strip_prefix("package:"),
        ) else {
            return;
        };
        let source_first = source_path.split('/').next().unwrap_or("");
        let import_first = import_path.split('/').next().unwrap_or("");
        if source_first.is_empty() || import_first.is_empty() || source_first != import_first {
            return;
        }
        let from = match source_path.rfind('/') {
            Some(i) => &source_path[..i],
            None => ".",
        };
        let relative = posix_relative(&format!("/{import_path}"), &format!("/{from}"));
        let range = c.range().node(ast[import].uri);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(range.offset + 1, range.length - 2, &relative)
        });
    }
);

assist!(
    RemoveDigitSeparators,
    a::REMOVE_DIGIT_SEPARATORS,
    |c, builder| {
        let ast = c.ast;
        let literal = if let Some(i) = ast.cast::<IntegerLiteral>(c.node) {
            ast[i].literal
        } else if let Some(d) = ast.cast::<DoubleLiteral>(c.node) {
            ast[d].literal
        } else {
            return;
        };
        let source = c.lexeme(literal);
        let without = dartr_parser::util::strip_separators(source);
        if without == source {
            return;
        }
        let range = c.range().node(c.node);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(range.offset, range.length, &without)
        });
    }
);

/// Dart `AddTypeAnnotation` (`AddTypeAnnotation.new`, `.bulkFixable` and
/// `.forRepresentationField`).
pub struct AddTypeAnnotation {
    pub applicability: Applicability,
    pub for_representation_field: bool,
}

impl AddTypeAnnotation {
    fn apply_change(
        &self,
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        keyword: Option<TokenId>,
        name: TokenId,
        ty: dartr_element::TypeId,
    ) {
        let name_offset = c.token_offset(name);
        let replace_keyword = keyword
            .filter(|k| self.for_representation_field || c.lexeme(*k) == "var")
            .map(|k| c.range().token(k));
        builder.add_dart_file_edit(c.path, |b| {
            if !b.can_write_type_at(name_offset, ty) {
                return;
            }
            match replace_keyword {
                Some(range) => b.add_replacement(range.offset, range.length, |e| {
                    e.write_type(Some(ty), &WriteType::default());
                }),
                None => b.add_insertion(name_offset, |e| {
                    e.write_type(Some(ty), &WriteType::default());
                    e.write(" ");
                }),
            }
        });
    }

    fn for_declared_identifier(
        &self,
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        declared: Id<DeclaredIdentifier>,
    ) {
        let ast = c.ast;
        let ctx = c.ctx;
        if ast[declared].type_.is_some() {
            return;
        }
        let Some(element) = c.locator().declared_element(declared) else {
            return;
        };
        let ty = dartr_resolver::element_ext::variable_type(ctx, element);
        if !matches!(
            ctx.ty(ty),
            TypeKind::Interface { .. }
                | TypeKind::Function(_)
                | TypeKind::Record { .. }
                | TypeKind::TypeParameter { .. }
        ) {
            return;
        }
        self.apply_change(c, builder, ast[declared].keyword, ast[declared].name, ty);
    }

    fn for_regular_formal_parameter(
        &self,
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        parameter: Id<RegularFormalParameter>,
    ) {
        let ast = c.ast;
        let ctx = c.ctx;
        if ast[parameter].type_.is_some() {
            return;
        }
        let Some(name) = ast[parameter].name else {
            return;
        };
        let Some(element) = c.locator().declared_element(parameter) else {
            return;
        };
        let ty = dartr_resolver::element_ext::variable_type(ctx, element);
        if !matches!(
            ctx.ty(ty),
            TypeKind::Interface { .. } | TypeKind::Record { .. }
        ) {
            return;
        }
        let keyword = if self.for_representation_field {
            ast[parameter].const_final_or_var_keyword
        } else {
            None
        };
        self.apply_change(c, builder, keyword, name, ty);
    }

    /// Dart `_typeForVariable`.
    fn type_for_variable(
        c: &ProducerContext<'_>,
        variable: Id<VariableDeclaration>,
    ) -> Option<dartr_element::TypeId> {
        let ast = c.ast;
        if let Some(initializer) = ast[variable].initializer {
            return static_type(c, initializer.raw());
        }
        let statement = ast.parent(variable).and_then(|l| ast.parent(l))?;
        let statement = ast.cast::<VariableDeclarationStatement>(statement)?;
        let block = ast.cast::<Block>(ast.parent(statement)?)?;
        let element = c.locator().declared_element(variable)?;
        if element.tag() != dartr_element::Tag::LocalVariable {
            return None;
        }
        let statements = ast.list(ast[block].statements);
        let index = statements.iter().position(|s| s.raw() == statement.raw())?;
        let mut assigned: Vec<dartr_element::TypeId> = Vec::new();
        for s in &statements[index + 1..] {
            super::variables::visit(ast, s.raw(), &mut |n| {
                let Some(a) = ast.cast::<AssignmentExpression>(n) else {
                    return;
                };
                let lhs = ast[a].left_hand_side;
                let Some(id) = ast.cast::<SimpleIdentifier>(lhs) else {
                    return;
                };
                if c.locator().element(id) != Some(element) {
                    return;
                }
                if let Some(t) = static_type(c, ast[a].right_hand_side.raw()) {
                    if !assigned.contains(&t) {
                        assigned.push(t);
                    }
                }
            });
        }
        let mut best = *assigned.first()?;
        let ts = dartr_typesystem::type_system::TypeSystem::new(*c.ctx);
        for t in &assigned[1..] {
            best = ts.least_upper_bound(best, *t);
        }
        Some(best)
    }

    fn for_variable_declaration(
        &self,
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        list: Id<VariableDeclarationList>,
    ) {
        let ast = c.ast;
        let ctx = c.ctx;
        if ast[list].type_.is_some() {
            return;
        }
        let variables = ast.list(ast[list].variables);
        let Some(&variable) = variables.first() else {
            return;
        };
        if c.selection_offset > c.token_end(ast[variable].name) {
            return;
        }
        let Some(ty) = Self::type_for_variable(c, variable) else {
            return;
        };
        for v in &variables[1..] {
            if Self::type_for_variable(c, *v) != Some(ty) {
                return;
            }
        }
        let interface =
            matches!(ctx.ty(ty), TypeKind::Interface { .. }) && !ctx.is_dart_core_null(ty);
        if !interface
            && !matches!(
                ctx.ty(ty),
                TypeKind::Function(_) | TypeKind::Record { .. } | TypeKind::TypeParameter { .. }
            )
        {
            return;
        }
        self.apply_change(c, builder, ast[list].keyword, ast[variable].name, ty);
    }

    fn typed_literal(
        &self,
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        node: NodeId,
    ) {
        let ast = c.ast;
        let ctx = c.ctx;
        let Some(ty) = static_type(c, node) else {
            return;
        };
        let TypeKind::Interface { args, .. } = *ctx.ty(ty) else {
            return;
        };
        let offset = if let Some(l) = ast.cast::<ListLiteral>(node) {
            c.token_offset(ast[l].left_bracket)
        } else if let Some(l) = ast.cast::<SetOrMapLiteral>(node) {
            c.token_offset(ast[l].left_bracket)
        } else {
            return;
        };
        let types: Vec<dartr_element::TypeId> = ctx.list(args).to_vec();
        builder.add_dart_file_edit(c.path, |b| {
            b.add_insertion(offset, |e| {
                e.write("<");
                let options = WriteType {
                    should_write_dynamic: true,
                    ..Default::default()
                };
                for (i, t) in types.iter().enumerate() {
                    if i > 0 {
                        e.write(", ");
                    }
                    e.write_type(Some(*t), &options);
                }
                e.write(">");
            });
        });
    }
}

impl CorrectionProducer for AddTypeAnnotation {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::ADD_TYPE_ANNOTATION)
    }

    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::ADD_TYPE_ANNOTATION_MULTI)
    }

    fn assist_kind(&self) -> Option<&'static FixKind> {
        Some(&a::ADD_TYPE_ANNOTATION)
    }

    fn applicability(&self) -> Applicability {
        self.applicability
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let node = c.node;
        if let Some(p) = ast.cast::<RegularFormalParameter>(node) {
            self.for_regular_formal_parameter(c, builder, p);
            return;
        }
        if let Some(p) = ast.cast::<DeclaredVariablePattern>(node) {
            let Some(ty) = c
                .tables
                .pattern_info
                .get(node)
                .and_then(|i| i.matched_value_type)
            else {
                return;
            };
            self.apply_change(c, builder, ast[p].keyword, ast[p].name, ty);
            return;
        }
        let type_arguments = if let Some(l) = ast.cast::<ListLiteral>(node) {
            Some(ast[l].type_arguments)
        } else {
            ast.cast::<SetOrMapLiteral>(node)
                .map(|l| ast[l].type_arguments)
        };
        if let Some(type_arguments) = type_arguments {
            let synthetic =
                type_arguments.is_some_and(|t| ast.tokens.get(ast.begin_token(t)).is_synthetic());
            if type_arguments.is_none() || synthetic {
                self.typed_literal(c, builder, node);
                return;
            }
        }
        let mut current = Some(node);
        while let Some(n) = current {
            if let Some(list) = ast.cast::<VariableDeclarationList>(n) {
                self.for_variable_declaration(c, builder, list);
                return;
            } else if let Some(d) = ast.cast::<DeclaredIdentifier>(n) {
                self.for_declared_identifier(c, builder, d);
                return;
            } else if let Some(f) = ast.cast::<ForStatement>(n) {
                let parts = ast[f].for_loop_parts;
                if let Some(p) = ast.cast::<ForEachPartsWithDeclaration>(parts) {
                    if ast.offset(node) < ast.offset(ast[p].iterable) {
                        self.for_declared_identifier(c, builder, ast[p].loop_variable);
                    }
                }
                return;
            }
            current = ast.parent(n);
        }
    }
}

/// Dart `UseCurlyBraces` (`UseCurlyBraces.new` and `.nonBulk`).
pub struct UseCurlyBraces {
    pub applicability: Applicability,
}

/// The edits of [UseCurlyBraces]: `(offset, length, text)`.
struct CurlyEdits<'c, 'a> {
    c: &'c ProducerContext<'a>,
    edits: Vec<(u32, u32, String)>,
}

impl CurlyEdits<'_, '_> {
    /// Dart `_endAfterComments`.
    fn end_after_comments(&self, node: NodeId) -> u32 {
        let c = self.c;
        let ast = c.ast;
        let mut end = ast.end(node);
        let next = ast.tokens.next(ast.end_token(node));
        if next.is_some() {
            let comments = ast.tokens.get(next).preceding_comments;
            if comments.is_some() {
                let comment = ast.tokens.get(comments);
                if c.utils.get_line_this(end) == c.utils.get_line_this(comment.offset) {
                    end = comment.end();
                }
            }
        }
        end
    }

    /// Dart `_replaceLeftParenthesis`.
    fn replace_left(&mut self, left: TokenId, right: NodeId, indent: &str) {
        let c = self.c;
        let ast = c.ast;
        let begin = ast.begin_token(right);
        let comments = ast.tokens.get(begin).preceding_comments;
        let start = if comments.is_some() {
            c.token_offset(comments)
        } else {
            ast.offset(right)
        };
        let left_end = c.token_end(left);
        let eol = c.eol().to_string();
        self.edits
            .push((left_end, start - left_end, format!(" {{{eol}{indent}")));
    }

    /// Dart `_replace`.
    fn replace(&mut self, left: TokenId, right: NodeId, indent: &str, prefix: &str) {
        self.replace_left(left, right, indent);
        let eol = self.c.eol().to_string();
        let end = self.end_after_comments(right);
        self.edits.push((end, 0, format!("{eol}{prefix}}}")));
    }

    /// Dart `_replaceRange`.
    fn replace_range(
        &mut self,
        left: TokenId,
        node: NodeId,
        right: TokenId,
        indent: &str,
        prefix: &str,
    ) {
        self.replace_left(left, node, indent);
        let eol = self.c.eol().to_string();
        let end = self.end_after_comments(node);
        let right_offset = self.c.token_offset(right);
        self.edits
            .push((end, right_offset - end, format!("{eol}{prefix}}} ")));
    }
}

impl UseCurlyBraces {
    fn prefix_indent(c: &ProducerContext<'_>, node: NodeId) -> (String, String) {
        let prefix = c.utils.get_line_prefix(c.ast.offset(node));
        let indent = format!("{prefix}{}", c.utils.one_indent());
        (prefix, indent)
    }

    fn if_statement(
        c: &ProducerContext<'_>,
        e: &mut CurlyEdits<'_, '_>,
        node: Id<IfStatement>,
        then_or_else: Option<NodeId>,
    ) {
        let ast = c.ast;
        if let Some(parent) = ast.parent(node).and_then(|p| ast.cast::<IfStatement>(p)) {
            if ast[parent].else_statement.map(|s| s.raw()) == Some(node.raw()) {
                return;
            }
        }
        let (prefix, indent) = Self::prefix_indent(c, node.raw());
        let then_statement = ast[node].then_statement.raw();
        let else_keyword = ast[node].else_keyword;
        if !ast.is::<Block>(then_statement)
            && (then_or_else.is_none() || then_or_else == Some(then_statement))
        {
            match else_keyword {
                None => e.replace(
                    ast[node].right_parenthesis,
                    then_statement,
                    &indent,
                    &prefix,
                ),
                Some(k) => e.replace_range(
                    ast[node].right_parenthesis,
                    then_statement,
                    k,
                    &indent,
                    &prefix,
                ),
            }
        }
        let (Some(else_keyword), Some(else_statement)) = (else_keyword, ast[node].else_statement)
        else {
            return;
        };
        let else_statement = else_statement.raw();
        if ast.is::<Block>(else_statement) || ast.is::<IfStatement>(else_statement) {
            return;
        }
        if then_or_else.is_none() || then_or_else == Some(else_statement) {
            e.replace(else_keyword, else_statement, &indent, &prefix);
        }
    }
}

impl CorrectionProducer for UseCurlyBraces {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::ADD_CURLY_BRACES)
    }

    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::ADD_CURLY_BRACES_MULTI)
    }

    fn assist_kind(&self) -> Option<&'static FixKind> {
        Some(&a::USE_CURLY_BRACES)
    }

    fn applicability(&self) -> Applicability {
        self.applicability
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let statement = ast
            .this_or_ancestor_of_type::<Statement>(c.node)
            .map(|s| s.raw());
        let parent = statement.and_then(|s| ast.parent(s));
        let mut e = CurlyEdits {
            c,
            edits: Vec::new(),
        };
        let pick = |t: fn(&Ast, NodeId) -> bool| -> Option<NodeId> {
            if statement.is_some_and(|s| t(ast, s)) {
                statement
            } else if parent.is_some_and(|p| t(ast, p)) {
                parent
            } else {
                None
            }
        };
        if let Some(d) =
            pick(|ast, n| ast.is::<DoStatement>(n)).and_then(|n| ast.cast::<DoStatement>(n))
        {
            let body = ast[d].body.raw();
            if ast.is::<Block>(body) {
                return;
            }
            let (prefix, indent) = Self::prefix_indent(c, d.raw());
            e.replace_range(
                ast[d].do_keyword,
                body,
                ast[d].while_keyword,
                &indent,
                &prefix,
            );
        } else if let Some(f) =
            pick(|ast, n| ast.is::<ForStatement>(n)).and_then(|n| ast.cast::<ForStatement>(n))
        {
            let body = ast[f].body.raw();
            if ast.is::<Block>(body) {
                return;
            }
            let (prefix, indent) = Self::prefix_indent(c, f.raw());
            e.replace(ast[f].right_parenthesis, body, &indent, &prefix);
        } else if let Some(i) = statement.and_then(|s| ast.cast::<IfStatement>(s)) {
            let else_selected = match (ast[i].else_keyword, ast[i].else_statement) {
                (Some(k), Some(_)) => c.range().token(k).contains(c.selection_offset),
                _ => false,
            };
            if else_selected {
                let else_statement = ast[i].else_statement.map(|s| s.raw());
                Self::if_statement(c, &mut e, i, else_statement);
            } else {
                Self::if_statement(c, &mut e, i, None);
            }
        } else if let Some(i) = parent.and_then(|p| ast.cast::<IfStatement>(p)) {
            Self::if_statement(c, &mut e, i, statement);
        } else if let Some(w) =
            pick(|ast, n| ast.is::<WhileStatement>(n)).and_then(|n| ast.cast::<WhileStatement>(n))
        {
            let body = ast[w].body.raw();
            if ast.is::<Block>(body) {
                return;
            }
            let (prefix, indent) = Self::prefix_indent(c, w.raw());
            e.replace(ast[w].right_parenthesis, body, &indent, &prefix);
        } else {
            return;
        }
        let edits = e.edits;
        builder.add_dart_file_edit(c.path, |b| {
            for (offset, length, text) in &edits {
                b.add_simple_replacement(*offset, *length, text);
            }
        });
    }
}
