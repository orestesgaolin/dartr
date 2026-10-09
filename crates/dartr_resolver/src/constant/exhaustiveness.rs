// Dart source: pkg/analyzer/lib/src/generated/exhaustiveness.dart (+ ConstantVerifier._validateSwitchExhaustiveness)

//! The analyzer client of the shared exhaustiveness checker
//! (`dartr_type_analyzer::exhaustiveness`): [`AnalyzerTypeOperations`],
//! [`AnalyzerEnumOperations`], [`AnalyzerSealedClassOperations`],
//! [`PatternConverter`], [`TypeParameterReplacer`], and the switch check of
//! the constant verifier ([`validate_switch_exhaustiveness`]).
//!
//! # Mapping from Dart
//!
//! - Dart `TypeImpl` is [`TypeId`]. `TypeId ==` is Dart `identical`, not Dart
//!   `==` (see the `dartr_element` README); the shared cache uses it as the
//!   key of its type caches.
//! - Dart `DartObject` values are used as keys (enum element values, unique
//!   constant identities, map pattern keys). [`DartObjectImpl`] has no
//!   `Eq`/`Hash` without a type system, so the keys are [`EnumValueKey`] and
//!   [`ConstIdentity`]: the type and the display string of the state.
//! - `AnalyzerExhaustivenessCache` is created per call: the static types are
//!   handles into an arena that borrows the type system of one unit (with
//!   its local arena), so they cannot outlive the call. [`ExhaustivenessCache`]
//!   (one per library analysis in Dart) keeps no state.
//! - `AnalyzerDartTemplateBuffer`, `MissingPatternPart` (data for the quick
//!   fix of the analysis server) and `ExhaustivenessDataForTesting` are not
//!   ported.

use std::cell::RefCell;

use dartr_ast::{
    Ast, CastPattern, ConstantPattern, DeclaredVariablePattern, ListPattern, LogicalAndPattern,
    LogicalOrPattern, MapPattern, MapPatternEntry, NodeId, NullAssertPattern, NullCheckPattern,
    ObjectPattern, ParenthesizedPattern, PatternField, RecordPattern, RelationalPattern,
    RestPatternElement, SwitchDefault, SwitchExpression, SwitchExpressionCase, SwitchPatternCase,
    SwitchStatement, WildcardPattern,
};
use dartr_constant::{DartObjectImpl, InstanceState};
use dartr_diagnostics::{Diagnostic, LocatableDiagnostic, diag};
use dartr_element::{
    ClassElement, Ctx, DisplayOptions, EId, ElemRef, EnumElement, ExtensionElement,
    ExtensionTypeElement, FeatureSet, FieldElement, FragmentFlags, GetterElement, InterfaceElement,
    LibraryElement, NamedType as NamedField, Nullability, SetterElement, TypeId, TypeKind, Version,
};
use dartr_syntax::TokenId;
use dartr_type_analyzer::exhaustiveness::{
    self as ex, EnumOperations, Identity, Key, ListTypeRestriction, MapKey, MapTypeRestriction,
    ObjectPropertyLookup, Path, SealedClassOperations, SimpleDartBuffer, Space, SpaceCreator,
    StaticType, StaticTypeArena, TypeOperations,
};
use dartr_typesystem::replacement_visitor::ReplacementVisitor;
use dartr_typesystem::{MapSubstitution, TypeExt, TypeSystem, member};
use indexmap::IndexMap;

use crate::ast_ext;
use crate::constant::evaluation::ConstantEvaluationEngine;
use crate::element_ext;
use crate::library_analyzer::ResolvedUnit;
use crate::pattern_resolver::variable_pattern_name;

// ---------------------------------------------------------------------------
// Public contract
// ---------------------------------------------------------------------------

/// Dart `AnalyzerExhaustivenessCache` (one per library analysis).
///
/// The shared cache is created per call of [validate_switch_exhaustiveness]
/// (see the module documentation), so this type keeps no state.
#[derive(Default)]
pub struct ExhaustivenessCache {
    _private: (),
}

/// The arguments of Dart `ConstantVerifier._validateSwitchExhaustiveness`.
pub struct SwitchExhaustivenessInput<'r> {
    /// A `SwitchStatement` or a `SwitchExpression`.
    pub node: NodeId,
    /// Dart `_mapPatternKeyValues` (key `Expression` node → value).
    pub map_pattern_key_values: &'r IndexMap<NodeId, DartObjectImpl>,
    /// Dart `_constantPatternValues` (`ConstantPattern` node → value).
    pub constant_pattern_values: &'r IndexMap<NodeId, DartObjectImpl>,
    pub must_be_exhaustive: bool,
    pub is_switch_expression: bool,
}

