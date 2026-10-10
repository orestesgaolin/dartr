// Dart source: pkg/analysis_server/lib/src/computer/computer_highlights.dart
// Dart source: pkg/analysis_server/lib/src/lsp/semantic_tokens/encoder.dart
// Dart source: pkg/analysis_server/lib/src/lsp/semantic_tokens/legend.dart
// Dart source: pkg/analysis_server/lib/src/lsp/semantic_tokens/mapping.dart

//! LSP semantic tokens (`textDocument/semanticTokens/{full,range}`): the
//! semantic tokens of `DartUnitHighlightsComputer.computeSemanticTokens`,
//! split into tokens without overlaps on one line each, encoded relative to
//! the previous token.

use dartr_ast::*;
use dartr_element::{Ctx, ElementId, Tag, TypeId, TypeKind};
use dartr_parser::quote::{Quote, analyze_quote, first_quote_length, last_quote_length};
use dartr_syntax::{LineInfo, TokenId, TokenType};
use dartr_typesystem::member;

use crate::element_locator::Unit;

/// `semanticTokenLegend.lspLegend.tokenTypes` (in Dart order).
pub const TOKEN_TYPES: &[&str] = &[
    "annotation",
    "class",
    "comment",
    "method",
    "variable",
    "parameter",
    "enum",
    "enumMember",
    "type",
    "source",
    "property",
    "keyword",
    "label",
    "namespace",
    "boolean",
    "number",
    "string",
    "function",
    "typeParameter",
];

/// `semanticTokenLegend.lspLegend.tokenModifiers` (in Dart order).
pub const TOKEN_MODIFIERS: &[&str] = &[
    "documentation",
    "constructor",
    "declaration",
    "importPrefix",
    "instance",
    "static",
    "escape",
    "annotation",
    "control",
    "label",
    "interpolation",
    "source",
    "void",
    "wildcard",
];

/// Dart `HighlightRegionType` (the values that the computer uses).
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum H {
    ANNOTATION,
    CLASS,
    COMMENT_BLOCK,
    COMMENT_DOCUMENTATION,
    COMMENT_END_OF_LINE,
    CONSTRUCTOR,
    CONSTRUCTOR_TEAR_OFF,
    DIRECTIVE,
    DYNAMIC_LOCAL_VARIABLE_DECLARATION,
    DYNAMIC_LOCAL_VARIABLE_REFERENCE,
    DYNAMIC_PARAMETER_DECLARATION,
    DYNAMIC_PARAMETER_REFERENCE,
    ENUM,
    ENUM_CONSTANT,
    EXTENSION,
    EXTENSION_TYPE,
    FIELD,
    FUNCTION_TYPE_ALIAS,
    IDENTIFIER_DEFAULT,
    IMPORT_PREFIX,
    INSTANCE_FIELD_DECLARATION,
    INSTANCE_FIELD_REFERENCE,
    INSTANCE_GETTER_DECLARATION,
    INSTANCE_GETTER_REFERENCE,
    INSTANCE_METHOD_DECLARATION,
    INSTANCE_METHOD_REFERENCE,
    INSTANCE_METHOD_TEAR_OFF,
    INSTANCE_SETTER_DECLARATION,
    INSTANCE_SETTER_REFERENCE,
    KEYWORD,
    LABEL,
    LIBRARY_NAME,
    LITERAL_BOOLEAN,
    LITERAL_DOUBLE,
    LITERAL_INTEGER,
    LITERAL_LIST,
    LITERAL_MAP,
    LITERAL_RECORD,
    LITERAL_STRING,
    LOCAL_FUNCTION_DECLARATION,
    LOCAL_FUNCTION_REFERENCE,
    LOCAL_FUNCTION_TEAR_OFF,
    LOCAL_VARIABLE_DECLARATION,
    LOCAL_VARIABLE_REFERENCE,
    MIXIN,
    PARAMETER_DECLARATION,
    PARAMETER_REFERENCE,
    STATIC_FIELD_DECLARATION,
    STATIC_GETTER_DECLARATION,
    STATIC_GETTER_REFERENCE,
    STATIC_METHOD_DECLARATION,
    STATIC_METHOD_REFERENCE,
    STATIC_METHOD_TEAR_OFF,
    STATIC_SETTER_DECLARATION,
    STATIC_SETTER_REFERENCE,
    TOP_LEVEL_FUNCTION_DECLARATION,
    TOP_LEVEL_FUNCTION_REFERENCE,
    TOP_LEVEL_FUNCTION_TEAR_OFF,
    TOP_LEVEL_GETTER_DECLARATION,
    TOP_LEVEL_GETTER_REFERENCE,
    TOP_LEVEL_SETTER_DECLARATION,
    TOP_LEVEL_SETTER_REFERENCE,
    TOP_LEVEL_VARIABLE_DECLARATION,
    TYPE_ALIAS,
    TYPE_NAME_DYNAMIC,
    TYPE_PARAMETER,
    UNRESOLVED_INSTANCE_MEMBER_REFERENCE,
    VALID_STRING_ESCAPE,
}

/// Dart `highlightRegionTokenTypes`.
fn token_type(h: H) -> Option<&'static str> {
    use H::*;
    Some(match h {
        ANNOTATION => "annotation",
        CLASS => "class",
        COMMENT_BLOCK | COMMENT_DOCUMENTATION | COMMENT_END_OF_LINE => "comment",
        CONSTRUCTOR_TEAR_OFF => "method",
        DYNAMIC_LOCAL_VARIABLE_DECLARATION | DYNAMIC_LOCAL_VARIABLE_REFERENCE => "variable",
        DYNAMIC_PARAMETER_DECLARATION | DYNAMIC_PARAMETER_REFERENCE => "parameter",
        ENUM => "enum",
        ENUM_CONSTANT => "enumMember",
        EXTENSION | EXTENSION_TYPE => "class",
        FUNCTION_TYPE_ALIAS => "type",
        IDENTIFIER_DEFAULT => "source",
        IMPORT_PREFIX => "variable",
        INSTANCE_FIELD_DECLARATION | INSTANCE_FIELD_REFERENCE => "variable",
        INSTANCE_GETTER_DECLARATION | INSTANCE_GETTER_REFERENCE => "property",
        INSTANCE_METHOD_DECLARATION | INSTANCE_METHOD_REFERENCE | INSTANCE_METHOD_TEAR_OFF => {
            "method"
        }
        INSTANCE_SETTER_DECLARATION | INSTANCE_SETTER_REFERENCE => "property",
        KEYWORD => "keyword",
        LABEL => "label",
        LIBRARY_NAME => "namespace",
        LITERAL_BOOLEAN => "boolean",
        LITERAL_DOUBLE | LITERAL_INTEGER => "number",
        LITERAL_STRING => "string",
        LOCAL_FUNCTION_DECLARATION | LOCAL_FUNCTION_REFERENCE | LOCAL_FUNCTION_TEAR_OFF => {
            "function"
        }
        LOCAL_VARIABLE_DECLARATION | LOCAL_VARIABLE_REFERENCE => "variable",
        MIXIN => "class",
        PARAMETER_DECLARATION | PARAMETER_REFERENCE => "parameter",
        STATIC_FIELD_DECLARATION => "variable",
        STATIC_GETTER_DECLARATION | STATIC_GETTER_REFERENCE => "property",
        STATIC_METHOD_DECLARATION | STATIC_METHOD_REFERENCE | STATIC_METHOD_TEAR_OFF => "method",
        STATIC_SETTER_DECLARATION | STATIC_SETTER_REFERENCE => "property",
        TOP_LEVEL_FUNCTION_DECLARATION
        | TOP_LEVEL_FUNCTION_REFERENCE
        | TOP_LEVEL_FUNCTION_TEAR_OFF => "function",
        TOP_LEVEL_GETTER_DECLARATION
        | TOP_LEVEL_GETTER_REFERENCE
        | TOP_LEVEL_SETTER_DECLARATION
        | TOP_LEVEL_SETTER_REFERENCE => "property",
        TOP_LEVEL_VARIABLE_DECLARATION => "variable",
        TYPE_ALIAS | TYPE_NAME_DYNAMIC => "type",
        TYPE_PARAMETER => "typeParameter",
        UNRESOLVED_INSTANCE_MEMBER_REFERENCE => "source",
        VALID_STRING_ESCAPE => "string",
        CONSTRUCTOR | DIRECTIVE | FIELD | LITERAL_LIST | LITERAL_MAP | LITERAL_RECORD => {
            return None;
        }
    })
}

