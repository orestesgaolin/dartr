// Dart source: pkg/analysis_server/lib/src/services/snippets/dart_snippet_request.dart
// Dart source: pkg/analysis_server/lib/src/services/snippets/snippet_manager.dart
// Dart source: pkg/analysis_server/lib/src/services/snippets/dart/*.dart (the non-Flutter producers)
// Dart source: pkg/analysis_server/lib/src/lsp/mapping.dart (snippetToCompletionItem)

//! The snippets of completion (Dart `DartSnippetManager`): unranked items
//! after the ranked suggestions.

use dartr_ast::*;
use serde_json::{Map, Value, json};

use super::Request;
use super::target::TokenExt;
use crate::fuzzy::FuzzyMatcher;

/// Dart `SnippetContext`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SnippetContext {
    AtTopLevel,
    InAnnotation,
    InBlock,
    InClass,
    InComment,
    InConstantExpression,
    InConstructorInvocation,
    InDotShorthand,
    InExpression,
    InEnumConstants,
    InEnumMembers,
    InIdentifierDeclaration,
    InName,
    InPattern,
    InQualifiedMemberAccess,
    InStatement,
    InString,
}

/// Dart `DartSnippetRequest._getContext`.
pub fn snippet_context(q: &Request<'_, '_>) -> SnippetContext {
    use SnippetContext::*;
    let ast = q.ast;
    let target = &q.target;
    match target.entity {
        Some(Entity::Token(t)) => {
            let before = ast.tokens.get(t).before_synthetic.get().unwrap_or(t);
            let ty = ast.t_ty(before);
            if ty == dartr_syntax::TokenType::MULTI_LINE_COMMENT || ty == dartr_syntax::TokenType::SINGLE_LINE_COMMENT {
                return InComment;
            }
            if ty == dartr_syntax::TokenType::STRING
                || ty == dartr_syntax::TokenType::STRING_INTERPOLATION_EXPRESSION
                || ty == dartr_syntax::TokenType::STRING_INTERPOLATION_IDENTIFIER
            {
                return InString;
            }
        }
        Some(Entity::Node(n)) => {
            if let Some(a) = ast.cast::<NamedArgument>(n) {
                let name = ast[a].name;
                if target.offset >= ast.t_offset(name) && target.offset <= ast.t_end(name) {
                    return InName;
                }
            }
        }
        None => {}
    }
    let mut node = Some(target.containing_node);
    while let Some(n) = node {
        if let Some(r) = ast.cast::<ReturnStatement>(n) {
            if target.offset > ast.t_end(ast[r].return_keyword) {
                return InExpression;
            }
        }
        match ast.kind(n) {
            NodeKind::Comment => return InComment,
            NodeKind::VariableDeclaration => {
                return if super::keyword::variable_declaration_is_const(ast, n) {
                    InConstantExpression
                } else {
                    InExpression
                };
            }
            NodeKind::VariableDeclarationList => return InIdentifierDeclaration,
            NodeKind::DotShorthandInvocation
            | NodeKind::DotShorthandConstructorInvocation
            | NodeKind::DotShorthandPropertyAccess => return InDotShorthand,
            NodeKind::PropertyAccess
            | NodeKind::FieldFormalParameter
            | NodeKind::PrefixedIdentifier
            | NodeKind::ConstructorReference => return InQualifiedMemberAccess,
            NodeKind::InstanceCreationExpression => return InConstructorInvocation,
            NodeKind::Block => return InBlock,
            NodeKind::SwitchExpression => return InPattern,
            NodeKind::Annotation => return InAnnotation,
            NodeKind::BlockFunctionBody => return InBlock,
            NodeKind::ClassDeclaration | NodeKind::ExtensionDeclaration | NodeKind::MixinDeclaration => {
                return InClass;
            }
            NodeKind::EnumConstantArguments => return InConstantExpression,
            NodeKind::EnumDeclaration => {
                let e = ast.cast::<EnumDeclaration>(n).unwrap();
                let body = ast[e].body.raw();
                let semicolon = if let Some(b) = ast.cast::<BlockEnumBody>(body) {
                    ast[b].semicolon
                } else {
                    ast.cast::<EmptyEnumBody>(body).map(|b| ast[b].semicolon)
                };
                return if semicolon.is_none_or(|s| target.offset <= ast.t_offset(s)) {
                    InEnumConstants
                } else {
                    InEnumMembers
                };
            }
            _ => {}
        }
        if ast.is::<StringLiteral>(n) {
            return InString;
        }
        if ast.is::<Statement>(n) {
            return InStatement;
        }
        if ast.is::<Expression>(n) {
            return if dartr_resolver::ast_ext::in_constant_context(ast, n) {
                InConstantExpression
            } else {
                InExpression
            };
        }
        node = ast.parent(n);
    }
    AtTopLevel
}

