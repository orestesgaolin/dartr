// Dart source: pkg/analyzer/lib/src/summary2/informative_data.dart
// (_InfoBuilder, InformativeDataApplier, the _Info* classes)

//! Informative data: offsets, code ranges and documentation comments of
//! declarations. The Dart code writes them to bytes when the file is read
//! (`writeUnitInformative`) and applies the bytes to the fragments after
//! the elements are built (`InformativeDataApplier.applyFromBytes`), by
//! pairing fragments and declarations in order (`forCorrespondingPairs`).
//!
//! This port builds the same `_Info*` values from the AST and applies them
//! in the same order, without the bytes. The constant offsets
//! (`constantOffsets`) are not ported: the linker copies expressions with
//! their offsets (`ConstExprs`).

use dartr_ast::*;
use dartr_element::{
    ClassFragment, ConstructorFragment, ElementStore, EnumFragment, ExtensionFragment,
    ExtensionTypeFragment, FId, FieldFragment, FormalParameterFragment, FragmentFlags, FragmentId,
    GetterFragment, LibraryElement, LibraryFragment, MixinFragment,
    NamespaceCombinator, SetterFragment, TopLevelFunctionFragment, TopLevelVariableFragment,
    TypeAliasFragment, TypeParameterFragment, EId,
};
use dartr_syntax::TokenId;
use std::sync::Arc;

use crate::ast_util::{
    class_body_members, class_name_part_name, class_name_part_type_parameters,
    enum_body_constants, enum_body_members, function_is_getter, function_is_setter,
    method_is_getter, method_is_setter, offset_if_not_empty,
};

/// Dart `_InfoNode` data shared by all declarations.
#[derive(Clone, Debug, Default)]
pub struct InfoNode {
    pub code_offset: u32,
    pub code_length: u32,
    pub first_token_offset: u32,
    pub name_offset: Option<u32>,
    pub documentation_comment: Option<Arc<str>>,
}

/// Dart `_InfoTypeParameter`.
#[derive(Clone, Debug, Default)]
pub struct InfoTypeParameter {
    pub node: InfoNode,
}

/// Dart `_InfoFormalParameter`.
#[derive(Clone, Debug, Default)]
pub struct InfoFormalParameter {
    pub node: InfoNode,
    pub type_parameters: Vec<InfoTypeParameter>,
    pub parameters: Vec<InfoFormalParameter>,
}

/// Dart `_InfoExecutableDeclaration`.
#[derive(Clone, Debug, Default)]
pub struct InfoExecutable {
    pub node: InfoNode,
    pub type_parameters: Vec<InfoTypeParameter>,
    pub parameters: Vec<InfoFormalParameter>,
}

/// Dart `_InfoConstructorDeclaration`.
#[derive(Clone, Debug, Default)]
pub struct InfoConstructor {
    pub executable: InfoExecutable,
    pub new_keyword_offset: Option<u32>,
    pub factory_keyword_offset: Option<u32>,
    pub type_name_offset: Option<u32>,
    pub period_offset: Option<u32>,
    pub name_end: Option<u32>,
    pub this_keyword_offset: Option<u32>,
}

/// Dart `_InstanceData` / `_InterfaceData`.
#[derive(Clone, Debug, Default)]
pub struct InfoInstance {
    pub node: InfoNode,
    pub type_parameters: Vec<InfoTypeParameter>,
    pub fields: Vec<InfoNode>,
    pub getters: Vec<InfoExecutable>,
    pub setters: Vec<InfoExecutable>,
    pub methods: Vec<InfoExecutable>,
    pub constructors: Vec<InfoConstructor>,
}

/// Dart `_InfoClassTypeAlias` / `_InfoTypeAlias`.
#[derive(Clone, Debug, Default)]
pub struct InfoTypeAlias {
    pub node: InfoNode,
    pub type_parameters: Vec<InfoTypeParameter>,
}

/// Dart `_InfoCombinator`.
#[derive(Clone, Copy, Debug, Default)]
pub struct InfoCombinator {
    pub offset: u32,
    pub end: u32,
}

/// Dart `_InfoImport`.
#[derive(Clone, Debug, Default)]
pub struct InfoImport {
    pub import_keyword_offset: u32,
    pub prefix_offset: Option<u32>,
    pub combinators: Vec<InfoCombinator>,
}

/// Dart `_InfoExport`.
#[derive(Clone, Debug, Default)]
pub struct InfoExport {
    pub export_keyword_offset: u32,
    pub combinators: Vec<InfoCombinator>,
}