/// Dart `highlightRegionTokenModifiers` (in set order).
fn token_modifiers(h: H) -> Option<&'static [&'static str]> {
    use H::*;
    Some(match h {
        COMMENT_DOCUMENTATION => &["documentation"],
        CONSTRUCTOR_TEAR_OFF => &["constructor"],
        DYNAMIC_LOCAL_VARIABLE_DECLARATION | DYNAMIC_PARAMETER_DECLARATION => &["declaration"],
        IMPORT_PREFIX => &["importPrefix"],
        INSTANCE_FIELD_DECLARATION
        | INSTANCE_GETTER_DECLARATION
        | INSTANCE_METHOD_DECLARATION
        | INSTANCE_SETTER_DECLARATION => &["declaration", "instance"],
        INSTANCE_FIELD_REFERENCE
        | INSTANCE_GETTER_REFERENCE
        | INSTANCE_METHOD_REFERENCE
        | INSTANCE_METHOD_TEAR_OFF
        | INSTANCE_SETTER_REFERENCE => &["instance"],
        LOCAL_FUNCTION_DECLARATION | LOCAL_VARIABLE_DECLARATION | PARAMETER_DECLARATION => {
            &["declaration"]
        }
        STATIC_FIELD_DECLARATION
        | STATIC_GETTER_DECLARATION
        | STATIC_METHOD_DECLARATION
        | STATIC_SETTER_DECLARATION
        | TOP_LEVEL_FUNCTION_DECLARATION => &["declaration", "static"],
        STATIC_GETTER_REFERENCE
        | STATIC_METHOD_REFERENCE
        | STATIC_METHOD_TEAR_OFF
        | STATIC_SETTER_REFERENCE => &["static"],
        TOP_LEVEL_GETTER_DECLARATION
        | TOP_LEVEL_SETTER_DECLARATION
        | TOP_LEVEL_VARIABLE_DECLARATION => &["declaration"],
        VALID_STRING_ESCAPE => &["escape"],
        _ => return None,
    })
}

/// Dart `SemanticTokenInfo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticTokenInfo {
    pub offset: u32,
    pub length: u32,
    pub ty: &'static str,
    /// `None` when the token has no modifier set (Dart `null`).
    pub modifiers: Option<Vec<&'static str>>,
}

/// Overrides of [Computer::add_region].
#[derive(Default)]
struct Opts {
    ty: Option<&'static str>,
    modifiers: Option<Vec<&'static str>>,
    additional: Option<Vec<&'static str>>,
}

fn m(modifiers: &[&'static str]) -> Opts {
    Opts {
        modifiers: Some(modifiers.to_vec()),
        ..Opts::default()
    }
}

fn control() -> Opts {
    m(&["control"])
}

/// Dart `DartUnitHighlightsComputer` (semantic tokens only).
pub struct Computer<'u, 'c, 'a> {
    unit: &'u Unit<'c, 'a>,
    range: Option<(u32, u32)>,
    pub tokens: Vec<SemanticTokenInfo>,
}

impl<'u, 'c, 'a> Computer<'u, 'c, 'a> {
    pub fn new(unit: &'u Unit<'c, 'a>, range: Option<(u32, u32)>) -> Self {
        Computer {
            unit,
            range,
            tokens: Vec::new(),
        }
    }

