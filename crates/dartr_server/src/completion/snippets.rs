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
            if ty == dartr_syntax::TokenType::MULTI_LINE_COMMENT
                || ty == dartr_syntax::TokenType::SINGLE_LINE_COMMENT
            {
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
            NodeKind::ClassDeclaration
            | NodeKind::ExtensionDeclaration
            | NodeKind::MixinDeclaration => {
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

/// One snippet: (prefix, label, documentation, snippet text, the URIs of
/// the libraries to import).
struct Snippet {
    prefix: &'static str,
    label: &'static str,
    documentation: &'static str,
    text: String,
    imports: Vec<String>,
}

/// The URI of the library of the Flutter widget classes (Dart
/// `widgetsUri`).
const WIDGETS_URI: &str = "package:flutter/widgets.dart";

/// The references of the Flutter snippets (Dart
/// `FlutterSnippetProducer.getClass` and `DartEditBuilder.writeReference`):
/// the prefix of each Flutter name, and whether the widgets library must be
/// imported.
struct FlutterRefs {
    prefixes: std::collections::HashMap<&'static str, String>,
    import_widgets: bool,
}

impl FlutterRefs {
    /// The reference to [name], with the prefix of its import.
    fn r(&self, name: &str) -> String {
        format!(
            "{}{name}",
            self.prefixes.get(name).map(String::as_str).unwrap_or("")
        )
    }
}

/// The names that the Flutter snippets reference.
const FLUTTER_NAMES: [&str; 9] = [
    "Widget",
    "Placeholder",
    "StatefulWidget",
    "StatelessWidget",
    "State",
    "BuildContext",
    "Key",
    "AnimationController",
    "SingleTickerProviderStateMixin",
];

/// Dart `FlutterSnippetProducer.isValid` (the Flutter classes are found)
/// and the import prefix of each class (Dart `_getImportElement`).
fn flutter_refs(q: &Request<'_, '_>) -> Option<FlutterRefs> {
    let ctx = q.ctx;
    let widgets = ctx.library_by_uri(WIDGETS_URI)?;
    let namespace = ctx.get(widgets).export_namespace.try_get()?;
    let mut elements = Vec::new();
    for name in FLUTTER_NAMES {
        let element = namespace
            .defined_names
            .iter()
            .find(|(n, _)| ctx.name_str(**n) == name)
            .map(|(_, e)| *e)?;
        elements.push((name, element));
    }
    let first = ctx.get(q.library).first_fragment();
    let imports: Vec<_> = ctx
        .fragment(first)
        .library_imports
        .iter()
        .filter_map(|i| {
            let (_, names) = super::declaration::import_namespace(ctx, i)?;
            Some((super::declaration::import_prefix_name(ctx, i), names))
        })
        .collect();
    let mut prefixes = std::collections::HashMap::new();
    let mut import_widgets = false;
    for (name, element) in elements {
        let found = imports
            .iter()
            .find(|(_, names)| names.iter().any(|(n, e)| n == name && *e == element));
        match found {
            Some((prefix, _)) => {
                prefixes.insert(
                    name,
                    prefix.as_ref().map(|p| format!("{p}.")).unwrap_or_default(),
                );
            }
            None => import_widgets = true,
        }
    }
    Some(FlutterRefs {
        prefixes,
        import_widgets,
    })
}

/// The Flutter widget snippets (Dart `FlutterStatefulWidget`,
/// `FlutterStatefulWidgetWithAnimationController` and
/// `FlutterStatelessWidget`), in this order.
fn flutter_snippets(q: &Request<'_, '_>, f: &FlutterRefs) -> Vec<Snippet> {
    let eol = q.end_of_line();
    let name = "${1:MyWidget}";
    let constructor =
        if q.feature_enabled(dartr_parser::experimental_flags::ExperimentalFlag::SuperParameters) {
            format!("  const {name}({{super.key}});")
        } else {
            format!("  const {name}({{{}? key}}) : super(key: key);", f.r("Key"))
        };
    let build = [
        "  @override".to_string(),
        format!(
            "  {} build({} context) {{",
            f.r("Widget"),
            f.r("BuildContext")
        ),
        format!("    return ${{0:const {}()}};", f.r("Placeholder")),
        "  }".to_string(),
    ]
    .join(eol.as_str());
    let stateful_widget = [
        format!("class {name} extends {} {{", f.r("StatefulWidget")),
        constructor.clone(),
        String::new(),
        "  @override".to_string(),
        format!("  State<{name}> createState() => _{name}State();"),
        "}".to_string(),
        String::new(),
    ]
    .join(eol.as_str());
    let imports = if f.import_widgets {
        vec![WIDGETS_URI.to_string()]
    } else {
        Vec::new()
    };
    let stful = [
        stateful_widget.clone(),
        format!("class _{name}State extends {}<{name}> {{", f.r("State")),
        build.clone(),
        "}".to_string(),
    ]
    .join(eol.as_str());
    let stanim = [
        stateful_widget,
        format!("class _{name}State extends {}<{name}>", f.r("State")),
        format!("    with {} {{", f.r("SingleTickerProviderStateMixin")),
        format!("  late {} _controller;", f.r("AnimationController")),
        String::new(),
        "  @override".to_string(),
        "  void initState() {".to_string(),
        "    super.initState();".to_string(),
        format!(
            "    _controller = {}(vsync: this);",
            f.r("AnimationController")
        ),
        "  }".to_string(),
        String::new(),
        "  @override".to_string(),
        "  void dispose() {".to_string(),
        "    _controller.dispose();".to_string(),
        "    super.dispose();".to_string(),
        "  }".to_string(),
        String::new(),
        build.clone(),
        "}".to_string(),
    ]
    .join(eol.as_str());
    let stless = [
        format!("class {name} extends {} {{", f.r("StatelessWidget")),
        constructor,
        String::new(),
        build,
        "}".to_string(),
    ]
    .join(eol.as_str());
    vec![
        Snippet {
            prefix: "stful",
            label: "Flutter Stateful Widget",
            documentation: "Insert a Flutter StatefulWidget.",
            text: stful,
            imports: imports.clone(),
        },
        Snippet {
            prefix: "stanim",
            label: "Flutter Widget with AnimationController",
            documentation: "Insert a Flutter StatefulWidget with an AnimationController.",
            text: stanim,
            imports: imports.clone(),
        },
        Snippet {
            prefix: "stless",
            label: "Flutter Stateless Widget",
            documentation: "Insert a Flutter StatelessWidget.",
            text: stless,
            imports,
        },
    ]
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
        imports: Vec::new(),
        text: block("${1:void} ${2:name}(${3:params}) {".to_string()),
    };
    match context {
        AtTopLevel => {
            let args = if q.in_test_directory {
                ""
            } else {
                "List<String> args"
            };
            let mut out = vec![Snippet {
                prefix: "class",
                label: "class",
                documentation: "Insert a class definition.",
                imports: Vec::new(),
                text: block("class ${1:ClassName} {".to_string()),
            }];
            if let Some(f) = flutter_refs(q) {
                out.extend(flutter_snippets(q, &f));
            }
            out.extend([
                fun(),
                Snippet {
                    prefix: "main",
                    label: "main()",
                    documentation: "Insert a main function, used as an entry point.",
                    imports: Vec::new(),
                    text: format!("void main({args}) {{{eol}  $0{eol}}}"),
                },
            ]);
            out
        }
        InBlock => {
            let var_or_final = if q.style.make_locals_final {
                "final"
            } else {
                "var"
            };
            let mut out = vec![
                Snippet {
                    prefix: "do",
                    label: "do while",
                    documentation: "Insert a do-while loop.",
                    imports: Vec::new(),
                    text: format!(
                        "do {{{eol}{}$0{eol}{}",
                        i("  "),
                        i("} while (${1:condition});")
                    ),
                },
                Snippet {
                    prefix: "forin",
                    label: "for in",
                    documentation: "Insert a for-in loop.",
                    imports: Vec::new(),
                    text: block(format!(
                        "for ({var_or_final} ${{1:element}} in ${{2:collection}}) {{"
                    )),
                },
                Snippet {
                    prefix: "for",
                    label: "for",
                    documentation: "Insert a for loop.",
                    imports: Vec::new(),
                    text: block("for (var i = 0; i < ${1:count}; i++) {".to_string()),
                },
                fun(),
                Snippet {
                    prefix: "ife",
                    label: "ife",
                    documentation: "Insert an if/else statement.",
                    imports: Vec::new(),
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
                    imports: Vec::new(),
                    text: block("if (${1:condition}) {".to_string()),
                },
                Snippet {
                    prefix: "switch",
                    label: "switch statement",
                    documentation: "Insert a switch statement.",
                    imports: Vec::new(),
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
                    imports: Vec::new(),
                    text: format!(
                        "test('${{1:test name}}', () {{{eol}{}$0{eol}{}",
                        i("  "),
                        i("});")
                    ),
                });
                out.push(Snippet {
                    prefix: "group",
                    label: "group",
                    documentation: "Insert a test group block.",
                    imports: Vec::new(),
                    text: format!(
                        "group('${{1:group name}}', () {{{eol}{}$0{eol}{}",
                        i("  "),
                        i("});")
                    ),
                });
            }
            out.push(Snippet {
                prefix: "try",
                label: "try",
                documentation: "Insert a try/catch statement.",
                imports: Vec::new(),
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
                imports: Vec::new(),
                text: block("while (${1:condition}) {".to_string()),
            });
            out
        }
        InClass | InEnumMembers => vec![fun()],
        InExpression => vec![Snippet {
            prefix: "switch",
            label: "switch expression",
            documentation: "Insert a switch expression.",
            imports: Vec::new(),
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
            // Dart sends the edit of a snippet as a snippet text edit.
            item.insert(
                "textEdit".into(),
                json!({"insertTextFormat": 2, "range": edit_range, "newText": s.text}),
            );
        }
        if !s.imports.is_empty() {
            let edits = super::imports::import_edits(
                q.ast,
                q.root,
                q.line_info,
                q.content,
                q.style.lint_quote,
                &s.imports,
            );
            // Dart `snippetToCompletionItem` maps the edits of the change
            // to snippet text edits.
            let edits: Vec<Value> = edits
                .into_iter()
                .map(|mut e| {
                    e["insertTextFormat"] = json!(2);
                    e
                })
                .collect();
            item.insert("additionalTextEdits".into(), Value::Array(edits));
        }
        // Dart `_FuzzyScoreHelper.completionItemMatches`.
        let filter = if s.prefix != s.label {
            s.prefix
        } else {
            s.label
        };
        if matcher.score(filter) > 0.0 {
            out.push(Value::Object(item));
        }
    }
    out
}