/// Dart `_InfoUnit`.
#[derive(Clone, Debug, Default)]
pub struct InfoUnit {
    pub code_offset: u32,
    pub code_length: u32,
    pub line_starts: Vec<u32>,
    pub library_name_offset: i32,
    pub library_name_length: u32,
    pub doc_comment: Option<Arc<str>>,
    pub imports: Vec<InfoImport>,
    pub exports: Vec<InfoExport>,
    pub parts: Vec<u32>,
    pub class_declarations: Vec<InfoInstance>,
    pub class_type_aliases: Vec<InfoTypeAlias>,
    pub enums: Vec<InfoInstance>,
    pub extensions: Vec<InfoInstance>,
    pub extension_types: Vec<InfoInstance>,
    pub mixin_declarations: Vec<InfoInstance>,
    pub top_level_functions: Vec<InfoExecutable>,
    pub top_level_getters: Vec<InfoExecutable>,
    pub top_level_setters: Vec<InfoExecutable>,
    pub top_level_variable: Vec<InfoNode>,
    pub type_aliases: Vec<InfoTypeAlias>,
}

/// Dart `_InfoBuilder`.
pub struct InfoBuilder<'a> {
    ast: &'a Ast,
}

impl<'a> InfoBuilder<'a> {
    /// Dart `_InfoBuilder.build`.
    pub fn build(ast: &'a Ast, unit: Id<CompilationUnit>, line_starts: &[u32]) -> InfoUnit {
        let b = InfoBuilder { ast };
        let u = ast.get(unit);
        let mut info = InfoUnit {
            code_offset: ast.offset(unit),
            code_length: ast.length(unit),
            line_starts: line_starts.to_vec(),
            library_name_offset: -1,
            ..Default::default()
        };
        for &declaration in ast.list(u.declarations) {
            let d = declaration.raw();
            if let Some(n) = ast.cast::<ClassDeclaration>(d) {
                let n2 = ast.get(n);
                info.class_declarations.push(b.interface_data(
                    d,
                    Some(class_name_part_name(ast, n2.name_part)),
                    class_name_part_type_parameters(ast, n2.name_part),
                    ast.cast::<PrimaryConstructorDeclaration>(n2.name_part.raw()),
                    &class_body_members(ast, n2.body),
                    None,
                ));
            } else if let Some(n) = ast.cast::<ClassTypeAlias>(d) {
                let n = ast.get(n);
                info.class_type_aliases.push(InfoTypeAlias {
                    node: b.node(d, Some(n.name), n.documentation_comment),
                    type_parameters: b.type_parameters(n.type_parameters),
                });
            } else if let Some(n) = ast.cast::<EnumDeclaration>(d) {
                let n2 = ast.get(n);
                let members = enum_body_members(ast, n2.body);
                let mut fields: Vec<InfoNode> = enum_body_constants(ast, n2.body)
                    .into_iter()
                    .map(|c| b.enum_constant(c))
                    .collect();
                for &m in &members {
                    if let Some(f) = ast.cast::<FieldDeclaration>(m.raw()) {
                        for &v in ast.list(ast.get(ast.get(f).fields).variables) {
                            fields.push(b.field(v));
                        }
                    }
                }
                info.enums.push(b.interface_data(
                    d,
                    Some(class_name_part_name(ast, n2.name_part)),
                    class_name_part_type_parameters(ast, n2.name_part),
                    ast.cast::<PrimaryConstructorDeclaration>(n2.name_part.raw()),
                    &members,
                    Some(fields),
                ));
            } else if let Some(n) = ast.cast::<ExtensionDeclaration>(d) {
                let n = ast.get(n);
                info.extensions.push(b.instance_data(
                    d,
                    n.name,
                    n.type_parameters,
                    &class_body_members(ast, n.body),
                    None,
                ));
            } else if let Some(n) = ast.cast::<ExtensionTypeDeclaration>(d) {
                let n2 = ast.get(n);
                info.extension_types.push(b.interface_data(
                    d,
                    Some(class_name_part_name(ast, n2.name_part)),
                    class_name_part_type_parameters(ast, n2.name_part),
                    ast.cast::<PrimaryConstructorDeclaration>(n2.name_part.raw()),
                    &class_body_members(ast, n2.body),
                    None,
                ));
            } else if let Some(n) = ast.cast::<MixinDeclaration>(d) {
                let n2 = ast.get(n);
                info.mixin_declarations.push(b.interface_data(
                    d,
                    Some(n2.name),
                    n2.type_parameters,
                    None,
                    &class_body_members(ast, n2.body),
                    None,
                ));
            } else if let Some(n) = ast.cast::<FunctionDeclaration>(d) {
                let f = b.top_level_function(n);
                if function_is_getter(ast, n) {
                    info.top_level_getters.push(f);
                } else if function_is_setter(ast, n) {
                    info.top_level_setters.push(f);
                } else {
                    info.top_level_functions.push(f);
                }
            } else if let Some(n) = ast.cast::<TopLevelVariableDeclaration>(d) {
                for &v in ast.list(ast.get(ast.get(n).variables).variables) {
                    info.top_level_variable.push(b.variable(v));
                }
            } else if let Some(n) = ast.cast::<FunctionTypeAlias>(d) {
                let n = ast.get(n);
                info.type_aliases.push(InfoTypeAlias {
                    node: b.node(d, Some(n.name), n.documentation_comment),
                    type_parameters: b.type_parameters(n.type_parameters),
                });
            } else if let Some(n) = ast.cast::<GenericTypeAlias>(d) {
                let n = ast.get(n);
                info.type_aliases.push(InfoTypeAlias {
                    node: b.node(d, Some(n.name), n.documentation_comment),
                    type_parameters: b.type_parameters(n.type_parameters),
                });
            }
        }
        let mut first_library_directive = None;
        for &directive in ast.list(u.directives) {
            let d = directive.raw();
            if let Some(n) = ast.cast::<ImportDirective>(d) {
                let n = ast.get(n);
                info.imports.push(InfoImport {
                    import_keyword_offset: ast.tokens.offset(n.import_keyword),
                    prefix_offset: n.prefix.and_then(|p| offset_if_not_empty(ast, Some(ast.get(p).token))),
                    combinators: b.combinators(n.combinators),
                });
            } else if let Some(n) = ast.cast::<ExportDirective>(d) {
                let n = ast.get(n);
                info.exports.push(InfoExport {
                    export_keyword_offset: ast.tokens.offset(n.export_keyword),
                    combinators: b.combinators(n.combinators),
                });
            } else if let Some(n) = ast.cast::<PartDirective>(d) {
                info.parts.push(ast.tokens.offset(ast.get(n).part_keyword));
            } else if let Some(n) = ast.cast::<LibraryDirective>(d) {
                if first_library_directive.is_none() {
                    first_library_directive = Some(n);
                }
            }
        }
        if let Some(name) = first_library_directive.and_then(|l| ast.get(l).name) {
            info.library_name_offset = ast.offset(name) as i32;
            info.library_name_length = ast.length(name);
        }
        info.doc_comment = ast
            .list(u.directives)
            .first()
            .and_then(|&d| b.annotated_comment(d.raw()))
            .and_then(|c| b.comment_raw_text(c));
        info
    }