/// Dart `ConstantVerifier._validateSwitchExhaustiveness`: reports
/// `nonExhaustiveSwitch*`, `unreachableSwitchCase` and
/// `unreachableSwitchDefault` for the switch `input.node` of the unit
/// [unit] of the [engine].
pub fn validate_switch_exhaustiveness(
    engine: &ConstantEvaluationEngine<'_>,
    cache: &mut ExhaustivenessCache,
    unit: u32,
    input: &SwitchExhaustivenessInput<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let _ = cache;
    let u = engine.unit(unit);
    let ctx = engine.ctx(&u);
    let ast = &u.ast;
    let library = engine.unit_library(unit);

    // The switch keyword, the scrutinee and the case nodes.
    let (switch_keyword, scrutinee, case_nodes): (TokenId, NodeId, Vec<NodeId>) =
        if let Some(n) = ast.cast::<SwitchExpression>(input.node) {
            let node = &ast[n];
            (
                node.switch_keyword,
                node.expression.raw(),
                ast.list_raw(node.cases).to_vec(),
            )
        } else if let Some(n) = ast.cast::<SwitchStatement>(input.node) {
            let node = &ast[n];
            (
                node.switch_keyword,
                node.expression.raw(),
                ast.list_raw(node.members).to_vec(),
            )
        } else {
            return;
        };

    let ts = TypeSystem::new(ctx);
    let shared = AnalyzerExhaustivenessCache::new(
        AnalyzerTypeOperations::new(ts, library),
        AnalyzerEnumOperations { engine, ts },
        AnalyzerSealedClassOperations { ts },
    );

    // Dart `scrutinee.typeOrThrow`.
    let scrutinee_type = u
        .tables
        .static_type
        .get(scrutinee)
        .copied()
        .unwrap_or(TypeId::INVALID);
    let scrutinee_type_ex = shared.get_static_type(&scrutinee_type);

    let mut case_nodes_with_space: Vec<NodeId> = vec![];
    let mut case_is_guarded: Vec<bool> = vec![];
    let mut case_spaces: Vec<Space> = vec![];
    let mut default_node: Option<NodeId> = None;

    let mut pattern_converter = PatternConverter {
        language_version: ctx.get(library).language_version.effective(),
        feature_set: engine.library_features(library),
        cache: &shared,
        unit: &u,
        ts,
        map_pattern_key_values: input.map_pattern_key_values,
        constant_pattern_values: input.constant_pattern_values,
        has_invalid_type: matches!(*ctx.ty(scrutinee_type), TypeKind::Invalid),
    };

    // Build spaces for cases.
    for case_node in case_nodes {
        let guarded_pattern = if let Some(c) = ast.cast::<SwitchExpressionCase>(case_node) {
            Some(ast[c].guarded_pattern)
        } else if let Some(c) = ast.cast::<SwitchPatternCase>(case_node) {
            Some(ast[c].guarded_pattern)
        } else {
            if ast.cast::<SwitchDefault>(case_node).is_some() {
                default_node = Some(case_node);
            }
            // A `SwitchCase` should not happen; ignore it.
            None
        };
        if let Some(guarded_pattern) = guarded_pattern {
            let guarded = &ast[guarded_pattern];
            let space =
                pattern_converter.create_root_space(scrutinee_type_ex, &guarded.pattern.raw());
            case_nodes_with_space.push(case_node);
            case_is_guarded.push(guarded.when_clause.is_some());
            case_spaces.push(space);
        }
    }
    let report_non_exhaustive = input.must_be_exhaustive && default_node.is_none();

    // Compute and report errors.
    if pattern_converter.has_invalid_type {
        return;
    }
    let mut case_unreachabilities = vec![];
    let non_exhaustiveness = ex::compute_exhaustiveness(
        &shared,
        scrutinee_type_ex,
        &case_is_guarded,
        &case_spaces,
        Some(&mut case_unreachabilities),
    );
    for case_unreachability in &case_unreachabilities {
        let case_node = case_nodes_with_space[case_unreachability.index];
        let error_token = if let Some(c) = ast.cast::<SwitchExpressionCase>(case_node) {
            ast[c].arrow
        } else {
            let c = ast
                .cast::<SwitchPatternCase>(case_node)
                .expect("SwitchPatternCase");
            ast[c].keyword
        };
        diagnostics.push(at_token(ast, diag::unreachable_switch_case(), error_token));
    }
    match non_exhaustiveness {
        Some(non_exhaustiveness) => {
            if report_non_exhaustive {
                let types: &dyn StaticTypeArena = &shared;
                let first = &non_exhaustiveness.witnesses[0];
                let mut error_buffer = SimpleDartBuffer::new();
                first.to_dart(types, &mut error_buffer, false);
                let mut correction_text_buffer = SimpleDartBuffer::new();
                first.to_dart(types, &mut correction_text_buffer, true);

                let value_type = non_exhaustiveness.value_type;
                let current_library_uri = ctx.library_uri(library);
                let type_arg = dartr_element::diagnostics::type_arg(&ctx, scrutinee_type);
                let diagnostic = if types.is_enum_subtype(value_type)
                    && types.library_uri(value_type).as_deref() != Some(current_library_uri)
                    && (types.is_private(value_type)
                        || non_exhaustiveness
                            .witnesses
                            .iter()
                            .all(|witness| witness.as_witness(types).contains("._")))
                {
                    if input.is_switch_expression {
                        diag::non_exhaustive_switch_expression_private(type_arg)
                    } else {
                        diag::non_exhaustive_switch_statement_private(type_arg)
                    }
                } else {
                    let unmatched_pattern = error_buffer.to_string();
                    let suggested_pattern = correction_text_buffer.to_string();
                    if input.is_switch_expression {
                        diag::non_exhaustive_switch_expression(
                            type_arg,
                            &unmatched_pattern,
                            &suggested_pattern,
                        )
                    } else {
                        diag::non_exhaustive_switch_statement(
                            type_arg,
                            &unmatched_pattern,
                            &suggested_pattern,
                        )
                    }
                };
                diagnostics.push(at_token(ast, diagnostic, switch_keyword));
            }
        }
        None => {
            if let Some(default_node) = default_node
                && input.must_be_exhaustive
            {
                // The default node is unreachable.
                let keyword = ast[ast.cast::<SwitchDefault>(default_node).unwrap()].keyword;
                diagnostics.push(at_token(ast, diag::unreachable_switch_default(), keyword));
            }
        }
    }
}

