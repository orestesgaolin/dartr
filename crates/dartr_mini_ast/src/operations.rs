// Dart source: pkg/_fe_analyzer_shared/test/mini_ast.dart (class
// `MiniAstOperations`)

//! [`MiniAstOperations`]: the type operations of the mini-AST test harness.
//!
//! The Dart class mixes in `TypeAnalyzerOperationsMixin`; here the mixin is
//! applied with [`dartr_type_analyzer::type_analyzer_operations_mixin!`].
//!
//! The Dart class mutates its tables (`addLub`, `addExhaustiveness`, ...)
//! through a shared reference; here the tables are in `RefCell`s, so the
//! `add_...` methods take `&self` too.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;

use dartr_flow::flow_analysis_operations::{
    FlowAnalysisOperations, FlowAnalysisTypeOperations, PropertyNonPromotabilityReason,
    TypeClassification,
};
use dartr_flow::shared_type::{
    SharedNamedFunctionParameter, SharedNamedType, SharedTypeKind, SharedTypeOperations,
    SharedTypeParameterView, SharedTypeSchemaView, SharedTypeView, Variance,
};
use dartr_flow::type_analyzer_operations::{
    DataForTestingOf, KeyValueTypes, TypeAnalyzerOperations, TypeDeclarationKind,
    TypeDeclarationMatchResult,
};

use super::mini_type_constraint_gatherer::TypeConstraintGatherer;
use super::mini_types::{
    DynamicType, FutureOrType, InvalidType, Name, NamedType, NeverType, NullType, PrimaryType,
    RecordType, Type, TypeKind, TypeParameter, TypeParameterType, TypeRegistry, TypeSystem, t,
};
use super::node::{Category, Node, PropertyElement, Var};
use dartr_flow::flow_analysis_impl::model::FlowTypes;

type View = SharedTypeView<Type>;
type SchemaView = SharedTypeSchemaView<Type>;

/// The [`FlowTypes`] of the mini AST: `FlowAnalysisImpl<MiniAstTypes>` is
/// Dart `FlowAnalysis<Node, Statement, Expression, Var>`. Statements and
/// expressions are [`Node`]s.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MiniAstTypes;

impl FlowTypes for MiniAstTypes {
    type Ops = MiniAstOperations;
    type Node = Node;
    type Statement = Node;
    type Expression = Node;

    fn statement_to_node(statement: Node) -> Node {
        statement
    }

    /// Dart `node is Expression`.
    fn is_expression(node: Node) -> bool {
        node.category() == Category::Expression
    }
}

/// The type operations of the mini-AST test harness (Dart class
/// `MiniAstOperations`).
///
/// Queries that the harness can't compute (glb, lub, normalization,
/// exhaustiveness, downward inference) are answered from tables keyed by
/// the type strings; tests extend the tables with the `add_...` methods.
///
/// A cheap handle (`Rc`): the harness, the type analyzer and flow analysis
/// (which owns its operations, `FlowTypes::Ops`) share one state.
#[derive(Clone)]
pub struct MiniAstOperations(Rc<MiniAstOperationsData>);

impl std::ops::Deref for MiniAstOperations {
    type Target = MiniAstOperationsData;
    fn deref(&self) -> &MiniAstOperationsData {
        &self.0
    }
}

/// The state of [`MiniAstOperations`].
pub struct MiniAstOperationsData {
    /// `objectQuestionType`.
    object_question_type: Type,
    /// `objectType`.
    object_type: Type,
    /// `unknownType`.
    unknown_type: Type,
    /// `intType`.
    int_type: Type,
    /// `doubleType`.
    double_type: Type,
    /// `boolType`.
    bool_type: Type,
    exhaustiveness: RefCell<HashMap<String, bool>>,
    extension_type_erasure: RefCell<HashMap<String, Type>>,
    glbs: RefCell<HashMap<String, Type>>,
    lubs: RefCell<HashMap<String, Type>>,
    downward_inference_results: RefCell<HashMap<String, Type>>,
    promotion_exceptions: RefCell<HashMap<String, HashMap<String, String>>>,
    normalize_results: RefCell<HashMap<String, Type>>,
    type_system: RefCell<TypeSystem>,
    variance: RefCell<HashMap<String, Vec<Variance>>>,
}

fn table(entries: &[(&str, &str)]) -> HashMap<String, Type> {
    entries.iter().map(|(k, v)| (k.to_string(), t(v))).collect()
}

impl Default for MiniAstOperations {
    fn default() -> Self {
        Self::new()
    }
}

