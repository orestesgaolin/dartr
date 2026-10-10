// Dart source: pkg/analysis_server/lib/src/services/correction/dart/create_getter.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/create_setter.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/create_field.dart
// Dart source: pkg/analysis_server_plugin/lib/edit/dart/correction_producer.dart (getDeclarationNodeFromElement, inStaticContext)
// Dart source: pkg/analysis_server/lib/src/services/correction/util.dart (climbPropertyAccess)
// Dart source: pkg/analyzer_plugin/lib/src/utilities/change_builder/change_builder_dart.dart (writeGetterDeclaration, writeSetterDeclaration, writeFieldDeclaration)

//! The producers that create getters, setters and fields (for unresolved
//! identifiers; not for object patterns and dot shorthands).

use dartr_ast::*;
use dartr_element::{ElementId, Tag, TypeId, TypeKind};

use super::super::change_builder::ChangeBuilder;
use super::super::code_style::CodeStyleOptions;
use super::super::dart_edit::WriteType;
use super::super::fix_kind::FixKind;
use super::super::generated::fix_kinds as k;
use super::super::producer::*;
use super::create::{Inferred, infer_undefined_expression_type, static_type};

/// Dart `climbPropertyAccess`.
pub fn climb_property_access(ast: &Ast, node: NodeId) -> NodeId {
    let mut node = node;
    loop {
        let Some(parent) = ast.parent(node) else {
            return node;
        };
        if let Some(p) = ast.cast::<PrefixedIdentifier>(parent) {
            if ast[p].identifier.raw() == node {
                node = parent;
                continue;
            }
        }
        if let Some(p) = ast.cast::<PropertyAccess>(parent) {
            if ast[p].property_name.raw() == node {
                node = parent;
                continue;
            }
        }
        return node;
    }
}

/// Dart `inStaticContext`.
pub fn in_static_context(c: &ProducerContext<'_>) -> bool {
    let ast = c.ast;
    if ast
        .this_or_ancestor_of_type::<ConstructorInitializer>(c.node)
        .is_some()
    {
        return true;
    }
    if let Some(f) = ast.this_or_ancestor_of_type::<FieldDeclaration>(c.node) {
        return ast[f].static_keyword.is_some() || ast[ast[f].fields].late_keyword.is_none();
    }
    ast.this_or_ancestor_of_type::<MethodDeclaration>(c.node)
        .is_some_and(|m| {
            ast[m]
                .modifier_keyword
                .is_some_and(|t| c.lexeme(t) == "static")
        })
}

/// Dart `SimpleIdentifier.inSetterContext` / `inGetterContext` (for
/// identifiers of property accesses): whether the identifier is the left
/// side of an assignment.
fn assignment_of(ast: &Ast, node: NodeId) -> Option<Id<AssignmentExpression>> {
    let climbed = climb_property_access(ast, node);
    let a = ast
        .parent(climbed)
        .and_then(|p| ast.cast::<AssignmentExpression>(p))?;
    (ast[a].left_hand_side.raw() == climbed).then_some(a)
}