    fn ast(&self) -> &'c Ast {
        self.unit.ast
    }

    fn ctx(&self) -> &'c Ctx<'a> {
        self.unit.ctx
    }

    /// Dart `computeSemanticTokens`.
    pub fn compute(mut self, root: Id<CompilationUnit>) -> Vec<SemanticTokenInfo> {
        let ast = self.ast();
        ast.accept(root.raw(), &mut self);
        self.add_comment_ranges(root);
        self.tokens
    }

    /// Dart `_addCommentRanges`.
    fn add_comment_ranges(&mut self, root: Id<CompilationUnit>) {
        let ast = self.ast();
        let mut token = ast[root].begin_token;
        loop {
            for comment in ast.tokens.comments(token) {
                let ty = ast.tokens.ty(comment);
                let lexeme = ast.tokens.lexeme(comment);
                let h = if ty == TokenType::MULTI_LINE_COMMENT {
                    Some(if lexeme.starts_with("/**") {
                        H::COMMENT_DOCUMENTATION
                    } else {
                        H::COMMENT_BLOCK
                    })
                } else if ty == TokenType::SINGLE_LINE_COMMENT {
                    Some(if lexeme.starts_with("///") {
                        H::COMMENT_DOCUMENTATION
                    } else {
                        H::COMMENT_END_OF_LINE
                    })
                } else {
                    None
                };
                if let Some(h) = h {
                    self.token(Some(comment), h, Opts::default());
                }
            }
            if ast.tokens.ty(token) == TokenType::EOF {
                break;
            }
            let next = ast.tokens.get(token).next;
            if next == token {
                break;
            }
            token = next;
        }
    }

    /// Dart `_addRegion`.
    fn add_region(&mut self, offset: u32, length: u32, h: H, opts: Opts) {
        if let Some((start, end)) = self.range {
            let token_end = offset + length;
            if token_end < start || offset > end {
                return;
            }
        }
        let ty = opts.ty.or_else(|| token_type(h));
        let mut modifiers = opts
            .modifiers
            .or_else(|| token_modifiers(h).map(|m| m.to_vec()));
        if let Some(additional) = opts.additional.filter(|a| !a.is_empty()) {
            let mut set = modifiers.unwrap_or_default();
            for a in additional {
                if !set.contains(&a) {
                    set.push(a);
                }
            }
            modifiers = Some(set);
        }
        if let Some(ty) = ty {
            self.tokens.push(SemanticTokenInfo {
                offset,
                length,
                ty,
                modifiers,
            });
        }
    }

    fn token(&mut self, token: Option<TokenId>, h: H, opts: Opts) -> bool {
        if let Some(t) = token {
            let tok = self.ast().tokens.get(t);
            let (offset, length) = (tok.offset, tok.length);
            self.add_region(offset, length, h, opts);
        }
        true
    }

    fn node(&mut self, node: NodeId, h: H, opts: Opts) -> bool {
        let ast = self.ast();
        let (offset, length) = (ast.offset(node), ast.length(node));
        self.add_region(offset, length, h, opts);
        true
    }

    fn type_of(&self, element: ElementId) -> TypeId {
        member::type_(self.ctx(), dartr_element::ElemRef::Base(element))
    }

    fn is_dynamic(&self, ty: TypeId) -> bool {
        matches!(self.ctx().ty(ty), TypeKind::Dynamic)
    }

    /// Dart `_additionalModifiersForElement`.
    fn wildcard(&self, element: Option<ElementId>) -> Option<Vec<&'static str>> {
        element
            .filter(|&e| dartr_resolver::error::correct_override::is_wildcard_variable(self.ctx(), e))
            .map(|_| vec!["wildcard"])
    }

    fn is_static(&self, element: ElementId) -> bool {
        member::is_static(self.ctx(), dartr_element::ElemRef::Base(element))
    }

    /// Dart `_isAnnotationIdentifier`.
    fn is_annotation_identifier(&self, parent: NodeId) -> bool {
        let ast = self.ast();
        ast.is::<Annotation>(parent)
            || (ast.is::<PrefixedIdentifier>(parent)
                && ast.parent(parent).is_some_and(|p| ast.is::<Annotation>(p)))
    }

    fn annotation_additional(&self, parent: NodeId) -> Opts {
        Opts {
            additional: self.is_annotation_identifier(parent).then(|| vec!["annotation"]),
            ..Opts::default()
        }
    }

    /// Dart `_addIdentifierRegion`.
    fn identifier(&mut self, parent: NodeId, name: TokenId, element: Option<ElementId>) {
        if self.identifier_keyword(name)
            || self.identifier_class(parent, name, element)
            || self.identifier_extension(name, element)
            || self.identifier_constructor(parent, name, element)
            || self.identifier_getter_setter_declaration(parent, name, element)
            || self.identifier_field(parent, name, element)
            || self.identifier_function(parent, name, element)
            || self.identifier_import_prefix(name, element)
            || self.identifier_label(parent, name, element)
            || self.identifier_local_variable(name, element)
            || self.identifier_method(parent, name, element)
            || self.identifier_parameter(parent, name, element)
            || self.identifier_type_alias(name, element)
            || self.identifier_type_parameter(name, element)
            || self.identifier_unresolved_instance_member(parent, name, element)
        {
            return;
        }
        self.token(Some(name), H::IDENTIFIER_DEFAULT, Opts::default());
    }

    fn identifier_keyword(&mut self, name: TokenId) -> bool {
        if self.ast().tokens.lexeme(name) == "void" {
            return self.token(Some(name), H::KEYWORD, m(&["void"]));
        }
        false
    }

    fn identifier_class(&mut self, parent: NodeId, name: TokenId, element: Option<ElementId>) -> bool {
        let Some(e) = element else { return false };
        if !matches!(e.tag(), Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType) {
            return false;
        }
        let ast = self.ast();
        let grand = ast.parent(parent);
        let mut opts = Opts::default();
        let h;
        if ast.is::<NamedType>(parent)
            && grand.is_some_and(|g| ast.is::<ConstructorName>(g))
            && grand
                .and_then(|g| ast.parent(g))
                .is_some_and(|gg| ast.is::<InstanceCreationExpression>(gg))
        {
            h = H::CONSTRUCTOR;
            opts.ty = Some("class");
            opts.modifiers = Some(vec!["constructor"]);
        } else if e.tag() == Tag::Enum {
            h = H::ENUM;
        } else if e.tag() == Tag::ExtensionType {
            h = H::EXTENSION_TYPE;
        } else if e.tag() == Tag::Mixin {
            h = H::MIXIN;
        } else {
            h = H::CLASS;
            if ast.is::<ConstructorDeclaration>(parent) {
                opts.modifiers = Some(vec!["constructor", "declaration"]);
            }
        }
        opts.additional = self.is_annotation_identifier(parent).then(|| vec!["annotation"]);
        self.token(Some(name), h, opts)
    }

    fn identifier_constructor(&mut self, parent: NodeId, name: TokenId, element: Option<ElementId>) -> bool {
        if element.map(|e| e.tag()) != Some(Tag::Constructor) {
            return false;
        }
        let mut modifiers = vec!["constructor"];
        if self.is_annotation_identifier(parent) {
            modifiers.push("annotation");
        }
        self.token(
            Some(name),
            H::CONSTRUCTOR,
            Opts {
                ty: Some("method"),
                modifiers: Some(modifiers),
                additional: None,
            },
        )
    }

    fn identifier_extension(&mut self, name: TokenId, element: Option<ElementId>) -> bool {
        if element.map(|e| e.tag()) != Some(Tag::Extension) {
            return false;
        }
        self.token(Some(name), H::EXTENSION, Opts::default())
    }

    /// Dart `PropertyAccessorElement.variable`.
    fn accessor_variable(&self, e: ElementId) -> Option<ElementId> {
        dartr_resolver::element_metadata::accessor_variable_any(self.ctx(), e)
    }

    fn identifier_field(&mut self, parent: NodeId, name: TokenId, element: Option<ElementId>) -> bool {
        let ctx = *self.ctx();
        let ast = self.ast();
        let mut h = None;
        match element.map(|e| (e, e.tag())) {
            Some((e, Tag::Field)) => {
                h = Some(if dartr_resolver::element_ext::is_enum_constant(&ctx, e) {
                    H::ENUM_CONSTANT
                } else if self.is_static(e) {
                    H::STATIC_FIELD_DECLARATION
                } else {
                    H::INSTANCE_FIELD_REFERENCE
                });
            }
            Some((_, Tag::TopLevelVariable)) => h = Some(H::TOP_LEVEL_VARIABLE_DECLARATION),
            Some((e, Tag::Getter)) | Some((e, Tag::Setter)) => {
                let getter = e.tag() == Tag::Getter;
                let variable = self.accessor_variable(e);
                h = Some(if variable.is_some_and(|v| v.tag() == Tag::TopLevelVariable) {
                    if getter {
                        H::TOP_LEVEL_GETTER_REFERENCE
                    } else {
                        H::TOP_LEVEL_SETTER_REFERENCE
                    }
                } else if variable.is_some_and(|v| {
                    v.tag() == Tag::Field && dartr_resolver::element_ext::is_enum_constant(&ctx, v)
                }) {
                    H::ENUM_CONSTANT
                } else if self.is_static(e) {
                    if getter {
                        H::STATIC_GETTER_REFERENCE
                    } else {
                        H::STATIC_SETTER_REFERENCE
                    }
                } else if getter {
                    H::INSTANCE_GETTER_REFERENCE
                } else {
                    H::INSTANCE_SETTER_REFERENCE
                });
            }
            Some(_) => {}
            None => {
                let mut static_type = None;
                if let Some(p) = ast.cast::<PropertyAccess>(parent)
                    && ast[ast[p].property_name].token == name
                {
                    static_type = property_access_real_target(ast, p)
                        .and_then(|t| self.unit.tables.static_type.get(t.raw()).copied());
                } else if !ast.is::<PrefixedIdentifier>(parent)
                    && let Some(extended) = self.enclosing_extension_type(parent)
                {
                    static_type = Some(extended);
                }
                if let Some(t) = static_type
                    && let TypeKind::Record { positional, named, .. } = ctx.ty(t)
                {
                    let lexeme = ast.tokens.lexeme(name);
                    let positional_count = ctx.list(*positional).len();
                    let has = lexeme
                        .strip_prefix('$')
                        .and_then(|n| n.parse::<usize>().ok())
                        .is_some_and(|i| i >= 1 && i <= positional_count)
                        || ctx.list(*named).iter().any(|f| ctx.name_str(f.name) == lexeme);
                    h = Some(if has {
                        H::INSTANCE_GETTER_REFERENCE
                    } else {
                        H::UNRESOLVED_INSTANCE_MEMBER_REFERENCE
                    });
                }
            }
        }
        if let Some(h) = h {
            let opts = self.annotation_additional(parent);
            return self.token(Some(name), h, opts);
        }
        false
    }

    /// Dart `parent.enclosingInstanceElement` when it is an extension: its
    /// extended type.
    fn enclosing_extension_type(&self, node: NodeId) -> Option<TypeId> {
        let ast = self.ast();
        let mut current = Some(node);
        while let Some(n) = current {
            if ast.is::<ClassDeclaration>(n)
                || ast.is::<MixinDeclaration>(n)
                || ast.is::<EnumDeclaration>(n)
                || ast.is::<ExtensionTypeDeclaration>(n)
            {
                return None;
            }
            if ast.is::<ExtensionDeclaration>(n) {
                let e = self.unit.declared_element(n)?;
                let extension = e.cast::<dartr_element::ExtensionElement>()?;
                return self.ctx().get(extension).extended_type.get();
            }
            current = ast.parent(n);
        }
        None
    }

    fn identifier_function(&mut self, parent: NodeId, name: TokenId, element: Option<ElementId>) -> bool {
        let Some(e) = element else { return false };
        if !matches!(e.tag(), Tag::TopLevelFunction | Tag::LocalFunction) {
            return false;
        }
        let ast = self.ast();
        let invocation = ast
            .cast::<MethodInvocation>(parent)
            .is_some_and(|mi| ast[ast[mi].method_name].token == name);
        let h = match (e.tag() == Tag::TopLevelFunction, invocation) {
            (true, true) => H::TOP_LEVEL_FUNCTION_REFERENCE,
            (true, false) => H::TOP_LEVEL_FUNCTION_TEAR_OFF,
            (false, true) => H::LOCAL_FUNCTION_REFERENCE,
            (false, false) => H::LOCAL_FUNCTION_TEAR_OFF,
        };
        self.token(Some(name), h, Opts::default())
    }

    fn identifier_getter_setter_declaration(
        &mut self,
        parent: NodeId,
        name: TokenId,
        element: Option<ElementId>,
    ) -> bool {
        let ast = self.ast();
        if !(ast.is::<MethodDeclaration>(parent) || ast.is::<FunctionDeclaration>(parent)) {
            return false;
        }
        let top = ast.parent(parent).is_some_and(|p| ast.is::<CompilationUnit>(p));
        let h = match element.map(|e| (e, e.tag())) {
            Some((e, Tag::Getter)) => {
                if top {
                    H::TOP_LEVEL_GETTER_DECLARATION
                } else if self.is_static(e) {
                    H::STATIC_GETTER_DECLARATION
                } else {
                    H::INSTANCE_GETTER_DECLARATION
                }
            }
            Some((e, Tag::Setter)) => {
                if top {
                    H::TOP_LEVEL_SETTER_DECLARATION
                } else if self.is_static(e) {
                    H::STATIC_SETTER_DECLARATION
                } else {
                    H::INSTANCE_SETTER_DECLARATION
                }
            }
            _ => return false,
        };
        self.token(Some(name), h, Opts::default())
    }

    fn identifier_import_prefix(&mut self, name: TokenId, element: Option<ElementId>) -> bool {
        if element.map(|e| e.tag()) != Some(Tag::Prefix) {
            return false;
        }
        self.token(Some(name), H::IMPORT_PREFIX, Opts::default())
    }

    fn identifier_label(&mut self, parent: NodeId, name: TokenId, element: Option<ElementId>) -> bool {
        if element.map(|e| e.tag()) != Some(Tag::Label) {
            return false;
        }
        let opts = if !self.ast().is::<BreakStatement>(parent) {
            m(&["declaration"])
        } else {
            Opts::default()
        };
        self.token(Some(name), H::LABEL, opts)
    }

    fn identifier_local_variable(&mut self, name: TokenId, element: Option<ElementId>) -> bool {
        let Some(e) = element else { return false };
        if !matches!(
            e.tag(),
            Tag::LocalVariable | Tag::PatternVariable | Tag::BindPatternVariable | Tag::JoinPatternVariable
        ) {
            return false;
        }
        let h = if self.is_dynamic(self.type_of(e)) {
            H::DYNAMIC_LOCAL_VARIABLE_REFERENCE
        } else {
            H::LOCAL_VARIABLE_REFERENCE
        };
        self.token(Some(name), h, Opts::default())
    }

    fn identifier_method(&mut self, parent: NodeId, name: TokenId, element: Option<ElementId>) -> bool {
        let ast = self.ast();
        let invocation = ast
            .cast::<MethodInvocation>(parent)
            .is_some_and(|mi| ast[ast[mi].method_name].token == name)
            || ast
                .cast::<DotShorthandInvocation>(parent)
                .is_some_and(|d| ast[ast[d].member_name].token == name);
        if self.is_call_method(parent, name) {
            let h = if invocation {
                H::INSTANCE_METHOD_REFERENCE
            } else {
                H::INSTANCE_METHOD_TEAR_OFF
            };
            return self.token(Some(name), h, Opts::default());
        }
        let Some(e) = element.filter(|e| e.tag() == Tag::Method) else {
            return false;
        };
        let h = match (self.is_static(e), invocation) {
            (true, true) => H::STATIC_METHOD_REFERENCE,
            (true, false) => H::STATIC_METHOD_TEAR_OFF,
            (false, true) => H::INSTANCE_METHOD_REFERENCE,
            (false, false) => H::INSTANCE_METHOD_TEAR_OFF,
        };
        self.token(Some(name), h, Opts::default())
    }

    fn identifier_parameter(&mut self, parent: NodeId, name: TokenId, element: Option<ElementId>) -> bool {
        let Some(e) = element else { return false };
        if !matches!(
            e.tag(),
            Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter
        ) {
            return false;
        }
        let h = if self.is_dynamic(self.type_of(e)) {
            H::DYNAMIC_PARAMETER_REFERENCE
        } else {
            H::PARAMETER_REFERENCE
        };
        let opts = if self.ast().is::<Label>(parent) {
            m(&["label"])
        } else {
            Opts::default()
        };
        self.token(Some(name), h, opts)
    }

    fn identifier_type_alias(&mut self, name: TokenId, element: Option<ElementId>) -> bool {
        let Some(alias) = element.and_then(|e| e.cast::<dartr_element::TypeAliasElement>()) else {
            return false;
        };
        let ctx = *self.ctx();
        let aliased = ctx.get(alias).aliased_type.get();
        let h = if aliased.is_some_and(|t| matches!(ctx.ty(t), TypeKind::Function(_))) {
            H::FUNCTION_TYPE_ALIAS
        } else {
            H::TYPE_ALIAS
        };
        self.token(Some(name), h, Opts::default())
    }

    fn identifier_type_parameter(&mut self, name: TokenId, element: Option<ElementId>) -> bool {
        let Some(e) = element.filter(|e| e.tag() == Tag::TypeParameter) else {
            return false;
        };
        let additional = self.wildcard(Some(e));
        self.token(
            Some(name),
            H::TYPE_PARAMETER,
            Opts {
                additional,
                ..Opts::default()
            },
        )
    }

    fn identifier_unresolved_instance_member(
        &mut self,
        parent: NodeId,
        name: TokenId,
        element: Option<ElementId>,
    ) -> bool {
        if element.is_some() {
            return false;
        }
        let ast = self.ast();
        let mut decorate = false;
        if let Some(mi) = ast.cast::<MethodInvocation>(parent) {
            let target = dartr_resolver::ast_ext::method_invocation_real_target(ast, mi);
            if ast[ast[mi].method_name].token == name
                && target.is_some_and(|t| self.is_dynamic_expression(t.raw()))
            {
                decorate = true;
            }
        } else if let Some(p) = ast.cast::<PrefixedIdentifier>(parent) {
            decorate = ast[ast[p].identifier].token == name;
        } else if let Some(p) = ast.cast::<PropertyAccess>(parent) {
            decorate = ast[ast[p].property_name].token == name;
        }
        if decorate {
            self.token(Some(name), H::UNRESOLVED_INSTANCE_MEMBER_REFERENCE, Opts::default());
            return true;
        }
        false
    }

    /// Dart `_isDynamicExpression`.
    fn is_dynamic_expression(&self, e: NodeId) -> bool {
        match self.unit.tables.static_type.get(e).copied() {
            Some(t) => matches!(self.ctx().ty(t), TypeKind::Dynamic | TypeKind::Invalid),
            None => false,
        }
    }

    /// Dart `DartType?.isFunction`.
    fn is_function_type(&self, ty: Option<TypeId>) -> bool {
        let Some(t) = ty else { return false };
        let ctx = self.ctx();
        match ctx.ty(t) {
            TypeKind::Function(_) => true,
            TypeKind::Interface { element, .. } => {
                element_name(&ctx, element.raw()) == Some("Function")
                    && dartr_resolver::error::support::library_of(ctx, element.raw())
                        .map(|l| ctx.fragment(ctx.get(l).first_fragment()).source.uri.to_string())
                        .as_deref()
                        == Some("dart:core")
            }
            _ => false,
        }
    }

    fn static_type_of(&self, node: Option<NodeId>) -> Option<TypeId> {
        node.and_then(|n| self.unit.tables.static_type.get(n).copied())
    }

    /// Dart `_isCallMethod`.
    fn is_call_method(&self, parent: NodeId, name: TokenId) -> bool {
        let ast = self.ast();
        let lexeme = ast.tokens.lexeme(name);
        let enclosing_function = || {
            self.enclosing_extension_type(parent)
                .is_some_and(|t| self.is_function_type(Some(t)))
        };
        if let Some(mi) = ast.cast::<MethodInvocation>(parent)
            && ast[ast[mi].method_name].token == name
            && lexeme == "call"
        {
            let target = dartr_resolver::ast_ext::method_invocation_real_target(ast, mi);
            if self.is_function_type(self.static_type_of(target.map(|t| t.raw())))
                || enclosing_function()
            {
                return true;
            }
        }
        if let Some(p) = ast.cast::<PrefixedIdentifier>(parent)
            && ast[ast[p].identifier].token == name
            && lexeme == "call"
            && self.is_function_type(self.static_type_of(Some(ast[p].prefix.raw())))
        {
            return true;
        }
        if let Some(p) = ast.cast::<PropertyAccess>(parent)
            && ast[ast[p].property_name].token == name
            && lexeme == "call"
            && self.is_function_type(
                self.static_type_of(property_access_real_target(ast, p).map(|t| t.raw())),
            )
        {
            return true;
        }
        // The special cases of extension methods.
        let expression: Option<NodeId> = if let Some(b) = ast.cast::<ExpressionFunctionBody>(parent) {
            Some(ast[b].expression.raw())
        } else if let Some(s) = ast.cast::<ExpressionStatement>(parent) {
            Some(ast[s].expression.raw())
        } else if let Some(r) = ast.cast::<ReturnStatement>(parent) {
            ast[r].expression.map(|e| e.raw())
        } else if let Some(l) = ast.cast::<ArgumentList>(parent) {
            ast.list_raw(ast[l].arguments)
                .iter()
                .map(|&a| match ast.cast::<NamedArgument>(a) {
                    Some(n) => ast[n].argument_expression.raw(),
                    None => a,
                })
                .find(|&a| {
                    ast.cast::<SimpleIdentifier>(a).is_some_and(|s| ast[s].token == name)
                })
        } else if let Some(a) = ast.cast::<AssignmentExpression>(parent) {
            Some(ast[a].right_hand_side.raw())
        } else if let Some(v) = ast.cast::<VariableDeclaration>(parent) {
            ast[v].initializer.map(|e| e.raw())
        } else {
            None
        };
        expression.is_some_and(|e| ast.is::<SimpleIdentifier>(e)) && lexeme == "call" && enclosing_function()
    }

    fn function_body_keyword(&mut self, keyword: Option<TokenId>, star: Option<TokenId>) {
        if let Some(k) = keyword {
            let ast = self.ast();
            let offset = ast.tokens.get(k).offset;
            let end = star.map(|s| ast.tokens.get(s).end()).unwrap_or(ast.tokens.get(k).end());
            self.add_region(offset, end - offset, H::KEYWORD, control());
        }
    }

    fn configurations(&mut self, configurations: NodeList<Configuration>) {
        let ast = self.ast();
        for &c in ast.list(configurations) {
            self.token(Some(ast[c].if_keyword), H::KEYWORD, control());
        }
    }

    /// Dart `_addRegions_stringEscapes`.
    fn string_escapes(&mut self, string: &[u16], quote: Quote, node_offset: u32, start: usize, end: usize) {
        if matches!(
            quote,
            Quote::RawSingle | Quote::RawDouble | Quote::RawMultiLineSingle | Quote::RawMultiLineDouble
        ) {
            return;
        }
        for (offset, escape_end) in find_escapes(string, start, end) {
            self.add_region(
                node_offset + offset as u32,
                (escape_end - offset) as u32,
                H::VALID_STRING_ESCAPE,
                Opts::default(),
            );
        }
    }

    fn element_of(&self, node: NodeId) -> Option<ElementId> {
        self.unit
            .tables
            .element
            .get(node)
            .map(|&e| member::base_element(self.ctx(), e))
    }

    fn declared(&self, node: NodeId) -> Option<ElementId> {
        self.unit.declared_element(node)
    }
}

