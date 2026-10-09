// Dart source: pkg/analyzer/lib/src/dart/constant/evaluation.dart

//! [`ConstantEvaluationEngine`] (computes the values of constant variables,
//! formal parameter defaults and annotations), [`ConstantVisitor`] (the
//! value of one constant expression) and `_InstanceCreationEvaluator`
//! (const constructor invocations).
//!
//! Nodes are [`NodeRef`]s: a unit of the engine's unit registry and a node
//! of its AST. See the module documentation of [`crate::constant`] for why
//! the evaluator reads resolved unit ASTs.

use std::cell::RefCell;
use std::ops::Deref;
use std::sync::Arc;

use dartr_ast::{
    AdjacentStrings, Annotation, ArgumentList, AsExpression, AssertInitializer, Ast,
    BinaryExpression, BooleanLiteral, CompilationUnit, ConditionalExpression,
    ConstructorDeclaration, ConstructorFieldInitializer, ConstructorReference,
    DotShorthandConstructorInvocation, DotShorthandPropertyAccess, DoubleLiteral,
    EnumConstantDeclaration, Expression, FieldFormalParameter, FunctionReference, Id, IfElement,
    ImportPrefixReference, InstanceCreationExpression, IntegerLiteral, InterpolationExpression,
    InterpolationString, IsExpression, ListLiteral, MapLiteralEntry, MethodInvocation,
    NamedArgument, NamedType, NodeId, NodeKind, NullAwareElement, ParenthesizedExpression,
    PrefixExpression, PrefixedIdentifier, PropertyAccess, RecordLiteral, RecordLiteralNamedField,
    RedirectingConstructorInvocation, RegularFormalParameter, SetOrMapLiteral, SimpleIdentifier,
    SimpleStringLiteral, SpreadElement, StringInterpolation, SuperConstructorInvocation,
    SuperFormalParameter, SymbolLiteral, TypeLiteral, VariableDeclaration,
};
use dartr_constant::{
    BoolState, Constant, ConstructorInvocationImpl, DartObjectImpl, DartObjectMap, DartObjectSet,
    DeclaredVariables, DoubleState, EvaluationException, FieldMap, FromEnvironmentEvaluator,
    FunctionState, GenericState, InstanceState, IntState, InvalidConstant, ListState, MapState,
    NullState, RecordState, SetState, StringState, SymbolState, TypeState,
    has_type_parameter_reference,
};
use dartr_diagnostics::{Diagnostic, DiagnosticMessage, LocatableDiagnostic, diag};
use dartr_element::{
    Ctx, EId, ElemRef, ElementId, ExecutableElement, FId, FeatureSet, FieldElement,
    FormalParameterElement, FragmentFlags, FragmentId, InterfaceElement, LibraryElement,
    LibraryFragment, NamedType as NamedField, NoopSink, Nullability, Tag, TypeAliasElement, TypeId,
    TypeKind, TypeProvider, VariableElement, WorldSnapshot,
};
use dartr_syntax::TokenType;
use dartr_typesystem::{MapSubstitution, TypeExt, TypeSystem, lookup, member};
use indexmap::{IndexMap, IndexSet};

use crate::ast_ext;
use crate::constant::potentially_constant::{ConstCheckInput, get_not_potentially_constants};
use crate::library_analyzer::ResolvedUnit;

// ---------------------------------------------------------------------------
// Units
// ---------------------------------------------------------------------------

/// Resolved units of other libraries (Dart: the linked, resolved constant
/// expressions of their elements). Implemented by the caller of the
/// library analysis, which caches the units.
pub trait ExternalUnits: Sync {
    /// The resolved unit of [fragment], a unit of a library other than the
    /// one under analysis. `None` when it cannot be resolved.
    fn resolved_unit(&self, fragment: FId<LibraryFragment>) -> Option<Arc<ResolvedUnit>>;
}

/// A unit of the engine: borrowed (the library under analysis) or shared
/// (another library).
#[derive(Clone)]
pub enum UnitHandle<'a> {
    Borrowed(&'a ResolvedUnit),
    Shared(Arc<ResolvedUnit>),
}

impl Deref for UnitHandle<'_> {
    type Target = ResolvedUnit;

    fn deref(&self) -> &ResolvedUnit {
        match self {
            UnitHandle::Borrowed(u) => u,
            UnitHandle::Shared(u) => u,
        }
    }
}

/// A node of a unit of the engine.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeRef {
    pub unit: u32,
    pub node: NodeId,
}

impl NodeRef {
    pub fn new(unit: u32, node: impl Into<NodeId>) -> NodeRef {
        NodeRef {
            unit,
            node: node.into(),
        }
    }

    fn with(self, node: impl Into<NodeId>) -> NodeRef {
        NodeRef {
            unit: self.unit,
            node: node.into(),
        }
    }
}

/// Dart `ConstantEvaluationTarget`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ConstantTarget {
    /// A variable, formal parameter or constructor (always a base element).
    Element(ElementId),
    /// An `Annotation` node (Dart `ElementAnnotationImpl`).
    Annotation(NodeRef),
}

#[derive(Default)]
struct UnitRegistry<'a> {
    units: Vec<UnitHandle<'a>>,
    /// The library of each unit.
    libraries: Vec<EId<LibraryElement>>,
    by_fragment: IndexMap<FId<LibraryFragment>, Option<u32>>,
    /// The unit of each local store (raw store id).
    by_local_store: IndexMap<u32, u32>,
    /// Per unit: the declaration node of each declared fragment.
    declarations: Vec<Option<Arc<IndexMap<FragmentId, NodeId>>>>,
}

// ---------------------------------------------------------------------------
// Results
// ---------------------------------------------------------------------------

/// The evaluation results (Dart: the `evaluationResult` fields of the
/// elements and annotations, `isConstantEvaluated`, `isCycleFree`).
#[derive(Default, Debug, Clone)]
pub struct ConstantValues {
    /// Dart `VariableElementImpl.evaluationResult` of base elements.
    pub elements: IndexMap<ElementId, Constant>,
    /// Dart `ElementAnnotationImpl.evaluationResult`, by (unit index,
    /// `Annotation` node); `None` is Dart `null` after evaluation.
    pub annotations: IndexMap<(u32, NodeId), Option<Constant>>,
    /// Dart `ElementAnnotationImpl.additionalErrors`.
    pub annotation_errors: IndexMap<(u32, NodeId), Vec<Diagnostic>>,
    /// Dart `ConstructorElementImpl.isConstantEvaluated`.
    pub constructors_evaluated: IndexSet<ElementId>,
    /// The constructors with Dart `isCycleFree == false`.
    pub not_cycle_free: IndexSet<ElementId>,
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Dart `ConstantEvaluationEngine` (with the state that Dart keeps in the
/// elements).
pub struct ConstantEvaluationEngine<'a> {
    pub world: &'a WorldSnapshot,
    pub tp: &'a TypeProvider,
    /// The library under analysis.
    pub library: EId<LibraryElement>,
    /// Dart `_declaredVariables`.
    pub declared_variables: &'a DeclaredVariables,
    external: Option<&'a dyn ExternalUnits>,
    sink: NoopSink,
    units: RefCell<UnitRegistry<'a>>,
    pub values: RefCell<ConstantValues>,
}

/// Dart `_EnumConstant`.
struct EnumConstant {
    index: i64,
    name: String,
}

impl<'a> ConstantEvaluationEngine<'a> {
    /// An engine over the [units] of [library] (unit index = position).
    pub fn new(
        world: &'a WorldSnapshot,
        tp: &'a TypeProvider,
        library: EId<LibraryElement>,
        declared_variables: &'a DeclaredVariables,
        units: &'a [ResolvedUnit],
        external: Option<&'a dyn ExternalUnits>,
    ) -> ConstantEvaluationEngine<'a> {
        let mut registry = UnitRegistry::default();
        for unit in units {
            registry.add(UnitHandle::Borrowed(unit), library);
        }
        ConstantEvaluationEngine {
            world,
            tp,
            library,
            declared_variables,
            external,
            sink: NoopSink,
            units: RefCell::new(registry),
            values: RefCell::new(ConstantValues::default()),
        }
    }

    /// The number of units of the library under analysis and of the units
    /// loaded so far.
    pub fn unit_count(&self) -> u32 {
        self.units.borrow().units.len() as u32
    }

