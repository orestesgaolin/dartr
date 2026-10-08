// Dart source: pkg/analyzer/lib/src/dart/analysis/unlinked_data.dart
// (UnlinkedUnit and the directive classes, without the binary format),
// pkg/analyzer/lib/src/dart/analysis/file_state.dart
// (FileState.serializeAstUnlinked2 and the _serialize* helpers),
// pkg/analyzer/lib/src/dart/analysis/unlinked_api_signature.dart

//! The unlinked data of one file: its directives, top-level names and API
//! signature, computed from the AST.

use dartr_ast::*;
use dartr_ast_builder::ParsedUnit;
use dartr_link::ast_util::{
    constructor_is_complete, dotted_name, field_is_static, function_is_complete,
    invokes_super_self, is_generator, is_synchronous, method_is_complete,
    mixin_super_invoked_names, string_value, class_name_part_name, class_body_members, enum_body_members, variable_list_is_const, variable_list_is_final,
};
use dartr_parser::experimental_features::ExperimentalFeatures;
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_syntax::TokenId;
use indexmap::IndexSet;

use crate::api_signature::ApiSignature;

/// Dart `UnlinkedCombinator`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnlinkedCombinator {
    pub keyword_offset: u32,
    pub end_offset: u32,
    pub is_show: bool,
    pub names: Vec<String>,
}

/// Dart `UnlinkedNamespaceDirectiveConfiguration`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnlinkedConfiguration {
    pub name: String,
    pub value: String,
    pub uri: Option<String>,
}

impl UnlinkedConfiguration {
    /// Dart `valueOrTrue`.
    pub fn value_or_true(&self) -> &str {
        if self.value.is_empty() { "true" } else { &self.value }
    }
}

/// Dart `UnlinkedLibraryImportPrefixName`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnlinkedLibraryImportPrefixName {
    pub name: String,
    pub name_offset: u32,
}

/// Dart `UnlinkedLibraryImportPrefix`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnlinkedLibraryImportPrefix {
    pub deferred_offset: Option<u32>,
    pub as_offset: u32,
    pub name_offset: u32,
    pub name: Option<UnlinkedLibraryImportPrefixName>,
}

/// Dart `UnlinkedLibraryImportDirective`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnlinkedLibraryImportDirective {
    pub combinators: Vec<UnlinkedCombinator>,
    pub configurations: Vec<UnlinkedConfiguration>,
    /// `-1` for the synthetic `dart:core` import.
    pub import_keyword_offset: i32,
    pub is_doc_import: bool,
    pub is_synthetic_dart_core: bool,
    pub prefix: Option<UnlinkedLibraryImportPrefix>,
    pub uri: Option<String>,
}

/// Dart `UnlinkedLibraryExportDirective`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnlinkedLibraryExportDirective {
    pub combinators: Vec<UnlinkedCombinator>,
    pub configurations: Vec<UnlinkedConfiguration>,
    pub export_keyword_offset: u32,
    pub uri: Option<String>,
}

/// Dart `UnlinkedPartDirective`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnlinkedPartDirective {
    pub configurations: Vec<UnlinkedConfiguration>,
    pub part_keyword_offset: u32,
    pub uri: Option<String>,
}

/// Dart `UnlinkedLibraryDirective`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnlinkedLibraryDirective {
    pub doc_imports: Vec<UnlinkedLibraryImportDirective>,
    pub name: Option<String>,
}

/// Dart `UnlinkedSourceRange`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnlinkedSourceRange {
    pub offset: u32,
    pub length: u32,
}

/// Dart `UnlinkedPartOfNameDirective`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnlinkedPartOfNameDirective {
    pub doc_imports: Vec<UnlinkedLibraryImportDirective>,
    pub name: String,
    pub name_range: UnlinkedSourceRange,
}

/// Dart `UnlinkedPartOfUriDirective`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnlinkedPartOfUriDirective {
    pub doc_imports: Vec<UnlinkedLibraryImportDirective>,
    pub uri: Option<String>,
    pub uri_range: UnlinkedSourceRange,
}

/// A configurable directive (Dart `UnlinkedConfigurableUriDirective`).
pub trait ConfigurableUriDirective {
    fn uri(&self) -> Option<&str>;
    fn configurations(&self) -> &[UnlinkedConfiguration];
}

macro_rules! configurable {
    ($t:ty) => {
        impl ConfigurableUriDirective for $t {
            fn uri(&self) -> Option<&str> {
                self.uri.as_deref()
            }
            fn configurations(&self) -> &[UnlinkedConfiguration] {
                &self.configurations
            }
        }
    };
}
configurable!(UnlinkedLibraryImportDirective);
configurable!(UnlinkedLibraryExportDirective);
configurable!(UnlinkedPartDirective);