/// Dart `diagnosticReporter.report(diagnostic.at(token))`.
fn at_token(ast: &Ast, diagnostic: LocatableDiagnostic, token: TokenId) -> Diagnostic {
    let offset = ast.tokens.offset(token);
    let end = ast_ext::token_end(ast, token);
    diagnostic.to_diagnostic(offset as usize, (end - offset) as usize)
}

// ---------------------------------------------------------------------------
// Keys for constant values
// ---------------------------------------------------------------------------

/// The Dart `DartObject` value of an enum element, used as
/// `EnumOperations.EnumElementValue`: the enum class and the display string
/// of the state (the fields `index` and `_name`, and the declared fields).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct EnumValueKey {
    enum_class: EId<EnumElement>,
    state: String,
}

impl EnumValueKey {
    fn new(ts: &TypeSystem<'_>, enum_class: EId<EnumElement>, value: &DartObjectImpl) -> Self {
        EnumValueKey {
            enum_class,
            state: value.state.display(ts),
        }
    }
}

/// A Dart `DartObjectImpl` used as unique identity of a constant (Dart
/// `getUniqueStaticType<DartObjectImpl>` and `MapKey`): the type (Dart
/// `runtimeTypesEqual`, approximated by `identical`) and the display string
/// of the state.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ConstIdentity {
    ty: TypeId,
    state: String,
}

// ---------------------------------------------------------------------------
// AnalyzerEnumOperations
// ---------------------------------------------------------------------------

/// Dart `AnalyzerEnumOperations`.
pub struct AnalyzerEnumOperations<'e, 'a, 'u> {
    engine: &'e ConstantEvaluationEngine<'a>,
    ts: TypeSystem<'u>,
}

impl EnumOperations for AnalyzerEnumOperations<'_, '_, '_> {
    type Type = TypeId;
    type EnumClass = EId<EnumElement>;
    type EnumElement = EId<FieldElement>;
    type EnumElementValue = EnumValueKey;

    fn get_enum_class(&self, type_: &TypeId) -> Option<EId<EnumElement>> {
        let element = self.ts.ctx.interface_element(*type_)?;
        element.raw().cast::<EnumElement>()
    }

    fn get_enum_elements(&self, enum_class: &EId<EnumElement>) -> Vec<EId<FieldElement>> {
        let ctx = &self.ts.ctx;
        ctx.instance(enum_class.upcast())
            .fields
            .iter()
            .copied()
            .filter(|field| element_ext::is_enum_constant(ctx, field.raw()))
            .collect()
    }

    fn get_enum_element_value(&self, enum_element: &EId<FieldElement>) -> Option<EnumValueKey> {
        let value = self.engine.compute_constant_value_of(enum_element.raw())?;
        let enum_class = self.ts.ctx.element_data(enum_element.raw())?.enclosing?;
        Some(EnumValueKey::new(
            &self.ts,
            enum_class.cast::<EnumElement>()?,
            &value,
        ))
    }

    fn get_enum_element_name(&self, enum_element: &EId<FieldElement>) -> String {
        let ctx = &self.ts.ctx;
        let enclosing = ctx
            .element_data(enum_element.raw())
            .and_then(|d| d.enclosing)
            .and_then(|e| ctx.element_name(e))
            .unwrap_or("null");
        let name = ctx.element_name(enum_element.raw()).unwrap_or("null");
        format!("{enclosing}.{name}")
    }

    fn get_enum_element_type(&self, enum_element: &EId<FieldElement>) -> TypeId {
        element_ext::variable_type(&self.ts.ctx, enum_element.raw())
    }
}

// ---------------------------------------------------------------------------
// AnalyzerSealedClassOperations
// ---------------------------------------------------------------------------

/// Dart `AnalyzerSealedClassOperations`.
pub struct AnalyzerSealedClassOperations<'u> {
    ts: TypeSystem<'u>,
}

impl SealedClassOperations for AnalyzerSealedClassOperations<'_> {
    type Type = TypeId;
    type Class = EId<InterfaceElement>;

    fn get_sealed_class(&self, type_: &TypeId) -> Option<EId<InterfaceElement>> {
        let ctx = &self.ts.ctx;
        let element = ctx.interface_element(*type_)?;
        if element.raw().is::<ClassElement>()
            && element_ext::first_fragment_flags(ctx, element.raw())
                .contains(FragmentFlags::CLASS_FRAGMENT_IS_SEALED)
        {
            return Some(element);
        }
        None
    }

    fn get_direct_subclasses(
        &self,
        sealed_class: &EId<InterfaceElement>,
    ) -> Vec<EId<InterfaceElement>> {
        direct_subtypes_of_sealed(&self.ts.ctx, *sealed_class)
    }

    fn get_subclass_as_instance_of(
        &self,
        sub_class: &EId<InterfaceElement>,
        sealed_class_type: &TypeId,
    ) -> Option<TypeId> {
        let ctx = &self.ts.ctx;
        let sub_class = *sub_class;
        let sealed_class_type = *sealed_class_type;
        let this_type = ctx.interface_this_type(sub_class);
        let sealed_element = ctx.interface_element(sealed_class_type)?;
        let as_sealed_class = ctx.as_instance_of(this_type, sealed_element)?;
        let this_args = ctx.type_arguments(this_type);
        if this_args.is_empty() {
            return Some(this_type);
        }
        let as_sealed_args = ctx.type_arguments(as_sealed_class);
        let sealed_args = ctx.type_arguments(sealed_class_type);
        let type_parameters = ctx.interface_type_parameters(sub_class);
        let mut trivial_substitution = true;
        if this_args.len() == as_sealed_args.len() {
            for i in 0..this_args.len() {
                // Dart: !=
                if !ctx.dart_eq(this_args[i], as_sealed_args[i]) {
                    trivial_substitution = false;
                    break;
                }
            }
            if trivial_substitution {
                let substitution = MapSubstitution::from_pairs(type_parameters, sealed_args);
                for (i, &parameter) in type_parameters.iter().enumerate() {
                    let bound = ctx.type_parameter_bound(parameter);
                    if let Some(bound) = bound
                        && !self
                            .ts
                            .is_subtype_of(sealed_args[i], substitution.substitute_type(ctx, bound))
                    {
                        trivial_substitution = false;
                        break;
                    }
                }
            }
        } else {
            trivial_substitution = false;
        }
        if trivial_substitution {
            Some(ctx.instantiate_interface(sub_class, sealed_args, Nullability::None))
        } else {
            Some(TypeParameterReplacer::replace_type_variables(
                self.ts, this_type,
            ))
        }
    }
}