/// The declaration of the class-like [element] (Dart
/// `getDeclarationNodeFromElement`): the path of its unit and the node in
/// the resolved unit of that path. `None` for SDK elements.
pub fn declaration_of(
    c: &ProducerContext<'_>,
    builder: &mut ChangeBuilder<'_>,
    element: ElementId,
) -> Option<(String, NodeId)> {
    let ctx = c.ctx;
    let library = dartr_resolver::error::support::library_of(ctx, element)?;
    if super::super::imports::library_uri(ctx, library).starts_with("dart:") {
        return None;
    }
    let data = ctx.element_data(element)?;
    let fragment = data.first_fragment;
    let name_offset = ctx.fragment_data(fragment)?.name_offset?;
    // The library fragment of the fragment.
    let mut f = fragment;
    let path = loop {
        if let Some(lf) = f.cast::<dartr_element::LibraryFragment>() {
            break ctx.fragment(lf).source.path.to_string();
        }
        f = ctx.fragment_data(f)?.enclosing_fragment?;
    };
    let resolved = builder.workspace.resolved_unit(&path)?;
    let unit = resolved.unit();
    let ast = &unit.ast;
    for &d in ast.list_raw(ast[unit.unit].declarations) {
        let name = if let Some(x) = ast.cast::<ClassDeclaration>(d) {
            Some(dartr_resolver::error::support::class_name_token(
                ast,
                ast[x].name_part,
            ))
        } else if let Some(x) = ast.cast::<EnumDeclaration>(d) {
            Some(dartr_resolver::error::support::class_name_token(
                ast,
                ast[x].name_part,
            ))
        } else if let Some(x) = ast.cast::<ExtensionTypeDeclaration>(d) {
            Some(dartr_resolver::error::support::class_name_token(
                ast,
                ast[x].name_part,
            ))
        } else if let Some(x) = ast.cast::<MixinDeclaration>(d) {
            Some(ast[x].name)
        } else {
            ast.cast::<ExtensionDeclaration>(d)
                .and_then(|x| ast[x].name)
        };
        if name.is_some_and(|t| ast.tokens.get(t).offset == name_offset) {
            return Some((path, d));
        }
    }
    None
}

/// The target of a getter, setter or field (Dart, shared by the
/// producers): the element, whether static, and the target type.
struct Target {
    element: ElementId,
    is_static: bool,
}

/// The enclosing class-like element of [node].
pub fn enclosing_instance_element(c: &ProducerContext<'_>, node: NodeId) -> Option<ElementId> {
    let ast = c.ast;
    let mut n = ast.parent(node);
    while let Some(x) = n {
        if ast.is::<ClassDeclaration>(x)
            || ast.is::<MixinDeclaration>(x)
            || ast.is::<EnumDeclaration>(x)
            || ast.is::<ExtensionDeclaration>(x)
            || ast.is::<ExtensionTypeDeclaration>(x)
        {
            return c.locator().declared_element(x);
        }
        n = ast.parent(x);
    }
    None
}

/// The extended interface element of an extension.
fn extended_interface_element(c: &ProducerContext<'_>, extension: ElementId) -> Option<ElementId> {
    let ext = extension.cast::<dartr_element::ExtensionElement>()?;
    let ty = c.ctx.get(ext).extended_type.get()?;
    match *c.ctx.ty(ty) {
        TypeKind::Interface { element, .. } => Some(element.raw()),
        _ => None,
    }
}

/// The target of the identifier [node] (`a.name`, `A.name` or `name`).
fn target_of(c: &ProducerContext<'_>, node: NodeId, for_setter: bool) -> Option<Target> {
    let ast = c.ast;
    let parent = ast.parent(node);
    let target = if let Some(p) = parent.and_then(|p| ast.cast::<PrefixedIdentifier>(p)) {
        Some(ast[p].prefix.raw())
    } else if let Some(p) = parent.and_then(|p| ast.cast::<PropertyAccess>(p)) {
        ast[p].target.map(|t| t.raw())
    } else {
        None
    };
    match target {
        Some(t) if ast.is::<ExtensionOverride>(t) => None,
        Some(t) => {
            let element = if ast.is::<Identifier>(t) {
                c.element_of(t)
            } else {
                None
            };
            let instance = element.filter(|e| {
                matches!(
                    e.tag(),
                    Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType
                ) || (!for_setter && e.tag() == Tag::Extension)
            });
            if let Some(e) = instance {
                return Some(Target {
                    element: e,
                    is_static: true,
                });
            }
            let ty = static_type(c, t)?;
            let TypeKind::Interface { element, .. } = *c.ctx.ty(ty) else {
                return None;
            };
            // Dart: static when the identifier names a class (handled
            // above).
            Some(Target {
                element: element.raw(),
                is_static: false,
            })
        }
        None => {
            let is_static = in_static_context(c);
            let mut element = enclosing_instance_element(c, node)?;
            if element.tag() == Tag::Extension {
                if is_static {
                    return None;
                }
                element = extended_interface_element(c, element)?;
            }
            Some(Target { element, is_static })
        }
    }
}

/// Which member to create.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MemberKind {
    Getter,
    Setter,
    Field,
}