    /// The unit with the index [unit].
    pub fn unit(&self, unit: u32) -> UnitHandle<'a> {
        self.units.borrow().units[unit as usize].clone()
    }

    /// The library of the unit [unit].
    pub fn unit_library(&self, unit: u32) -> EId<LibraryElement> {
        self.units.borrow().libraries[unit as usize]
    }

    /// A lookup context with the local arena of [unit].
    pub fn ctx<'u>(&'u self, unit: &'u ResolvedUnit) -> Ctx<'u> {
        Ctx {
            world: self.world,
            current: None,
            local: Some(&unit.local),
            tp: self.tp,
            features: self.library_features(self.library),
            req: &self.sink,
        }
    }

    /// A lookup context without local arena.
    pub fn global_ctx(&self) -> Ctx<'_> {
        Ctx {
            world: self.world,
            current: None,
            local: None,
            tp: self.tp,
            features: self.library_features(self.library),
            req: &self.sink,
        }
    }

    /// Dart `library.featureSet`.
    pub fn library_features(&self, library: EId<LibraryElement>) -> &'a FeatureSet {
        let ctx = Ctx {
            world: self.world,
            current: None,
            local: None,
            tp: self.tp,
            features: &EMPTY_FEATURES,
            req: &NOOP_SINK,
        };
        // The element lives as long as the world snapshot.
        let features: &FeatureSet = &ctx.get(library).feature_set;
        // SAFETY-free: `ctx.get` returns a reference with the lifetime of the
        // world (`'a`).
        unsafe_extend(features)
    }

    /// The unit of the library fragment [fragment], loading it from the
    /// external units when it is not a unit of the library under analysis.
    fn unit_of_library_fragment(&self, fragment: FId<LibraryFragment>) -> Option<u32> {
        if let Some(&u) = self.units.borrow().by_fragment.get(&fragment) {
            return u;
        }
        let resolved = self.external.and_then(|e| e.resolved_unit(fragment));
        let mut registry = self.units.borrow_mut();
        let result = resolved.map(|unit| {
            let library = {
                let ctx = Ctx {
                    world: self.world,
                    current: None,
                    local: None,
                    tp: self.tp,
                    features: &EMPTY_FEATURES,
                    req: &NOOP_SINK,
                };
                ctx.fragment(unit.fragment).library
            };
            registry.add(UnitHandle::Shared(unit), library)
        });
        registry.by_fragment.insert(fragment, result);
        result
    }

    /// The unit that declares the element [e] (a local element: the unit of
    /// its local arena).
    fn unit_of_element(&self, e: ElementId) -> Option<u32> {
        let store = e.store();
        if store.is_local() {
            return self
                .units
                .borrow()
                .by_local_store
                .get(&store.raw())
                .copied();
        }
        let ctx = self.global_ctx();
        let data = ctx.element_data(e)?;
        let fragment = dartr_element::diagnostics::library_fragment_of(&ctx, data.first_fragment)?;
        self.unit_of_library_fragment(fragment)
    }

    /// The declaration node of [fragment] in [unit].
    fn declaration_in(&self, unit: u32, fragment: FragmentId) -> Option<NodeRef> {
        let index = {
            let registry = self.units.borrow();
            registry.declarations[unit as usize].clone()
        };
        let index = match index {
            Some(index) => index,
            None => {
                let handle = self.unit(unit);
                let mut map = IndexMap::new();
                for (node, &f) in handle.tables.declared_fragment.iter() {
                    map.entry(f).or_insert(node);
                }
                let index = Arc::new(map);
                self.units.borrow_mut().declarations[unit as usize] = Some(index.clone());
                index
            }
        };
        index.get(&fragment).map(|&node| NodeRef { unit, node })
    }

    /// The declaration node of the fragments of [e], first fragment first
    /// (Dart: the AST of the element, for example `constantInitializer`).
    fn declarations(&self, e: ElementId) -> Vec<NodeRef> {
        let Some(unit) = self.unit_of_element(e) else {
            return Vec::new();
        };
        let handle = self.unit(unit);
        let ctx = self.ctx(&handle);
        let mut result = Vec::new();
        let Some(data) = ctx.element_data(e) else {
            return result;
        };
        let mut fragment = Some(data.first_fragment);
        while let Some(f) = fragment {
            let decl = if f.store().is_local() {
                self.declaration_in(unit, f)
            } else {
                dartr_element::diagnostics::library_fragment_of(&ctx, f)
                    .and_then(|lf| self.unit_of_library_fragment(lf))
                    .and_then(|u| self.declaration_in(u, f))
            };
            if let Some(decl) = decl {
                result.push(decl);
            }
            fragment = ctx.fragment_data(f).and_then(|d| d.next_fragment);
        }
        result
    }

    /// The first declaration node of [e].
    fn declaration(&self, e: ElementId) -> Option<NodeRef> {
        self.declarations(e).into_iter().next()
    }

    // -----------------------------------------------------------------
    // Element facts
    // -----------------------------------------------------------------

    /// Dart `VariableElementImpl.constantInitializer`: the initializer of
    /// the last fragment that has a constant initializer (a const variable,
    /// a final instance field of a class with a const constructor, the
    /// default value of a formal parameter).
    pub fn constant_initializer(&self, e: ElementId) -> Option<NodeRef> {
        let ctx = self.global_ctx();
        let is_local = e.store().is_local();
        let mut result = None;
        for decl in self.declarations(e) {
            let handle = self.unit(decl.unit);
            let ast = &handle.ast;
            let linked = if is_local {
                true
            } else {
                // The linker decides which variables have a constant
                // initializer (`VariableFragmentImpl.constantInitializer`).
                handle
                    .tables
                    .declared_fragment
                    .get(decl.node)
                    .is_some_and(|&f| fragment_has_constant_initializer(&ctx, f))
            };
            if !linked {
                continue;
            }
            match ast.kind(decl.node) {
                NodeKind::VariableDeclaration => {
                    let node = Id::<VariableDeclaration>::from_raw(decl.node);
                    if let Some(init) = ast[node].initializer {
                        result = Some(decl.with(init));
                    }
                }
                NodeKind::RegularFormalParameter
                | NodeKind::FieldFormalParameter
                | NodeKind::SuperFormalParameter => {
                    if let Some(value) = formal_parameter_default_value(ast, decl.node) {
                        result = Some(decl.with(value));
                    }
                }
                _ => {}
            }
        }
        result
    }

    /// Dart `ConstructorElementImpl.constantInitializers`.
    pub fn constant_initializers(&self, constructor: ElementId) -> Vec<NodeRef> {
        let mut result = Vec::new();
        for decl in self.declarations(constructor) {
            let handle = self.unit(decl.unit);
            let ast = &handle.ast;
            if let Some(node) = ast.cast::<ConstructorDeclaration>(decl.node) {
                for &i in ast.list(ast[node].initializers) {
                    result.push(decl.with(i));
                }
            }
        }
        result
    }

    /// Dart `evaluationResult` of a variable or formal parameter
    /// (`SuperFormalParameterElementImpl.evaluationResult` falls back to the
    /// super constructor parameter).
    pub fn evaluation_result(&self, e: ElementId) -> Option<Constant> {
        if e.tag() == Tag::SuperFormalParameter && self.constant_initializer(e).is_none() {
            let ctx = self.global_ctx();
            let super_parameter = super_constructor_parameter(&ctx, e)?;
            let base = member::base_element(&ctx, super_parameter);
            return self.evaluation_result(base);
        }
        self.values.borrow().elements.get(&e).cloned()
    }

    fn set_evaluation_result(&self, e: ElementId, value: Constant) {
        self.values.borrow_mut().elements.insert(e, value);
    }

    /// Dart `ConstantEvaluationTarget.isConstantEvaluated`.
    pub fn is_constant_evaluated(&self, target: ConstantTarget) -> bool {
        match target {
            ConstantTarget::Element(e) if e.tag() == Tag::Constructor => {
                self.values.borrow().constructors_evaluated.contains(&e)
            }
            ConstantTarget::Element(e) => self.values.borrow().elements.contains_key(&e),
            ConstantTarget::Annotation(n) => self
                .values
                .borrow()
                .annotations
                .get(&(n.unit, n.node))
                .is_some_and(|v| v.is_some()),
        }
    }

    /// Dart `ConstructorElementImpl.isCycleFree`.
    pub fn is_cycle_free(&self, constructor: ElementId) -> bool {
        !self.values.borrow().not_cycle_free.contains(&constructor)
    }

    /// Dart `ElementAnnotationImpl.evaluationResult`.
    pub fn annotation_result(&self, annotation: NodeRef) -> Option<Constant> {
        self.values
            .borrow()
            .annotations
            .get(&(annotation.unit, annotation.node))
            .cloned()
            .flatten()
    }

    /// Dart `computeConstantValue()` of a variable: computes the constants
    /// that [e] depends on and [e], then returns its value.
    pub fn compute_constant_value_of(&self, e: ElementId) -> Option<DartObjectImpl> {
        if self.evaluation_result(e).is_none() {
            crate::constant::compute::compute_constants(self, &[ConstantTarget::Element(e)]);
        }
        match self.evaluation_result(e) {
            Some(Constant::Value(v)) => Some(v),
            _ => None,
        }
    }

    // -----------------------------------------------------------------
    // ConstantEvaluationEngine
    // -----------------------------------------------------------------

    /// Dart `computeConstantValue(constant)`.
    pub fn compute_constant_value(&self, constant: ConstantTarget) {
        match constant {
            ConstantTarget::Element(e) if is_formal_parameter(e) => {
                let Some(unit) = self.unit_of_element(e) else {
                    return;
                };
                let library = self.unit_library(unit);
                match self.constant_initializer(e) {
                    Some(default_value) => {
                        let visitor = ConstantVisitor::new(self, library, None);
                        let result = visitor.evaluate_constant(default_value);
                        self.set_evaluation_result(e, result);
                    }
                    None => {
                        let handle = self.unit(unit);
                        let ctx = self.ctx(&handle);
                        let ts = TypeSystem::new(ctx);
                        self.set_evaluation_result(e, Constant::Value(null_object(&ts)));
                    }
                }
            }
            ConstantTarget::Element(e) if e.tag() == Tag::Constructor => {
                let ctx = self.global_ctx();
                if is_const_constructor(&ctx, e) {
                    self.values.borrow_mut().constructors_evaluated.insert(e);
                }
            }
            ConstantTarget::Element(e) if e.is::<VariableElement>() => {
                self.compute_variable_value(e);
            }
            ConstantTarget::Element(_) => {}
            ConstantTarget::Annotation(node) => self.compute_annotation_value(node),
        }
    }

    fn compute_variable_value(&self, e: ElementId) {
        let Some(unit) = self.unit_of_element(e) else {
            return;
        };
        let library = self.unit_library(unit);
        let handle = self.unit(unit);
        let ctx = self.ctx(&handle);
        let ts = TypeSystem::new(ctx);

        // An enum constant: Dart evaluates the synthetic initializer
        // `E<typeArguments>.name(arguments)`, with errors at the
        // `EnumConstantDeclaration`.
        let enum_constant_decl = if is_enum_constant(&ctx, e) {
            self.declaration(e)
                .filter(|d| self.unit(d.unit).ast.kind(d.node) == NodeKind::EnumConstantDeclaration)
        } else {
            None
        };
        let is_enum_values = first_fragment_flags(&ctx, e)
            .contains(FragmentFlags::FIELD_FRAGMENT_IS_ORIGIN_ENUM_VALUES);

        let (dart_constant, initializer_type) = if let Some(decl) = enum_constant_decl {
            let visitor = ConstantVisitor::new(self, library, None);
            (visitor.evaluate_enum_constant(decl), None)
        } else if is_enum_values {
            (self.evaluate_enum_values(e), None)
        } else {
            let Some(constant_initializer) = self.constant_initializer(e) else {
                return;
            };
            let visitor = ConstantVisitor::new(self, library, None);
            let result = visitor.evaluate_constant(constant_initializer);
            let init_handle = self.unit(constant_initializer.unit);
            let ty = init_handle
                .tables
                .static_type
                .get(constant_initializer.node)
                .copied();
            (result, ty.or(Some(TypeId::INVALID)))
        };

        let mut dart_constant = dart_constant;
        if let Constant::Value(value) = &dart_constant {
            let mut value = value.clone();
            // Only check the type for truly const declarations (don't check
            // final fields with initializers, since their types may be
            // generic. The type of the final field will be checked later,
            // when the constructor is invoked).
            if is_const_variable(&ctx, e) {
                let variable_type = variable_type(&ctx, e);
                if !runtime_type_match(&ts, &value, variable_type) {
                    // If the static types are mismatched, an error would
                    // have already been reported.
                    if let Some(initializer_type) = initializer_type
                        && ts.is_assignable_to(initializer_type, variable_type, false)
                        && let Some(initializer) = self.constant_initializer(e)
                    {
                        let ih = self.unit(initializer.unit);
                        let diagnostic = diag::variable_type_mismatch(
                            &ts.display_type(value.ty),
                            &ts.display_type(variable_type),
                        );
                        self.set_evaluation_result(
                            e,
                            invalid_at_node(&ih.ast, initializer.node, diagnostic),
                        );
                        return;
                    }
                }
                // Associate with the variable.
                value = DartObjectImpl::for_variable(value, EId::<VariableElement>::from_raw(e));
            }
            if let Some(enum_constant) = enum_constant_of(&ctx, e) {
                value.update_enum_constant(&ts, enum_constant.index, &enum_constant.name);
            }
            dart_constant = Constant::Value(value);
        }
        self.set_evaluation_result(e, dart_constant);
    }

    /// The value of the synthetic `values` field of an enum: Dart evaluates
    /// the synthetic list literal `const <E>[c1, c2, ...]`.
    fn evaluate_enum_values(&self, e: ElementId) -> Constant {
        let ctx = self.global_ctx();
        let ts = TypeSystem::new(ctx);
        let Some(enclosing) = ctx.element_data(e).and_then(|d| d.enclosing) else {
            return Constant::Value(null_object(&ts));
        };
        let element_type = enclosing
            .cast::<InterfaceElement>()
            .map(|i| ctx.interface_this_type(i))
            .unwrap_or(TypeId::DYNAMIC);
        let mut elements = Vec::new();
        for c in crate::element_ext::enum_constants(&ctx, EId::from_raw(enclosing)) {
            match self.evaluation_result(c) {
                Some(Constant::Value(v)) => elements.push(v),
                Some(invalid @ Constant::Invalid(_)) => return invalid,
                None => {
                    return Constant::Invalid(Box::new(InvalidConstant::for_element(
                        &ctx,
                        e,
                        diag::invalid_constant(),
                    )));
                }
            }
        }
        let list_type = self.tp.list_type(&ctx, element_type);
        Constant::Value(DartObjectImpl::new(
            &ts,
            list_type,
            InstanceState::List(Arc::new(ListState::new(&ts, element_type, elements, false))),
        ))
    }

    fn compute_annotation_value(&self, node: NodeRef) {
        let handle = self.unit(node.unit);
        let ast = &handle.ast;
        let ctx = self.ctx(&handle);
        let library = self.unit_library(node.unit);
        let annotation = Id::<Annotation>::from_raw(node.node);
        let element = annotation_element(&handle, annotation);
        let key = (node.unit, node.node);
        let result = match element {
            Some(element) if is_property_accessor(member::base_element(&ctx, element)) => {
                // The annotation is a reference to a compile-time constant
                // variable. Just copy the evaluation result.
                member::variable(&ctx, element)
                    .map(|v| member::base_element(&ctx, v))
                    .and_then(|v| self.evaluation_result(v))
            }
            Some(element)
                if member::base_element(&ctx, element).tag() == Tag::Constructor
                    && is_const_constructor(&ctx, member::base_element(&ctx, element))
                    && ast[annotation].arguments.is_some() =>
            {
                let diagnostics = RefCell::new(Vec::new());
                let visitor = ConstantVisitor::new(self, library, Some(&diagnostics));
                let arguments = argument_nodes(ast, ast[annotation].arguments.unwrap())
                    .into_iter()
                    .map(|a| node.with(a))
                    .collect::<Vec<_>>();
                let type_arguments = ctx
                    .type_arguments(member::return_type(&ctx, element))
                    .to_vec();
                let result = self.evaluate_and_format_errors_in_constructor_call(
                    library,
                    node,
                    Some(type_arguments),
                    &arguments,
                    element,
                    &visitor,
                    None,
                );
                self.values
                    .borrow_mut()
                    .annotation_errors
                    .insert(key, diagnostics.into_inner());
                Some(result)
            }
            // This may happen for invalid code (e.g. failing to pass
            // arguments to an annotation which references a const
            // constructor). The error is detected elsewhere, so just
            // silently ignore it here.
            _ => None,
        };
        self.values.borrow_mut().annotations.insert(key, result);
    }

    /// Dart `computeDependencies(constant, callback)`.
    pub fn compute_dependencies(
        &self,
        constant: ConstantTarget,
        callback: &mut dyn FnMut(ConstantTarget),
    ) {
        if let ConstantTarget::Element(e) = constant {
            let ctx = self.global_ctx();
            if !e.store().is_local() && is_enum_constant(&ctx, e) {
                let enclosing = ctx.element_data(e).and_then(|d| d.enclosing);
                if let Some(enclosing) = enclosing
                    && enclosing.tag() == Tag::Enum
                {
                    let enum_name = ctx.element_data(enclosing).and_then(|d| d.name);
                    if enum_name.map(|n| ctx.name_str(n)) == Some("values") {
                        return;
                    }
                    if ctx.element_data(e).and_then(|d| d.name) == enum_name {
                        return;
                    }
                }
            }
        }

        match constant {
            ConstantTarget::Element(e) if e.tag() == Tag::Constructor => {
                self.compute_constructor_dependencies(e, callback);
            }
            ConstantTarget::Element(e) if e.is::<VariableElement>() => {
                let ctx = if e.store().is_local() {
                    None
                } else {
                    Some(self.global_ctx())
                };
                let is_values = ctx.as_ref().is_some_and(|ctx| {
                    first_fragment_flags(ctx, e)
                        .contains(FragmentFlags::FIELD_FRAGMENT_IS_ORIGIN_ENUM_VALUES)
                });
                if is_values {
                    // Dart: the synthetic `values` list literal references
                    // every enum constant.
                    let ctx = self.global_ctx();
                    if let Some(enclosing) = ctx.element_data(e).and_then(|d| d.enclosing) {
                        for c in crate::element_ext::enum_constants(&ctx, EId::from_raw(enclosing))
                        {
                            callback(ConstantTarget::Element(c));
                        }
                    }
                    return;
                }
                if let Some(decl) = self.enum_constant_declaration(e) {
                    // The synthetic initializer `E.name(arguments)`.
                    let handle = self.unit(decl.unit);
                    let ctx = self.ctx(&handle);
                    if let Some(constructor) = handle.tables.element.get(decl.node).copied() {
                        let base = member::base_element(&ctx, constructor);
                        if base.tag() == Tag::Constructor && is_const_constructor(&ctx, base) {
                            callback(ConstantTarget::Element(base));
                        }
                    }
                    let ast = &handle.ast;
                    let node = Id::<EnumConstantDeclaration>::from_raw(decl.node);
                    if let Some(arguments) = ast[node].arguments {
                        crate::constant::utilities::find_references(
                            self,
                            decl.with(arguments),
                            callback,
                        );
                    }
                    return;
                }
                if let Some(initializer) = self.constant_initializer(e) {
                    crate::constant::utilities::find_references(self, initializer, callback);
                }
            }
            ConstantTarget::Element(_) => {}
            ConstantTarget::Annotation(node) => {
                let handle = self.unit(node.unit);
                let ctx = self.ctx(&handle);
                let annotation = Id::<Annotation>::from_raw(node.node);
                match annotation_element(&handle, annotation) {
                    Some(element) if is_property_accessor(member::base_element(&ctx, element)) => {
                        // The annotation is a reference to a compile-time
                        // constant variable, so it depends on the variable.
                        if let Some(variable) = member::variable(&ctx, element) {
                            callback(ConstantTarget::Element(member::base_element(
                                &ctx, variable,
                            )));
                        }
                    }
                    Some(element)
                        if member::base_element(&ctx, element).tag() == Tag::Constructor =>
                    {
                        // The annotation is a constructor invocation, so it
                        // depends on the constructor.
                        callback(ConstantTarget::Element(member::base_element(&ctx, element)));
                    }
                    _ => {
                        // This could happen in the event of invalid code.
                        // The error will be reported at constant evaluation
                        // time.
                    }
                }
                if let Some(arguments) = handle.ast[annotation].arguments {
                    crate::constant::utilities::find_references(
                        self,
                        node.with(arguments),
                        callback,
                    );
                }
            }
        }
    }

    fn enum_constant_declaration(&self, e: ElementId) -> Option<NodeRef> {
        if e.store().is_local() || e.tag() != Tag::Field {
            return None;
        }
        let ctx = self.global_ctx();
        if !is_enum_constant(&ctx, e) {
            return None;
        }
        self.declaration(e)
            .filter(|d| self.unit(d.unit).ast.kind(d.node) == NodeKind::EnumConstantDeclaration)
    }

    fn compute_constructor_dependencies(
        &self,
        constant: ElementId,
        callback: &mut dyn FnMut(ConstantTarget),
    ) {
        let ctx = self.global_ctx();
        if !is_const_constructor(&ctx, constant) {
            return;
        }
        if let Some(redirected) = get_const_redirected_constructor(&ctx, ElemRef::Base(constant)) {
            callback(ConstantTarget::Element(member::base_element(
                &ctx, redirected,
            )));
            return;
        } else if is_factory_constructor(&ctx, constant) {
            // Factory constructor, but getConstRedirectedConstructor
            // returned null. This can happen if we're visiting one of the
            // special external const factory constructors in the SDK, or if
            // the code contains errors (such as delegating to a non-const
            // constructor, or delegating to a constructor that can't be
            // resolved). In any of these cases, we'll evaluate calls to this
            // constructor without having to refer to any other constants.
            // So we don't need to report any dependencies.
            return;
        }
        let mut default_super_invocation_needed = true;
        let initializers = self.constant_initializers(constant);
        for initializer in &initializers {
            let kind = self.unit(initializer.unit).ast.kind(initializer.node);
            if matches!(
                kind,
                NodeKind::SuperConstructorInvocation | NodeKind::RedirectingConstructorInvocation
            ) {
                default_super_invocation_needed = false;
            }
            crate::constant::utilities::find_references(self, *initializer, callback);
        }
        if default_super_invocation_needed {
            // No explicit superconstructor invocation found, so we need to
            // manually insert a reference to the implicit superconstructor.
            let return_type = member::return_type(&ctx, ElemRef::Base(constant));
            if let Some(superclass) = ctx.superclass(return_type)
                && !ctx.is_dart_core_object(superclass)
                && let Some(element) = ctx.interface_element(superclass)
                && let Some(unnamed) = lookup::get_named_constructor(&ctx, element, "new")
                && is_const_constructor(&ctx, unnamed.raw())
            {
                callback(ConstantTarget::Element(unnamed.raw()));
            }
        }
        if let Some(enclosing) = ctx.element_data(constant).and_then(|d| d.enclosing)
            && let Some(instance) = enclosing.cast::<dartr_element::InstanceElement>()
        {
            let fields = ctx.instance(instance).fields.clone();
            for field in fields {
                let f = field.raw();
                // Note: non-static const isn't allowed but we handle it
                // anyway so that we won't be confused by incorrect code.
                if (is_final_variable(&ctx, f) || is_const_variable(&ctx, f))
                    && !is_static_variable(&ctx, f)
                    && let Some(initializer) = self.constant_initializer(f)
                {
                    crate::constant::utilities::find_references(self, initializer, callback);
                }
            }
        }
        for parameter in ctx
            .executable(EId::<ExecutableElement>::from_raw(constant))
            .formal_params
            .clone()
        {
            callback(ConstantTarget::Element(parameter.raw()));
        }
    }

    /// Dart `evaluateAndFormatErrorsInConstructorCall`.
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_and_format_errors_in_constructor_call(
        &self,
        library: EId<LibraryElement>,
        node: NodeRef,
        type_arguments: Option<Vec<TypeId>>,
        arguments: &[NodeRef],
        constructor: ElemRef,
        constant_visitor: &ConstantVisitor<'_, 'a>,
        invocation: Option<Arc<ConstructorInvocationImpl>>,
    ) -> Constant {
        let result = InstanceCreationEvaluator::evaluate(
            self,
            library,
            node,
            constructor,
            type_arguments,
            arguments,
            constant_visitor,
            invocation,
            &IndexMap::new(),
        );
        let Constant::Invalid(result) = result else {
            return result;
        };

        // If we found an evaluation exception, report a context message
        // linking to where the exception was found.
        if result.is_runtime_exception {
            let handle = self.unit(node.unit);
            let formatted_message = result.locatable_diagnostic.to_diagnostic(0, 0).message;
            let library_path = {
                let ctx = self.global_ctx();
                let fragment = ctx.get(library).first_fragment();
                ctx.fragment(fragment).source.path.to_string()
            };
            let context_message = DiagnosticMessage {
                file_path: library_path,
                length: result.length,
                message: format!("The exception is '{formatted_message}' and occurs here."),
                offset: result.offset,
                url: None,
            };
            let mut messages = result.locatable_diagnostic.context_messages.clone();
            messages.push(context_message);
            let diagnostic = diag::const_eval_throws_exception().with_context_messages(messages);
            return invalid_at_node(&handle.ast, node.node, diagnostic);
        }
        Constant::Invalid(result)
    }

    /// Dart `generateCycleError(cycle, constant)`.
    pub fn generate_cycle_error(&self, constant: ConstantTarget) {
        match constant {
            ConstantTarget::Element(e) if e.is::<VariableElement>() => {
                let ctx = match self.unit_of_element(e) {
                    Some(_) if e.store().is_local() => None,
                    _ => Some(self.global_ctx()),
                };
                let value = match ctx {
                    Some(ctx) => InvalidConstant::for_element(
                        &ctx,
                        e,
                        diag::recursive_compile_time_constant(),
                    ),
                    None => {
                        let unit = self.unit_of_element(e).unwrap();
                        let handle = self.unit(unit);
                        let ctx = self.ctx(&handle);
                        InvalidConstant::for_element(
                            &ctx,
                            e,
                            diag::recursive_compile_time_constant(),
                        )
                    }
                };
                self.set_evaluation_result(e, Constant::Invalid(Box::new(value)));
            }
            _ => {
                // We don't report cycle errors on constructor declarations
                // here since there is nowhere to put the error information.
            }
        }
    }

    /// Marks [constructor] as not cycle free (Dart `isCycleFree = false`).
    pub fn set_not_cycle_free(&self, constructor: ElementId) {
        self.values.borrow_mut().not_cycle_free.insert(constructor);
    }
}

static EMPTY_FEATURES: std::sync::LazyLock<FeatureSet> =
    std::sync::LazyLock::new(FeatureSet::default);
static NOOP_SINK: NoopSink = NoopSink;

/// Extends the lifetime of a reference into the world snapshot.
fn unsafe_extend<'a>(features: &FeatureSet) -> &'a FeatureSet {
    // The library elements of a world snapshot are never dropped while the
    // snapshot lives, and the engine borrows the snapshot for `'a`.
    unsafe { &*(features as *const FeatureSet) }
}

impl<'a> UnitRegistry<'a> {
    fn add(&mut self, unit: UnitHandle<'a>, library: EId<LibraryElement>) -> u32 {
        let index = self.units.len() as u32;
        self.by_fragment.insert(unit.fragment, Some(index));
        self.by_local_store.insert(unit.local.store.id.raw(), index);
        self.units.push(unit);
        self.libraries.push(library);
        self.declarations.push(None);
        index
    }
}

// ---------------------------------------------------------------------------
// Element helpers
// ---------------------------------------------------------------------------

fn first_fragment_flags(ctx: &Ctx<'_>, e: ElementId) -> FragmentFlags {
    crate::element_ext::first_fragment_flags(ctx, e)
}

fn fragment_has_constant_initializer(ctx: &Ctx<'_>, f: FragmentId) -> bool {
    let Some(id) = f.cast::<dartr_element::VariableFragment>() else {
        return false;
    };
    ctx.store(f.store())
        .variable_fragment(id)
        .constant_initializer
        .is_some()
}

fn is_formal_parameter(e: ElementId) -> bool {
    matches!(
        e.tag(),
        Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter
    )
}

fn is_property_accessor(e: ElementId) -> bool {
    matches!(e.tag(), Tag::Getter | Tag::Setter)
}

/// Dart `ConstructorElement.isConst`.
pub fn is_const_constructor(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_flags(ctx, e).contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST)
}

/// Dart `ConstructorElement.isFactory`.
pub fn is_factory_constructor(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_flags(ctx, e).contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY)
}

fn is_const_variable(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_flags(ctx, e).contains(FragmentFlags::VARIABLE_FRAGMENT_IS_CONST)
}

fn is_final_variable(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_flags(ctx, e).contains(FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL)
}

fn is_static_variable(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_flags(ctx, e).contains(FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC)
}

fn is_enum_constant(ctx: &Ctx<'_>, e: ElementId) -> bool {
    e.tag() == Tag::Field && crate::element_ext::is_enum_constant(ctx, e)
}

fn variable_type(ctx: &Ctx<'_>, e: ElementId) -> TypeId {
    member::type_(ctx, ElemRef::Base(e))
}

fn element_name(ctx: &Ctx<'_>, e: ElementId) -> Option<String> {
    ctx.element_data(e)
        .and_then(|d| d.name)
        .map(|n| ctx.name_str(n).to_string())
}

/// Dart `_enumConstant(element)`.
fn enum_constant_of(ctx: &Ctx<'_>, e: ElementId) -> Option<EnumConstant> {
    if e.store().is_local() || !is_enum_constant(ctx, e) {
        return None;
    }
    let enclosing = ctx.element_data(e)?.enclosing?;
    if enclosing.tag() != Tag::Enum {
        return None;
    }
    let constants = crate::element_ext::enum_constants(ctx, EId::from_raw(enclosing));
    let index = constants.iter().position(|&c| c == e)?;
    Some(EnumConstant {
        index: index as i64,
        name: element_name(ctx, e).unwrap_or_default(),
    })
}

/// Dart `SuperFormalParameterElementImpl.superConstructorParameter`.
pub fn super_constructor_parameter(ctx: &Ctx<'_>, e: ElementId) -> Option<ElemRef> {
    let enclosing = ctx.element_data(e)?.enclosing?;
    if enclosing.tag() != Tag::Constructor {
        return None;
    }
    let constructor = ctx.get(EId::<dartr_element::ConstructorElement>::from_raw(
        enclosing,
    ));
    let super_constructor = constructor.super_constructor.get()?;
    let super_parameters = member::formal_parameters(ctx, super_constructor);
    let parameter = ctx.get(EId::<FormalParameterElement>::from_raw(e));
    if parameter.kind.is_named() {
        let name = element_name(ctx, e);
        super_parameters.into_iter().find(|&p| {
            let base = member::base_element(ctx, p);
            ctx.get(EId::<FormalParameterElement>::from_raw(base))
                .kind
                .is_named()
                && element_name(ctx, base) == name
        })
    } else {
        let index = ctx
            .executable(EId::<ExecutableElement>::from_raw(enclosing))
            .formal_params
            .iter()
            .filter(|p| p.raw().tag() == Tag::SuperFormalParameter)
            .position(|p| p.raw() == e)?;
        let positional: Vec<ElemRef> = super_parameters
            .into_iter()
            .filter(|&p| {
                let base = member::base_element(ctx, p);
                ctx.get(EId::<FormalParameterElement>::from_raw(base))
                    .kind
                    .is_positional()
            })
            .collect();
        positional.get(index).copied()
    }
}

