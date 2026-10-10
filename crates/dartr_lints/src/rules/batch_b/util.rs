// Dart source: pkg/linter/lib/src/extensions.dart
// Dart source: pkg/linter/lib/src/ast.dart
// Dart source: pkg/linter/lib/src/util/dart_type_utilities.dart
//! Shared helpers of the batch B rules: ports of the linter extensions they
//! use.

use crate::LinterContext;
use dartr_ast::*;
use dartr_element::{Ctx, EId, ElemRef, ElementId, InterfaceElement, Tag, TypeId, TypeKind};
use dartr_typesystem::{TypeExt, member};

/// The resolved lookup context.
pub fn rctx<'a>(c: &LinterContext<'a>) -> Option<Ctx<'a>> {
    c.resolved.map(|r| r.ctx)
}

pub fn lexeme<'a>(c: &'a LinterContext<'_>, token: dartr_syntax::TokenId) -> &'a str {
    c.ast.tokens.lexeme(token)
}

/// The ancestors of [node], nearest first.
pub fn ancestors<'a>(c: &'a LinterContext<'_>, node: NodeId) -> impl Iterator<Item = NodeId> + 'a {
    std::iter::successors(c.ast.parent(node), |n| c.ast.parent(*n))
}

/// Dart `thisOrAncestorMatching`.
pub fn this_or_ancestor(
    c: &LinterContext<'_>,
    node: NodeId,
    mut f: impl FnMut(NodeId) -> bool,
) -> Option<NodeId> {
    std::iter::once(node).chain(ancestors(c, node)).find(|&n| f(n))
}

/// Dart `thisOrAncestorOfType<T>()` for a node kind.
pub fn this_or_ancestor_kind(c: &LinterContext<'_>, node: NodeId, kind: NodeKind) -> Option<NodeId> {
    this_or_ancestor(c, node, |n| c.ast.kind(n) == kind)
}

/// Dart `Expression.unParenthesized`.
pub fn unparenthesized(c: &LinterContext<'_>, mut node: NodeId) -> NodeId {
    while let Some(p) = c.ast.cast::<ParenthesizedExpression>(node) {
        node = c.ast[p].expression.raw();
    }
    node
}

/// Dart `ExpressionNullableExtension.isNullLiteral`.
pub fn is_null_literal(c: &LinterContext<'_>, node: NodeId) -> bool {
    c.ast.kind(unparenthesized(c, node)) == NodeKind::NullLiteral
}

/// Dart `AstNodeNullableExtension.canonicalElement`.
pub fn canonical_element(c: &LinterContext<'_>, node: NodeId) -> Option<ElemRef> {
    if !Expression::test(c.ast.kind(node)) {
        return None;
    }
    let node = unparenthesized(c, node);
    if Identifier::test(c.ast.kind(node)) {
        c.element(node)
    } else if let Some(access) = c.ast.cast::<PropertyAccess>(node) {
        c.element(c.ast[access].property_name)
    } else {
        None
    }
}

/// The base element of an element reference.
pub fn base(c: &LinterContext<'_>, element: ElemRef) -> ElementId {
    member::base_element(&c.resolved.expect("resolved").ctx, element)
}

/// Dart `Element.name` (of the base element).
pub fn name<'a>(c: &LinterContext<'a>, element: ElementId) -> Option<&'a str> {
    c.resolved?.ctx.element_name(element)
}

/// Dart `Element.library?.name` of [element].
pub fn library_name<'a>(c: &LinterContext<'a>, element: ElementId) -> Option<&'a str> {
    let ctx = c.resolved?.ctx;
    let library = member::library(&ctx, ElemRef::Base(element))?;
    ctx.element_name(library.raw())
}

/// Dart `Element.library?.uri` of [element].
pub fn library_uri<'a>(c: &LinterContext<'a>, element: ElementId) -> Option<&'a str> {
    let ctx = c.resolved?.ctx;
    let library = member::library(&ctx, ElemRef::Base(element))?;
    Some(ctx.library_uri(library))
}

/// Dart `Element.enclosingElement` of the base element.
pub fn enclosing(c: &LinterContext<'_>, element: ElementId) -> Option<ElementId> {
    c.resolved?.ctx.element_data(element)?.enclosing
}

