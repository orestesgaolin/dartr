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
pub(crate) fn replace_type_with_future(
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
                // `dynamic`.
                e.write("dynamic ");
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

/// The import of [node] in the element model (Dart
/// `ImportDirective.libraryImport`): the namespace of the import.
fn import_namespace(
    c: &ProducerContext<'_>,
    node: Id<ImportDirective>,
) -> Option<std::sync::Arc<dartr_element::Namespace>> {
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
    let import = data
        .library_imports
        .iter()
        .filter(|i| !i.is_synthetic)
        .nth(index)?;
    // Without combinators, the import namespace (Dart
    // `getImportNamespace`, without the prefix) is the export namespace of
    // the imported library.
    let library = dartr_resolver::error::support::uri_library(&import.directive.uri)?;
    c.ctx.get(library).export_namespace.try_get().cloned()
}

assist!(ImportAddShow, a::IMPORT_ADD_SHOW, |c, builder| {
    let ast = c.ast;
    let ctx = c.ctx;
    let Some(import) = ast.this_or_ancestor_of_type::<ImportDirective>(c.node) else {
        return;
    };
    if !ast.list(ast[import].combinators).is_empty() {
        return;
    }
    let Some(namespace) = import_namespace(c, import) else {
        return;
    };
    let lookup = |name: &str| -> Option<dartr_element::ElementId> {
        namespace
            .defined_names
            .iter()
            .find(|(n, _)| ctx.name_str(**n) == name)
            .map(|(_, e)| *e)
    };
    let display_name =
        |e: dartr_element::ElementId| dartr_resolver::error::support::display_name(ctx, e);
    let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    // Dart `_addImplicitExtensionName`.
    let add_extension = |names: &mut std::collections::BTreeSet<String>,
                         element: Option<dartr_element::ElementId>| {
        let Some(element) = element else { return };
        let Some(enclosing) = ctx.element_data(element).and_then(|d| d.enclosing) else {
            return;
        };
        if enclosing.tag() != dartr_element::Tag::Extension {
            return;
        }
        let Some(name) = ctx.element_name(enclosing) else {
            return;
        };
        if lookup(name) == Some(enclosing) {
            names.insert(display_name(enclosing));
        }
    };
    let add_name = |names: &mut std::collections::BTreeSet<String>,
                    name: &str,
                    element: Option<dartr_element::ElementId>| {
        let Some(element) = element else { return };
        if lookup(name) == Some(element) || lookup(&format!("{name}=")) == Some(element) {
            names.insert(display_name(element));
        }
    };
    let locator = c.locator();
    super::variables::visit(ast, c.unit.raw(), &mut |n| {
        if let Some(a) = ast.cast::<AssignmentExpression>(n) {
            add_extension(
                &mut names,
                dartr_resolver::error::support::read_element(ctx, c.tables, a),
            );
            add_extension(
                &mut names,
                dartr_resolver::error::support::write_element(ctx, c.tables, a),
            );
        } else if ast.is::<BinaryExpression>(n)
            || ast.is::<FunctionExpressionInvocation>(n)
            || ast.is::<IndexExpression>(n)
            || ast.is::<PatternField>(n)
            || ast.is::<PrefixExpression>(n)
        {
            add_extension(&mut names, locator.element(n));
        } else if let Some(p) = ast.cast::<PrefixedIdentifier>(n) {
            let element = locator
                .element(n)
                .or_else(|| locator.write_or_read_element(ast[p].identifier));
            add_extension(&mut names, element);
        } else if let Some(m) = ast.cast::<MethodInvocation>(n) {
            add_extension(&mut names, locator.element(ast[m].method_name));
        } else if let Some(p) = ast.cast::<PropertyAccess>(n) {
            add_extension(&mut names, locator.element(ast[p].property_name));
        } else if let Some(t) = ast.cast::<NamedType>(n) {
            add_name(&mut names, c.lexeme(ast[t].name), locator.element(n));
        } else if let Some(id) = ast.cast::<SimpleIdentifier>(n) {
            add_name(
                &mut names,
                c.lexeme(ast[id].token),
                locator.write_or_read_element(id),
            );
        }
    });
    if names.is_empty() {
        return;
    }
    let names: Vec<String> = names.into_iter().collect();
    let offset = ast.end(import) - 1;
    let text = format!(" show {}", names.join(", "));
    builder.add_dart_file_edit(c.path, |b| b.add_simple_insertion(offset, &text));
});

/// Dart `InheritanceOverrideVerifier.missingOverrides` of the class or enum
/// [declaration] (the members that the verifier reports in
/// `non_abstract_class_inherits_abstract_member`).
fn missing_overrides(c: &ProducerContext<'_>, declaration: NodeId) -> Vec<dartr_element::ElemRef> {
    use dartr_typesystem::inheritance_manager3::InheritanceManager3;
    use dartr_typesystem::member;
    let ast = c.ast;
    let ctx = c.ctx;
    let Some(element) = c
        .locator()
        .declared_element(declaration)
        .and_then(|e| e.cast::<dartr_element::InterfaceElement>())
    else {
        return Vec::new();
    };
    let is_enum = ast.is::<EnumDeclaration>(declaration);
    let Some(library) = ctx.element_data(element.raw()).and_then(|d| d.library) else {
        return Vec::new();
    };
    let manager = InheritanceManager3::new(ctx.global());
    let interface = manager.get_interface(element);
    // The names of the members that the class declares.
    let mut declared: Vec<String> = Vec::new();
    for member in super::super::insert::members_of(ast, declaration).unwrap_or_default() {
        if let Some(m) = ast.cast::<MethodDeclaration>(member) {
            let name = c.lexeme(ast[m].name).to_string();
            if ast[m]
                .property_keyword
                .is_some_and(|k| c.lexeme(k) == "set")
            {
                declared.push(format!("{name}="));
            } else {
                declared.push(name);
            }
        } else if let Some(f) = ast.cast::<FieldDeclaration>(member) {
            let fields = ast[f].fields;
            let is_final = ast[fields].keyword.is_some_and(|k| c.lexeme(k) == "final");
            for &v in ast.list(ast[fields].variables) {
                let name = c.lexeme(ast[v].name).to_string();
                if !is_final {
                    declared.push(format!("{name}="));
                }
                declared.push(name);
            }
        }
    }
    // Dart `_isNotImplementedInConcreteSuperClass`.
    let super_interface = ctx
        .element_supertype(element)
        .and_then(|t| ctx.interface_element(t))
        .filter(|e| {
            e.raw().tag() == dartr_element::Tag::Class
                && !ctx.element_data(e.raw()).is_some_and(|d| {
                    d.flags
                        .has(dartr_element::ElementFlags::CLASS_ELEMENT_IS_ABSTRACT)
                })
        })
        .map(|e| manager.get_interface(e));
    let mut out = Vec::new();
    for (name, &interface_element) in &interface.map {
        if !name.is_accessible_for(ctx, library) {
            continue;
        }
        if interface.implemented.contains_key(name) {
            continue;
        }
        let text = name.text(ctx);
        if declared.iter().any(|d| d == text) {
            continue;
        }
        if member::enclosing_element(ctx, interface_element) == Some(element.raw()) {
            continue;
        }
        if super_interface
            .as_ref()
            .is_some_and(|i| i.map.contains_key(name))
        {
            continue;
        }
        if is_enum && matches!(text, "values" | "values=") {
            continue;
        }
        out.push(interface_element);
    }
    out
}

/// Dart `CreateMissingOverrides`.
pub struct CreateMissingOverrides {
    count: usize,
}

impl CreateMissingOverrides {
    pub fn new() -> Self {
        CreateMissingOverrides { count: 0 }
    }
}

impl CorrectionProducer for CreateMissingOverrides {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::CREATE_MISSING_OVERRIDES)
    }

    fn fix_arguments(&self) -> Vec<String> {
        vec![
            self.count.to_string(),
            if self.count == 1 { "" } else { "s" }.to_string(),
        ]
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        use dartr_typesystem::member;
        let ast = c.ast;
        let ctx = c.ctx;
        let Some(declaration) = ast.parent(c.node) else {
            return;
        };
        let is_enum = ast.is::<EnumDeclaration>(declaration);
        if !ast.is::<ClassDeclaration>(declaration) && !is_enum {
            return;
        }
        let mut signatures = missing_overrides(c, declaration);
        // Sort by name, getters before setters.
        let display = |e: dartr_element::ElemRef| {
            dartr_resolver::error::support::display_name(ctx, member::base_element(ctx, e))
        };
        signatures.sort_by(|a, b| {
            display(*a)
                .encode_utf16()
                .cmp(display(*b).encode_utf16())
                .then_with(|| {
                    if member::base_element(ctx, *a).tag() == dartr_element::Tag::Getter {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    }
                })
        });
        self.count = signatures.len();
        let prefix = c.utils.one_indent().to_string();
        let count = &mut self.count;
        builder.add_dart_file_edit(c.path, |b| {
            b.insert_into_unit_member(declaration, None, true, |e| {
                let mut written = 0;
                let separator = |e: &mut super::super::change_builder::EditBuilder<'_, '_, '_>,
                                 written: &mut usize,
                                 count: usize| {
                    if *written == 0 {
                        if count > 1 {
                            e.select_here();
                        }
                    } else {
                        e.newline();
                        e.newline();
                        e.write(&prefix);
                    }
                    *written += 1;
                };
                // Merge getter/setter pairs into fields.
                let mut i = 0;
                while i < signatures.len() {
                    let element = signatures[i];
                    let is_getter =
                        member::base_element(ctx, element).tag() == dartr_element::Tag::Getter;
                    if is_getter && i + 1 < signatures.len() {
                        let next = signatures[i + 1];
                        if member::base_element(ctx, next).tag() == dartr_element::Tag::Setter {
                            signatures.remove(i + 1);
                            signatures.remove(i);
                            *count -= 1;
                            separator(e, &mut written, *count);
                            e.write("@override");
                            e.newline();
                            e.write(&prefix);
                            if is_enum {
                                e.write("final ");
                            }
                            let ty = member::return_type(ctx, element);
                            e.write_type(
                                Some(ty),
                                &WriteType {
                                    required: true,
                                    ..Default::default()
                                },
                            );
                            e.write(" ");
                            e.write(member::name(ctx, element).unwrap_or(""));
                            e.write(";");
                            continue;
                        }
                    }
                    i += 1;
                }
                let single = *count == 1;
                for element in &signatures {
                    separator(e, &mut written, *count);
                    e.write_override(*element, single);
                }
            });
        });
    }
}
