// Dart source: pkg/analyzer/lib/src/dart/constant/potentially_constant.dart

//! Checks of the potentially constant expressions and the (potentially)
//! constant type expressions of the Dart specification: which sub-nodes of
//! an expression are not potentially constant
//! ([`get_not_potentially_constants`]), and whether a type annotation is a
//! constant type expression ([`is_constant_type_expression`],
//! [`is_potentially_constant_type_expression`]).
//!
//! The checks read the resolution results of the unit
//! ([`ConstCheckInput`]); they do not change them.
//!
//! # `temporaryConstConstructorElements`
//!
//! Dart marks a constructor as "const" for the time of one check with the
//! `Expando` `temporaryConstConstructorElements` (the error verifier uses
//! it to check if a non-const constructor could be const). The port keeps
//! this state in a thread-local set: [`with_temporary_const_constructor`]
//! adds the constructor, runs the check, and removes it again. A
//! thread-local set keeps the shape of [`ConstCheckInput`] and the
//! signatures the same for all callers, and the check runs on the thread
//! that marked the constructor (the library analyzer resolves one unit on
//! one thread), so no other unit can see the mark.

use std::cell::RefCell;

use dartr_ast::{
    AdjacentStrings, AsExpression, Ast, BinaryExpression, ConditionalExpression,
    ConstructorInitializer, ConstructorReference, DotShorthandConstructorInvocation,
    DotShorthandPropertyAccess, FunctionReference, GenericFunctionType, Id, Identifier, IfElement,
    InstanceCreationExpression, InterpolationExpression, IsExpression, ListLiteral,
    MapLiteralEntry, MethodInvocation, NamedArgument, NamedType, NodeId, NodeKind,
    ParenthesizedExpression, PrefixExpression, PrefixedIdentifier, PropertyAccess, RecordLiteral,
    RecordLiteralNamedField, RecordTypeAnnotation, RegularFormalParameter, SetOrMapLiteral,
    SpreadElement, StringInterpolation, TypeAnnotation, TypeArgumentList, TypeLiteral,
    TypedLiteral, VariableDeclaration,
};
use dartr_element::{
    Ctx, ElemRef, ElementId, FeatureSet, FragmentFlags, PrefixElement, PrefixFragment,
    ResolutionTables, Tag, TypeKind,
};
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_typesystem::TypeExt;
use dartr_typesystem::member;
use indexmap::IndexSet;

use crate::ast_ext;
use crate::element_ext;
use crate::tables::ResolverTables;

/// The resolved unit data a check reads.
pub struct ConstCheckInput<'a> {
    /// The element context, with the local arena of the unit.
    pub ctx: &'a Ctx<'a>,
    /// The resolved AST of the unit.
    pub ast: &'a Ast,
    /// The resolution results of the unit.
    pub tables: &'a ResolutionTables,
    /// The resolver-private node data of the unit.
    pub rt: &'a ResolverTables,
    /// The feature set of the library (Dart `featureSet`).
    pub features: &'a FeatureSet,
}

thread_local! {
    /// Dart `temporaryConstConstructorElements`: the constructors that are
    /// temporarily marked as "const" (see the module documentation).
    static TEMPORARY_CONST_CONSTRUCTOR_ELEMENTS: RefCell<IndexSet<ElementId>> =
        RefCell::new(IndexSet::new());
}

/// Dart `temporaryConstConstructorElements[constructor] = true`, then
/// [f], then `temporaryConstConstructorElements[constructor] = null`: runs
/// [f] while [constructor] is marked as "const".
pub fn with_temporary_const_constructor<R>(constructor: ElementId, f: impl FnOnce() -> R) -> R {
    struct Unmark(ElementId, bool);
    impl Drop for Unmark {
        fn drop(&mut self) {
            if self.1 {
                TEMPORARY_CONST_CONSTRUCTOR_ELEMENTS.with(|s| {
                    s.borrow_mut().shift_remove(&self.0);
                });
            }
        }
    }
    let inserted =
        TEMPORARY_CONST_CONSTRUCTOR_ELEMENTS.with(|s| s.borrow_mut().insert(constructor));
    let _unmark = Unmark(constructor, inserted);
    f()
}