/// Dart `getConstRedirectedConstructor(constructor)`: if [constructor]
/// redirects to another const constructor, the const constructor it
/// redirects to.
pub fn get_const_redirected_constructor(ctx: &Ctx<'_>, constructor: ElemRef) -> Option<ElemRef> {
    let base = member::base_element(ctx, constructor);
    if !is_factory_constructor(ctx, base) {
        return None;
    }
    let enclosing = ctx.element_data(base)?.enclosing?;
    if enclosing == ctx.tp.symbol_element().raw() {
        // The dart:core.Symbol has a const factory constructor that
        // redirects to dart:_internal.Symbol. That in turn redirects to an
        // external const constructor, which we won't be able to evaluate.
        // So stop following the chain of redirections at dart:core.Symbol,
        // and let [evaluateInstanceCreationExpression] handle it specially.
        return None;
    }
    let redirected = ctx
        .get(EId::<dartr_element::ConstructorElement>::from_raw(base))
        .redirected_constructor
        .get()?;
    // Dart `ConstructorMember.redirectedConstructor` substitutes.
    let redirected = match constructor {
        ElemRef::Base(_) => redirected,
        ElemRef::Member(_) => {
            member::substitute(ctx, redirected, &member::substitution(ctx, constructor))
        }
    };
    if !is_const_constructor(ctx, member::base_element(ctx, redirected)) {
        // Delegating to a non-const constructor--this is not allowed (and
        // is checked elsewhere).
        return None;
    }
    Some(redirected)
}

/// The element of an annotation (Dart `ElementAnnotationImpl.element`:
/// `annotation.element`).
fn annotation_element(unit: &ResolvedUnit, annotation: Id<Annotation>) -> Option<ElemRef> {
    unit.tables.element.get(annotation).copied()
}

/// The value of the default clause of a formal parameter node.
fn formal_parameter_default_value(ast: &Ast, node: NodeId) -> Option<NodeId> {
    let clause = match ast.kind(node) {
        NodeKind::RegularFormalParameter => {
            ast[Id::<RegularFormalParameter>::from_raw(node)].default_clause
        }
        NodeKind::FieldFormalParameter => {
            ast[Id::<FieldFormalParameter>::from_raw(node)].default_clause
        }
        NodeKind::SuperFormalParameter => {
            ast[Id::<SuperFormalParameter>::from_raw(node)].default_clause
        }
        _ => None,
    }?;
    Some(ast[clause].value.raw())
}

/// The argument nodes of an argument list.
fn argument_nodes(ast: &Ast, list: Id<ArgumentList>) -> Vec<NodeId> {
    ast.list_raw(ast[list].arguments).to_vec()
}

/// Dart `TypeSystemImpl.runtimeTypeMatch(obj, type)` (`RuntimeExtensions`).
pub fn runtime_type_match(ts: &TypeSystem<'_>, obj: &DartObjectImpl, ty: TypeId) -> bool {
    let ty = ts.extension_type_erasure(ty);
    ts.is_subtype_of(obj.ty, ty)
}

/// Dart `ConstantEvaluationEngine._nullObject`.
fn null_object(ts: &TypeSystem<'_>) -> DartObjectImpl {
    DartObjectImpl::new(
        ts,
        ts.ctx.tp.null_type(),
        InstanceState::Null(NullState::NULL_STATE),
    )
}

/// Dart `ConstantEvaluationEngine._unresolvedObject`.
fn unresolved_object(ts: &TypeSystem<'_>, default_type: TypeId) -> DartObjectImpl {
    DartObjectImpl::new(
        ts,
        default_type,
        InstanceState::Null(NullState { is_invalid: true }),
    )
}

trait DisplayType {
    fn display_type(&self, t: TypeId) -> String;
}

impl DisplayType for TypeSystem<'_> {
    fn display_type(&self, t: TypeId) -> String {
        dartr_element::type_display_string_with(
            &self.ctx,
            t,
            dartr_element::DisplayOptions::default(),
        )
    }
}

// ---------------------------------------------------------------------------
// InvalidConstant constructors that take AST entities
// ---------------------------------------------------------------------------

/// Dart `InvalidConstant.forEntity(entity: node, ...)`.
fn invalid_at_node(ast: &Ast, node: NodeId, diagnostic: LocatableDiagnostic) -> Constant {
    Constant::Invalid(Box::new(invalid_constant_at_node(ast, node, diagnostic)))
}

fn invalid_constant_at_node(
    ast: &Ast,
    node: NodeId,
    diagnostic: LocatableDiagnostic,
) -> InvalidConstant {
    InvalidConstant::new(
        i64::from(ast.offset(node)),
        i64::from(ast.length(node)),
        diagnostic,
    )
}

/// Dart `InvalidConstant.forEntity(entity: token, ...)`.
fn invalid_constant_at_token(
    ast: &Ast,
    token: dartr_syntax::TokenId,
    diagnostic: LocatableDiagnostic,
) -> InvalidConstant {
    let offset = ast.tokens.offset(token);
    let end = ast_ext::token_end(ast, token);
    InvalidConstant::new(i64::from(offset), i64::from(end - offset), diagnostic)
}

/// Dart `InvalidConstant.copyWithEntity(other:, entity: node)`.
fn copy_with_entity(other: &InvalidConstant, ast: &Ast, node: NodeId) -> Constant {
    let mut result = invalid_constant_at_node(ast, node, other.locatable_diagnostic.clone());
    result.avoid_reporting = other.avoid_reporting;
    result.is_unresolved = other.is_unresolved;
    result.is_runtime_exception = other.is_runtime_exception;
    Constant::Invalid(Box::new(result))
}

/// Dart `InvalidConstant.genericError(node:, isUnresolved:)`.
fn generic_error(ast: &Ast, node: NodeId, is_unresolved: bool) -> Constant {
    let parent = ast.parent(node);
    let parent2 = parent.and_then(|p| ast.parent(p));
    let diagnostic = match (parent.map(|p| ast.kind(p)), parent2) {
        (Some(NodeKind::ArgumentList), Some(p2))
            if ast
                .cast::<InstanceCreationExpression>(p2)
                .is_some_and(|ice| ast_ext::instance_creation_is_const(ast, ice)) =>
        {
            diag::const_with_non_constant_argument()
        }
        _ => diag::invalid_constant(),
    };
    let mut result = invalid_constant_at_node(ast, node, diagnostic);
    result.is_unresolved = is_unresolved;
    Constant::Invalid(Box::new(result))
}

fn with_flags(mut c: InvalidConstant, avoid_reporting: bool, is_unresolved: bool) -> Constant {
    c.avoid_reporting = avoid_reporting;
    c.is_unresolved = is_unresolved;
    Constant::Invalid(Box::new(c))
}

/// Dart `exception is EvaluationException` → `InvalidConstant.forEntity`.
fn from_exception(ast: &Ast, node: NodeId, e: EvaluationException, keep_runtime: bool) -> Constant {
    let mut result = invalid_constant_at_node(ast, node, e.locatable_diagnostic);
    if keep_runtime {
        result.is_runtime_exception = e.is_runtime_exception;
    }
    Constant::Invalid(Box::new(result))
}

fn same_code(d: &LocatableDiagnostic, other: &LocatableDiagnostic) -> bool {
    std::ptr::eq(d.code, other.code)
}

// ---------------------------------------------------------------------------
// ConstantVisitor
// ---------------------------------------------------------------------------

/// Dart `ConstantVisitor`: evaluates constant expressions.
pub struct ConstantVisitor<'e, 'a> {
    engine: &'e ConstantEvaluationEngine<'a>,
    /// Dart `_library`.
    library: EId<LibraryElement>,
    /// Dart `_lexicalEnvironment` (base formal parameter → value).
    lexical_environment: Option<&'e IndexMap<ElementId, DartObjectImpl>>,
    /// Dart `_lexicalTypeEnvironment`.
    lexical_type_environment: Option<&'e IndexMap<ElementId, TypeId>>,
    /// Dart `_substitution`.
    substitution: Option<MapSubstitution>,
    /// Dart `_diagnosticReporter` (`None`: the diagnostics are dropped).
    diagnostics: Option<&'e RefCell<Vec<Diagnostic>>>,
}

impl<'e, 'a> ConstantVisitor<'e, 'a> {
    /// Dart `ConstantVisitor(engine, library, reporter)`.
    pub fn new(
        engine: &'e ConstantEvaluationEngine<'a>,
        library: EId<LibraryElement>,
        diagnostics: Option<&'e RefCell<Vec<Diagnostic>>>,
    ) -> ConstantVisitor<'e, 'a> {
        ConstantVisitor {
            engine,
            library,
            lexical_environment: None,
            lexical_type_environment: None,
            substitution: None,
            diagnostics,
        }
    }