/// Dart `UnlinkedUnit` (without `informativeBytes` and
/// `dartdocTemplates`: the linker reads the AST directly).
#[derive(Clone, Debug)]
pub struct UnlinkedUnit {
    pub api_signature: [u8; 16],
    pub exports: Vec<UnlinkedLibraryExportDirective>,
    pub has_dart_core_import: bool,
    pub imports: Vec<UnlinkedLibraryImportDirective>,
    pub is_dart_core: bool,
    pub library_directive: Option<UnlinkedLibraryDirective>,
    pub line_starts: Vec<u32>,
    pub parts: Vec<UnlinkedPartDirective>,
    pub part_of_name_directive: Option<UnlinkedPartOfNameDirective>,
    pub part_of_uri_directive: Option<UnlinkedPartOfUriDirective>,
    pub top_level_declarations: IndexSet<String>,
}

/// Dart `FileState.serializeAstUnlinked2`.
pub fn serialize_ast_unlinked2(parsed: &ParsedUnit, exists: bool, is_dart_core: bool) -> UnlinkedUnit {
    let ast = &parsed.ast;
    let unit = ast.get(parsed.unit);

    let build_doc_imports = |comment: Option<Id<Comment>>| -> Vec<UnlinkedLibraryImportDirective> {
        match comment {
            None => Vec::new(),
            Some(comment) => ast
                .get(comment)
                .doc_imports
                .iter()
                .map(|doc| serialize_import(&doc.ast, doc.import, true))
                .collect(),
        }
    };

    let mut library_directive = None;
    let mut part_of_name_directive = None;
    let mut part_of_uri_directive = None;
    let mut exports = Vec::new();
    let mut imports = Vec::new();
    let mut parts = Vec::new();
    let mut has_dart_core_import = false;
    for &directive in ast.list(unit.directives) {
        let d = directive.raw();
        if let Some(e) = ast.cast::<ExportDirective>(d) {
            exports.push(serialize_export(ast, e));
        } else if let Some(i) = ast.cast::<ImportDirective>(d) {
            let builder = serialize_import(ast, i, false);
            if builder.uri.as_deref() == Some("dart:core") {
                has_dart_core_import = true;
            }
            imports.push(builder);
        } else if let Some(l) = ast.cast::<LibraryDirective>(d) {
            let l = ast.get(l);
            library_directive = Some(UnlinkedLibraryDirective {
                doc_imports: build_doc_imports(l.documentation_comment),
                name: l.name.map(|n| dotted_name(ast, n)),
            });
        } else if let Some(p) = ast.cast::<PartDirective>(d) {
            let p = ast.get(p);
            parts.push(UnlinkedPartDirective {
                configurations: Vec::new(),
                part_keyword_offset: ast.tokens.offset(p.part_keyword),
                uri: string_value(ast, p.uri),
            });
        } else if let Some(p) = ast.cast::<PartOfDirective>(d) {
            let p = ast.get(p);
            if let Some(library_name) = p.library_name {
                if part_of_name_directive.is_none() {
                    part_of_name_directive = Some(UnlinkedPartOfNameDirective {
                        doc_imports: build_doc_imports(p.documentation_comment),
                        name: dotted_name(ast, library_name),
                        name_range: UnlinkedSourceRange {
                            offset: ast.offset(library_name),
                            length: ast.length(library_name),
                        },
                    });
                }
            } else if let Some(uri) = p.uri {
                if part_of_uri_directive.is_none() {
                    part_of_uri_directive = Some(UnlinkedPartOfUriDirective {
                        doc_imports: build_doc_imports(p.documentation_comment),
                        uri: string_value(ast, uri),
                        uri_range: UnlinkedSourceRange {
                            offset: ast.offset(uri),
                            length: ast.length(uri),
                        },
                    });
                }
            }
        }
    }

    let mut top_level_declarations = IndexSet::new();
    let lexeme = |t: TokenId| ast.tokens.lexeme(t).to_string();
    for &declaration in ast.list(unit.declarations) {
        let d = declaration.raw();
        if let Some(c) = ast.cast::<ClassDeclaration>(d) {
            top_level_declarations.insert(lexeme(class_name_part_name(ast, ast.get(c).name_part)));
        } else if let Some(e) = ast.cast::<EnumDeclaration>(d) {
            top_level_declarations.insert(lexeme(class_name_part_name(ast, ast.get(e).name_part)));
        } else if let Some(e) = ast.cast::<ExtensionDeclaration>(d) {
            if let Some(name) = ast.get(e).name {
                top_level_declarations.insert(lexeme(name));
            }
        } else if let Some(e) = ast.cast::<ExtensionTypeDeclaration>(d) {
            top_level_declarations.insert(lexeme(class_name_part_name(ast, ast.get(e).name_part)));
        } else if let Some(f) = ast.cast::<FunctionDeclaration>(d) {
            top_level_declarations.insert(lexeme(ast.get(f).name));
        } else if let Some(m) = ast.cast::<MixinDeclaration>(d) {
            top_level_declarations.insert(lexeme(ast.get(m).name));
        } else if let Some(v) = ast.cast::<TopLevelVariableDeclaration>(d) {
            for &variable in ast.list(ast.get(ast.get(v).variables).variables) {
                top_level_declarations.insert(lexeme(ast.get(variable).name));
            }
        }
    }

    let api_signature = {
        let mut builder = ApiSignature::new();
        builder.add_bytes(&compute_unlinked_api_signature(parsed));
        builder.add_bool(exists);
        builder.to_byte_list()
    };

    UnlinkedUnit {
        api_signature,
        exports,
        has_dart_core_import,
        imports,
        is_dart_core,
        library_directive,
        line_starts: parsed.line_info.line_starts.to_vec(),
        parts,
        part_of_name_directive,
        part_of_uri_directive,
        top_level_declarations,
    }
}