/// Dart `DartTypeExtension.typeForInterfaceCheck`.
pub fn type_for_interface_check(c: &LinterContext<'_>, ty: TypeId) -> TypeId {
    let ctx = c.resolved.expect("resolved").ctx;
    match *ctx.ty(ty) {
        TypeKind::TypeParameter {
            param,
            promoted_bound,
            ..
        } => match promoted_bound {
            Some(promoted) => type_for_interface_check(c, promoted),
            None => type_for_interface_check(
                c,
                ctx.get(param)
                    .bound
                    .get()
                    .unwrap_or(ctx.tp.object_question_type()),
            ),
        },
        _ => c.type_system().unwrap().extension_type_erasure(ty),
    }
}

/// Dart `DartTypeExtension.isSameAs(interface, library)` (library name).
pub fn is_same_as(c: &LinterContext<'_>, ty: TypeId, interface: &str, library: &str) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    ctx.interface_element(ty).is_some_and(|e| {
        ctx.element_name(e.raw()) == Some(interface)
            && library_name(c, e.raw()) == Some(library)
    })
}

/// Dart `DartTypeExtension.implementsInterface(interface, library)`.
pub fn implements_interface(
    c: &LinterContext<'_>,
    ty: Option<TypeId>,
    interface: &str,
    library: &str,
) -> bool {
    let Some(ty) = ty else { return false };
    let Some(ctx) = rctx(c) else { return false };
    let check = type_for_interface_check(c, ty);
    let Some(element) = ctx.interface_element(check) else {
        return false;
    };
    is_same_as(c, check, interface, library)
        || ctx
            .element_all_supertypes(element)
            .iter()
            .any(|&t| is_same_as(c, t, interface, library))
}

/// Dart `DartTypeExtension.implementsAnyInterface(definitions)`.
pub fn implements_any_interface(
    c: &LinterContext<'_>,
    ty: Option<TypeId>,
    definitions: &[(&str, &str)],
) -> bool {
    definitions
        .iter()
        .any(|(i, l)| implements_interface(c, ty, i, l))
}

/// Dart `DartTypeExtension.extendsClass(className, library)`.
pub fn extends_class(c: &LinterContext<'_>, ty: Option<TypeId>, class: &str, library: &str) -> bool {
    let Some(ty) = ty else { return false };
    let Some(ctx) = rctx(c) else { return false };
    let check = type_for_interface_check(c, ty);
    let mut seen = indexmap::IndexSet::new();
    let mut current = ctx.interface_element(check).map(|_| check);
    while let Some(t) = current {
        let Some(e) = ctx.interface_element(t) else {
            return false;
        };
        if !seen.insert(e) {
            return false;
        }
        if ctx.element_name(e.raw()) == Some(class) && library_name(c, e.raw()) == Some(library)
        {
            return true;
        }
        current = ctx.superclass(t);
    }
    false
}

/// Dart `InterfaceType.element` of a type, when it is an interface type.
pub fn interface_element(c: &LinterContext<'_>, ty: TypeId) -> Option<EId<InterfaceElement>> {
    rctx(c)?.interface_element(ty)
}

/// Whether [element] is the class [name] declared in the library named
/// [library] (Dart `element.name == name && element.library.name == library`).
pub fn is_element(c: &LinterContext<'_>, element: ElementId, name: &str, library: &str) -> bool {
    self::name(c, element) == Some(name) && library_name(c, element) == Some(library)
}

/// Dart `FunctionBody.isAsynchronous` / `isSynchronous` / `isGenerator`
/// (`keyword` and `star`).
pub fn body_keyword<'a>(c: &'a LinterContext<'_>, body: NodeId) -> (Option<&'a str>, bool) {
    match c.ast.kind(body) {
        NodeKind::BlockFunctionBody => {
            let b = &c.ast[Id::<BlockFunctionBody>::from_raw(body)];
            (b.keyword.map(|k| lexeme(c, k)), b.star.is_some())
        }
        NodeKind::ExpressionFunctionBody => {
            let b = &c.ast[Id::<ExpressionFunctionBody>::from_raw(body)];
            (b.keyword.map(|k| lexeme(c, k)), b.star.is_some())
        }
        _ => (None, false),
    }
}

/// Dart `FunctionBody.isAsynchronous`.
pub fn is_asynchronous(c: &LinterContext<'_>, body: NodeId) -> bool {
    body_keyword(c, body).0 == Some("async")
}

/// The tag of the base element of [element].
pub fn tag(c: &LinterContext<'_>, element: ElemRef) -> Tag {
    base(c, element).tag()
}

