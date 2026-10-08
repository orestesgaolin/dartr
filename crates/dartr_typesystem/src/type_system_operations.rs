// Dart source: pkg/analyzer/lib/src/dart/resolver/flow_analysis_visitor.dart
// (TypeSystemOperations), and the `Shared*` members of the analyzer types and
// elements: pkg/analyzer/lib/src/dart/element/type.dart (TypeImpl
// .isQuestionType / .asQuestionType / .isStructurallyEqualTo,
// FunctionTypeImpl.*Shared, RecordTypeImpl.*Shared),
// pkg/analyzer/lib/src/dart/element/element.dart (TypeParameterElementImpl
// .boundShared / .variance / .isLegacyCovariant / .displayName).

//! [`TypeSystemOperations`]: the analyzer's implementation of the shared
//! operations traits of `dartr_flow` ([`SharedTypeOperations`],
//! [`FlowAnalysisTypeOperations`], [`FlowAnalysisOperations`],
//! [`TypeAnalyzerOperations`]) over [`TypeId`].
//!
//! The Dart class is in the resolver (`flow_analysis_visitor.dart`), but it
//! needs only the type system, and type inference (`GenericInferrer`,
//! `TypeConstraintGatherer`) needs it. So it lives here; the resolver uses
//! it as it is.
//!
//! Not ported yet (they need element getters that the resolver units port):
//! `isFinal`, `isVariableFinal`, `variableType`, `isPropertyPromotable`,
//! `whyPropertyIsNotPromotable` (they are `todo!()`). Dart throws
//! `UnimplementedError` for `doubleType`, `intType` and
//! `lookupMemberTypeInternal`; so does this port (`unimplemented!()`).

use std::cmp::Ordering;

use dartr_ast::NodeId;
use dartr_element::{
    EId, ElemRef, InterfaceElement, Name, NamedType, Nullability, PromotableElement, TypeId,
    TypeKind, TypeParameterElement,
};
use dartr_flow::flow_analysis_operations::{
    FlowAnalysisOperations, FlowAnalysisTypeOperations, PropertyNonPromotabilityReason,
    TypeClassification,
};
use dartr_flow::shared_type::{
    SharedNamedFunctionParameter, SharedNamedType, SharedTypeKind, SharedTypeOperations,
    SharedTypeParameterView, SharedTypeSchemaView, SharedTypeView, Variance,
};
use dartr_flow::type_analyzer_operations::{
    DataForTestingOf, TypeAnalyzerOperations, TypeDeclarationKind, TypeDeclarationMatchResult,
};
use dartr_flow::type_constraint::TypeConstraintGenerationDataForTesting;

use crate::type_algebra::MapSubstitution;
use crate::type_constraint_gatherer::TypeConstraintGatherer;
use crate::type_ext::{TypeExt, is_named, is_required_named};
use crate::type_schema;
use crate::type_system::TypeSystem;

/// `TypeConstraintGenerationDataForTesting<PromotableElementImpl,
/// AstNodeImpl>` of the analyzer.
pub type TypeConstraintGenerationDataForTestingImpl =
    TypeConstraintGenerationDataForTesting<EId<TypeParameterElement>, TypeId, NodeId>;

/// Converts the element model variance to the shared variance.
pub fn shared_variance(variance: dartr_element::Variance) -> Variance {
    match variance {
        dartr_element::Variance::Unrelated => Variance::Unrelated,
        dartr_element::Variance::Covariant => Variance::Covariant,
        dartr_element::Variance::Contravariant => Variance::Contravariant,
        dartr_element::Variance::Invariant => Variance::Invariant,
    }
}

/// `TypeSystemOperations`.
#[derive(Clone, Copy)]
pub struct TypeSystemOperations<'a> {
    pub strict_casts: bool,
    pub type_system: TypeSystem<'a>,
}

impl<'a> TypeSystemOperations<'a> {
    /// `TypeSystemOperations(typeSystem, strictCasts: ...)`.
    pub fn new(type_system: TypeSystem<'a>, strict_casts: bool) -> TypeSystemOperations<'a> {
        TypeSystemOperations {
            strict_casts,
            type_system,
        }
    }

    fn ctx(&self) -> dartr_element::Ctx<'a> {
        self.type_system.ctx
    }