    fn features(&self) -> &'a FeatureSet {
        self.engine.library_features(self.library)
    }

    fn report(&self, unit: u32, invalid: &InvalidConstant) {
        if let Some(diagnostics) = self.diagnostics {
            let _ = unit;
            diagnostics
                .borrow_mut()
                .push(invalid.locatable_diagnostic.to_diagnostic(
                    invalid.offset.max(0) as usize,
                    invalid.length.max(0) as usize,
                ));
        }
    }

    /// Dart `evaluateAndReportInvalidConstant(node)`.
    pub fn evaluate_and_report_invalid_constant(&self, node: NodeRef) -> Constant {
        let result = self.evaluate_constant(node);
        if let Constant::Invalid(invalid) = &result
            && !invalid.avoid_reporting
        {
            self.report(node.unit, invalid);
        }
        result
    }

    /// Dart `evaluateConstant(node)`.
    pub fn evaluate_constant(&self, n: NodeRef) -> Constant {
        let u = self.engine.unit(n.unit);
        let ctx = self.engine.ctx(&u);
        let ts = TypeSystem::new(ctx);
        let ast = &u.ast;
        let node = n.node;
        let tp = self.engine.tp;
        match ast.kind(node) {
            NodeKind::AdjacentStrings => self.visit_adjacent_strings(n, &ts, Id::from_raw(node)),
            NodeKind::AsExpression => {
                let e = &ast[Id::<AsExpression>::from_raw(node)];
                let expression = match self.evaluate_constant(n.with(e.expression)) {
                    Constant::Value(v) => v,
                    invalid => return invalid,
                };
                let ty = match self.evaluate_constant(n.with(e.type_)) {
                    Constant::Value(v) => v,
                    invalid => return invalid,
                };
                match expression.cast_to_type(&ts, &ty) {
                    Ok(v) => Constant::Value(v),
                    Err(e) => from_exception(ast, node, e, false),
                }
            }
            NodeKind::BinaryExpression => self.visit_binary_expression(n, &u, &ts),
            NodeKind::BooleanLiteral => Constant::Value(DartObjectImpl::new(
                &ts,
                tp.bool_type(),
                InstanceState::Bool(BoolState::from(
                    ast[Id::<BooleanLiteral>::from_raw(node)].value,
                )),
            )),
            NodeKind::ConditionalExpression => self.visit_conditional_expression(n, &u, &ts),
            NodeKind::ConstructorReference => self.visit_constructor_reference(n, &u, &ts),
            NodeKind::DotShorthandConstructorInvocation => {
                self.visit_dot_shorthand_constructor_invocation(n, &u, &ts)
            }
            NodeKind::DotShorthandInvocation => self.invalid_constant_for_method_invocation(n, &u),
            NodeKind::DotShorthandPropertyAccess => {
                let property_name =
                    ast[Id::<DotShorthandPropertyAccess>::from_raw(node)].property_name;
                self.get_constant_value(
                    n,
                    &u,
                    &ts,
                    node,
                    Some(node),
                    Some(property_name),
                    u.tables.element.get(property_name).copied(),
                    None,
                )
            }
            NodeKind::DoubleLiteral => Constant::Value(DartObjectImpl::new(
                &ts,
                tp.double_type(),
                InstanceState::Double(DoubleState::new(Some(
                    ast[Id::<DoubleLiteral>::from_raw(node)].value,
                ))),
            )),
            NodeKind::FunctionReference => self.visit_function_reference(n, &u, &ts),
            NodeKind::GenericFunctionType | NodeKind::RecordTypeAnnotation => {
                let ty = u
                    .tables
                    .annotation_type
                    .get(node)
                    .copied()
                    .unwrap_or(TypeId::DYNAMIC);
                Constant::Value(DartObjectImpl::new(
                    &ts,
                    tp.type_type(),
                    InstanceState::Type(TypeState::new(&ts, Some(ty))),
                ))
            }
            NodeKind::InstanceCreationExpression => {
                self.visit_instance_creation_expression(n, &u, &ts)
            }
            NodeKind::IntegerLiteral => {
                let value = ast[Id::<IntegerLiteral>::from_raw(node)].value;
                if u.tables.static_type.get(node).copied() == Some(tp.double_type()) {
                    return Constant::Value(DartObjectImpl::new(
                        &ts,
                        tp.double_type(),
                        InstanceState::Double(DoubleState::new(value.map(|v| v as f64))),
                    ));
                }
                Constant::Value(DartObjectImpl::new(
                    &ts,
                    tp.int_type(),
                    InstanceState::Int(IntState::new(value)),
                ))
            }
            NodeKind::InterpolationExpression => {
                let expression = ast[Id::<InterpolationExpression>::from_raw(node)].expression;
                let result = match self.evaluate_constant(n.with(expression)) {
                    Constant::Value(v) => v,
                    invalid => return invalid,
                };
                if !result.is_bool_num_string_or_null() {
                    return invalid_at_node(ast, node, diag::const_eval_type_bool_num_string());
                }
                match result.perform_to_string(&ts) {
                    Ok(v) => Constant::Value(v),
                    Err(e) => from_exception(ast, node, e, false),
                }
            }
            NodeKind::InterpolationString => Constant::Value(DartObjectImpl::new(
                &ts,
                tp.string_type(),
                InstanceState::String(StringState::new(
                    &*ast[Id::<InterpolationString>::from_raw(node)].value,
                )),
            )),
            NodeKind::IsExpression => {
                let e = &ast[Id::<IsExpression>::from_raw(node)];
                let expression = match self.evaluate_constant(n.with(e.expression)) {
                    Constant::Value(v) => v,
                    invalid => return invalid,
                };
                let ty = match self.evaluate_constant(n.with(e.type_)) {
                    Constant::Value(v) => v,
                    invalid => return invalid,
                };
                // Dart `DartObjectComputer.typeTest`.
                let result = expression.has_type(&ts, &ty).and_then(|result| {
                    if e.not_operator.is_some() {
                        result.logical_not(&ts)
                    } else {
                        Ok(result)
                    }
                });
                match result {
                    Ok(v) => Constant::Value(v),
                    Err(e) => from_exception(ast, node, e, false),
                }
            }
            NodeKind::ListLiteral => self.visit_list_literal(n, &u, &ts),
            NodeKind::MethodInvocation => self.visit_method_invocation(n, &u, &ts),
            NodeKind::NamedArgument => {
                let e = ast[Id::<NamedArgument>::from_raw(node)].argument_expression;
                self.evaluate_constant(n.with(e))
            }
            NodeKind::NamedType => self.visit_named_type(n, &u, &ts),
            NodeKind::NullLiteral => Constant::Value(null_object(&ts)),
            NodeKind::ParenthesizedExpression => {
                let e = ast[Id::<ParenthesizedExpression>::from_raw(node)].expression;
                self.evaluate_constant(n.with(e))
            }
            NodeKind::PrefixedIdentifier => self.visit_prefixed_identifier(n, &u, &ts),
            NodeKind::PrefixExpression => self.visit_prefix_expression(n, &u, &ts),
            NodeKind::PropertyAccess => self.visit_property_access(n, &u, &ts),
            NodeKind::RecordLiteral => self.visit_record_literal(n, &u, &ts),
            NodeKind::SetOrMapLiteral => self.visit_set_or_map_literal(n, &u, &ts),
            NodeKind::SimpleIdentifier => self.visit_simple_identifier(n, &u, &ts),
            NodeKind::SimpleStringLiteral => Constant::Value(DartObjectImpl::new(
                &ts,
                tp.string_type(),
                InstanceState::String(StringState::new(
                    &*ast[Id::<SimpleStringLiteral>::from_raw(node)].value,
                )),
            )),
            NodeKind::StringInterpolation => {
                let elements = ast
                    .list_raw(ast[Id::<StringInterpolation>::from_raw(node)].elements)
                    .to_vec();
                self.concatenate_nodes(n, &u, &ts, &elements)
            }
            NodeKind::SymbolLiteral => {
                let components =
                    ast.token_list(ast[Id::<SymbolLiteral>::from_raw(node)].components);
                let text = components
                    .iter()
                    .map(|&t| ast.tokens.lexeme(t))
                    .collect::<Vec<_>>()
                    .join(".");
                Constant::Value(DartObjectImpl::new(
                    &ts,
                    tp.symbol_type(),
                    InstanceState::Symbol(SymbolState::new(Some(Arc::from(text.as_str())))),
                ))
            }
            NodeKind::TypeLiteral => {
                let ty = ast[Id::<TypeLiteral>::from_raw(node)].type_;
                self.evaluate_constant(n.with(ty))
            }
            // Dart `visitNode`.
            _ => generic_error(ast, node, false),
        }
    }

    fn visit_adjacent_strings(
        &self,
        n: NodeRef,
        ts: &TypeSystem<'_>,
        node: Id<AdjacentStrings>,
    ) -> Constant {
        // Adjacent string literals are common in generated code (for
        // example, large data tables). Buffering them avoids repeatedly
        // materializing larger intermediate strings.
        let u = self.engine.unit(n.unit);
        let strings = u.ast.list_raw(u.ast[node].strings).to_vec();
        let mut buffer = String::new();
        for &string in &strings {
            let constant = match self.evaluate_constant(n.with(string)) {
                Constant::Value(v) => v,
                invalid => return invalid,
            };
            match &constant.state {
                InstanceState::String(StringState { value: Some(v), .. }) => buffer.push_str(v),
                _ => return self.concatenate_nodes(n, &u, ts, &strings),
            }
        }
        Constant::Value(DartObjectImpl::new(
            ts,
            self.engine.tp.string_type(),
            InstanceState::String(StringState::new(buffer.as_str())),
        ))
    }

    fn visit_binary_expression(
        &self,
        n: NodeRef,
        u: &ResolvedUnit,
        ts: &TypeSystem<'_>,
    ) -> Constant {
        let ast = &u.ast;
        let node = Id::<BinaryExpression>::from_raw(n.node);
        if let Some(error) = self.extension_operator_error(u, ts, n.node) {
            return error;
        }
        let e = &ast[node];
        let operator_type = ast.tokens.ty(e.operator);
        let left_result = match self.evaluate_constant(n.with(e.left_operand)) {
            Constant::Value(v) => v,
            invalid => return invalid,
        };
        let right = n.with(e.right_operand);

        // Used for the [DartObjectComputer], which will handle any
        // exceptions.
        let compute_right_operand = || -> Result<DartObjectImpl, EvaluationException> {
            match self.evaluate_constant(right) {
                Constant::Value(v) => Ok(v),
                Constant::Invalid(invalid) => {
                    Err(EvaluationException::new(invalid.locatable_diagnostic))
                }
            }
        };

        // Evaluate lazy operators.
        if operator_type == TokenType::AMPERSAND_AMPERSAND {
            if left_result.to_bool_value() == Some(false)
                && let Some(error) = self.report_not_potential_constants(right)
            {
                return error;
            }
            return match left_result.lazy_and(ts, compute_right_operand) {
                Ok(v) => Constant::Value(v),
                Err(e) => from_exception(ast, n.node, e, false),
            };
        } else if operator_type == TokenType::BAR_BAR {
            if left_result.to_bool_value() == Some(true)
                && let Some(error) = self.report_not_potential_constants(right)
            {
                return error;
            }
            return match left_result.lazy_or(ts, compute_right_operand) {
                Ok(v) => Constant::Value(v),
                Err(e) => from_exception(ast, n.node, e, false),
            };
        } else if operator_type == TokenType::QUESTION_QUESTION {
            if !left_result.is_null()
                && let Some(error) = self.report_not_potential_constants(right)
            {
                return error;
            }
            // Dart `lazyQuestionQuestion`.
            if left_result.is_null() {
                return self.evaluate_constant(right);
            }
            return Constant::Value(left_result);
        }

        // Evaluate eager operators.
        let right_result = match self.evaluate_constant(right) {
            Constant::Value(v) => v,
            invalid => return invalid,
        };
        let features = self.features();
        let l = &left_result;
        let r = &right_result;
        let result = match operator_type {
            TokenType::AMPERSAND => l.eager_and(ts, r),
            TokenType::BANG_EQ => l.not_equal(ts, features, r),
            TokenType::BAR => l.eager_or(ts, r),
            TokenType::CARET => l.eager_xor(ts, r),
            TokenType::EQ_EQ => l.equal_equal(ts, features, r),
            TokenType::GT => l.greater_than(ts, r),
            TokenType::GT_EQ => l.greater_than_or_equal(ts, r),
            TokenType::GT_GT => l.shift_right(ts, r),
            TokenType::GT_GT_GT => l.logical_shift_right(ts, r),
            TokenType::LT => l.less_than(ts, r),
            TokenType::LT_EQ => l.less_than_or_equal(ts, r),
            TokenType::LT_LT => l.shift_left(ts, r),
            TokenType::MINUS => l.minus(ts, r),
            TokenType::PERCENT => l.remainder(ts, r),
            TokenType::PLUS => l.add(ts, r),
            TokenType::STAR => l.times(ts, r),
            TokenType::SLASH => l.divide(ts, r),
            TokenType::TILDE_SLASH => {
                return match l.integer_divide(ts, r) {
                    Ok(v) => Constant::Value(v),
                    Err(e) => from_exception(ast, n.node, e, true),
                };
            }
            // TODO(srawlins): Use a specific error code.
            _ => return generic_error(ast, n.node, false),
        };
        match result {
            Ok(v) => Constant::Value(v),
            Err(e) => from_exception(ast, n.node, e, false),
        }
    }

    /// The operator of [node] is an extension or extension type member.
    fn extension_operator_error(
        &self,
        u: &ResolvedUnit,
        ts: &TypeSystem<'_>,
        node: NodeId,
    ) -> Option<Constant> {
        let element = u.tables.element.get(node).copied()?;
        let container = member::enclosing_element(&ts.ctx, element)?;
        match container.tag() {
            Tag::Extension => Some(invalid_at_node(
                &u.ast,
                node,
                diag::const_eval_extension_method(),
            )),
            Tag::ExtensionType => Some(invalid_at_node(
                &u.ast,
                node,
                diag::const_eval_extension_type_method(),
            )),
            _ => None,
        }
    }

    fn visit_conditional_expression(
        &self,
        n: NodeRef,
        u: &ResolvedUnit,
        ts: &TypeSystem<'_>,
    ) -> Constant {
        let ast = &u.ast;
        let e = &ast[Id::<ConditionalExpression>::from_raw(n.node)];
        let condition = e.condition;
        let condition_constant = match self.evaluate_constant(n.with(condition)) {
            Constant::Value(v) => v,
            invalid => return invalid,
        };
        if !condition_constant.is_bool() {
            return invalid_at_node(ast, condition.raw(), diag::const_eval_type_bool());
        }
        let condition_constant = match condition_constant.convert_to_bool(ts) {
            Ok(v) => v,
            Err(err) => return from_exception(ast, condition.raw(), err, false),
        };
        match condition_constant.to_bool_value() {
            Some(true) => {
                if let Some(error) = self.report_not_potential_constants(n.with(e.else_expression))
                {
                    return error;
                }
                self.evaluate_constant(n.with(e.then_expression))
            }
            Some(false) => {
                if let Some(error) = self.report_not_potential_constants(n.with(e.then_expression))
                {
                    return error;
                }
                self.evaluate_constant(n.with(e.else_expression))
            }
            None => {
                let then_constant = self.evaluate_constant(n.with(e.then_expression));
                if let Constant::Invalid(_) = then_constant {
                    return then_constant;
                }
                let else_constant = self.evaluate_constant(n.with(e.else_expression));
                if let Constant::Invalid(_) = else_constant {
                    return else_constant;
                }
                let ty = u
                    .tables
                    .static_type
                    .get(n.node)
                    .copied()
                    .unwrap_or(TypeId::DYNAMIC);
                Constant::Value(DartObjectImpl::valid_with_unknown_value(ts, ty))
            }
        }
    }

    fn visit_constructor_reference(
        &self,
        n: NodeRef,
        u: &ResolvedUnit,
        ts: &TypeSystem<'_>,
    ) -> Constant {
        let ast = &u.ast;
        let ctx = &ts.ctx;
        let constructor_function_type = u
            .tables
            .static_type
            .get(n.node)
            .copied()
            .unwrap_or(TypeId::INVALID);
        let TypeKind::Function(function) = *ctx.ty(constructor_function_type) else {
            return invalid_at_node(ast, n.node, diag::invalid_constant());
        };
        let type_arguments = ctx.type_arguments(function.ret).to_vec();
        // The result is already instantiated during resolution;
        // [_dartObjectComputer.typeInstantiate] is unnecessary.
        let constructor_name = ast[Id::<ConstructorReference>::from_raw(n.node)].constructor_name;
        let named_type = ast[constructor_name].type_;
        let type_element = u
            .tables
            .element
            .get(named_type)
            .map(|&e| member::base_element(ctx, e));

        let mut via_type_alias = None;
        if let Some(type_element) = type_element
            && type_element.tag() == Tag::TypeAlias
            && !ctx.list(function.type_params).is_empty()
            && !crate::constant::utilities::is_proper_rename(
                ctx,
                EId::<TypeAliasElement>::from_raw(type_element),
            )
        {
            // The type alias is not a proper rename of the aliased class, so
            // the constructor tear-off is distinct from the associated
            // constructor function of the aliased class.
            via_type_alias = Some(EId::<TypeAliasElement>::from_raw(type_element));
        }

        let constructor_element = u
            .tables
            .element
            .get(constructor_name)
            .map(|&e| member::base_element(ctx, e))
            .filter(|e| e.tag() == Tag::Constructor);
        let Some(constructor_element) = constructor_element else {
            return invalid_at_node(ast, n.node, diag::invalid_constant());
        };
        Constant::Value(DartObjectImpl::new(
            ts,
            constructor_function_type,
            InstanceState::Function(Arc::new(FunctionState::new(
                EId::<ExecutableElement>::from_raw(constructor_element),
                Some(type_arguments),
                via_type_alias,
            ))),
        ))
    }

    fn visit_dot_shorthand_constructor_invocation(
        &self,
        n: NodeRef,
        u: &ResolvedUnit,
        ts: &TypeSystem<'_>,
    ) -> Constant {
        let ast = &u.ast;
        let node = Id::<DotShorthandConstructorInvocation>::from_raw(n.node);
        // This check is used by the [ConstantVerifier] to check for constant
        // default parameters and other instances where the invocation must
        // be constant.
        if !dot_shorthand_constructor_invocation_is_const(ast, node) {
            return generic_error(ast, n.node, false);
        }
        let constructor_name = ast[node].constructor_name;
        let constructor = u.tables.element.get(constructor_name).copied();
        if let Some(constructor) = constructor
            && member::base_element(&ts.ctx, constructor).tag() == Tag::Constructor
        {
            let type_arguments = ts
                .ctx
                .type_arguments(member::return_type(&ts.ctx, constructor))
                .to_vec();
            let arguments: Vec<NodeRef> = argument_nodes(ast, ast[node].argument_list)
                .into_iter()
                .map(|a| n.with(a))
                .collect();
            return self.engine.evaluate_and_format_errors_in_constructor_call(
                self.library,
                n,
                Some(type_arguments),
                &arguments,
                constructor,
                self,
                None,
            );
        }
        // Couldn't resolve the constructor so we can't compute a value. No
        // problem - the error has already been reported.
        invalid_at_node(ast, n.node, diag::invalid_constant())
    }

    fn visit_function_reference(
        &self,
        n: NodeRef,
        u: &ResolvedUnit,
        ts: &TypeSystem<'_>,
    ) -> Constant {
        let ast = &u.ast;
        let ctx = &ts.ctx;
        let node = Id::<FunctionReference>::from_raw(n.node);
        let function = ast[node].function;
        let function_result = match self.evaluate_constant(n.with(function)) {
            Constant::Value(v) => v,
            invalid => return invalid,
        };

        // Report an error if any of the _inferred_ type argument types refer
        // to a type parameter. If, however, `node.typeArguments` is not
        // `null`, then any type parameters contained therein are reported as
        // non-constant in [ConstantVerifier].
        let type_argument_list = ast[node].type_arguments;
        if type_argument_list.is_none()
            && let Some(type_argument_types) = u.tables.type_arg_types.get(n.node)
        {
            let any = ctx.list(*type_argument_types).iter().any(|&ty| {
                let ty = match *ctx.ty(ty) {
                    TypeKind::TypeParameter { param, .. } => self
                        .lexical_type_environment
                        .and_then(|env| env.get(&param.raw()).copied())
                        .unwrap_or(ty),
                    _ => ty,
                };
                has_type_parameter_reference(ctx, ty)
            });
            if any {
                return invalid_at_node(
                    ast,
                    n.node,
                    diag::const_with_type_parameters_function_tearoff(),
                );
            }
        }

        let Some(type_argument_list) = type_argument_list else {
            return Constant::Value(self.instantiate_function_type(u, ts, n.node, function_result));
        };

        let mut type_arguments = Vec::new();
        for &type_argument in ast.list_raw(ast[type_argument_list].arguments) {
            match self.evaluate_constant(n.with(type_argument)) {
                Constant::Invalid(invalid)
                    if same_code(&invalid.locatable_diagnostic, &diag::const_type_parameter()) =>
                {
                    // If there's a type parameter error in the evaluated
                    // constant, we convert the message to a more specific
                    // function reference error.
                    return invalid_at_node(
                        ast,
                        type_argument,
                        diag::const_with_type_parameters_function_tearoff(),
                    );
                }
                invalid @ Constant::Invalid(_) => return invalid,
                Constant::Value(v) => match v.to_type_value() {
                    Some(t) => type_arguments.push(t),
                    None => return invalid_at_node(ast, type_argument, diag::invalid_constant()),
                },
            }
        }
        // Dart `DartObjectComputer.typeInstantiate`.
        let raw_type = function_result.ty;
        if let TypeKind::Function(f) = *ctx.ty(raw_type) {
            let type_parameter_count = ctx.list(f.type_params).len();
            if type_arguments.len() != type_parameter_count {
                let diagnostic = crate::constant::utilities::wrong_number_of_type_arguments_error(
                    ctx,
                    u,
                    function,
                    raw_type,
                    type_parameter_count,
                    type_arguments.len(),
                );
                return invalid_at_node(ast, type_argument_list.raw(), diagnostic);
            }
            let ty = ctx.instantiate_function_type(raw_type, &type_arguments);
            Constant::Value(function_result.type_instantiate(ts, ty, type_arguments))
        } else {
            invalid_at_node(ast, function.raw(), diag::invalid_constant())
        }
    }

    fn visit_instance_creation_expression(
        &self,
        n: NodeRef,
        u: &ResolvedUnit,
        ts: &TypeSystem<'_>,
    ) -> Constant {
        let ast = &u.ast;
        let node = Id::<InstanceCreationExpression>::from_raw(n.node);
        if !ast_ext::instance_creation_is_const(ast, node) {
            // TODO(srawlins): Use a specific error code.
            return generic_error(ast, n.node, false);
        }
        let constructor_name = ast[node].constructor_name;
        let Some(constructor) = u.tables.element.get(constructor_name).copied() else {
            // Couldn't resolve the constructor so we can't compute a value.
            // No problem - the error has already been reported.
            return invalid_at_node(ast, n.node, diag::invalid_constant());
        };
        let type_arguments = ts
            .ctx
            .type_arguments(member::return_type(&ts.ctx, constructor))
            .to_vec();
        let arguments: Vec<NodeRef> = argument_nodes(ast, ast[node].argument_list)
            .into_iter()
            .map(|a| n.with(a))
            .collect();
        self.engine.evaluate_and_format_errors_in_constructor_call(
            self.library,
            n,
            Some(type_arguments),
            &arguments,
            constructor,
            self,
            None,
        )
    }

    /// The value of an enum constant: Dart evaluates the synthetic
    /// `InstanceCreationExpression` of the constant, with errors at the
    /// `EnumConstantDeclaration` [n].
    pub fn evaluate_enum_constant(&self, n: NodeRef) -> Constant {
        let u = self.engine.unit(n.unit);
        let ast = &u.ast;
        let ctx = self.engine.ctx(&u);
        let node = Id::<EnumConstantDeclaration>::from_raw(n.node);
        let Some(constructor) = u.tables.element.get(n.node).copied() else {
            return invalid_at_node(ast, n.node, diag::invalid_constant());
        };
        let type_arguments = ctx
            .type_arguments(member::return_type(&ctx, constructor))
            .to_vec();
        let arguments: Vec<NodeRef> = match ast[node].arguments {
            Some(arguments) => argument_nodes(ast, ast[arguments].argument_list)
                .into_iter()
                .map(|a| n.with(a))
                .collect(),
            None => Vec::new(),
        };
        self.engine.evaluate_and_format_errors_in_constructor_call(
            self.library,
            n,
            Some(type_arguments),
            &arguments,
            constructor,
            self,
            None,
        )
    }

    fn visit_list_literal(&self, n: NodeRef, u: &ResolvedUnit, ts: &TypeSystem<'_>) -> Constant {
        let ast = &u.ast;
        let node = Id::<ListLiteral>::from_raw(n.node);
        if !list_literal_is_const(ast, node) {
            return invalid_at_node(ast, n.node, diag::missing_const_in_list_literal());
        }
        let element_type = self.first_type_argument(u, ts, n.node);
        let list_type = self.engine.tp.list_type(&ts.ctx, element_type);
        let mut list = Vec::new();
        let elements = ast.list_raw(ast[node].elements).to_vec();
        self.build_list_constant(n, ts, &mut list, &elements, list_type, element_type)
    }

    fn first_type_argument(&self, u: &ResolvedUnit, ts: &TypeSystem<'_>, node: NodeId) -> TypeId {
        let node_type = u.tables.static_type.get(node).copied();
        match node_type.map(|t| *ts.ctx.ty(t)) {
            Some(TypeKind::Interface { args, .. }) if !ts.ctx.list(args).is_empty() => {
                ts.ctx.list(args)[0]
            }
            _ => TypeId::DYNAMIC,
        }
    }

    fn visit_method_invocation(
        &self,
        n: NodeRef,
        u: &ResolvedUnit,
        ts: &TypeSystem<'_>,
    ) -> Constant {
        let ast = &u.ast;
        let node = Id::<MethodInvocation>::from_raw(n.node);
        let method_name = ast[node].method_name;
        let element = u
            .tables
            .element
            .get(method_name)
            .map(|&e| member::base_element(&ts.ctx, e));
        if let Some(element) = element
            && element.tag() == Tag::TopLevelFunction
            && ts.ctx.is_element(element, "dart.core", "identical")
        {
            let arguments = argument_nodes(ast, ast[node].argument_list);
            if arguments.len() >= 2 {
                let left_argument = match self.evaluate_constant(n.with(arguments[0])) {
                    Constant::Value(v) => v,
                    invalid => return invalid,
                };
                let right_argument = match self.evaluate_constant(n.with(arguments[1])) {
                    Constant::Value(v) => v,
                    invalid => return invalid,
                };
                return Constant::Value(left_argument.is_identical2(ts, &right_argument));
            }
        }
        self.invalid_constant_for_method_invocation(n, u)
    }

    fn visit_named_type(&self, n: NodeRef, u: &ResolvedUnit, ts: &TypeSystem<'_>) -> Constant {
        let ast = &u.ast;
        let node = Id::<NamedType>::from_raw(n.node);
        let mut ty = u
            .tables
            .annotation_type
            .get(n.node)
            .copied()
            .unwrap_or(TypeId::INVALID);

        if named_type_is_type_literal_in_constant_pattern(ast, node)
            && has_type_parameter_reference(&ts.ctx, ty)
        {
            return invalid_at_node(ast, n.node, diag::const_type_parameter());
        } else if named_type_is_deferred(u, &ts.ctx, node) {
            return self.get_deferred_library_error(u, n.node, Entity::Token(ast[node].name));
        }

        if let Some(substitution) = &self.substitution {
            ty = substitution.substitute_type(&ts.ctx, ty);
        }

        self.get_constant_value(
            n,
            u,
            ts,
            n.node,
            None,
            None,
            u.tables.element.get(n.node).copied(),
            Some(ty),
        )
    }

    fn visit_prefixed_identifier(
        &self,
        n: NodeRef,
        u: &ResolvedUnit,
        ts: &TypeSystem<'_>,
    ) -> Constant {
        let ast = &u.ast;
        let node = Id::<PrefixedIdentifier>::from_raw(n.node);
        let prefix_node = ast[node].prefix;
        let identifier = ast[node].identifier;
        let prefix_element = u
            .tables
            .element
            .get(prefix_node)
            .map(|&e| member::base_element(&ts.ctx, e));

        // A top-level constant, imported with a prefix.
        if prefix_element.is_some_and(|e| e.tag() == Tag::Prefix) {
            if prefixed_identifier_is_deferred(u, &ts.ctx, node) {
                return self.get_deferred_library_error(u, n.node, Entity::Node(identifier.raw()));
            }
        } else if !prefix_element.is_some_and(|e| e.tag() == Tag::Extension) {
            let prefix_result = match self.evaluate_constant(n.with(prefix_node)) {
                Constant::Value(v) => v,
                invalid => return invalid,
            };

            // For example, `String.length`.
            if !prefix_element.is_some_and(|e| e.is::<InterfaceElement>())
                && let Some(property_access_result) =
                    self.evaluate_property_access(u, ts, &prefix_result, identifier, n.node, false)
            {
                return property_access_result;
            }
        }

        // Validate prefixed identifier.
        self.get_constant_value(
            n,
            u,
            ts,
            n.node,
            Some(n.node),
            Some(identifier),
            u.tables.element.get(identifier).copied(),
            None,
        )
    }

    fn visit_prefix_expression(
        &self,
        n: NodeRef,
        u: &ResolvedUnit,
        ts: &TypeSystem<'_>,
    ) -> Constant {
        let ast = &u.ast;
        let node = Id::<PrefixExpression>::from_raw(n.node);
        if let Some(error) = self.extension_operator_error(u, ts, n.node) {
            return error;
        }
        let operand = match self.evaluate_constant(n.with(ast[node].operand)) {
            Constant::Value(v) => v,
            invalid => return invalid,
        };
        let result = match ast.tokens.ty(ast[node].operator) {
            TokenType::BANG => operand.logical_not(ts),
            TokenType::TILDE => operand.bit_not(ts),
            TokenType::MINUS => operand.negated(ts),
            // TODO(srawlins): Use a specific error code.
            _ => return generic_error(ast, n.node, false),
        };
        match result {
            Ok(v) => Constant::Value(v),
            Err(e) => from_exception(ast, n.node, e, false),
        }
    }

    fn visit_property_access(&self, n: NodeRef, u: &ResolvedUnit, ts: &TypeSystem<'_>) -> Constant {
        let ast = &u.ast;
        let node = Id::<PropertyAccess>::from_raw(n.node);
        let property_name = ast[node].property_name;
        if let Some(target) = ast[node].target {
            if let Some(target_pi) = ast.cast::<PrefixedIdentifier>(target) {
                let target_element = u
                    .tables
                    .element
                    .get(ast[target_pi].identifier)
                    .map(|&e| member::base_element(&ts.ctx, e));
                if target_element
                    .is_some_and(|e| matches!(e.tag(), Tag::Extension | Tag::ExtensionType))
                {
                    let prefix = ast[target_pi].prefix;
                    let prefix_element = u
                        .tables
                        .element
                        .get(prefix)
                        .map(|&e| member::base_element(&ts.ctx, e));
                    if prefix_element.is_some_and(|e| e.tag() == Tag::Prefix)
                        && prefixed_identifier_is_deferred(u, &ts.ctx, target_pi)
                    {
                        let identifier = ast[target_pi].identifier;
                        return self.get_deferred_library_error(
                            u,
                            n.node,
                            Entity::Node(identifier.raw()),
                        );
                    }

                    // For example, `async.FutureExtensions.wait`.
                    return self.get_constant_value(
                        n,
                        u,
                        ts,
                        n.node,
                        Some(n.node),
                        Some(property_name),
                        u.tables.element.get(property_name).copied(),
                        None,
                    );
                }
            }
            let prefix_result = match self.evaluate_constant(n.with(target)) {
                Constant::Value(v) => v,
                invalid => return invalid,
            };
            let is_null_aware = matches!(
                ast.tokens.ty(ast[node].operator),
                TokenType::QUESTION_PERIOD | TokenType::QUESTION_PERIOD_PERIOD
            );
            if let Some(property_access_result) = self.evaluate_property_access(
                u,
                ts,
                &prefix_result,
                property_name,
                n.node,
                is_null_aware,
            ) {
                return property_access_result;
            }
        }
        self.get_constant_value(
            n,
            u,
            ts,
            n.node,
            Some(n.node),
            Some(property_name),
            u.tables.element.get(property_name).copied(),
            None,
        )
    }

    fn visit_record_literal(&self, n: NodeRef, u: &ResolvedUnit, ts: &TypeSystem<'_>) -> Constant {
        let ast = &u.ast;
        let ctx = &ts.ctx;
        let node = Id::<RecordLiteral>::from_raw(n.node);
        let mut positional_fields = Vec::new();
        let mut named_fields = FieldMap::new();
        for &field in ast.list_raw(ast[node].fields) {
            if let Some(named) = ast.cast::<RecordLiteralNamedField>(field) {
                let name = ast.tokens.lexeme(ast[named].name).to_string();
                let value = match self.evaluate_constant(n.with(ast[named].field_expression)) {
                    Constant::Value(v) => v,
                    invalid => return invalid,
                };
                named_fields.insert(Arc::from(name.as_str()), value);
            } else {
                let value = match self.evaluate_constant(n.with(field)) {
                    Constant::Value(v) => v,
                    invalid => return invalid,
                };
                positional_fields.push(value);
            }
        }

        let positional: Vec<TypeId> = positional_fields.iter().map(|e| e.ty).collect();
        let named: Vec<NamedField> = named_fields
            .iter()
            .map(|(name, value)| NamedField {
                name: ctx.name(name),
                ty: value.ty,
            })
            .collect();
        let node_type = ctx.record_type(&positional, &named, Nullability::None, None);
        Constant::Value(DartObjectImpl::new(
            ts,
            node_type,
            InstanceState::Record(Arc::new(RecordState::new(positional_fields, named_fields))),
        ))
    }

    fn visit_set_or_map_literal(
        &self,
        n: NodeRef,
        u: &ResolvedUnit,
        ts: &TypeSystem<'_>,
    ) -> Constant {
        let ast = &u.ast;
        let ctx = &ts.ctx;
        let node = Id::<SetOrMapLiteral>::from_raw(n.node);
        let (is_set, is_map) = set_or_map_kind(u, ctx, node);
        let is_const = set_or_map_literal_is_const(ast, node);
        let elements = ast.list_raw(ast[node].elements).to_vec();
        // Note: due to dartbug.com/33441, it's possible that a set/map
        // literal resynthesized from a summary will have neither its `isSet`
        // or `isMap` boolean set to `true`. We work around the problem by
        // assuming such literals are maps.
        if !is_set {
            if !is_const {
                return invalid_at_node(ast, n.node, diag::missing_const_in_map_literal());
            }
            let mut key_type = TypeId::DYNAMIC;
            let mut value_type = TypeId::DYNAMIC;
            if let Some(node_type) = u.tables.static_type.get(n.node)
                && let TypeKind::Interface { args, .. } = *ctx.ty(*node_type)
            {
                let args = ctx.list(args);
                if args.len() >= 2 {
                    key_type = args[0];
                    value_type = args[1];
                }
            }
            let map_type = self.engine.tp.map_type(ctx, key_type, value_type);
            let mut map = DartObjectMap::new();
            let result =
                self.build_map_constant(n, ts, &mut map, &elements, map_type, key_type, value_type);
            if let Constant::Invalid(mut invalid) = result {
                if !is_map {
                    // We don't report the error if we know this is an
                    // ambiguous map or set.
                    invalid.avoid_reporting = true;
                }
                return Constant::Invalid(invalid);
            }
            result
        } else {
            if !is_const {
                return invalid_at_node(ast, n.node, diag::missing_const_in_set_literal());
            }
            let element_type = self.first_type_argument(u, ts, n.node);
            let set_type = self.engine.tp.set_type(ctx, element_type);
            let mut set = DartObjectSet::new();
            self.build_set_constant(n, ts, &mut set, &elements, set_type, element_type)
        }
    }

    fn visit_simple_identifier(
        &self,
        n: NodeRef,
        u: &ResolvedUnit,
        ts: &TypeSystem<'_>,
    ) -> Constant {
        let node = Id::<SimpleIdentifier>::from_raw(n.node);
        let element = u.tables.element.get(n.node).copied();
        if let Some(element) = element {
            let base = member::base_element(&ts.ctx, element);
            if is_formal_parameter(base)
                && let Some(value) = self.lexical_environment.and_then(|env| env.get(&base))
            {
                return self.instantiate_function_type_for_simple_identifier(
                    u,
                    ts,
                    node,
                    value.clone(),
                );
            }
        }
        self.get_constant_value(n, u, ts, n.node, Some(n.node), Some(node), element, None)
    }

    /// Dart `_buildListConstant`.
    fn build_list_constant(
        &self,
        n: NodeRef,
        ts: &TypeSystem<'_>,
        list: &mut Vec<DartObjectImpl>,
        elements: &[NodeId],
        list_type: TypeId,
        element_type: TypeId,
    ) -> Constant {
        let u = self.engine.unit(n.unit);
        let ast = &u.ast;
        for &element in elements {
            match ast.kind(element) {
                NodeKind::ForElement => {
                    return invalid_at_node(ast, element, diag::const_eval_for_element());
                }
                NodeKind::IfElement => {
                    let e = &ast[Id::<IfElement>::from_raw(element)];
                    let condition = match self.evaluate_constant(n.with(e.expression)) {
                        Constant::Value(v) => v,
                        invalid => return invalid,
                    };
                    // If the condition is unknown, we mark this list as
                    // unknown.
                    if condition.is_unknown() {
                        return Constant::Value(DartObjectImpl::new(
                            ts,
                            list_type,
                            InstanceState::List(Arc::new(ListState::unknown(ts, element_type))),
                        ));
                    }
                    let branch_result = match condition.to_bool_value() {
                        None => {
                            return invalid_at_node(
                                ast,
                                e.expression.raw(),
                                diag::non_bool_condition(),
                            );
                        }
                        Some(true) => Some(self.build_list_constant(
                            n,
                            ts,
                            list,
                            &[e.then_element.raw()],
                            list_type,
                            element_type,
                        )),
                        Some(false) => e.else_element.map(|else_element| {
                            self.build_list_constant(
                                n,
                                ts,
                                list,
                                &[else_element.raw()],
                                list_type,
                                element_type,
                            )
                        }),
                    };
                    if let Some(invalid @ Constant::Invalid(_)) = branch_result {
                        return invalid;
                    }
                }
                NodeKind::MapLiteralEntry => {
                    return invalid_at_node(ast, element, diag::map_entry_not_in_map());
                }
                NodeKind::SpreadElement => {
                    let e = &ast[Id::<SpreadElement>::from_raw(element)];
                    let spread = match self.evaluate_constant(n.with(e.expression)) {
                        Constant::Value(v) => v,
                        invalid => return invalid,
                    };
                    // Special case for ...?
                    if spread.is_null() && spread_is_null_aware(ast, e) {
                        continue;
                    }
                    if let Some(values) = spread.to_list_value() {
                        list.extend(values.iter().cloned());
                    } else if let Some(values) = spread.to_set_value() {
                        list.extend(values.iter().cloned());
                    } else {
                        return invalid_at_node(
                            ast,
                            e.expression.raw(),
                            diag::const_spread_expected_list_or_set(),
                        );
                    }
                }
                NodeKind::NullAwareElement => {
                    let value_node = ast[Id::<NullAwareElement>::from_raw(element)].value;
                    let value = match self.evaluate_constant(n.with(value_node)) {
                        Constant::Value(v) => v,
                        invalid => return invalid,
                    };
                    if value.is_null() {
                        continue;
                    }
                    return self.build_list_constant(
                        n,
                        ts,
                        list,
                        &[value_node.raw()],
                        list_type,
                        element_type,
                    );
                }
                _ => {
                    // An expression.
                    match self.evaluate_constant(n.with(element)) {
                        invalid @ Constant::Invalid(_) => return invalid,
                        Constant::Value(v) => list.push(v),
                    }
                }
            }
        }
        Constant::Value(DartObjectImpl::new(
            ts,
            list_type,
            InstanceState::List(Arc::new(ListState::new(
                ts,
                element_type,
                list.clone(),
                false,
            ))),
        ))
    }

    /// Dart `_buildMapConstant`.
    #[allow(clippy::too_many_arguments)]
    fn build_map_constant(
        &self,
        n: NodeRef,
        ts: &TypeSystem<'_>,
        map: &mut DartObjectMap,
        elements: &[NodeId],
        map_type: TypeId,
        key_type: TypeId,
        value_type: TypeId,
    ) -> Constant {
        let u = self.engine.unit(n.unit);
        let ast = &u.ast;
        for &element in elements {
            match ast.kind(element) {
                NodeKind::ForElement => {
                    return invalid_at_node(ast, element, diag::const_eval_for_element());
                }
                NodeKind::IfElement => {
                    let e = &ast[Id::<IfElement>::from_raw(element)];
                    let condition = match self.evaluate_constant(n.with(e.expression)) {
                        Constant::Value(v) => v,
                        invalid => return invalid,
                    };
                    // If the condition is unknown, we mark this map as
                    // unknown.
                    if condition.is_unknown() {
                        return Constant::Value(DartObjectImpl::new(
                            ts,
                            map_type,
                            InstanceState::Map(Arc::new(MapState::unknown(
                                ts, key_type, value_type,
                            ))),
                        ));
                    }
                    let branch_result = match condition.to_bool_value() {
                        None => {
                            return invalid_at_node(
                                ast,
                                e.expression.raw(),
                                diag::non_bool_condition(),
                            );
                        }
                        Some(true) => Some(self.build_map_constant(
                            n,
                            ts,
                            map,
                            &[e.then_element.raw()],
                            map_type,
                            key_type,
                            value_type,
                        )),
                        Some(false) => e.else_element.map(|else_element| {
                            self.build_map_constant(
                                n,
                                ts,
                                map,
                                &[else_element.raw()],
                                map_type,
                                key_type,
                                value_type,
                            )
                        }),
                    };
                    if let Some(invalid @ Constant::Invalid(_)) = branch_result {
                        return invalid;
                    }
                }
                NodeKind::MapLiteralEntry => {
                    let e = &ast[Id::<MapLiteralEntry>::from_raw(element)];
                    let key_result = self.evaluate_constant(n.with(e.key));
                    let value_result = self.evaluate_constant(n.with(e.value));
                    match (key_result, value_result) {
                        (invalid @ Constant::Invalid(_), _) => return invalid,
                        (_, invalid @ Constant::Invalid(_)) => return invalid,
                        (Constant::Value(k), Constant::Value(v)) => map.insert(ts, k, v),
                    }
                }
                NodeKind::SpreadElement => {
                    let e = &ast[Id::<SpreadElement>::from_raw(element)];
                    let spread = match self.evaluate_constant(n.with(e.expression)) {
                        Constant::Value(v) => v,
                        invalid => return invalid,
                    };
                    // Special case for ...?
                    if spread.is_null() && spread_is_null_aware(ast, e) {
                        continue;
                    }
                    let Some(map_value) = spread.to_map_value() else {
                        return invalid_at_node(
                            ast,
                            e.expression.raw(),
                            diag::const_spread_expected_map(),
                        );
                    };
                    for (k, v) in map_value.iter() {
                        map.insert(ts, k.clone(), v.clone());
                    }
                }
                NodeKind::NullAwareElement => {
                    return invalid_at_node(ast, element, diag::expression_in_map());
                }
                _ => {
                    return invalid_at_node(ast, element, diag::expression_in_map());
                }
            }
        }
        Constant::Value(DartObjectImpl::new(
            ts,
            map_type,
            InstanceState::Map(Arc::new(MapState::new(
                ts,
                key_type,
                value_type,
                map.clone(),
                false,
            ))),
        ))
    }

    /// Dart `_buildSetConstant`.
    fn build_set_constant(
        &self,
        n: NodeRef,
        ts: &TypeSystem<'_>,
        set: &mut DartObjectSet,
        elements: &[NodeId],
        set_type: TypeId,
        element_type: TypeId,
    ) -> Constant {
        let u = self.engine.unit(n.unit);
        let ast = &u.ast;
        for &element in elements {
            match ast.kind(element) {
                NodeKind::ForElement => {
                    return invalid_at_node(ast, element, diag::const_eval_for_element());
                }
                NodeKind::IfElement => {
                    let e = &ast[Id::<IfElement>::from_raw(element)];
                    let condition = match self.evaluate_constant(n.with(e.expression)) {
                        Constant::Value(v) => v,
                        invalid => return invalid,
                    };
                    // If the condition is unknown, we mark this set as
                    // unknown.
                    if condition.is_unknown() {
                        return Constant::Value(DartObjectImpl::new(
                            ts,
                            set_type,
                            InstanceState::Set(Arc::new(SetState::unknown(ts, element_type))),
                        ));
                    }
                    let branch_result = match condition.to_bool_value() {
                        None => {
                            return invalid_at_node(
                                ast,
                                e.expression.raw(),
                                diag::non_bool_condition(),
                            );
                        }
                        Some(true) => Some(self.build_set_constant(
                            n,
                            ts,
                            set,
                            &[e.then_element.raw()],
                            set_type,
                            element_type,
                        )),
                        Some(false) => e.else_element.map(|else_element| {
                            self.build_set_constant(
                                n,
                                ts,
                                set,
                                &[else_element.raw()],
                                set_type,
                                element_type,
                            )
                        }),
                    };
                    if let Some(invalid @ Constant::Invalid(_)) = branch_result {
                        return invalid;
                    }
                }
                NodeKind::MapLiteralEntry => {
                    return invalid_at_node(ast, element, diag::map_entry_not_in_map());
                }
                NodeKind::SpreadElement => {
                    let e = &ast[Id::<SpreadElement>::from_raw(element)];
                    let spread = match self.evaluate_constant(n.with(e.expression)) {
                        Constant::Value(v) => v,
                        invalid => return invalid,
                    };
                    // Special case for ...?
                    if spread.is_null() && spread_is_null_aware(ast, e) {
                        continue;
                    }
                    if let Some(values) = spread.to_set_value() {
                        for v in values.iter() {
                            set.add(ts, v.clone());
                        }
                    } else if let Some(values) = spread.to_list_value() {
                        for v in values {
                            set.add(ts, v.clone());
                        }
                    } else {
                        return invalid_at_node(
                            ast,
                            e.expression.raw(),
                            diag::const_spread_expected_list_or_set(),
                        );
                    }
                }
                NodeKind::NullAwareElement => {
                    let value_node = ast[Id::<NullAwareElement>::from_raw(element)].value;
                    let value = match self.evaluate_constant(n.with(value_node)) {
                        Constant::Value(v) => v,
                        invalid => return invalid,
                    };
                    if value.is_null() {
                        continue;
                    }
                    return self.build_set_constant(
                        n,
                        ts,
                        set,
                        &[value_node.raw()],
                        set_type,
                        element_type,
                    );
                }
                _ => match self.evaluate_constant(n.with(element)) {
                    invalid @ Constant::Invalid(_) => return invalid,
                    Constant::Value(v) => {
                        set.add(ts, v);
                    }
                },
            }
        }
        Constant::Value(DartObjectImpl::new(
            ts,
            set_type,
            InstanceState::Set(Arc::new(SetState::new(
                ts,
                element_type,
                set.clone(),
                false,
            ))),
        ))
    }

    /// Dart `_concatenateNodes`.
    fn concatenate_nodes(
        &self,
        n: NodeRef,
        u: &ResolvedUnit,
        ts: &TypeSystem<'_>,
        ast_nodes: &[NodeId],
    ) -> Constant {
        let mut result: Option<DartObjectImpl> = None;
        for &ast_node in ast_nodes {
            let constant = match self.evaluate_constant(n.with(ast_node)) {
                Constant::Value(v) => v,
                invalid => return invalid,
            };
            result = Some(match result {
                None => constant,
                Some(r) => match r.concatenate(ts, &constant) {
                    Ok(v) => v,
                    Err(e) => return from_exception(&u.ast, n.node, e, false),
                },
            });
        }
        match result {
            Some(r) => Constant::Value(r),
            // No errors have been detected, but we did not concatenate any
            // nodes.
            None => Constant::Value(DartObjectImpl::new(
                ts,
                self.engine.tp.string_type(),
                InstanceState::String(StringState::UNKNOWN_VALUE),
            )),
        }
    }

    /// Dart `_evaluatePropertyAccess`.
    fn evaluate_property_access(
        &self,
        u: &ResolvedUnit,
        ts: &TypeSystem<'_>,
        target_result: &DartObjectImpl,
        identifier: Id<SimpleIdentifier>,
        error_node: NodeId,
        is_null_aware: bool,
    ) -> Option<Constant> {
        let ctx = &ts.ctx;
        let ast = &u.ast;
        let property_element = u.tables.element.get(identifier).copied();
        if let Some(p) = property_element
            && member::base_element(ctx, p).tag() == Tag::Getter
            && member::is_static(ctx, p)
        {
            return None;
        }

        if let Some(p) = property_element {
            match member::enclosing_element(ctx, p).map(|e| e.tag()) {
                Some(Tag::Extension) => {
                    return Some(invalid_at_node(
                        ast,
                        error_node,
                        diag::const_eval_extension_method(),
                    ));
                }
                Some(Tag::ExtensionType) => {
                    return Some(invalid_at_node(
                        ast,
                        error_node,
                        diag::const_eval_extension_type_method(),
                    ));
                }
                _ => {}
            }
        }

        let target_type = target_result.ty;

        // Evaluate a constant that reads the length of a `String`.
        let name = ast_ext::identifier_name(ast, identifier);
        if name == "length" {
            if ctx.is_dart_core_string(target_type) {
                return Some(match target_result.string_length(ts) {
                    Ok(v) => Constant::Value(v),
                    Err(e) => from_exception(ast, error_node, e, false),
                });
            } else if ctx.is_dart_core_null(target_type) && is_null_aware {
                return Some(Constant::Value(null_object(ts)));
            }
        }

        if let Some(element) = property_element
            && member::base_element(ctx, element).is::<ExecutableElement>()
            && member::is_static(ctx, element)
        {
            return None;
        }

        // No other property access is allowed except for `.length` of a
        // `String`.
        Some(invalid_at_node(
            ast,
            error_node,
            diag::const_eval_property_access(name, &ts.display_type(target_type)),
        ))
    }

    /// Dart `_getConstantValue`.
    #[allow(clippy::too_many_arguments)]
    fn get_constant_value(
        &self,
        n: NodeRef,
        u: &ResolvedUnit,
        ts: &TypeSystem<'_>,
        error_node: NodeId,
        expression: Option<NodeId>,
        identifier: Option<Id<SimpleIdentifier>>,
        element: Option<ElemRef>,
        given_type: Option<TypeId>,
    ) -> Constant {
        let ast = &u.ast;
        let ctx = &ts.ctx;
        let tp = self.engine.tp;
        let element = element.map(|e| member::base_element(ctx, e));

        let variable_element = match element {
            Some(e) if is_property_accessor(e) => {
                member::variable(ctx, ElemRef::Base(e)).map(|v| member::base_element(ctx, v))
            }
            other => other,
        };

        // TODO(srawlins): Remove this check when [FunctionReference]s are
        // inserted for generic function instantiation for
        // pre-constructor-references code.
        if let Some(expression) = expression
            && ast.kind(expression) == NodeKind::SimpleIdentifier
            && let Some(types) = u.tables.type_arg_types.get(expression)
            && ctx
                .list(*types)
                .iter()
                .any(|&t| has_type_parameter_reference(ctx, t))
        {
            return invalid_at_node(ast, expression, diag::const_type_parameter());
        }

        if let Some(v) = variable_element {
            if v.is::<VariableElement>() {
                // We access values of constant variables here in two cases:
                // when we compute values of other constant variables, or
                // when we compute values and errors for other constant
                // expressions. In either case we have already computed
                // values of all dependencies first (or detect a cycle), so
                // the value has already been computed and we can just
                // return it.
                let is_const = if v.store().is_local() {
                    crate::element_ext::is_const(ctx, v)
                } else {
                    is_const_variable(ctx, v)
                };
                if is_const {
                    return match self.engine.evaluation_result(v) {
                        // The constant value isn't computed yet, or there is
                        // an error while computing. We will mark it and
                        // determine whether or not to continue the
                        // evaluation upstream.
                        None => generic_error(ast, error_node, true),
                        Some(Constant::Value(value)) => match identifier {
                            None => invalid_at_node(ast, error_node, diag::invalid_constant()),
                            Some(identifier) => self
                                .instantiate_function_type_for_simple_identifier(
                                    u, ts, identifier, value,
                                ),
                        },
                        // TODO(kallentu): Investigate and fix the test
                        // failures that occur if we remove `avoidReporting`.
                        Some(Constant::Invalid(_)) => with_flags(
                            invalid_constant_at_node(ast, error_node, diag::invalid_constant()),
                            true,
                            true,
                        ),
                    };
                }
            } else if v.tag() == Tag::Constructor && expression.is_some() {
                let ty = u
                    .tables
                    .static_type
                    .get(expression.unwrap())
                    .copied()
                    .unwrap_or(TypeId::INVALID);
                return Constant::Value(DartObjectImpl::new(
                    ts,
                    ty,
                    InstanceState::Function(Arc::new(FunctionState::new(
                        EId::<ExecutableElement>::from_raw(v),
                        None,
                        None,
                    ))),
                ));
            } else if v.is::<ExecutableElement>() {
                if member::is_static(ctx, ElemRef::Base(v)) {
                    let raw_type = DartObjectImpl::new(
                        ts,
                        member::type_(ctx, ElemRef::Base(v)),
                        InstanceState::Function(Arc::new(FunctionState::new(
                            EId::<ExecutableElement>::from_raw(v),
                            None,
                            None,
                        ))),
                    );
                    return match identifier {
                        None => invalid_at_node(ast, error_node, diag::invalid_constant()),
                        Some(identifier) => self.instantiate_function_type_for_simple_identifier(
                            u, ts, identifier, raw_type,
                        ),
                    };
                }
            } else if v.is::<InterfaceElement>() {
                let ty = given_type.unwrap_or_else(|| {
                    let element = EId::<InterfaceElement>::from_raw(v);
                    let args = vec![TypeId::DYNAMIC; ctx.interface(element).type_params.len()];
                    ctx.interface_type(element, &args, Nullability::None)
                });
                return Constant::Value(DartObjectImpl::new(
                    ts,
                    tp.type_type(),
                    InstanceState::Type(TypeState::new(ts, Some(ty))),
                ));
            } else if v.tag() == Tag::Dynamic {
                return Constant::Value(DartObjectImpl::new(
                    ts,
                    tp.type_type(),
                    InstanceState::Type(TypeState::new(
                        ts,
                        Some(given_type.unwrap_or(TypeId::DYNAMIC)),
                    )),
                ));
            } else if v.tag() == Tag::TypeAlias {
                let ty = given_type.unwrap_or_else(|| {
                    let element = EId::<TypeAliasElement>::from_raw(v);
                    let args: Vec<TypeId> = ctx
                        .get(element)
                        .type_params
                        .iter()
                        .map(|&p| ctx.type_parameter_bound(p).unwrap_or(TypeId::DYNAMIC))
                        .collect();
                    ctx.instantiate_type_alias(element, &args, Nullability::None)
                });
                return Constant::Value(DartObjectImpl::new(
                    ts,
                    tp.type_type(),
                    InstanceState::Type(TypeState::new(ts, Some(ty))),
                ));
            } else if v.tag() == Tag::Never {
                return Constant::Value(DartObjectImpl::new(
                    ts,
                    tp.type_type(),
                    InstanceState::Type(TypeState::new(
                        ts,
                        Some(given_type.unwrap_or_else(|| tp.never_type())),
                    )),
                ));
            } else if v.tag() == Tag::TypeParameter {
                // Constants may refer to type parameters only if the
                // constructor-tearoffs feature is enabled.
                if self.features().is_enabled("constructor-tearoffs")
                    && let Some(type_argument) = self
                        .lexical_type_environment
                        .and_then(|env| env.get(&v).copied())
                {
                    return Constant::Value(DartObjectImpl::new(
                        ts,
                        tp.type_type(),
                        InstanceState::Type(TypeState::new(ts, Some(type_argument))),
                    ));
                }
                return invalid_at_node(ast, error_node, diag::const_type_parameter());
            }
        }

        // The expression is unresolved by the time we are evaluating it.
        // We'll mark it and return immediately.
        if let Some(expression) = expression
            && u.tables.static_type.get(expression).copied() == Some(TypeId::INVALID)
        {
            return generic_error(ast, error_node, true);
        }

        // TODO(srawlins): Use a specific error code.
        let _ = n;
        generic_error(ast, error_node, false)
    }

    /// Dart `_getDeferredLibraryError`.
    fn get_deferred_library_error(
        &self,
        u: &ResolvedUnit,
        node: NodeId,
        error_target: Entity,
    ) -> Constant {
        let ast = &u.ast;
        let mut previous: Option<NodeId> = None;
        let mut current = Some(node);
        let mut error_code = None;
        while let Some(c) = current {
            let code = match ast.kind(c) {
                NodeKind::Annotation => {
                    Some(diag::invalid_annotation_constant_value_from_deferred_library())
                }
                NodeKind::FormalParameterDefaultClause => {
                    Some(diag::non_constant_default_value_from_deferred_library())
                }
                NodeKind::IfElement
                    if ast[Id::<IfElement>::from_raw(c)].expression.raw() == node =>
                {
                    Some(diag::if_element_condition_from_deferred_library())
                }
                NodeKind::InstanceCreationExpression => {
                    Some(diag::const_constructor_constant_from_deferred_library())
                }
                NodeKind::ListLiteral => {
                    Some(diag::non_constant_list_element_from_deferred_library())
                }
                NodeKind::MapLiteralEntry => {
                    if previous == Some(ast[Id::<MapLiteralEntry>::from_raw(c)].key.raw()) {
                        Some(diag::non_constant_map_key_from_deferred_library())
                    } else {
                        Some(diag::non_constant_map_value_from_deferred_library())
                    }
                }
                NodeKind::RecordLiteral => {
                    Some(diag::non_constant_record_field_from_deferred_library())
                }
                NodeKind::SetOrMapLiteral => Some(diag::set_element_from_deferred_library()),
                NodeKind::SpreadElement => Some(diag::spread_expression_from_deferred_library()),
                NodeKind::SwitchCase => {
                    Some(diag::non_constant_case_expression_from_deferred_library())
                }
                NodeKind::SwitchPatternCase => Some(diag::pattern_constant_from_deferred_library()),
                NodeKind::VariableDeclaration => {
                    Some(diag::const_initialized_with_non_constant_value_from_deferred_library())
                }
                _ => None,
            };
            if code.is_some() {
                error_code = code;
                break;
            }
            previous = Some(c);
            current = ast.parent(c);
        }
        match error_code {
            Some(code) => Constant::Invalid(Box::new(match error_target {
                Entity::Node(t) => invalid_constant_at_node(ast, t, code),
                Entity::Token(t) => invalid_constant_at_token(ast, t, code),
            })),
            None => invalid_at_node(ast, node, diag::invalid_constant()),
        }
    }

    /// Dart `_instantiateFunctionType(node, value)` (a `FunctionReference`).
    fn instantiate_function_type(
        &self,
        u: &ResolvedUnit,
        ts: &TypeSystem<'_>,
        node: NodeId,
        value: DartObjectImpl,
    ) -> DartObjectImpl {
        let ctx = &ts.ctx;
        let Some(function_element) = value.to_function_value() else {
            return value;
        };
        let value_type = member::type_(ctx, ElemRef::Base(function_element.raw()));
        let TypeKind::Function(f) = *ctx.ty(value_type) else {
            return value;
        };
        if !ctx.list(f.type_params).is_empty()
            && let Some(type_argument_types) = u.tables.type_arg_types.get(node)
        {
            let type_argument_types = ctx.list(*type_argument_types).to_vec();
            if !type_argument_types.is_empty() {
                let mut instantiated_type =
                    ctx.instantiate_function_type(value_type, &type_argument_types);
                if let Some(substitution) = &self.substitution {
                    instantiated_type = substitution.substitute_type(ctx, instantiated_type);
                }
                return value.type_instantiate(ts, instantiated_type, type_argument_types);
            }
        }
        value
    }

    /// Dart `_instantiateFunctionTypeForSimpleIdentifier`.
    fn instantiate_function_type_for_simple_identifier(
        &self,
        u: &ResolvedUnit,
        ts: &TypeSystem<'_>,
        node: Id<SimpleIdentifier>,
        value: DartObjectImpl,
    ) -> Constant {
        let ctx = &ts.ctx;
        // TODO(srawlins): When all code uses [FunctionReference]s generated
        // via generic function instantiation, remove this method and all
        // call sites.
        let Some(function_element) = value.to_function_value() else {
            return Constant::Value(value);
        };
        let value_type = member::type_(ctx, ElemRef::Base(function_element.raw()));
        let TypeKind::Function(f) = *ctx.ty(value_type) else {
            return Constant::Value(value);
        };
        if !ctx.list(f.type_params).is_empty()
            && let Some(types) = u.tables.type_arg_types.get(node)
        {
            let types = ctx.list(*types).to_vec();
            if !types.is_empty() {
                let instantiated_type = ctx.instantiate_function_type(value_type, &types);
                return Constant::Value(value.type_instantiate(ts, instantiated_type, types));
            }
        }
        Constant::Value(value)
    }

    /// Dart `_invalidConstantForMethodInvocation`.
    fn invalid_constant_for_method_invocation(&self, n: NodeRef, u: &ResolvedUnit) -> Constant {
        // Some methods aren't resolved by the time we are evaluating it.
        // We'll mark it and return immediately.
        if u.tables.static_type.get(n.node).copied() == Some(TypeId::INVALID) {
            let mut result = invalid_constant_at_node(&u.ast, n.node, diag::invalid_constant());
            result.is_unresolved = true;
            return Constant::Invalid(Box::new(result));
        }
        invalid_at_node(&u.ast, n.node, diag::const_eval_method_invocation())
    }

    /// Dart `_reportNotPotentialConstants`.
    fn report_not_potential_constants(&self, n: NodeRef) -> Option<Constant> {
        let u = self.engine.unit(n.unit);
        let ctx = self.engine.ctx(&u);
        let input = ConstCheckInput {
            ctx: &ctx,
            ast: &u.ast,
            tables: &u.tables,
            rt: &u.rt,
            features: self.features(),
        };
        let not_potentially_constants = get_not_potentially_constants(&input, n.node);
        let first = *not_potentially_constants.first()?;
        // Only report the first invalid constant we see.
        Some(invalid_at_node(&u.ast, first, diag::invalid_constant()))
    }

    /// Dart `_valueOf(expression, defaultType)`.
    fn value_of(&self, expression: NodeRef, default_type: TypeId) -> Constant {
        let expression_value = self.evaluate_constant(expression);
        match expression_value {
            // TODO(kallentu): g3 relies on reporting errors found here, but
            // also being able to continue the evaluation with populating
            // fields. Fix the interaction with g3 more elegantly.
            Constant::Invalid(invalid) if invalid.is_unresolved => {
                if !invalid.avoid_reporting {
                    self.report(expression.unit, &invalid);
                }
                let u = self.engine.unit(expression.unit);
                let ts = TypeSystem::new(self.engine.ctx(&u));
                Constant::Value(unresolved_object(&ts, default_type))
            }
            other => other,
        }
    }
}