/// The fragment flags of the first fragment of [element].
pub fn flags(c: &LinterContext<'_>, element: ElementId) -> dartr_element::FragmentFlags {
    let Some(ctx) = rctx(c) else {
        return dartr_element::FragmentFlags::EMPTY;
    };
    ctx.element_data(element)
        .and_then(|d| ctx.fragment_data(d.first_fragment))
        .map_or(dartr_element::FragmentFlags::EMPTY, |f| f.flags.get())
}

/// Dart `ElementAnnotation.isDeprecated` of an annotation element.
pub fn is_deprecated_annotation_element(c: &LinterContext<'_>, element: ElementId) -> bool {
    let is_dart_core = library_uri(c, element) == Some("dart:core");
    if !is_dart_core {
        return false;
    }
    match element.tag() {
        Tag::Constructor => {
            enclosing(c, element).and_then(|e| name(c, e)) == Some("Deprecated")
        }
        Tag::Getter | Tag::Setter => name(c, element) == Some("deprecated"),
        _ => false,
    }
}

/// The source text of [node].
pub fn text(c: &LinterContext<'_>, node: impl Into<NodeId>) -> String {
    c.text(node)
}

/// Dart `Identifier.name` / `SimpleIdentifier.name`.
pub fn identifier_name(c: &LinterContext<'_>, node: NodeId) -> Option<String> {
    match c.ast.kind(node) {
        NodeKind::SimpleIdentifier => Some(
            lexeme(c, c.ast[Id::<SimpleIdentifier>::from_raw(node)].token).to_string(),
        ),
        NodeKind::PrefixedIdentifier => {
            let p = &c.ast[Id::<PrefixedIdentifier>::from_raw(node)];
            Some(format!(
                "{}.{}",
                lexeme(c, c.ast[p.prefix].token),
                lexeme(c, c.ast[p.identifier].token)
            ))
        }
        _ => None,
    }
}

/// The lexeme of a `SimpleIdentifier` node.
pub fn simple_name<'a>(c: &'a LinterContext<'_>, node: Id<SimpleIdentifier>) -> &'a str {
    lexeme(c, c.ast[node].token)
}

/// The kind of a node.
pub fn kind(c: &LinterContext<'_>, node: impl Into<NodeId>) -> NodeKind {
    c.ast.kind(node.into())
}

/// Dart `SuperFormalParameterElement.superConstructorParameter` (a member
/// of the super constructor when it is substituted).
pub fn super_constructor_parameter(c: &LinterContext<'_>, parameter: ElementId) -> Option<ElemRef> {
    let ctx = rctx(c)?;
    let enclosing = ctx.element_data(parameter)?.enclosing?;
    if enclosing.tag() != Tag::Constructor {
        return None;
    }
    let constructor = ctx.get(EId::<dartr_element::ConstructorElement>::from_raw(enclosing));
    let super_constructor = constructor.super_constructor.get()?;
    let super_parameters = member::formal_parameters(&ctx, super_constructor);
    let kind = |e: ElemRef| {
        ctx.get(EId::<dartr_element::FormalParameterElement>::from_raw(
            member::base_element(&ctx, e),
        ))
        .kind
    };
    if kind(ElemRef::Base(parameter)).is_named() {
        let name = ctx.element_name(parameter);
        super_parameters
            .into_iter()
            .find(|&p| kind(p).is_named() && ctx.element_name(member::base_element(&ctx, p)) == name)
    } else {
        let index = ctx
            .executable(EId::<dartr_element::ExecutableElement>::from_raw(enclosing))
            .formal_params
            .iter()
            .filter(|p| p.raw().tag() == Tag::SuperFormalParameter)
            .position(|p| p.raw() == parameter)?;
        super_parameters
            .into_iter()
            .filter(|&p| kind(p).is_positional())
            .nth(index)
    }
}

/// Dart `DartType ==`.
pub fn types_equal(c: &LinterContext<'_>, a: TypeId, b: TypeId) -> bool {
    rctx(c).is_some_and(|ctx| dartr_typesystem::equality::dart_eq(&ctx, a, b))
}

/// The type of an element (Dart `VariableElement.type`, of a member too).
pub fn element_type(c: &LinterContext<'_>, element: ElemRef) -> Option<TypeId> {
    Some(member::type_(&rctx(c)?, element))
}