/// Dart `ClassElementImpl.directSubtypesOfSealed`: the interface elements of
/// the library of [sealed_class] (`library.children` order, without
/// extension types) that extend, mix in, implement or are constrained to
/// it.
fn direct_subtypes_of_sealed(
    ctx: &Ctx<'_>,
    sealed_class: EId<InterfaceElement>,
) -> Vec<EId<InterfaceElement>> {
    let mut subclasses = vec![];
    let Some(library) = ctx.element_data(sealed_class.raw()).and_then(|d| d.library) else {
        return subclasses;
    };
    let library: &LibraryElement = ctx.get(library);
    let declarations = library
        .classes
        .iter()
        .map(|c| c.upcast::<InterfaceElement>())
        .chain(library.enums.iter().map(|e| e.upcast::<InterfaceElement>()))
        .chain(
            library
                .mixins
                .iter()
                .map(|m| m.upcast::<InterfaceElement>()),
        );
    let is_sealed_class = |t: TypeId| ctx.interface_element(t).is_some_and(|e| e == sealed_class);
    for declaration in declarations {
        if declaration == sealed_class {
            continue;
        }
        let matches = ctx
            .element_supertype(declaration)
            .is_some_and(is_sealed_class)
            || ctx
                .element_mixins(declaration)
                .iter()
                .any(|&t| is_sealed_class(t))
            || ctx
                .element_interfaces(declaration)
                .iter()
                .any(|&t| is_sealed_class(t))
            || (declaration.raw().is::<dartr_element::MixinElement>()
                && ctx
                    .element_superclass_constraints(declaration)
                    .iter()
                    .any(|&t| is_sealed_class(t)));
        if matches {
            subclasses.push(declaration);
        }
    }
    subclasses
}

// ---------------------------------------------------------------------------
// AnalyzerTypeOperations
// ---------------------------------------------------------------------------

/// Dart `AnalyzerTypeOperations`.
pub struct AnalyzerTypeOperations<'u> {
    ts: TypeSystem<'u>,
    enclosing_library: EId<LibraryElement>,
    /// Dart `_interfaceFieldTypesCaches`.
    interface_field_types_caches: RefCell<IndexMap<TypeId, IndexMap<Key, TypeId>>>,
}

impl<'u> AnalyzerTypeOperations<'u> {
    pub fn new(ts: TypeSystem<'u>, enclosing_library: EId<LibraryElement>) -> Self {
        AnalyzerTypeOperations {
            ts,
            enclosing_library,
            interface_field_types_caches: RefCell::new(IndexMap::new()),
        }
    }

    fn ctx(&self) -> &Ctx<'u> {
        &self.ts.ctx
    }

    /// Dart `_getInterfaceFieldTypes`.
    fn get_interface_field_types(&self, type_: TypeId) -> IndexMap<Key, TypeId> {
        if let Some(field_types) = self.interface_field_types_caches.borrow().get(&type_) {
            return field_types.clone();
        }
        self.interface_field_types_caches
            .borrow_mut()
            .insert(type_, IndexMap::new());
        let ctx = *self.ctx();
        let mut field_types: IndexMap<Key, TypeId> = IndexMap::new();
        for supertype in ctx.all_supertypes(type_) {
            for (key, value) in self.get_interface_field_types(supertype) {
                field_types.insert(key, value);
            }
        }
        let element = ctx.interface_element(type_).expect("interface type");
        let substitution = MapSubstitution::from_interface_type(&ctx, type_);
        let instance = ctx.instance(element.upcast());
        for &getter in &instance.getters {
            let getter = ElemRef::Base(getter.raw());
            if !self.is_accessible(getter) {
                continue;
            }
            let Some(name) = member::name(&ctx, getter) else {
                continue;
            };
            if !member::is_static(&ctx, getter) {
                let return_type =
                    substitution.substitute_type(&ctx, member::return_type(&ctx, getter));
                field_types.insert(Key::Name(name.to_string()), return_type);
            }
        }
        for &method in &instance.methods {
            let method = ElemRef::Base(method.raw());
            if !self.is_accessible(method) {
                continue;
            }
            let Some(name) = member::name(&ctx, method) else {
                continue;
            };
            if !member::is_static(&ctx, method) {
                let method_type = substitution.substitute_type(&ctx, member::type_(&ctx, method));
                field_types.insert(Key::Name(name.to_string()), method_type);
            }
        }
        self.interface_field_types_caches
            .borrow_mut()
            .insert(type_, field_types.clone());
        field_types
    }

    /// Dart `!(e.isPrivate && e.library != _enclosingLibrary)`.
    fn is_accessible(&self, e: ElemRef) -> bool {
        let ctx = self.ctx();
        let is_private = member::name(ctx, e).is_some_and(|n| n.starts_with('_'));
        !(is_private && member::library(ctx, e) != Some(self.enclosing_library))
    }
}