/// Dart `PropertyAccessImpl.realTarget`.
fn property_access_real_target(ast: &Ast, node: Id<PropertyAccess>) -> Option<Id<Expression>> {
    if dartr_resolver::ast_ext::property_access_is_cascaded(ast, node) {
        return dartr_resolver::ast_ext::ancestor_cascade_target(ast, node.raw());
    }
    ast[node].target
}

/// Dart `_findEscapes`: (offset, end) of each escape in [string] (UTF-16).
fn find_escapes(s: &[u16], start: usize, end: usize) -> Vec<(usize, usize)> {
    let length = s.len();
    let at = |i: usize, c: u8| i < length && s[i] == c as u16;
    let hex_digits = |i: usize, min: usize, max: usize| -> Option<usize> {
        let mut n = 0;
        let mut j = i;
        while j < (i + max).min(length) {
            let c = s[j];
            let hex = (c >= b'0' as u16 && c <= b'9' as u16)
                || (c >= b'a' as u16 && c <= b'f' as u16)
                || (c >= b'A' as u16 && c <= b'F' as u16);
            if !hex {
                break;
            }
            n += 1;
            j += 1;
        }
        (n >= min).then_some(n)
    };
    let mut out = Vec::new();
    let mut i = start;
    while i < end {
        if at(i, b'\\') {
            let backslash = i;
            i += 1;
            if at(i, b'x') {
                if let Some(n) = hex_digits(i + 1, 2, 2) {
                    i += 1 + n;
                    out.push((backslash, i));
                }
            } else if at(i, b'u') && at(i + 1, b'{') {
                if let Some(n) = hex_digits(i + 2, 1, 6)
                    && at(i + 2 + n, b'}')
                {
                    i += 2 + n + 1;
                    out.push((backslash, i));
                }
            } else if at(i, b'u') {
                if let Some(n) = hex_digits(i + 1, 4, 4) {
                    i += 1 + n;
                    out.push((backslash, i));
                }
            } else {
                i += 1;
                out.push((backslash, i));
            }
        } else {
            i += 1;
        }
    }
    out
}

