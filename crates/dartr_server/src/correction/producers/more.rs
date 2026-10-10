// Dart source: pkg/analysis_server/lib/src/services/correction/dart/make_class_abstract.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_multiline_string.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_documentation_into_block.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_into_async_body.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/create_no_such_method.dart
// Dart source: pkg/analyzer_plugin/lib/src/utilities/change_builder/change_builder_dart.dart (convertFunctionFromSyncToAsync, replaceTypeWithFuture)

//! More fix and assist producers.

use dartr_ast::*;
use dartr_element::TypeKind;
use dartr_syntax::{TokenId, TokenType};
use dartr_typesystem::type_ext::TypeExt;

use super::super::change_builder::{ChangeBuilder, FileEditBuilder};
use super::super::dart_edit::WriteType;
use super::super::fix_kind::FixKind;
use super::super::generated::assist_kinds as a;
use super::super::generated::fix_kinds as k;
use super::super::producer::*;
use super::assists::enclosing_function_body;
use super::convert_quotes::string_facts;
use super::simple::assist;

/// Dart `MakeClassAbstract`.
pub struct MakeClassAbstract {
    class_name: String,
}

impl MakeClassAbstract {
    pub fn new() -> Self {
        MakeClassAbstract {
            class_name: String::new(),
        }
    }
}

impl CorrectionProducer for MakeClassAbstract {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::MAKE_CLASS_ABSTRACT)
    }

    fn fix_arguments(&self) -> Vec<String> {
        vec![self.class_name.clone()]
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let Some(class) = ast.this_or_ancestor_of_type::<ClassDeclaration>(c.node) else {
            return;
        };
        let name = dartr_resolver::error::support::class_name_token(ast, ast[class].name_part);
        self.class_name = c.lexeme(name).to_string();
        let first = super::super::utils::first_token_after_comment_and_metadata(ast, class.raw());
        let offset = c.token_offset(first);
        builder.add_dart_file_edit(c.path, |b| b.add_simple_insertion(offset, "abstract "));
    }
}

assist!(
    ConvertToMultilineString,
    a::CONVERT_TO_MULTILINE_STRING,
    |c, builder| {
        let ast = c.ast;
        let mut target = c.node;
        if ast.is::<InterpolationExpression>(target) || ast.is::<InterpolationString>(target) {
            let Some(p) = ast.parent(target) else { return };
            target = p;
        }
        // Dart `SingleStringLiteral`: the facts of the first token.
        let first_token = if let Some(s) = ast.cast::<SimpleStringLiteral>(target) {
            ast[s].literal
        } else if ast.is::<StringInterpolation>(target) {
            ast.begin_token(target)
        } else {
            return;
        };
        if ast.tokens.get(first_token).is_synthetic() {
            return;
        }
        let facts = string_facts(c.lexeme(first_token));
        if facts.is_multiline {
            return;
        }
        let new_quote = if facts.is_single_quoted {
            "'''"
        } else {
            "\"\"\""
        };
        let offset = ast.offset(target) + if facts.is_raw { 1 } else { 0 };
        let end = ast.end(target);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_replacement(offset, 1, |e| e.writeln(new_quote));
            b.add_simple_replacement(end - 1, 1, new_quote);
        });
    }
);

assist!(
    ConvertDocumentationIntoBlock,
    a::CONVERT_DOCUMENTATION_INTO_BLOCK,
    |c, builder| {
        let ast = c.ast;
        let Some(comment) = ast.this_or_ancestor_of_type::<Comment>(c.node) else {
            return;
        };
        let tokens = ast.token_list(ast[comment].tokens).to_vec();
        if tokens.is_empty()
            || tokens.iter().any(|t| {
                let token = ast.tokens.get(*t);
                !token.is_doc_comment() || ast.tokens.ty(*t) != TokenType::SINGLE_LINE_COMMENT
            })
        {
            return;
        }
        let prefix = c.utils.get_node_prefix(comment.raw());
        let range = c.range().node(comment);
        let lines: Vec<String> = tokens
            .iter()
            .map(|t| c.lexeme(*t)[3..].to_string())
            .collect();
        builder.add_dart_file_edit(c.path, |b| {
            b.add_replacement(range.offset, range.length, |e| {
                e.writeln("/**");
                for line in &lines {
                    e.write(&prefix);
                    e.write(" *");
                    e.writeln(line);
                }
                e.write(&prefix);
                e.write(" */");
            });
        });
    }
);

/// Dart `replaceTypeWithFuture`.
fn replace_type_with_future(
    c: &ProducerContext<'_>,
    b: &mut FileEditBuilder<'_, '_>,
    annotation: Option<Id<TypeAnnotation>>,
) {
    let Some(annotation) = annotation else { return };
    let ctx = c.ctx;
    let Some(ty) = c.tables.annotation_type.get(annotation.raw()).copied() else {
        return;
    };
    if matches!(ctx.ty(ty), TypeKind::Dynamic) || ctx.is_dart_async_future(ty) {
        return;
    }
    let ts = dartr_typesystem::type_system::TypeSystem::new(*ctx);
    let value = ts.flatten(ty);
    let future = ctx.tp.future_type(ctx, value);
    let range = c.range().node(annotation);
    b.add_replacement(range.offset, range.length, |e| {
        if !e.write_type(Some(future), &WriteType::default()) {
            e.write("void");
        }
    });
}