/// Dart `FileState._serializeCombinators`.
fn serialize_combinators(ast: &Ast, combinators: NodeList<Combinator>) -> Vec<UnlinkedCombinator> {
    ast.list(combinators)
        .iter()
        .map(|&c| {
            let (keyword, names, is_show) = if let Some(s) = ast.cast::<ShowCombinator>(c.raw()) {
                let s = ast.get(s);
                (s.keyword, s.shown_names, true)
            } else {
                let h = ast.cast::<HideCombinator>(c.raw()).expect("combinator");
                let h = ast.get(h);
                (h.keyword, h.hidden_names, false)
            };
            UnlinkedCombinator {
                keyword_offset: ast.tokens.offset(keyword),
                end_offset: ast.end(c),
                is_show,
                names: ast
                    .list(names)
                    .iter()
                    .map(|&n| ast.tokens.lexeme(ast.get(n).token).to_string())
                    .collect(),
            }
        })
        .collect()
}

/// Dart `FileState._serializeConfigurations`.
fn serialize_configurations(ast: &Ast, configurations: NodeList<Configuration>) -> Vec<UnlinkedConfiguration> {
    ast.list(configurations)
        .iter()
        .map(|&c| {
            let c = ast.get(c);
            UnlinkedConfiguration {
                name: dotted_name(ast, c.name),
                value: c.value.and_then(|v| string_value(ast, v)).unwrap_or_default(),
                uri: string_value(ast, c.uri),
            }
        })
        .collect()
}

/// Dart `FileState._serializeExport`.
fn serialize_export(ast: &Ast, node: Id<ExportDirective>) -> UnlinkedLibraryExportDirective {
    let n = ast.get(node);
    UnlinkedLibraryExportDirective {
        combinators: serialize_combinators(ast, n.combinators),
        configurations: serialize_configurations(ast, n.configurations),
        export_keyword_offset: ast.tokens.offset(n.export_keyword),
        uri: string_value(ast, n.uri),
    }
}

/// Dart `FileState._serializeImport`.
fn serialize_import(ast: &Ast, node: Id<ImportDirective>, is_doc_import: bool) -> UnlinkedLibraryImportDirective {
    let n = ast.get(node);
    let prefix = n.prefix.map(|prefix| {
        let token = ast.get(prefix).token;
        let name = if ast.tokens.get(token).is_synthetic() {
            None
        } else {
            Some(UnlinkedLibraryImportPrefixName {
                name: ast.tokens.lexeme(token).to_string(),
                name_offset: ast.tokens.offset(token),
            })
        };
        UnlinkedLibraryImportPrefix {
            deferred_offset: n.deferred_keyword.map(|t| ast.tokens.offset(t)),
            as_offset: ast.tokens.offset(n.as_keyword.expect("asKeyword")),
            name_offset: ast.tokens.offset(token),
            name,
        }
    });
    UnlinkedLibraryImportDirective {
        combinators: serialize_combinators(ast, n.combinators),
        configurations: serialize_configurations(ast, n.configurations),
        import_keyword_offset: ast.tokens.offset(n.import_keyword) as i32,
        is_doc_import,
        is_synthetic_dart_core: false,
        prefix,
        uri: string_value(ast, n.uri),
    }
}