/// Dart `CreateGetter`, `CreateSetter` and `CreateField` (from an
/// identifier).
pub struct CreateMember {
    pub kind: MemberKind,
    name: String,
}

impl CreateMember {
    pub fn new(kind: MemberKind) -> Self {
        CreateMember {
            kind,
            name: String::new(),
        }
    }
}

impl CorrectionProducer for CreateMember {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(match self.kind {
            MemberKind::Getter => &k::CREATE_GETTER,
            MemberKind::Setter => &k::CREATE_SETTER,
            MemberKind::Field => &k::CREATE_FIELD,
        })
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
        if self.kind == MemberKind::Field {
            if let Some(p) = ast.this_or_ancestor_of_type::<FieldFormalParameter>(c.node) {
                self.from_field_formal_parameter(c, builder, p);
                return;
            }
        }
        let Some(id) = ast.cast::<SimpleIdentifier>(c.node) else {
            return;
        };
        self.name = c.lexeme(ast[id].token).to_string();
        let assignment = assignment_of(ast, c.node);
        match self.kind {
            // Dart `inGetterContext`: not the target of `=`.
            MemberKind::Getter => {
                if assignment.is_some_and(|a| c.lexeme(ast[a].operator) == "=") {
                    return;
                }
            }
            MemberKind::Setter => {
                if assignment.is_none() {
                    return;
                }
            }
            MemberKind::Field => {}
        }
        let Some(target) = target_of(c, c.node, self.kind == MemberKind::Setter) else {
            return;
        };
        let mut element = target.element;
        if self.kind == MemberKind::Setter && element.tag() == Tag::Extension {
            let Some(e) = extended_interface_element(c, element) else {
                return;
            };
            element = e;
        }
        let type_node = climb_property_access(ast, c.node);
        if self.kind == MemberKind::Field
            && element.tag() == Tag::Enum
            && assignment.is_some_and(|a| ast[a].left_hand_side.raw() == type_node)
            && !target.is_static
        {
            return;
        }
        let ty = match infer_undefined_expression_type(c, type_node) {
            Inferred::Invalid => return,
            Inferred::Type(t) => Some(t),
            Inferred::Unknown => None,
        };
        let Some((path, declaration)) = declaration_of(c, builder, element) else {
            return;
        };
        let is_static = target.is_static;
        let name = self.name.clone();
        let kind = self.kind;
        let options = c.options.clone();
        let style = CodeStyleOptions { options: &options };
        let dynamic_return =
            style.specify_return_types() && !style.is_lint_enabled("avoid_annotating_with_dynamic");
        let dynamic_non_return =
            style.specify_types() && !style.is_lint_enabled("avoid_annotating_with_dynamic");
        builder.add_dart_file_edit(&path, |b| {
            let declaration_ast_enum = {
                let r = b.resolved();
                r.unit().ast.is::<EnumDeclaration>(declaration)
            };
            match kind {
                MemberKind::Getter => b.insert_getter(declaration, |e| {
                    write_getter(
                        ctx,
                        e,
                        &name,
                        is_static,
                        ty.unwrap_or_else(|| ctx.tp.dynamic_type()),
                        dynamic_return,
                    )
                }),
                MemberKind::Setter => b.insert_getter(declaration, |e| {
                    write_setter(ctx, e, &name, is_static, ty, dynamic_non_return)
                }),
                MemberKind::Field => {
                    let kinds_ok = {
                        let r = b.resolved();
                        let a = &r.unit().ast;
                        a.is::<ClassDeclaration>(declaration)
                            || a.is::<EnumDeclaration>(declaration)
                            || a.is::<MixinDeclaration>(declaration)
                    };
                    if kinds_ok {
                        b.insert_field(declaration, |e| {
                            write_field(
                                ctx,
                                e,
                                &name,
                                declaration_ast_enum && !is_static,
                                is_static,
                                ty,
                                dynamic_non_return,
                            )
                        });
                    }
                }
            }
        });
    }
}