    /// The function type data of [t]; panics if [t] is not a function type
    /// (Dart would fail the cast to `FunctionTypeImpl`).
    fn function_data(&self, t: TypeId) -> dartr_element::FunctionTypeData {
        match *self.ctx().ty(t) {
            TypeKind::Function(f) => f,
            _ => panic!("not a function type: {t:?}"),
        }
    }
}

// ================================================================ SharedType

impl<'a> SharedTypeOperations for TypeSystemOperations<'a> {
    type Type = TypeId;
    type TypeParameter = EId<TypeParameterElement>;
    type Name = Name;

    fn compare_names(&self, name1: Name, name2: Name) -> Ordering {
        // Dart `String.compareTo`: UTF-16 code units.
        let ctx = self.ctx();
        ctx.name_str(name1)
            .encode_utf16()
            .cmp(ctx.name_str(name2).encode_utf16())
    }

    fn shared_type_kind(&self, ty: TypeId) -> SharedTypeKind {
        let ctx = self.ctx();
        match *ctx.ty(ty) {
            TypeKind::Dynamic => SharedTypeKind::Dynamic,
            TypeKind::Void => SharedTypeKind::Void,
            TypeKind::Invalid => SharedTypeKind::Invalid,
            TypeKind::Unknown => SharedTypeKind::Unknown,
            TypeKind::Function(_) => SharedTypeKind::Function,
            TypeKind::Record { .. } => SharedTypeKind::Record,
            // `NullTypeImpl implements SharedNullType`.
            TypeKind::Interface { .. } if ctx.is_dart_core_null(ty) => SharedTypeKind::Null,
            _ => SharedTypeKind::Other,
        }
    }

    fn is_question_type(&self, ty: TypeId) -> bool {
        self.ctx().is_question_type(ty)
    }

    fn as_question_type(&self, ty: TypeId, is_question_type: bool) -> TypeId {
        self.ctx().with_nullability(
            ty,
            if is_question_type {
                Nullability::Question
            } else {
                Nullability::None
            },
        )
    }

    fn get_display_string(&self, ty: TypeId) -> String {
        dartr_element::type_display_string_with(
            &self.ctx(),
            ty,
            dartr_element::DisplayOptions::default(),
        )
    }

    fn is_structurally_equal_to(&self, ty: TypeId, other: TypeId) -> bool {
        // Dart: `this == other`.
        self.type_system.dart_eq(ty, other)
    }

    fn positional_parameter_types_shared(&self, function_type: TypeId) -> Vec<TypeId> {
        let ctx = self.ctx();
        let f = self.function_data(function_type);
        ctx.list(f.params)
            .iter()
            .filter(|p| !is_named(p.kind))
            .map(|p| p.ty)
            .collect()
    }

    fn required_positional_parameter_count(&self, function_type: TypeId) -> usize {
        self.function_data(function_type).required_positional as usize
    }

    fn return_type_shared(&self, function_type: TypeId) -> TypeId {
        self.function_data(function_type).ret
    }

    fn sorted_named_parameters_shared(
        &self,
        function_type: TypeId,
    ) -> Vec<SharedNamedFunctionParameter<Name, TypeId>> {
        let ctx = self.ctx();
        let f = self.function_data(function_type);
        // The named parameters are sorted by name (the `FunctionTypeImpl`
        // factory).
        ctx.list(f.params)
            .iter()
            .filter(|p| is_named(p.kind))
            .map(|p| SharedNamedFunctionParameter {
                is_required: is_required_named(p.kind),
                name_shared: p.name.unwrap_or_else(|| ctx.name("")),
                type_shared: p.ty,
            })
            .collect()
    }

    fn type_parameters_shared(&self, function_type: TypeId) -> Vec<EId<TypeParameterElement>> {
        let ctx = self.ctx();
        ctx.list(self.function_data(function_type).type_params)
            .to_vec()
    }

    fn positional_types_shared(&self, record_type: TypeId) -> Vec<TypeId> {
        let ctx = self.ctx();
        match *ctx.ty(record_type) {
            TypeKind::Record { positional, .. } => ctx.list(positional).to_vec(),
            _ => panic!("not a record type: {record_type:?}"),
        }
    }