/// Whether the experiment with the analyzer name [name] is enabled in
/// [features].
pub fn feature_enabled(features: ExperimentalFeatures, name: &str) -> bool {
    ExperimentalFlag::VALUES
        .iter()
        .any(|&f| f.name() == name && features.is_experiment_enabled(f))
}

/// Dart `computeUnlinkedApiSignature`.
pub fn compute_unlinked_api_signature(parsed: &ParsedUnit) -> [u8; 16] {
    let mut computer = UnitApiSignatureComputer {
        ast: &parsed.ast,
        signature: ApiSignature::new(),
    };
    computer.compute(parsed);
    computer.signature.to_byte_list()
}

const KIND_CONSTRUCTOR_DECLARATION: u32 = 1;
const KIND_FIELD_DECLARATION: u32 = 2;
const KIND_METHOD_DECLARATION: u32 = 3;
const KIND_PRIMARY_CONSTRUCTOR_BODY: u32 = 4;
const NULL_NODE: u32 = 0;
const NOT_NULL_NODE: u32 = 1;
const NULL_TOKEN: u32 = 0;
const NOT_NULL_TOKEN: u32 = 1;

/// Dart `_UnitApiSignatureComputer`.
struct UnitApiSignatureComputer<'a> {
    ast: &'a Ast,
    signature: ApiSignature,
}