impl CreateMember {
    /// Dart `CreateField._proposeFromFieldFormalParameter`.
    fn from_field_formal_parameter(
        &mut self,
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        p: Id<FieldFormalParameter>,
    ) {
        let ast = c.ast;
        let ctx = c.ctx;
        let Some(constructor) = ast.this_or_ancestor_of_type::<ConstructorDeclaration>(p) else {
            return;
        };
        let Some(container) = ast.this_or_ancestor_of_type::<CompilationUnitMember>(constructor)
        else {
            return;
        };
        if !ast.is::<ClassDeclaration>(container) && !ast.is::<EnumDeclaration>(container) {
            return;
        }
        self.name = c.lexeme(ast[p].name).to_string();
        let ty = c
            .locator()
            .declared_element(p)
            .map(|e| dartr_resolver::element_ext::variable_type(ctx, e));
        let is_final = ast[constructor].const_keyword.is_some();
        let name = self.name.clone();
        let options = c.options.clone();
        let style = CodeStyleOptions { options: &options };
        let dynamic_non_return =
            style.specify_types() && !style.is_lint_enabled("avoid_annotating_with_dynamic");
        let container = container.raw();
        builder.add_dart_file_edit(c.path, |b| {
            b.insert_field(container, |e| {
                write_field(ctx, e, &name, is_final, false, ty, dynamic_non_return)
            });
        });
    }
}

/// Dart `writeGetterDeclaration` (without body: `=> null;`).
fn write_getter(
    ctx: &dartr_element::Ctx<'_>,
    e: &mut super::super::change_builder::EditBuilder<'_, '_, '_>,
    name: &str,
    is_static: bool,
    return_type: TypeId,
    always_write_type: bool,
) {
    if is_static {
        e.write("static ");
    }
    if always_write_type || !matches!(ctx.ty(return_type), TypeKind::Dynamic) {
        let options = WriteType {
            group_name: Some("TYPE".into()),
            should_write_dynamic: always_write_type,
            ..Default::default()
        };
        if e.write_type(Some(return_type), &options) {
            e.write(" ");
        }
    }
    e.write("get ");
    e.add_simple_linked_edit("NAME", name, None);
    e.write(" => null;");
}

/// Dart `writeSetterDeclaration` (without body: `{}`).
fn write_setter(
    ctx: &dartr_element::Ctx<'_>,
    e: &mut super::super::change_builder::EditBuilder<'_, '_, '_>,
    name: &str,
    is_static: bool,
    parameter_type: Option<TypeId>,
    always_write_type: bool,
) {
    if is_static {
        e.write("static ");
    }
    e.write("set ");
    e.add_simple_linked_edit("NAME", name, None);
    e.write("(");
    let ty = parameter_type.unwrap_or_else(|| ctx.tp.dynamic_type());
    if always_write_type || !matches!(ctx.ty(ty), TypeKind::Dynamic) {
        let options = WriteType {
            group_name: Some("TYPE".into()),
            should_write_dynamic: always_write_type,
            ..Default::default()
        };
        if e.write_type(Some(ty), &options) {
            e.write(" ");
        }
    }
    e.write("value");
    e.write(") ");
    e.write("{}");
}

/// Dart `writeFieldDeclaration`.
fn write_field(
    ctx: &dartr_element::Ctx<'_>,
    e: &mut super::super::change_builder::EditBuilder<'_, '_, '_>,
    name: &str,
    is_final: bool,
    is_static: bool,
    ty: Option<TypeId>,
    always_write_type: bool,
) {
    if is_static {
        e.write("static ");
    }
    if is_final {
        e.write("final ");
    }
    if ty.is_some() || always_write_type {
        if (is_final || always_write_type)
            && ty.is_some_and(|t| matches!(ctx.ty(t), TypeKind::Dynamic))
        {
            e.write("dynamic");
        } else {
            let options = WriteType {
                group_name: Some("TYPE".into()),
                required: !is_final,
                should_write_dynamic: always_write_type,
                ..Default::default()
            };
            e.write_type(ty, &options);
        }
        e.write(" ");
    } else if !is_final {
        e.write("var ");
    }
    e.add_simple_linked_edit("NAME", name, None);
    e.write(";");
}