/// One snippet: (prefix, label, documentation, snippet text).
struct Snippet {
    prefix: &'static str,
    label: &'static str,
    documentation: &'static str,
    text: String,
}

/// Dart `CorrectionUtils.getLinePrefix(offset)`: the whitespace at the
/// start of the line of [offset].
fn line_prefix(content: &str, offset: u32) -> String {
    let units: Vec<u16> = content.encode_utf16().collect();
    let mut start = (offset as usize).min(units.len());
    while start > 0 && units[start - 1] != b'\n' as u16 && units[start - 1] != b'\r' as u16 {
        start -= 1;
    }
    let mut end = start;
    while end < units.len() && (units[end] == b' ' as u16 || units[end] == b'\t' as u16) {
        end += 1;
    }
    String::from_utf16_lossy(&units[start..end])
}

/// The snippets of the producers of [context] (Dart
/// `DartSnippetManager.producerGenerators`), without the Flutter widget
/// snippets.
fn snippets_for(q: &Request<'_, '_>, context: SnippetContext) -> Vec<Snippet> {
    use SnippetContext::*;
    let indent = line_prefix(q.content, q.offset);
    let eol = q.end_of_line();
    let i = |s: &str| format!("{indent}{s}");
    let block = |head: String| format!("{head}{eol}{}$0{eol}{}", i("  "), i("}"));
    let fun = || Snippet {
        prefix: "fun",
        label: "fun",
        documentation: "Insert a function definition.",
        text: block("${1:void} ${2:name}(${3:params}) {".to_string()),
    };
    match context {
        AtTopLevel => {
            let args = if q.in_test_directory { "" } else { "List<String> args" };
            vec![
                Snippet {
                    prefix: "class",
                    label: "class",
                    documentation: "Insert a class definition.",
                    text: block("class ${1:ClassName} {".to_string()),
                },
                fun(),
                Snippet {
                    prefix: "main",
                    label: "main()",
                    documentation: "Insert a main function, used as an entry point.",
                    text: format!("void main({args}) {{{eol}  $0{eol}}}"),
                },
            ]
        }
        InBlock => {
            let var_or_final = if q.style.make_locals_final { "final" } else { "var" };
            let mut out = vec![
                Snippet {
                    prefix: "do",
                    label: "do while",
                    documentation: "Insert a do-while loop.",
                    text: format!("do {{{eol}{}$0{eol}{}", i("  "), i("} while (${1:condition});")),
                },
                Snippet {
                    prefix: "forin",
                    label: "for in",
                    documentation: "Insert a for-in loop.",
                    text: block(format!("for ({var_or_final} ${{1:element}} in ${{2:collection}}) {{")),
                },
                Snippet {
                    prefix: "for",
                    label: "for",
                    documentation: "Insert a for loop.",
                    text: block("for (var i = 0; i < ${1:count}; i++) {".to_string()),
                },
                fun(),
                Snippet {
                    prefix: "ife",
                    label: "ife",
                    documentation: "Insert an if/else statement.",
                    text: format!(
                        "if (${{1:condition}}) {{{eol}{}$0{eol}{}{eol}{}{eol}{}",
                        i("  "),
                        i("} else {"),
                        i("  "),
                        i("}")
                    ),
                },
                Snippet {
                    prefix: "if",
                    label: "if",
                    documentation: "Insert an if statement.",
                    text: block("if (${1:condition}) {".to_string()),
                },
                Snippet {
                    prefix: "switch",
                    label: "switch statement",
                    documentation: "Insert a switch statement.",
                    text: format!(
                        "switch (${{1:expression}}) {{{eol}{}{eol}{}$0{eol}{}{eol}{}{eol}{}",
                        i("  case ${2:value}:"),
                        i("    "),
                        i("    break;"),
                        i("  default:"),
                        i("}")
                    ),
                },
            ];
            if q.in_test_directory {
                out.push(Snippet {
                    prefix: "test",
                    label: "test",
                    documentation: "Insert a test block.",
                    text: format!("test('${{1:test name}}', () {{{eol}{}$0{eol}{}", i("  "), i("});")),
                });
                out.push(Snippet {
                    prefix: "group",
                    label: "group",
                    documentation: "Insert a test group block.",
                    text: format!("group('${{1:group name}}', () {{{eol}{}$0{eol}{}", i("  "), i("});")),
                });
            }
            out.push(Snippet {
                prefix: "try",
                label: "try",
                documentation: "Insert a try/catch statement.",
                text: format!(
                    "try {{{eol}{}$0{eol}{}{eol}{}{eol}{}",
                    i("  "),
                    i("} catch (${1:e}) {"),
                    i("  "),
                    i("}")
                ),
            });
            out.push(Snippet {
                prefix: "while",
                label: "while",
                documentation: "Insert a while loop.",
                text: block("while (${1:condition}) {".to_string()),
            });
            out
        }
        InClass | InEnumMembers => vec![fun()],
        InExpression => vec![Snippet {
            prefix: "switch",
            label: "switch expression",
            documentation: "Insert a switch expression.",
            text: format!(
                "switch (${{1:expression}}) {{{eol}{}$0{eol}{}",
                i("  ${2:pattern} => ${3:value},"),
                i("}")
            ),
        }],
        _ => Vec::new(),
    }
}