impl UnitApiSignatureComputer<'_> {
    fn compute(&mut self, parsed: &ParsedUnit) {
        let ast = self.ast;
        let features = parsed.feature_set;
        self.signature.add_feature_set(|name| feature_enabled(features, name));
        let unit = ast.get(parsed.unit);
        let directives = ast.list(unit.directives);
        self.signature.add_int(directives.len() as u32);
        for &d in directives {
            self.add_node(Some(d.raw()));
        }
        let declarations = ast.list(unit.declarations);
        self.signature.add_int(declarations.len() as u32);
        for &declaration in declarations {
            let d = declaration.raw();
            if let Some(c) = ast.cast::<ClassDeclaration>(d) {
                let body = ast.get(c).body;
                self.add_tokens(ast.begin_token(d), ast.begin_token(body));
                let members = class_body_members(ast, body);
                let has_const_constructor = members.iter().any(|&m| {
                    ast.cast::<ConstructorDeclaration>(m.raw())
                        .is_some_and(|c| ast.get(c).const_keyword.is_some())
                });
                self.add_class_members(&members, has_const_constructor);
            } else if let Some(e) = ast.cast::<EnumDeclaration>(d) {
                let body = ast.get(e).body;
                let members = enum_body_members(ast, body);
                match members.first() {
                    None => self.add_node(Some(d)),
                    Some(&first) => {
                        self.add_tokens(ast.begin_token(d), ast.begin_token(first));
                        self.add_class_members(&members, true);
                    }
                }
            } else if let Some(e) = ast.cast::<ExtensionDeclaration>(d) {
                let body = ast.get(e).body;
                self.add_tokens(ast.begin_token(d), ast.begin_token(body));
                let members = class_body_members(ast, body);
                self.add_class_members(&members, false);
            } else if let Some(f) = ast.cast::<FunctionDeclaration>(d) {
                let fd = ast.get(f);
                let fe = ast.get(fd.function_expression);
                let end = match fe.parameters {
                    Some(p) => ast.end_token(p),
                    None => fd.name,
                };
                self.add_tokens(ast.begin_token(d), end);
                self.signature.add_bool(function_is_complete(ast, f));
                self.add_function_body_modifiers(Some(fe.body));
            } else if let Some(m) = ast.cast::<MixinDeclaration>(d) {
                let body = ast.get(m).body;
                self.add_tokens(ast.begin_token(d), ast.begin_token(body));
                let members = class_body_members(ast, body);
                self.add_class_members(&members, false);
                self.signature.add_string_list(&mixin_super_invoked_names(ast, m));
            } else if let Some(v) = ast.cast::<TopLevelVariableDeclaration>(d) {
                let v = ast.get(v);
                self.add_token(v.abstract_keyword);
                self.add_token(v.augment_keyword);
                self.add_token(v.external_keyword);
                self.add_node_list(ast.list_raw(v.metadata));
                let list = v.variables;
                let include = ast.get(list).type_.is_none() || variable_list_is_const(ast, list);
                self.variable_list(list, include);
            } else {
                self.add_node(Some(d));
            }
        }
    }

    fn add_class_members(&mut self, members: &[Id<ClassMember>], has_const_constructor: bool) {
        let ast = self.ast;
        self.signature.add_int(members.len() as u32);
        for &member in members {
            let m = member.raw();
            if let Some(c) = ast.cast::<ConstructorDeclaration>(m) {
                let cd = ast.get(c);
                self.signature.add_int(KIND_CONSTRUCTOR_DECLARATION);
                self.add_tokens(ast.begin_token(m), ast.end_token(cd.parameters));
                self.add_node_list(ast.list_raw(cd.initializers));
                self.add_node(cd.redirected_constructor.map(|r| r.raw()));
                self.signature.add_bool(constructor_is_complete(ast, c));
            } else if let Some(f) = ast.cast::<FieldDeclaration>(m) {
                let fd = ast.get(f);
                self.signature.add_int(KIND_FIELD_DECLARATION);
                self.add_token(fd.abstract_keyword);
                self.add_token(fd.augment_keyword);
                self.add_token(fd.covariant_keyword);
                self.add_token(fd.external_keyword);
                self.add_token(fd.static_keyword);
                self.add_node_list(ast.list_raw(fd.metadata));
                let list = fd.fields;
                let include = ast.get(list).type_.is_none()
                    || variable_list_is_const(ast, list)
                    || (has_const_constructor
                        && !field_is_static(ast, f)
                        && variable_list_is_final(ast, list));
                self.variable_list(list, include);
            } else if let Some(md) = ast.cast::<MethodDeclaration>(m) {
                let n = ast.get(md);
                self.signature.add_int(KIND_METHOD_DECLARATION);
                let end = match n.parameters {
                    Some(p) => ast.end_token(p),
                    None => n.name,
                };
                self.add_tokens(ast.begin_token(m), end);
                self.signature.add_bool(method_is_complete(ast, md));
                self.add_function_body_modifiers(Some(n.body));
                self.signature.add_bool(invokes_super_self(ast, md));
            } else if let Some(p) = ast.cast::<PrimaryConstructorBody>(m) {
                self.signature.add_int(KIND_PRIMARY_CONSTRUCTOR_BODY);
                self.add_tokens(ast.begin_token(m), ast.begin_token(ast.get(p).body));
            } else {
                panic!("unexpected class member {:?}", ast.kind(m));
            }
        }
    }

    fn add_function_body_modifiers(&mut self, node: Option<Id<FunctionBody>>) {
        if let Some(body) = node {
            let ast = self.ast;
            self.signature.add_bool(is_synchronous(ast, body));
            self.signature.add_bool(is_generator(ast, body));
            self.signature.add_bool(ast.is::<NativeFunctionBody>(body.raw()));
        }
    }

    fn add_node(&mut self, node: Option<NodeId>) {
        match node {
            Some(node) => {
                self.signature.add_int(NOT_NULL_NODE);
                let ast = self.ast;
                self.add_tokens(ast.begin_token(node), ast.end_token(node));
            }
            None => self.signature.add_int(NULL_NODE),
        }
    }

    fn add_node_list(&mut self, nodes: &[NodeId]) {
        for &node in nodes {
            self.add_node(Some(node));
        }
    }

    fn add_token(&mut self, token: Option<TokenId>) {
        match token {
            Some(token) => {
                self.signature.add_int(NOT_NULL_TOKEN);
                self.signature.add_string(self.ast.tokens.lexeme(token));
            }
            None => self.signature.add_int(NULL_TOKEN),
        }
    }

    /// Dart `_addTokens`: the tokens from [begin] to [end], stopping at EOF.
    fn add_tokens(&mut self, begin: TokenId, end: TokenId) {
        let tokens = &self.ast.tokens;
        let mut token = begin;
        loop {
            self.add_token(Some(token));
            if token == end {
                break;
            }
            let next = tokens.next(token);
            if next.is_none() || next == token || tokens.get(token).is_eof() {
                break;
            }
            token = next;
        }
    }

    fn variable_list(&mut self, node: Id<VariableDeclarationList>, include_initializers: bool) {
        let ast = self.ast;
        let n = ast.get(node);
        self.add_token(n.keyword);
        self.add_token(n.late_keyword);
        self.add_node(n.type_.map(|t| t.raw()));
        let variables = ast.list(n.variables);
        self.signature.add_int(variables.len() as u32);
        for &variable in variables {
            let v = ast.get(variable);
            self.add_token(Some(v.name));
            self.signature.add_bool(v.initializer.is_some());
            if include_initializers {
                self.add_node(v.initializer.map(|i| i.raw()));
            }
        }
    }
}