/// Dart `temporaryConstConstructorElements[constructor] ?? false`.
pub fn is_temporary_const_constructor(constructor: ElementId) -> bool {
    TEMPORARY_CONST_CONSTRUCTOR_ELEMENTS.with(|s| s.borrow().contains(&constructor))
}

/// Dart `getNotPotentiallyConstants(node, featureSet:)`: checks if the
/// [node] and all its sub-nodes are potentially constant. Returns the nodes
/// that are not potentially constant.
pub fn get_not_potentially_constants(input: &ConstCheckInput<'_>, node: NodeId) -> Vec<NodeId> {
    let mut collector = Collector {
        input,
        nodes: Vec::new(),
    };
    collector.collect(node);
    collector.nodes
}

/// Dart `isConstantTypeExpression(node)`.
pub fn is_constant_type_expression(input: &ConstCheckInput<'_>, node: Id<TypeAnnotation>) -> bool {
    ConstantTypeChecker {
        input,
        potentially: false,
    }
    .check(Some(node))
}

/// Dart `isPotentiallyConstantTypeExpression(node)`.
pub fn is_potentially_constant_type_expression(
    input: &ConstCheckInput<'_>,
    node: Id<TypeAnnotation>,
) -> bool {
    ConstantTypeChecker {
        input,
        potentially: true,
    }
    .check(Some(node))
}

// ------------------------------------------------------------ element helpers

/// Dart `node.element` of a node with an element in
/// `ResolutionTables.element`, as a base element.
fn element_of(input: &ConstCheckInput<'_>, node: NodeId) -> Option<ElementId> {
    let e = *input.tables.element.get(node)?;
    Some(member::base_element(input.ctx, e))
}

/// The element reference of [node] (keeps a substituted member).
fn elem_ref_of(input: &ConstCheckInput<'_>, node: NodeId) -> Option<ElemRef> {
    input.tables.element.get(node).copied()
}

/// Dart `Identifier.element`: the element of a `SimpleIdentifier`, the
/// element of the `identifier` of a `PrefixedIdentifier`.
fn identifier_element_ref(input: &ConstCheckInput<'_>, node: Id<Identifier>) -> Option<ElemRef> {
    let ast = input.ast;
    match ast.cast::<PrefixedIdentifier>(node) {
        Some(p) => elem_ref_of(input, ast[p].identifier.raw()),
        None => elem_ref_of(input, node.raw()),
    }
}

/// Dart `PrefixElement.fragments.any((f) => f.isDeferred)` of the element
/// [e] (`false` when [e] is not a prefix).
fn is_deferred_prefix(ctx: &Ctx<'_>, e: Option<ElementId>) -> bool {
    let Some(prefix) = e.and_then(|e| e.cast::<PrefixElement>()) else {
        return false;
    };
    let mut fragment = Some(ctx.get(prefix).first_fragment());
    while let Some(f) = fragment {
        let data = ctx.fragment(f);
        if data.is_deferred {
            return true;
        }
        fragment = data.next_fragment.and_then(|n| n.cast::<PrefixFragment>());
    }
    false
}

/// Dart `PrefixedIdentifierImpl.isDeferred`.
fn prefixed_identifier_is_deferred(
    input: &ConstCheckInput<'_>,
    node: Id<PrefixedIdentifier>,
) -> bool {
    let prefix = input.ast[node].prefix;
    is_deferred_prefix(input.ctx, element_of(input, prefix.raw()))
}

/// Dart `NamedTypeImpl.isDeferred`.
fn named_type_is_deferred(input: &ConstCheckInput<'_>, node: Id<NamedType>) -> bool {
    match input.ast[node].import_prefix {
        Some(import_prefix) => {
            is_deferred_prefix(input.ctx, element_of(input, import_prefix.raw()))
        }
        None => false,
    }
}