impl MiniAstOperations {
    /// `MiniAstOperations()`. The type registry must be initialized.
    pub fn new() -> Self {
        let core_exhaustiveness: HashMap<String, bool> = [
            ("()", true),
            ("(int, int?)", false),
            ("bool", true),
            ("dynamic", false),
            ("int", false),
            ("int?", false),
            ("List<int>", false),
            ("Never", false),
            ("num", false),
            ("num?", false),
            ("Object", false),
            ("Object?", false),
            ("String", false),
            ("String?", false),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect();

        let core_glbs = table(&[
            ("_, int", "int"),
            ("(int,), _", "(int,)"),
            ("(num,), _", "(num,)"),
            ("Object?, double", "double"),
            ("Object?, int", "int"),
            ("double, int", "Never"),
            ("double?, int?", "Null"),
            ("int?, num", "int"),
            ("Null, int", "Never"),
        ]);

        let core_lubs = table(&[
            ("double, int", "num"),
            ("double?, int?", "num?"),
            ("int, num", "num"),
            ("Null, bool", "bool?"),
            ("Null, dynamic", "dynamic"),
            ("Null, int", "int?"),
            ("Null, Object", "Object?"),
            ("Null, String", "String?"),
            ("int, _", "int"),
            ("List<_>, _", "List<_>"),
            ("Null, _", "Null"),
        ]);

        let core_downward_inference_results = table(&[
            ("bool <: bool", "bool"),
            ("dynamic <: int", "dynamic"),
            ("dynamic <: Null", "dynamic"),
            ("error <: int", "error"),
            ("error <: num", "error"),
            ("int <: dynamic", "int"),
            ("int <: int", "int"),
            ("int <: Null", "int"),
            ("int <: num", "int"),
            ("int <: Object", "int"),
            ("int <: Object?", "int"),
            ("List <: Iterable<int>", "List<int>"),
            ("Never <: int", "Never"),
            ("Null <: int", "Null"),
            ("Null <: Null", "Null"),
            ("num <: int", "num"),
            ("num <: Object", "num"),
            ("Object <: num", "Object"),
            ("String <: num", "String"),
        ]);

        let core_normalize_results = table(&[
            ("Object", "Object"),
            ("FutureOr<Object>", "Object"),
            ("double", "double"),
            ("int", "int"),
            ("int?", "int?"),
            ("num", "num"),
            ("String?", "String?"),
            ("List<int>", "List<int>"),
        ]);

        MiniAstOperations(Rc::new(MiniAstOperationsData {
            object_question_type: t("Object?"),
            object_type: t("Object"),
            unknown_type: t("_"),
            int_type: t("int"),
            double_type: t("double"),
            bool_type: t("bool"),
            exhaustiveness: RefCell::new(core_exhaustiveness),
            extension_type_erasure: RefCell::new(HashMap::new()),
            glbs: RefCell::new(core_glbs),
            lubs: RefCell::new(core_lubs),
            downward_inference_results: RefCell::new(core_downward_inference_results),
            promotion_exceptions: RefCell::new(HashMap::new()),
            normalize_results: RefCell::new(core_normalize_results),
            type_system: RefCell::new(TypeSystem::new()),
            variance: RefCell::new(HashMap::new()),
        }))
    }

    /// Updates the harness with a new result for
    /// [`downward_infer`](Self::downward_infer).
    pub fn add_downward_infer(&self, name: &str, context: &str, result: &str) {
        let query = format!("{name} <: {context}");
        self.downward_inference_results
            .borrow_mut()
            .insert(query, t(result));
    }

    /// Updates the harness so that when an `isAlwaysExhaustiveType` query is
    /// invoked on type `ty`, `is_exhaustive` will be returned.
    pub fn add_exhaustiveness(&self, ty: &str, is_exhaustive: bool) {
        self.exhaustiveness
            .borrow_mut()
            .insert(ty.to_string(), is_exhaustive);
    }

    /// Updates the harness so that when an extension type erasure query is
    /// invoked on type `ty`, `representation` will be returned.
    pub fn add_extension_type_erasure(&self, ty: &str, representation: &str) {
        self.extension_type_erasure
            .borrow_mut()
            .insert(ty.to_string(), t(representation));
    }

    /// `addLub`.
    pub fn add_lub(&self, type1: &str, type2: &str, result_type: &str) {
        self.lubs
            .borrow_mut()
            .insert(format!("{type1}, {type2}"), t(result_type));
    }

    /// `addPromotionException`.
    pub fn add_promotion_exception(&self, from: &str, to: &str, result: &str) {
        self.promotion_exceptions
            .borrow_mut()
            .entry(from.to_string())
            .or_default()
            .insert(to.to_string(), result.to_string());
    }

    /// `addSuperInterfaces`.
    pub fn add_super_interfaces(
        &self,
        class_name: &str,
        template: impl Fn(&[Type]) -> Vec<Type> + 'static,
    ) {
        self.type_system
            .borrow_mut()
            .add_super_interfaces(class_name, template);
    }

    /// `addVariance`.
    pub fn add_variance(&self, type_name: &str, variance_by_argument: Vec<Variance>) {
        self.variance
            .borrow_mut()
            .insert(type_name.to_string(), variance_by_argument);
    }

    /// Returns the downward inference result of a type with the given
    /// `name`, in the `context`. For example infer `List<int>` from
    /// `Iterable<int>`.
    pub fn downward_infer(&self, name: &str, context: Type) -> Type {
        let query = format!("{name} <: {context}");
        match self.downward_inference_results.borrow().get(&query) {
            Some(result) => *result,
            None => panic!("Unknown downward inference query: {query}"),
        }
    }

    /// `flatten`, on unwrapped types.
    fn flatten_type(&self, t: Type) -> Type {
        // (Note: comments below are pulled from the definition of the "flatten"
        // function in the "Function Expressions" section of the language  spec.)

        // We define the auxiliary function flatten(T) as follows, using the first
        // applicable case:

        // - If T is X & S for some type variable X and type S then
        if let Some(TypeParameterType {
            type_parameter,
            promotion: Some(s),
            ..
        }) = t.as_type_parameter_type()
        {
            //   - If S derives a future type U then flatten(T) ≜ flatten(U).
            let derived = self.type_system.borrow().derived_future_type(s);
            if let Some(u) = derived {
                return self.flatten_type(u);
            }

            //   - otherwise, flatten(T) ≜ flatten(X)
            return self.flatten_type(TypeParameterType::new(type_parameter));
        }

        // - If T derives a future type Future<S> or FutureOr<S> then
        //   flatten(T) ≜ S.
        // - If T derives a future type Future<S>? or FutureOr<S>? then
        //   flatten(T) ≜ S?.
        let derived = self.type_system.borrow().derived_future_type(t);
        if let Some(f) = derived {
            let s = match f.as_primary_type() {
                Some(p) if p.name() == "Future" && p.args.len() == 1 => p.args[0],
                _ => match f.future_or_type_argument() {
                    Some(s) => s,
                    None => {
                        panic!("Derived future type should always be Future<...> or FutureOr<...>")
                    }
                },
            };
            if f.is_question_type() {
                return s.as_question_type(true);
            } else {
                return s;
            }
        }

        // - Otherwise, flatten(T) ≜ T.
        t
    }
}

// ============================================================ SharedTypeOperations

impl SharedTypeOperations for MiniAstOperations {
    type Type = Type;
    type TypeParameter = TypeParameter;
    type Name = Name;

