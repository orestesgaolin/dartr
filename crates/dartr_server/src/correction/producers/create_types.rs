// Dart source: pkg/analysis_server/lib/src/services/correction/dart/create_class.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/create_mixin.dart
// Dart source: pkg/analysis_server/lib/src/utilities/extensions/string.dart (firstLetterIsLowercase)
// Dart source: pkg/analyzer_plugin/lib/src/utilities/change_builder/change_builder_dart.dart (writeClassDeclaration, writeConstructorDeclaration, writeMixinDeclaration)

//! Dart `CreateClass` and `CreateMixin` (multi producers): a class or
//! mixin for an unresolved type name.

use dartr_ast::*;
use dartr_element::{Tag, TypeKind};

use super::super::change::utf16_len;
use super::super::change_builder::{ChangeBuilder, ChangeWorkspace};
use super::super::fix_kind::FixKind;
use super::super::generated::fix_kinds as k;
use super::super::imports::existing_imports;
use super::super::producer::*;
use super::create::{ArgumentInfo, Inferred, argument_infos, infer_undefined_expression_type};

/// Dart `String.firstLetterIsLowercase` (`^([_$]|[_$]+[0-9])*[a-z]`).
pub fn first_letter_is_lowercase(name: &str) -> bool {
    let bytes = name.as_bytes();
    let mut i = 0;
    loop {
        let start = i;
        while i < bytes.len() && (bytes[i] == b'_' || bytes[i] == b'$') {
            i += 1;
        }
        if i > start {
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
        }
        if i == start {
            break;
        }
    }
    i < bytes.len() && bytes[i].is_ascii_lowercase()
}

/// Dart `nameOfType` of `create_class.dart`.
fn name_of_type(c: &ProducerContext<'_>, id: Id<SimpleIdentifier>) -> Option<String> {
    let name = c.lexeme(c.ast[id].token).to_string();
    let parent_is_named_type = c.ast.parent(id).is_some_and(|p| c.ast.is::<NamedType>(p));
    let is_name_of_type = name
        .chars()
        .next()
        .is_some_and(|f| f.to_uppercase().to_string() == f.to_string());
    (parent_is_named_type || is_name_of_type).then_some(name)
}

/// Dart `inConstantContext` (approximation: an enclosing const creation,
/// const literal, const variable or annotation).
fn in_constant_context(c: &ProducerContext<'_>, node: NodeId) -> bool {
    let ast = c.ast;
    let mut n = ast.parent(node);
    while let Some(x) = n {
        if ast.is::<FunctionBody>(x)
            || ast.is::<Statement>(x) && !ast.is::<VariableDeclarationStatement>(x)
        {
            return false;
        }
        if let Some(i) = ast.cast::<InstanceCreationExpression>(x) {
            if ast[i].keyword.is_some_and(|k| c.lexeme(k) == "const") {
                return true;
            }
        }
        if let Some(l) = ast.cast::<ListLiteral>(x) {
            if ast[l].const_keyword.is_some() {
                return true;
            }
        }
        if let Some(l) = ast.cast::<SetOrMapLiteral>(x) {
            if ast[l].const_keyword.is_some() {
                return true;
            }
        }
        if let Some(l) = ast.cast::<VariableDeclarationList>(x) {
            return ast[l].keyword.is_some_and(|k| c.lexeme(k) == "const");
        }
        if ast.is::<Annotation>(x) {
            return true;
        }
        n = ast.parent(x);
    }
    false
}