/// Dart `element is InterfaceElement || element is TypeAliasElement`.
fn is_interface_or_type_alias(e: Option<ElementId>) -> bool {
    e.is_some_and(|e| e.is::<dartr_element::InterfaceElement>() || e.tag() == Tag::TypeAlias)
}

/// Dart `element is MethodElement && element.isStatic`.
fn is_static_method(ctx: &Ctx<'_>, e: Option<ElemRef>) -> bool {
    e.is_some_and(|e| {
        member::base_element(ctx, e).tag() == Tag::Method && member::is_static(ctx, e)
    })
}

/// Dart `GetterElement.variable.isConst` of the getter [getter].
fn getter_variable_is_const(ctx: &Ctx<'_>, getter: ElemRef) -> bool {
    match member::variable(ctx, getter) {
        Some(variable) => element_ext::is_const(ctx, member::base_element(ctx, variable)),
        None => false,
    }
}

/// Dart `TopLevelFunctionElement.isDartCoreIdentical`.
fn is_dart_core_identical(ctx: &Ctx<'_>, e: ElementId) -> bool {
    e.tag() == Tag::TopLevelFunction
        && ctx.element_name(e) == Some("identical")
        && ctx
            .element_data(e)
            .and_then(|d| d.library)
            .is_some_and(|l| ctx.tp.core_library.try_get() == Some(&l))
}

/// Dart `ConstructorElement.isConst`.
fn constructor_is_const(ctx: &Ctx<'_>, e: ElementId) -> bool {
    element_ext::first_fragment_flags(ctx, e).contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST)
}

/// Dart `_isConstantTypeName(name)`.
fn is_constant_type_name(input: &ConstCheckInput<'_>, name: Id<Identifier>) -> bool {
    let element = identifier_element_ref(input, name).map(|e| member::base_element(input.ctx, e));
    if is_interface_or_type_alias(element) {
        if let Some(p) = input.ast.cast::<PrefixedIdentifier>(name) {
            if prefixed_identifier_is_deferred(input, p) {
                return false;
            }
        }
        return true;
    }
    false
}

/// Dart `node.thisOrAncestorOfType<T>()`.
fn this_or_ancestor_of_type<T: dartr_ast::NodeType + ?Sized>(
    ast: &Ast,
    node: NodeId,
) -> Option<Id<T>> {
    let mut current = Some(node);
    while let Some(n) = current {
        if let Some(t) = ast.cast::<T>(n) {
            return Some(t);
        }
        current = ast.parent(n);
    }
    None
}

// ------------------------------------------------------------------ collector

/// Dart `_Collector`.
struct Collector<'i, 'a> {
    input: &'i ConstCheckInput<'a>,
    nodes: Vec<NodeId>,
}

impl Collector<'_, '_> {
    fn is_enabled(&self, flag: ExperimentalFlag) -> bool {
        self.input.features.is_enabled(flag.name())
    }