    fn compare_names(&self, name1: Name, name2: Name) -> Ordering {
        name1.cmp(name2)
    }

    fn shared_type_kind(&self, ty: Type) -> SharedTypeKind {
        match ty.kind() {
            TypeKind::Dynamic => SharedTypeKind::Dynamic,
            TypeKind::Invalid => SharedTypeKind::Invalid,
            TypeKind::Null => SharedTypeKind::Null,
            TypeKind::Void => SharedTypeKind::Void,
            TypeKind::Unknown => SharedTypeKind::Unknown,
            TypeKind::Function => SharedTypeKind::Function,
            TypeKind::Record => SharedTypeKind::Record,
            TypeKind::Never | TypeKind::FutureOr | TypeKind::Primary | TypeKind::TypeParameter => {
                SharedTypeKind::Other
            }
        }
    }

    fn is_question_type(&self, ty: Type) -> bool {
        ty.is_question_type()
    }

    fn as_question_type(&self, ty: Type, is_question_type: bool) -> Type {
        ty.as_question_type(is_question_type)
    }

    fn get_display_string(&self, ty: Type) -> String {
        ty.get_display_string()
    }

    fn is_structurally_equal_to(&self, ty: Type, other: Type) -> bool {
        ty.is_structurally_equal_to(other)
    }

    fn positional_parameter_types_shared(&self, function_type: Type) -> Vec<Type> {
        function_type
            .as_function_type()
            .expect("not a FunctionType")
            .positional_parameters
    }