    fn sorted_named_types_shared(&self, record_type: TypeId) -> Vec<SharedNamedType<Name, TypeId>> {
        let ctx = self.ctx();
        match *ctx.ty(record_type) {
            TypeKind::Record { named, .. } => ctx
                .list(named)
                .iter()
                .map(|f| SharedNamedType {
                    name_shared: f.name,
                    type_shared: f.ty,
                })
                .collect(),
            _ => panic!("not a record type: {record_type:?}"),
        }
    }

    fn bound_shared(&self, type_parameter: EId<TypeParameterElement>) -> Option<TypeId> {
        self.ctx().type_parameter_bound(type_parameter)
    }

    fn display_name(&self, type_parameter: EId<TypeParameterElement>) -> String {
        // `ElementImpl.displayName`: `name ?? '<unnamed>'`.
        self.ctx()
            .element_name(type_parameter.raw())
            .unwrap_or("<unnamed>")
            .to_string()
    }

    fn variance(&self, type_parameter: EId<TypeParameterElement>) -> Variance {
        shared_variance(self.ctx().type_parameter_variance(type_parameter))
    }

    fn is_legacy_covariant(&self, type_parameter: EId<TypeParameterElement>) -> bool {
        self.ctx()
            .type_parameter_is_legacy_covariant(type_parameter)
    }

    fn invocation_structural_context_schema_return_type(&self, _: TypeId) -> TypeId {
        unreachable!("the analyzer has no structural context schemas")
    }

    fn lookup_structural_context_schema_lookup_name(&self, _: TypeId) -> Name {
        unreachable!("the analyzer has no structural context schemas")
    }

    fn lookup_structural_context_schema_lookup_type(&self, _: TypeId) -> TypeId {
        unreachable!("the analyzer has no structural context schemas")
    }
}

// ====================================================== flow analysis types

impl<'a> FlowAnalysisTypeOperations for TypeSystemOperations<'a> {
    fn bool_type(&self) -> SharedTypeView<TypeId> {
        SharedTypeView::new(self.ctx().tp.bool_type())
    }

    fn classify_type(&self, ty: SharedTypeView<TypeId>) -> TypeClassification {
        let unwrapped = ty.unwrap_type_view();
        let tp = self.ctx().tp;
        if matches!(self.ctx().ty(unwrapped), TypeKind::Invalid) {
            TypeClassification::PotentiallyNullable
        } else if self.is_subtype_of_internal(unwrapped, tp.object_type()) {
            TypeClassification::NonNullable
        } else if self.is_subtype_of_internal(unwrapped, tp.null_type()) {
            TypeClassification::NullOrEquivalent
        } else {
            TypeClassification::PotentiallyNullable
        }
    }

    fn extension_type_erasure(&self, ty: SharedTypeView<TypeId>) -> SharedTypeView<TypeId> {
        SharedTypeView::new(
            self.type_system
                .extension_type_erasure(ty.unwrap_type_view()),
        )
    }

    fn factor(
        &self,
        from: SharedTypeView<TypeId>,
        what: SharedTypeView<TypeId>,
    ) -> SharedTypeView<TypeId> {
        SharedTypeView::new(
            self.type_system
                .factor(from.unwrap_type_view(), what.unwrap_type_view()),
        )
    }

    fn is_bottom_type(&self, ty: SharedTypeView<TypeId>) -> bool {
        self.ctx().is_bottom(ty.unwrap_type_view())
    }

    fn is_subtype_of(
        &self,
        left_type: SharedTypeView<TypeId>,
        right_type: SharedTypeView<TypeId>,
    ) -> bool {
        // `TypeAnalyzerOperationsMixin.isSubtypeOf`.
        self.is_subtype_of_internal(left_type.unwrap_type_view(), right_type.unwrap_type_view())
    }

    fn is_type_parameter_type(&self, ty: SharedTypeView<TypeId>) -> bool {
        matches!(
            self.ctx().ty(ty.unwrap_type_view()),
            TypeKind::TypeParameter { .. }
        )
    }

    fn is_invalid_type(&self, ty: SharedTypeView<TypeId>) -> bool {
        matches!(self.ctx().ty(ty.unwrap_type_view()), TypeKind::Invalid)
    }