    /// Dart `collect(node)`.
    fn collect(&mut self, node: NodeId) {
        let ast = self.input.ast;
        let kind = ast.kind(node);
        if matches!(
            kind,
            NodeKind::BooleanLiteral
                | NodeKind::DoubleLiteral
                | NodeKind::IntegerLiteral
                | NodeKind::NullLiteral
                | NodeKind::SimpleStringLiteral
                | NodeKind::SymbolLiteral
        ) {
            return;
        }

        if let Some(n) = ast.cast::<AdjacentStrings>(node) {
            for &string in ast.list(ast[n].strings) {
                self.collect(string.raw());
            }
            return;
        }

        if let Some(n) = ast.cast::<DotShorthandPropertyAccess>(node) {
            return self.identifier(ast[n].property_name.upcast());
        }

        if let Some(n) = ast.cast::<DotShorthandConstructorInvocation>(node) {
            if !ast_ext::dot_shorthand_constructor_invocation_is_const(ast, n) {
                self.nodes.push(node);
            }
            return;
        }

        if let Some(n) = ast.cast::<StringInterpolation>(node) {
            for &component in ast.list(ast[n].elements) {
                if let Some(c) = ast.cast::<InterpolationExpression>(component) {
                    self.collect(ast[c].expression.raw());
                }
            }
            return;
        }

        if let Some(n) = ast.cast::<Identifier>(node) {
            return self.identifier(n);
        }

        if let Some(n) = ast.cast::<InstanceCreationExpression>(node) {
            if !ast_ext::instance_creation_is_const(ast, n) {
                self.nodes.push(node);
            }
            return;
        }

        if let Some(n) = ast.cast::<TypedLiteral>(node) {
            return self.typed_literal(n);
        }

        if let Some(n) = ast.cast::<ParenthesizedExpression>(node) {
            self.collect(ast[n].expression.raw());
            return;
        }

        if let Some(n) = ast.cast::<RecordLiteral>(node) {
            return self.record_literal(n);
        }

        if let Some(n) = ast.cast::<MethodInvocation>(node) {
            return self.method_invocation(n);
        }

        if let Some(n) = ast.cast::<NamedArgument>(node) {
            return self.collect(ast[n].argument_expression.raw());
        }

        if let Some(n) = ast.cast::<RecordLiteralNamedField>(node) {
            return self.collect(ast[n].field_expression.raw());
        }

        if let Some(n) = ast.cast::<BinaryExpression>(node) {
            self.collect(ast[n].left_operand.raw());
            self.collect(ast[n].right_operand.raw());
            return;
        }

        if let Some(n) = ast.cast::<PrefixExpression>(node) {
            let operator = ast.tokens.lexeme(ast[n].operator);
            // TokenType.BANG, TokenType.MINUS, TokenType.TILDE.
            if matches!(operator, "!" | "-" | "~") {
                self.collect(ast[n].operand.raw());
                return;
            }
            self.nodes.push(node);
            return;
        }

        if let Some(n) = ast.cast::<ConditionalExpression>(node) {
            self.collect(ast[n].condition.raw());
            self.collect(ast[n].then_expression.raw());
            self.collect(ast[n].else_expression.raw());
            return;
        }

        if let Some(n) = ast.cast::<PropertyAccess>(node) {
            return self.property_access(n);
        }

        if let Some(n) = ast.cast::<AsExpression>(node) {
            let (expression, type_) = (ast[n].expression, ast[n].type_);
            self.type_test_type(type_);
            self.collect(expression.raw());
            return;
        }

        if let Some(n) = ast.cast::<IsExpression>(node) {
            let (expression, type_) = (ast[n].expression, ast[n].type_);
            self.type_test_type(type_);
            self.collect(expression.raw());
            return;
        }

        if let Some(n) = ast.cast::<MapLiteralEntry>(node) {
            self.collect(ast[n].key.raw());
            self.collect(ast[n].value.raw());
            return;
        }

        if let Some(n) = ast.cast::<SpreadElement>(node) {
            self.collect(ast[n].expression.raw());
            return;
        }

        if let Some(n) = ast.cast::<IfElement>(node) {
            self.collect(ast[n].expression.raw());
            self.collect(ast[n].then_element.raw());
            if let Some(else_element) = ast[n].else_element {
                self.collect(else_element.raw());
            }
            return;
        }

        if let Some(n) = ast.cast::<ConstructorReference>(node) {
            let type_ = ast[ast[n].constructor_name].type_;
            self.type_argument_list(ast[type_].type_arguments);
            return;
        }

        if let Some(n) = ast.cast::<FunctionReference>(node) {
            self.type_argument_list(ast[n].type_arguments);
            self.collect(ast[n].function.raw());
            return;
        }

        if let Some(n) = ast.cast::<TypeLiteral>(node) {
            let type_ = ast[n].type_;
            let element = element_of(self.input, type_.raw());
            if element.is_some_and(|e| e.tag() == Tag::TypeParameter)
                && !self.is_enabled(ExperimentalFlag::ConstructorTearoffs)
            {
                self.nodes.push(node);
            }
            self.type_argument_list(ast[type_].type_arguments);
            return;
        }

        self.nodes.push(node);
    }