    /// The documentation comment of an annotated node (its first field).
    fn annotated_comment(&self, node: NodeId) -> Option<Id<Comment>> {
        let ast = self.ast;
        if !ast.kind(node).info().annotated {
            return None;
        }
        // The first child entity of an annotated node with a comment is the
        // comment.
        match ast.child_entities(node).first() {
            Some(Entity::Node(n)) => ast.cast::<Comment>(*n),
            _ => None,
        }
    }

    /// Dart `getCommentNodeRawText`.
    fn comment_raw_text(&self, comment: Id<Comment>) -> Option<Arc<str>> {
        let ast = self.ast;
        let tokens = ast.token_list(ast.get(comment).tokens);
        if tokens.len() == 1 {
            return Some(ast.tokens.lexeme(tokens[0]).replace("\r\n", "\n").into());
        }
        let mut out = String::new();
        for (i, &t) in tokens.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(ast.tokens.lexeme(t));
        }
        Some(out.into())
    }

    fn doc(&self, comment: Option<Id<Comment>>) -> Option<Arc<str>> {
        comment.and_then(|c| self.comment_raw_text(c))
    }

    fn node(&self, node: NodeId, name: Option<TokenId>, comment: Option<Id<Comment>>) -> InfoNode {
        let ast = self.ast;
        InfoNode {
            code_offset: ast.offset(node),
            code_length: ast.length(node),
            first_token_offset: ast.offset(node),
            name_offset: offset_if_not_empty(ast, name),
            documentation_comment: self.doc(comment),
        }
    }

    fn combinators(&self, combinators: NodeList<Combinator>) -> Vec<InfoCombinator> {
        let ast = self.ast;
        ast.list(combinators)
            .iter()
            .map(|&c| InfoCombinator {
                offset: ast.offset(c),
                end: ast.end(c),
            })
            .collect()
    }

    fn type_parameters(&self, list: Option<Id<TypeParameterList>>) -> Vec<InfoTypeParameter> {
        let ast = self.ast;
        let Some(list) = list else { return Vec::new() };
        ast.list(ast.get(list).type_parameters)
            .iter()
            .map(|&tp| {
                let n = ast.get(tp);
                InfoTypeParameter {
                    node: InfoNode {
                        code_offset: ast.offset(tp),
                        code_length: ast.length(tp),
                        first_token_offset: ast.offset(tp),
                        name_offset: offset_if_not_empty(ast, Some(n.name)),
                        documentation_comment: None,
                    },
                }
            })
            .collect()
    }

    /// Dart `_buildFormalParameters`.
    fn formal_parameters(&self, list: Option<Id<FormalParameterList>>) -> Vec<InfoFormalParameter> {
        let ast = self.ast;
        let Some(list) = list else { return Vec::new() };
        ast.list(ast.get(list).parameters)
            .iter()
            .map(|&p| {
                let (name, comment, suffix) = formal_parameter_parts(ast, p);
                InfoFormalParameter {
                    node: InfoNode {
                        code_offset: ast.offset(p),
                        code_length: ast.length(p),
                        first_token_offset: ast.offset(p),
                        name_offset: offset_if_not_empty(ast, name),
                        documentation_comment: self.doc(comment),
                    },
                    type_parameters: self.type_parameters(
                        suffix.and_then(|s| ast.get(s).type_parameters),
                    ),
                    parameters: self.formal_parameters(suffix.map(|s| ast.get(s).formal_parameters)),
                }
            })
            .collect()
    }

    fn method(&self, node: Id<MethodDeclaration>) -> InfoExecutable {
        let n = self.ast.get(node);
        InfoExecutable {
            node: self.node(node.raw(), Some(n.name), n.documentation_comment),
            type_parameters: self.type_parameters(n.type_parameters),
            parameters: self.formal_parameters(n.parameters),
        }
    }

    fn top_level_function(&self, node: Id<FunctionDeclaration>) -> InfoExecutable {
        let ast = self.ast;
        let n = ast.get(node);
        let fe = ast.get(n.function_expression);
        InfoExecutable {
            node: self.node(node.raw(), Some(n.name), n.documentation_comment),
            type_parameters: self.type_parameters(fe.type_parameters),
            parameters: self.formal_parameters(fe.parameters),
        }
    }

    /// Dart `_codeOffsetForVariable`.
    fn code_offset_for_variable(&self, node: Id<VariableDeclaration>) -> u32 {
        let ast = self.ast;
        let list = ast.parent(node).expect("variable list");
        let list_id = ast.cast::<VariableDeclarationList>(list).expect("variable list");
        if ast.list(ast.get(list_id).variables).first() == Some(&node) {
            ast.offset(ast.parent(list).expect("declaration"))
        } else {
            ast.offset(node)
        }
    }

    /// Dart `_buildField` / `_buildTopLevelVariable`.
    fn variable(&self, node: Id<VariableDeclaration>) -> InfoNode {
        let ast = self.ast;
        let code_offset = self.code_offset_for_variable(node);
        let n = ast.get(node);
        InfoNode {
            code_offset,
            code_length: ast.end(node) - code_offset,
            first_token_offset: ast.offset(node),
            name_offset: offset_if_not_empty(ast, Some(n.name)),
            documentation_comment: self.doc(n.documentation_comment),
        }
    }

    fn field(&self, node: Id<VariableDeclaration>) -> InfoNode {
        self.variable(node)
    }

    /// Dart `_buildEnumConstant`.
    fn enum_constant(&self, node: Id<EnumConstantDeclaration>) -> InfoNode {
        let ast = self.ast;
        let n = ast.get(node);
        let code_offset = ast.offset(node);
        InfoNode {
            code_offset,
            code_length: ast.end(node) - code_offset,
            first_token_offset: ast.offset(node),
            name_offset: offset_if_not_empty(ast, Some(n.name)),
            documentation_comment: self.doc(n.documentation_comment),
        }
    }

    /// Dart `_buildInstanceData`.
    fn instance_data(
        &self,
        node: NodeId,
        name: Option<TokenId>,
        type_parameters: Option<Id<TypeParameterList>>,
        members: &[Id<ClassMember>],
        fields: Option<Vec<InfoNode>>,
    ) -> InfoInstance {
        let ast = self.ast;
        let process_fields = fields.is_none();
        let mut fields = fields.unwrap_or_default();
        let mut getters = Vec::new();
        let mut setters = Vec::new();
        let mut methods = Vec::new();
        for &member in members {
            let m = member.raw();
            if let Some(md) = ast.cast::<MethodDeclaration>(m) {
                if method_is_getter(ast, md) {
                    getters.push(self.method(md));
                } else if method_is_setter(ast, md) {
                    setters.push(self.method(md));
                } else {
                    methods.push(self.method(md));
                }
            } else if process_fields && let Some(f) = ast.cast::<FieldDeclaration>(m) {
                for &v in ast.list(ast.get(ast.get(f).fields).variables) {
                    fields.push(self.field(v));
                }
            }
        }
        InfoInstance {
            node: self.node(node, name, self.annotated_comment(node)),
            type_parameters: self.type_parameters(type_parameters),
            fields,
            getters,
            setters,
            methods,
            constructors: Vec::new(),
        }
    }

    /// Dart `_buildInterfaceData`.
    fn interface_data(
        &self,
        node: NodeId,
        name: Option<TokenId>,
        type_parameters: Option<Id<TypeParameterList>>,
        primary_constructor: Option<Id<PrimaryConstructorDeclaration>>,
        members: &[Id<ClassMember>],
        fields: Option<Vec<InfoNode>>,
    ) -> InfoInstance {
        let ast = self.ast;
        let mut data = self.instance_data(node, name, type_parameters, members, fields);
        let body = members
            .iter()
            .find_map(|&m| ast.cast::<PrimaryConstructorBody>(m.raw()));
        if let Some(p) = primary_constructor {
            data.constructors.push(self.primary_constructor(p, body));
        }
        for &m in members {
            if let Some(c) = ast.cast::<ConstructorDeclaration>(m.raw()) {
                data.constructors.push(self.constructor(c));
            }
        }
        data
    }

    /// Dart `_buildConstructor`.
    fn constructor(&self, node: Id<ConstructorDeclaration>) -> InfoConstructor {
        let ast = self.ast;
        let n = ast.get(node);
        let type_name_token = n.type_name.map(|t| ast.get(t).token);
        InfoConstructor {
            executable: InfoExecutable {
                node: self.node(node.raw(), n.name, n.documentation_comment),
                type_parameters: Vec::new(),
                parameters: self.formal_parameters(Some(n.parameters)),
            },
            new_keyword_offset: n.new_keyword.map(|t| ast.tokens.offset(t)),
            factory_keyword_offset: n.factory_keyword.map(|t| ast.tokens.offset(t)),
            type_name_offset: n.type_name.map(|t| ast.offset(t)),
            period_offset: n.period.map(|t| ast.tokens.offset(t)),
            name_end: n.name.or(type_name_token).map(|t| ast.tokens.get(t).end()),
            this_keyword_offset: None,
        }
    }

    /// Dart `_buildPrimaryConstructor`.
    fn primary_constructor(
        &self,
        node: Id<PrimaryConstructorDeclaration>,
        body: Option<Id<PrimaryConstructorBody>>,
    ) -> InfoConstructor {
        let ast = self.ast;
        let n = ast.get(node);
        let constructor_name = n.constructor_name.map(|c| ast.get(c));
        InfoConstructor {
            executable: InfoExecutable {
                node: self.node(
                    node.raw(),
                    constructor_name.map(|c| c.name),
                    body.and_then(|b| ast.get(b).documentation_comment),
                ),
                type_parameters: Vec::new(),
                parameters: self.formal_parameters(Some(n.formal_parameters)),
            },
            new_keyword_offset: None,
            factory_keyword_offset: None,
            type_name_offset: Some(ast.tokens.offset(n.type_name)),
            period_offset: constructor_name.map(|c| ast.tokens.offset(c.period)),
            name_end: Some(ast.tokens.get(constructor_name.map(|c| c.name).unwrap_or(n.type_name)).end()),
            this_keyword_offset: body.map(|b| ast.tokens.offset(ast.get(b).this_keyword)),
        }
    }
}