    fn make_nullable(&self, ty: SharedTypeView<TypeId>) -> SharedTypeView<TypeId> {
        // `TypeAnalyzerOperationsMixin.makeNullable`.
        SharedTypeView::new(self.make_nullable_internal(ty.unwrap_type_view()))
    }

    fn promote_to_non_null(&self, ty: SharedTypeView<TypeId>) -> SharedTypeView<TypeId> {
        SharedTypeView::new(self.type_system.promote_to_non_null(ty.unwrap_type_view()))
    }

    fn try_promote_to_type(
        &self,
        to: SharedTypeView<TypeId>,
        from: SharedTypeView<TypeId>,
    ) -> Option<SharedTypeView<TypeId>> {
        self.type_system
            .try_promote_to_type(to.unwrap_type_view(), from.unwrap_type_view())
            .map(SharedTypeView::new)
    }
}

impl<'a> FlowAnalysisOperations for TypeSystemOperations<'a> {
    /// Dart `PromotableElementImpl` (formal parameters, local variables).
    type Variable = EId<PromotableElement>;
    /// Dart `Object property` (an executable element or a member).
    type PropertyMember = ElemRef;

    fn is_final(&self, variable: EId<PromotableElement>) -> bool {
        let _ = variable;
        todo!("TypeSystemOperations.isFinal (resolver unit C2: PromotableElementImpl.isFinal)")
    }

    fn is_property_promotable(&self, property: &ElemRef) -> bool {
        let _ = property;
        todo!(
            "TypeSystemOperations.isPropertyPromotable (resolver unit C2: FieldElement.isPromotable)"
        )
    }

    fn variable_type(&self, variable: EId<PromotableElement>) -> SharedTypeView<TypeId> {
        let _ = variable;
        todo!("TypeSystemOperations.variableType (resolver unit C2: PromotableElementImpl.type)")
    }

    fn why_property_is_not_promotable(
        &self,
        property: &ElemRef,
    ) -> Option<PropertyNonPromotabilityReason> {
        let _ = property;
        todo!("TypeSystemOperations.whyPropertyIsNotPromotable (resolver unit C2)")
    }
}

// =================================================== TypeAnalyzerOperations

impl<'a> TypeAnalyzerOperations for TypeSystemOperations<'a> {
    /// Dart `InterfaceTypeImpl`.
    type TypeDeclarationType = TypeId;
    /// Dart `InterfaceElementImpl`.
    type TypeDeclaration = EId<InterfaceElement>;
    /// Dart `AstNodeImpl`.
    type AstNode = NodeId;
    type ConstraintGenerator<'b>
        = TypeConstraintGatherer<'b, 'a>
    where
        Self: 'b;

    fn double_type(&self) -> SharedTypeView<TypeId> {
        unimplemented!("TODO(paulberry)")
    }

    fn dynamic_type(&self) -> SharedTypeView<TypeId> {
        SharedTypeView::new(self.ctx().tp.dynamic_type())
    }

    fn error_type(&self) -> SharedTypeView<TypeId> {
        SharedTypeView::new(TypeId::INVALID)
    }

    fn int_type(&self) -> SharedTypeView<TypeId> {
        unimplemented!("TODO(paulberry)")
    }

    fn never_type(&self) -> SharedTypeView<TypeId> {
        SharedTypeView::new(self.ctx().tp.never_type())
    }

    fn null_type(&self) -> SharedTypeView<TypeId> {
        SharedTypeView::new(self.ctx().tp.null_type())
    }

    fn object_question_type(&self) -> SharedTypeView<TypeId> {
        SharedTypeView::new(self.type_system.object_question())
    }

    fn object_type(&self) -> SharedTypeView<TypeId> {
        SharedTypeView::new(self.type_system.object_none())
    }

    fn unknown_type(&self) -> SharedTypeSchemaView<TypeId> {
        SharedTypeSchemaView::new(TypeId::UNKNOWN)
    }