/// Dart `TypeAnnotation.type`.
pub fn annotation_type(c: &LinterContext<'_>, node: impl Into<NodeId>) -> Option<TypeId> {
    c.resolved?.tables.annotation_type.get(node.into()).copied()
}

/// Dart `MethodInvocation.realTarget` / `PropertyAccess.realTarget` /
/// `IndexExpression.realTarget`: the target, or the target of the enclosing
/// cascade.
pub fn real_target(c: &LinterContext<'_>, node: NodeId) -> Option<NodeId> {
    let target = match kind(c, node) {
        NodeKind::MethodInvocation => c.ast[Id::<MethodInvocation>::from_raw(node)].target.map(|t| t.raw()),
        NodeKind::PropertyAccess => c.ast[Id::<PropertyAccess>::from_raw(node)].target.map(|t| t.raw()),
        NodeKind::IndexExpression => c.ast[Id::<IndexExpression>::from_raw(node)].target.map(|t| t.raw()),
        _ => None,
    };
    if target.is_some() {
        return target;
    }
    ancestors(c, node)
        .find_map(|a| c.ast.cast::<CascadeExpression>(a))
        .map(|cascade| c.ast[cascade].target.raw())
}

/// The arguments of an argument list.
pub fn arguments(c: &LinterContext<'_>, list: Id<ArgumentList>) -> Vec<NodeId> {
    c.ast.list_raw(c.ast[list].arguments).to_vec()
}

/// The name of a named argument.
pub fn named_argument_name<'a>(c: &'a LinterContext<'_>, argument: NodeId) -> Option<&'a str> {
    c.ast
        .cast::<NamedArgument>(argument)
        .map(|n| lexeme(c, c.ast[n].name))
}

/// Dart `Argument.argumentExpression`.
pub fn argument_expression(c: &LinterContext<'_>, argument: NodeId) -> NodeId {
    c.ast
        .cast::<NamedArgument>(argument)
        .map_or(argument, |n| c.ast[n].argument_expression.raw())
}

/// Dart `canonicalElementsFromIdentifiersAreEqual`
/// (`util/dart_type_utilities.dart`).
pub fn canonical_elements_from_identifiers_are_equal(
    c: &LinterContext<'_>,
    e1: Option<NodeId>,
    e2: Option<NodeId>,
) -> bool {
    let (Some(e1), Some(e2)) = (e1, e2) else {
        return false;
    };
    let e1 = unparenthesized(c, e1);
    let e2 = unparenthesized(c, e2);
    // Dart `canonicalElementsAreEqual` of `writeOrReadElement`s.
    let wr = |n: NodeId| c.write_or_read_element(n).and_then(|e| c.canonical_element2(e));
    let el = |n: NodeId| c.element(n).and_then(|e| c.canonical_element2(e));
    match (kind(c, e1), kind(c, e2)) {
        (NodeKind::SimpleIdentifier, k) => k == NodeKind::SimpleIdentifier && wr(e1) == wr(e2),
        (NodeKind::PrefixedIdentifier, k) => {
            if k != NodeKind::PrefixedIdentifier {
                return false;
            }
            let a = &c.ast[Id::<PrefixedIdentifier>::from_raw(e1)];
            let b = &c.ast[Id::<PrefixedIdentifier>::from_raw(e2)];
            el(a.prefix.raw()) == el(b.prefix.raw()) && wr(a.identifier.raw()) == wr(b.identifier.raw())
        }
        (NodeKind::PropertyAccess, NodeKind::PropertyAccess) => {
            let a = &c.ast[Id::<PropertyAccess>::from_raw(e1)];
            let b = &c.ast[Id::<PropertyAccess>::from_raw(e2)];
            canonical_elements_from_identifiers_are_equal(
                c,
                a.target.map(|t| t.raw()),
                b.target.map(|t| t.raw()),
            ) && wr(a.property_name.raw()) == wr(b.property_name.raw())
        }
        _ => false,
    }
}