/// The name, documentation comment and function-typed suffix of a formal
/// parameter.
pub fn formal_parameter_parts(
    ast: &Ast,
    p: Id<FormalParameter>,
) -> (Option<TokenId>, Option<Id<Comment>>, Option<Id<FunctionTypedFormalParameterSuffix>>) {
    let p = p.raw();
    if let Some(r) = ast.cast::<RegularFormalParameter>(p) {
        let r = ast.get(r);
        (r.name, r.documentation_comment, r.function_typed_suffix)
    } else if let Some(f) = ast.cast::<FieldFormalParameter>(p) {
        let f = ast.get(f);
        (Some(f.name), f.documentation_comment, f.function_typed_suffix)
    } else {
        let s = ast.cast::<SuperFormalParameter>(p).expect("formal parameter");
        let s = ast.get(s);
        (Some(s.name), s.documentation_comment, s.function_typed_suffix)
    }
}

/// Dart `forCorrespondingPairs`.
fn pairs<A: Copy, B>(a: impl IntoIterator<Item = A>, b: &[B], mut f: impl FnMut(A, &B)) {
    for (x, y) in a.into_iter().zip(b.iter()) {
        f(x, y);
    }
}

/// Dart `InformativeDataApplier` (`applyFromBytes`, members not deferred).
pub struct InformativeDataApplier<'s> {
    pub store: &'s mut ElementStore,
}