/// A syntactic entity to report at.
#[derive(Clone, Copy)]
enum Entity {
    Node(NodeId),
    Token(dartr_syntax::TokenId),
}

// ---------------------------------------------------------------------------
// AST facts
// ---------------------------------------------------------------------------

/// Dart `ListLiteralImpl.isConst`.
pub fn list_literal_is_const(ast: &Ast, node: Id<ListLiteral>) -> bool {
    ast[node].const_keyword.is_some() || ast_ext::in_constant_context(ast, node.raw())
}

/// Dart `SetOrMapLiteralImpl.isConst`.
pub fn set_or_map_literal_is_const(ast: &Ast, node: Id<SetOrMapLiteral>) -> bool {
    ast[node].const_keyword.is_some() || ast_ext::in_constant_context(ast, node.raw())
}

/// Dart `RecordLiteralImpl.isConst`.
pub fn record_literal_is_const(ast: &Ast, node: Id<RecordLiteral>) -> bool {
    ast[node].const_keyword.is_some() || ast_ext::in_constant_context(ast, node.raw())
}

/// Dart `DotShorthandConstructorInvocationImpl.isConst`.
pub fn dot_shorthand_constructor_invocation_is_const(
    ast: &Ast,
    node: Id<DotShorthandConstructorInvocation>,
) -> bool {
    ast[node].const_keyword.is_some() || ast_ext::in_constant_context(ast, node.raw())
}

