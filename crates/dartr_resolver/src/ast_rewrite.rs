// Dart source: pkg/analyzer/lib/src/dart/resolver/ast_rewrite.dart

//! `AstRewriter`: rewrites nodes whose meaning depends on the scope
//! (`MethodInvocation` → `InstanceCreationExpression` / `ExtensionOverride`,
//! `InstanceCreationExpression` → `MethodInvocation`, `PrefixedIdentifier` /
//! `PropertyAccess` → `ConstructorReference`, identifiers → `TypeLiteral`).
//!
//! Each method returns the node that replaces [node] (or [node] itself).
//! New nodes are added with `Ast::add` (which makes them the parent of the
//! moved children) and put in place with `Ast::replace_with`; the old node
//! stays in the arena, detached. Side tables are keyed by node, so a new
//! node starts without resolution data.
//!
//! The element that Dart stores in a new `ExtensionOverride`
//! (`ExtensionOverrideImpl.element`) and in a new `ImportPrefixReference`
//! is in `ResolutionTables.element`; the type of the `NamedType` of a new
//! type literal (`typeName.type = element.aliasedType`) is in
//! `ResolutionTables.annotation_type`.

// The Dart names `_toX` take `this`.
#![allow(clippy::wrong_self_convention)]

use dartr_ast::{
    Annotation, AssignmentExpression, Ast, CommentReference, ConstantPattern, ConstructorName,
    ConstructorReference, ExtensionOverride, FunctionReference, Id, Identifier,
    ImportPrefixReference, InstanceCreationExpression, MethodInvocation, NamedType, NodeId,
    PrefixedIdentifier, PropertyAccess, SimpleIdentifier, TypeArgumentList, TypeLiteral,
};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{
    Ctx, EId, ElemRef, ElementId, InterfaceElement, LibraryElement, PrefixElement,
    ResolutionTables, Tag, TypeAliasElement, TypeId, TypeKind,
};
use dartr_syntax::TokenId;
use dartr_typesystem::inheritance_manager3::{InheritanceManager3, Name};

use crate::ast_ext::{
    identifier_name, method_invocation_real_target, property_access_is_cascaded, token_is_synthetic,
};
use crate::scope::NameScope;
use crate::scope_context::ScopeContext;

/// Dart `AstRewriter`, with what its methods read and write.
pub struct AstRewriter<'r, 'a> {
    pub ctx: Ctx<'a>,
    /// Dart `nameScope` (and the prefix scopes through it).
    pub scope: &'r ScopeContext<'a>,
    pub tables: &'r mut ResolutionTables,
    /// Dart `_diagnosticReporter`.
    pub diagnostics: &'r mut Vec<Diagnostic>,
}

/// Whether [e] is a Dart `ExecutableElement`.
fn is_executable(e: ElementId) -> bool {
    matches!(
        e.tag(),
        Tag::Method
            | Tag::Constructor
            | Tag::Getter
            | Tag::Setter
            | Tag::TopLevelFunction
            | Tag::LocalFunction
    )
}