    /// The type check of `AsExpression` and `IsExpression` in Dart
    /// `collect(node)`.
    fn type_test_type(&mut self, type_: Id<TypeAnnotation>) {
        if self.is_enabled(ExperimentalFlag::NonNullable) {
            if !is_potentially_constant_type_expression(self.input, type_) {
                self.nodes.push(type_.raw());
            }
        } else {
            if !is_constant_type_expression(self.input, type_) {
                self.nodes.push(type_.raw());
            }
        }
    }

    /// Dart `_identifier(node)`.
    fn identifier(&mut self, node: Id<Identifier>) {
        let input = self.input;
        let ctx = input.ctx;
        let ast = input.ast;
        let element_ref = identifier_element_ref(input, node);
        let element = element_ref.map(|e| member::base_element(ctx, e));

        if let Some(p) = ast.cast::<PrefixedIdentifier>(node) {
            if prefixed_identifier_is_deferred(input, p) {
                self.nodes.push(node.raw());
                return;
            }
            let identifier = ast[p].identifier;
            if ast_ext::identifier_name(ast, identifier) == "length" {
                self.collect(ast[p].prefix.raw());
                return;
            }
            if is_static_method(ctx, element_ref) {
                if !is_constant_type_name(input, ast[p].prefix.upcast()) {
                    self.nodes.push(node.raw());
                }
                return;
            }
        }

        let Some(element) = element else {
            self.nodes.push(node.raw());
            return;
        };

        // Dart `element is FormalParameterElement`.
        if matches!(
            element.tag(),
            Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter
        ) {
            let enclosing = ctx.element_data(element).and_then(|d| d.enclosing);
            if let Some(enclosing) = enclosing
                && enclosing.tag() == Tag::Constructor
                && Collector::is_const_constructor_element(ctx, enclosing)
            {
                if this_or_ancestor_of_type::<ConstructorInitializer>(ast, node.raw()).is_some() {
                    return;
                }
                let field_element =
                    this_or_ancestor_of_type::<VariableDeclaration>(ast, node.raw())
                        .and_then(|d| input.tables.declared_fragment.get(d.raw()).copied())
                        .and_then(|f| ctx.fragment_data(f)?.element.try_get().copied());
                if let Some(field_element) = field_element
                    && field_element.tag() == Tag::Field
                    && !member::is_static(ctx, ElemRef::Base(field_element))
                    && !element_ext::is_late(ctx, field_element)
                {
                    return;
                }
            }
            self.nodes.push(node.raw());
            return;
        }

        if element.is::<dartr_element::VariableElement>() {
            if !element_ext::is_const(ctx, element) {
                self.nodes.push(node.raw());
            }
            return;
        }
        if element.tag() == Tag::Getter {
            if let Some(getter) = element_ref
                && !getter_variable_is_const(ctx, getter)
            {
                self.nodes.push(node.raw());
            }
            return;
        }
        if is_constant_type_name(input, node) {
            return;
        }
        if element.tag() == Tag::TopLevelFunction {
            return;
        }
        if is_static_method(ctx, element_ref) {
            return;
        }
        if element.tag() == Tag::TypeParameter
            && self.is_enabled(ExperimentalFlag::ConstructorTearoffs)
        {
            return;
        }
        self.nodes.push(node.raw());
    }

    /// Dart `_methodInvocation(node)`.
    fn method_invocation(&mut self, node: Id<MethodInvocation>) {
        let ast = self.input.ast;
        let arguments = ast.list(ast[ast[node].argument_list].arguments);
        if arguments.len() == 2 {
            let element = element_of(self.input, ast[node].method_name.raw());
            if let Some(element) = element
                && is_dart_core_identical(self.input.ctx, element)
            {
                self.collect(arguments[0].raw());
                self.collect(arguments[1].raw());
                return;
            }
        }
        // TODO(srawlins): collect type arguments.
        self.nodes.push(node.raw());
    }