/// Dart `MethodDeclarationExtension.isOverride`.
pub fn method_is_override(c: &LinterContext<'_>, node: NodeId) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    let Some(element) = c.declared_element(node) else {
        return false;
    };
    let Some(name) = ctx.element_name(element) else {
        return false;
    };
    let Some(parent) = enclosing(c, element).and_then(|e| e.cast::<InterfaceElement>()) else {
        return false;
    };
    let Some(library) = ctx.element_data(parent.raw()).and_then(|d| d.library) else {
        return false;
    };
    let n = &c.ast[Id::<MethodDeclaration>::from_raw(node)];
    let property = n.property_keyword.map(|k| lexeme(c, k));
    let options = dartr_typesystem::lookup::LookUpOptions::default();
    ctx.element_all_supertypes(parent).iter().any(|&t| match property {
        Some("get") => dartr_typesystem::lookup::type_look_up_getter(&ctx, t, name, library, options).is_some(),
        Some("set") => dartr_typesystem::lookup::type_look_up_setter(&ctx, t, name, library, options).is_some(),
        _ => dartr_typesystem::lookup::type_look_up_method(&ctx, t, name, library, options).is_some(),
    })
}

/// Dart `MethodDeclarationExtension.lookUpInheritedMethod`.
pub fn look_up_inherited_method(c: &LinterContext<'_>, node: NodeId) -> Option<ElemRef> {
    let ctx = rctx(c)?;
    let element = c.declared_element(node)?;
    let parent = enclosing(c, element)?.cast::<InterfaceElement>()?;
    let name = dartr_typesystem::inheritance_manager3::Name::for_element(&ctx, ElemRef::Base(element))?;
    let inherited = dartr_typesystem::inheritance_manager3::InheritanceManager3::new(ctx)
        .get_inherited(parent, name)?;
    (member::base_element(&ctx, inherited).tag() == Tag::Method).then_some(inherited)
}

/// Dart `ExpressionImpl.inConstantContext`.
pub fn in_constant_context(c: &LinterContext<'_>, node: NodeId) -> bool {
    crate::rules::q_z::in_constant_context(c, node)
}

/// Whether [node] is a `MethodDeclaration` getter / setter.
pub fn method_property<'a>(c: &'a LinterContext<'_>, node: NodeId) -> Option<&'a str> {
    c.ast[Id::<MethodDeclaration>::from_raw(node)]
        .property_keyword
        .map(|k| lexeme(c, k))
}

/// Dart `VariableElement.computeConstantValue()`.
pub fn element_constant_value(
    c: &LinterContext<'_>,
    element: ElementId,
) -> Option<dartr_constant::DartObjectImpl> {
    c.resolved?.metadata?.element_constant_value(element)
}

/// Dart `DartObject ==` (both may be `null`).
pub fn constant_values_equal(
    c: &LinterContext<'_>,
    a: &Option<dartr_constant::DartObjectImpl>,
    b: &Option<dartr_constant::DartObjectImpl>,
) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => c
            .constant_type_system()
            .is_some_and(|ts| a.dart_eq(b, &ts)),
        _ => false,
    }
}

/// Dart `Element.isStatic` of a field / method / accessor.
pub fn is_static(c: &LinterContext<'_>, element: ElementId) -> bool {
    rctx(c).is_some_and(|ctx| member::is_static(&ctx, ElemRef::Base(element)))
}

/// Dart `ElementExtension.isWildcardVariable` of a local / parameter
/// element (`name == '_'` and the wildcard variables feature).
pub fn is_wildcard_variable(c: &LinterContext<'_>, element: ElementId) -> bool {
    name(c, element) == Some("_")
        && matches!(
            element.tag(),
            Tag::LocalFunction
                | Tag::LocalVariable
                | Tag::PatternVariable
                | Tag::BindPatternVariable
                | Tag::JoinPatternVariable
                | Tag::Prefix
                | Tag::TypeParameter
                | Tag::FormalParameter
        )
        && c.is_feature_enabled(crate::ExperimentalFlag::WildcardVariables)
}

/// Dart `FormalParameter.isFinal` / `isConst` (the keyword).
pub fn parameter_keyword<'a>(c: &'a LinterContext<'_>, parameter: NodeId) -> Option<&'a str> {
    let keyword = match kind(c, parameter) {
        NodeKind::RegularFormalParameter => {
            c.ast[Id::<RegularFormalParameter>::from_raw(parameter)].const_final_or_var_keyword
        }
        NodeKind::FieldFormalParameter => {
            c.ast[Id::<FieldFormalParameter>::from_raw(parameter)].const_final_or_var_keyword
        }
        NodeKind::SuperFormalParameter => {
            c.ast[Id::<SuperFormalParameter>::from_raw(parameter)].const_final_or_var_keyword
        }
        _ => None,
    };
    keyword.map(|k| lexeme(c, k))
}