/// Dart `_requiresConstConstructor`.
fn requires_const_constructor(c: &ProducerContext<'_>, node: NodeId) -> bool {
    let ast = c.ast;
    let parent = ast.parent(node);
    if ast.is::<SimpleIdentifier>(node) {
        if let Some(p) = parent.filter(|p| ast.is::<NamedType>(*p)) {
            return requires_const_constructor(c, p);
        }
        if parent.is_some_and(|p| ast.is::<MethodInvocation>(p)) {
            return in_constant_context(c, parent.unwrap());
        }
    }
    if ast.is::<NamedType>(node) {
        if let Some(p) = parent.filter(|p| ast.is::<ConstructorName>(*p)) {
            return requires_const_constructor(c, p);
        }
    }
    if ast.is::<ConstructorName>(node) {
        if let Some(i) = parent.and_then(|p| ast.cast::<InstanceCreationExpression>(p)) {
            return ast[i].keyword.is_some_and(|k| c.lexeme(k) == "const")
                || in_constant_context(c, i.raw());
        }
    }
    false
}

/// What to create.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TypeKindToCreate {
    Class,
    Mixin,
}

/// Dart `_CreateClass` and `_CreateMixin`.
pub struct CreateType {
    what: TypeKindToCreate,
    kind: &'static FixKind,
    name: String,
    target: NodeId,
    /// The import prefix of the name, when the type is created in an
    /// imported library.
    prefix: Option<String>,
    arguments: Option<Id<ArgumentList>>,
    requires_const_constructor: bool,
    expression: Option<NodeId>,
}

/// Dart `CreateClass.producers`.
pub fn create_class_producers(
    c: &ProducerContext<'_>,
    _: &mut dyn ChangeWorkspace,
) -> Vec<Box<dyn CorrectionProducer>> {
    let ast = c.ast;
    let mut target = c.node;
    let mut prefix: Option<String> = None;
    let mut arguments = None;
    let mut with_keyword = false;
    let mut requires_const = false;
    if let Some(a) = ast.cast::<Annotation>(target) {
        arguments = ast[a].arguments;
        if c.element_of(ast[a].name.raw()).is_some() || arguments.is_none() {
            return Vec::new();
        }
        target = ast[a].name.raw();
        requires_const = true;
    }
    let class_name;
    if let Some(t) = ast.cast::<NamedType>(target) {
        if let Some(p) = ast[t].import_prefix {
            if c.element_of(p.raw()).is_none() {
                return Vec::new();
            }
            prefix = Some(c.lexeme(ast[p].name).to_string());
        }
        with_keyword = ast.parent(c.node).is_some_and(|p| ast.is::<WithClause>(p));
        class_name = c.lexeme(ast[t].name).to_string();
        requires_const |= requires_const_constructor(c, target);
    } else if let Some(id) = ast.cast::<SimpleIdentifier>(target) {
        let parent = ast.parent(id);
        let in_property =
            parent.is_some_and(|p| ast.is::<PropertyAccess>(p) || ast.is::<PrefixedIdentifier>(p));
        if !in_property {
            if let Some(m) = parent.and_then(|p| ast.cast::<MethodInvocation>(p)) {
                if let Some(t) = ast[m].target {
                    let prefix_element = ast
                        .cast::<SimpleIdentifier>(t)
                        .and_then(|i| c.element_of(i.raw()))
                        .filter(|e| e.tag() == Tag::Prefix);
                    if let Some(_) = prefix_element {
                        prefix = Some(c.utils.get_node_text(t));
                    } else if c.tables.static_type.get(t.raw()).is_some() {
                        return Vec::new();
                    }
                }
            }
            class_name = name_of_type(c, id).unwrap_or_else(|| c.lexeme(ast[id].token).to_string());
            requires_const |= requires_const_constructor(c, target);
        } else if let Some(p) = parent
            .and_then(|p| ast.cast::<PrefixedIdentifier>(p))
            .filter(|p| ast[*p].identifier != id)
        {
            let _ = p;
            class_name = name_of_type(c, id).unwrap_or_else(|| c.lexeme(ast[id].token).to_string());
            requires_const |= requires_const_constructor(c, target);
        } else {
            return Vec::new();
        }
    } else if let Some(p) = ast.cast::<PrefixedIdentifier>(target) {
        if c.element_of(ast[p].prefix.raw()).is_none() {
            return Vec::new();
        }
        prefix = Some(c.lexeme(ast[ast[p].prefix].token).to_string());
        let id = ast[p].identifier;
        class_name = name_of_type(c, id).unwrap_or_else(|| c.lexeme(ast[id].token).to_string());
    } else {
        return Vec::new();
    }
    if class_name.is_empty() {
        return Vec::new();
    }
    let lowercase = first_letter_is_lowercase(&class_name);
    let kind = match (lowercase, with_keyword) {
        (true, true) => &k::CREATE_CLASS_LOWERCASE_WITH,
        (true, false) => &k::CREATE_CLASS_LOWERCASE,
        (false, true) => &k::CREATE_CLASS_UPPERCASE_WITH,
        (false, false) => &k::CREATE_CLASS_UPPERCASE,
    };
    // Dart: the expression is the target or its parent.
    let expression = if ast.is::<Expression>(target) {
        Some(target)
    } else {
        ast.parent(target).filter(|p| ast.is::<Expression>(*p))
    };
    vec![Box::new(CreateType {
        what: TypeKindToCreate::Class,
        kind,
        name: class_name,
        target,
        prefix,
        arguments,
        requires_const_constructor: requires_const,
        expression,
    })]
}