/// Dart `SetOrMapLiteral.isSet` / `isMap` (from the resolved type).
pub fn set_or_map_kind(u: &ResolvedUnit, ctx: &Ctx<'_>, node: Id<SetOrMapLiteral>) -> (bool, bool) {
    let element = u
        .tables
        .static_type
        .get(node)
        .and_then(|&t| ctx.interface_element(t));
    match element {
        Some(e) if e == ctx.tp.set_element().upcast() => (true, false),
        Some(e) if e == ctx.tp.map_element().upcast() => (false, true),
        _ => (false, false),
    }
}

fn spread_is_null_aware(ast: &Ast, e: &SpreadElement) -> bool {
    ast.tokens.ty(e.spread_operator) == TokenType::PERIOD_PERIOD_PERIOD_QUESTION
}

/// Dart `NamedType.isTypeLiteralInConstantPattern`.
fn named_type_is_type_literal_in_constant_pattern(ast: &Ast, node: Id<NamedType>) -> bool {
    let Some(parent) = ast.parent(node) else {
        return false;
    };
    ast.kind(parent) == NodeKind::TypeLiteral
        && ast
            .parent(parent)
            .and_then(|p| ast.parent(p))
            .is_some_and(|p| ast.kind(p) == NodeKind::ConstantPattern)
}

/// Whether the prefix element [prefix] is of a deferred import.
pub fn prefix_is_deferred(ctx: &Ctx<'_>, prefix: ElementId) -> bool {
    let Some(data) = ctx.element_data(prefix) else {
        return false;
    };
    let mut fragment = Some(data.first_fragment);
    while let Some(f) = fragment {
        if let Some(pf) = f.cast::<dartr_element::PrefixFragment>()
            && ctx.fragment(pf).is_deferred
        {
            return true;
        }
        fragment = ctx.fragment_data(f).and_then(|d| d.next_fragment);
    }
    false
}

/// Dart `NamedType.isDeferred`.
pub fn named_type_is_deferred(u: &ResolvedUnit, ctx: &Ctx<'_>, node: Id<NamedType>) -> bool {
    let Some(prefix) = u.ast[node].import_prefix else {
        return false;
    };
    import_prefix_is_deferred(u, ctx, prefix)
}

fn import_prefix_is_deferred(
    u: &ResolvedUnit,
    ctx: &Ctx<'_>,
    prefix: Id<ImportPrefixReference>,
) -> bool {
    u.tables
        .element
        .get(prefix)
        .map(|&e| member::base_element(ctx, e))
        .is_some_and(|e| e.tag() == Tag::Prefix && prefix_is_deferred(ctx, e))
}

/// Dart `PrefixedIdentifier.isDeferred`.
pub fn prefixed_identifier_is_deferred(
    u: &ResolvedUnit,
    ctx: &Ctx<'_>,
    node: Id<PrefixedIdentifier>,
) -> bool {
    u.tables
        .element
        .get(u.ast[node].prefix)
        .map(|&e| member::base_element(ctx, e))
        .is_some_and(|e| e.tag() == Tag::Prefix && prefix_is_deferred(ctx, e))
}

// ---------------------------------------------------------------------------
// _InstanceCreationEvaluator
// ---------------------------------------------------------------------------

/// Dart `_InitializersEvaluationResult`.
struct InitializersEvaluationResult {
    result: Option<Constant>,
    evaluation_is_complete: bool,
    super_name: Option<String>,
    super_arguments: Option<Vec<NodeRef>>,
}

impl InitializersEvaluationResult {
    fn complete(result: Constant) -> InitializersEvaluationResult {
        InitializersEvaluationResult {
            result: Some(result),
            evaluation_is_complete: true,
            super_name: None,
            super_arguments: None,
        }
    }
}

/// Dart `_InstanceCreationEvaluator`.
struct InstanceCreationEvaluator<'e, 'a> {
    engine: &'e ConstantEvaluationEngine<'a>,
    library: EId<LibraryElement>,
    /// The node used for most error reporting.
    error_node: NodeRef,
    constructor: ElemRef,
    type_arguments: Option<Vec<TypeId>>,
    invocation: Arc<ConstructorInvocationImpl>,
    argument_values: Vec<DartObjectImpl>,
    argument_value_map: IndexMap<ElementId, DartObjectImpl>,
    argument_node_map: IndexMap<ElementId, Option<NodeRef>>,
    type_parameter_map: IndexMap<ElementId, TypeId>,
    parameter_map: IndexMap<ElementId, DartObjectImpl>,
    field_map: FieldMap,
}

/// Dart `_RedirectionResult`.
struct RedirectionResult {
    constructor: ElemRef,
    argument_value_map: IndexMap<ElementId, DartObjectImpl>,
    argument_node_map: IndexMap<ElementId, Option<NodeRef>>,
}

const DEFAULT_VALUE_PARAM: &str = "defaultValue";

