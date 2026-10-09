// Dart source: pkg/analyzer/lib/src/generated/ffi_verifier.dart

//! `FfiVerifier`: the visitor that finds problems with the way the
//! `dart:ffi` APIs are used. It runs on each resolved unit right after the
//! `ErrorVerifier` (Dart `LibraryAnalyzer._computeVerifyErrors`).
//!
//! Differences to the Dart code, until the units it depends on land:
//!
//! - Annotations: the annotation resolver (unit C9) is a stub, so
//!   `annotation.element` is often missing. [`FfiVerifier::annotation_element`]
//!   reads `element` of the node and else resolves the name of the
//!   annotation from the scope lookup results (class, prefixed class,
//!   named or unnamed constructor).
//! - Constant values (`ElementAnnotation.computeConstantValue`,
//!   `Expression.computeConstantValue`): the constant evaluator (D1–D2) is
//!   not in this crate yet. The values that the verifier reads (the type of
//!   an annotation, `Native.isLeaf`, the `@Array` dimensions,
//!   `Packed.memberAlignment`) are computed from the syntax of the
//!   annotation (literals only). When a value cannot be computed, the
//!   check that needs it is skipped (no false positives). `_isConst`
//!   reports only expressions that are certainly not constant.
//! - `element.metadata` of an element in another unit (used by
//!   `Native.addressOf` and by the `.address` position check) is not
//!   available: those checks are skipped for such elements.

use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, LocatableDiagnostic, diag};
use dartr_element::FnParam;
use dartr_element::diagnostics::{non_synthetic, type_arg, type_display_string};
use dartr_element::{
    Ctx, EId, ElemRef, ElementId, FragmentFlags, FragmentId, FunctionTypeData, InterfaceElement,
    Nullability, ParameterKind, ResolutionTables, Tag, TypeId, TypeKind,
};
use dartr_syntax::TokenId;
use dartr_typesystem::{TypeExt, TypeSystem, member};
use indexmap::IndexMap;

use crate::element_ext::first_fragment_flags;
use crate::resolver::UnitContext;
use crate::tables::ResolverTables;

const ABI_SPECIFIC_INTEGER_CLASS_NAME: &str = "AbiSpecificInteger";
const ABI_SPECIFIC_INTEGER_MAPPING_CLASS_NAME: &str = "AbiSpecificIntegerMapping";
const ALLOCATE_EXTENSION_METHOD_NAME: &str = "call";
const ALLOCATOR_CLASS_NAME: &str = "Allocator";
const ALLOCATOR_EXTENSION_NAME: &str = "AllocatorAlloc";
const ARRAY_CLASS_NAME: &str = "Array";
const DART_FFI_LIBRARY_NAME: &str = "dart.ffi";
const DART_TYPED_DATA_LIBRARY_NAME: &str = "dart.typed_data";
const FINALIZABLE_CLASS_NAME: &str = "Finalizable";
const IS_LEAF_PARAM_NAME: &str = "isLeaf";
const NATIVE_ADDRESS_OF: &str = "Native.addressOf";
const NATIVE_CALLABLE: &str = "NativeCallable";
const OPAQUE_CLASS_NAME: &str = "Opaque";

const ADDRESS_OF_COMPOUND_EXTENSION_NAMES: &[&str] =
    &["ArrayAddress", "StructAddress", "UnionAddress"];
const ADDRESS_OF_PRIMITIVE_EXTENSION_NAMES: &[&str] =
    &["BoolAddress", "DoubleAddress", "IntAddress"];
const ADDRESS_OF_TYPED_DATA_EXTENSION_NAMES: &[&str] = &[
    "Float32ListAddress",
    "Float64ListAddress",
    "Int16ListAddress",
    "Int32ListAddress",
    "Int64ListAddress",
    "Int8ListAddress",
    "Uint16ListAddress",
    "Uint32ListAddress",
    "Uint64ListAddress",
    "Uint8ListAddress",
];

const PRIMITIVE_INTEGER_NATIVE_TYPES_FIXED_SIZE: &[&str] = &[
    "Int8", "Int16", "Int32", "Int64", "Uint8", "Uint16", "Uint32", "Uint64",
];
const PRIMITIVE_DOUBLE_NATIVE_TYPES: &[&str] = &["Float", "Double"];
const PRIMITIVE_BOOL_NATIVE_TYPE: &str = "Bool";
const STRUCT_CLASS_NAME: &str = "Struct";
const UNION_CLASS_NAME: &str = "Union";

/// Dart `_primitiveIntegerNativeTypes`.
fn is_primitive_integer_native_type(name: &str) -> bool {
    PRIMITIVE_INTEGER_NATIVE_TYPES_FIXED_SIZE.contains(&name) || name == "IntPtr"
}

/// Dart `_FfiTypeCheckDirection`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FfiTypeCheckDirection {
    /// Passing a value from native code to Dart code.
    NativeToDart,
    /// Passing a value from Dart code to native code.
    DartToNative,
}

impl FfiTypeCheckDirection {
    fn reverse(self) -> FfiTypeCheckDirection {
        match self {
            FfiTypeCheckDirection::NativeToDart => FfiTypeCheckDirection::DartToNative,
            FfiTypeCheckDirection::DartToNative => FfiTypeCheckDirection::NativeToDart,
        }
    }
}

/// Dart `_PrimitiveDartType`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PrimitiveDartType {
    Double,
    Int,
    Bool,
    Void,
    Handle,
    None,
}

/// The named parameters of Dart `_isValidFfiNativeType`.
#[derive(Clone, Copy, Default)]
struct NativeTypeOptions {
    allow_void: bool,
    allow_empty_struct: bool,
    allow_array: bool,
    allow_handle: bool,
    allow_opaque: bool,
}

/// A source range (Dart `SyntacticEntity` used as an error location).
#[derive(Clone, Copy)]
struct Span {
    offset: usize,
    length: usize,
}

/// Dart `FfiVerifier`.
pub struct FfiVerifier<'a> {
    pub ctx: Ctx<'a>,
    /// Dart `typeSystem`.
    pub type_system: TypeSystem<'a>,
    pub ast: &'a Ast,
    pub tables: &'a ResolutionTables,
    pub rt: &'a ResolverTables,
    /// Dart `_diagnosticReporter`.
    pub diagnostics: &'a mut Vec<Diagnostic>,
    pub unit: UnitContext<'a>,
    /// Dart `inCompound`.
    in_compound: bool,
    /// Dart `compound`.
    compound: Option<Id<ClassDeclaration>>,
    /// Dart `ffiVoidType`.
    ffi_void_type: Option<TypeId>,
    /// The metadata of the declarations of this unit, by declared fragment
    /// (Dart `element.metadata`), built on first use.
    metadata_by_fragment: IndexMap<FragmentId, NodeList<Annotation>>,
    /// The variable declarations of this unit, by declared fragment.
    variable_by_fragment: IndexMap<FragmentId, Id<VariableDeclaration>>,
}

/// Runs the FFI verifier on one resolved unit (Dart
/// `unit.accept(FfiVerifier(...))` in `_computeVerifyErrors`).
pub fn verify_unit(
    ctx: Ctx<'_>,
    ast: &Ast,
    root: Id<CompilationUnit>,
    tables: &ResolutionTables,
    rt: &ResolverTables,
    diagnostics: &mut Vec<Diagnostic>,
    unit: UnitContext<'_>,
) {
    let mut verifier = FfiVerifier {
        ctx,
        type_system: TypeSystem::new(ctx),
        ast,
        tables,
        rt,
        diagnostics,
        unit,
        in_compound: false,
        compound: None,
        ffi_void_type: None,
        metadata_by_fragment: IndexMap::new(),
        variable_by_fragment: IndexMap::new(),
    };
    verifier.collect_declarations();
    ast.accept(root, &mut verifier);
}