impl TypeOperations for AnalyzerTypeOperations<'_> {
    type Type = TypeId;

    fn bool_type(&self) -> TypeId {
        self.ctx().tp.bool_type()
    }

    fn non_nullable_object_type(&self) -> TypeId {
        self.ts.object_none()
    }

    fn nullable_object_type(&self) -> TypeId {
        self.ts.object_question()
    }

    fn get_extension_type_erasure(&self, type_: &TypeId) -> TypeId {
        self.ts.extension_type_erasure(*type_)
    }

    fn get_field_types(&self, type_: &TypeId) -> IndexMap<Key, TypeId> {
        let ctx = *self.ctx();
        match *ctx.ty(*type_) {
            TypeKind::Interface { .. } => self.get_interface_field_types(*type_),
            TypeKind::Record {
                positional, named, ..
            } => {
                let mut field_types = self.get_field_types(&ctx.tp.object_type());
                for (index, &field) in ctx.list(positional).iter().enumerate() {
                    field_types.insert(Key::RecordIndex(index), field);
                }
                for field in ctx.list(named) {
                    field_types.insert(
                        Key::RecordName(ctx.name_str(field.name).to_string()),
                        field.ty,
                    );
                }
                field_types
            }
            _ => self.get_field_types(&ctx.tp.object_type()),
        }
    }

    fn get_future_or_type_argument(&self, type_: &TypeId) -> Option<TypeId> {
        if self.ctx().is_dart_async_future_or(*type_) {
            Some(self.ts.future_or_base(*type_))
        } else {
            None
        }
    }

    fn get_list_element_type(&self, type_: &TypeId) -> Option<TypeId> {
        let ctx = self.ctx();
        let list_type = ctx.as_instance_of(*type_, ctx.tp.list_element().upcast())?;
        Some(ctx.type_arguments(list_type)[0])
    }

    fn get_list_type(&self, type_: &TypeId) -> Option<TypeId> {
        let ctx = self.ctx();
        ctx.as_instance_of(*type_, ctx.tp.list_element().upcast())
    }

    fn get_map_value_type(&self, type_: &TypeId) -> Option<TypeId> {
        let ctx = self.ctx();
        let map_type = ctx.as_instance_of(*type_, ctx.tp.map_element().upcast())?;
        Some(ctx.type_arguments(map_type)[1])
    }

    fn get_non_nullable(&self, type_: &TypeId) -> TypeId {
        self.ts.promote_to_non_null(*type_)
    }

    fn get_type_variable_bound(&self, type_: &TypeId) -> Option<TypeId> {
        match *self.ctx().ty(*type_) {
            TypeKind::TypeParameter { .. } => Some(self.ctx().type_parameter_type_bound(*type_)),
            _ => None,
        }
    }

    fn has_simple_name(&self, type_: &TypeId) -> bool {
        matches!(
            *self.ctx().ty(*type_),
            TypeKind::Interface { .. }
                | TypeKind::Dynamic
                | TypeKind::Void
                | TypeKind::Never(_)
                // TODO(johnniwinther): What about intersection types?
                | TypeKind::TypeParameter { .. }
        )
    }

    fn instantiate_future(&self, type_: &TypeId) -> TypeId {
        let ctx = self.ctx();
        ctx.tp.future_type(ctx, *type_)
    }

    fn is_bool_type(&self, type_: &TypeId) -> bool {
        self.ctx().is_dart_core_bool(*type_) && !self.is_nullable(type_)
    }

    fn is_dynamic(&self, type_: &TypeId) -> bool {
        matches!(*self.ctx().ty(*type_), TypeKind::Dynamic)
    }

    fn is_enum(&self, type_: &TypeId) -> bool {
        self.ctx()
            .interface_element(*type_)
            .is_some_and(|e| e.raw().is::<EnumElement>())
    }

    fn is_generic(&self, type_: &TypeId) -> bool {
        matches!(*self.ctx().ty(*type_), TypeKind::Interface { args, .. }
            if !self.ctx().list(args).is_empty())
    }

    fn is_never_type(&self, type_: &TypeId) -> bool {
        matches!(*self.ctx().ty(*type_), TypeKind::Never(_))
    }

    fn is_non_nullable_object(&self, type_: &TypeId) -> bool {
        self.ctx().is_dart_core_object(*type_) && !self.is_nullable(type_)
    }

    fn is_nullable(&self, type_: &TypeId) -> bool {
        self.ctx().nullability_suffix(*type_) == Nullability::Question
    }

    fn is_nullable_object(&self, type_: &TypeId) -> bool {
        self.ctx().is_dart_core_object(*type_) && self.is_nullable(type_)
    }

    fn is_null_type(&self, type_: &TypeId) -> bool {
        self.ctx().is_dart_core_null(*type_)
    }

    fn is_potentially_nullable(&self, type_: &TypeId) -> bool {
        self.ts.is_potentially_nullable(*type_)
    }

    fn is_record_type(&self, type_: &TypeId) -> bool {
        matches!(*self.ctx().ty(*type_), TypeKind::Record { .. }) && !self.is_nullable(type_)
    }

    fn is_subtype_of(&self, s: &TypeId, t: &TypeId) -> bool {
        self.ts.is_subtype_of(*s, *t)
    }

    fn library_uri(&self, type_: &TypeId) -> Option<String> {
        let ctx = self.ctx();
        let element = match *ctx.ty(*type_) {
            TypeKind::Interface { element, .. } => element.raw(),
            TypeKind::TypeParameter { param, .. } => param.raw(),
            _ => return None,
        };
        ctx.element_library_uri(element).map(str::to_string)
    }

    fn overapproximate(&self, type_: &TypeId) -> TypeId {
        TypeParameterReplacer::replace_type_variables(self.ts, *type_)
    }

    fn type_to_string(&self, type_: &TypeId) -> String {
        dartr_element::type_display_string_with(self.ctx(), *type_, DisplayOptions::default())
    }
}