/// The parameters of an optional formal parameter list.
pub fn parameters(c: &LinterContext<'_>, list: Option<Id<FormalParameterList>>) -> Vec<NodeId> {
    list.map(|l| c.ast.list_raw(c.ast[l].parameters).to_vec())
        .unwrap_or_default()
}

/// Dart `element is LocalVariableElement` (pattern variables are local
/// variables in Dart).
pub fn is_local_variable(element: ElementId) -> bool {
    matches!(
        element.tag(),
        Tag::LocalVariable | Tag::PatternVariable | Tag::BindPatternVariable | Tag::JoinPatternVariable
    )
}

/// Dart `FormalParameter.isRequired` (required positional or required
/// named).
pub fn parameter_is_required(c: &LinterContext<'_>, parameter: NodeId) -> bool {
    let kind = match self::kind(c, parameter) {
        NodeKind::RegularFormalParameter => c.ast[Id::<RegularFormalParameter>::from_raw(parameter)].kind,
        NodeKind::FieldFormalParameter => c.ast[Id::<FieldFormalParameter>::from_raw(parameter)].kind,
        NodeKind::SuperFormalParameter => c.ast[Id::<SuperFormalParameter>::from_raw(parameter)].kind,
        _ => return false,
    };
    kind.is_required()
}

/// Dart `getNodeToAnnotate(node)` (linter `ast.dart`): the (offset, length)
/// of the entity to report for a declaration.
pub fn node_to_annotate(c: &LinterContext<'_>, node: NodeId) -> (usize, usize) {
    let token = |t: dartr_syntax::TokenId| {
        let t = c.ast.tokens.get(t);
        (t.offset as usize, t.length as usize)
    };
    let whole = |n: NodeId| (c.ast.offset(n) as usize, c.ast.length(n) as usize);
    let type_name = |name_part: NodeId| {
        if let Some(n) = c.ast.cast::<NameWithTypeParameters>(name_part) {
            Some(c.ast[n].type_name)
        } else {
            c.ast
                .cast::<PrimaryConstructorDeclaration>(name_part)
                .map(|p| c.ast[p].type_name)
        }
    };
    match kind(c, node) {
        NodeKind::ClassDeclaration => {
            type_name(c.ast[Id::<ClassDeclaration>::from_raw(node)].name_part.raw()).map_or(whole(node), token)
        }
        NodeKind::ClassTypeAlias => token(c.ast[Id::<ClassTypeAlias>::from_raw(node)].name),
        NodeKind::ConstructorDeclaration => {
            let n = &c.ast[Id::<ConstructorDeclaration>::from_raw(node)];
            if let Some(name) = n.name {
                token(name)
            } else if let Some(t) = n.type_name {
                whole(t.raw())
            } else {
                token(n.new_keyword.or(n.factory_keyword).unwrap())
            }
        }
        NodeKind::EnumConstantDeclaration => token(c.ast[Id::<EnumConstantDeclaration>::from_raw(node)].name),
        NodeKind::EnumDeclaration => {
            type_name(c.ast[Id::<EnumDeclaration>::from_raw(node)].name_part.raw()).map_or(whole(node), token)
        }
        NodeKind::ExtensionDeclaration => c.ast[Id::<ExtensionDeclaration>::from_raw(node)]
            .name
            .map_or(whole(node), token),
        NodeKind::FieldDeclaration => whole(c.ast[Id::<FieldDeclaration>::from_raw(node)].fields.raw()),
        NodeKind::FunctionDeclaration => token(c.ast[Id::<FunctionDeclaration>::from_raw(node)].name),
        NodeKind::FunctionTypeAlias => token(c.ast[Id::<FunctionTypeAlias>::from_raw(node)].name),
        NodeKind::GenericTypeAlias => token(c.ast[Id::<GenericTypeAlias>::from_raw(node)].name),
        NodeKind::MethodDeclaration => token(c.ast[Id::<MethodDeclaration>::from_raw(node)].name),
        NodeKind::MixinDeclaration => token(c.ast[Id::<MixinDeclaration>::from_raw(node)].name),
        NodeKind::PrimaryConstructorBody => token(c.ast[Id::<PrimaryConstructorBody>::from_raw(node)].this_keyword),
        NodeKind::PrimaryConstructorDeclaration => {
            let n = &c.ast[Id::<PrimaryConstructorDeclaration>::from_raw(node)];
            token(n.constructor_name.map(|cn| c.ast[cn].name).unwrap_or(n.type_name))
        }
        NodeKind::TopLevelVariableDeclaration => {
            whole(c.ast[Id::<TopLevelVariableDeclaration>::from_raw(node)].variables.raw())
        }
        NodeKind::TypeParameter => token(c.ast[Id::<TypeParameter>::from_raw(node)].name),
        NodeKind::VariableDeclaration => token(c.ast[Id::<VariableDeclaration>::from_raw(node)].name),
        NodeKind::ExtensionTypeDeclaration => {
            type_name(c.ast[Id::<ExtensionTypeDeclaration>::from_raw(node)].name_part.raw()).map_or(whole(node), token)
        }
        _ => whole(node),
    }
}