/// Dart `CreateMixin.producers`.
pub fn create_mixin_producers(
    c: &ProducerContext<'_>,
    _: &mut dyn ChangeWorkspace,
) -> Vec<Box<dyn CorrectionProducer>> {
    let ast = c.ast;
    let node = c.node;
    let mut prefix = None;
    let mut with_keyword = false;
    let mut expression = None;
    let name;
    if let Some(t) = ast.cast::<NamedType>(node) {
        if let Some(p) = ast[t].import_prefix {
            if c.element_of(p.raw()).is_none() {
                return Vec::new();
            }
            prefix = Some(c.lexeme(ast[p].name).to_string());
        }
        with_keyword = ast.parent(node).is_some_and(|p| ast.is::<WithClause>(p));
        name = c.lexeme(ast[t].name).to_string();
    } else if let Some(id) = ast.cast::<SimpleIdentifier>(node) {
        if let Some(parent) = ast.parent(id) {
            let invalid = if let Some(p) = ast.cast::<PrefixedIdentifier>(parent) {
                Some(ast[p].identifier.raw())
            } else if let Some(p) = ast.cast::<PropertyAccess>(parent) {
                Some(ast[p].property_name.raw())
            } else {
                ast.cast::<ExpressionFunctionBody>(parent)
                    .map(|b| ast[b].expression.raw())
            };
            if invalid == Some(node) {
                return Vec::new();
            }
        }
        expression = Some(node);
        name = c.lexeme(ast[id].token).to_string();
    } else if let Some(p) = ast.cast::<PrefixedIdentifier>(node) {
        if ast
            .parent(node)
            .is_some_and(|x| ast.is::<InstanceCreationExpression>(x))
        {
            return Vec::new();
        }
        if c.element_of(ast[p].prefix.raw()).is_none() {
            return Vec::new();
        }
        prefix = Some(c.lexeme(ast[ast[p].prefix].token).to_string());
        expression = Some(node);
        name = c.lexeme(ast[ast[p].identifier].token).to_string();
    } else {
        return Vec::new();
    }
    if name.is_empty() {
        return Vec::new();
    }
    let lowercase = first_letter_is_lowercase(&name);
    let kind = match (lowercase, with_keyword) {
        (true, true) => &k::CREATE_MIXIN_LOWERCASE_WITH,
        (true, false) => &k::CREATE_MIXIN_LOWERCASE,
        (false, true) => &k::CREATE_MIXIN_UPPERCASE_WITH,
        (false, false) => &k::CREATE_MIXIN_UPPERCASE,
    };
    vec![Box::new(CreateType {
        what: TypeKindToCreate::Mixin,
        kind,
        name,
        target: node,
        prefix,
        arguments: None,
        requires_const_constructor: false,
        expression,
    })]
}