/// Dart `_getDartSnippetItems` + `snippetToCompletionItem`: the snippet
/// items that match [prefix].
pub fn snippet_items(
    q: &Request<'_, '_>,
    prefix: &str,
    edit_range: &Value,
    default_range: Option<&Value>,
    formats: &Option<Vec<String>>,
    as_is_insert_mode: bool,
) -> Vec<Value> {
    let context = snippet_context(q);
    let mut matcher = FuzzyMatcher::new(prefix);
    let mut out = Vec::new();
    for s in snippets_for(q, context) {
        if matcher.score(s.prefix) <= 0.0 {
            continue;
        }
        let mut item = Map::new();
        item.insert("label".into(), json!(s.label));
        if s.prefix != s.label {
            item.insert("filterText".into(), json!(s.prefix));
        }
        item.insert("kind".into(), json!(15));
        item.insert(
            "documentation".into(),
            crate::server::Server::markup_content_or_string(formats, s.documentation.to_string()),
        );
        item.insert("sortText".into(), json!(format!("zzz{}", s.prefix)));
        item.insert("insertTextFormat".into(), json!(2));
        if as_is_insert_mode {
            item.insert("insertTextMode".into(), json!(1));
        }
        if default_range == Some(edit_range) {
            if s.text != s.label {
                item.insert("textEditText".into(), json!(s.text));
            }
        } else {
            item.insert("textEdit".into(), json!({"range": edit_range, "newText": s.text}));
        }
        // Dart `_FuzzyScoreHelper.completionItemMatches`.
        let filter = if s.prefix != s.label { s.prefix } else { s.label };
        if matcher.score(filter) > 0.0 {
            out.push(Value::Object(item));
        }
    }
    out
}