/// Dart `AnalyzerExhaustivenessCache` (the shared cache with the analyzer
/// operations).
type AnalyzerExhaustivenessCache<'e, 'a, 'u> = ex::ExhaustivenessCache<
    AnalyzerTypeOperations<'u>,
    AnalyzerEnumOperations<'e, 'a, 'u>,
    AnalyzerSealedClassOperations<'u>,
>;

// ---------------------------------------------------------------------------
// PatternConverter
// ---------------------------------------------------------------------------

/// Dart `PatternConverter`: creates the [Space]s of the patterns of a unit.
pub struct PatternConverter<'c, 'e, 'a, 'u> {
    language_version: Version,
    feature_set: &'a FeatureSet,
    cache: &'c AnalyzerExhaustivenessCache<'e, 'a, 'u>,
    unit: &'c ResolvedUnit,
    ts: TypeSystem<'u>,
    map_pattern_key_values: &'c IndexMap<NodeId, DartObjectImpl>,
    constant_pattern_values: &'c IndexMap<NodeId, DartObjectImpl>,

    /// If we saw an invalid type, we already have a diagnostic reported,
    /// and there is no need to verify exhaustiveness.
    has_invalid_type: bool,
}

impl PatternConverter<'_, '_, '_, '_> {
    fn ast(&self) -> &Ast {
        &self.unit.ast
    }

    /// Dart `node.typeOrThrow` of a type annotation.
    fn annotation_type(&self, node: NodeId) -> TypeId {
        self.unit
            .tables
            .annotation_type
            .get(node)
            .copied()
            .unwrap_or(TypeId::INVALID)
    }

    /// Dart `pattern.requiredType`.
    fn required_type(&self, node: NodeId) -> Option<TypeId> {
        self.unit
            .tables
            .pattern_info
            .get(node)
            .and_then(|info| info.required_type)
    }

    /// Dart `PatternField.effectiveName`.
    fn effective_name(&self, field: dartr_ast::Id<PatternField>) -> Option<String> {
        let ast = self.ast();
        let name_node = ast[field].name?;
        let name_token = ast[name_node]
            .name
            .or_else(|| variable_pattern_name(ast, ast[field].pattern))?;
        Some(ast.tokens.lexeme(name_token).to_string())
    }

    /// Dart `_convertConstantValue`.
    fn convert_constant_value(&mut self, value: &DartObjectImpl, path: &Path) -> Space {
        let ctx = self.ts.ctx;
        let type_ = value.ty;
        if value.is_null() {
            return Space::new(path.clone(), StaticType::NULL_TYPE);
        }
        match &value.state {
            InstanceState::Bool(state) => {
                if let Some(value) = state.value {
                    return Space::new(path.clone(), self.cache.get_bool_value_static_type(value));
                }
            }
            InstanceState::Record(state) => {
                let mut properties: IndexMap<Key, Space> = IndexMap::new();
                for (index, value) in state.positional_fields.iter().enumerate() {
                    let key = Key::RecordIndex(index);
                    let space = self.convert_constant_value(value, &path.add(key.clone()));
                    properties.insert(key, space);
                }
                for (name, value) in &state.named_fields {
                    let key = Key::RecordName(name.to_string());
                    let space = self.convert_constant_value(value, &path.add(key.clone()));
                    properties.insert(key, space);
                }
                return Space::with_properties(
                    path.clone(),
                    self.cache.get_static_type(&type_),
                    properties,
                    IndexMap::new(),
                );
            }
            _ => {}
        }
        if let Some(element) = ctx.interface_element(type_)
            && let Some(enum_class) = element.raw().cast::<EnumElement>()
        {
            let key = EnumValueKey::new(&self.ts, enum_class, value);
            // Dart throws for a value that is not a value of the enum
            // elements; the space is unknown here instead.
            let enum_operations = &self.cache.enum_operations;
            let is_enum_value = enum_operations
                .get_enum_elements(&enum_class)
                .iter()
                .any(|e| enum_operations.get_enum_element_value(e).as_ref() == Some(&key));
            if !is_enum_value {
                return self.create_unknown_space(path);
            }
            return Space::new(
                path.clone(),
                self.cache.get_enum_element_static_type(&enum_class, &key),
            );
        }

        let static_type = if value.has_primitive_equality(&self.ts, self.feature_set) {
            let text = value.state.display(&self.ts);
            self.cache.get_unique_static_type(
                &type_,
                Identity::new(ConstIdentity {
                    ty: type_,
                    state: text.clone(),
                }),
                &text,
            )
        } else {
            // If [value] doesn't have primitive equality we cannot tell if it
            // is equal to itself.
            self.cache.get_unknown_static_type()
        };
        Space::new(path.clone(), static_type)
    }

    /// The element of a `PatternField` when it is a member of an extension
    /// or an extension type, with its Dart `returnType` (getters and
    /// setters) or `type` (other executables).
    fn extension_property_type(&self, field: NodeId) -> Option<TypeId> {
        let ctx = self.ts.ctx;
        let element = *self.unit.tables.element.get(field)?;
        let base = member::base_element(&ctx, element);
        let enclosing = member::enclosing_element(&ctx, element)?;
        if !(enclosing.is::<ExtensionElement>() || enclosing.is::<ExtensionTypeElement>()) {
            return None;
        }
        if base.is::<GetterElement>() || base.is::<SetterElement>() {
            Some(member::return_type(&ctx, element))
        } else if base.cast::<dartr_element::ExecutableElement>().is_some() {
            Some(member::type_(&ctx, element))
        } else {
            None
        }
    }
}