    fn required_positional_parameter_count(&self, function_type: Type) -> usize {
        function_type
            .as_function_type()
            .expect("not a FunctionType")
            .required_positional_parameter_count
    }

    fn return_type_shared(&self, function_type: Type) -> Type {
        function_type
            .as_function_type()
            .expect("not a FunctionType")
            .return_type
    }

    fn sorted_named_parameters_shared(
        &self,
        function_type: Type,
    ) -> Vec<SharedNamedFunctionParameter<Name, Type>> {
        function_type
            .as_function_type()
            .expect("not a FunctionType")
            .named_parameters
            .iter()
            .map(|p| SharedNamedFunctionParameter {
                is_required: p.is_required,
                name_shared: p.name,
                type_shared: p.ty,
            })
            .collect()
    }

    fn type_parameters_shared(&self, function_type: Type) -> Vec<TypeParameter> {
        function_type
            .as_function_type()
            .expect("not a FunctionType")
            .type_parameters_shared
    }

    fn positional_types_shared(&self, record_type: Type) -> Vec<Type> {
        record_type
            .as_record_type()
            .expect("not a RecordType")
            .positional_types
    }

    fn sorted_named_types_shared(&self, record_type: Type) -> Vec<SharedNamedType<Name, Type>> {
        record_type
            .as_record_type()
            .expect("not a RecordType")
            .named_types
            .iter()
            .map(|n| SharedNamedType {
                name_shared: n.name,
                type_shared: n.ty,
            })
            .collect()
    }

    fn bound_shared(&self, type_parameter: TypeParameter) -> Option<Type> {
        type_parameter.bound_shared()
    }

    fn display_name(&self, type_parameter: TypeParameter) -> String {
        type_parameter.display_name()
    }

    fn variance(&self, type_parameter: TypeParameter) -> Variance {
        type_parameter.variance()
    }

    fn is_legacy_covariant(&self, type_parameter: TypeParameter) -> bool {
        type_parameter.is_legacy_covariant()
    }

    fn invocation_structural_context_schema_return_type(&self, schema: Type) -> Type {
        panic!("mini types have no structural context schemas: {schema}")
    }

    fn lookup_structural_context_schema_lookup_name(&self, schema: Type) -> Name {
        panic!("mini types have no structural context schemas: {schema}")
    }

    fn lookup_structural_context_schema_lookup_type(&self, schema: Type) -> Type {
        panic!("mini types have no structural context schemas: {schema}")
    }
}

// ====================================================== FlowAnalysisTypeOperations

impl FlowAnalysisTypeOperations for MiniAstOperations {
    fn bool_type(&self) -> View {
        SharedTypeView::new(self.bool_type)
    }

    fn classify_type(&self, ty: View) -> TypeClassification {
        if self.is_subtype_of_internal(ty.unwrap_type_view(), t("Object")) {
            TypeClassification::NonNullable
        } else if self.is_subtype_of_internal(ty.unwrap_type_view(), NullType::instance()) {
            TypeClassification::NullOrEquivalent
        } else {
            TypeClassification::PotentiallyNullable
        }
    }

    fn extension_type_erasure(&self, ty: View) -> View {
        let query = ty.unwrap_type_view().to_string();
        SharedTypeView::new(
            self.extension_type_erasure
                .borrow()
                .get(&query)
                .copied()
                .unwrap_or(ty.unwrap_type_view()),
        )
    }

    fn factor(&self, from: View, what: View) -> View {
        SharedTypeView::new(
            self.type_system
                .borrow()
                .factor(from.unwrap_type_view(), what.unwrap_type_view()),
        )
    }

    fn is_bottom_type(&self, ty: View) -> bool {
        let unwrapped_type = ty.unwrap_type_view();
        unwrapped_type.kind() == TypeKind::Never && !unwrapped_type.is_question_type()
    }

    fn is_subtype_of(&self, left_type: View, right_type: View) -> bool {
        // `TypeAnalyzerOperationsMixin.isSubtypeOf`.
        self.is_subtype_of_internal(left_type.unwrap_type_view(), right_type.unwrap_type_view())
    }

    fn is_type_parameter_type(&self, ty: View) -> bool {
        let unwrapped_type = ty.unwrap_type_view();
        unwrapped_type.kind() == TypeKind::TypeParameter && !unwrapped_type.is_question_type()
    }

    fn is_invalid_type(&self, ty: View) -> bool {
        ty.unwrap_type_view().kind() == TypeKind::Invalid
    }