macro_rules! kw {
    ($self:ident, $ast:ident, $node:ident, $($field:ident),*) => {
        $( $self.token(opt($ast[$node].$field), H::KEYWORD, Opts::default()); )*
    };
}

/// A token or an optional token.
trait OptToken {
    fn opt(self) -> Option<TokenId>;
}

impl OptToken for TokenId {
    fn opt(self) -> Option<TokenId> {
        Some(self)
    }
}

impl OptToken for Option<TokenId> {
    fn opt(self) -> Option<TokenId> {
        self
    }
}

fn opt(t: impl OptToken) -> Option<TokenId> {
    t.opt()
}

impl AstVisitor for Computer<'_, '_, '_> {
    fn visit_annotation(&mut self, ast: &Ast, node: Id<Annotation>) {
        match ast[node].arguments {
            None => {
                self.node(node.raw(), H::ANNOTATION, Opts::default());
            }
            Some(arguments) => {
                let offset = ast.offset(node.raw());
                let begin = ast.tokens.get(ast[arguments].left_parenthesis).end();
                self.add_region(offset, begin - offset, H::ANNOTATION, Opts::default());
                self.token(Some(ast[arguments].right_parenthesis), H::ANNOTATION, Opts::default());
            }
        }
        ast.visit_children(node.raw(), self);
    }

    fn visit_as_expression(&mut self, ast: &Ast, node: Id<AsExpression>) {
        kw!(self, ast, node, as_operator);
        ast.visit_children(node.raw(), self);
    }

    fn visit_assert_statement(&mut self, ast: &Ast, node: Id<AssertStatement>) {
        self.token(Some(ast[node].assert_keyword), H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_assigned_variable_pattern(&mut self, ast: &Ast, node: Id<AssignedVariablePattern>) {
        self.token(Some(ast[node].name), H::LOCAL_VARIABLE_REFERENCE, Opts::default());
        ast.visit_children(node.raw(), self);
    }

    fn visit_await_expression(&mut self, ast: &Ast, node: Id<AwaitExpression>) {
        self.token(Some(ast[node].await_keyword), H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_block_function_body(&mut self, ast: &Ast, node: Id<BlockFunctionBody>) {
        self.function_body_keyword(ast[node].keyword, ast[node].star);
        ast.visit_children(node.raw(), self);
    }

    fn visit_boolean_literal(&mut self, ast: &Ast, node: Id<BooleanLiteral>) {
        self.node(node.raw(), H::KEYWORD, Opts::default());
        self.node(node.raw(), H::LITERAL_BOOLEAN, Opts::default());
        ast.visit_children(node.raw(), self);
    }

    fn visit_break_statement(&mut self, ast: &Ast, node: Id<BreakStatement>) {
        self.token(Some(ast[node].break_keyword), H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_case_clause(&mut self, ast: &Ast, node: Id<CaseClause>) {
        self.token(Some(ast[node].case_keyword), H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_cast_pattern(&mut self, ast: &Ast, node: Id<CastPattern>) {
        kw!(self, ast, node, as_token);
        ast.visit_children(node.raw(), self);
    }

    fn visit_catch_clause(&mut self, ast: &Ast, node: Id<CatchClause>) {
        self.token(ast[node].catch_keyword, H::KEYWORD, control());
        self.token(ast[node].on_keyword, H::KEYWORD, control());
        for p in [ast[node].exception_parameter, ast[node].stack_trace_parameter]
            .into_iter()
            .flatten()
        {
            let additional = self.wildcard(self.declared(p.raw()));
            self.token(
                Some(ast[p].name),
                H::LOCAL_VARIABLE_DECLARATION,
                Opts {
                    additional,
                    ..Opts::default()
                },
            );
        }
        ast.visit_children(node.raw(), self);
    }

    fn visit_class_declaration(&mut self, ast: &Ast, node: Id<ClassDeclaration>) {
        kw!(
            self,
            ast,
            node,
            augment_keyword,
            abstract_keyword,
            sealed_keyword,
            base_keyword,
            interface_keyword,
            final_keyword,
            mixin_keyword,
            class_keyword
        );
        let name = class_name_part_type_name(ast, ast[node].name_part.raw());
        self.token(name, H::CLASS, m(&["declaration"]));
        ast.visit_children(node.raw(), self);
    }

    fn visit_class_type_alias(&mut self, ast: &Ast, node: Id<ClassTypeAlias>) {
        kw!(
            self,
            ast,
            node,
            abstract_keyword,
            sealed_keyword,
            base_keyword,
            interface_keyword,
            final_keyword,
            mixin_keyword
        );
        ast.visit_children(node.raw(), self);
    }

    fn visit_comment(&mut self, ast: &Ast, node: Id<Comment>) {
        ast.visit_children(node.raw(), self);
        for code in &ast[node].code_blocks {
            for line in &code.lines {
                self.add_region(
                    line.offset,
                    line.length,
                    H::COMMENT_DOCUMENTATION,
                    Opts {
                        additional: Some(vec!["source"]),
                        ..Opts::default()
                    },
                );
            }
        }
    }

    fn visit_constant_pattern(&mut self, ast: &Ast, node: Id<ConstantPattern>) {
        kw!(self, ast, node, const_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_constructor_declaration(&mut self, ast: &Ast, node: Id<ConstructorDeclaration>) {
        kw!(self, ast, node, augment_keyword, external_keyword);
        let factory_opts = if ast[node].type_name.is_some() {
            Opts::default()
        } else {
            m(&["constructor", "declaration"])
        };
        self.token(ast[node].factory_keyword, H::KEYWORD, factory_opts);
        kw!(self, ast, node, const_keyword);
        self.token(ast[node].new_keyword, H::KEYWORD, m(&["constructor", "declaration"]));
        self.token(
            ast[node].name,
            H::CONSTRUCTOR,
            Opts {
                ty: Some("method"),
                modifiers: Some(vec!["constructor", "declaration"]),
                additional: None,
            },
        );
        ast.visit_children(node.raw(), self);
    }

    fn visit_constructor_reference(&mut self, ast: &Ast, node: Id<ConstructorReference>) {
        let constructor_name = ast[node].constructor_name;
        ast.accept(ast[constructor_name].type_.raw(), self);
        if let Some(name) = ast[constructor_name].name {
            self.node(name.raw(), H::CONSTRUCTOR_TEAR_OFF, Opts::default());
        }
    }

    fn visit_constructor_selector(&mut self, ast: &Ast, node: Id<ConstructorSelector>) {
        self.node(ast[node].name.raw(), H::CONSTRUCTOR, Opts::default());
    }

    fn visit_continue_statement(&mut self, ast: &Ast, node: Id<ContinueStatement>) {
        self.token(Some(ast[node].continue_keyword), H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_declared_identifier(&mut self, ast: &Ast, node: Id<DeclaredIdentifier>) {
        kw!(self, ast, node, keyword);
        let additional = self.wildcard(self.declared(node.raw()));
        self.token(
            Some(ast[node].name),
            H::LOCAL_VARIABLE_DECLARATION,
            Opts {
                additional,
                ..Opts::default()
            },
        );
        ast.visit_children(node.raw(), self);
    }

    fn visit_declared_variable_pattern(&mut self, ast: &Ast, node: Id<DeclaredVariablePattern>) {
        kw!(self, ast, node, keyword);
        self.token(Some(ast[node].name), H::LOCAL_VARIABLE_DECLARATION, Opts::default());
        ast.visit_children(node.raw(), self);
    }

    fn visit_do_statement(&mut self, ast: &Ast, node: Id<DoStatement>) {
        self.token(Some(ast[node].do_keyword), H::KEYWORD, control());
        self.token(Some(ast[node].while_keyword), H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_dot_shorthand_property_access(&mut self, ast: &Ast, node: Id<DotShorthandPropertyAccess>) {
        let property = ast[node].property_name;
        let element = self.element_of(property.raw());
        if element.is_some_and(|e| e.tag() == Tag::Constructor) {
            self.node(property.raw(), H::CONSTRUCTOR_TEAR_OFF, Opts::default());
        } else {
            self.identifier(node.raw(), ast[property].token, element);
        }
    }

    fn visit_dotted_name(&mut self, ast: &Ast, node: Id<DottedName>) {
        if ast.parent(node.raw()).is_some_and(|p| ast.is::<Configuration>(p)) {
            for &t in ast.token_list(ast[node].tokens) {
                if ast.tokens.ty(t) != TokenType::PERIOD {
                    self.token(Some(t), H::IDENTIFIER_DEFAULT, Opts::default());
                }
            }
        } else {
            self.node(node.raw(), H::LIBRARY_NAME, Opts::default());
        }
        ast.visit_children(node.raw(), self);
    }

    fn visit_double_literal(&mut self, ast: &Ast, node: Id<DoubleLiteral>) {
        self.node(node.raw(), H::LITERAL_DOUBLE, Opts::default());
        ast.visit_children(node.raw(), self);
    }

    fn visit_enum_constant_declaration(&mut self, ast: &Ast, node: Id<EnumConstantDeclaration>) {
        self.token(Some(ast[node].name), H::ENUM_CONSTANT, Opts::default());
        ast.visit_children(node.raw(), self);
    }

    fn visit_enum_declaration(&mut self, ast: &Ast, node: Id<EnumDeclaration>) {
        kw!(self, ast, node, enum_keyword);
        let name = class_name_part_type_name(ast, ast[node].name_part.raw());
        self.token(name, H::ENUM, Opts::default());
        ast.visit_children(node.raw(), self);
    }

    fn visit_export_directive(&mut self, ast: &Ast, node: Id<ExportDirective>) {
        kw!(self, ast, node, export_keyword);
        self.configurations(ast[node].configurations);
        ast.visit_children(node.raw(), self);
    }

    fn visit_expression_function_body(&mut self, ast: &Ast, node: Id<ExpressionFunctionBody>) {
        self.function_body_keyword(ast[node].keyword, ast[node].star);
        ast.visit_children(node.raw(), self);
    }

    fn visit_extends_clause(&mut self, ast: &Ast, node: Id<ExtendsClause>) {
        kw!(self, ast, node, extends_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_extension_declaration(&mut self, ast: &Ast, node: Id<ExtensionDeclaration>) {
        kw!(self, ast, node, augment_keyword, extension_keyword);
        self.token(ast[node].name, H::EXTENSION, Opts::default());
        ast.visit_children(node.raw(), self);
    }

    fn visit_extension_on_clause(&mut self, ast: &Ast, node: Id<ExtensionOnClause>) {
        kw!(self, ast, node, on_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_extension_override(&mut self, ast: &Ast, node: Id<ExtensionOverride>) {
        self.token(Some(ast[node].name), H::EXTENSION, Opts::default());
        ast.visit_children(node.raw(), self);
    }

    fn visit_extension_type_declaration(&mut self, ast: &Ast, node: Id<ExtensionTypeDeclaration>) {
        kw!(self, ast, node, extension_keyword, type_keyword);
        let name = class_name_part_type_name(ast, ast[node].name_part.raw());
        self.token(name, H::EXTENSION_TYPE, m(&["declaration"]));
        ast.visit_children(node.raw(), self);
    }

    fn visit_field_declaration(&mut self, ast: &Ast, node: Id<FieldDeclaration>) {
        kw!(self, ast, node, abstract_keyword, external_keyword, static_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_field_formal_parameter(&mut self, ast: &Ast, node: Id<FieldFormalParameter>) {
        kw!(self, ast, node, required_keyword, const_final_or_var_keyword, this_keyword);
        let additional = self.wildcard(self.declared(node.raw()));
        self.token(
            Some(ast[node].name),
            H::INSTANCE_FIELD_REFERENCE,
            Opts {
                additional,
                ..Opts::default()
            },
        );
        ast.visit_children(node.raw(), self);
    }

    fn visit_for_each_parts_with_declaration(&mut self, ast: &Ast, node: Id<ForEachPartsWithDeclaration>) {
        self.token(Some(ast[node].in_keyword), H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_for_each_parts_with_identifier(&mut self, ast: &Ast, node: Id<ForEachPartsWithIdentifier>) {
        self.token(Some(ast[node].in_keyword), H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_for_each_parts_with_pattern(&mut self, ast: &Ast, node: Id<ForEachPartsWithPattern>) {
        kw!(self, ast, node, keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_for_element(&mut self, ast: &Ast, node: Id<ForElement>) {
        self.token(ast[node].await_keyword, H::KEYWORD, control());
        self.token(Some(ast[node].for_keyword), H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_for_statement(&mut self, ast: &Ast, node: Id<ForStatement>) {
        self.token(ast[node].await_keyword, H::KEYWORD, control());
        self.token(Some(ast[node].for_keyword), H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_function_declaration(&mut self, ast: &Ast, node: Id<FunctionDeclaration>) {
        kw!(self, ast, node, augment_keyword, external_keyword, property_keyword);
        let property = ast[node].property_keyword.map(|t| ast.tokens.lexeme(t));
        let h = if property == Some("get") {
            H::TOP_LEVEL_GETTER_DECLARATION
        } else if property == Some("set") {
            H::TOP_LEVEL_SETTER_DECLARATION
        } else if ast.parent(node.raw()).is_some_and(|p| ast.is::<CompilationUnit>(p)) {
            H::TOP_LEVEL_FUNCTION_DECLARATION
        } else {
            H::LOCAL_FUNCTION_DECLARATION
        };
        self.token(Some(ast[node].name), h, Opts::default());
        ast.visit_children(node.raw(), self);
    }

    fn visit_function_type_alias(&mut self, ast: &Ast, node: Id<FunctionTypeAlias>) {
        kw!(self, ast, node, typedef_keyword);
        self.token(Some(ast[node].name), H::FUNCTION_TYPE_ALIAS, Opts::default());
        ast.visit_children(node.raw(), self);
    }

    fn visit_generic_function_type(&mut self, ast: &Ast, node: Id<GenericFunctionType>) {
        kw!(self, ast, node, function_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_generic_type_alias(&mut self, ast: &Ast, node: Id<GenericTypeAlias>) {
        kw!(self, ast, node, typedef_keyword);
        let h = if ast.is::<GenericFunctionType>(ast[node].type_.raw()) {
            H::FUNCTION_TYPE_ALIAS
        } else {
            H::TYPE_ALIAS
        };
        self.token(Some(ast[node].name), h, Opts::default());
        ast.visit_children(node.raw(), self);
    }

    fn visit_hide_combinator(&mut self, ast: &Ast, node: Id<HideCombinator>) {
        kw!(self, ast, node, keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_if_element(&mut self, ast: &Ast, node: Id<IfElement>) {
        self.token(Some(ast[node].if_keyword), H::KEYWORD, control());
        self.token(ast[node].else_keyword, H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_if_statement(&mut self, ast: &Ast, node: Id<IfStatement>) {
        self.token(Some(ast[node].if_keyword), H::KEYWORD, control());
        self.token(ast[node].else_keyword, H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_implements_clause(&mut self, ast: &Ast, node: Id<ImplementsClause>) {
        kw!(self, ast, node, implements_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_import_directive(&mut self, ast: &Ast, node: Id<ImportDirective>) {
        kw!(self, ast, node, import_keyword, deferred_keyword, as_keyword);
        self.configurations(ast[node].configurations);
        ast.visit_children(node.raw(), self);
    }

    fn visit_import_prefix_reference(&mut self, ast: &Ast, node: Id<ImportPrefixReference>) {
        self.token(Some(ast[node].name), H::IMPORT_PREFIX, Opts::default());
    }

    fn visit_instance_creation_expression(&mut self, ast: &Ast, node: Id<InstanceCreationExpression>) {
        kw!(self, ast, node, keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_integer_literal(&mut self, ast: &Ast, node: Id<IntegerLiteral>) {
        self.node(node.raw(), H::LITERAL_INTEGER, Opts::default());
        ast.visit_children(node.raw(), self);
    }

    fn visit_interpolation_expression(&mut self, ast: &Ast, node: Id<InterpolationExpression>) {
        self.node(
            node.raw(),
            H::LITERAL_STRING,
            Opts {
                ty: Some("source"),
                modifiers: Some(vec!["interpolation"]),
                additional: None,
            },
        );
        ast.visit_children(node.raw(), self);
    }

    fn visit_interpolation_string(&mut self, ast: &Ast, node: Id<InterpolationString>) {
        self.node(node.raw(), H::LITERAL_STRING, Opts::default());
        let string: Vec<u16> = ast.tokens.lexeme(ast[node].contents).encode_utf16().collect();
        let quote = ast
            .parent(node.raw())
            .and_then(|p| ast.cast::<StringInterpolation>(p))
            .map(|p| interpolation_quote(ast, p))
            .unwrap_or(Quote::Single);
        let offset = ast.offset(node.raw());
        let len = string.len();
        self.string_escapes(&string, quote, offset, 0, len);
        ast.visit_children(node.raw(), self);
    }

    fn visit_is_expression(&mut self, ast: &Ast, node: Id<IsExpression>) {
        kw!(self, ast, node, is_operator);
        ast.visit_children(node.raw(), self);
    }

    fn visit_label(&mut self, ast: &Ast, node: Id<Label>) {
        self.token(Some(ast[node].name), H::LABEL, m(&["declaration"]));
        ast.visit_children(node.raw(), self);
    }

    fn visit_label_reference(&mut self, ast: &Ast, node: Id<LabelReference>) {
        self.token(Some(ast[node].name), H::LABEL, Opts::default());
        ast.visit_children(node.raw(), self);
    }

    fn visit_library_directive(&mut self, ast: &Ast, node: Id<LibraryDirective>) {
        kw!(self, ast, node, library_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_list_literal(&mut self, ast: &Ast, node: Id<ListLiteral>) {
        kw!(self, ast, node, const_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_method_declaration(&mut self, ast: &Ast, node: Id<MethodDeclaration>) {
        kw!(
            self,
            ast,
            node,
            augment_keyword,
            external_keyword,
            modifier_keyword,
            operator_keyword,
            property_keyword
        );
        let property = ast[node].property_keyword.map(|t| ast.tokens.lexeme(t));
        let is_static = ast[node].modifier_keyword.map(|t| ast.tokens.lexeme(t)) == Some("static");
        let h = match (property, is_static) {
            (Some("get"), true) => H::STATIC_GETTER_DECLARATION,
            (Some("get"), false) => H::INSTANCE_GETTER_DECLARATION,
            (Some("set"), true) => H::STATIC_SETTER_DECLARATION,
            (Some("set"), false) => H::INSTANCE_SETTER_DECLARATION,
            (_, true) => H::STATIC_METHOD_DECLARATION,
            (_, false) => H::INSTANCE_METHOD_DECLARATION,
        };
        self.token(Some(ast[node].name), h, Opts::default());
        ast.visit_children(node.raw(), self);
    }

    fn visit_mixin_declaration(&mut self, ast: &Ast, node: Id<MixinDeclaration>) {
        kw!(self, ast, node, augment_keyword, base_keyword, mixin_keyword);
        self.token(Some(ast[node].name), H::MIXIN, Opts::default());
        ast.visit_children(node.raw(), self);
    }

    fn visit_mixin_on_clause(&mut self, ast: &Ast, node: Id<MixinOnClause>) {
        kw!(self, ast, node, on_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_named_argument(&mut self, ast: &Ast, node: Id<NamedArgument>) {
        let parameter = self
            .unit
            .tables
            .param_element
            .get(node.raw())
            .or_else(|| self.unit.tables.param_element.get(ast[node].argument_expression.raw()))
            .copied();
        match parameter {
            Some(p) => {
                let ty = member::type_(self.ctx(), p);
                let h = if self.is_dynamic(ty) {
                    H::DYNAMIC_PARAMETER_REFERENCE
                } else {
                    H::PARAMETER_REFERENCE
                };
                self.token(Some(ast[node].name), h, m(&["label"]));
            }
            None => self.identifier(node.raw(), ast[node].name, None),
        }
        ast.accept(ast[node].argument_expression.raw(), self);
    }

    fn visit_named_type(&mut self, ast: &Ast, node: Id<NamedType>) {
        if let Some(prefix) = ast[node].import_prefix {
            self.token(Some(ast[prefix].name), H::IMPORT_PREFIX, Opts::default());
        }
        if let Some(t) = self.unit.tables.annotation_type.get(node.raw()).copied() {
            let name = ast[node].name;
            let is_dynamic =
                matches!(self.ctx().ty(t), TypeKind::Dynamic) && ast.tokens.lexeme(name) == "dynamic";
            let is_never = matches!(self.ctx().ty(t), TypeKind::Never(_));
            if is_dynamic || is_never {
                let h = if is_dynamic { H::TYPE_NAME_DYNAMIC } else { H::CLASS };
                self.token(
                    Some(name),
                    h,
                    Opts {
                        ty: Some("type"),
                        ..Opts::default()
                    },
                );
                return;
            }
        }
        let element = self.element_of(node.raw());
        self.identifier(node.raw(), ast[node].name, element);
        if let Some(args) = ast[node].type_arguments {
            ast.accept(args.raw(), self);
        }
    }

    fn visit_native_clause(&mut self, ast: &Ast, node: Id<NativeClause>) {
        kw!(self, ast, node, native_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_native_function_body(&mut self, ast: &Ast, node: Id<NativeFunctionBody>) {
        kw!(self, ast, node, native_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_null_literal(&mut self, ast: &Ast, node: Id<NullLiteral>) {
        kw!(self, ast, node, literal);
        ast.visit_children(node.raw(), self);
    }

    fn visit_part_directive(&mut self, ast: &Ast, node: Id<PartDirective>) {
        kw!(self, ast, node, part_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_part_of_directive(&mut self, ast: &Ast, node: Id<PartOfDirective>) {
        let start = ast.tokens.get(ast[node].part_keyword).offset;
        let end = ast.tokens.get(ast[node].of_keyword).end();
        self.add_region(start, end - start, H::KEYWORD, Opts::default());
        ast.visit_children(node.raw(), self);
    }

    fn visit_pattern_field(&mut self, ast: &Ast, node: Id<PatternField>) {
        if let Some(name) = ast[node].name.and_then(|n| ast[n].name) {
            let h = if self.element_of(node.raw()).is_some_and(|e| e.tag() == Tag::Method) {
                H::INSTANCE_METHOD_TEAR_OFF
            } else {
                H::INSTANCE_GETTER_REFERENCE
            };
            self.token(Some(name), h, Opts::default());
        }
        ast.visit_children(node.raw(), self);
    }

    fn visit_pattern_variable_declaration(&mut self, ast: &Ast, node: Id<PatternVariableDeclaration>) {
        kw!(self, ast, node, keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_primary_constructor_body(&mut self, ast: &Ast, node: Id<PrimaryConstructorBody>) {
        kw!(self, ast, node, this_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_primary_constructor_declaration(&mut self, ast: &Ast, node: Id<PrimaryConstructorDeclaration>) {
        kw!(self, ast, node, const_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_primary_constructor_name(&mut self, ast: &Ast, node: Id<PrimaryConstructorName>) {
        self.token(Some(ast[node].name), H::CONSTRUCTOR, Opts::default());
        ast.visit_children(node.raw(), self);
    }

    fn visit_record_literal(&mut self, ast: &Ast, node: Id<RecordLiteral>) {
        kw!(self, ast, node, const_keyword);
        for &field in ast.list_raw(ast[node].fields) {
            if let Some(named) = ast.cast::<RecordLiteralNamedField>(field) {
                self.token(Some(ast[named].name), H::PARAMETER_REFERENCE, Opts::default());
                ast.accept(ast[named].field_expression.raw(), self);
            } else {
                ast.accept(field, self);
            }
        }
    }

    fn visit_regular_formal_parameter(&mut self, ast: &Ast, node: Id<RegularFormalParameter>) {
        kw!(self, ast, node, required_keyword, covariant_keyword, const_final_or_var_keyword);
        let declared = self.declared(node.raw());
        let h = if declared.is_some_and(|e| self.is_dynamic(self.type_of(e))) {
            H::DYNAMIC_PARAMETER_DECLARATION
        } else {
            H::PARAMETER_DECLARATION
        };
        let additional = self.wildcard(declared);
        self.token(
            ast[node].name,
            h,
            Opts {
                additional,
                ..Opts::default()
            },
        );
        ast.visit_children(node.raw(), self);
    }

    fn visit_rethrow_expression(&mut self, ast: &Ast, node: Id<RethrowExpression>) {
        self.token(Some(ast[node].rethrow_keyword), H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_return_statement(&mut self, ast: &Ast, node: Id<ReturnStatement>) {
        self.token(Some(ast[node].return_keyword), H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_set_or_map_literal(&mut self, ast: &Ast, node: Id<SetOrMapLiteral>) {
        kw!(self, ast, node, const_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_show_combinator(&mut self, ast: &Ast, node: Id<ShowCombinator>) {
        kw!(self, ast, node, keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        if let Some(parent) = ast.parent(node.raw()) {
            let element = self.unit.write_or_read_element(node);
            self.identifier(parent, ast[node].token, element);
        }
        ast.visit_children(node.raw(), self);
    }

    fn visit_simple_string_literal(&mut self, ast: &Ast, node: Id<SimpleStringLiteral>) {
        self.node(node.raw(), H::LITERAL_STRING, Opts::default());
        let lexeme = ast.tokens.lexeme(ast[node].literal);
        let quote = analyze_quote(lexeme);
        let string: Vec<u16> = lexeme.encode_utf16().collect();
        let start = first_quote_length(lexeme, quote);
        let end = string.len().saturating_sub(last_quote_length(quote));
        let offset = ast.offset(node.raw());
        self.string_escapes(&string, quote, offset, start, end);
        ast.visit_children(node.raw(), self);
    }

    fn visit_super_constructor_invocation(&mut self, ast: &Ast, node: Id<SuperConstructorInvocation>) {
        kw!(self, ast, node, super_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_super_expression(&mut self, ast: &Ast, node: Id<SuperExpression>) {
        kw!(self, ast, node, super_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_super_formal_parameter(&mut self, ast: &Ast, node: Id<SuperFormalParameter>) {
        kw!(self, ast, node, required_keyword, const_final_or_var_keyword, super_keyword);
        let declared = self.declared(node.raw());
        let h = if declared.is_some_and(|e| self.is_dynamic(self.type_of(e))) {
            H::DYNAMIC_PARAMETER_DECLARATION
        } else {
            H::PARAMETER_DECLARATION
        };
        self.token(Some(ast[node].name), h, Opts::default());
        ast.visit_children(node.raw(), self);
    }

    fn visit_switch_case(&mut self, ast: &Ast, node: Id<SwitchCase>) {
        self.token(Some(ast[node].keyword), H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_switch_default(&mut self, ast: &Ast, node: Id<SwitchDefault>) {
        self.token(Some(ast[node].keyword), H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_switch_expression(&mut self, ast: &Ast, node: Id<SwitchExpression>) {
        kw!(self, ast, node, switch_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_switch_pattern_case(&mut self, ast: &Ast, node: Id<SwitchPatternCase>) {
        self.token(Some(ast[node].keyword), H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_switch_statement(&mut self, ast: &Ast, node: Id<SwitchStatement>) {
        self.token(Some(ast[node].switch_keyword), H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_this_expression(&mut self, ast: &Ast, node: Id<ThisExpression>) {
        kw!(self, ast, node, this_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_throw_expression(&mut self, ast: &Ast, node: Id<ThrowExpression>) {
        self.token(Some(ast[node].throw_keyword), H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_top_level_variable_declaration(&mut self, ast: &Ast, node: Id<TopLevelVariableDeclaration>) {
        kw!(self, ast, node, augment_keyword, external_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_try_statement(&mut self, ast: &Ast, node: Id<TryStatement>) {
        self.token(Some(ast[node].try_keyword), H::KEYWORD, control());
        self.token(ast[node].finally_keyword, H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_type_parameter(&mut self, ast: &Ast, node: Id<TypeParameter>) {
        let additional = self.wildcard(self.declared(node.raw()));
        self.token(
            Some(ast[node].name),
            H::TYPE_PARAMETER,
            Opts {
                additional,
                ..Opts::default()
            },
        );
        kw!(self, ast, node, extends_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_variable_declaration(&mut self, ast: &Ast, node: Id<VariableDeclaration>) {
        let element = self.declared(node.raw());
        match element.map(|e| (e, e.tag())) {
            Some((e, Tag::Field)) => {
                let h = if self.is_static(e) {
                    H::STATIC_FIELD_DECLARATION
                } else {
                    H::INSTANCE_FIELD_DECLARATION
                };
                self.token(Some(ast[node].name), h, Opts::default());
            }
            Some((e, Tag::LocalVariable)) => {
                let h = if self.is_dynamic(self.type_of(e)) {
                    H::DYNAMIC_LOCAL_VARIABLE_DECLARATION
                } else {
                    H::LOCAL_VARIABLE_DECLARATION
                };
                let additional = self.wildcard(Some(e));
                self.token(
                    Some(ast[node].name),
                    h,
                    Opts {
                        additional,
                        ..Opts::default()
                    },
                );
            }
            Some((_, Tag::TopLevelVariable)) => {
                self.token(Some(ast[node].name), H::TOP_LEVEL_VARIABLE_DECLARATION, Opts::default());
            }
            _ => {}
        }
        ast.visit_children(node.raw(), self);
    }

    fn visit_variable_declaration_list(&mut self, ast: &Ast, node: Id<VariableDeclarationList>) {
        kw!(self, ast, node, late_keyword, keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_when_clause(&mut self, ast: &Ast, node: Id<WhenClause>) {
        self.token(Some(ast[node].when_keyword), H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_while_statement(&mut self, ast: &Ast, node: Id<WhileStatement>) {
        self.token(Some(ast[node].while_keyword), H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }

    fn visit_wildcard_pattern(&mut self, ast: &Ast, node: Id<WildcardPattern>) {
        kw!(self, ast, node, keyword);
        self.token(
            Some(ast[node].name),
            H::LOCAL_VARIABLE_DECLARATION,
            Opts {
                additional: Some(vec!["wildcard"]),
                ..Opts::default()
            },
        );
        ast.visit_children(node.raw(), self);
    }

    fn visit_with_clause(&mut self, ast: &Ast, node: Id<WithClause>) {
        kw!(self, ast, node, with_keyword);
        ast.visit_children(node.raw(), self);
    }

    fn visit_yield_statement(&mut self, ast: &Ast, node: Id<YieldStatement>) {
        let keyword = ast[node].yield_keyword;
        let offset = ast.tokens.get(keyword).offset;
        let end = ast[node]
            .star
            .map(|s| ast.tokens.get(s).end())
            .unwrap_or(ast.tokens.get(keyword).end());
        self.add_region(offset, end - offset, H::KEYWORD, control());
        ast.visit_children(node.raw(), self);
    }
}

/// Dart `ClassNamePart.typeName`.
fn class_name_part_type_name(ast: &Ast, part: NodeId) -> Option<TokenId> {
    if let Some(n) = ast.cast::<NameWithTypeParameters>(part) {
        return Some(ast[n].type_name);
    }
    ast.cast::<PrimaryConstructorDeclaration>(part).map(|p| ast[p].type_name)
}

/// The quote of a string interpolation (Dart `StringInterpolation.quote`):
/// from its first token.
fn interpolation_quote(ast: &Ast, node: Id<StringInterpolation>) -> Quote {
    let first = ast.list_raw(ast[node].elements).first().copied();
    match first.and_then(|f| ast.cast::<InterpolationString>(f)) {
        Some(s) => analyze_quote(ast.tokens.lexeme(ast[s].contents)),
        None => Quote::Single,
    }
}

/// Dart `SemanticTokenInfo.offsetLengthPrioritySort`.
fn sort_tokens(tokens: &mut [SemanticTokenInfo]) {
    tokens.sort_by(|a, b| {
        a.offset
            .cmp(&b.offset)
            .then(b.length.cmp(&a.length))
            .then_with(|| {
                let p = |t: &SemanticTokenInfo| u8::from(t.ty == "boolean");
                p(a).cmp(&p(b))
            })
            .then_with(|| a.ty.cmp(b.ty))
    });
}

/// Dart `splitOverlappingTokens` (tokens sorted).
fn split_overlapping(sorted: Vec<SemanticTokenInfo>) -> Vec<SemanticTokenInfo> {
    let mut out = Vec::new();
    if sorted.is_empty() {
        return out;
    }
    let mut stack: Vec<SemanticTokenInfo> = Vec::new();
    fn process(
        stack: &mut Vec<SemanticTokenInfo>,
        out: &mut Vec<SemanticTokenInfo>,
        mut from: u32,
        to: u32,
    ) {
        while let Some(last) = stack.last() {
            let last_end = last.offset + last.length;
            let end = last_end.min(to);
            if end > from {
                out.push(SemanticTokenInfo {
                    offset: from,
                    length: end - from,
                    ty: last.ty,
                    modifiers: last.modifiers.clone(),
                });
                from = end;
            }
            if last_end <= to {
                stack.pop();
            } else {
                return;
            }
        }
    }
    let mut last_pos = sorted[0].offset;
    for current in sorted {
        process(&mut stack, &mut out, last_pos, current.offset);
        last_pos = current.offset;
        stack.push(current);
    }
    if let Some(first) = stack.first() {
        let to = first.offset + first.length;
        process(&mut stack, &mut out, last_pos, to);
    }
    out
}

/// Dart `splitMultilineTokens`.
fn split_multiline(token: SemanticTokenInfo, lines: &LineInfo) -> Vec<SemanticTokenInfo> {
    let start = lines.get_location(token.offset);
    let end = lines.get_location(token.offset + token.length);
    let mut out = Vec::new();
    for line in start.line_number..=end.line_number {
        let line_offset = lines.get_offset_of_line((line - 1) as usize).unwrap_or(0);
        let start_offset = if line == start.line_number { start.column_number - 1 } else { 0 };
        let end_offset = if line == end.line_number {
            end.column_number - 1
        } else {
            lines.get_offset_of_line(line as usize).unwrap_or(line_offset) - line_offset
        };
        out.push(SemanticTokenInfo {
            offset: line_offset + start_offset,
            length: end_offset - start_offset,
            ty: token.ty,
            modifiers: token.modifiers.clone(),
        });
    }
    out
}

/// The handler pipeline (Dart `_handleImpl`): sort, split overlapping and
/// multiline tokens, filter by [range], encode.
pub fn encode(mut tokens: Vec<SemanticTokenInfo>, lines: &LineInfo, range: Option<(u32, u32)>) -> Vec<u32> {
    sort_tokens(&mut tokens);
    let tokens = split_overlapping(tokens);
    let tokens: Vec<SemanticTokenInfo> = tokens
        .into_iter()
        .flat_map(|t| split_multiline(t, lines))
        .filter(|t| match range {
            Some((start, end)) => !(t.offset + t.length < start || t.offset > end),
            None => true,
        })
        .collect();
    let mut data = Vec::with_capacity(tokens.len() * 5);
    let (mut last_line, mut last_column) = (0u32, 0u32);
    for t in &tokens {
        let location = lines.get_location(t.offset);
        let line = location.line_number - 1;
        let column = location.column_number - 1;
        let relative_line = line - last_line;
        let relative_column = if relative_line == 0 { column - last_column } else { column };
        let ty = TOKEN_TYPES.iter().position(|x| *x == t.ty).unwrap_or(0) as u32;
        let mask = t.modifiers.as_ref().map_or(0, |ms| {
            ms.iter()
                .map(|m| 1u32 << TOKEN_MODIFIERS.iter().position(|x| x == m).unwrap_or(0))
                .sum()
        });
        data.extend([relative_line, relative_column, t.length, ty, mask]);
        last_line = line;
        last_column = column;
    }
    data
}

/// The name of [element] (Dart `element.name`).
fn element_name<'a>(ctx: &Ctx<'a>, element: ElementId) -> Option<&'a str> {
    let name = ctx.element_data(element)?.name?;
    Some(ctx.name_str(name))
}