impl SpaceCreator for PatternConverter<'_, '_, '_, '_> {
    type Pattern = NodeId;
    type Type = TypeId;

    fn type_operations(&self) -> &dyn TypeOperations<Type = TypeId> {
        &self.cache.type_operations
    }

    fn object_field_lookup(&self) -> &dyn ObjectPropertyLookup {
        self.cache
    }

    fn has_language_version(&self, major: u32, minor: u32) -> bool {
        self.language_version >= Version { major, minor }
    }

    fn create_unknown_static_type(&mut self) -> StaticType {
        self.cache.get_unknown_static_type()
    }

    fn create_static_type(&mut self, type_: &TypeId) -> StaticType {
        self.has_invalid_type |= matches!(*self.ts.ctx.ty(*type_), TypeKind::Invalid);
        self.cache.get_static_type(type_)
    }

    fn create_list_type(
        &mut self,
        type_: &TypeId,
        restriction: ListTypeRestriction<TypeId>,
    ) -> StaticType {
        self.cache.get_list_static_type(type_, restriction)
    }

    fn create_map_type(
        &mut self,
        type_: &TypeId,
        restriction: MapTypeRestriction<TypeId>,
    ) -> StaticType {
        self.cache.get_map_static_type(type_, restriction)
    }

    fn dispatch_pattern(
        &mut self,
        path: &Path,
        context_type: StaticType,
        pattern: &NodeId,
        non_null: bool,
    ) -> Space {
        let pattern = *pattern;
        let ctx = self.ts.ctx;
        let unit = self.unit;
        let ast = &unit.ast;
        if ast.cast::<DeclaredVariablePattern>(pattern).is_some() {
            let element = unit
                .tables
                .declared_fragment
                .get(pattern)
                .and_then(|&f| ctx.fragment_data(f))
                .and_then(|f| f.element.try_get().copied());
            let type_ = match element {
                Some(e) => element_ext::variable_type(&ctx, e),
                None => TypeId::INVALID,
            };
            self.create_variable_space(path, context_type, &type_, non_null)
        } else if let Some(p) = ast.cast::<ObjectPattern>(pattern) {
            let mut properties: IndexMap<String, NodeId> = IndexMap::new();
            let mut extension_property_types: IndexMap<String, TypeId> = IndexMap::new();
            for &field in ast.list(ast[p].fields) {
                let Some(name) = self.effective_name(field) else {
                    // Error case, skip field.
                    continue;
                };
                properties.insert(name.clone(), ast[field].pattern.raw());
                if let Some(extension_property_type) = self.extension_property_type(field.raw()) {
                    extension_property_types.insert(name, extension_property_type);
                }
            }
            let type_ = self.annotation_type(ast[p].type_.raw());
            self.create_object_space(
                path,
                context_type,
                &type_,
                &properties,
                &extension_property_types,
                non_null,
            )
        } else if let Some(p) = ast.cast::<WildcardPattern>(pattern) {
            let type_ = ast[p].type_.map(|t| self.annotation_type(t.raw()));
            self.create_wildcard_space(path, context_type, type_.as_ref(), non_null)
        } else if let Some(p) = ast.cast::<RecordPattern>(pattern) {
            let dynamic_type = ctx.tp.dynamic_type();
            let mut positional_types: Vec<TypeId> = vec![];
            let mut positional_patterns: Vec<NodeId> = vec![];
            let mut named_types: Vec<NamedField> = vec![];
            let mut named_patterns: IndexMap<String, NodeId> = IndexMap::new();
            for &field in ast.list(ast[p].fields) {
                if ast[field].name.is_none() {
                    positional_types.push(dynamic_type);
                    positional_patterns.push(ast[field].pattern.raw());
                } else if let Some(name) = self.effective_name(field) {
                    named_types.push(NamedField {
                        name: ctx.name(&name),
                        ty: dynamic_type,
                    });
                    named_patterns.insert(name, ast[field].pattern.raw());
                } else {
                    // Error case, skip field.
                    continue;
                }
            }
            let record_type =
                ctx.record_type(&positional_types, &named_types, Nullability::None, None);
            self.create_record_space(
                path,
                context_type,
                &record_type,
                &positional_patterns,
                &named_patterns,
            )
        } else if let Some(p) = ast.cast::<LogicalOrPattern>(pattern) {
            let (left, right) = (ast[p].left_operand.raw(), ast[p].right_operand.raw());
            self.create_logical_or_space(path, context_type, &left, &right, non_null)
        } else if let Some(p) = ast.cast::<NullCheckPattern>(pattern) {
            self.create_null_check_space(path, context_type, &ast[p].pattern.raw())
        } else if let Some(p) = ast.cast::<ParenthesizedPattern>(pattern) {
            self.dispatch_pattern(path, context_type, &ast[p].pattern.raw(), non_null)
        } else if let Some(p) = ast.cast::<NullAssertPattern>(pattern) {
            self.create_null_assert_space(path, context_type, &ast[p].pattern.raw())
        } else if let Some(p) = ast.cast::<CastPattern>(pattern) {
            let type_ = self.annotation_type(ast[p].type_.raw());
            self.create_cast_space(path, context_type, &type_, &ast[p].pattern.raw(), non_null)
        } else if let Some(p) = ast.cast::<LogicalAndPattern>(pattern) {
            let (left, right) = (ast[p].left_operand.raw(), ast[p].right_operand.raw());
            self.create_logical_and_space(path, context_type, &left, &right, non_null)
        } else if ast.cast::<RelationalPattern>(pattern).is_some() {
            self.create_relational_space(path)
        } else if let Some(p) = ast.cast::<ListPattern>(pattern) {
            let Some(type_) = self.required_type(pattern) else {
                return self.create_unknown_space(path);
            };
            debug_assert!(ctx.interface_element(type_) == Some(ctx.tp.list_element().upcast()));
            let element_type = ctx.type_arguments(type_)[0];
            let mut head_elements: Vec<NodeId> = vec![];
            let mut rest_element: Option<NodeId> = None;
            let mut tail_elements: Vec<NodeId> = vec![];
            let mut has_rest = false;
            for &element in ast.list_raw(ast[p].elements) {
                if let Some(rest) = ast.cast::<RestPatternElement>(element) {
                    rest_element = ast[rest].pattern.map(|p| p.raw());
                    has_rest = true;
                } else if has_rest {
                    tail_elements.push(element);
                } else {
                    head_elements.push(element);
                }
            }
            self.create_list_space(
                path,
                &type_,
                &element_type,
                &head_elements,
                rest_element.as_ref(),
                &tail_elements,
                has_rest,
                ast[p].type_arguments.is_some(),
            )
        } else if let Some(p) = ast.cast::<MapPattern>(pattern) {
            let Some(type_) = self.required_type(pattern) else {
                return self.create_unknown_space(path);
            };
            debug_assert!(ctx.interface_element(type_) == Some(ctx.tp.map_element().upcast()));
            let key_type = ctx.type_arguments(type_)[0];
            let value_type = ctx.type_arguments(type_)[1];
            let mut entries: IndexMap<MapKey, NodeId> = IndexMap::new();
            for &entry in ast.list_raw(ast[p].elements) {
                // Rest patterns are illegal in map patterns, so just skip
                // over them.
                let Some(entry) = ast.cast::<MapPatternEntry>(entry) else {
                    continue;
                };
                let expression = ast[entry].key.raw();
                // TODO(johnniwinther): Assert that we have a constant value.
                let Some(constant) = self.map_pattern_key_values.get(&expression) else {
                    return self.create_unknown_space(path);
                };
                let text = constant.state.display(&self.ts);
                let key = MapKey::new(
                    Identity::new(ConstIdentity {
                        ty: constant.ty,
                        state: text.clone(),
                    }),
                    text,
                );
                entries.insert(key, ast[entry].value.raw());
            }
            let map_type = ctx.tp.map_type(&ctx, key_type, value_type);
            self.create_map_space(
                path,
                &map_type,
                &key_type,
                &value_type,
                &entries,
                ast[p].type_arguments.is_some(),
            )
        } else if ast.cast::<ConstantPattern>(pattern).is_some() {
            if let Some(value) = self.constant_pattern_values.get(&pattern) {
                let value = value.clone();
                return self.convert_constant_value(&value, path);
            }
            self.has_invalid_type = true;
            self.create_unknown_space(path)
        } else {
            debug_assert!(false, "Unexpected pattern {:?}", ast.kind(pattern));
            self.create_unknown_space(path)
        }
    }
}