impl AstVisitor for FfiVerifier<'_> {
    fn visit_class_declaration(&mut self, _ast: &Ast, node: Id<ClassDeclaration>) {
        self.visit_class_declaration_(node);
    }

    fn visit_field_declaration(&mut self, ast: &Ast, node: Id<FieldDeclaration>) {
        if self.in_compound {
            self.validate_fields_in_compound(node);
        }
        let fields = ast[node].fields;
        for &declared in ast.list(ast[fields].variables) {
            if let Some(declared_element) = self.declared_element(declared.raw()) {
                self.check_ffi_native(
                    self.span_token(ast[declared].name),
                    declared_element,
                    ast[node].metadata,
                    None,
                    ast[node].external_keyword.is_some(),
                );
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_function_declaration(&mut self, ast: &Ast, node: Id<FunctionDeclaration>) {
        let function_expression = ast[node].function_expression;
        let element = self
            .declared_element(node.raw())
            .or_else(|| self.declared_element(function_expression.raw()));
        if let Some(element) = element {
            self.check_ffi_native(
                self.span_token(ast[node].name),
                element,
                ast[node].metadata,
                ast[function_expression].parameters,
                ast[node].external_keyword.is_some(),
            );
        }
        ast.visit_children(node, self);
    }

    fn visit_function_expression_invocation(
        &mut self,
        ast: &Ast,
        node: Id<FunctionExpressionInvocation>,
    ) {
        if let Some(element) = self.base_element(node.raw())
            && element.tag() == Tag::Method
        {
            let enclosing = self.enclosing_element(element);
            if self.is_ffi_extension_named(enclosing, ALLOCATOR_EXTENSION_NAME)
                && self.ctx.element_name(element) == Some(ALLOCATE_EXTENSION_METHOD_NAME)
            {
                self.validate_allocate(node);
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_index_expression(&mut self, _ast: &Ast, node: Id<IndexExpression>) {
        if let Some(element) = self.base_element(node.raw())
            && element.tag() == Tag::Method
        {
            let enclosing = self.enclosing_element(element);
            if self.is_ffi_extension_named(enclosing, "StructPointer")
                || self.is_ffi_extension_named(enclosing, "StructArray")
                || self.is_ffi_extension_named(enclosing, "UnionPointer")
                || self.is_ffi_extension_named(enclosing, "UnionArray")
            {
                if self.ctx.element_name(element) == Some("[]") {
                    self.validate_ref_indexed(node);
                }
            }
        }
        // Dart does not call `super.visitIndexExpression(node)`.
    }

    fn visit_instance_creation_expression(
        &mut self,
        ast: &Ast,
        node: Id<InstanceCreationExpression>,
    ) {
        let constructor_name = ast[node].constructor_name;
        let constructor = self.base_element(constructor_name.raw());
        let class = constructor.and_then(|c| self.enclosing_element(c));
        if self.is_struct_subclass(class) || self.is_union_subclass(class) {
            if let Some(constructor) = constructor
                && !first_fragment_flags(&self.ctx, constructor)
                    .contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY)
            {
                self.report_at(diag::creation_of_struct_or_union(), constructor_name);
            }
        } else if self.is_ffi_class_named(class, NATIVE_CALLABLE) {
            self.validate_native_callable(node);
        }
        ast.visit_children(node, self);
    }

    fn visit_library_directive(&mut self, ast: &Ast, node: Id<LibraryDirective>) {
        // Ensure there is at most one @DefaultAsset annotation per library.
        let mut has_default_asset = false;
        for &annotation in ast.list(ast[node].metadata) {
            let Some(value_type) = self.annotation_value_type(annotation) else {
                continue;
            };
            let element = self.ctx.interface_element(value_type).map(|e| e.raw());
            if self.is_ffi_class_named(element, "DefaultAsset") {
                if has_default_asset {
                    let name = ast[annotation].name;
                    self.report_at(diag::ffi_native_invalid_duplicate_default_asset(), name);
                }
                has_default_asset = true;
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_method_declaration(&mut self, ast: &Ast, node: Id<MethodDeclaration>) {
        if let Some(element) = self.declared_element(node.raw()) {
            self.check_ffi_native(
                self.span_token(ast[node].name),
                element,
                ast[node].metadata,
                ast[node].parameters,
                ast[node].external_keyword.is_some(),
            );
        }
        ast.visit_children(node, self);
    }

    fn visit_method_invocation(&mut self, ast: &Ast, node: Id<MethodInvocation>) {
        let method_name = ast[node].method_name;
        if let Some(element) = self.base_element(method_name.raw()) {
            let name = self.ctx.element_name(element);
            if element.tag() == Tag::Method {
                let enclosing = self.enclosing_element(element);
                if self.is_ffi_class_named(enclosing, "Pointer") {
                    if name == Some("fromFunction") {
                        self.validate_from_function(node);
                    } else if name == Some("elementAt") {
                        self.validate_element_at(node);
                    }
                } else if self.is_ffi_class_named(enclosing, STRUCT_CLASS_NAME)
                    || self.is_ffi_class_named(enclosing, UNION_CLASS_NAME)
                {
                    if name == Some("create") {
                        let class_name = enclosing
                            .and_then(|e| self.ctx.element_name(e))
                            .unwrap_or("")
                            .to_string();
                        self.validate_create(node, &class_name);
                    }
                } else if self.is_ffi_class_named(enclosing, "Native") {
                    if name == Some("addressOf") {
                        self.validate_native_address_of(node);
                    }
                } else if self.is_ffi_extension_named(enclosing, "NativeFunctionPointer") {
                    if name == Some("asFunction") {
                        self.validate_as_function(node);
                    }
                } else if self.is_ffi_extension_named(enclosing, "DynamicLibraryExtension") {
                    if name == Some("lookupFunction") {
                        self.validate_lookup_function(node);
                    }
                } else if self.is_ffi_extension_named(enclosing, "StructPointer")
                    || self.is_ffi_extension_named(enclosing, "UnionPointer")
                {
                    if name == Some("refWithFinalizer") {
                        self.validate_ref_with_finalizer(node);
                    }
                }
            } else if element.tag() == Tag::TopLevelFunction {
                if self.ctx.element_library_name(element) == Some(DART_FFI_LIBRARY_NAME)
                    && name == Some("sizeOf")
                {
                    self.validate_size_of(node);
                }
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_prefixed_identifier(&mut self, ast: &Ast, node: Id<PrefixedIdentifier>) {
        let element = self
            .base_element(node.raw())
            .or_else(|| self.base_element(ast[node].identifier.raw()));
        if let Some(element) = element {
            let enclosing = self.enclosing_element(element);
            let name = self.ctx.element_name(element);
            if self.is_ffi_extension_named(enclosing, "StructPointer")
                || self.is_ffi_extension_named(enclosing, "UnionPointer")
            {
                if name == Some("ref") {
                    self.validate_ref_prefixed_identifier(node);
                }
            } else if self.is_address_of_extension(enclosing) {
                if name == Some("address") {
                    self.validate_address_prefixed_identifier(node, enclosing);
                }
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_property_access(&mut self, ast: &Ast, node: Id<PropertyAccess>) {
        let property_name = ast[node].property_name;
        if let Some(element) = self.base_element(property_name.raw()) {
            let enclosing = self.enclosing_element(element);
            let name = self.ctx.element_name(element);
            if self.is_ffi_extension_named(enclosing, "StructPointer")
                || self.is_ffi_extension_named(enclosing, "UnionPointer")
            {
                if name == Some("ref") {
                    self.validate_ref_property_access(node);
                }
            } else if self.is_address_of_extension(enclosing) {
                if name == Some("address") {
                    self.validate_address_property_access(node, enclosing);
                }
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_top_level_variable_declaration(
        &mut self,
        ast: &Ast,
        node: Id<TopLevelVariableDeclaration>,
    ) {
        let variables = ast[node].variables;
        for &declared in ast.list(ast[variables].variables) {
            if let Some(declared_element) = self.declared_element(declared.raw()) {
                self.check_ffi_native(
                    self.span_token(ast[declared].name),
                    declared_element,
                    ast[node].metadata,
                    None,
                    ast[node].external_keyword.is_some(),
                );
            }
        }
        ast.visit_children(node, self);
    }
}

impl<'a> FfiVerifier<'a> {
    // ------------------------------------------------------------ reporting

    fn span_token(&self, token: TokenId) -> Span {
        let t = self.ast.tokens.get(token);
        Span {
            offset: t.offset as usize,
            length: (t.end() - t.offset) as usize,
        }
    }

    fn span_node(&self, node: impl Into<NodeId>) -> Span {
        let node = node.into();
        Span {
            offset: self.ast.offset(node) as usize,
            length: self.ast.length(node) as usize,
        }
    }

    fn report_span(&mut self, diagnostic: LocatableDiagnostic, span: Span) {
        self.diagnostics.push(
            diagnostic
                .at_offset(span.offset, span.length)
                .into_diagnostic(),
        );
    }

    fn report_at(&mut self, diagnostic: LocatableDiagnostic, node: impl Into<NodeId>) {
        let span = self.span_node(node);
        self.report_span(diagnostic, span);
    }

    fn report_at_token(&mut self, diagnostic: LocatableDiagnostic, token: TokenId) {
        let span = self.span_token(token);
        self.report_span(diagnostic, span);
    }

    fn lexeme(&self, token: TokenId) -> &'a str {
        self.ast.tokens.lexeme(token)
    }

    fn type_arg(&self, t: TypeId) -> dartr_diagnostics::TypeArg {
        type_arg(&self.ctx, t)
    }

    // ------------------------------------------------------------ nodes

    /// Dart `node.element` (the base element of a member).
    fn base_element(&self, node: NodeId) -> Option<ElementId> {
        let e = *self.tables.element.get(node)?;
        Some(member::base_element(&self.ctx, e))
    }

    /// Dart `node.declaredFragment?.element`.
    fn declared_element(&self, node: NodeId) -> Option<ElementId> {
        let fragment = *self.tables.declared_fragment.get(node)?;
        Some(*self.ctx.fragment_data(fragment)?.element.try_get()?)
    }

    /// Dart `node.staticType`.
    fn static_type(&self, node: impl Into<NodeId>) -> Option<TypeId> {
        self.tables.static_type.get(node).copied()
    }

    /// Dart `typeAnnotation.type`.
    fn annotation_type(&self, node: impl Into<NodeId>) -> Option<TypeId> {
        self.tables.annotation_type.get(node).copied()
    }

    /// Dart `node.typeArgumentTypes`.
    fn type_argument_types(&self, node: impl Into<NodeId>) -> Option<&'a [TypeId]> {
        let list = *self.tables.type_arg_types.get(node)?;
        Some(self.ctx.list(list))
    }

    /// Dart `argument.argumentExpression`.
    fn argument_expression(&self, argument: Id<Argument>) -> Id<Expression> {
        match self.ast.cast::<NamedArgument>(argument.raw()) {
            Some(named) => self.ast[named].argument_expression,
            None => Id::from_raw(argument.raw()),
        }
    }

    /// The name of the parameter that a named [argument] corresponds to
    /// (Dart `arg.correspondingParameter?.name`).
    fn named_argument_parameter_name(&self, argument: Id<NamedArgument>) -> Option<&'a str> {
        let expression = self.ast[argument].argument_expression;
        let parameter = self
            .tables
            .param_element
            .get(argument)
            .or_else(|| self.tables.param_element.get(expression));
        match parameter {
            Some(&p) => member::name(&self.ctx, p),
            // Not resolved (annotation arguments): the name of the argument.
            None => Some(self.lexeme(self.ast[argument].name)),
        }
    }

    /// Dart `classDeclaration.namePart.typeName`.
    fn class_type_name(&self, node: Id<ClassDeclaration>) -> TokenId {
        let name_part = self.ast[node].name_part.raw();
        if let Some(n) = self.ast.cast::<NameWithTypeParameters>(name_part) {
            self.ast[n].type_name
        } else if let Some(p) = self.ast.cast::<PrimaryConstructorDeclaration>(name_part) {
            self.ast[p].type_name
        } else {
            self.ast[node].class_keyword
        }
    }

    /// Dart `classDeclaration.namePart.typeParameters`.
    fn class_type_parameters(&self, node: Id<ClassDeclaration>) -> Option<Id<TypeParameterList>> {
        let name_part = self.ast[node].name_part.raw();
        if let Some(n) = self.ast.cast::<NameWithTypeParameters>(name_part) {
            self.ast[n].type_parameters
        } else if let Some(p) = self.ast.cast::<PrimaryConstructorDeclaration>(name_part) {
            self.ast[p].type_parameters
        } else {
            None
        }
    }

    /// Dart `classDeclaration.body.members`.
    fn class_members(&self, node: Id<ClassDeclaration>) -> &'a [Id<ClassMember>] {
        let body = self.ast[node].body.raw();
        match self.ast.cast::<BlockClassBody>(body) {
            Some(b) => self.ast.list(self.ast[b].members),
            None => &[],
        }
    }

    /// Dart `MethodInvocation.realTarget`.
    fn method_invocation_real_target(&self, node: Id<MethodInvocation>) -> Option<Id<Expression>> {
        crate::ast_ext::method_invocation_real_target(self.ast, node)
    }

    /// The target of a cascade section (`_ancestorCascade.target`).
    fn cascade_target(&self, node: NodeId) -> Option<Id<Expression>> {
        let mut current = self.ast.parent(node);
        while let Some(p) = current {
            if let Some(c) = self.ast.cast::<CascadeExpression>(p) {
                return Some(self.ast[c].target);
            }
            current = self.ast.parent(p);
        }
        None
    }

    /// Dart `IndexExpression.realTarget`.
    fn index_expression_real_target(&self, node: Id<IndexExpression>) -> Option<Id<Expression>> {
        match self.ast[node].target {
            Some(t) => Some(t),
            None => self.cascade_target(node.raw()),
        }
    }

    /// Dart `PropertyAccess.realTarget`.
    fn property_access_real_target(&self, node: Id<PropertyAccess>) -> Option<Id<Expression>> {
        if crate::ast_ext::property_access_is_cascaded(self.ast, node) {
            return self.cascade_target(node.raw());
        }
        self.ast[node].target
    }

    // ------------------------------------------------------------ elements

    fn enclosing_element(&self, element: ElementId) -> Option<ElementId> {
        member::enclosing_element(&self.ctx, ElemRef::Base(element))
    }

    /// Dart `InterfaceElement.isFfiClass` / `ExtensionElement.isFfiExtension`.
    fn is_ffi_element(&self, element: ElementId) -> bool {
        self.ctx.element_library_name(element) == Some(DART_FFI_LIBRARY_NAME)
    }

    /// Whether [element] is the class named [name] of `dart:ffi` (Dart
    /// `isPointer`, `isStruct`, `isNative`, ... of `Element?`).
    fn is_ffi_class_named(&self, element: Option<ElementId>, name: &str) -> bool {
        element.is_some_and(|e| {
            e.tag() == Tag::Class
                && self.ctx.element_name(e) == Some(name)
                && self.is_ffi_element(e)
        })
    }

    /// Whether [element] is the extension named [name] of `dart:ffi`.
    fn is_ffi_extension_named(&self, element: Option<ElementId>, name: &str) -> bool {
        element.is_some_and(|e| {
            e.tag() == Tag::Extension
                && self.ctx.element_name(e) == Some(name)
                && self.is_ffi_element(e)
        })
    }

    /// Dart `Element?.isAddressOfExtension`.
    fn is_address_of_extension(&self, element: Option<ElementId>) -> bool {
        element.is_some_and(|e| {
            e.tag() == Tag::Extension
                && self.is_ffi_element(e)
                && self.ctx.element_name(e).is_some_and(|n| {
                    ADDRESS_OF_COMPOUND_EXTENSION_NAMES.contains(&n)
                        || ADDRESS_OF_PRIMITIVE_EXTENSION_NAMES.contains(&n)
                        || ADDRESS_OF_TYPED_DATA_EXTENSION_NAMES.contains(&n)
                })
        })
    }

    /// Dart `Element?.ffiClass`: the class of `dart:ffi` that [element] is,
    /// or whose constructor it is.
    fn ffi_class(&self, element: Option<ElementId>) -> Option<ElementId> {
        let mut element = element?;
        if element.tag() == Tag::Constructor {
            element = self.enclosing_element(element)?;
        }
        (element.tag() == Tag::Class && self.is_ffi_element(element)).then_some(element)
    }

    /// The supertype of a class element.
    fn element_supertype(&self, element: Option<ElementId>) -> Option<TypeId> {
        let element = element?.cast::<InterfaceElement>()?;
        self.ctx.element_supertype(element)
    }

    /// Dart `isStructSubclass`.
    fn is_struct_subclass(&self, element: Option<ElementId>) -> bool {
        element.is_some_and(|e| e.tag() == Tag::Class)
            && self.type_is_ffi_class(self.element_supertype(element), STRUCT_CLASS_NAME)
    }

    /// Dart `isUnionSubclass`.
    fn is_union_subclass(&self, element: Option<ElementId>) -> bool {
        element.is_some_and(|e| e.tag() == Tag::Class)
            && self.type_is_ffi_class(self.element_supertype(element), UNION_CLASS_NAME)
    }

    /// Dart `isAbiSpecificIntegerSubclass`.
    fn is_abi_specific_integer_subclass(&self, element: Option<ElementId>) -> bool {
        element.is_some_and(|e| e.tag() == Tag::Class)
            && self.type_is_ffi_class(
                self.element_supertype(element),
                ABI_SPECIFIC_INTEGER_CLASS_NAME,
            )
    }

    /// Dart `InterfaceElement.isEmptyStruct`.
    fn is_empty_struct(&self, element: ElementId) -> bool {
        let Some(interface) = element.cast::<InterfaceElement>() else {
            return true;
        };
        for &field in &self.ctx.interface(interface).fields {
            let declared_type = member::type_(&self.ctx, ElemRef::Base(field.raw()));
            if self.ctx.is_dart_core_int(declared_type)
                || self.ctx.is_dart_core_double(declared_type)
                || self.ctx.is_dart_core_bool(declared_type)
                || self.is_pointer(declared_type)
                || self.is_compound_subtype(declared_type)
                || self.is_array(declared_type)
            {
                return false;
            }
        }
        true
    }

    /// Dart `library.getClass(name)!.thisType` for the library of
    /// [element].
    fn library_class_type(&self, element: ElementId, name: &str) -> Option<TypeId> {
        let library = self.ctx.element_data(element)?.library?;
        let class = self
            .ctx
            .get(library)
            .classes
            .iter()
            .copied()
            .find(|c| self.ctx.element_name(c.raw()) == Some(name))?;
        Some(self.ctx.interface_this_type(class.upcast()))
    }

    /// Dart `element.type` of a variable, accessor or executable.
    fn element_type(&self, element: ElementId) -> TypeId {
        member::type_(&self.ctx, ElemRef::Base(element))
    }

    fn is_static(&self, element: ElementId) -> bool {
        // Dart `TopLevelFunctionElement.isStatic` is `true`.
        element.tag() == Tag::TopLevelFunction
            || member::is_static(&self.ctx, ElemRef::Base(element))
    }

    // ------------------------------------------------------------ types

    fn interface(&self, t: TypeId) -> Option<(EId<InterfaceElement>, &'a [TypeId])> {
        match *self.ctx.ty(t) {
            TypeKind::Interface { element, .. } => Some((element, self.ctx.type_arguments(t))),
            _ => None,
        }
    }

    fn function(&self, t: TypeId) -> Option<FunctionTypeData> {
        match *self.ctx.ty(t) {
            TypeKind::Function(data) => Some(data),
            _ => None,
        }
    }

    /// Whether [t] is an interface type of the `dart:ffi` class [name].
    fn type_is_ffi_class(&self, t: Option<TypeId>, name: &str) -> bool {
        let Some(t) = t else { return false };
        match self.interface(t) {
            Some((e, _)) => {
                self.ctx.element_name(e.raw()) == Some(name) && self.is_ffi_element(e.raw())
            }
            None => false,
        }
    }

    /// Dart `DartType.isArray`.
    fn is_array(&self, t: TypeId) -> bool {
        self.type_is_ffi_class(Some(t), ARRAY_CLASS_NAME)
    }

    /// Dart `DartType.isPointer`.
    fn is_pointer(&self, t: TypeId) -> bool {
        self.interface(t)
            .is_some_and(|(e, _)| self.is_ffi_class_named(Some(e.raw()), "Pointer"))
    }

    /// Dart `DartType.isHandle`.
    fn is_handle(&self, t: TypeId) -> bool {
        self.type_is_ffi_class(Some(t), "Handle")
    }

    /// Dart `DartType.isNativeFunction`.
    fn is_native_function(&self, t: TypeId) -> bool {
        self.type_is_ffi_class(Some(t), "NativeFunction")
    }

    /// Dart `DartType.isNativeType`.
    fn is_native_type(&self, t: TypeId) -> bool {
        self.type_is_ffi_class(Some(t), "NativeType")
    }

    /// Dart `DartType.isOpaque`.
    fn is_opaque(&self, t: TypeId) -> bool {
        self.interface(t)
            .is_some_and(|(e, _)| self.is_ffi_class_named(Some(e.raw()), OPAQUE_CLASS_NAME))
    }

    /// Dart `DartType.isVarArgs`.
    fn is_var_args(&self, t: TypeId) -> bool {
        self.type_is_ffi_class(Some(t), "VarArgs")
    }

    /// Dart `DartType.isCompound`.
    fn is_compound(&self, t: TypeId) -> bool {
        self.type_is_ffi_class(Some(t), STRUCT_CLASS_NAME)
            || self.type_is_ffi_class(Some(t), UNION_CLASS_NAME)
    }

    /// The supertype of the element of the interface type [t].
    fn type_element_supertype(&self, t: TypeId) -> Option<TypeId> {
        let (e, _) = self.interface(t)?;
        self.ctx.element_supertype(e)
    }

    /// Dart `DartType.isCompoundSubtype`.
    fn is_compound_subtype(&self, t: TypeId) -> bool {
        self.type_element_supertype(t)
            .is_some_and(|s| self.is_compound(s))
    }

    /// Dart `DartType.isAbiSpecificIntegerSubtype`.
    fn is_abi_specific_integer_subtype(&self, t: TypeId) -> bool {
        self.type_is_ffi_class(
            self.type_element_supertype(t),
            ABI_SPECIFIC_INTEGER_CLASS_NAME,
        )
    }

    /// Dart `DartType.isOpaqueSubtype`.
    fn is_opaque_subtype(&self, t: TypeId) -> bool {
        self.type_element_supertype(t)
            .is_some_and(|s| self.is_opaque(s))
    }

    /// Dart `InterfaceElement.isTypedDataClass`.
    fn is_typed_data_class(&self, element: ElementId) -> bool {
        self.ctx.element_library_name(element) == Some(DART_TYPED_DATA_LIBRARY_NAME)
    }

    /// Dart `DartType.isTypedData`.
    fn is_typed_data(&self, t: TypeId) -> bool {
        let Some((e, _)) = self.interface(t) else {
            return false;
        };
        if !self.is_typed_data_class(e.raw()) {
            return false;
        }
        let Some(element_name) = self.ctx.element_name(e.raw()) else {
            return false;
        };
        if !element_name.ends_with("List") {
            return false;
        }
        if element_name == "Float32List" || element_name == "Float64List" {
            return true;
        }
        let fixed_integer_type_name = element_name.replace("List", "");
        PRIMITIVE_INTEGER_NATIVE_TYPES_FIXED_SIZE.contains(&fixed_integer_type_name.as_str())
    }

    /// Dart `DartType.arrayDimensions`.
    fn array_dimensions(&self, mut t: TypeId) -> i64 {
        let mut dimensions = 0;
        while self.is_array(t) {
            dimensions += 1;
            match self.ctx.type_arguments(t) {
                [single] => t = *single,
                _ => break,
            }
        }
        dimensions
    }

    /// Dart `NamedType.isCompoundSubtype`.
    fn named_type_is_compound_subtype(&self, element: Option<ElementId>) -> bool {
        let Some(e) = element.filter(|e| e.tag() == Tag::Class) else {
            return false;
        };
        let Some(interface) = e.cast::<InterfaceElement>() else {
            return false;
        };
        self.ctx
            .element_all_supertypes(interface)
            .iter()
            .any(|&s| self.is_compound(s))
    }

    /// Dart `NamedType.isAbiSpecificIntegerSubtype`.
    fn named_type_is_abi_specific_integer_subtype(&self, element: Option<ElementId>) -> bool {
        let Some(e) = element.filter(|e| e.tag() == Tag::Class) else {
            return false;
        };
        let Some(interface) = e.cast::<InterfaceElement>() else {
            return false;
        };
        self.ctx
            .element_all_supertypes(interface)
            .iter()
            .any(|&s| self.type_is_ffi_class(Some(s), ABI_SPECIFIC_INTEGER_CLASS_NAME))
    }

    /// Dart `FunctionType.normalParameterTypes`.
    fn normal_parameter_types(&self, f: &FunctionTypeData) -> Vec<TypeId> {
        self.ctx
            .list(f.params)
            .iter()
            .filter(|p| p.kind == ParameterKind::Required)
            .map(|p| p.ty)
            .collect()
    }

    /// Whether [f] has optional positional or named parameters.
    fn has_optional_or_named_parameters(&self, f: &FunctionTypeData) -> bool {
        self.ctx
            .list(f.params)
            .iter()
            .any(|p| p.kind != ParameterKind::Required)
    }

    /// Dart `List<TypeImpl>.flattenVarArgs()`.
    fn flatten_var_args(&self, types: Vec<TypeId>) -> Vec<TypeId> {
        let Some(&last) = types.last() else {
            return types;
        };
        if !self.is_var_args(last) {
            return types;
        }
        let [type_argument] = self.ctx.type_arguments(last) else {
            return types;
        };
        let TypeKind::Record {
            positional, named, ..
        } = *self.ctx.ty(*type_argument)
        else {
            return types;
        };
        if !named.is_empty() {
            // Don't flatten if invalid record.
            return types;
        }
        let mut result = types[..types.len() - 1].to_vec();
        result.extend_from_slice(self.ctx.list(positional));
        result
    }

    /// A function type like [f] with other [params] and [ret] (Dart
    /// `FunctionTypeImpl(typeParameters:, formalParameters:, returnType:,
    /// nullabilitySuffix:)`).
    fn function_type_with(&self, f: &FunctionTypeData, params: &[FnParam], ret: TypeId) -> TypeId {
        let type_params = self.ctx.list(f.type_params).to_vec();
        self.ctx
            .function_type(&type_params, params, ret, f.nullability, None)
    }

    // ------------------------------------------------------------ annotations

    /// Dart `annotation.element`. The annotation resolver (C9) is not
    /// ported yet: when the node has no element, the name of the
    /// annotation is resolved from the scope lookup results.
    fn annotation_element(&self, annotation: Id<Annotation>) -> Option<ElementId> {
        if let Some(e) = self.base_element(annotation.raw()) {
            return Some(e);
        }
        let ast = self.ast;
        let name = ast[annotation].name;
        let (first, second) = match ast.cast::<PrefixedIdentifier>(name.raw()) {
            Some(p) => (ast[p].prefix, Some(ast[p].identifier)),
            None => (ast.cast::<SimpleIdentifier>(name.raw())?, None),
        };
        let element1 = self.rt.scope_lookup_result.get(first)?.getter?;
        let (element, constructor_name) =
            match (element1.cast::<dartr_element::PrefixElement>(), second) {
                (Some(prefix), Some(second)) => {
                    let id = crate::ast_ext::identifier_name(ast, second);
                    let element = self
                        .unit
                        .scopes
                        .prefix_lookup(&self.ctx, prefix, id)
                        .getter?;
                    (element, ast[annotation].constructor_name)
                }
                (None, Some(second)) => (element1, Some(second)),
                _ => (element1, ast[annotation].constructor_name),
            };
        if let Some(class) = element.cast::<InterfaceElement>() {
            ast[annotation].arguments?;
            let name = match constructor_name {
                Some(n) => crate::ast_ext::identifier_name(ast, n),
                None => "new",
            };
            let constructor =
                dartr_typesystem::lookup::get_named_constructor(&self.ctx, class, name)?;
            return Some(constructor.raw());
        }
        Some(element)
    }

    /// Dart `annotation.elementAnnotation?.computeConstantValue()?.type`
    /// for an annotation that invokes a constructor (the class with the
    /// type arguments of the annotation; `dynamic` when they are omitted).
    /// `None` for other annotations (their value needs the constant
    /// evaluator).
    fn annotation_value_type(&self, annotation: Id<Annotation>) -> Option<TypeId> {
        let element = self.annotation_element(annotation)?;
        if element.tag() != Tag::Constructor {
            // A constant variable: the type of its initializer.
            let initializer = self.const_variable_initializer(element)?;
            let t = self.static_type(initializer)?;
            return self.interface(t).map(|_| t);
        }
        let class = self
            .enclosing_element(element)?
            .cast::<InterfaceElement>()?;
        let type_parameters = self.ctx.interface_type_parameters(class);
        let mut args: Vec<TypeId> = Vec::new();
        if let Some(type_arguments) = self.ast[annotation].type_arguments {
            for &argument in self.ast.list(self.ast[type_arguments].arguments) {
                args.push(self.annotation_type(argument).unwrap_or(TypeId::DYNAMIC));
            }
        }
        if args.len() != type_parameters.len() {
            args = vec![TypeId::DYNAMIC; type_parameters.len()];
        }
        Some(self.ctx.interface_type(class, &args, Nullability::None))
    }

    /// Dart `DartObject.isNative` for the value of [annotation]: the type
    /// of the value (`Native<T>`) when it is.
    fn native_annotation_type(&self, annotation: Id<Annotation>) -> Option<TypeId> {
        let t = self.annotation_value_type(annotation)?;
        let (e, _) = self.interface(t)?;
        self.is_ffi_class_named(Some(e.raw()), "Native")
            .then_some(t)
    }

    /// Dart `annotationValue.getField('isLeaf')?.toBoolValue() ?? false`.
    fn native_annotation_is_leaf(&self, annotation: Id<Annotation>) -> bool {
        let value_node = self.annotation_value_node(annotation);
        let Some(arguments) = self.invocation_arguments(value_node) else {
            return false;
        };
        for &argument in self.ast.list(arguments) {
            if let Some(named) = self.ast.cast::<NamedArgument>(argument.raw())
                && self.lexeme(self.ast[named].name) == IS_LEAF_PARAM_NAME
            {
                let expression = self.ast[named].argument_expression;
                return self.maybe_get_bool_const_value(expression).unwrap_or(false);
            }
        }
        false
    }

    /// Whether the annotation invokes a constructor of the `dart:ffi` class
    /// [class_name] (Dart `Annotation.isArray`, `isPacked`,
    /// `isAbiSpecificIntegerMapping`).
    fn annotation_is_ffi_constructor_of(
        &self,
        annotation: Id<Annotation>,
        class_name: &str,
    ) -> bool {
        let element = self.annotation_element(annotation);
        element.is_some_and(|e| e.tag() == Tag::Constructor)
            && self.ffi_class(element).is_some()
            && element
                .and_then(|e| self.enclosing_element(e))
                .and_then(|c| self.ctx.element_name(c))
                == Some(class_name)
    }

    /// The metadata of [element] when it is declared in this unit (Dart
    /// `element.metadata`); `None` when it is declared elsewhere.
    fn element_metadata(&self, element: ElementId) -> Option<NodeList<Annotation>> {
        let fragment = self.ctx.element_data(element)?.first_fragment;
        self.metadata_by_fragment.get(&fragment).copied()
    }

    /// The initializer of the constant variable [element] (or of the
    /// variable of the getter [element]) when it is declared in this unit:
    /// the expression whose value Dart computes with the constant
    /// evaluator.
    fn const_variable_initializer(&self, element: ElementId) -> Option<Id<Expression>> {
        let variable = match element.tag() {
            Tag::Getter => member::base_element(
                &self.ctx,
                member::variable(&self.ctx, ElemRef::Base(element))?,
            ),
            Tag::TopLevelVariable | Tag::Field => element,
            _ => return None,
        };
        if !crate::element_ext::is_const(&self.ctx, variable) {
            return None;
        }
        let fragment = self.ctx.element_data(variable)?.first_fragment;
        let declaration = *self.variable_by_fragment.get(&fragment)?;
        self.ast[declaration].initializer
    }

    /// The argument list of a constant constructor invocation (an
    /// annotation or the initializer of a constant variable).
    fn invocation_arguments(&self, node: NodeId) -> Option<NodeList<Argument>> {
        let ast = self.ast;
        let list = if let Some(a) = ast.cast::<Annotation>(node) {
            ast[a].arguments?
        } else {
            ast.cast::<InstanceCreationExpression>(node)
                .map(|c| ast[c].argument_list)
                .or_else(|| {
                    ast.cast::<MethodInvocation>(node)
                        .map(|m| ast[m].argument_list)
                })?
        };
        Some(ast[list].arguments)
    }

    /// The node that gives the value of [annotation]: the annotation, or
    /// the initializer of the constant variable that it references.
    fn annotation_value_node(&self, annotation: Id<Annotation>) -> NodeId {
        match self.annotation_element(annotation) {
            Some(e) if e.tag() != Tag::Constructor => match self.const_variable_initializer(e) {
                Some(init) => crate::ast_ext::un_parenthesized(self.ast, init).raw(),
                None => annotation.raw(),
            },
            _ => annotation.raw(),
        }
    }

    /// Collects the metadata and the variable declarations of every
    /// declaration of the unit, by declared fragment.
    fn collect_declarations(&mut self) {
        let ast = self.ast;
        let mut map = IndexMap::new();
        for index in 0..ast.node_count() {
            let node = NodeId::from_index(index);
            let metadata = match ast.kind(node) {
                NodeKind::FunctionDeclaration => {
                    let n = ast.cast::<FunctionDeclaration>(node).expect("kind");
                    if let Some(&f) = self
                        .tables
                        .declared_fragment
                        .get(ast[n].function_expression)
                    {
                        map.insert(f, ast[n].metadata);
                    }
                    Some(ast[n].metadata)
                }
                NodeKind::MethodDeclaration => {
                    let n = ast.cast::<MethodDeclaration>(node).expect("kind");
                    Some(ast[n].metadata)
                }
                NodeKind::VariableDeclaration => {
                    if let Some(&fragment) = self.tables.declared_fragment.get(node) {
                        self.variable_by_fragment.insert(
                            fragment,
                            ast.cast::<VariableDeclaration>(node).expect("kind"),
                        );
                    }
                    // The metadata of the enclosing field or top-level
                    // variable declaration.
                    ast.parent(node)
                        .and_then(|list| ast.parent(list))
                        .and_then(|d| {
                            if let Some(f) = ast.cast::<FieldDeclaration>(d) {
                                Some(ast[f].metadata)
                            } else {
                                ast.cast::<TopLevelVariableDeclaration>(d)
                                    .map(|t| ast[t].metadata)
                            }
                        })
                }
                _ => None,
            };
            if let Some(metadata) = metadata
                && let Some(&fragment) = self.tables.declared_fragment.get(node)
            {
                map.insert(fragment, metadata);
            }
        }
        self.metadata_by_fragment = map;
    }

    // ------------------------------------------------------------ constants

    /// Dart `_maybeGetBoolConstValue(expr)` (literals only; the value of a
    /// constant variable needs the constant evaluator).
    fn maybe_get_bool_const_value(&self, expr: Id<Expression>) -> Option<bool> {
        let expr = crate::ast_ext::un_parenthesized(self.ast, expr);
        let b = self.ast.cast::<BooleanLiteral>(expr.raw())?;
        Some(self.ast[b].value)
    }

    /// The value of an integer constant expression (literals, unary minus).
    fn int_value(&self, expr: Id<Expression>) -> Option<i64> {
        let expr = crate::ast_ext::un_parenthesized(self.ast, expr);
        if let Some(i) = self.ast.cast::<IntegerLiteral>(expr.raw()) {
            return self.ast[i].value;
        }
        if let Some(p) = self.ast.cast::<PrefixExpression>(expr.raw())
            && self.lexeme(self.ast[p].operator) == "-"
        {
            return self
                .int_value(self.ast[p].operand)
                .map(|v| v.wrapping_neg());
        }
        None
    }

    /// The integer values of the elements of a list literal.
    fn int_list_value(&self, expr: Id<Expression>) -> Option<Vec<i64>> {
        let expr = crate::ast_ext::un_parenthesized(self.ast, expr);
        let list = self.ast.cast::<ListLiteral>(expr.raw())?;
        self.ast
            .list(self.ast[list].elements)
            .iter()
            .map(|&e| self.int_value(self.ast.cast::<Expression>(e.raw())?))
            .collect()
    }

    /// Dart `_isConst(expr)` (`expr.computeConstantValue()?.value != null`):
    /// `false` only for expressions that are certainly not constant.
    fn is_const(&self, expr: Id<Expression>) -> bool {
        let ast = self.ast;
        let expr = crate::ast_ext::un_parenthesized(ast, expr);
        let node = expr.raw();
        match ast.kind(node) {
            NodeKind::MethodInvocation
            | NodeKind::FunctionExpressionInvocation
            | NodeKind::FunctionExpression
            | NodeKind::ThisExpression
            | NodeKind::SuperExpression
            | NodeKind::ThrowExpression
            | NodeKind::AssignmentExpression
            | NodeKind::AwaitExpression
            | NodeKind::CascadeExpression
            | NodeKind::IndexExpression
            | NodeKind::PostfixExpression => false,
            NodeKind::SimpleIdentifier | NodeKind::PrefixedIdentifier => {
                let Some(element) = self.base_element(node) else {
                    return true;
                };
                self.element_is_const_reference(element)
            }
            NodeKind::PropertyAccess => {
                let p = ast.cast::<PropertyAccess>(node).expect("kind");
                match self.base_element(ast[p].property_name.raw()) {
                    Some(element) => self.element_is_const_reference(element),
                    None => true,
                }
            }
            NodeKind::PrefixExpression => {
                let p = ast.cast::<PrefixExpression>(node).expect("kind");
                self.is_const(ast[p].operand)
            }
            NodeKind::BinaryExpression => {
                let b = ast.cast::<BinaryExpression>(node).expect("kind");
                self.is_const(ast[b].left_operand) && self.is_const(ast[b].right_operand)
            }
            NodeKind::ListLiteral | NodeKind::SetOrMapLiteral => {
                let constant = match ast.cast::<ListLiteral>(node) {
                    Some(l) => ast[l].const_keyword,
                    None => ast[ast.cast::<SetOrMapLiteral>(node).expect("kind")].const_keyword,
                };
                constant.is_some() || crate::ast_ext::in_constant_context(ast, node)
            }
            _ => true,
        }
    }

    /// Whether a reference to [element] can be a constant.
    fn element_is_const_reference(&self, element: ElementId) -> bool {
        match element.tag() {
            Tag::Field
            | Tag::TopLevelVariable
            | Tag::LocalVariable
            | Tag::FormalParameter
            | Tag::FieldFormalParameter
            | Tag::SuperFormalParameter
            | Tag::PatternVariable
            | Tag::BindPatternVariable
            | Tag::JoinPatternVariable => crate::element_ext::is_const(&self.ctx, element),
            Tag::Getter => match member::variable(&self.ctx, ElemRef::Base(element)) {
                Some(v) => {
                    let v = member::base_element(&self.ctx, v);
                    crate::element_ext::is_const(&self.ctx, v)
                }
                None => false,
            },
            _ => true,
        }
    }

    // ------------------------------------------------------------ visits

    /// Dart `visitClassDeclaration`.
    fn visit_class_declaration_(&mut self, node: Id<ClassDeclaration>) {
        let ast = self.ast;
        self.in_compound = false;
        self.compound = None;
        let type_name = self.class_type_name(node);
        let class_name = self.lexeme(type_name);
        let class_element = self.declared_element(node.raw());
        // Only the Allocator, Opaque and Struct class may be extended.
        if let Some(extends_clause) = ast[node].extends_clause {
            let superclass = ast[extends_clause].superclass;
            let superclass_element = self.base_element(superclass.raw());
            if let Some(ffi_class) = self.ffi_class(superclass_element) {
                let ffi_class_name = self.ctx.element_name(ffi_class).unwrap_or("<null>");
                if ffi_class_name == STRUCT_CLASS_NAME || ffi_class_name == UNION_CLASS_NAME {
                    self.in_compound = true;
                    self.compound = Some(node);
                    if class_element.is_some_and(|e| self.is_empty_struct(e)) {
                        self.report_at_token(
                            diag::empty_struct(class_name, ffi_class_name),
                            type_name,
                        );
                    }
                    if ffi_class_name == STRUCT_CLASS_NAME {
                        self.validate_packed_annotation(ast[node].metadata);
                    }
                } else if ffi_class_name == ABI_SPECIFIC_INTEGER_CLASS_NAME {
                    self.validate_abi_specific_integer_annotation(node);
                    self.validate_abi_specific_integer_mapping_annotation(
                        type_name,
                        ast[node].metadata,
                    );
                }
            } else if self.named_type_is_compound_subtype(superclass_element)
                || self.named_type_is_abi_specific_integer_subtype(superclass_element)
            {
                let superclass_name = self.lexeme(ast[superclass].name);
                self.report_at(
                    diag::subtype_of_struct_class_in_extends(class_name, superclass_name),
                    superclass,
                );
            }
        }

        // No classes from the FFI may be explicitly implemented.
        let check_supertype =
            |this: &mut Self,
             typename: Id<NamedType>,
             code: fn(&str, &str) -> LocatableDiagnostic| {
                let element = this.base_element(typename.raw());
                let super_name = element.and_then(|e| this.ctx.element_name(e));
                if super_name == Some(ALLOCATOR_CLASS_NAME)
                    || super_name == Some(FINALIZABLE_CLASS_NAME)
                {
                    return;
                }
                if this.named_type_is_compound_subtype(element)
                    || this.named_type_is_abi_specific_integer_subtype(element)
                {
                    let superclass_name = this.lexeme(ast[typename].name);
                    this.report_at(code(class_name, superclass_name), typename);
                }
            };
        if let Some(implements_clause) = ast[node].implements_clause {
            for &t in ast.list(ast[implements_clause].interfaces) {
                check_supertype(self, t, diag::subtype_of_struct_class_in_implements);
            }
        }
        if let Some(with_clause) = ast[node].with_clause {
            for &t in ast.list(ast[with_clause].mixin_types) {
                check_supertype(self, t, diag::subtype_of_struct_class_in_with);
            }
        }

        if self.in_compound {
            if let Some(class) = class_element.and_then(|e| e.cast::<InterfaceElement>()) {
                if !self.ctx.interface_type_parameters(class).is_empty() {
                    self.report_at_token(diag::generic_struct_subclass(class_name), type_name);
                }
                if ast[node].implements_clause.is_some() {
                    let compound_type = self.ctx.interface_this_type(class);
                    if let Some(struct_type) = self.ctx.element_supertype(class)
                        && let Some((struct_element, _)) = self.interface(struct_type)
                        && let Some(finalizable_type) =
                            self.library_class_type(struct_element.raw(), FINALIZABLE_CLASS_NAME)
                        && self
                            .type_system
                            .is_subtype_of(compound_type, finalizable_type)
                    {
                        self.report_at_token(
                            diag::compound_implements_finalizable(class_name),
                            type_name,
                        );
                    }
                }
            }
        }
        ast.visit_children(node, self);
        self.in_compound = false;
    }

    // ------------------------------------------------------------ natives

    /// Dart `_checkFfiNative`.
    fn check_ffi_native(
        &mut self,
        error_node: Span,
        declaration_element: ElementId,
        metadata: NodeList<Annotation>,
        formal_parameter_list: Option<Id<FormalParameterList>>,
        is_external: bool,
    ) {
        let ast = self.ast;
        let formal_parameters: Vec<Id<FormalParameter>> = match formal_parameter_list {
            Some(list) => ast.list(ast[list].parameters).to_vec(),
            None => Vec::new(),
        };
        let mut had_native_annotation = false;

        for &annotation in ast.list(metadata) {
            let Some(annotation_type) = self.native_annotation_type(annotation) else {
                continue;
            };

            if had_native_annotation {
                let name = ast[annotation].name;
                self.report_at(diag::ffi_native_invalid_multiple_annotations(), name);
                break;
            }

            had_native_annotation = true;

            if !is_external {
                self.report_span(diag::ffi_native_must_be_external(), error_node);
            }

            // The T in @Native<T>.
            let ffi_signature = self
                .ctx
                .type_arguments(annotation_type)
                .first()
                .copied()
                .unwrap_or(TypeId::DYNAMIC);

            if self.function(ffi_signature).is_some() {
                if declaration_element.is::<dartr_element::ExecutableElement>() {
                    self.check_ffi_native_function(
                        error_node,
                        declaration_element,
                        ffi_signature,
                        annotation,
                        annotation_type,
                        &formal_parameters,
                    );
                } else {
                    // Field annotated with a function type, that can't work.
                    let d = diag::native_field_invalid_type(self.type_arg(ffi_signature));
                    self.report_span(d, error_node);
                }
            } else if matches!(
                declaration_element.tag(),
                Tag::TopLevelFunction | Tag::Method
            ) {
                let mut dart_signature = self.element_type(declaration_element);

                if self.is_static(declaration_element) && ffi_signature == TypeId::DYNAMIC {
                    // No type argument was given on the @Native annotation,
                    // so we try to infer the native type from the Dart
                    // signature.
                    if let Some(f) = self.function(dart_signature)
                        && f.ret == TypeId::VOID
                    {
                        // The Dart signature has a `void` return type, so we
                        // create a new `FunctionType` with FFI's `Void` as
                        // the return type.
                        let void_type = match self.ffi_void_type {
                            Some(t) => Some(t),
                            None => {
                                let native = self.interface(annotation_type).map(|(e, _)| e.raw());
                                let t = native.and_then(|e| self.library_class_type(e, "Void"));
                                self.ffi_void_type = t;
                                t
                            }
                        };
                        if let Some(void_type) = void_type {
                            let params = self.ctx.list(f.params).to_vec();
                            dart_signature = self.function_type_with(&f, &params, void_type);
                        }
                    }
                    self.check_ffi_native_function(
                        error_node,
                        declaration_element,
                        dart_signature,
                        annotation,
                        annotation_type,
                        &formal_parameters,
                    );
                    return;
                }

                // Function annotated with something that isn't a function
                // type.
                let d =
                    diag::must_be_a_native_function_type(self.type_arg(ffi_signature), "Native");
                self.report_span(d, error_node);
            } else {
                self.check_ffi_native_field(
                    error_node,
                    declaration_element,
                    metadata,
                    ffi_signature,
                    false,
                );
            }
        }
    }

    /// Dart `_checkFfiNativeField`.
    fn check_ffi_native_field(
        &mut self,
        error_token: Span,
        declaration_element: ElementId,
        metadata: NodeList<Annotation>,
        mut ffi_signature: TypeId,
        allow_variable_length: bool,
    ) {
        let type_ = match declaration_element.tag() {
            Tag::Field => {
                if !self.is_static(declaration_element) {
                    self.report_span(diag::native_field_not_static(), error_token);
                }
                self.element_type(declaration_element)
            }
            Tag::TopLevelVariable => self.element_type(declaration_element),
            Tag::Getter | Tag::Setter => {
                match member::variable(&self.ctx, ElemRef::Base(declaration_element)) {
                    Some(v) => member::type_(&self.ctx, v),
                    None => return,
                }
            }
            _ => {
                self.report_span(diag::native_field_not_static(), error_token);
                return;
            }
        };

        if ffi_signature == TypeId::DYNAMIC {
            // Attempt to infer the native type from the Dart type.
            match self.canonical_ffi_type_for_dart_type(type_) {
                None => {
                    self.report_span(diag::native_field_missing_type(), error_token);
                    return;
                }
                Some(canonical) => ffi_signature = canonical,
            }
        }

        if !self.validate_compatible_native_type(
            FfiTypeCheckDirection::NativeToDart,
            type_,
            ffi_signature,
            false,
            // Functions are not allowed in native fields, but allowing them
            // in the subtype check allows reporting the more-specific
            // diagnostic for the invalid field type.
            true,
        ) {
            let d = diag::must_be_a_subtype(
                self.type_arg(type_),
                self.type_arg(ffi_signature),
                "Native",
            );
            self.report_span(d, error_token);
        } else if self.is_array(ffi_signature) {
            // Array fields need an `@Array` size annotation.
            let dimensions = self.array_dimensions(ffi_signature);
            self.validate_size_of_annotation(
                error_token,
                metadata,
                dimensions,
                allow_variable_length,
            );
        } else if self.is_handle(ffi_signature) || self.is_native_function(ffi_signature) {
            let d = diag::native_field_invalid_type(self.type_arg(ffi_signature));
            self.report_span(d, error_token);
        }
    }

    /// Dart `_canonicalFfiTypeForDartType`.
    fn canonical_ffi_type_for_dart_type(&self, dart_type: TypeId) -> Option<TypeId> {
        (self.is_pointer(dart_type)
            || self.is_compound_subtype(dart_type)
            || self.is_array(dart_type))
        .then_some(dart_type)
    }

    /// Dart `_checkFfiNativeFunction`.
    fn check_ffi_native_function(
        &mut self,
        error_token: Span,
        declaration_element: ElementId,
        ffi_signature: TypeId,
        annotation: Id<Annotation>,
        annotation_type: TypeId,
        formal_parameters: &[Id<FormalParameter>],
    ) {
        let Some(ffi) = self.function(ffi_signature) else {
            return;
        };
        // Leaf call FFI Natives can't use Handles.
        if self.native_annotation_is_leaf(annotation) {
            self.validate_ffi_leaf_call_uses_no_handles(ffi_signature, error_token);
        }

        let ffi_normal_parameter_types = self.normal_parameter_types(&ffi);
        let mut ffi_parameter_types = self.flatten_var_args(ffi_normal_parameter_types.clone());
        let mut ffi_parameters: Vec<FnParam> = self.ctx.list(ffi.params).to_vec();
        let mut dart_type = self.element_type(declaration_element);
        let enclosing_element = self.enclosing_element(declaration_element);
        let is_instance_member = matches!(
            declaration_element.tag(),
            Tag::Method | Tag::Getter | Tag::Setter
        ) && !self.is_static(declaration_element);

        let enclosing_tag = enclosing_element.map(|e| e.tag());
        let is_extension_like_instance_member = is_instance_member
            && matches!(enclosing_tag, Some(Tag::Extension | Tag::ExtensionType));

        let parameter_count = formal_parameters.len() as i64;
        if is_extension_like_instance_member {
            // Extension members require a receiver argument in the Native
            // annotation.
            if parameter_count + 1 != ffi_parameter_types.len() as i64 {
                let d = diag::ffi_native_unexpected_number_of_parameters(
                    parameter_count + 1,
                    ffi_parameter_types.len() as i64,
                );
                self.report_span(d, error_token);
                return;
            }

            let receiver_type = match enclosing_element {
                Some(e) if e.tag() == Tag::Extension => {
                    let extension = e.cast::<dartr_element::ExtensionElement>();
                    extension.and_then(|x| self.ctx.get(x).extended_type.get())
                }
                Some(e) if e.tag() == Tag::ExtensionType => e
                    .cast::<InterfaceElement>()
                    .map(|i| self.ctx.interface_this_type(i)),
                _ => None,
            };

            // Keep the receiver pointer restriction diagnostic for
            // extensions.
            if ffi_normal_parameter_types
                .first()
                .is_some_and(|&t| self.is_pointer(t))
                && !receiver_type.is_some_and(|r| {
                    self.interface(r).is_some() && self.extends_native_field_wrapper_class1(Some(r))
                })
            {
                self.report_span(
                    diag::ffi_native_only_classes_extending_nativefieldwrapperclass1_can_be_pointer(
                    ),
                    error_token,
                );
                return;
            }

            let Some(receiver_type) = receiver_type else {
                return;
            };

            // Include receiver when validating the full function type.
            if let Some(d) = self.function(dart_type) {
                let mut params = vec![FnParam {
                    name: None,
                    kind: ParameterKind::Required,
                    ty: receiver_type,
                    covariant: false,
                    element: None,
                }];
                params.extend_from_slice(self.ctx.list(d.params));
                dart_type = self.function_type_with(&d, &params, d.ret);
            }

            // Explicit parameters in the declaration start after the
            // receiver. Note that ffiParameters is intentionally not sliced
            // for extensions, because dartType above is augmented with a
            // synthetic receiver.
            ffi_parameter_types.remove(0);
        } else if is_instance_member {
            // Instance methods must have the receiver as an extra parameter
            // in the Native annotation.
            if parameter_count + 1 != ffi_parameter_types.len() as i64 {
                let d = diag::ffi_native_unexpected_number_of_parameters_with_receiver(
                    parameter_count + 1,
                    ffi_parameter_types.len() as i64,
                );
                self.report_span(d, error_token);
                return;
            }

            // Receiver can only be Pointer if the class extends
            // NativeFieldWrapperClass1.
            if ffi_normal_parameter_types
                .first()
                .is_some_and(|&t| self.is_pointer(t))
            {
                let receiver_type = match enclosing_element {
                    Some(e) => match e.cast::<InterfaceElement>() {
                        Some(i) => Some(self.ctx.interface_this_type(i)),
                        None => e
                            .cast::<dartr_element::ExtensionElement>()
                            .and_then(|x| self.ctx.get(x).extended_type.get())
                            .filter(|&t| self.interface(t).is_some()),
                    },
                    None => None,
                };
                if !self.extends_native_field_wrapper_class1(receiver_type) {
                    self.report_span(
                        diag::ffi_native_only_classes_extending_nativefieldwrapperclass1_can_be_pointer(),
                        error_token,
                    );
                }
            }

            ffi_parameter_types.remove(0);
            if !ffi_parameters.is_empty() {
                ffi_parameters.remove(0);
            }
        } else {
            // Number of parameters in the Native annotation must match the
            // annotated declaration.
            if parameter_count != ffi_parameter_types.len() as i64 {
                let d = diag::ffi_native_unexpected_number_of_parameters(
                    ffi_parameter_types.len() as i64,
                    parameter_count,
                );
                self.report_span(d, error_token);
                return;
            }
        }

        // Arguments can only be Pointer if the class extends Pointer or
        // NativeFieldWrapperClass1.
        for (i, &parameter) in formal_parameters.iter().enumerate() {
            if ffi_parameter_types
                .get(i)
                .is_some_and(|&t| self.is_pointer(t))
            {
                let Some(element) = self.declared_element(parameter.raw()) else {
                    continue;
                };
                let type_ = self.element_type(element);
                if self.interface(type_).is_none()
                    || (!self.is_pointer(type_)
                        && !self.extends_native_field_wrapper_class1(Some(type_))
                        && !self.is_typed_data(type_))
                {
                    self.report_span(
                        diag::ffi_native_only_classes_extending_nativefieldwrapperclass1_can_be_pointer(),
                        error_token,
                    );
                }
            }
        }

        let native_type = self.function_type_with(&ffi, &ffi_parameters, ffi.ret);
        if !self.is_valid_ffi_native_function_type(native_type) {
            let native_type_is_omitted = self
                .ctx
                .type_arguments(annotation_type)
                .first()
                .is_none_or(|&t| t == TypeId::DYNAMIC);
            if native_type_is_omitted {
                self.report_span(diag::native_function_missing_type(), error_token);
            } else {
                let d = diag::must_be_a_native_function_type(self.type_arg(native_type), "Native");
                self.report_span(d, error_token);
            }
            return;
        }
        if !self.validate_compatible_function_types(
            FfiTypeCheckDirection::NativeToDart,
            dart_type,
            native_type,
            true,
        ) {
            let d = diag::must_be_a_subtype(
                self.type_arg(native_type),
                self.type_arg(dart_type),
                "Native",
            );
            self.report_span(d, error_token);
        }
    }

    /// Dart `_extendsNativeFieldWrapperClass1`.
    fn extends_native_field_wrapper_class1(&self, mut t: Option<TypeId>) -> bool {
        let mut steps = 0;
        while let Some(current) = t {
            if type_display_string(&self.ctx, current, true) == "NativeFieldWrapperClass1" {
                return true;
            }
            steps += 1;
            if steps > 1000 {
                return false;
            }
            t = self.type_element_supertype(current);
        }
        false
    }

    /// Dart `_isLeaf(args)`.
    fn is_leaf(&self, args: NodeList<Argument>) -> bool {
        for &arg in self.ast.list(args) {
            let Some(named) = self.ast.cast::<NamedArgument>(arg.raw()) else {
                continue;
            };
            if self.named_argument_parameter_name(named) != Some(IS_LEAF_PARAM_NAME) {
                continue;
            }
            let expression = self.ast[named].argument_expression;
            return self.maybe_get_bool_const_value(expression).unwrap_or(false);
        }
        false
    }

    /// Dart `_isSized(nativeType)`.
    fn is_sized(&self, native_type: TypeId) -> bool {
        match self.primitive_native_type(native_type) {
            PrimitiveDartType::Double | PrimitiveDartType::Int | PrimitiveDartType::Bool => {
                return true;
            }
            PrimitiveDartType::Void | PrimitiveDartType::Handle => return false,
            PrimitiveDartType::None => {}
        }
        self.is_compound_subtype(native_type)
            || self.is_pointer(native_type)
            || self.is_array(native_type)
            || self.is_abi_specific_integer_subtype(native_type)
    }

    /// Dart `_isValidFfiNativeFunctionType(nativeType)`.
    fn is_valid_ffi_native_function_type(&self, native_type: TypeId) -> bool {
        let Some(f) = self.function(native_type) else {
            return false;
        };
        if self.has_optional_or_named_parameters(&f) {
            return false;
        }
        if !self.is_valid_ffi_native_type(
            Some(f.ret),
            NativeTypeOptions {
                allow_void: true,
                allow_handle: true,
                ..Default::default()
            },
        ) {
            return false;
        }
        let parameter_types = self.flatten_var_args(self.normal_parameter_types(&f));
        for type_arg in parameter_types {
            if !self.is_valid_ffi_native_type(
                Some(type_arg),
                NativeTypeOptions {
                    allow_handle: true,
                    ..Default::default()
                },
            ) {
                return false;
            }
        }
        true
    }

    /// Dart `_isValidFfiNativeType(nativeType, ...)`.
    fn is_valid_ffi_native_type(
        &self,
        native_type: Option<TypeId>,
        options: NativeTypeOptions,
    ) -> bool {
        let Some(native_type) = native_type else {
            return false;
        };
        if let Some((element, args)) = self.interface(native_type) {
            match self.primitive_native_type(native_type) {
                PrimitiveDartType::Void => return options.allow_void,
                PrimitiveDartType::Handle => return options.allow_handle,
                PrimitiveDartType::Double | PrimitiveDartType::Int | PrimitiveDartType::Bool => {
                    return true;
                }
                PrimitiveDartType::None => {}
            }
            if self.is_native_function(native_type) {
                return match args {
                    [single] => self.is_valid_ffi_native_function_type(*single),
                    _ => false,
                };
            }
            if self.is_pointer(native_type) {
                let [native_argument_type] = args else {
                    return false;
                };
                let native_argument_type = *native_argument_type;
                return self.is_valid_ffi_native_type(
                    Some(native_argument_type),
                    NativeTypeOptions {
                        allow_void: true,
                        allow_empty_struct: true,
                        allow_handle: true,
                        allow_opaque: true,
                        allow_array: false,
                    },
                ) || self.is_compound_subtype(native_argument_type)
                    || self.is_native_type(native_argument_type);
            }
            if self.is_compound_subtype(native_type) {
                if !options.allow_empty_struct && self.is_empty_struct(element.raw()) {
                    // TODO(dacoharkes): This results in an error message not
                    // mentioning empty structs at all.
                    return false;
                }
                return true;
            }
            if self.is_opaque(native_type) {
                return options.allow_opaque;
            }
            if self.is_opaque_subtype(native_type) {
                return true;
            }
            if self.is_abi_specific_integer_subtype(native_type) {
                return true;
            }
            if options.allow_array && self.is_array(native_type) {
                return match args {
                    [single] => {
                        self.is_valid_ffi_native_type(Some(*single), NativeTypeOptions::default())
                    }
                    _ => false,
                };
            }
        } else if self.function(native_type).is_some() {
            return self.is_valid_ffi_native_function_type(native_type);
        }
        false
    }

    /// Dart `_isValidTypedData(nativeType, dartType)`.
    fn is_valid_typed_data(&self, native_type: TypeId, dart_type: TypeId) -> bool {
        if !self.is_pointer(native_type) {
            return false;
        }
        let [element_type] = self.ctx.type_arguments(native_type) else {
            return false;
        };
        let element_name = self
            .ctx
            .type_element(*element_type)
            .and_then(|e| self.ctx.element_name(e));
        let Some((dart_element, _)) = self.interface(dart_type) else {
            return false;
        };
        if self.is_typed_data_class(dart_element.raw()) {
            let dart_name = self.ctx.element_name(dart_element.raw()).unwrap_or("");
            if element_name == Some("Float") && dart_name == "Float32List" {
                return true;
            }
            if element_name == Some("Double") && dart_name == "Float64List" {
                return true;
            }
            if let Some(element_name) = element_name
                && PRIMITIVE_INTEGER_NATIVE_TYPES_FIXED_SIZE.contains(&element_name)
                && dart_name == format!("{element_name}List")
            {
                return true;
            }
        }
        false
    }

    /// Dart `_primitiveNativeType(nativeType)`.
    fn primitive_native_type(&self, native_type: TypeId) -> PrimitiveDartType {
        if let Some((element, _)) = self.interface(native_type)
            && element.raw().tag() == Tag::Class
            && self.is_ffi_element(element.raw())
        {
            let name = self.ctx.element_name(element.raw()).unwrap_or("");
            if is_primitive_integer_native_type(name) {
                return PrimitiveDartType::Int;
            }
            if PRIMITIVE_DOUBLE_NATIVE_TYPES.contains(&name) {
                return PrimitiveDartType::Double;
            }
            if name == PRIMITIVE_BOOL_NATIVE_TYPE {
                return PrimitiveDartType::Bool;
            }
            if name == "Void" {
                return PrimitiveDartType::Void;
            }
            if name == "Handle" {
                return PrimitiveDartType::Handle;
            }
        }
        PrimitiveDartType::None
    }

    /// Dart `_typeForAnnotation(annotation)`.
    fn type_for_annotation(&self, annotation: Id<Annotation>) -> PrimitiveDartType {
        let Some(element) = self.annotation_element(annotation) else {
            return PrimitiveDartType::None;
        };
        if element.tag() == Tag::Constructor {
            let name = self
                .enclosing_element(element)
                .and_then(|c| self.ctx.element_name(c))
                .unwrap_or("");
            if is_primitive_integer_native_type(name) {
                return PrimitiveDartType::Int;
            } else if PRIMITIVE_DOUBLE_NATIVE_TYPES.contains(&name) {
                return PrimitiveDartType::Double;
            } else if PRIMITIVE_BOOL_NATIVE_TYPE == name {
                return PrimitiveDartType::Bool;
            }
            let return_type = member::return_type(&self.ctx, ElemRef::Base(element));
            if self.is_abi_specific_integer_subtype(return_type) {
                return PrimitiveDartType::Int;
            }
        }
        PrimitiveDartType::None
    }

    // ------------------------------------------------------------ validation

    /// Dart `_validateAbiSpecificIntegerAnnotation`.
    fn validate_abi_specific_integer_annotation(&mut self, node: Id<ClassDeclaration>) {
        let ast = self.ast;
        let type_parameter_count = self
            .class_type_parameters(node)
            .map(|l| ast[l].type_parameters.len())
            .unwrap_or(0);
        let members = self.class_members(node);
        let valid = type_parameter_count == 0
            && members.len() == 1
            && ast
                .cast::<ConstructorDeclaration>(members[0].raw())
                .is_some_and(|c| ast[c].const_keyword.is_some());
        if !valid {
            let type_name = self.class_type_name(node);
            self.report_at_token(diag::abi_specific_integer_invalid(), type_name);
        }
    }

    /// Dart `_validateAbiSpecificIntegerMappingAnnotation`.
    fn validate_abi_specific_integer_mapping_annotation(
        &mut self,
        error_token: TokenId,
        annotations: NodeList<Annotation>,
    ) {
        let ast = self.ast;
        let ffi_mapping_annotations: Vec<Id<Annotation>> = ast
            .list(annotations)
            .iter()
            .copied()
            .filter(|&a| {
                self.annotation_is_ffi_constructor_of(a, ABI_SPECIFIC_INTEGER_MAPPING_CLASS_NAME)
            })
            .collect();

        if ffi_mapping_annotations.is_empty() {
            self.report_at_token(diag::abi_specific_integer_mapping_missing(), error_token);
            return;
        }

        for &annotation in ffi_mapping_annotations.iter().skip(1) {
            let name = ast[annotation].name;
            self.report_at(diag::abi_specific_integer_mapping_extra(), name);
        }

        let annotation = ffi_mapping_annotations[0];
        let Some(arguments) = ast[annotation].arguments else {
            return;
        };

        for &argument in ast.list(ast[arguments].arguments) {
            if let Some(literal) = ast.cast::<SetOrMapLiteral>(argument.raw()) {
                for &element in ast.list(ast[literal].elements) {
                    if let Some(entry) = ast.cast::<MapLiteralEntry>(element.raw()) {
                        let value = ast[entry].value;
                        if let Some(name) = self.expression_interface_name(value)
                            && !PRIMITIVE_INTEGER_NATIVE_TYPES_FIXED_SIZE.contains(&name)
                        {
                            self.report_at(
                                diag::abi_specific_integer_mapping_unsupported(name),
                                value,
                            );
                        }
                    }
                }
                return;
            }
        }
        // Dart reads the `mapping` field of the constant value of the
        // annotation. Without the constant evaluator: a mapping given by a
        // constant variable of this unit whose initializer is a map
        // literal (other forms are not checked).
        let Some(&first) = ast.list(ast[arguments].arguments).first() else {
            return;
        };
        let first_expression = self.argument_expression(first);
        let Some(element) = self.base_element(first_expression.raw()).or_else(|| {
            ast.cast::<SimpleIdentifier>(first_expression.raw())
                .and_then(|i| self.rt.scope_lookup_result.get(i)?.getter)
        }) else {
            return;
        };
        let Some(initializer) = self.const_variable_initializer(element) else {
            return;
        };
        let initializer = crate::ast_ext::un_parenthesized(ast, initializer);
        let Some(literal) = ast.cast::<SetOrMapLiteral>(initializer.raw()) else {
            return;
        };
        for &element in ast.list(ast[literal].elements) {
            if let Some(entry) = ast.cast::<MapLiteralEntry>(element.raw())
                && let Some(native_type_name) = self.expression_interface_name(ast[entry].value)
                && !PRIMITIVE_INTEGER_NATIVE_TYPES_FIXED_SIZE.contains(&native_type_name)
            {
                self.report_at(
                    diag::abi_specific_integer_mapping_unsupported(native_type_name),
                    first,
                );
            }
        }
    }

    /// The name of the class of the static type of [expr] (Dart
    /// `expr.staticType` when it is an `InterfaceType`). Annotation
    /// arguments are not resolved yet (C9): a constructor invocation is
    /// resolved from the scope lookup results.
    fn expression_interface_name(&self, expr: Id<Expression>) -> Option<&'a str> {
        if let Some(t) = self.static_type(expr) {
            let (e, _) = self.interface(t)?;
            return self.ctx.element_name(e.raw());
        }
        let ast = self.ast;
        let class = if let Some(m) = ast.cast::<MethodInvocation>(expr.raw()) {
            if ast[m].target.is_some() {
                return None;
            }
            self.rt
                .scope_lookup_result
                .get(ast[m].method_name)?
                .getter?
        } else {
            let c = ast.cast::<InstanceCreationExpression>(expr.raw())?;
            let named_type = ast[ast[c].constructor_name].type_;
            self.base_element(named_type.raw())?
        };
        class.cast::<InterfaceElement>()?;
        self.ctx.element_name(class)
    }

    /// Dart `_validateAddressPosition(node, errorNode)`.
    fn validate_address_position(&mut self, node: NodeId, error_node: NodeId) {
        let ast = self.ast;
        let mut parent = ast.parent(node);
        // Since we are allowing .address.cast(), we need to traverse up one
        // level to get the ffi Invocation (.cast() nested down one level the
        // expression).
        if let Some(p) = parent
            && let Some(invocation) = ast.cast::<MethodInvocation>(p)
        {
            let method_name = ast[invocation].method_name;
            if let Some(element) = self.base_element(method_name.raw())
                && element.tag() == Tag::Method
                && crate::ast_ext::identifier_name(ast, method_name) == "cast"
                && self.is_ffi_class_named(self.enclosing_element(element), "Pointer")
            {
                parent = ast.parent(p);
            }
        }
        let grand_parent = parent.and_then(|p| ast.parent(p));
        let valid = match (parent, grand_parent) {
            (Some(p), Some(g)) if ast.is::<ArgumentList>(p) => {
                match ast.cast::<MethodInvocation>(g) {
                    // Unknown (the callee is declared in another unit): no
                    // diagnostic.
                    Some(invocation) => self.is_native_leaf_invocation(invocation).unwrap_or(true),
                    None => false,
                }
            }
            _ => false,
        };
        if !valid {
            self.report_at(diag::address_position(), error_node);
        }
    }

    /// Dart `MethodInvocation.isNativeLeafInvocation` (`None` when the
    /// metadata of the invoked element is not available).
    fn is_native_leaf_invocation(&self, node: Id<MethodInvocation>) -> Option<bool> {
        let method_name = self.ast[node].method_name;
        let Some(element) = self.base_element(method_name.raw()) else {
            return Some(false);
        };
        if !matches!(element.tag(), Tag::TopLevelFunction | Tag::Method) {
            return Some(false);
        }
        let metadata = self.element_metadata(element)?;
        Some(self.ast.list(metadata).iter().any(|&annotation| {
            self.native_annotation_type(annotation).is_some()
                && self.native_annotation_is_leaf(annotation)
        }))
    }

    /// Dart `_validateAddressPrefixedIdentifier`.
    fn validate_address_prefixed_identifier(
        &mut self,
        node: Id<PrefixedIdentifier>,
        extension: Option<ElementId>,
    ) {
        let error_node = self.ast[node].identifier.raw();
        self.validate_address_position(node.raw(), error_node);
        let extension_name = extension.and_then(|e| self.ctx.element_name(e));
        let receiver = self.ast[node].prefix.raw();
        self.validate_address_receiver(extension_name, Some(receiver), error_node);
    }

    /// Dart `_validateAddressPropertyAccess`.
    fn validate_address_property_access(
        &mut self,
        node: Id<PropertyAccess>,
        extension: Option<ElementId>,
    ) {
        let error_node = self.ast[node].property_name.raw();
        self.validate_address_position(node.raw(), error_node);
        let extension_name = extension.and_then(|e| self.ctx.element_name(e));
        let receiver = self.ast[node].target.map(|t| t.raw());
        self.validate_address_receiver(extension_name, receiver, error_node);
    }

    /// Dart `_validateAddressReceiver`.
    fn validate_address_receiver(
        &mut self,
        extension_name: Option<&str>,
        receiver: Option<NodeId>,
        error_node: NodeId,
    ) {
        if let Some(name) = extension_name
            && (ADDRESS_OF_COMPOUND_EXTENSION_NAMES.contains(&name)
                || ADDRESS_OF_TYPED_DATA_EXTENSION_NAMES.contains(&name))
        {
            return; // Only primitives need their receiver checked.
        }
        let Some(receiver) = receiver else {
            return;
        };
        let ast = self.ast;
        if let Some(index) = ast.cast::<IndexExpression>(receiver) {
            // Array or TypedData element.
            let t = ast[index].target.and_then(|t| self.static_type(t));
            if t.is_some_and(|t| self.is_array(t) || self.is_typed_data(t)) {
                return;
            }
        } else if let Some(p) = ast.cast::<PrefixedIdentifier>(receiver) {
            // Struct or Union field.
            let t = self.static_type(ast[p].prefix);
            if t.is_some_and(|t| self.is_compound_subtype(t)) {
                return;
            }
        } else if let Some(p) = ast.cast::<PropertyAccess>(receiver) {
            // Struct or Union field.
            let t = ast[p].target.and_then(|t| self.static_type(t));
            if t.is_some_and(|t| self.is_compound_subtype(t)) {
                return;
            }
        }
        self.report_at(diag::address_receiver(), error_node);
    }

    /// Dart `_validateAllocate`.
    fn validate_allocate(&mut self, node: Id<FunctionExpressionInvocation>) {
        let Some([dart_type]) = self.type_argument_types(node) else {
            return;
        };
        if !self.is_valid_ffi_native_type(
            Some(*dart_type),
            NativeTypeOptions {
                allow_void: true,
                allow_empty_struct: true,
                ..Default::default()
            },
        ) {
            let name = format!("{ALLOCATOR_EXTENSION_NAME}.{ALLOCATE_EXTENSION_METHOD_NAME}");
            self.report_at(diag::non_constant_type_argument(&name), node);
        }
    }

    /// Dart `_validateAnnotations`.
    fn validate_annotations(
        &mut self,
        error_node: Id<TypeAnnotation>,
        annotations: NodeList<Annotation>,
        required_type: PrimitiveDartType,
    ) {
        let ast = self.ast;
        let mut required_found = false;
        let mut extra_annotations: Vec<Id<Annotation>> = Vec::new();
        for &annotation in ast.list(annotations) {
            let element = self.annotation_element(annotation);
            if self.ffi_class(element).is_some()
                || self.is_abi_specific_integer_subclass(
                    element.and_then(|e| self.enclosing_element(e)),
                )
            {
                if required_found {
                    extra_annotations.push(annotation);
                } else {
                    let found_type = self.type_for_annotation(annotation);
                    if found_type == required_type {
                        required_found = true;
                    } else {
                        extra_annotations.push(annotation);
                    }
                }
            }
        }
        if !extra_annotations.is_empty() {
            if !required_found {
                let invalid_annotation = extra_annotations.remove(0);
                self.report_at(
                    diag::mismatched_annotation_on_struct_field(),
                    invalid_annotation,
                );
            }
            for extra_annotation in extra_annotations {
                self.report_at(diag::extra_annotation_on_struct_field(), extra_annotation);
            }
        } else if !required_found {
            let Some(t) = self.annotation_type(error_node) else {
                return;
            };
            let superclass_name = self
                .compound
                .and_then(|c| ast[c].extends_clause)
                .map(|e| self.lexeme(ast[ast[e].superclass].name))
                .unwrap_or("");
            let d = diag::missing_annotation_on_struct_field(self.type_arg(t), superclass_name);
            self.report_at(d, error_node);
        }
    }

    /// Dart `_validateAsFunction`.
    fn validate_as_function(&mut self, node: Id<MethodInvocation>) {
        let ast = self.ast;
        let type_arguments: Option<&[Id<TypeAnnotation>]> =
            ast[node].type_arguments.map(|l| ast.list(ast[l].arguments));
        let error_node: NodeId = match type_arguments {
            Some([first, ..]) => first.raw(),
            _ => node.raw(),
        };
        if let Some([single]) = type_arguments
            && self.validate_type_argument(*single, "asFunction")
        {
            return;
        }
        let Some(target) = self.method_invocation_real_target(node) else {
            return;
        };
        let target_type = self.static_type(target);
        if let Some(target_type) = target_type
            && self.is_pointer(target_type)
        {
            let Some(&t) = self.ctx.type_arguments(target_type).first() else {
                return;
            };
            if !self.is_native_function(t) {
                return;
            }
            let [pointer_type_arg] = self.ctx.type_arguments(t) else {
                return;
            };
            let pointer_type_arg = *pointer_type_arg;
            if matches!(
                self.ctx.ty(pointer_type_arg),
                TypeKind::TypeParameter { .. }
            ) {
                self.report_at(diag::non_constant_type_argument("asFunction"), target);
                return;
            }
            if !self.is_valid_ffi_native_function_type(pointer_type_arg) {
                let d = diag::non_native_function_type_argument_to_pointer(self.type_arg(t));
                self.report_at(d, error_node);
                return;
            }

            let t_prime = pointer_type_arg;
            let Some(&f) = self.type_argument_types(node).and_then(|a| a.first()) else {
                return;
            };
            let is_leaf = self.is_leaf(ast[ast[node].argument_list].arguments);
            if !self.validate_compatible_function_types(
                FfiTypeCheckDirection::NativeToDart,
                f,
                t_prime,
                false,
            ) {
                let d =
                    diag::must_be_a_subtype(self.type_arg(t_prime), self.type_arg(f), "asFunction");
                self.report_at(d, node);
            }
            if is_leaf {
                let span = self.span_node(ast[node].method_name);
                self.validate_ffi_leaf_call_uses_no_handles(t_prime, span);
            }
        }
        self.validate_is_leaf_is_const(node);
    }

    /// Dart `_validateCompatibleFunctionTypes`.
    fn validate_compatible_function_types(
        &self,
        direction: FfiTypeCheckDirection,
        dart_type: TypeId,
        native_type: TypeId,
        native_field_wrappers_as_pointer: bool,
    ) -> bool {
        // We require both to be valid function types.
        let (Some(dart), Some(native)) = (self.function(dart_type), self.function(native_type))
        else {
            return false;
        };

        let native_type_normal_parameter_types =
            self.flatten_var_args(self.normal_parameter_types(&native));
        let dart_normal_parameter_types = self.normal_parameter_types(&dart);

        // We disallow any optional parameters.
        let parameter_count = dart_normal_parameter_types.len();
        if parameter_count != native_type_normal_parameter_types.len() {
            return false;
        }
        // We disallow generic function types.
        if !dart.type_params.is_empty() || !native.type_params.is_empty() {
            return false;
        }
        if self.has_optional_or_named_parameters(&dart)
            || self.has_optional_or_named_parameters(&native)
        {
            return false;
        }

        // Validate that the return types are compatible.
        if !self.validate_compatible_native_type(direction, dart.ret, native.ret, false, false) {
            return false;
        }

        // Validate that the parameter types are compatible.
        for i in 0..parameter_count {
            if !self.validate_compatible_native_type(
                direction.reverse(),
                dart_normal_parameter_types[i],
                native_type_normal_parameter_types[i],
                native_field_wrappers_as_pointer,
                false,
            ) {
                return false;
            }
        }

        // Signatures have same number of parameters and the types match.
        true
    }

    /// Dart `_validateCompatibleNativeType`.
    fn validate_compatible_native_type(
        &self,
        direction: FfiTypeCheckDirection,
        dart_type: TypeId,
        native_type: TypeId,
        native_field_wrappers_as_pointer: bool,
        allow_functions: bool,
    ) -> bool {
        let native_return_type = self.primitive_native_type(native_type);
        let native_superclass_is_abi_specific_integer = self.interface(native_type).is_some()
            && self
                .ctx
                .superclass(native_type)
                .and_then(|s| self.interface(s))
                .and_then(|(e, _)| self.ctx.element_name(e.raw()))
                == Some(ABI_SPECIFIC_INTEGER_CLASS_NAME);
        if native_return_type == PrimitiveDartType::Int || native_superclass_is_abi_specific_integer
        {
            self.ctx.is_dart_core_int(dart_type)
        } else if native_return_type == PrimitiveDartType::Double {
            self.ctx.is_dart_core_double(dart_type)
        } else if native_return_type == PrimitiveDartType::Bool {
            self.ctx.is_dart_core_bool(dart_type)
        } else if native_return_type == PrimitiveDartType::Void {
            direction == FfiTypeCheckDirection::DartToNative || dart_type == TypeId::VOID
        } else if dart_type == TypeId::VOID {
            // Don't allow other native subtypes if the Dart return type is
            // void.
            native_return_type == PrimitiveDartType::Void
        } else if native_return_type == PrimitiveDartType::Handle {
            // `Handle` matches against any type in positions of any variance.
            true
        } else if self.interface(dart_type).is_some() && self.interface(native_type).is_some() {
            if native_field_wrappers_as_pointer
                && self.extends_native_field_wrapper_class1(Some(dart_type))
            {
                // Must be `Pointer<Void>`, `Handle` already checked above.
                return self.is_pointer(native_type)
                    && match self.ctx.type_arguments(native_type) {
                        [single] => self.primitive_native_type(*single) == PrimitiveDartType::Void,
                        _ => false,
                    };
            }
            // Always allow typed data here, error on nonLeaf or return value
            // in `_validateFfiNonLeafCallUsesNoTypedData`.
            if self.is_valid_typed_data(native_type, dart_type) {
                return true;
            }
            match direction {
                FfiTypeCheckDirection::DartToNative => {
                    self.type_system.is_subtype_of(dart_type, native_type)
                }
                FfiTypeCheckDirection::NativeToDart => {
                    self.type_system.is_subtype_of(native_type, dart_type)
                }
            }
        } else if self.function(dart_type).is_some()
            && allow_functions
            && self.is_native_function(native_type)
        {
            let Some(&native_function) = self.ctx.type_arguments(native_type).first() else {
                return false;
            };
            self.validate_compatible_function_types(
                direction,
                dart_type,
                native_function,
                native_field_wrappers_as_pointer,
            )
        } else {
            // If the [nativeType] is not a primitive int/double type then it
            // has to be a Pointer type atm.
            false
        }
    }

    /// Dart `_validateCreate`.
    fn validate_create(&mut self, node: Id<MethodInvocation>, error_class: &str) {
        let Some([dart_type]) = self.type_argument_types(node) else {
            return;
        };
        if !self.is_valid_ffi_native_type(Some(*dart_type), NativeTypeOptions::default()) {
            let name = format!("{error_class}.create");
            self.report_at(diag::non_constant_type_argument(&name), node);
        }
    }

    /// Dart `_validateElementAt`.
    fn validate_element_at(&mut self, node: Id<MethodInvocation>) {
        let target_type = self
            .method_invocation_real_target(node)
            .and_then(|t| self.static_type(t));
        if let Some(target_type) = target_type
            && self.is_pointer(target_type)
        {
            let t = self.ctx.type_arguments(target_type).first().copied();
            if !self.is_valid_ffi_native_type(
                t,
                NativeTypeOptions {
                    allow_void: true,
                    allow_empty_struct: true,
                    ..Default::default()
                },
            ) {
                self.report_at(diag::non_constant_type_argument("elementAt"), node);
            }
        }
    }

    /// Dart `_validateFfiLeafCallUsesNoHandles`.
    fn validate_ffi_leaf_call_uses_no_handles(&mut self, native_type: TypeId, error_entity: Span) {
        if let Some(f) = self.function(native_type) {
            if self.primitive_native_type(f.ret) == PrimitiveDartType::Handle {
                self.report_span(diag::leaf_call_must_not_return_handle(), error_entity);
            }
            for param in self.normal_parameter_types(&f) {
                if self.primitive_native_type(param) == PrimitiveDartType::Handle {
                    self.report_span(diag::leaf_call_must_not_take_handle(), error_entity);
                }
            }
        }
    }

    /// Dart `_validateFieldsInCompound`.
    fn validate_fields_in_compound(&mut self, node: Id<FieldDeclaration>) {
        let ast = self.ast;
        if ast[node].static_keyword.is_some() {
            return;
        }

        let fields = ast[node].fields;
        let annotations = ast[node].metadata;
        let Some(&first_variable) = ast.list(ast[fields].variables).first() else {
            return;
        };

        if ast[node].external_keyword.is_none() {
            self.report_at_token(
                diag::field_must_be_external_in_struct(),
                ast[first_variable].name,
            );
        }

        let Some(field_type) = ast[fields].type_ else {
            self.report_at_token(
                diag::missing_field_type_in_struct(),
                ast[first_variable].name,
            );
            return;
        };
        let Some(declared_type) = self.annotation_type(field_type) else {
            return;
        };
        if self.ctx.nullability_suffix(declared_type) == Nullability::Question {
            let source = dartr_ast::to_source::to_source(ast, field_type);
            self.report_at(diag::invalid_field_type_in_struct(&source), field_type);
        } else if self.ctx.is_dart_core_int(declared_type) {
            self.validate_annotations(field_type, annotations, PrimitiveDartType::Int);
        } else if self.ctx.is_dart_core_double(declared_type) {
            self.validate_annotations(field_type, annotations, PrimitiveDartType::Double);
        } else if self.ctx.is_dart_core_bool(declared_type) {
            self.validate_annotations(field_type, annotations, PrimitiveDartType::Bool);
        } else if self.is_pointer(declared_type) {
            self.validate_no_annotations(annotations);
        } else if self.is_array(declared_type) {
            if let [type_arg] = self.ctx.type_arguments(declared_type)
                && !self.is_sized(*type_arg)
            {
                let mut error_node = field_type.raw();
                if let Some(named_type) = ast.cast::<NamedType>(field_type.raw())
                    && let Some(type_arguments) = ast[named_type].type_arguments
                    && let Some(&first) = ast.list(ast[type_arguments].arguments).first()
                {
                    error_node = first.raw();
                }
                let d = diag::non_sized_type_argument(ARRAY_CLASS_NAME, self.type_arg(*type_arg));
                self.report_at(d, error_node);
            }
            let array_dimensions = self.array_dimensions(declared_type);
            let field_element = self.declared_element(first_variable.raw());
            let last_element = field_element
                .and_then(|f| self.enclosing_element(f))
                .and_then(|c| c.cast::<InterfaceElement>())
                .and_then(|class| {
                    self.ctx
                        .interface(class)
                        .fields
                        .iter()
                        .rev()
                        .map(|f| f.raw())
                        .find(|&field| self.is_external_instance_field(field))
                });
            let is_last_field = field_element == last_element;
            let span = self.span_node(field_type);
            self.validate_size_of_annotation(span, annotations, array_dimensions, is_last_field);
        } else if self.is_compound_subtype(declared_type) {
            if let Some((class, _)) = self.interface(declared_type)
                && self.is_empty_struct(class.raw())
            {
                let name = self.ctx.element_name(class.raw()).unwrap_or("");
                let supertype = self
                    .ctx
                    .element_supertype(class)
                    .map(|s| type_display_string(&self.ctx, s, true))
                    .unwrap_or_default();
                self.report_at(diag::empty_struct(name, &supertype), node);
            }
        } else {
            let source = dartr_ast::to_source::to_source(ast, field_type);
            self.report_at(diag::invalid_field_type_in_struct(&source), field_type);
        }
    }

    /// The filter of the last field in `_validateFieldsInCompound`: an
    /// instance field that is external (or has an external accessor).
    fn is_external_instance_field(&self, field: ElementId) -> bool {
        if self.is_static(field) {
            return false;
        }
        if first_fragment_flags(&self.ctx, field)
            .contains(FragmentFlags::VARIABLE_FRAGMENT_IS_EXTERNAL)
        {
            return true;
        }
        let Some(data) = field
            .cast::<dartr_element::PropertyInducingElement>()
            .map(|p| self.ctx.property_inducing(p))
        else {
            return false;
        };
        let accessor_is_external = |e: Option<ElementId>| {
            e.is_some_and(|e| {
                first_fragment_flags(&self.ctx, e)
                    .contains(FragmentFlags::EXECUTABLE_FRAGMENT_IS_EXTERNAL)
            })
        };
        accessor_is_external(data.getter.map(|g| g.raw()))
            || accessor_is_external(data.setter.map(|s| s.raw()))
    }

    /// Dart `_validateFromFunction`.
    fn validate_from_function(&mut self, node: Id<MethodInvocation>) {
        let ast = self.ast;
        let arguments = ast.list(ast[ast[node].argument_list].arguments);
        let arg_count = arguments.len();
        if !(1..=2).contains(&arg_count) {
            // There are other diagnostics reported against the invocation
            // and the diagnostics generated below might be inaccurate, so
            // don't report them.
            return;
        }

        let Some(&t) = self.type_argument_types(node).and_then(|a| a.first()) else {
            return;
        };
        if !self.is_valid_ffi_native_function_type(t) {
            let mut error_node = ast[node].method_name.raw();
            if let Some(type_arguments) = ast[node].type_arguments
                && let Some(&first) = ast.list(ast[type_arguments].arguments).first()
            {
                error_node = first.raw();
            }
            let d = diag::must_be_a_native_function_type(self.type_arg(t), "fromFunction");
            self.report_at(d, error_node);
            return;
        }

        let f = arguments[0];
        let Some(ft) = self.static_type(self.argument_expression(f)) else {
            return;
        };
        if !self.validate_compatible_function_types(
            FfiTypeCheckDirection::DartToNative,
            ft,
            t,
            false,
        ) {
            let d = diag::must_be_a_subtype(self.type_arg(ft), self.type_arg(t), "fromFunction");
            self.report_at(d, f);
            return;
        }

        // TODO(brianwilkerson): Validate that `f` is a top-level function.
        let Some(function) = self.function(t) else {
            return;
        };
        let r = function.ret;
        if self.primitive_native_type(r) == PrimitiveDartType::Void
            || self.is_pointer(r)
            || self.is_handle(r)
            || self.is_compound_subtype(r)
        {
            if arg_count != 1 {
                self.report_at(diag::invalid_exception_value("fromFunction"), arguments[1]);
            }
        } else if arg_count != 2 {
            self.report_at(
                diag::missing_exception_value("fromFunction"),
                ast[node].method_name,
            );
        } else {
            let e = self.argument_expression(arguments[1]);
            let Some(e_type) = self.static_type(e) else {
                return;
            };
            if !self.validate_compatible_native_type(
                FfiTypeCheckDirection::DartToNative,
                e_type,
                r,
                false,
                false,
            ) {
                let d = diag::must_be_a_subtype(
                    self.type_arg(e_type),
                    self.type_arg(r),
                    "fromFunction",
                );
                self.report_at(d, e);
            }
            if !self.is_const(e) {
                self.report_at(diag::argument_must_be_a_constant("exceptionalReturn"), e);
            }
        }
    }

    /// Dart `_validateIsLeafIsConst`.
    fn validate_is_leaf_is_const(&mut self, node: Id<MethodInvocation>) {
        let ast = self.ast;
        let args = ast[ast[node].argument_list].arguments;
        for &arg in ast.list(args) {
            if let Some(named) = ast.cast::<NamedArgument>(arg.raw())
                && self.named_argument_parameter_name(named) == Some(IS_LEAF_PARAM_NAME)
            {
                let expression = ast[named].argument_expression;
                if !self.is_const(expression) {
                    self.report_at(
                        diag::argument_must_be_a_constant(IS_LEAF_PARAM_NAME),
                        expression,
                    );
                }
            }
        }
    }

    /// Dart `_validateLookupFunction`.
    fn validate_lookup_function(&mut self, node: Id<MethodInvocation>) {
        let ast = self.ast;
        let Some(type_arguments) = ast[node].type_arguments else {
            return;
        };
        let type_arguments = ast.list(ast[type_arguments].arguments);
        if type_arguments.len() != 2 {
            // There are other diagnostics reported against the invocation
            // and the diagnostics generated below might be inaccurate, so
            // don't report them.
            return;
        }

        let Some(&[s, f]) = self.type_argument_types(node) else {
            return;
        };
        if !self.is_valid_ffi_native_function_type(s) {
            let d = diag::must_be_a_native_function_type(self.type_arg(s), "lookupFunction");
            self.report_at(d, type_arguments[0]);
            return;
        }
        let is_leaf = self.is_leaf(ast[ast[node].argument_list].arguments);
        if !self.validate_compatible_function_types(
            FfiTypeCheckDirection::NativeToDart,
            f,
            s,
            false,
        ) {
            let d = diag::must_be_a_subtype(self.type_arg(s), self.type_arg(f), "lookupFunction");
            self.report_at(d, type_arguments[1]);
        }
        self.validate_is_leaf_is_const(node);
        if is_leaf {
            let span = self.span_node(type_arguments[0]);
            self.validate_ffi_leaf_call_uses_no_handles(s, span);
        }
    }

    /// Dart `_validateNativeAddressOf`.
    fn validate_native_address_of(&mut self, node: Id<MethodInvocation>) {
        let ast = self.ast;
        let arguments = ast.list(ast[ast[node].argument_list].arguments);
        let Some(&[target_type]) = self.type_argument_types(node) else {
            return;
        };
        if arguments.len() != 1 {
            // There are other diagnostics reported against the invocation
            // and the diagnostics generated below might be inaccurate, so
            // don't report them.
            return;
        }

        let argument = arguments[0];
        let mut valid_target = false;

        let referenced_element = if ast.is::<Identifier>(argument.raw()) {
            let element = self.base_element(argument.raw()).or_else(|| {
                ast.cast::<PrefixedIdentifier>(argument.raw())
                    .and_then(|p| self.base_element(ast[p].identifier.raw()))
            });
            element.map(|e| non_synthetic(&self.ctx, e))
        } else {
            None
        };

        if let Some(referenced_element) = referenced_element {
            let Some(metadata) = self.element_metadata(referenced_element) else {
                // Declared in another unit: the metadata is not available.
                return;
            };
            for &annotation in ast.list(metadata) {
                let Some(annotation_type) = self.native_annotation_type(annotation) else {
                    continue;
                };
                let native_type = self
                    .ctx
                    .type_arguments(annotation_type)
                    .first()
                    .copied()
                    .unwrap_or(TypeId::DYNAMIC);

                let target_function_type = if self.is_native_function(target_type) {
                    self.ctx.type_arguments(target_type).first().copied()
                } else {
                    None
                };

                if self.function(native_type).is_some() {
                    // When referencing a function, the target type must be a
                    // `NativeFunction<T>` so that `T` matches the type from
                    // the annotation.
                    match target_function_type {
                        Some(target_function_type) => {
                            if !self
                                .type_system
                                .is_equal_to(native_type, target_function_type)
                            {
                                let d = diag::must_be_a_subtype(
                                    self.type_arg(native_type),
                                    self.type_arg(target_function_type),
                                    NATIVE_ADDRESS_OF,
                                );
                                self.report_at(d, node);
                            }
                        }
                        None => {
                            let d = diag::must_be_a_native_function_type(
                                self.type_arg(target_type),
                                NATIVE_ADDRESS_OF,
                            );
                            self.report_at(d, node);
                        }
                    }
                } else if let Some(mut static_type) =
                    self.static_type(self.argument_expression(argument))
                    && native_type == TypeId::DYNAMIC
                {
                    // No type argument was given on the @Native annotation,
                    // so we try to infer the native type from the Dart
                    // signature.
                    if let Some(f) = self.function(static_type) {
                        if f.ret == TypeId::VOID {
                            // The Dart signature has a `void` return type, so
                            // we create a new `FunctionType` with FFI's
                            // `Void` as the return type.
                            let void_type = match self.ffi_void_type {
                                Some(t) => Some(t),
                                None => {
                                    let native =
                                        self.interface(annotation_type).map(|(e, _)| e.raw());
                                    let t = native.and_then(|e| self.library_class_type(e, "Void"));
                                    self.ffi_void_type = t;
                                    t
                                }
                            };
                            if let Some(void_type) = void_type {
                                let params = self.ctx.list(f.params).to_vec();
                                static_type = self.function_type_with(&f, &params, void_type);
                            }
                        }

                        match target_function_type {
                            Some(target_function_type) => {
                                if !self
                                    .type_system
                                    .is_equal_to(static_type, target_function_type)
                                {
                                    let d = diag::must_be_a_subtype(
                                        self.type_arg(static_type),
                                        self.type_arg(target_function_type),
                                        NATIVE_ADDRESS_OF,
                                    );
                                    self.report_at(d, node);
                                }
                            }
                            None => {
                                let d = diag::must_be_a_native_function_type(
                                    self.type_arg(target_type),
                                    NATIVE_ADDRESS_OF,
                                );
                                self.report_at(d, node);
                            }
                        }
                    } else if !self.type_system.is_equal_to(static_type, target_type) {
                        let d = diag::must_be_a_subtype(
                            self.type_arg(static_type),
                            self.type_arg(target_type),
                            NATIVE_ADDRESS_OF,
                        );
                        self.report_at(d, node);
                    }
                }

                valid_target = true;
                break;
            }
        }

        if !valid_target {
            self.report_at(diag::argument_must_be_native(), argument);
        }
    }

    /// Dart `_validateNativeCallable`.
    fn validate_native_callable(&mut self, node: Id<InstanceCreationExpression>) {
        let ast = self.ast;
        let constructor_name = ast[node].constructor_name;
        let name = ast[constructor_name]
            .name
            .map(|n| crate::ast_ext::identifier_name(ast, n))
            .unwrap_or("");
        let isolate_local = name == "isolateLocal";

        // listener takes 1 arg, isolateLocal takes 1 or 2.
        let arguments = ast.list(ast[ast[node].argument_list].arguments);
        let arg_count = arguments.len();
        if !(arg_count == 1 || (isolate_local && arg_count == 2)) {
            // There are other diagnostics reported against the invocation
            // and the diagnostics generated below might be inaccurate, so
            // don't report them.
            return;
        }

        let Some(node_type) = self.static_type(node) else {
            return;
        };
        if self.interface(node_type).is_none() {
            return;
        }

        let Some(&type_arg) = self.ctx.type_arguments(node_type).first() else {
            return;
        };
        if !self.is_valid_ffi_native_function_type(type_arg) {
            let d = diag::must_be_a_native_function_type(self.type_arg(type_arg), NATIVE_CALLABLE);
            self.report_at(d, constructor_name);
            return;
        }

        let f = arguments[0];
        let Some(func_type) = self.static_type(self.argument_expression(f)) else {
            return;
        };
        if !self.validate_compatible_function_types(
            FfiTypeCheckDirection::DartToNative,
            func_type,
            type_arg,
            false,
        ) {
            let d = diag::must_be_a_subtype(
                self.type_arg(func_type),
                self.type_arg(type_arg),
                NATIVE_CALLABLE,
            );
            self.report_at(d, f);
            return;
        }

        let Some(function) = self.function(type_arg) else {
            return;
        };
        let nat_ret_type = function.ret;
        if isolate_local {
            if self.primitive_native_type(nat_ret_type) == PrimitiveDartType::Void
                || self.is_pointer(nat_ret_type)
                || self.is_handle(nat_ret_type)
                || self.is_compound_subtype(nat_ret_type)
            {
                if arg_count != 1 {
                    self.report_at(diag::invalid_exception_value(name), arguments[1]);
                }
            } else if arg_count != 2 {
                self.report_at(diag::missing_exception_value(name), node);
            } else {
                let e = self.argument_expression(arguments[1]);
                let Some(e_type) = self.static_type(e) else {
                    return;
                };
                if !self.validate_compatible_native_type(
                    FfiTypeCheckDirection::DartToNative,
                    e_type,
                    nat_ret_type,
                    false,
                    false,
                ) {
                    let d = diag::must_be_a_subtype(
                        self.type_arg(e_type),
                        self.type_arg(nat_ret_type),
                        name,
                    );
                    self.report_at(d, e);
                }
                if !self.is_const(e) {
                    self.report_at(diag::argument_must_be_a_constant("exceptionalReturn"), e);
                }
            }
        } else if self.primitive_native_type(nat_ret_type) != PrimitiveDartType::Void {
            self.report_at(diag::must_return_void(self.type_arg(nat_ret_type)), f);
        }
    }

    /// Dart `_validateNoAnnotations`.
    fn validate_no_annotations(&mut self, annotations: NodeList<Annotation>) {
        for &annotation in self.ast.list(annotations) {
            if self
                .ffi_class(self.annotation_element(annotation))
                .is_some()
            {
                self.report_at(diag::annotation_on_pointer_field(), annotation);
            }
        }
    }

    /// Dart `_validatePackedAnnotation`.
    fn validate_packed_annotation(&mut self, annotations: NodeList<Annotation>) {
        let ast = self.ast;
        let ffi_packed_annotations: Vec<Id<Annotation>> = ast
            .list(annotations)
            .iter()
            .copied()
            .filter(|&a| self.annotation_is_ffi_constructor_of(a, "Packed"))
            .collect();

        let Some(&annotation) = ffi_packed_annotations.first() else {
            return;
        };

        for &extra in ffi_packed_annotations.iter().skip(1) {
            self.report_at(diag::packed_annotation(), extra);
        }

        // Check number of dimensions.
        let arguments = ast[annotation]
            .arguments
            .map(|a| ast.list(ast[a].arguments));
        // Dart `annotation.elementAnnotation?.packedMemberAlignment`: a
        // missing argument is an invalid constant (null); an argument that
        // is not a literal needs the constant evaluator (not checked).
        let value = match arguments {
            Some([first, ..]) => match self.int_value(self.argument_expression(*first)) {
                Some(v) => Some(v),
                None => return,
            },
            _ => None,
        };
        if !matches!(value, Some(1 | 2 | 4 | 8 | 16)) {
            let error_node: NodeId = match arguments {
                Some([first, ..]) => first.raw(),
                _ => annotation.raw(),
            };
            self.report_at(diag::packed_annotation_alignment(), error_node);
        }
    }

    /// Dart `_validateRefIndexed`.
    fn validate_ref_indexed(&mut self, node: Id<IndexExpression>) {
        let target_type = self
            .index_expression_real_target(node)
            .and_then(|t| self.static_type(t));
        if !self.is_valid_ffi_native_type(
            target_type,
            NativeTypeOptions {
                allow_empty_struct: true,
                allow_array: true,
                ..Default::default()
            },
        ) {
            self.report_at(diag::non_constant_type_argument("[]"), node);
        }
    }

    /// Dart `_validateRefPrefixedIdentifier`.
    fn validate_ref_prefixed_identifier(&mut self, node: Id<PrefixedIdentifier>) {
        let target_type = self.static_type(self.ast[node].prefix);
        if !self.is_valid_ffi_native_type(
            target_type,
            NativeTypeOptions {
                allow_empty_struct: true,
                ..Default::default()
            },
        ) {
            self.report_at(diag::non_constant_type_argument("ref"), node);
        }
    }

    /// Dart `_validateRefPropertyAccess`.
    fn validate_ref_property_access(&mut self, node: Id<PropertyAccess>) {
        let target_type = self
            .property_access_real_target(node)
            .and_then(|t| self.static_type(t));
        if !self.is_valid_ffi_native_type(
            target_type,
            NativeTypeOptions {
                allow_empty_struct: true,
                ..Default::default()
            },
        ) {
            self.report_at(diag::non_constant_type_argument("ref"), node);
        }
    }

    /// Dart `_validateRefWithFinalizer`.
    fn validate_ref_with_finalizer(&mut self, node: Id<MethodInvocation>) {
        let target_type = self
            .method_invocation_real_target(node)
            .and_then(|t| self.static_type(t));
        if !self.is_valid_ffi_native_type(
            target_type,
            NativeTypeOptions {
                allow_empty_struct: true,
                ..Default::default()
            },
        ) {
            self.report_at(diag::non_constant_type_argument("refWithFinalizer"), node);
        }
    }

    /// Dart `_validateSizeOf`.
    fn validate_size_of(&mut self, node: Id<MethodInvocation>) {
        let Some([t]) = self.type_argument_types(node) else {
            return;
        };
        if !self.is_valid_ffi_native_type(
            Some(*t),
            NativeTypeOptions {
                allow_void: true,
                allow_empty_struct: true,
                ..Default::default()
            },
        ) {
            self.report_at(diag::non_constant_type_argument("sizeOf"), node);
        }
    }

    /// Dart `ElementAnnotation.arraySizeDimensions` of an `@Array`
    /// annotation, from the literal arguments (`None` when an argument is
    /// not a literal).
    fn array_size_dimensions(&self, annotation: Id<Annotation>) -> Option<(Vec<i64>, bool)> {
        let ast = self.ast;
        let constructor = self
            .annotation_element(annotation)
            .and_then(|e| self.ctx.element_name(e))
            .unwrap_or("new");
        let arguments: Vec<Id<Argument>> = match ast[annotation].arguments {
            Some(a) => ast.list(ast[a].arguments).to_vec(),
            None => Vec::new(),
        };
        let mut positional = Vec::new();
        let mut named: Vec<(&str, Id<Expression>)> = Vec::new();
        for &argument in &arguments {
            match ast.cast::<NamedArgument>(argument.raw()) {
                Some(n) => named.push((self.lexeme(ast[n].name), ast[n].argument_expression)),
                None => positional.push(Id::<Expression>::from_raw(argument.raw())),
            }
        }
        let ints = |list: &[Id<Expression>]| -> Option<Vec<i64>> {
            list.iter().map(|&e| self.int_value(e)).collect()
        };
        match constructor {
            "multi" => {
                let list = self.int_list_value(*positional.first()?)?;
                Some((list, false))
            }
            "variableMulti" => {
                let list = self.int_list_value(*positional.first()?)?;
                let variable_dimension = match named.iter().find(|(n, _)| *n == "variableDimension")
                {
                    Some(&(_, e)) => self.int_value(e)?,
                    None => 0,
                };
                let mut dimensions = vec![variable_dimension];
                dimensions.extend(list);
                Some((dimensions, true))
            }
            "variable" => {
                let mut dimensions = vec![0];
                dimensions.extend(ints(&positional)?);
                Some((dimensions, true))
            }
            "variableWithVariableDimension" => {
                let dimensions = ints(&positional)?;
                let variable_length = !dimensions.is_empty();
                Some((dimensions, variable_length))
            }
            _ => Some((ints(&positional)?, false)),
        }
    }

    /// Dart `_validateSizeOfAnnotation`.
    fn validate_size_of_annotation(
        &mut self,
        error_entity: Span,
        annotations: NodeList<Annotation>,
        array_dimensions: i64,
        allow_variable_length: bool,
    ) {
        let ast = self.ast;
        let ffi_size_annotations: Vec<Id<Annotation>> = ast
            .list(annotations)
            .iter()
            .copied()
            .filter(|&a| self.annotation_is_ffi_constructor_of(a, ARRAY_CLASS_NAME))
            .collect();

        let Some(&annotation) = ffi_size_annotations.first() else {
            self.report_span(diag::missing_size_annotation_carray(), error_entity);
            return;
        };

        for &extra in ffi_size_annotations.iter().skip(1) {
            self.report_at(diag::extra_size_annotation_carray(), extra);
        }

        // Check number of dimensions.
        let Some((dimensions, variable_length)) = self.array_size_dimensions(annotation) else {
            // Not literals: needs the constant evaluator.
            return;
        };
        let annotation_dimensions = dimensions.len() as i64;
        if annotation_dimensions != array_dimensions {
            self.report_at(diag::size_annotation_dimensions(), annotation);
        }

        if variable_length && !allow_variable_length {
            self.report_at(diag::variable_length_array_not_last(), annotation);
        }

        // Check dimensions are valid.
        let (dimensions_nodes, variable_dimension_node): (Option<Vec<NodeId>>, Option<NodeId>) =
            match ast[annotation].arguments {
                Some(argument_list) => {
                    let arguments = ast.list(ast[argument_list].arguments);
                    let list_elements = |e: Id<Argument>| -> Option<Vec<NodeId>> {
                        let l = ast.cast::<ListLiteral>(e.raw())?;
                        Some(ast.list(ast[l].elements).iter().map(|x| x.raw()).collect())
                    };
                    match arguments {
                        // `@Array.variableMulti([..], variableDimension: ..)`
                        [first, second]
                            if list_elements(*first).is_some()
                                && ast.is::<NamedArgument>(second.raw()) =>
                        {
                            let named = ast.cast::<NamedArgument>(second.raw()).expect("is");
                            (
                                list_elements(*first),
                                Some(ast[named].argument_expression.raw()),
                            )
                        }
                        // `@Array.variableMulti([..])`
                        [first] if list_elements(*first).is_some() => (list_elements(*first), None),
                        // `@Array(..)`, `@Array.variable(..)`,
                        // `@Array.variableWithVariableDimension(..)`
                        _ => (Some(arguments.iter().map(|a| a.raw()).collect()), None),
                    }
                }
                None => (None, None),
            };
        let mut error_node = variable_dimension_node.unwrap_or(annotation.raw());

        for i in 0..dimensions.len() {
            if let Some(dimensions_nodes) = &dimensions_nodes
                && dimensions_nodes.len() > i
                && variable_dimension_node.is_none()
            {
                let node = dimensions_nodes[i];
                error_node = match ast.cast::<NamedArgument>(node) {
                    Some(n) => ast[n].argument_expression.raw(),
                    None => node,
                };
            }

            // First dimension is variable.
            if i == 0 && variable_length {
                // Variable dimension can't be negative.
                if dimensions[0] < 0 {
                    self.report_at(diag::negative_variable_dimension(), error_node);
                }
                continue;
            }

            if dimensions[i] <= 0 {
                self.report_at(diag::non_positive_array_dimension(), error_node);
            }
        }
    }

    /// Dart `_validateTypeArgument`.
    fn validate_type_argument(
        &mut self,
        type_argument: Id<TypeAnnotation>,
        function_name: &str,
    ) -> bool {
        if let Some(t) = self.annotation_type(type_argument)
            && matches!(self.ctx.ty(t), TypeKind::TypeParameter { .. })
        {
            self.report_at(
                diag::non_constant_type_argument(function_name),
                type_argument,
            );
            return true;
        }
        false
    }
}