    fn make_nullable(&self, ty: View) -> View {
        // `TypeAnalyzerOperationsMixin.makeNullable`.
        SharedTypeView::new(self.make_nullable_internal(ty.unwrap_type_view()))
    }

    fn promote_to_non_null(&self, ty: View) -> View {
        let unwrapped_type = ty.unwrap_type_view();
        if unwrapped_type.is_question_type() {
            SharedTypeView::new(unwrapped_type.as_question_type(false))
        } else if unwrapped_type.kind() == TypeKind::Null {
            SharedTypeView::new(NeverType::instance())
        } else {
            ty
        }
    }

    fn try_promote_to_type(&self, to: View, from: View) -> Option<View> {
        let exception = self
            .promotion_exceptions
            .borrow()
            .get(&from.unwrap_type_view().to_string())
            .and_then(|m| m.get(&to.unwrap_type_view().to_string()).cloned());
        if let Some(exception) = exception {
            return Some(SharedTypeView::new(t(&exception)));
        }
        if self.is_subtype_of_internal(to.unwrap_type_view(), from.unwrap_type_view()) {
            Some(to)
        } else {
            None
        }
    }
}

// ========================================================== FlowAnalysisOperations

impl FlowAnalysisOperations for MiniAstOperations {
    type Variable = Var;
    type PropertyMember = PropertyElement;

    fn is_final(&self, variable: Var) -> bool {
        variable.is_final()
    }

    fn is_property_promotable(&self, property: &PropertyElement) -> bool {
        property.is_promotable
    }

    fn is_private_name(&self, name: Name) -> bool {
        name.starts_with('_')
    }

    fn variable_type(&self, variable: Var) -> View {
        SharedTypeView::new(variable.ty())
    }

    fn why_property_is_not_promotable(
        &self,
        property: &PropertyElement,
    ) -> Option<PropertyNonPromotabilityReason> {
        property.why_not_promotable
    }
}

// ========================================================== TypeAnalyzerOperations

impl TypeAnalyzerOperations for MiniAstOperations {
    type TypeDeclarationType = Type;
    type TypeDeclaration = Name;
    type AstNode = Node;
    type ConstraintGenerator<'a>
        = TypeConstraintGatherer
    where
        Self: 'a;

    fn double_type(&self) -> View {
        SharedTypeView::new(self.double_type)
    }

    fn dynamic_type(&self) -> View {
        SharedTypeView::new(DynamicType::instance())
    }

    fn error_type(&self) -> View {
        SharedTypeView::new(InvalidType::instance())
    }

    fn int_type(&self) -> View {
        SharedTypeView::new(self.int_type)
    }

    fn never_type(&self) -> View {
        SharedTypeView::new(NeverType::instance())
    }

    fn null_type(&self) -> View {
        SharedTypeView::new(NullType::instance())
    }

    fn object_question_type(&self) -> View {
        SharedTypeView::new(self.object_question_type)
    }

    fn object_type(&self) -> View {
        SharedTypeView::new(self.object_type)
    }

    fn unknown_type(&self) -> SchemaView {
        SharedTypeSchemaView::new(self.unknown_type)
    }

    fn create_type_constraint_generator<'a>(
        &'a self,
        type_constraint_generation_data_for_testing: Option<&'a mut DataForTestingOf<Self>>,
        type_parameters_to_infer: &[SharedTypeParameterView<TypeParameter>],
        inference_using_bounds_is_enabled: bool,
    ) -> TypeConstraintGatherer {
        let _ = (
            type_constraint_generation_data_for_testing,
            inference_using_bounds_is_enabled,
        );
        let names: Vec<Name> = type_parameters_to_infer
            .iter()
            .map(|p| {
                p.unwrap_type_parameter_view_as_type_parameter_structure()
                    .name()
            })
            .collect();
        TypeConstraintGatherer::new(&names)
    }

    fn flatten(&self, ty: View) -> View {
        SharedTypeView::new(self.flatten_type(ty.unwrap_type_view()))
    }

    fn future_or_type_internal(&self, argument_type: Type) -> Type {
        FutureOrType::new(argument_type)
    }

    fn future_type_internal(&self, argument_type: Type) -> Type {
        PrimaryType::new(TypeRegistry::future(), vec![argument_type])
    }

    fn get_type_declaration_kind_internal(&self, ty: Type) -> Option<TypeDeclarationKind> {
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
        type_declaration: Name,
        parameter_index: usize,
    ) -> Variance {
        self.variance
            .borrow()
            .get(type_declaration)
            .and_then(|v| v.get(parameter_index).copied())
            .unwrap_or(Variance::Covariant)
    }