impl InformativeDataApplier<'_> {
    fn set_node(&mut self, f: FragmentId, info: &InfoNode) {
        let s = &mut *self.store;
        let data = fragment_data_mut(s, f);
        data.code_offset = Some(info.code_offset);
        data.code_length = Some(info.code_length);
        data.first_token_offset = Some(info.first_token_offset);
        data.name_offset = info.name_offset;
        data.documentation_comment = info.documentation_comment.clone();
    }

    /// Dart `_applyFromInfo`.
    pub fn apply(&mut self, library: EId<LibraryElement>, unit: FId<LibraryFragment>, info: &InfoUnit) {
        let is_first = self.store.get(library).first_fragment().raw() == unit.raw();
        if is_first {
            let l = self.store.get_mut(library);
            l.name_offset = info.library_name_offset;
            l.name_length = info.library_name_length;
            l.documentation_comment = info.doc_comment.clone();
        }
        {
            let u = self.store.fragment_mut(unit);
            u.code_offset = Some(info.code_offset);
            u.code_length = Some(info.code_length);
            u.line_starts = info.line_starts.as_slice().into();
        }
        // Imports, exports, parts.
        let import_count = self.store.fragment(unit).library_imports.len();
        for (i, ii) in (0..import_count).zip(info.imports.iter()) {
            let prefix = {
                let u = self.store.fragment_mut(unit);
                let import = &mut u.library_imports[i];
                import.import_keyword_offset = ii.import_keyword_offset as i32;
                apply_combinators(&mut import.combinators, &ii.combinators);
                import.prefix
            };
            if let Some(prefix) = prefix {
                let p = self.store.fragment_mut(prefix);
                p.name_offset = ii.prefix_offset;
                p.offset = ii.prefix_offset.unwrap_or(ii.import_keyword_offset);
            }
        }
        {
            let u = self.store.fragment_mut(unit);
            for (export, ie) in u.library_exports.iter_mut().zip(info.exports.iter()) {
                export.export_keyword_offset = ie.export_keyword_offset as i32;
                apply_combinators(&mut export.combinators, &ie.combinators);
            }
            for (part, &offset) in u.parts.iter_mut().zip(info.parts.iter()) {
                part.part_keyword_offset = offset as i32;
            }
        }

        let u = self.store.fragment(unit);
        let getters: Vec<FId<GetterFragment>> = u.getters.clone();
        let setters: Vec<FId<SetterFragment>> = u.setters.clone();
        let classes: Vec<FId<ClassFragment>> = u.classes.clone();
        let enums: Vec<FId<EnumFragment>> = u.enums.clone();
        let extensions: Vec<FId<ExtensionFragment>> = u.extensions.clone();
        let extension_types: Vec<FId<ExtensionTypeFragment>> = u.extension_types.clone();
        let functions: Vec<FId<TopLevelFunctionFragment>> = u.functions.clone();
        let mixins: Vec<FId<MixinFragment>> = u.mixins.clone();
        let variables: Vec<FId<TopLevelVariableFragment>> = u.variables.clone();
        let type_aliases: Vec<FId<TypeAliasFragment>> = u.type_aliases.clone();

        self.apply_accessors(&getters, &setters, &info.top_level_getters, &info.top_level_setters);
        let has = |s: &ElementStore, f: FragmentId, flag: FragmentFlags| {
            s.fragment_data(f).is_some_and(|d| d.flags.has(flag))
        };
        let (mixin_apps, plain): (Vec<FId<ClassFragment>>, Vec<FId<ClassFragment>>) = classes
            .iter()
            .copied()
            .partition(|c| has(self.store, c.raw(), FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_APPLICATION));
        pairs(plain, &info.class_declarations, |f, i| {
            self.apply_instance(f.raw(), i, true)
        });
        pairs(mixin_apps, &info.class_type_aliases, |f, i| {
            self.set_node(f.raw(), &i.node);
            let tps = self.store.fragment(f).type_params.clone();
            self.apply_type_parameters(&tps, &i.type_parameters);
        });
        pairs(enums, &info.enums, |f, i| self.apply_instance(f.raw(), i, true));
        pairs(extensions, &info.extensions, |f, i| {
            self.apply_instance(f.raw(), i, false)
        });
        pairs(extension_types, &info.extension_types, |f, i| {
            self.apply_instance(f.raw(), i, true)
        });
        pairs(functions, &info.top_level_functions, |f, i| {
            self.set_node(f.raw(), &i.node);
            let e = self.store.fragment(f);
            let tps = e.type_params.clone();
            let ps = e.formal_params.clone();
            self.apply_type_parameters(&tps, &i.type_parameters);
            self.apply_formal_parameters(&ps, &i.parameters);
        });
        pairs(mixins, &info.mixin_declarations, |f, i| {
            self.apply_instance(f.raw(), i, true)
        });
        let origin_variables: Vec<_> = variables
            .into_iter()
            .filter(|v| {
                has(
                    self.store,
                    v.raw(),
                    FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION,
                )
            })
            .collect();
        pairs(origin_variables, &info.top_level_variable, |f, i| {
            self.set_node(f.raw(), i)
        });
        pairs(type_aliases, &info.type_aliases, |f, i| {
            self.set_node(f.raw(), &i.node);
            let tps = self.store.fragment(f).type_params.clone();
            self.apply_type_parameters(&tps, &i.type_parameters);
        });
    }

    /// Dart `_applyToAccessors` for getters and setters.
    fn apply_accessors(
        &mut self,
        getters: &[FId<GetterFragment>],
        setters: &[FId<SetterFragment>],
        getter_info: &[InfoExecutable],
        setter_info: &[InfoExecutable],
    ) {
        let origin = FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION;
        let gs: Vec<FragmentId> = getters
            .iter()
            .map(|g| g.raw())
            .filter(|&g| self.store.fragment_data(g).unwrap().flags.has(origin))
            .collect();
        let ss: Vec<FragmentId> = setters
            .iter()
            .map(|s| s.raw())
            .filter(|&s| self.store.fragment_data(s).unwrap().flags.has(origin))
            .collect();
        pairs(gs, getter_info, |f, i| self.apply_executable(f, i));
        pairs(ss, setter_info, |f, i| self.apply_executable(f, i));
    }

    fn apply_executable(&mut self, f: FragmentId, info: &InfoExecutable) {
        self.set_node(f, &info.node);
        let e = self.store.executable_fragment(FId::from_raw(f));
        let tps = e.type_params.clone();
        let ps = e.formal_params.clone();
        self.apply_type_parameters(&tps, &info.type_parameters);
        self.apply_formal_parameters(&ps, &info.parameters);
    }

    /// Dart `_applyToClassDeclaration` and the other instance declarations.
    fn apply_instance(&mut self, f: FragmentId, info: &InfoInstance, has_constructors: bool) {
        self.set_node(f, &info.node);
        let instance = self.store.instance_fragment(FId::from_raw(f));
        let tps = instance.type_params.clone();
        let fields = instance.fields.clone();
        let getters = instance.getters.clone();
        let setters = instance.setters.clone();
        let methods = instance.methods.clone();
        self.apply_type_parameters(&tps, &info.type_parameters);
        if has_constructors {
            let constructors = interface_constructors(self.store, f);
            let origin = FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_DECLARATION;
            let cs: Vec<FId<ConstructorFragment>> = constructors
                .into_iter()
                .filter(|c| self.store.fragment(*c).flags.has(origin))
                .collect();
            pairs(cs, &info.constructors, |c, i| {
                self.set_node(c.raw(), &i.executable.node);
                let ps = {
                    let cf = self.store.fragment_mut(c);
                    cf.new_keyword_offset = i.new_keyword_offset;
                    cf.factory_keyword_offset = i.factory_keyword_offset;
                    cf.type_name_offset = i.type_name_offset;
                    cf.period_offset = i.period_offset;
                    cf.name_end = i.name_end;
                    cf.this_keyword_offset = i.this_keyword_offset;
                    cf.formal_params.clone()
                };
                self.apply_formal_parameters(&ps, &i.executable.parameters);
            });
        }
        let origin = FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION;
        let fs: Vec<FId<FieldFragment>> = fields
            .into_iter()
            .filter(|x| self.store.fragment(*x).flags.has(origin))
            .collect();
        pairs(fs, &info.fields, |x, i| self.set_node(x.raw(), i));
        self.apply_accessors(&getters, &setters, &info.getters, &info.setters);
        pairs(methods, &info.methods, |m, i| self.apply_executable(m.raw(), i));
    }

    fn apply_type_parameters(&mut self, list: &[FId<TypeParameterFragment>], info: &[InfoTypeParameter]) {
        pairs(list.iter().copied(), info, |f, i| {
            let d = self.store.fragment_mut(f);
            d.code_offset = Some(i.node.code_offset);
            d.code_length = Some(i.node.code_length);
            d.first_token_offset = Some(i.node.first_token_offset);
            d.name_offset = i.node.name_offset;
        });
    }

    /// Dart `_applyToFormalParameters`.
    fn apply_formal_parameters(&mut self, list: &[FId<FormalParameterFragment>], info: &[InfoFormalParameter]) {
        let origin = FragmentFlags::FORMAL_PARAMETER_FRAGMENT_IS_ORIGIN_DECLARATION;
        let ps: Vec<FId<FormalParameterFragment>> = list
            .iter()
            .copied()
            .filter(|p| self.store.fragment(*p).flags.has(origin))
            .collect();
        pairs(ps, info, |f, i| self.set_node(f.raw(), &i.node));
    }
}