    /// Dart `_propertyAccess(node)`.
    fn property_access(&mut self, node: Id<PropertyAccess>) {
        let input = self.input;
        let ast = input.ast;
        // CascadeExpression is not a constant, so the target is never null.
        // (Dart throws on a null target; the port reports the node.)
        let Some(target) = ast[node].target else {
            self.nodes.push(node.raw());
            return;
        };
        let property_name = ast[node].property_name;

        if ast_ext::identifier_name(ast, property_name) == "length" {
            self.collect(target.raw());
            return;
        }

        if let Some(target) = ast.cast::<PrefixedIdentifier>(target) {
            if prefixed_identifier_is_deferred(input, target) {
                self.nodes.push(node.raw());
                return;
            }

            let element_ref = elem_ref_of(input, property_name.raw());
            let element = element_ref.map(|e| member::base_element(input.ctx, e));
            if let (Some(element_ref), Some(element)) = (element_ref, element) {
                if element.tag() == Tag::Getter {
                    if !getter_variable_is_const(input.ctx, element_ref) {
                        self.nodes.push(property_name.raw());
                    }
                    return;
                } else if element.tag() == Tag::Method {
                    if !member::is_static(input.ctx, element_ref) {
                        self.nodes.push(property_name.raw());
                    }
                    return;
                }
            }
        }

        self.nodes.push(node.raw());
    }

    /// Dart `_recordLiteral(node)`.
    fn record_literal(&mut self, node: Id<RecordLiteral>) {
        let ast = self.input.ast;
        for &field in ast.list(ast[node].fields) {
            self.collect(field.raw());
        }
    }

    /// Dart `_typeArgumentList(typeArgumentList)`.
    fn type_argument_list(&mut self, type_argument_list: Option<Id<TypeArgumentList>>) {
        let ast = self.input.ast;
        if let Some(list) = type_argument_list {
            for &type_argument in ast.list(ast[list].arguments) {
                if !is_potentially_constant_type_expression(self.input, type_argument) {
                    self.nodes.push(type_argument.raw());
                }
            }
        }
    }

    /// Dart `_typedLiteral(node)`.
    fn typed_literal(&mut self, node: Id<TypedLiteral>) {
        let input = self.input;
        let ast = input.ast;
        if !ast_ext::typed_literal_is_const(ast, node.raw()) {
            self.nodes.push(node.raw());
            return;
        }

        if let Some(n) = ast.cast::<ListLiteral>(node) {
            let type_arguments = ast[n].type_arguments.map(|l| ast.list(ast[l].arguments));
            if let Some(type_arguments) = type_arguments
                && type_arguments.len() == 1
            {
                let element_type = type_arguments[0];
                if !is_potentially_constant_type_expression(input, element_type) {
                    self.nodes.push(element_type.raw());
                }
            }

            for &element in ast.list(ast[n].elements) {
                self.collect(element.raw());
            }
            return;
        }

        if let Some(n) = ast.cast::<SetOrMapLiteral>(node) {
            let type_arguments = ast[n].type_arguments.map(|l| ast.list(ast[l].arguments));
            if let Some(type_arguments) = type_arguments
                && type_arguments.len() == 1
            {
                let element_type = type_arguments[0];
                if !is_potentially_constant_type_expression(input, element_type) {
                    self.nodes.push(element_type.raw());
                }
            }

            if let Some(type_arguments) = type_arguments
                && type_arguments.len() == 2
            {
                let key_type = type_arguments[0];
                let value_type = type_arguments[1];
                if !is_constant_type_expression(input, key_type) {
                    self.nodes.push(key_type.raw());
                }
                if !is_constant_type_expression(input, value_type) {
                    self.nodes.push(value_type.raw());
                }
            }

            for &element in ast.list(ast[n].elements) {
                self.collect(element.raw());
            }
        }
    }