    fn create_type_constraint_generator<'b>(
        &'b self,
        type_constraint_generation_data_for_testing: Option<&'b mut DataForTestingOf<Self>>,
        type_parameters_to_infer: &[SharedTypeParameterView<EId<TypeParameterElement>>],
        inference_using_bounds_is_enabled: bool,
    ) -> TypeConstraintGatherer<'b, 'a> {
        let type_parameters: Vec<EId<TypeParameterElement>> = type_parameters_to_infer
            .iter()
            .map(|p| p.unwrap_type_parameter_view_as_type_parameter_structure())
            .collect();
        TypeConstraintGatherer::new(
            &type_parameters,
            self,
            inference_using_bounds_is_enabled,
            type_constraint_generation_data_for_testing,
        )
    }

    fn flatten(&self, ty: SharedTypeView<TypeId>) -> SharedTypeView<TypeId> {
        SharedTypeView::new(self.type_system.flatten(ty.unwrap_type_view()))
    }

    fn future_or_type_internal(&self, type_structure: TypeId) -> TypeId {
        let ctx = self.ctx();
        ctx.tp.future_or_type(&ctx, type_structure)
    }

    fn future_type_internal(&self, type_structure: TypeId) -> TypeId {
        let ctx = self.ctx();
        ctx.tp.future_type(&ctx, type_structure)
    }

    fn get_type_declaration_kind_internal(&self, ty: TypeId) -> Option<TypeDeclarationKind> {
        if self.is_interface_type_internal(ty) {
            Some(TypeDeclarationKind::InterfaceDeclaration)
        } else if self.is_extension_type_internal(ty) {
            Some(TypeDeclarationKind::ExtensionTypeDeclaration)
        } else {
            None
        }
    }

    fn get_type_parameter_variance(
        &self,
        type_declaration: EId<InterfaceElement>,
        parameter_index: usize,
    ) -> Variance {
        let ctx = self.ctx();
        let parameter = ctx.interface_type_parameters(type_declaration)[parameter_index];
        shared_variance(ctx.type_parameter_variance(parameter))
    }

    fn glb_internal(&self, type1: TypeId, type2: TypeId) -> TypeId {
        self.type_system.greatest_lower_bound(type1, type2)
    }

    fn greatest_closure_of_schema(
        &self,
        schema: SharedTypeSchemaView<TypeId>,
        top_type: Option<SharedTypeView<TypeId>>,
    ) -> SharedTypeView<TypeId> {
        // Dart ignores `topType` here.
        let _ = top_type;
        SharedTypeView::new(
            self.type_system
                .greatest_closure_of_schema(schema.unwrap_type_schema_view()),
        )
    }

    fn greatest_closure_of_type_internal(
        &self,
        ty: TypeId,
        type_parameters_to_eliminate: &[EId<TypeParameterElement>],
    ) -> TypeId {
        self.type_system
            .greatest_closure(ty, type_parameters_to_eliminate)
    }

    fn is_always_exhaustive_type(&self, ty: SharedTypeView<TypeId>) -> bool {
        self.type_system.is_always_exhaustive(ty.unwrap_type_view())
    }

    fn is_assignable_to(
        &self,
        from_type: SharedTypeView<TypeId>,
        to_type: SharedTypeView<TypeId>,
    ) -> bool {
        self.type_system.is_assignable_to(
            from_type.unwrap_type_view(),
            to_type.unwrap_type_view(),
            self.strict_casts,
        )
    }

    fn is_bound_omitted(&self, type_parameter: EId<TypeParameterElement>) -> bool {
        self.bound_shared(type_parameter).is_none()
    }

    fn is_dart_core_function_internal(&self, ty: TypeId) -> bool {
        let ctx = self.ctx();
        ctx.nullability_suffix(ty) == Nullability::None && ctx.is_dart_core_function(ty)
    }

    fn is_dart_core_record_internal(&self, ty: TypeId) -> bool {
        let ctx = self.ctx();
        ctx.nullability_suffix(ty) == Nullability::None && ctx.is_dart_core_record(ty)
    }

    fn is_extension_type_internal(&self, ty: TypeId) -> bool {
        self.ctx().is_extension_type(ty)
    }

    fn is_interface_type_internal(&self, ty: TypeId) -> bool {
        let ctx = self.ctx();
        matches!(ctx.ty(ty), TypeKind::Interface { .. })
            && !ctx.is_dart_core_null(ty)
            && !ctx.is_dart_async_future_or(ty)
            && !ctx.is_extension_type(ty)
    }

    fn is_known_type(&self, type_schema: SharedTypeSchemaView<TypeId>) -> bool {
        type_schema::is_known(&self.ctx(), type_schema.unwrap_type_schema_view())
    }

    fn is_non_nullable_internal(&self, ty: TypeId) -> bool {
        self.type_system.is_non_nullable(ty)
    }

    fn is_nullable_internal(&self, ty: TypeId) -> bool {
        self.type_system.is_nullable(ty)
    }

    fn is_object(&self, ty: SharedTypeView<TypeId>) -> bool {
        let ctx = self.ctx();
        let t = ty.unwrap_type_view();
        ctx.is_dart_core_object(t) && !ctx.is_question_type(t)
    }

    fn is_subtype_of_internal(&self, left: TypeId, right: TypeId) -> bool {
        self.type_system.is_subtype_of(left, right)
    }

    fn is_type_schema_satisfied(
        &self,
        type_schema: SharedTypeSchemaView<TypeId>,
        ty: SharedTypeView<TypeId>,
    ) -> bool {
        self.type_system
            .is_subtype_of(ty.unwrap_type_view(), type_schema.unwrap_type_schema_view())
    }

    fn is_variable_final(&self, node: EId<PromotableElement>) -> bool {
        let _ = node;
        todo!(
            "TypeSystemOperations.isVariableFinal (resolver unit C2: PromotableElementImpl.isFinal)"
        )
    }

    fn iterable_type_schema(
        &self,
        element_type_schema: SharedTypeSchemaView<TypeId>,
    ) -> SharedTypeSchemaView<TypeId> {
        let ctx = self.ctx();
        SharedTypeSchemaView::new(
            ctx.tp
                .iterable_type(&ctx, element_type_schema.unwrap_type_schema_view()),
        )
    }

    fn least_closure_of_schema(
        &self,
        schema: SharedTypeSchemaView<TypeId>,
    ) -> SharedTypeView<TypeId> {
        SharedTypeView::new(
            self.type_system
                .least_closure_of_schema(schema.unwrap_type_schema_view()),
        )
    }

    fn least_closure_of_type_internal(
        &self,
        ty: TypeId,
        type_parameters_to_eliminate: &[EId<TypeParameterElement>],
    ) -> TypeId {
        self.type_system
            .least_closure(ty, type_parameters_to_eliminate)
    }

    fn list_type_internal(&self, element_type: TypeId) -> TypeId {
        let ctx = self.ctx();
        ctx.tp.list_type(&ctx, element_type)
    }

    fn lub_internal(&self, type1: TypeId, type2: TypeId) -> TypeId {
        self.type_system.least_upper_bound(type1, type2)
    }

    fn make_nullable_internal(&self, ty: TypeId) -> TypeId {
        self.type_system.make_nullable(ty)
    }

    fn map_type_internal(&self, key_type: TypeId, value_type: TypeId) -> TypeId {
        let ctx = self.ctx();
        ctx.tp.map_type(&ctx, key_type, value_type)
    }

    fn match_future_or_internal(&self, ty: TypeId) -> Option<TypeId> {
        let ctx = self.ctx();
        if ctx.is_dart_async_future_or(ty) {
            Some(ctx.type_arguments(ty)[0])
        } else {
            None
        }
    }

    fn match_inferable_parameter_internal(&self, ty: TypeId) -> Option<EId<TypeParameterElement>> {
        match *self.ctx().ty(ty) {
            TypeKind::TypeParameter { param, .. } => Some(param),
            _ => None,
        }
    }

    fn match_iterable_type_internal(&self, ty: TypeId) -> Option<TypeId> {
        let ctx = self.ctx();
        let iterable_element = ctx.tp.iterable_element();
        let list_type = ctx.as_instance_of(ty, iterable_element.upcast())?;
        Some(ctx.type_arguments(list_type)[0])
    }

    fn match_list_type(&self, ty: SharedTypeView<TypeId>) -> Option<SharedTypeView<TypeId>> {
        let ctx = self.ctx();
        let list_element = ctx.tp.list_element();
        let list_type = ctx.as_instance_of(ty.unwrap_type_view(), list_element.upcast())?;
        Some(SharedTypeView::new(ctx.type_arguments(list_type)[0]))
    }

    fn match_map_type(
        &self,
        ty: SharedTypeView<TypeId>,
    ) -> Option<dartr_flow::type_analyzer_operations::KeyValueTypes<SharedTypeView<TypeId>>> {
        let ctx = self.ctx();
        let map_element = ctx.tp.map_element();
        let map_type = ctx.as_instance_of(ty.unwrap_type_view(), map_element.upcast())?;
        let args = ctx.type_arguments(map_type);
        Some(dartr_flow::type_analyzer_operations::KeyValueTypes {
            key_type: SharedTypeView::new(args[0]),
            value_type: SharedTypeView::new(args[1]),
        })
    }

    fn match_stream_type(&self, ty: SharedTypeView<TypeId>) -> Option<SharedTypeView<TypeId>> {
        let ctx = self.ctx();
        let stream_element = ctx.tp.stream_element();
        let list_type = ctx.as_instance_of(ty.unwrap_type_view(), stream_element.upcast())?;
        Some(SharedTypeView::new(ctx.type_arguments(list_type)[0]))
    }

    fn match_type_declaration_type_internal(
        &self,
        ty: TypeId,
    ) -> Option<TypeDeclarationMatchResult<TypeId, EId<InterfaceElement>, TypeId>> {
        let ctx = self.ctx();
        let type_declaration_kind = if self.is_interface_type_internal(ty) {
            TypeDeclarationKind::InterfaceDeclaration
        } else if self.is_extension_type_internal(ty) {
            TypeDeclarationKind::ExtensionTypeDeclaration
        } else {
            return None;
        };
        Some(TypeDeclarationMatchResult {
            type_declaration_kind,
            type_declaration_type: ty,
            type_declaration: ctx.interface_element(ty).expect("interface type"),
            type_arguments: ctx.type_arguments(ty).to_vec(),
        })
    }

    fn match_type_parameter_bound_internal(&self, ty: TypeId) -> Option<TypeId> {
        let ctx = self.ctx();
        match *ctx.ty(ty) {
            TypeKind::TypeParameter {
                param,
                nullability: Nullability::None,
                promoted_bound,
                ..
            } => promoted_bound.or_else(|| ctx.type_parameter_bound(param)),
            _ => None,
        }
    }

    fn normalize(&self, ty: SharedTypeView<TypeId>) -> SharedTypeView<TypeId> {
        SharedTypeView::new(self.type_system.normalize(ty.unwrap_type_view()))
    }

    fn record_type_internal(&self, positional: &[TypeId], named: &[(Name, TypeId)]) -> TypeId {
        let named: Vec<NamedType> = named
            .iter()
            .map(|&(name, ty)| NamedType { name, ty })
            .collect();
        self.ctx()
            .record_type(positional, &named, Nullability::None, None)
    }

    fn stream_type_schema(
        &self,
        element_type_schema: SharedTypeSchemaView<TypeId>,
    ) -> SharedTypeSchemaView<TypeId> {
        let ctx = self.ctx();
        SharedTypeSchemaView::new(
            ctx.tp
                .stream_type(&ctx, element_type_schema.unwrap_type_schema_view()),
        )
    }

    fn substitute_type_from_iterables(
        &self,
        type_to_substitute: TypeId,
        type_parameters: &[EId<TypeParameterElement>],
        types: &[TypeId],
    ) -> TypeId {
        MapSubstitution::from_pairs(type_parameters, types)
            .substitute_type(&self.ctx(), type_to_substitute)
    }

    fn type_to_schema(&self, ty: SharedTypeView<TypeId>) -> SharedTypeSchemaView<TypeId> {
        SharedTypeSchemaView::new(ty.unwrap_type_view())
    }

    fn lookup_member_type_internal(&self, ty: TypeId, lookup_name: Name) -> Option<TypeId> {
        let _ = (ty, lookup_name);
        // TODO(cstefantsova): Implement lookupMemberTypeInternal.
        unimplemented!()
    }

    dartr_type_analyzer::type_analyzer_operations_mixin!();
}