    fn glb_internal(&self, type1: Type, type2: Type) -> Type {
        if type1.to_string() == type2.to_string() {
            return type1;
        }
        let mut type_names = [type1.to_string(), type2.to_string()];
        type_names.sort();
        let query = type_names.join(", ");
        match self.glbs.borrow().get(&query) {
            Some(result) => *result,
            None => panic!("Unknown glb query: {query}"),
        }
    }

    fn greatest_closure_of_schema(&self, schema: SchemaView, top_type: Option<View>) -> View {
        let _ = top_type;
        let schema = schema.unwrap_type_schema_view();
        SharedTypeView::new(
            schema
                .closure_with_respect_to_unknown(true)
                .unwrap_or(schema),
        )
    }

    fn greatest_closure_of_type_internal(
        &self,
        ty: Type,
        type_parameters_to_eliminate: &[TypeParameter],
    ) -> Type {
        let _ = (ty, type_parameters_to_eliminate);
        // TODO(paulberry): Implement greatest closure of types in mini ast.
        unimplemented!("greatestClosureOfTypeInternal")
    }

    fn is_always_exhaustive_type(&self, ty: View) -> bool {
        let query = ty.unwrap_type_view().to_string();
        match self.exhaustiveness.borrow().get(&query) {
            Some(result) => *result,
            None => panic!("Unknown exhaustiveness query: {query}"),
        }
    }

    fn is_assignable_to(&self, from_type: View, to_type: View) -> bool {
        if from_type.unwrap_type_view().kind() == TypeKind::Dynamic {
            return true;
        }
        if from_type.unwrap_type_view().kind() == TypeKind::Invalid {
            return true;
        }
        self.is_subtype_of_internal(from_type.unwrap_type_view(), to_type.unwrap_type_view())
    }

    fn is_bound_omitted(&self, type_parameter: TypeParameter) -> bool {
        let _ = type_parameter;
        // TODO(paulberry): Implement isBoundOmitted in mini ast.
        unimplemented!("isBoundOmitted")
    }

    fn is_dart_core_function_internal(&self, ty: Type) -> bool {
        matches!(ty.as_primary_type(), Some(p)
            if !p.is_question_type && p.name() == "Function" && p.args.is_empty())
    }

    fn is_dart_core_record_internal(&self, ty: Type) -> bool {
        matches!(ty.as_primary_type(), Some(p)
            if !p.is_question_type && p.name() == "Record" && p.args.is_empty())
    }

    fn is_extension_type_internal(&self, ty: Type) -> bool {
        let _ = ty;
        // TODO(cstefantsova): Add the support for extension types in the mini ast
        // testing framework.
        false
    }

    fn is_interface_type_internal(&self, ty: Type) -> bool {
        matches!(ty.as_primary_type(), Some(p) if p.is_interface_type())
    }

    fn is_known_type(&self, type_schema: SchemaView) -> bool {
        let unwrapped = type_schema.unwrap_type_schema_view();
        let known = |t: Type| self.is_known_type(SharedTypeSchemaView::new(t));
        if let Some(type_argument) = unwrapped.future_or_type_argument() {
            return known(type_argument);
        }
        if let Some(p) = unwrapped.as_primary_type() {
            return p.args.iter().all(|arg| known(*arg));
        }
        if let Some(f) = unwrapped.as_function_type() {
            if !known(f.return_type) {
                return false;
            }
            for type_parameter in &f.type_parameters_shared {
                if !known(type_parameter.bound()) {
                    return false;
                }
            }
            for positional_parameter in &f.positional_parameters {
                if !known(*positional_parameter) {
                    return false;
                }
            }
            for named_parameter in &f.named_parameters {
                if !known(named_parameter.ty) {
                    return false;
                }
            }
            return true;
        }
        if let Some(r) = unwrapped.as_record_type() {
            for positional_type in &r.positional_types {
                if !known(*positional_type) {
                    return false;
                }
            }
            for NamedType { ty, .. } in &r.named_types {
                if !known(*ty) {
                    return false;
                }
            }
            return true;
        }
        if unwrapped.as_unknown_type().is_some() {
            return false;
        }
        true
    }