    /// Dart `_Collector.isConstConstructorElement(element)`.
    fn is_const_constructor_element(ctx: &Ctx<'_>, element: ElementId) -> bool {
        if constructor_is_const(ctx, element) {
            return true;
        }
        is_temporary_const_constructor(element)
    }
}

// ------------------------------------------------------ constant type checker

/// Dart `_ConstantTypeChecker`.
struct ConstantTypeChecker<'i, 'a> {
    input: &'i ConstCheckInput<'a>,
    potentially: bool,
}

impl ConstantTypeChecker<'_, '_> {
    /// Dart `check(node)`: whether the [node] is a (potentially) constant
    /// type expression.
    fn check(&self, node: Option<Id<TypeAnnotation>>) -> bool {
        let Some(node) = node else {
            return false;
        };
        let ast = self.input.ast;
        if let Some(named_type) = ast.cast::<NamedType>(node) {
            if self.potentially
                && element_of(self.input, named_type.raw())
                    .is_some_and(|e| e.tag() == Tag::TypeParameter)
            {
                return true;
            }
            return self.check_named_type(named_type);
        }
        if let Some(n) = ast.cast::<GenericFunctionType>(node) {
            return self.check_generic_function_type(n);
        }
        if let Some(n) = ast.cast::<RecordTypeAnnotation>(node) {
            return self.check_record_type_annotation(n);
        }
        false
    }

    /// Dart `_checkGenericFunctionType(node)`.
    fn check_generic_function_type(&self, node: Id<GenericFunctionType>) -> bool {
        let ast = self.input.ast;
        if let Some(return_type) = ast[node].return_type {
            if !self.check(Some(return_type)) {
                return false;
            }
        }

        if let Some(type_parameters) = ast[node].type_parameters {
            for &parameter in ast.list(ast[type_parameters].type_parameters) {
                if let Some(bound) = ast[parameter].bound
                    && !self.check(Some(bound))
                {
                    return false;
                }
            }
        }

        let formal_parameters = ast[ast[node].parameters].parameters;
        for &formal_parameter in ast.list(formal_parameters) {
            if let Some(p) = ast.cast::<RegularFormalParameter>(formal_parameter)
                && ast[p].function_typed_suffix.is_none()
            {
                if !self.check(ast[p].type_) {
                    return false;
                }
            }
        }
        true
    }

    /// Dart `_checkNamedType(node)`.
    fn check_named_type(&self, node: Id<NamedType>) -> bool {
        let ast = self.input.ast;
        if self.is_constant_named_type(node) {
            if let Some(arguments) = ast[node].type_arguments {
                if ast
                    .list(ast[arguments].arguments)
                    .iter()
                    .any(|&argument| !self.check(Some(argument)))
                {
                    return false;
                }
            }
            return true;
        }
        let Some(&type_) = self.input.tables.annotation_type.get(node.raw()) else {
            return false;
        };
        matches!(
            self.input.ctx.ty(type_),
            TypeKind::Dynamic | TypeKind::Never(_) | TypeKind::Void
        )
    }

    /// Dart `_checkRecordTypeAnnotation(node)`.
    fn check_record_type_annotation(&self, node: Id<RecordTypeAnnotation>) -> bool {
        let ast = self.input.ast;
        let positional = ast
            .list(ast[node].positional_fields)
            .iter()
            .map(|&f| ast[f].type_);
        let named: Vec<Id<TypeAnnotation>> = match ast[node].named_fields {
            Some(named) => ast
                .list(ast[named].fields)
                .iter()
                .map(|&f| ast[f].type_)
                .collect(),
            None => Vec::new(),
        };
        if positional
            .chain(named)
            .any(|type_| !self.check(Some(type_)))
        {
            return false;
        }
        true
    }

    /// Dart `NamedType.isConstantNamedType`.
    fn is_constant_named_type(&self, node: Id<NamedType>) -> bool {
        let element = element_of(self.input, node.raw());
        if is_interface_or_type_alias(element) {
            return !named_type_is_deferred(self.input, node);
        }
        false
    }
}