impl<'e, 'a> InstanceCreationEvaluator<'e, 'a> {
    fn ctx(&self) -> Ctx<'e> {
        self.engine.global_ctx()
    }

    /// Dart `definingType` (`_constructor.returnType`).
    fn defining_type(&self) -> TypeId {
        member::return_type(&self.ctx(), self.constructor)
    }

    fn constructor_base(&self) -> ElementId {
        member::base_element(&self.ctx(), self.constructor)
    }

    /// Dart `_initializerVisitor`.
    fn initializer_visitor(&self) -> ConstantVisitor<'_, 'a> {
        let ctx = self.ctx();
        let library = ctx
            .element_data(self.constructor_base())
            .and_then(|d| d.library)
            .unwrap_or(self.library);
        ConstantVisitor {
            engine: self.engine,
            library,
            lexical_environment: Some(&self.parameter_map),
            lexical_type_environment: Some(&self.type_parameter_map),
            substitution: Some(MapSubstitution::from_interface_type(
                &ctx,
                self.defining_type(),
            )),
            diagnostics: None,
        }
    }

    fn error_unit(&self) -> UnitHandle<'a> {
        self.engine.unit(self.error_node.unit)
    }

    /// Dart `InvalidConstant.forEntity(entity: _errorNode, ...)`.
    fn invalid_at_error_node(&self, diagnostic: LocatableDiagnostic) -> Constant {
        let u = self.error_unit();
        invalid_at_node(&u.ast, self.error_node.node, diagnostic)
    }

    fn constructor_display_name(&self) -> String {
        let ctx = self.ctx();
        let base = self.constructor_base();
        let class_name = ctx
            .element_data(base)
            .and_then(|d| d.enclosing)
            .and_then(|e| element_name(&ctx, e))
            .unwrap_or_default();
        match element_name(&ctx, base).as_deref() {
            None | Some("new") => class_name,
            Some(name) => format!("{class_name}.{name}"),
        }
    }

    fn constructor_file_path(&self) -> String {
        let ctx = self.ctx();
        let base = self.constructor_base();
        ctx.element_data(base)
            .and_then(|d| dartr_element::diagnostics::library_fragment_of(&ctx, d.first_fragment))
            .map(|lf| ctx.fragment(lf).source.path.to_string())
            .unwrap_or_default()
    }

    /// Dart `evaluateFactoryConstructorCall(arguments)`.
    fn evaluate_factory_constructor_call(&self, arguments: &[NodeRef]) -> Constant {
        let ctx = self.ctx();
        let ts = TypeSystem::new(ctx);
        let tp = self.engine.tp;
        let base = self.constructor_base();
        let defining_class = ctx.element_data(base).and_then(|d| d.enclosing);
        let defining_type = self.defining_type();
        let argument_count = arguments.len();
        let name = element_name(&ctx, base);
        let first_argument = self.argument_values.first();
        let environment = FromEnvironmentEvaluator::new(&ts, self.engine.declared_variables);
        if name.as_deref() == Some("fromEnvironment") {
            if !self.check_from_environment_arguments(arguments, defining_type) {
                return self.invalid_at_error_node(diag::const_eval_throws_exception());
            }
            let variable_name = if argument_count < 1 {
                None
            } else {
                first_argument.and_then(|a| a.to_string_value().map(str::to_string))
            };
            let default_value_parameter = member::formal_parameters(&ctx, self.constructor)
                .into_iter()
                .find(|&p| {
                    element_name(&ctx, member::base_element(&ctx, p)).as_deref()
                        == Some(DEFAULT_VALUE_PARAM)
                });
            let default_value = default_value_parameter.and_then(|p| {
                self.argument_value_map
                    .get(&member::base_element(&ctx, p))
                    .cloned()
            });

            if defining_class == Some(tp.bool_element().raw()) {
                // Special cases: https://github.com/dart-lang/sdk/issues/50045
                if matches!(
                    variable_name.as_deref(),
                    Some("dart.library.js_util") | Some("dart.library.js_interop")
                ) {
                    return Constant::Value(DartObjectImpl::new(
                        &ts,
                        tp.bool_type(),
                        InstanceState::Bool(BoolState::UNKNOWN_VALUE),
                    ));
                }
                return Constant::Value(
                    environment.get_bool(variable_name.as_deref(), default_value),
                );
            } else if defining_class == Some(tp.int_element().raw()) {
                return Constant::Value(
                    environment.get_int(variable_name.as_deref(), default_value),
                );
            } else if defining_class == Some(tp.string_element().raw()) {
                return Constant::Value(
                    environment.get_string(variable_name.as_deref(), default_value),
                );
            }
        } else if name.as_deref() == Some("hasEnvironment")
            && defining_class == Some(tp.bool_element().raw())
        {
            let variable_name = if argument_count < 1 {
                None
            } else {
                first_argument.and_then(|a| a.to_string_value().map(str::to_string))
            };
            return Constant::Value(environment.has_environment(variable_name.as_deref()));
        } else if name.as_deref() == Some("new")
            && defining_class == Some(tp.symbol_element().raw())
            && argument_count == 1
        {
            if !self.check_symbol_arguments(arguments) {
                return self.invalid_at_error_node(diag::const_eval_throws_exception());
            }
            let value = first_argument
                .and_then(|a| a.to_string_value())
                .map(Arc::from);
            return Constant::Value(DartObjectImpl::new(
                &ts,
                defining_type,
                InstanceState::Symbol(SymbolState::new(value)),
            ));
        }
        // Either it's an external const factory constructor that we can't
        // emulate, or an error occurred (a cycle, or a const constructor
        // trying to delegate to a non-const constructor).
        //
        // In the former case, the best we can do is consider it an unknown
        // value. In the latter case, the error has already been reported,
        // so considering it an unknown value will suppress further errors.
        Constant::Value(DartObjectImpl::valid_with_unknown_value(&ts, defining_type))
    }

    /// Dart `evaluateGenerativeConstructorCall()`.
    fn evaluate_generative_constructor_call(&mut self) -> Constant {
        self.check_type_parameters();

        if let Some(error) = self.check_parameters() {
            return error;
        }

        // Redirecting constructors delegate this to the target constructor.
        let initializers = self.engine.constant_initializers(self.constructor_base());
        let has_redirecting = initializers.iter().any(|i| {
            self.engine.unit(i.unit).ast.kind(i.node) == NodeKind::RedirectingConstructorInvocation
        });
        if !has_redirecting && let Some(error) = self.check_fields() {
            return error;
        }

        let evaluation_result = self.check_initializers(&initializers);
        if let Some(result) = evaluation_result.result
            && evaluation_result.evaluation_is_complete
        {
            return result;
        }

        if let Some(error) = self.check_super_constructor_call(
            evaluation_result.super_name,
            evaluation_result.super_arguments,
        ) {
            return error;
        }

        let ctx = self.ctx();
        let ts = TypeSystem::new(ctx);
        let defining_type = self.defining_type();
        if let Some(element) = ctx.interface_element(defining_type)
            && element.raw().tag() == Tag::ExtensionType
        {
            let representation_name = ctx
                .instance(element.upcast())
                .fields
                .first()
                .and_then(|f| element_name(&ctx, f.raw()));
            // Dart `element.representation.name`.
            let representation_name =
                extension_type_representation_name(&ctx, element.raw()).or(representation_name);
            if let Some(representation) =
                representation_name.and_then(|n| self.field_map.get(n.as_str()))
            {
                return Constant::Value(DartObjectImpl::with_extension_type(
                    representation.ty,
                    defining_type,
                    representation.state.clone(),
                    representation.variable,
                ));
            }
        }

        Constant::Value(DartObjectImpl::new(
            &ts,
            defining_type,
            InstanceState::Generic(Arc::new(GenericState::new(
                std::mem::take(&mut self.field_map),
                Some(self.invocation.clone()),
                false,
            ))),
        ))
    }

    /// Dart `_checkFields()`.
    fn check_fields(&mut self) -> Option<Constant> {
        let ctx = self.ctx();
        let ts = TypeSystem::new(ctx);
        let substitution = MapSubstitution::from_interface_type(&ctx, self.defining_type());
        let interface_element = ctx
            .element_data(self.constructor_base())
            .and_then(|d| d.enclosing)?;
        let instance = interface_element.cast::<dartr_element::InstanceElement>()?;
        let can_reuse_field_value = !has_primary_constructor(&ctx, interface_element);
        let fields = ctx.instance(instance).fields.clone();
        for field in fields {
            let f = field.raw();
            if (is_final_variable(&ctx, f) || is_const_variable(&ctx, f))
                && !is_static_variable(&ctx, f)
            {
                let Some(initializer) = self.engine.constant_initializer(f) else {
                    continue;
                };

                let mut field_value = if can_reuse_field_value {
                    self.engine.evaluation_result(f)
                } else {
                    None
                };
                if field_value.is_none() {
                    field_value = Some(self.initializer_visitor().evaluate_constant(initializer));
                }
                let field_value = match field_value.unwrap() {
                    invalid @ Constant::Invalid(_) => return Some(invalid),
                    Constant::Value(v) => v,
                };

                // Match the value and the type.
                let declared_type = variable_type(&ctx, f);
                let field_type = substitution.substitute_type(&ctx, declared_type);
                if !runtime_type_match(&ts, &field_value, field_type) {
                    let is_runtime_exception = has_type_parameter_reference(&ctx, declared_type);
                    let u = self.engine.unit(initializer.unit);
                    let mut invalid = invalid_constant_at_node(
                        &u.ast,
                        initializer.node,
                        diag::const_constructor_field_type_mismatch(
                            &ts.display_type(field_value.ty),
                            &element_name(&ctx, f).unwrap_or_default(),
                            &ts.display_type(field_type),
                        ),
                    );
                    invalid.is_runtime_exception = is_runtime_exception;
                    return Some(Constant::Invalid(Box::new(invalid)));
                }

                // Skip, if the field was already initialized by an
                // initializing formal.
                if let Some(field_name) = element_name(&ctx, f)
                    && !self.field_map.contains_key(field_name.as_str())
                {
                    self.field_map
                        .insert(Arc::from(field_name.as_str()), field_value);
                }
            }
        }
        None
    }

    /// Dart `_checkFromEnvironmentArguments`.
    fn check_from_environment_arguments(
        &self,
        arguments: &[NodeRef],
        expected_default_value_type: TypeId,
    ) -> bool {
        let tp = self.engine.tp;
        let argument_count = arguments.len();
        if !(1..=2).contains(&argument_count) {
            return false;
        }
        let first_kind = self
            .engine
            .unit(arguments[0].unit)
            .ast
            .kind(arguments[0].node);
        if first_kind == NodeKind::NamedArgument {
            return false;
        }
        if self.argument_values.first().map(|a| a.ty) != Some(tp.string_type()) {
            return false;
        }
        if argument_count == 2 {
            let second_argument = arguments[1];
            let u = self.engine.unit(second_argument.unit);
            let ast = &u.ast;
            let Some(named) = ast.cast::<NamedArgument>(second_argument.node) else {
                return false;
            };
            if ast.tokens.lexeme(ast[named].name) != DEFAULT_VALUE_PARAM {
                return false;
            }
            let ctx = self.engine.ctx(&u);
            let Some(element) = u.tables.param_element.get(second_argument.node).copied() else {
                return false;
            };
            let base = member::base_element(&ctx, element);
            if !is_formal_parameter(base) {
                return false;
            }
            let Some(default_value) = self.argument_value_map.get(&base) else {
                return false;
            };
            let default_value_type = default_value.ty;
            if !(default_value_type == expected_default_value_type
                || default_value_type == tp.null_type())
            {
                return false;
            }
        }
        true
    }

    /// Dart `_checkInitializers()`.
    fn check_initializers(&mut self, initializers: &[NodeRef]) -> InitializersEvaluationResult {
        let ctx = self.ctx();
        let ts = TypeSystem::new(ctx);
        // If we encounter a superinitializer, store the name of the
        // constructor, and the arguments.
        let mut super_name: Option<String> = None;
        let mut super_arguments: Option<Vec<NodeRef>> = None;
        for &initializer in initializers {
            let u = self.engine.unit(initializer.unit);
            let ast = &u.ast;
            match ast.kind(initializer.node) {
                NodeKind::ConstructorFieldInitializer => {
                    let i = &ast[Id::<ConstructorFieldInitializer>::from_raw(initializer.node)];
                    let initializer_expression = initializer.with(i.expression);
                    let evaluation_result = self
                        .initializer_visitor()
                        .evaluate_constant(initializer_expression);
                    match evaluation_result {
                        Constant::Value(value) => {
                            let field_name =
                                ast_ext::identifier_name(ast, i.field_name).to_string();
                            self.field_map
                                .insert(Arc::from(field_name.as_str()), value.clone());
                            let getter =
                                lookup::type_get_getter(&ctx, self.defining_type(), &field_name);
                            if let Some(getter) = getter
                                && let Some(field) = member::variable(&ctx, getter)
                            {
                                let field_type = member::type_(&ctx, field);
                                if !runtime_type_match(&ts, &value, field_type) {
                                    // Mark the type mismatch error as a
                                    // runtime exception if the initializer
                                    // is statically assignable to the field.
                                    let lib_ctx = self.engine.ctx(&u);
                                    let lib_ts = TypeSystem::new(lib_ctx);
                                    let expression_type = u
                                        .tables
                                        .static_type
                                        .get(i.expression)
                                        .copied()
                                        .unwrap_or(TypeId::INVALID);
                                    let is_runtime_exception =
                                        lib_ts.is_assignable_to(expression_type, field_type, false);
                                    let diagnostic = diag::const_constructor_field_type_mismatch(
                                        &ts.display_type(value.ty),
                                        &field_name,
                                        &ts.display_type(field_type),
                                    );
                                    let mut invalid = if is_runtime_exception {
                                        invalid_constant_at_node(
                                            ast,
                                            initializer_expression.node,
                                            diagnostic,
                                        )
                                    } else {
                                        let eu = self.error_unit();
                                        invalid_constant_at_node(
                                            &eu.ast,
                                            self.error_node.node,
                                            diagnostic,
                                        )
                                    };
                                    invalid.is_runtime_exception = is_runtime_exception;
                                    return InitializersEvaluationResult::complete(
                                        Constant::Invalid(Box::new(invalid)),
                                    );
                                }
                            }
                        }
                        Constant::Invalid(mut invalid) if !invalid.is_runtime_exception => {
                            // Add additional information to the error in the
                            // field initializer because the error is
                            // reported at the location of [_errorNode].
                            if invalid.locatable_diagnostic.context_messages.is_empty() {
                                let message = DiagnosticMessage {
                                    file_path: self.constructor_file_path(),
                                    length: invalid.length,
                                    message: format!(
                                        "The error is in the field initializer of '{}', and occurs here.",
                                        self.constructor_display_name()
                                    ),
                                    offset: invalid.offset,
                                    url: None,
                                };
                                invalid.locatable_diagnostic.context_messages.push(message);
                            }
                            let eu = self.error_unit();
                            return InitializersEvaluationResult::complete(copy_with_entity(
                                &invalid,
                                &eu.ast,
                                self.error_node.node,
                            ));
                        }
                        invalid => return InitializersEvaluationResult::complete(invalid),
                    }
                }
                NodeKind::SuperConstructorInvocation => {
                    let i = &ast[Id::<SuperConstructorInvocation>::from_raw(initializer.node)];
                    if let Some(name) = i.constructor_name {
                        super_name = Some(ast_ext::identifier_name(ast, name).to_string());
                    }
                    super_arguments = Some(
                        argument_nodes(ast, i.argument_list)
                            .into_iter()
                            .map(|a| initializer.with(a))
                            .collect(),
                    );
                }
                NodeKind::RedirectingConstructorInvocation => {
                    // This is a redirecting constructor, so just evaluate
                    // the constructor it redirects to.
                    let lib_ctx = self.engine.ctx(&u);
                    let base_element = u
                        .tables
                        .element
                        .get(initializer.node)
                        .map(|&e| member::base_element(&lib_ctx, e));
                    if let Some(base_element) = base_element
                        && base_element.tag() == Tag::Constructor
                        && is_const_constructor(&ctx, base_element)
                    {
                        // Instantiate the constructor with the in-scope type
                        // arguments.
                        let constructor =
                            member::constructor_from2(&ctx, base_element, self.defining_type());
                        let i = &ast
                            [Id::<RedirectingConstructorInvocation>::from_raw(initializer.node)];
                        let arguments: Vec<NodeRef> = argument_nodes(ast, i.argument_list)
                            .into_iter()
                            .map(|a| initializer.with(a))
                            .collect();
                        let visitor = self.initializer_visitor();
                        let result = InstanceCreationEvaluator::evaluate(
                            self.engine,
                            self.library,
                            self.error_node,
                            constructor,
                            self.type_arguments.clone(),
                            &arguments,
                            &visitor,
                            Some(self.invocation.clone()),
                            &IndexMap::new(),
                        );
                        return InitializersEvaluationResult::complete(result);
                    }
                }
                NodeKind::AssertInitializer => {
                    let i = &ast[Id::<AssertInitializer>::from_raw(initializer.node)];
                    let condition = initializer.with(i.condition);
                    let evaluation_result = self.initializer_visitor().evaluate_constant(condition);
                    match evaluation_result {
                        Constant::Value(value) => {
                            if !value.is_bool() || value.to_bool_value() == Some(false) {
                                let mut invalid_constant = None;

                                // Adds the assert message if we are able to
                                // evaluate it.
                                if let Some(message) = i.message
                                    && let Constant::Value(message_constant) = self
                                        .initializer_visitor()
                                        .evaluate_constant(initializer.with(message))
                                    && let Some(assert_message) = message_constant.to_string_value()
                                {
                                    let mut c = invalid_constant_at_node(
                                        ast,
                                        initializer.node,
                                        diag::const_eval_assertion_failure_with_message(
                                            assert_message,
                                        ),
                                    );
                                    c.is_runtime_exception = true;
                                    invalid_constant = Some(c);
                                }

                                let invalid_constant = invalid_constant.unwrap_or_else(|| {
                                    let mut c = invalid_constant_at_node(
                                        ast,
                                        initializer.node,
                                        diag::const_eval_assertion_failure(),
                                    );
                                    c.is_runtime_exception = true;
                                    c
                                });
                                return InitializersEvaluationResult::complete(Constant::Invalid(
                                    Box::new(invalid_constant),
                                ));
                            }
                        }
                        Constant::Invalid(mut invalid) if !invalid.is_runtime_exception => {
                            // Add additional information to the error in the
                            // assert initializer because the error is
                            // reported at the location of [_errorNode].
                            if invalid.locatable_diagnostic.context_messages.is_empty() {
                                let message = DiagnosticMessage {
                                    file_path: self.constructor_file_path(),
                                    length: invalid.length,
                                    message: format!(
                                        "The error is in the assert initializer of '{}', and occurs here.",
                                        self.constructor_display_name()
                                    ),
                                    offset: invalid.offset,
                                    url: None,
                                };
                                invalid.locatable_diagnostic.context_messages.push(message);
                            }
                            let eu = self.error_unit();
                            return InitializersEvaluationResult::complete(copy_with_entity(
                                &invalid,
                                &eu.ast,
                                self.error_node.node,
                            ));
                        }
                        invalid => return InitializersEvaluationResult::complete(invalid),
                    }
                }
                _ => {}
            }
        }

        // Dart: the synthetic forwarding constructors of a mixin
        // application have the constant initializer `super.name(args)`
        // (built by the linker, not in the AST).
        if initializers.is_empty() && super_arguments.is_none() {
            let base = self.constructor_base();
            if first_fragment_flags(&ctx, base)
                .contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_MIXIN_APPLICATION)
            {
                super_name = element_name(&ctx, base).filter(|n| n != "new");
            }
        }

        if ctx.superclass(self.defining_type()).is_some() && super_arguments.is_none() {
            super_arguments = Some(Vec::new());
        }

        InitializersEvaluationResult {
            result: None,
            evaluation_is_complete: false,
            super_name,
            super_arguments,
        }
    }

    /// Dart `_checkParameters()`.
    fn check_parameters(&mut self) -> Option<Constant> {
        let ctx = self.ctx();
        let ts = TypeSystem::new(ctx);
        let parameters = member::formal_parameters(&ctx, self.constructor);
        for parameter in parameters {
            let base_parameter = member::base_element(&ctx, parameter);
            let mut argument_value = self.argument_value_map.get(&base_parameter).cloned();
            // No argument node that we can direct error messages to, because
            // we are handling an optional parameter that wasn't specified.
            // So just direct error messages to the constructor call.
            let error_target = self
                .argument_node_map
                .get(&base_parameter)
                .copied()
                .flatten()
                .unwrap_or(self.error_node);
            let parameter_data = ctx.get(EId::<FormalParameterElement>::from_raw(base_parameter));
            if argument_value.is_none() && parameter_data.kind.is_optional() {
                // The parameter is an optional positional parameter for
                // which no value was provided, so use the default value.
                match self.engine.evaluation_result(base_parameter) {
                    // No default was provided, so the default value is null.
                    None => argument_value = Some(null_object(&ts)),
                    Some(Constant::Value(v)) => argument_value = Some(v),
                    Some(Constant::Invalid(_)) => {}
                }
            }
            if let Some(argument_value) = argument_value {
                let parameter_type = member::type_(&ctx, parameter);
                if !argument_value.is_invalid(&ctx)
                    && !runtime_type_match(&ts, &argument_value, parameter_type)
                {
                    // Mark the type mismatch error as a runtime exception if
                    // the argument is statically assignable to the
                    // parameter.
                    let eu = self.engine.unit(error_target.unit);
                    let lib_ts = TypeSystem::new(self.engine.ctx(&eu));
                    let is_evaluation_exception =
                        eu.ast.cast::<Expression>(error_target.node).is_some()
                            && lib_ts.is_assignable_to(
                                eu.tables
                                    .static_type
                                    .get(error_target.node)
                                    .copied()
                                    .unwrap_or(TypeId::INVALID),
                                parameter_type,
                                false,
                            );
                    let mut invalid = invalid_constant_at_node(
                        &eu.ast,
                        error_target.node,
                        diag::const_constructor_param_type_mismatch(
                            &ts.display_type(argument_value.ty),
                            &ts.display_type(parameter_type),
                        ),
                    );
                    invalid.is_runtime_exception = is_evaluation_exception;
                    return Some(Constant::Invalid(Box::new(invalid)));
                }
                if base_parameter.tag() == Tag::FieldFormalParameter
                    && let Some(field) = parameter_data.field.get()
                {
                    let field_type = member::type_(
                        &ctx,
                        member::substitute(
                            &ctx,
                            ElemRef::Base(field.raw()),
                            &member::substitution(&ctx, self.constructor),
                        ),
                    );
                    if field_type != parameter_type {
                        // We've already checked that the argument can be
                        // assigned to the parameter; we also need to check
                        // that it can be assigned to the field.
                        if !argument_value.is_invalid(&ctx)
                            && !runtime_type_match(&ts, &argument_value, field_type)
                        {
                            let eu = self.engine.unit(error_target.unit);
                            return Some(invalid_at_node(
                                &eu.ast,
                                error_target.node,
                                diag::const_constructor_param_type_mismatch(
                                    &ts.display_type(argument_value.ty),
                                    &ts.display_type(field_type),
                                ),
                            ));
                        }
                    }
                    let field_name = element_name(&ctx, field.raw()).unwrap_or_default();
                    self.field_map
                        .insert(Arc::from(field_name.as_str()), argument_value.clone());
                }
                self.parameter_map.insert(base_parameter, argument_value);
            }
        }
        None
    }

    /// Dart `_checkSuperConstructorCall`.
    fn check_super_constructor_call(
        &mut self,
        super_name: Option<String>,
        super_arguments: Option<Vec<NodeRef>>,
    ) -> Option<Constant> {
        let ctx = self.ctx();
        let superclass = ctx.superclass(self.defining_type())?;
        if ctx.is_dart_core_object(superclass) {
            return None;
        }
        let constructor_library = ctx
            .element_data(self.constructor_base())
            .and_then(|d| d.library)
            .unwrap_or(self.library);
        let super_constructor = lookup::type_look_up_constructor(
            &ctx,
            superclass,
            super_name.as_deref(),
            constructor_library,
        )?;

        if !is_const_constructor(&ctx, member::base_element(&ctx, super_constructor)) {
            return None;
        }
        let implicit_super_arguments = self.collect_implicit_super_formal_arguments();
        let visitor = self.initializer_visitor();
        let evaluation_result = InstanceCreationEvaluator::evaluate(
            self.engine,
            self.library,
            self.error_node,
            super_constructor,
            Some(ctx.type_arguments(superclass).to_vec()),
            &super_arguments.unwrap_or_default(),
            &visitor,
            None,
            &implicit_super_arguments,
        );
        drop(visitor);
        match evaluation_result {
            Constant::Value(v) => {
                self.field_map
                    .insert(Arc::from(GenericState::SUPERCLASS_FIELD), v);
                None
            }
            Constant::Invalid(mut invalid) if !invalid.is_runtime_exception => {
                // Add additional information to the error in the super
                // constructor call because the error is reported at the
                // location of [_errorNode].
                if invalid.locatable_diagnostic.context_messages.is_empty() {
                    let message = DiagnosticMessage {
                        file_path: self.constructor_file_path(),
                        length: invalid.length,
                        message: format!(
                            "The error is in the super constructor invocation of '{}', and occurs here.",
                            self.constructor_display_name()
                        ),
                        offset: invalid.offset,
                        url: None,
                    };
                    invalid.locatable_diagnostic.context_messages.push(message);
                } else {
                    let message = self.stack_trace_context_message(super_constructor);
                    invalid.locatable_diagnostic.context_messages.push(message);
                }
                let eu = self.error_unit();
                Some(copy_with_entity(&invalid, &eu.ast, self.error_node.node))
            }
            Constant::Invalid(mut invalid) => {
                let message = self.stack_trace_context_message(super_constructor);
                invalid.locatable_diagnostic.context_messages.push(message);
                Some(Constant::Invalid(invalid))
            }
        }
    }

    /// Dart `_checkSymbolArguments`.
    fn check_symbol_arguments(&self, arguments: &[NodeRef]) -> bool {
        if arguments.len() != 1 {
            return false;
        }
        if self
            .engine
            .unit(arguments[0].unit)
            .ast
            .kind(arguments[0].node)
            == NodeKind::NamedArgument
        {
            return false;
        }
        let Some(first) = self.argument_values.first() else {
            return false;
        };
        if first.ty != self.engine.tp.string_type() {
            return false;
        }
        first.to_string_value().is_some()
    }

    /// Dart `_checkTypeParameters()`.
    fn check_type_parameters(&mut self) {
        let ctx = self.ctx();
        let Some(enclosing) = ctx
            .element_data(self.constructor_base())
            .and_then(|d| d.enclosing)
        else {
            return;
        };
        let Some(instance) = enclosing.cast::<dartr_element::InstanceElement>() else {
            return;
        };
        let type_parameters = ctx.instance(instance).type_params.clone();
        if let Some(type_arguments) = &self.type_arguments
            && !type_parameters.is_empty()
            && type_parameters.len() == type_arguments.len()
        {
            for (p, &a) in type_parameters.iter().zip(type_arguments) {
                self.type_parameter_map.insert(p.raw(), a);
            }
        }
    }

    /// Dart `_collectImplicitSuperFormalArguments()`.
    fn collect_implicit_super_formal_arguments(&self) -> IndexMap<ElementId, DartObjectImpl> {
        let ctx = self.ctx();
        let mut result = IndexMap::new();
        for parameter in member::formal_parameters(&ctx, self.constructor) {
            let base = member::base_element(&ctx, parameter);
            if base.tag() != Tag::SuperFormalParameter {
                continue;
            }
            if let Some(super_parameter) = super_constructor_parameter(&ctx, base)
                && let Some(value) = self.parameter_map.get(&base)
            {
                result.insert(member::base_element(&ctx, super_parameter), value.clone());
            }
        }
        // Dart: a synthetic forwarding constructor of a mixin application
        // passes its parameters to the super constructor.
        let base = self.constructor_base();
        if first_fragment_flags(&ctx, base)
            .contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_MIXIN_APPLICATION)
            && let Some(super_constructor) = ctx
                .get(EId::<dartr_element::ConstructorElement>::from_raw(base))
                .super_constructor
                .get()
        {
            let super_parameters = member::formal_parameters(&ctx, super_constructor);
            for (p, sp) in member::formal_parameters(&ctx, self.constructor)
                .into_iter()
                .zip(super_parameters)
            {
                if let Some(value) = self.parameter_map.get(&member::base_element(&ctx, p)) {
                    result.insert(member::base_element(&ctx, sp), value.clone());
                }
            }
        }
        result
    }

    /// Dart `_stackTraceContextMessage`.
    fn stack_trace_context_message(&self, super_constructor: ElemRef) -> DiagnosticMessage {
        let ctx = self.ctx();
        let super_display_name = {
            let base = member::base_element(&ctx, super_constructor);
            let class_name = ctx
                .element_data(base)
                .and_then(|d| d.enclosing)
                .and_then(|e| element_name(&ctx, e))
                .unwrap_or_default();
            match element_name(&ctx, base).as_deref() {
                None | Some("new") => class_name,
                Some(name) => format!("{class_name}.{name}"),
            }
        };
        let display_name = self.constructor_display_name();
        let offset = ctx
            .element_data(self.constructor_base())
            .and_then(|d| ctx.fragment_data(d.first_fragment))
            .and_then(|f| f.name_offset.or(f.first_token_offset))
            .map_or(-1, i64::from);
        DiagnosticMessage {
            file_path: self.constructor_file_path(),
            length: 1,
            message: format!(
                "The evaluated constructor '{super_display_name}' is called by '{display_name}' and \
                 '{display_name}' is defined here."
            ),
            offset,
            url: None,
        }
    }

    /// Dart `_InstanceCreationEvaluator.evaluate(...)`.
    #[allow(clippy::too_many_arguments)]
    fn evaluate(
        engine: &'e ConstantEvaluationEngine<'a>,
        library: EId<LibraryElement>,
        node: NodeRef,
        constructor: ElemRef,
        type_arguments: Option<Vec<TypeId>>,
        arguments: &[NodeRef],
        constant_visitor: &ConstantVisitor<'_, 'a>,
        invocation: Option<Arc<ConstructorInvocationImpl>>,
        implicit_argument_values: &IndexMap<ElementId, DartObjectImpl>,
    ) -> Constant {
        let ctx = engine.global_ctx();
        let base = member::base_element(&ctx, constructor);
        if !is_const_constructor(&ctx, base) {
            let u = engine.unit(node.unit);
            let ast = &u.ast;
            let keyword = match ast.kind(node.node) {
                NodeKind::InstanceCreationExpression => {
                    ast[Id::<InstanceCreationExpression>::from_raw(node.node)].keyword
                }
                NodeKind::DotShorthandConstructorInvocation => {
                    ast[Id::<DotShorthandConstructorInvocation>::from_raw(node.node)].const_keyword
                }
                _ => None,
            };
            return Constant::Invalid(Box::new(match keyword {
                Some(keyword) => {
                    invalid_constant_at_token(ast, keyword, diag::const_with_non_const())
                }
                None => invalid_constant_at_node(ast, node.node, diag::const_with_non_const()),
            }));
        }

        let ts = TypeSystem::new(ctx);
        if !engine.is_cycle_free(base) {
            // It's not safe to evaluate this constructor, so bail out.
            //
            // Instead of reporting an error at the call-sites, we will
            // report an error at each constructor in
            // [ConstantVerifier.visitConstructorDeclaration].
            return Constant::Value(DartObjectImpl::valid_with_unknown_value(
                &ts,
                member::return_type(&ctx, constructor),
            ));
        }

        let formal_parameters = member::formal_parameters(&ctx, constructor);
        let is_positional = |p: ElemRef| {
            ctx.get(EId::<FormalParameterElement>::from_raw(
                member::base_element(&ctx, p),
            ))
            .kind
            .is_positional()
        };
        let positional_parameters: Vec<ElemRef> = formal_parameters
            .iter()
            .copied()
            .filter(|&p| is_positional(p))
            .collect();
        let mut positional_parameter_index = 0;
        let mut argument_value_map: IndexMap<ElementId, DartObjectImpl> = IndexMap::new();
        let mut argument_node_map: IndexMap<ElementId, Option<NodeRef>> = IndexMap::new();
        for &argument in arguments {
            let au = engine.unit(argument.unit);
            let actx = engine.ctx(&au);
            // Use the corresponding parameter type as the default value if
            // an unresolved expression is evaluated. We do this to continue
            // the rest of the evaluation without producing unrelated errors.
            if let Some(named) = au.ast.cast::<NamedArgument>(argument.node) {
                let corresponding = au.tables.param_element.get(argument.node).copied();
                let parameter_type =
                    corresponding.map_or(TypeId::INVALID, |p| member::type_(&actx, p));
                let argument_constant = constant_visitor.value_of(
                    argument.with(au.ast[named].argument_expression),
                    parameter_type,
                );
                let Constant::Value(argument_constant) = argument_constant else {
                    return argument_constant;
                };
                if let Some(corresponding) = corresponding {
                    let base_parameter = member::base_element(&actx, corresponding);
                    if is_formal_parameter(base_parameter) {
                        argument_value_map.insert(base_parameter, argument_constant);
                        argument_node_map.insert(base_parameter, Some(argument));
                    }
                }
            } else {
                let parameter = positional_parameters
                    .get(positional_parameter_index)
                    .copied();
                positional_parameter_index += 1;
                let parameter_type = parameter.map_or(TypeId::INVALID, |p| member::type_(&ctx, p));
                let argument_constant = constant_visitor.value_of(argument, parameter_type);
                let Constant::Value(argument_constant) = argument_constant else {
                    return argument_constant;
                };
                if let Some(parameter) = parameter {
                    let base_parameter = member::base_element(&ctx, parameter);
                    argument_value_map.insert(base_parameter, argument_constant);
                    argument_node_map.insert(base_parameter, Some(argument));
                }
            }
        }

        for (parameter, value) in implicit_argument_values {
            argument_value_map.insert(*parameter, value.clone());
            argument_node_map.insert(*parameter, None);
        }

        let mut invocation_positional_values = Vec::new();
        let mut invocation_positional_nodes = Vec::new();
        let mut invocation_named_values = FieldMap::new();
        let mut invocation_named_nodes: IndexMap<String, Option<NodeRef>> = IndexMap::new();
        for &parameter in &formal_parameters {
            let base_parameter = member::base_element(&ctx, parameter);
            if let Some(value) = argument_value_map.get(&base_parameter) {
                if is_positional(parameter) {
                    invocation_positional_values.push(value.clone());
                    invocation_positional_nodes
                        .push(argument_node_map.get(&base_parameter).copied().flatten());
                } else if let Some(parameter_name) = element_name(&ctx, base_parameter) {
                    invocation_named_values
                        .insert(Arc::from(parameter_name.as_str()), value.clone());
                    invocation_named_nodes.insert(
                        parameter_name,
                        argument_node_map.get(&base_parameter).copied().flatten(),
                    );
                }
            }
        }

        let invocation = invocation.unwrap_or_else(|| {
            Arc::new(ConstructorInvocationImpl {
                constructor,
                positional_arguments: invocation_positional_values.clone(),
                named_arguments: invocation_named_values.clone(),
            })
        });

        let redirection_result = follow_constant_redirection_chain(
            &ctx,
            constructor,
            &invocation_positional_values,
            &invocation_positional_nodes,
            &invocation_named_values,
            &invocation_named_nodes,
            argument_value_map,
            argument_node_map,
        );
        let constructor = redirection_result.constructor;

        let mut evaluator = InstanceCreationEvaluator {
            engine,
            library,
            error_node: node,
            constructor,
            type_arguments,
            invocation,
            argument_values: invocation_positional_values,
            argument_value_map: redirection_result.argument_value_map,
            argument_node_map: redirection_result.argument_node_map,
            type_parameter_map: IndexMap::new(),
            parameter_map: IndexMap::new(),
            field_map: FieldMap::new(),
        };

        if is_factory_constructor(&ctx, member::base_element(&ctx, constructor)) {
            // We couldn't find a non-factory constructor.
            // See if it's because we reached an external const factory
            // constructor that we can emulate.
            evaluator.evaluate_factory_constructor_call(arguments)
        } else {
            evaluator.evaluate_generative_constructor_call()
        }
    }
}