    fn is_non_nullable_internal(&self, ty: Type) -> bool {
        let unwrapped_type = ty;
        if matches!(
            unwrapped_type.kind(),
            TypeKind::Dynamic
                | TypeKind::Unknown
                | TypeKind::Void
                | TypeKind::Null
                | TypeKind::Invalid
        ) {
            return false;
        } else if let Some(TypeParameterType {
            promotion,
            type_parameter,
            is_question_type: false,
        }) = unwrapped_type.as_type_parameter_type()
        {
            return match promotion {
                Some(promotion) => self.is_non_nullable_internal(promotion),
                None => self.is_non_nullable_internal(type_parameter.bound()),
            };
        } else if ty.is_question_type() {
            return false;
        } else if let Some(type_argument) = self.match_future_or_internal(unwrapped_type) {
            return self.is_non_nullable_internal(type_argument);
        }
        true
    }

    fn is_nullable_internal(&self, ty: Type) -> bool {
        let unwrapped_type = ty;
        if matches!(
            unwrapped_type.kind(),
            TypeKind::Dynamic | TypeKind::Unknown | TypeKind::Void | TypeKind::Null
        ) {
            return true;
        } else if ty.is_question_type() {
            return false;
        } else if let Some(type_argument) = self.match_future_or_internal(unwrapped_type) {
            return self.is_nullable_internal(type_argument);
        }
        // TODO(cstefantsova): Update to a fast-pass implementation when the
        // mini-ast testing framework supports looking up superinterfaces of
        // extension types or looking up bounds of type parameters.
        self.type_system
            .borrow()
            .is_subtype(NullType::instance(), unwrapped_type)
    }

    fn is_object(&self, ty: View) -> bool {
        matches!(ty.unwrap_type_view().as_primary_type(), Some(p)
            if !p.is_question_type && p.name() == "Object" && p.args.is_empty())
    }

    fn is_subtype_of_internal(&self, left_type: Type, right_type: Type) -> bool {
        self.type_system.borrow().is_subtype(left_type, right_type)
    }

    fn is_type_schema_satisfied(&self, type_schema: SchemaView, ty: View) -> bool {
        self.is_subtype_of_internal(ty.unwrap_type_view(), type_schema.unwrap_type_schema_view())
    }

    fn is_variable_final(&self, node: Var) -> bool {
        node.is_final()
    }

    fn iterable_type_schema(&self, element_type_schema: SchemaView) -> SchemaView {
        SharedTypeSchemaView::new(PrimaryType::new(
            TypeRegistry::iterable(),
            vec![element_type_schema.unwrap_type_schema_view()],
        ))
    }

    fn least_closure_of_schema(&self, schema: SchemaView) -> View {
        let _ = schema;
        // TODO(paulberry): Implement leastClosureOfSchema in mini ast.
        unimplemented!("leastClosureOfSchema")
    }

    fn least_closure_of_type_internal(
        &self,
        ty: Type,
        type_parameters_to_eliminate: &[TypeParameter],
    ) -> Type {
        let _ = (ty, type_parameters_to_eliminate);
        // TODO(paulberry): Implement greatest closure of types in mini ast.
        unimplemented!("leastClosureOfTypeInternal")
    }

    fn list_type_internal(&self, element_type: Type) -> Type {
        PrimaryType::new(TypeRegistry::list(), vec![element_type])
    }

    fn lub_internal(&self, type1: Type, type2: Type) -> Type {
        let view1 = SharedTypeView::new(type1);
        let view2 = SharedTypeView::new(type2);
        if type1 == type2 {
            type1
        } else if self.promote_to_non_null(view1) == view2 {
            type1
        } else if self.promote_to_non_null(view2) == view1 {
            type2
        } else if type1.kind() == TypeKind::Null && self.promote_to_non_null(view2) != view2 {
            // type2 is already nullable
            type2
        } else if type2.kind() == TypeKind::Null && self.promote_to_non_null(view1) != view1 {
            // type1 is already nullable
            type1
        } else if type1.kind() == TypeKind::Never && !type1.is_question_type() {
            type2
        } else if type2.kind() == TypeKind::Never && !type2.is_question_type() {
            type1
        } else {
            let mut type_names = [type1.to_string(), type2.to_string()];
            type_names.sort();
            let query = type_names.join(", ");
            match self.lubs.borrow().get(&query) {
                Some(result) => *result,
                None => panic!("Unknown lub query: {query}"),
            }
        }
    }

    fn make_nullable_internal(&self, ty: Type) -> Type {
        self.lub_internal(ty, NullType::instance())
    }

    fn map_type_internal(&self, key_type: Type, value_type: Type) -> Type {
        PrimaryType::new(TypeRegistry::map(), vec![key_type, value_type])
    }

    fn match_future_or_internal(&self, ty: Type) -> Option<Type> {
        ty.future_or_type_argument()
    }