/// Dart `convertFunctionFromSyncToAsync`.
pub fn convert_function_from_sync_to_async(
    c: &ProducerContext<'_>,
    b: &mut FileEditBuilder<'_, '_>,
    body: Id<FunctionBody>,
) {
    let ast = c.ast;
    if !ast.is::<EmptyFunctionBody>(body) {
        let begin = ast.begin_token(body);
        let previous = ast.tokens.previous(begin);
        let fused: bool = previous.is_some() && c.token_end(previous) == c.token_offset(begin);
        b.add_insertion(ast.offset(body), |e| {
            if fused {
                e.write(" ");
            }
            e.write("async ");
        });
    }
    // Dart `_replaceReturnTypeWithFuture`.
    let mut node: Option<NodeId> = Some(body.raw());
    while let Some(n) = node {
        node = ast.parent(n);
        let Some(p) = node else { break };
        if let Some(f) = ast.cast::<FunctionDeclaration>(p) {
            replace_type_with_future(c, b, ast[f].return_type);
            return;
        } else if let Some(f) = ast.cast::<FunctionExpression>(p) {
            if ast
                .parent(f)
                .is_none_or(|g| !ast.is::<FunctionDeclaration>(g))
            {
                return;
            }
        } else if let Some(m) = ast.cast::<MethodDeclaration>(p) {
            replace_type_with_future(c, b, ast[m].return_type);
            return;
        }
    }
}

fn body_keyword(ast: &Ast, body: NodeId) -> Option<TokenId> {
    if let Some(b) = ast.cast::<BlockFunctionBody>(body) {
        ast[b].keyword
    } else if let Some(b) = ast.cast::<ExpressionFunctionBody>(body) {
        ast[b].keyword
    } else {
        None
    }
}

assist!(
    ConvertIntoAsyncBody,
    a::CONVERT_INTO_ASYNC_BODY,
    |c, builder| {
        let ast = c.ast;
        let Some(body) = enclosing_function_body(c) else {
            return;
        };
        if ast.is::<EmptyFunctionBody>(body) || body_keyword(ast, body.raw()).is_some() {
            return;
        }
        if let Some(b) = ast.cast::<BlockFunctionBody>(body) {
            let left = ast[ast[b].block].left_bracket;
            if c.selection_offset > c.token_end(left) {
                return;
            }
        }
        if ast.is::<ExpressionFunctionBody>(body) {
            if c.selection_offset > c.token_end(ast.begin_token(body)) {
                return;
            }
        }
        let Some(parent) = ast.parent(body) else {
            return;
        };
        if ast.is::<ConstructorDeclaration>(parent) || ast.is::<PrimaryConstructorBody>(parent) {
            return;
        }
        if let Some(f) = ast.cast::<FunctionExpression>(parent) {
            if ast
                .parent(f)
                .is_none_or(|g| !ast.is::<FunctionDeclaration>(g))
            {
                return;
            }
        }
        builder.add_dart_file_edit(c.path, |b| convert_function_from_sync_to_async(c, b, body));
    }
);

/// Dart `CreateNoSuchMethod`.
pub struct CreateNoSuchMethod;

impl CorrectionProducer for CreateNoSuchMethod {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::CREATE_NO_SUCH_METHOD)
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let Some(class) = ast
            .parent(c.node)
            .filter(|p| ast.is::<ClassDeclaration>(*p))
        else {
            return;
        };
        let ctx = c.ctx;
        let invocation = ctx
            .library_by_uri("dart:core")
            .and_then(|l| super::import_library::exported_element(ctx, l, "Invocation"))
            .and_then(|e| e.cast::<dartr_element::InterfaceElement>())
            .map(|element| {
                let args = ctx.intern_list(&[]);
                ctx.intern(TypeKind::Interface {
                    element,
                    args,
                    nullability: dartr_element::Nullability::None,
                    alias: None,
                })
            });
        let indent = c.utils.one_indent().to_string();
        builder.add_dart_file_edit(c.path, |b| {
            b.insert_into_unit_member(class, None, true, |e| {
                e.select_here();
                e.write("@override");
                e.newline();
                e.write(&indent);
                // Dart `writeFunctionDeclaration` with the return type
                // `dynamic` (not written).
                e.write("noSuchMethod");
                e.write("(");
                if e.write_type(invocation, &WriteType::default()) {
                    e.write(" ");
                }
                e.write("invocation");
                e.write(")");
                e.write(" ");
                e.write("=> super.noSuchMethod(invocation);");
            });
        });
    }
}