/// Dart `_followConstantRedirectionChain`.
#[allow(clippy::too_many_arguments)]
fn follow_constant_redirection_chain(
    ctx: &Ctx<'_>,
    original_constructor: ElemRef,
    positional_values: &[DartObjectImpl],
    positional_nodes: &[Option<NodeRef>],
    named_values: &FieldMap,
    named_nodes: &IndexMap<String, Option<NodeRef>>,
    argument_value_map: IndexMap<ElementId, DartObjectImpl>,
    argument_node_map: IndexMap<ElementId, Option<NodeRef>>,
) -> RedirectionResult {
    let mut constructor = original_constructor;
    let mut constructors_visited = IndexSet::new();
    while let Some(redirected_constructor) = get_const_redirected_constructor(ctx, constructor) {
        constructors_visited.insert(member::base_element(ctx, constructor));
        if constructors_visited.contains(&member::base_element(ctx, redirected_constructor)) {
            // Cycle in redirecting factory constructors--this is not allowed
            // and is checked elsewhere.
            break;
        }
        constructor = redirected_constructor;
    }

    if constructor == original_constructor {
        return RedirectionResult {
            constructor,
            argument_value_map,
            argument_node_map,
        };
    }

    let mut result_argument_value_map = IndexMap::new();
    let mut result_argument_node_map = IndexMap::new();
    let mut positional_index = 0;
    for parameter in member::formal_parameters(ctx, constructor) {
        let base_parameter = member::base_element(ctx, parameter);
        let kind = ctx
            .get(EId::<FormalParameterElement>::from_raw(base_parameter))
            .kind;
        if kind.is_positional() {
            if positional_index < positional_values.len() {
                result_argument_value_map
                    .insert(base_parameter, positional_values[positional_index].clone());
                result_argument_node_map.insert(base_parameter, positional_nodes[positional_index]);
            }
            positional_index += 1;
        } else if let Some(parameter_name) = element_name(ctx, base_parameter)
            && let Some(value) = named_values.get(parameter_name.as_str())
        {
            result_argument_value_map.insert(base_parameter, value.clone());
            result_argument_node_map.insert(
                base_parameter,
                named_nodes.get(&parameter_name).copied().flatten(),
            );
        }
    }

    RedirectionResult {
        constructor,
        argument_value_map: result_argument_value_map,
        argument_node_map: result_argument_node_map,
    }
}

/// Dart `InterfaceElement.primaryConstructor != null`.
fn has_primary_constructor(ctx: &Ctx<'_>, interface: ElementId) -> bool {
    let Some(element) = interface.cast::<InterfaceElement>() else {
        return false;
    };
    ctx.interface(element).constructors.iter().any(|c| {
        first_fragment_flags(ctx, c.raw()).contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_PRIMARY)
    })
}

/// Dart `ExtensionTypeElement.representation.name`.
fn extension_type_representation_name(ctx: &Ctx<'_>, element: ElementId) -> Option<String> {
    let instance = element.cast::<dartr_element::InstanceElement>()?;
    ctx.instance(instance)
        .fields
        .iter()
        .find(|f| !is_static_variable(ctx, f.raw()))
        .and_then(|f| element_name(ctx, f.raw()))
}

#[allow(dead_code)]
fn unused(_: Id<CompilationUnit>, _: FieldElement) {}