impl<'a> AstRewriter<'_, 'a> {
    fn lookup_getter(&self, id: &str) -> Option<ElementId> {
        self.scope.lookup(id).getter
    }

    fn prefix_lookup_getter(&self, prefix: EId<PrefixElement>, id: &str) -> Option<ElementId> {
        self.scope
            .library_scopes()
            .prefix_lookup(&self.ctx, prefix, id)
            .getter
    }

    /// The aliased type of a type alias element.
    fn aliased_type(&self, e: EId<TypeAliasElement>) -> Option<TypeId> {
        self.ctx.get(e).aliased_type.get()
    }

    /// `element is TypeAliasElement && element.aliasedType is FunctionType`.
    fn is_function_type_alias(&self, e: ElementId) -> Option<EId<TypeAliasElement>> {
        let alias = e.cast::<TypeAliasElement>()?;
        let aliased = self.aliased_type(alias)?;
        matches!(self.ctx.ty(aliased), TypeKind::Function(_)).then_some(alias)
    }

    /// `element is TypeAliasElement && element.aliasedType is InterfaceType`:
    /// the element of the aliased interface type.
    fn aliased_interface_element(&self, e: ElementId) -> Option<EId<InterfaceElement>> {
        let alias = e.cast::<TypeAliasElement>()?;
        let aliased = self.aliased_type(alias)?;
        match *self.ctx.ty(aliased) {
            TypeKind::Interface { element, .. } => Some(element),
            _ => None,
        }
    }

    fn simple_identifier(ast: &mut Ast, token: TokenId) -> Id<SimpleIdentifier> {
        ast.add(SimpleIdentifier { token })
    }

    fn prefixed_identifier(
        ast: &mut Ast,
        prefix: TokenId,
        period: TokenId,
        identifier: TokenId,
    ) -> Id<Identifier> {
        let prefix = Self::simple_identifier(ast, prefix);
        let identifier = Self::simple_identifier(ast, identifier);
        ast.add(PrefixedIdentifier {
            prefix,
            period,
            identifier,
        })
        .upcast()
    }

    /// Dart `instanceCreationExpression`: possibly rewrites `a<...>.b(...)`
    /// (or `p.a<...>.b(...)`) as a `MethodInvocation` with a
    /// `FunctionReference` target.
    pub fn instance_creation_expression(
        &mut self,
        ast: &mut Ast,
        node: Id<InstanceCreationExpression>,
        library: EId<LibraryElement>,
        enclosing_instance_element: Option<EId<dartr_element::InstanceElement>>,
    ) -> NodeId {
        if ast[node].keyword.is_some() {
            // Either `new` or `const` has been specified.
            return node.raw();
        }
        let type_node = ast[ast[node].constructor_name].type_;
        let (import_prefix, name_token) = (ast[type_node].import_prefix, ast[type_node].name);
        match import_prefix {
            None => {
                let name = ast.tokens.lexeme(name_token).to_string();
                let mut element = self.lookup_getter(&name);
                if element.is_none() {
                    if let Some(enclosing) =
                        enclosing_instance_element.and_then(|e| e.raw().cast::<InterfaceElement>())
                    {
                        let manager = InheritanceManager3::new(self.ctx.global());
                        let member_name = Name::new(&self.ctx, Some(library), &name);
                        element = manager
                            .get_member(enclosing, member_name)
                            .map(|m| dartr_typesystem::member::base_element(&self.ctx, m));
                    }
                }
                if let Some(e) = element {
                    if is_executable(e) {
                        let function = Self::simple_identifier(ast, name_token).upcast();
                        return self
                            .to_method_invocation_of_function_reference(ast, node, function);
                    } else if let Some(alias) = self.is_function_type_alias(e) {
                        return self.to_method_invocation_of_aliased_type_literal(ast, node, alias);
                    }
                }
            }
            Some(import_prefix) => {
                let (prefix_token, period) = (ast[import_prefix].name, ast[import_prefix].period);
                let prefix_name = ast.tokens.lexeme(prefix_token).to_string();
                let prefix_element = self.lookup_getter(&prefix_name);
                if let Some(prefix) = prefix_element.and_then(|e| e.cast::<PrefixElement>()) {
                    let prefixed_name = ast.tokens.lexeme(name_token).to_string();
                    let element = self.prefix_lookup_getter(prefix, &prefixed_name);
                    if let Some(e) = element {
                        if e.tag() == Tag::TopLevelFunction {
                            let function =
                                Self::prefixed_identifier(ast, prefix_token, period, name_token);
                            return self
                                .to_method_invocation_of_function_reference(ast, node, function);
                        } else if let Some(alias) = self.is_function_type_alias(e) {
                            return self
                                .to_method_invocation_of_aliased_type_literal(ast, node, alias);
                        }
                    }
                    // A class, a type alias of an interface type, a type
                    // alias of a function type with a method call, or an
                    // unresolved name: do not rewrite.
                    return node.raw();
                } else {
                    // `typeName`, as a prefixed identifier, cannot refer to
                    // a class or an aliased type.
                    let function = Self::prefixed_identifier(ast, prefix_token, period, name_token);
                    return self.to_method_invocation_of_function_reference(ast, node, function);
                }
            }
        }
        node.raw()
    }

    /// Dart `methodInvocation`: possibly rewrites [node] as an
    /// `ExtensionOverride` or as an `InstanceCreationExpression`.
    pub fn method_invocation(&mut self, ast: &mut Ast, node: Id<MethodInvocation>) -> NodeId {
        let method_name = ast[node].method_name;
        let method_token = ast[method_name].token;
        if token_is_synthetic(ast, method_token) {
            // Not a constructor invocation: the method name is synthetic.
            return node.raw();
        }
        let target = ast[node].target;
        let operator = ast[node].operator;
        let method_name_text = identifier_name(ast, method_name).to_string();
        match target {
            None => {
                // Possible cases: C() or C<>()
                if method_invocation_real_target(ast, node).is_some() {
                    // Not a constructor invocation: it is in a cascade.
                    return node.raw();
                }
                let element = self.lookup_getter(&method_name_text);
                if let Some(e) = element {
                    if e.is::<InterfaceElement>() {
                        return self.to_instance_creation_type(ast, node, method_name);
                    } else if e.tag() == Tag::Extension {
                        let (type_arguments, argument_list) =
                            (ast[node].type_arguments, ast[node].argument_list);
                        let extension_override = ast.add(ExtensionOverride {
                            import_prefix: None,
                            name: method_token,
                            type_arguments,
                            argument_list,
                        });
                        self.tables
                            .element
                            .insert(extension_override, ElemRef::Base(e));
                        ast.replace_with(node, extension_override);
                        return extension_override.raw();
                    } else if self.aliased_interface_element(e).is_some() {
                        return self.to_instance_creation_type(ast, node, method_name);
                    }
                }
            }
            Some(target) => {
                if let (Some(target), Some(operator)) =
                    (ast.cast::<SimpleIdentifier>(target), operator)
                {
                    // Possible cases: C.n(), p.C() or p.C<>()
                    let target_name = identifier_name(ast, target).to_string();
                    let element = self.lookup_getter(&target_name);
                    if let Some(e) = element {
                        if let Some(class_element) = e.cast::<InterfaceElement>() {
                            // class C { C.named(); }
                            // C.named()
                            return self.to_instance_creation_type_constructor(
                                ast,
                                node,
                                target,
                                method_name,
                                class_element,
                            );
                        } else if let Some(prefix) = e.cast::<PrefixElement>() {
                            // Possible cases: p.C() or p.C<>()
                            let prefixed_element =
                                self.prefix_lookup_getter(prefix, &method_name_text);
                            if let Some(pe) = prefixed_element {
                                if pe.is::<InterfaceElement>() {
                                    return self.to_instance_creation_prefix_type(
                                        ast,
                                        node,
                                        target,
                                        method_name,
                                    );
                                } else if pe.tag() == Tag::Extension {
                                    let target_token = ast[target].token;
                                    let import_prefix = ast.add(ImportPrefixReference {
                                        name: target_token,
                                        period: operator,
                                    });
                                    self.tables.element.insert(import_prefix, ElemRef::Base(e));
                                    let (type_arguments, argument_list) =
                                        (ast[node].type_arguments, ast[node].argument_list);
                                    let extension_override = ast.add(ExtensionOverride {
                                        import_prefix: Some(import_prefix),
                                        name: method_token,
                                        type_arguments,
                                        argument_list,
                                    });
                                    self.tables
                                        .element
                                        .insert(extension_override, ElemRef::Base(pe));
                                    ast.replace_with(node, extension_override);
                                    return extension_override.raw();
                                } else if self.aliased_interface_element(pe).is_some() {
                                    return self.to_instance_creation_prefix_type(
                                        ast,
                                        node,
                                        target,
                                        method_name,
                                    );
                                }
                            }
                        } else if let Some(class_element) = self.aliased_interface_element(e) {
                            // class C { C.named(); }
                            // typedef X = C;
                            // X.named()
                            return self.to_instance_creation_type_constructor(
                                ast,
                                node,
                                target,
                                method_name,
                                class_element,
                            );
                        }
                    }
                } else if let Some(target) = ast.cast::<PrefixedIdentifier>(target) {
                    // Possible case: p.C.n()
                    let prefix = ast[target].prefix;
                    let prefix_name = identifier_name(ast, prefix).to_string();
                    let prefix_element = self.lookup_getter(&prefix_name);
                    match prefix_element {
                        Some(e) => {
                            self.tables.element.insert(prefix, ElemRef::Base(e));
                        }
                        None => {
                            self.tables.element.remove(prefix);
                        }
                    }
                    if let Some(prefix_element) =
                        prefix_element.and_then(|e| e.cast::<PrefixElement>())
                    {
                        let prefixed_name =
                            identifier_name(ast, ast[target].identifier).to_string();
                        let element = self.prefix_lookup_getter(prefix_element, &prefixed_name);
                        if let Some(e) = element {
                            if let Some(class_element) = e.cast::<InterfaceElement>() {
                                return self.instance_creation_prefix_type_name(
                                    ast,
                                    node,
                                    target,
                                    method_name,
                                    class_element,
                                );
                            } else if let Some(class_element) = self.aliased_interface_element(e) {
                                return self.instance_creation_prefix_type_name(
                                    ast,
                                    node,
                                    target,
                                    method_name,
                                    class_element,
                                );
                            }
                        }
                    }
                }
            }
        }
        node.raw()
    }

    /// Dart `prefixedIdentifier`: possibly rewrites `List.filled` as a
    /// `ConstructorReference`, or `p.C` as a `TypeLiteral`.
    pub fn prefixed_identifier_node(
        &mut self,
        ast: &mut Ast,
        node: Id<PrefixedIdentifier>,
    ) -> NodeId {
        let Some(parent) = ast.parent(node) else {
            return node.raw();
        };
        if ast.is::<Annotation>(parent) {
            // An annotation which is a const constructor invocation can
            // initially be a prefixed identifier.
            return node.raw();
        }
        if ast.is::<CommentReference>(parent) {
            return node.raw();
        }
        if let Some(a) = ast.cast::<AssignmentExpression>(parent) {
            if ast[a].left_hand_side.raw() == node.raw() {
                // A constructor cannot be assigned to.
                return node.raw();
            }
        }
        let identifier = ast[node].identifier;
        if token_is_synthetic(ast, ast[identifier].token) {
            return node.raw();
        }
        let prefix = ast[node].prefix;
        let prefix_name = identifier_name(ast, prefix).to_string();
        let identifier_text = identifier_name(ast, identifier).to_string();
        let prefix_element = self.lookup_getter(&prefix_name);

        if let Some(prefix_scope) = prefix_element.and_then(|e| e.cast::<PrefixElement>()) {
            if ast.is::<ConstantPattern>(parent) {
                let element = self.prefix_lookup_getter(prefix_scope, &identifier_text);
                if element.is_some_and(|e| {
                    matches!(
                        e.tag(),
                        Tag::Dynamic | Tag::Never | Tag::TypeAlias | Tag::TypeParameter
                    ) || e.is::<InterfaceElement>()
                }) {
                    return self.to_type_literal(ast, node.upcast());
                }
            }
        }

        if ast.is_in_value_expression_slot(parent, node) {
            if let Some(prefix_scope) = prefix_element.and_then(|e| e.cast::<PrefixElement>()) {
                let element = self.prefix_lookup_getter(prefix_scope, &identifier_text);
                if element.is_some_and(|e| {
                    matches!(e.tag(), Tag::Dynamic | Tag::Never | Tag::TypeAlias)
                        || e.is::<InterfaceElement>()
                }) {
                    return self.to_type_literal(ast, node.upcast());
                }
            }
        }

        if let Some(e) = prefix_element {
            if let Some(class_element) = e.cast::<InterfaceElement>() {
                // class C { C.named(); }
                // C.named
                return self.to_constructor_reference_prefixed(ast, node, class_element);
            } else if let Some(class_element) = self.aliased_interface_element(e) {
                // class C { C.named(); }
                // typedef X = C;
                // X.named
                return self.to_constructor_reference_prefixed(ast, node, class_element);
            }
        }
        node.raw()
    }

    /// Dart `propertyAccess`: possibly rewrites `async.Future.value` or
    /// `List<int>.filled` as a `ConstructorReference`.
    pub fn property_access(&mut self, ast: &mut Ast, node: Id<PropertyAccess>) -> NodeId {
        if property_access_is_cascaded(ast, node) {
            // For example, `List..filled`: a property access on a `Type`.
            return node.raw();
        }
        if ast
            .parent(node)
            .is_some_and(|p| ast.is::<CommentReference>(p))
        {
            return node.raw();
        }
        let Some(receiver) = ast[node].target else {
            return node.raw();
        };

        let receiver_identifier: Id<Identifier>;
        let mut type_arguments: Option<Id<TypeArgumentList>> = None;
        if let Some(r) = ast.cast::<PrefixedIdentifier>(receiver) {
            receiver_identifier = r.upcast();
        } else if let Some(r) = ast.cast::<FunctionReference>(receiver) {
            // `List<int>.filled` or `core.List<int>.filled` is parsed as a
            // property access with a function reference target.
            let function = ast[r].function;
            let Some(function) = ast.cast::<Identifier>(function) else {
                return node.raw();
            };
            receiver_identifier = function;
            type_arguments = ast[r].type_arguments;
        } else {
            return node.raw();
        }

        let element = if let Some(s) = ast.cast::<SimpleIdentifier>(receiver_identifier) {
            let name = identifier_name(ast, s).to_string();
            self.lookup_getter(&name)
        } else if let Some(p) = ast.cast::<PrefixedIdentifier>(receiver_identifier) {
            let prefix_name = identifier_name(ast, ast[p].prefix).to_string();
            let prefix_element = self.lookup_getter(&prefix_name);
            match prefix_element.and_then(|e| e.cast::<PrefixElement>()) {
                Some(prefix) => {
                    let name = identifier_name(ast, ast[p].identifier).to_string();
                    self.prefix_lookup_getter(prefix, &name)
                }
                // Something like `foo.List<int>.filled` where `foo` is not
                // an import prefix.
                None => return node.raw(),
            }
        } else {
            None
        };

        if let Some(e) = element {
            if let Some(class_element) = e.cast::<InterfaceElement>() {
                return self.to_constructor_reference_property_access(
                    ast,
                    node,
                    receiver_identifier,
                    type_arguments,
                    class_element,
                );
            } else if let Some(class_element) = self.aliased_interface_element(e) {
                return self.to_constructor_reference_property_access(
                    ast,
                    node,
                    receiver_identifier,
                    type_arguments,
                    class_element,
                );
            }
        }
        node.raw()
    }

    /// Dart `simpleIdentifier`: possibly rewrites [node] as a `TypeLiteral`.
    pub fn simple_identifier_node(&mut self, ast: &mut Ast, node: Id<SimpleIdentifier>) -> NodeId {
        let Some(parent) = ast.parent(node) else {
            return node.raw();
        };
        let is_type_element = |e: ElementId| {
            matches!(
                e.tag(),
                Tag::Dynamic | Tag::Never | Tag::TypeAlias | Tag::TypeParameter
            ) || e.is::<InterfaceElement>()
        };
        if ast.is::<ConstantPattern>(parent) {
            let name = identifier_name(ast, node).to_string();
            if self.lookup_getter(&name).is_some_and(is_type_element) {
                return self.to_type_literal(ast, node.upcast());
            }
        }
        if ast.is_in_value_expression_slot(parent, node) {
            let name = identifier_name(ast, node).to_string();
            if self.lookup_getter(&name).is_some_and(is_type_element) {
                return self.to_type_literal(ast, node.upcast());
            }
        }
        node.raw()
    }

    fn report_wrong_number_of_type_arguments_constructor(
        &mut self,
        ast: &Ast,
        type_arguments: Id<TypeArgumentList>,
        class_name: &str,
        constructor_name: &str,
    ) {
        self.diagnostics.push(
            diag::wrong_number_of_type_arguments_constructor(class_name, constructor_name)
                .at_offset(
                    ast.offset(type_arguments) as usize,
                    ast.length(type_arguments) as usize,
                )
                .into_diagnostic(),
        );
    }

    /// Dart `_instanceCreation_prefix_type_name`.
    fn instance_creation_prefix_type_name(
        &mut self,
        ast: &mut Ast,
        node: Id<MethodInvocation>,
        type_name_identifier: Id<PrefixedIdentifier>,
        constructor_identifier: Id<SimpleIdentifier>,
        class_element: EId<InterfaceElement>,
    ) -> NodeId {
        let constructor_name_text = identifier_name(ast, constructor_identifier).to_string();
        let constructor = dartr_typesystem::lookup::get_named_constructor(
            &self.ctx,
            class_element,
            &constructor_name_text,
        );
        if constructor.is_none() {
            return node.raw();
        }

        let type_arguments = ast[node].type_arguments;
        if let Some(type_arguments) = type_arguments {
            // Dart `typeNameIdentifier.toString()`: `p.C`.
            let p = &ast[type_name_identifier];
            let class_name = format!(
                "{}.{}",
                identifier_name(ast, p.prefix),
                identifier_name(ast, p.identifier)
            );
            self.report_wrong_number_of_type_arguments_constructor(
                ast,
                type_arguments,
                &class_name,
                &constructor_name_text,
            );
        }

        let p = ast[type_name_identifier].clone();
        let prefix_token = ast[p.prefix].token;
        let identifier_token = ast[p.identifier].token;
        let import_prefix = ast.add(ImportPrefixReference {
            name: prefix_token,
            period: p.period,
        });
        let type_name = ast.add(NamedType {
            import_prefix: Some(import_prefix),
            name: identifier_token,
            type_arguments,
            question: None,
        });
        let operator = ast[node].operator;
        let constructor_name = ast.add(ConstructorName {
            type_: type_name,
            period: operator,
            name: Some(constructor_identifier),
        });
        let argument_list = ast[node].argument_list;
        let instance_creation = ast.add(InstanceCreationExpression {
            keyword: None,
            constructor_name,
            type_arguments: None,
            argument_list,
        });
        ast.replace_with(node, instance_creation);
        instance_creation.raw()
    }

    /// Dart `_toConstructorReference_prefixed`.
    fn to_constructor_reference_prefixed(
        &mut self,
        ast: &mut Ast,
        node: Id<PrefixedIdentifier>,
        class_element: EId<InterfaceElement>,
    ) -> NodeId {
        let identifier = ast[node].identifier;
        let name = identifier_name(ast, identifier).to_string();
        let constructor =
            dartr_typesystem::lookup::get_named_constructor(&self.ctx, class_element, &name);
        if constructor.is_none() {
            return node.raw();
        }
        let (prefix, period) = (ast[node].prefix, ast[node].period);
        let prefix_token = ast[prefix].token;
        let type_name = ast.add(NamedType {
            import_prefix: None,
            name: prefix_token,
            type_arguments: None,
            question: None,
        });
        let constructor_name = ast.add(ConstructorName {
            type_: type_name,
            period: Some(period),
            name: Some(identifier),
        });
        let constructor_reference = ast.add(ConstructorReference { constructor_name });
        ast.replace_with(node, constructor_reference);
        constructor_reference.raw()
    }

    /// Dart `_toConstructorReference_propertyAccess`.
    fn to_constructor_reference_property_access(
        &mut self,
        ast: &mut Ast,
        node: Id<PropertyAccess>,
        receiver: Id<Identifier>,
        type_arguments: Option<Id<TypeArgumentList>>,
        class_element: EId<InterfaceElement>,
    ) -> NodeId {
        let property_name = ast[node].property_name;
        let name = identifier_name(ast, property_name).to_string();
        let constructor =
            dartr_typesystem::lookup::get_named_constructor(&self.ctx, class_element, &name);
        if constructor.is_none() && type_arguments.is_none() {
            // No constructor by this name and no type arguments: do not
            // rewrite. With type arguments (`prefix.C<int>.name`) it looks
            // more like a constructor tear-off than anything else.
            return node.raw();
        }
        let operator = ast[node].operator;
        let type_name = ast.identifier_to_named_type(receiver, type_arguments, None);
        let constructor_name = ast.add(ConstructorName {
            type_: type_name,
            period: Some(operator),
            name: Some(property_name),
        });
        let constructor_reference = ast.add(ConstructorReference { constructor_name });
        ast.replace_with(node, constructor_reference);
        constructor_reference.raw()
    }

    /// Dart `_toInstanceCreation_prefix_type`.
    fn to_instance_creation_prefix_type(
        &mut self,
        ast: &mut Ast,
        node: Id<MethodInvocation>,
        prefix_identifier: Id<SimpleIdentifier>,
        type_identifier: Id<SimpleIdentifier>,
    ) -> NodeId {
        let Some(operator) = ast[node].operator else {
            return node.raw();
        };
        let prefix_token = ast[prefix_identifier].token;
        let type_token = ast[type_identifier].token;
        let import_prefix = ast.add(ImportPrefixReference {
            name: prefix_token,
            period: operator,
        });
        let type_arguments = ast[node].type_arguments;
        let type_name = ast.add(NamedType {
            import_prefix: Some(import_prefix),
            name: type_token,
            type_arguments,
            question: None,
        });
        let constructor_name = ast.add(ConstructorName {
            type_: type_name,
            period: None,
            name: None,
        });
        let argument_list = ast[node].argument_list;
        let instance_creation = ast.add(InstanceCreationExpression {
            keyword: None,
            constructor_name,
            type_arguments: None,
            argument_list,
        });
        ast.replace_with(node, instance_creation);
        instance_creation.raw()
    }

    /// Dart `_toInstanceCreation_type`.
    fn to_instance_creation_type(
        &mut self,
        ast: &mut Ast,
        node: Id<MethodInvocation>,
        type_identifier: Id<SimpleIdentifier>,
    ) -> NodeId {
        let type_token = ast[type_identifier].token;
        let type_arguments = ast[node].type_arguments;
        let type_name = ast.add(NamedType {
            import_prefix: None,
            name: type_token,
            type_arguments,
            question: None,
        });
        let constructor_name = ast.add(ConstructorName {
            type_: type_name,
            period: None,
            name: None,
        });
        let argument_list = ast[node].argument_list;
        let instance_creation = ast.add(InstanceCreationExpression {
            keyword: None,
            constructor_name,
            type_arguments: None,
            argument_list,
        });
        ast.replace_with(node, instance_creation);
        instance_creation.raw()
    }

    /// Dart `_toInstanceCreation_type_constructor`.
    fn to_instance_creation_type_constructor(
        &mut self,
        ast: &mut Ast,
        node: Id<MethodInvocation>,
        type_identifier: Id<SimpleIdentifier>,
        constructor_identifier: Id<SimpleIdentifier>,
        class_element: EId<InterfaceElement>,
    ) -> NodeId {
        let name = identifier_name(ast, constructor_identifier).to_string();
        let constructor =
            dartr_typesystem::lookup::get_named_constructor(&self.ctx, class_element, &name);
        if constructor.is_none() {
            return node.raw();
        }

        let type_arguments = ast[node].type_arguments;
        if let Some(type_arguments) = type_arguments {
            let class_name = identifier_name(ast, type_identifier).to_string();
            self.report_wrong_number_of_type_arguments_constructor(
                ast,
                type_arguments,
                &class_name,
                &name,
            );
        }
        let type_token = ast[type_identifier].token;
        let type_name = ast.add(NamedType {
            import_prefix: None,
            name: type_token,
            type_arguments: None,
            question: None,
        });
        let operator = ast[node].operator;
        let constructor_name = ast.add(ConstructorName {
            type_: type_name,
            period: operator,
            name: Some(constructor_identifier),
        });
        let argument_list = ast[node].argument_list;
        // Dart keeps the type arguments here too ("I think we should drop
        // typeArguments below").
        let instance_creation = ast.add(InstanceCreationExpression {
            keyword: None,
            constructor_name,
            type_arguments,
            argument_list,
        });
        ast.replace_with(node, instance_creation);
        instance_creation.raw()
    }

    /// Dart `_toMethodInvocationOfAliasedTypeLiteral`.
    fn to_method_invocation_of_aliased_type_literal(
        &mut self,
        ast: &mut Ast,
        node: Id<InstanceCreationExpression>,
        element: EId<TypeAliasElement>,
    ) -> NodeId {
        let constructor_name = ast[node].constructor_name;
        let (type_node, period, name) = {
            let c = &ast[constructor_name];
            (c.type_, c.period, c.name)
        };
        // Dart `node.constructorName.name!`: there is always a name here
        // in valid parses; recover by not rewriting.
        let Some(method_name) = name else {
            return node.raw();
        };
        let t = ast[type_node].clone();
        let type_name = ast.add(NamedType {
            import_prefix: t.import_prefix,
            name: t.name,
            type_arguments: t.type_arguments,
            question: None,
        });
        if let Some(aliased) = self.aliased_type(element) {
            self.tables.annotation_type.insert(type_name, aliased);
        }
        let type_literal = ast.add(TypeLiteral { type_: type_name });
        let argument_list = ast[node].argument_list;
        let method_invocation = ast.add(MethodInvocation {
            target: Some(type_literal.upcast()),
            operator: period,
            method_name,
            type_arguments: None,
            argument_list,
        });
        ast.replace_with(node, method_invocation);
        method_invocation.raw()
    }

    /// Dart `_toMethodInvocationOfFunctionReference`.
    fn to_method_invocation_of_function_reference(
        &mut self,
        ast: &mut Ast,
        node: Id<InstanceCreationExpression>,
        function: Id<Identifier>,
    ) -> NodeId {
        let constructor_name = ast[node].constructor_name;
        let (type_node, period, constructor_id) = {
            let c = &ast[constructor_name];
            (c.type_, c.period, c.name)
        };
        let (Some(period), Some(constructor_id)) = (period, constructor_id) else {
            return node.raw();
        };
        let type_arguments = ast[type_node].type_arguments;
        let function_reference = ast.add(FunctionReference {
            function: function.upcast(),
            type_arguments,
        });
        let argument_list = ast[node].argument_list;
        let method_invocation = ast.add(MethodInvocation {
            target: Some(function_reference.upcast()),
            operator: Some(period),
            method_name: constructor_id,
            type_arguments: None,
            argument_list,
        });
        ast.replace_with(node, method_invocation);
        method_invocation.raw()
    }

    /// Dart `_toTypeLiteral` / `_toPatternTypeLiteral`.
    fn to_type_literal(&mut self, ast: &mut Ast, node: Id<Identifier>) -> NodeId {
        let type_name = ast.identifier_to_named_type(node, None, None);
        let result = ast.add(TypeLiteral { type_: type_name });
        ast.replace_with(node, result);
        result.raw()
    }
}