/// Dart `Element.isPublic` (`!isPrivate`: a name that does not start with
/// `_`).
pub fn is_public(c: &LinterContext<'_>, element: ElementId) -> bool {
    !name(c, element).is_some_and(|n| n.starts_with('_'))
}

/// Dart `Metadata.hasVisibleForTesting`.
pub fn has_visible_for_testing(c: &LinterContext<'_>, element: ElementId) -> bool {
    c.has_package_meta_getter(element, "visibleForTesting")
}

/// Dart `Expression.hasObviousType` (`util/obvious_types.dart`).
pub fn has_obvious_type(c: &LinterContext<'_>, node: NodeId) -> bool {
    crate::rules::batch_a3::omit_obvious_property_types::is_obvious(c, node)
}

/// Dart `ElementExtension.overriddenMember` (linter `extensions.dart`).
pub fn overridden_member(c: &LinterContext<'_>, element: ElementId) -> Option<ElemRef> {
    let ctx = rctx(c)?;
    let member = match ctx.any(element) {
        dartr_element::AnyElement::Field(f) => f.getter.map(|g| g.raw())?,
        dartr_element::AnyElement::Method(_)
        | dartr_element::AnyElement::Getter(_)
        | dartr_element::AnyElement::Setter(_) => element,
        _ => return None,
    };
    let interface = enclosing(c, member)?.cast::<InterfaceElement>()?;
    let name = dartr_typesystem::inheritance_manager3::Name::for_element(&ctx, ElemRef::Base(member))?;
    dartr_typesystem::inheritance_manager3::InheritanceManager3::new(ctx).get_inherited(interface, name)
}

/// Dart `PromotableElementImpl`: local variables and formal parameters.
pub fn is_promotable(element: ElementId) -> bool {
    is_local_variable(element)
        || matches!(
            element.tag(),
            Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter
        )
}

/// Dart `StringExtension.isJustUnderscores`.
pub fn is_just_underscores(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c == '_')
}

/// Dart `isAugmentation` of a declaration (its `augment` keyword).
pub fn is_augmentation(c: &LinterContext<'_>, node: NodeId) -> bool {
    macro_rules! augment {
        ($($t:ident),*) => {
            match kind(c, node) {
                $(NodeKind::$t => c.ast[Id::<$t>::from_raw(node)].augment_keyword.is_some(),)*
                _ => false,
            }
        };
    }
    augment!(
        ClassDeclaration,
        ClassTypeAlias,
        ConstructorDeclaration,
        EnumConstantDeclaration,
        EnumDeclaration,
        ExtensionDeclaration,
        ExtensionTypeDeclaration,
        FieldDeclaration,
        FunctionDeclaration,
        FunctionTypeAlias,
        GenericTypeAlias,
        MethodDeclaration,
        MixinDeclaration,
        TopLevelVariableDeclaration
    )
}

/// Dart `getIntValue(expression, context)` (linter `ast.dart`).
pub fn get_int_value(c: &LinterContext<'_>, expression: NodeId) -> Option<i64> {
    if let Some(prefix) = c.ast.cast::<PrefixExpression>(expression) {
        if lexeme(c, c.ast[prefix].operator) != "-" {
            return None;
        }
        return get_int_value_inner(c, c.ast[prefix].operand.raw()).map(|v| v.wrapping_neg());
    }
    get_int_value_inner(c, expression)
}

fn get_int_value_inner(c: &LinterContext<'_>, expression: NodeId) -> Option<i64> {
    if let Some(literal) = c.ast.cast::<IntegerLiteral>(expression) {
        c.ast[literal].value
    } else if kind(c, expression) == NodeKind::SimpleIdentifier {
        c.constant_value(expression).and_then(|v| v.to_int_value())
    } else {
        None
    }
}