impl CorrectionProducer for CreateType {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(self.kind)
    }

    fn fix_arguments(&self) -> Vec<String> {
        vec![self.name.clone()]
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let ctx = c.ctx;
        if let Some(expression) = self.expression {
            match infer_undefined_expression_type(c, expression) {
                Inferred::Invalid => return,
                Inferred::Type(t) => {
                    if matches!(ctx.ty(t), TypeKind::Invalid) {
                        return;
                    }
                    let ts = dartr_typesystem::type_system::TypeSystem::new(*ctx);
                    if !ts.is_assignable_to(t, ctx.tp.type_type(), c.options.strict_casts)
                        || !ts.is_subtype_of(t, ctx.tp.object_type())
                    {
                        return;
                    }
                }
                Inferred::Unknown => {}
            }
        }
        let (file_path, offset) = match &self.prefix {
            None => {
                let member = ast.this_or_ancestor_matching(self.target, |a, n| {
                    a.is::<CompilationUnitMember>(n)
                        && a.parent(n).is_some_and(|p| a.is::<CompilationUnit>(p))
                });
                let Some(member) = member else { return };
                (c.path.to_string(), ast.end(member))
            }
            Some(prefix) => {
                let library = c.resolved.library.library.library;
                let import = existing_imports(ctx, library)
                    .into_iter()
                    .find(|i| i.prefix.as_deref() == Some(prefix.as_str()));
                let Some(imported) = import.and_then(|i| i.library) else {
                    return;
                };
                let path = super::super::imports::library_path(ctx, imported);
                let Some(content) = builder.workspace.content(&path) else {
                    return;
                };
                (path, utf16_len(&content))
            }
        };
        let same_file = file_path == c.path;
        let infos: Vec<ArgumentInfo> = self
            .arguments
            .map(|a| argument_infos(c, a))
            .unwrap_or_default();
        let name = self.name.clone();
        let what = self.what;
        let with_constructor = self.arguments.is_some() || self.requires_const_constructor;
        let is_const = self.requires_const_constructor;
        let primary = ctx.features.is_enabled("primary-constructors");
        let (target_offset, target_length) = (ast.offset(self.target), ast.length(self.target));
        let has_prefix = self.prefix.is_some();
        builder.add_dart_file_edit(&file_path, |b| {
            let eol = b.eol();
            let prefix_text = if same_file {
                format!("{eol}{eol}")
            } else {
                eol.clone()
            };
            let suffix = if same_file {
                String::new()
            } else {
                eol.clone()
            };
            b.add_insertion(offset, |e| {
                e.write(&prefix_text);
                match what {
                    TypeKindToCreate::Mixin => {
                        // Dart `writeMixinDeclaration`.
                        e.write("mixin ");
                        e.add_simple_linked_edit("NAME", &name, None);
                        e.writeln(" {");
                        e.write("}");
                    }
                    TypeKindToCreate::Class => {
                        // Dart `writeClassDeclaration`.
                        e.write("class ");
                        e.add_simple_linked_edit("NAME", &name, None);
                        e.writeln(" {");
                        if with_constructor {
                            e.write("  ");
                            // Dart `writeConstructorDeclaration`.
                            if is_const {
                                e.write("const ");
                            }
                            if primary {
                                e.write("new");
                            } else {
                                e.add_simple_linked_edit("NAME", &name, None);
                            }
                            e.write("(");
                            e.write_parameters_matching_arguments(ctx, &infos);
                            e.write(")");
                            e.write(";");
                            e.newline();
                        }
                        e.write("}");
                    }
                }
                e.write(&suffix);
            });
            if !has_prefix {
                b.add_linked_position(target_offset, target_length, "NAME");
            }
        });
    }
}