    fn match_inferable_parameter_internal(&self, ty: Type) -> Option<TypeParameter> {
        match ty.as_type_parameter_type() {
            Some(TypeParameterType {
                type_parameter,
                is_question_type: false,
                ..
            }) => Some(type_parameter),
            _ => None,
        }
    }

    fn match_iterable_type_internal(&self, ty: Type) -> Option<Type> {
        if let Some(p) = ty.as_primary_type()
            && !p.is_question_type
            && p.args.len() == 1
            && (p.name() == "Iterable" || p.name() == "List")
        {
            return Some(p.args[0]);
        }
        None
    }

    fn match_list_type(&self, ty: View) -> Option<View> {
        match ty.unwrap_type_view().as_primary_type() {
            Some(p) if !p.is_question_type && p.name() == "List" && p.args.len() == 1 => {
                Some(SharedTypeView::new(p.args[0]))
            }
            _ => None,
        }
    }

    fn match_map_type(&self, ty: View) -> Option<KeyValueTypes<View>> {
        match ty.unwrap_type_view().as_primary_type() {
            Some(p) if !p.is_question_type && p.name() == "Map" && p.args.len() == 2 => {
                Some(KeyValueTypes {
                    key_type: SharedTypeView::new(p.args[0]),
                    value_type: SharedTypeView::new(p.args[1]),
                })
            }
            _ => None,
        }
    }

    fn match_stream_type(&self, ty: View) -> Option<View> {
        match ty.unwrap_type_view().as_primary_type() {
            Some(p) if !p.is_question_type && p.args.len() == 1 && p.name() == "Stream" => {
                Some(SharedTypeView::new(p.args[0]))
            }
            _ => None,
        }
    }

    fn match_type_declaration_type_internal(
        &self,
        ty: Type,
    ) -> Option<TypeDeclarationMatchResult<Type, Name, Type>> {
        let p = ty.as_primary_type()?;
        let type_declaration_kind = if p.is_interface_type() {
            TypeDeclarationKind::InterfaceDeclaration
        } else if self.is_extension_type_internal(ty) {
            TypeDeclarationKind::ExtensionTypeDeclaration
        } else {
            return None;
        };
        Some(TypeDeclarationMatchResult {
            type_declaration_kind,
            type_declaration: p.name(),
            type_declaration_type: ty,
            type_arguments: p.args,
        })
    }

    fn match_type_parameter_bound_internal(&self, ty: Type) -> Option<Type> {
        match ty.as_type_parameter_type() {
            Some(TypeParameterType {
                promotion,
                type_parameter,
                is_question_type: false,
            }) => Some(promotion.unwrap_or_else(|| type_parameter.bound())),
            _ => None,
        }
    }

    fn normalize(&self, ty: View) -> View {
        let query = ty.unwrap_type_view().to_string();
        match self.normalize_results.borrow().get(&query) {
            Some(result) => SharedTypeView::new(*result),
            None => panic!("Unknown query: {query}"),
        }
    }

    fn record_type_internal(&self, positional: &[Type], named: &[(Name, Type)]) -> Type {
        let mut named_types: Vec<NamedType> = named
            .iter()
            .map(|(name, ty)| NamedType { name, ty: *ty })
            .collect();
        named_types.sort_by(|a, b| a.name.cmp(b.name));
        RecordType::new(positional.to_vec(), named_types)
    }

    fn stream_type_schema(&self, element_type_schema: SchemaView) -> SchemaView {
        SharedTypeSchemaView::new(PrimaryType::new(
            TypeRegistry::stream(),
            vec![element_type_schema.unwrap_type_schema_view()],
        ))
    }

    fn substitute_type_from_iterables(
        &self,
        type_to_substitute: Type,
        type_parameters: &[TypeParameter],
        types: &[Type],
    ) -> Type {
        let _ = (type_to_substitute, type_parameters, types);
        // TODO(paulberry): Implement substituteTypeFromIterables.
        unimplemented!("substituteTypeFromIterables")
    }

    fn type_to_schema(&self, ty: View) -> SchemaView {
        SharedTypeSchemaView::new(ty.unwrap_type_view())
    }

    fn lookup_member_type_internal(&self, ty: Type, lookup_name: Name) -> Option<Type> {
        let _ = (ty, lookup_name);
        // TODO(cstefantsova): implement lookupMemberTypeInternal
        unimplemented!("lookupMemberTypeInternal")
    }

    dartr_type_analyzer::type_analyzer_operations_mixin!();
}