// ---------------------------------------------------------------------------
// TypeParameterReplacer
// ---------------------------------------------------------------------------

/// Dart `Variance` (of `_fe_analyzer_shared`), the values this file uses.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Variance {
    Covariant,
    Contravariant,
    Invariant,
}

/// Dart `TypeParameterReplacer`: replaces the type parameter types with
/// their default types (`Never` in contravariant positions).
pub struct TypeParameterReplacer<'u> {
    ts: TypeSystem<'u>,
    variance: Variance,
}

impl<'u> TypeParameterReplacer<'u> {
    fn new(ts: TypeSystem<'u>) -> Self {
        TypeParameterReplacer {
            ts,
            variance: Variance::Covariant,
        }
    }

    /// Dart `_replaceTypeParameterTypes`.
    fn replace_type_parameter_types(&mut self, type_: TypeId) -> TypeId {
        self.visit(type_).unwrap_or(type_)
    }

    /// Dart `TypeParameterReplacer.replaceTypeVariables`.
    pub fn replace_type_variables(ts: TypeSystem<'u>, type_: TypeId) -> TypeId {
        TypeParameterReplacer::new(ts).replace_type_parameter_types(type_)
    }
}

impl<'u> ReplacementVisitor<'u> for TypeParameterReplacer<'u> {
    fn ctx(&self) -> Ctx<'u> {
        self.ts.ctx
    }

    fn change_variance(&mut self) {
        if self.variance == Variance::Covariant {
            self.variance = Variance::Contravariant;
        } else if self.variance == Variance::Contravariant {
            self.variance = Variance::Covariant;
        }
    }

    fn visit_type_parameter_bound(&mut self, t: TypeId) -> Option<TypeId> {
        let saved_variance = self.variance;
        self.variance = Variance::Invariant;
        let result = self.visit(t);
        self.variance = saved_variance;
        result
    }

    fn visit_type_parameter_type(&mut self, t: TypeId) -> Option<TypeId> {
        if self.variance == Variance::Contravariant {
            let never_type = self.ts.ctx.tp.never_type();
            Some(self.replace_type_parameter_types(never_type))
        } else {
            let ctx = self.ts.ctx;
            let TypeKind::TypeParameter { param, .. } = *ctx.ty(t) else {
                unreachable!("type parameter type");
            };
            let default_type = ctx.get(param).default_type.get().unwrap_or(TypeId::DYNAMIC);
            Some(self.replace_type_parameter_types(default_type))
        }
    }
}