fn apply_combinators(list: &mut [NamespaceCombinator], info: &[InfoCombinator]) {
    for (c, i) in list.iter_mut().zip(info.iter()) {
        match c {
            NamespaceCombinator::Show { offset, end, .. }
            | NamespaceCombinator::Hide { offset, end, .. } => {
                *offset = i.offset;
                *end = i.end as i32;
            }
        }
    }
}

/// The constructors of an interface fragment.
pub fn interface_constructors(store: &ElementStore, f: FragmentId) -> Vec<FId<ConstructorFragment>> {
    use dartr_element::Tag;
    let i = f.index();
    let fr = &store.fragments;
    match f.tag() {
        Tag::Class => fr.classes.get(i).constructors.clone(),
        Tag::Enum => fr.enums.get(i).constructors.clone(),
        Tag::Mixin => fr.mixins.get(i).constructors.clone(),
        Tag::ExtensionType => fr.extension_types.get(i).constructors.clone(),
        _ => Vec::new(),
    }
}

/// `FragmentImpl` data of any fragment, mutable.
pub fn fragment_data_mut(store: &mut ElementStore, id: FragmentId) -> &mut dartr_element::FragmentData {
    use dartr_element::Tag;
    let i = id.index();
    let f = &mut store.fragments;
    match id.tag() {
        Tag::Class => co(f.classes.get_mut(i)),
        Tag::Enum => co(f.enums.get_mut(i)),
        Tag::Mixin => co(f.mixins.get_mut(i)),
        Tag::Extension => co(f.extensions.get_mut(i)),
        Tag::ExtensionType => co(f.extension_types.get_mut(i)),
        Tag::Field => co(f.fields.get_mut(i)),
        Tag::Getter => co(f.getters.get_mut(i)),
        Tag::Setter => co(f.setters.get_mut(i)),
        Tag::Method => co(f.methods.get_mut(i)),
        Tag::Constructor => co(f.constructors.get_mut(i)),
        Tag::TopLevelFunction => co(f.functions.get_mut(i)),
        Tag::TopLevelVariable => co(f.variables.get_mut(i)),
        Tag::TypeAlias => co(f.type_aliases.get_mut(i)),
        Tag::TypeParameter => co(f.type_params.get_mut(i)),
        Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => {
            co(f.params.get_mut(i))
        }
        Tag::Prefix => co(f.prefixes.get_mut(i)),
        Tag::Library => co(f.units.get_mut(i)),
        Tag::GenericFunctionType => co(f.generic_function_types.get_mut(i)),
        Tag::LocalVariable | Tag::PatternVariable | Tag::BindPatternVariable | Tag::JoinPatternVariable => {
            co(f.locals.get_mut(i))
        }
        Tag::LocalFunction => co(f.local_functions.get_mut(i)),
        Tag::Label => co(f.labels.get_mut(i)),
        Tag::MultiplyDefined => co(f.multiply_defined.get_mut(i)),
        Tag::Dynamic | Tag::Never => unreachable!("{id:?}"),
    }
}

/// Deref coercion of a fragment struct to its [`dartr_element::FragmentData`].
fn co<T: std::ops::DerefMut>(x: &mut T) -> &mut dartr_element::FragmentData
where
    T::Target: CoerceFragmentData,
{
    (**x).coerce()
}

/// Steps down the `DerefMut` chain of fragment structs.
pub trait CoerceFragmentData {
    fn coerce(&mut self) -> &mut dartr_element::FragmentData;
}

impl CoerceFragmentData for dartr_element::FragmentData {
    fn coerce(&mut self) -> &mut dartr_element::FragmentData {
        self
    }
}

macro_rules! coerce_via {
    ($($t:ty),*) => {$(
        impl CoerceFragmentData for $t {
            fn coerce(&mut self) -> &mut dartr_element::FragmentData {
                (**self).coerce()
            }
        }
    )*};
}
coerce_via!(
    dartr_element::InstanceFragmentData,
    dartr_element::InterfaceFragmentData,
    dartr_element::ExecutableFragmentData,
    dartr_element::PropertyAccessorFragmentData,
    dartr_element::VariableFragmentData,
    dartr_element::PropertyInducingFragmentData
);
