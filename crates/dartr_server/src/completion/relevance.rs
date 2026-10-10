// Dart source: pkg/analysis_server/lib/src/services/completion/dart/feature_computer.dart
// Dart source: pkg/analysis_server/lib/src/services/completion/dart/relevance_computer.dart
// Dart source: pkg/analysis_server/lib/src/services/completion/dart/probability_range.dart
// Dart source: pkg/analyzer_plugin/lib/utilities/completion/relevance.dart

//! The relevance of the candidates: the features (Dart `FeatureComputer`),
//! the context type at the completion offset (`_ContextTypeVisitor`), and
//! the relevance of each candidate (`RelevanceComputer`).

use std::collections::HashMap;
use std::sync::OnceLock;

use dartr_ast::*;
use dartr_element::{Ctx, EId, ElemRef, ElementId, InterfaceElement, Nullability, Tag, TypeId, TypeKind};
use dartr_syntax::TokenType;
use dartr_typesystem::{TypeExt, TypeSystem, member};

use super::Request;
use super::candidate::{Candidate, Kind};
use super::elem;
use super::target::TokenExt;

/// Dart `maximumRelevance`.
pub const MAXIMUM_RELEVANCE: i32 = 1000;

/// Dart `sortTextMaxValue` (`9999`).
pub const MAXIMUM_RELEVANCE_SORT: i64 = 9999;

/// Dart `Relevance` constants.
pub mod relevance {
    pub const CALL_FUNCTION: i32 = 500;
    pub const CLOSURE: i32 = 900;
    pub const IMPORT: i32 = 1000;
    pub const IMPORT_DART_CORE: i32 = 1001;
    pub const LABEL: i32 = 1000;
    pub const LOAD_LIBRARY: i32 = 1000;
    pub const NAMED_ARGUMENT: i32 = 950;
    pub const OVERRIDE: i32 = 750;
    pub const REQUIRED_NAMED_ARGUMENT: i32 = 1250;
    pub const SUPER_FORMAL_PARAMETER: i32 = 1000;
}

type Table = HashMap<&'static str, HashMap<&'static str, (f64, f64)>>;

fn build(rows: &'static [(&'static str, &'static [(&'static str, f64, f64)])]) -> Table {
    rows.iter()
        .map(|(location, entries)| {
            (
                *location,
                entries.iter().map(|(k, l, u)| (*k, (*l, *u))).collect(),
            )
        })
        .collect()
}

fn element_kind_table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| build(super::relevance_tables::ELEMENT_KIND_RELEVANCE))
}

fn keyword_table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| build(super::relevance_tables::KEYWORD_RELEVANCE))
}

/// Dart `ProbabilityRange.middle`.
fn middle((lower, upper): (f64, f64)) -> f64 {
    (upper + lower) / 2.0
}

/// Dart `ProbabilityRange.conditionalProbability`.
fn conditional_probability(range: (f64, f64), probability: f64) -> f64 {
    middle(range) + ((range.1 - range.0) * probability / 2.0)
}

/// Dart `FeatureComputer.featureWeights` (`defaultFeatureWeights`).
const FEATURE_WEIGHTS: [f64; 10] = [1.0, 1.0, 0.5, 1.0, 1.0, 1.0, 1.0, 0.5, 1.0, 1.0];

/// The features of a candidate (the arguments of Dart `computeScore`).
#[derive(Default, Clone, Copy)]
pub struct Features {
    pub context_type: f64,
    pub element_kind: f64,
    pub has_deprecated: f64,
    pub is_constant: f64,
    pub is_no_such_method: f64,
    pub is_not_imported: f64,
    pub keyword: f64,
    pub starts_with_dollar: f64,
    pub super_matches: f64,
    pub local_variable_distance: f64,
}

/// Dart `weightedAverage` + `toRelevance`.
pub fn compute_score(f: Features) -> i32 {
    let values = [
        f.context_type,
        f.element_kind,
        f.has_deprecated,
        f.is_constant,
        f.is_no_such_method,
        f.is_not_imported,
        f.keyword,
        f.starts_with_dollar,
        f.super_matches,
        f.local_variable_distance,
    ];
    let mut total_value = 0.0;
    let mut total_weight = 0.0;
    for (v, w) in values.iter().zip(FEATURE_WEIGHTS) {
        total_weight += w;
        total_value += v * w;
    }
    let average = total_value / total_weight;
    let score = (average + 1.0) / 2.0;
    (score * MAXIMUM_RELEVANCE as f64).trunc() as i32
}

/// Dart `FeatureComputer.distanceToPercent`.
pub fn distance_to_percent(distance: i32) -> f64 {
    if distance < 0 {
        return 0.0;
    }
    0.9f64.powi(distance)
}

/// Dart `computeElementKind2`: the protocol element kind name.
pub fn element_kind_name(ctx: &Ctx<'_>, element: ElementId) -> &'static str {
    let mut element = element;
    match element.tag() {
        Tag::Library => return "PREFIX",
        Tag::Enum => return "ENUM",
        Tag::Mixin => return "MIXIN",
        Tag::Class => return "CLASS",
        Tag::Field if super::candidate::is_enum_constant(ctx, element) => return "ENUM_CONSTANT",
        Tag::Getter | Tag::Setter => {
            if let Some(v) = elem::accessor_variable(ctx, element) {
                element = v;
            }
        }
        _ => {}
    }
    match element.tag() {
        Tag::Constructor => "CONSTRUCTOR",
        Tag::Extension => "EXTENSION",
        Tag::Field => "FIELD",
        Tag::TopLevelFunction | Tag::LocalFunction => "FUNCTION",
        Tag::GenericFunctionType => "FUNCTION_TYPE_ALIAS",
        Tag::Label => "LABEL",
        Tag::LocalVariable
        | Tag::PatternVariable
        | Tag::BindPatternVariable
        | Tag::JoinPatternVariable => "LOCAL_VARIABLE",
        Tag::Method => "METHOD",
        Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => {
            "PARAMETER"
        }
        Tag::Prefix => "PREFIX",
        Tag::TopLevelVariable => "TOP_LEVEL_VARIABLE",
        Tag::TypeAlias => "TYPE_ALIAS",
        Tag::TypeParameter => "TYPE_PARAMETER",
        _ => "UNKNOWN",
    }
}

/// Dart `FeatureComputer`.
pub struct FeatureComputer<'r, 'a> {
    pub ctx: &'r Ctx<'a>,
}

impl FeatureComputer<'_, '_> {
    fn ts(&self) -> TypeSystem<'_> {
        TypeSystem::new(*self.ctx)
    }

    /// Dart `contextTypeFeature`.
    pub fn context_type_feature(&self, context: Option<TypeId>, element: Option<TypeId>) -> f64 {
        let (Some(context), Some(element)) = (context, element) else {
            return 0.0;
        };
        if element == context {
            1.0
        } else if self.ts().is_subtype_of(element, context) {
            0.40
        } else if self.ts().is_subtype_of(context, element) {
            0.02
        } else {
            0.13
        }
    }

    /// Dart `elementKindFeature`.
    pub fn element_kind_feature(
        &self,
        element: ElementId,
        location: Option<&str>,
        distance: Option<f64>,
    ) -> f64 {
        let Some(location) = location else {
            return 0.0;
        };
        let Some(table) = element_kind_table().get(location) else {
            return 0.0;
        };
        let Some(&range) = table.get(element_kind_name(self.ctx, element)) else {
            return 0.0;
        };
        match distance {
            None => middle(range),
            Some(d) => conditional_probability(range, d),
        }
    }

    /// Dart `hasDeprecatedFeature`.
    pub fn has_deprecated_feature(&self, element: ElementId) -> f64 {
        if elem::has_or_inherits_deprecated(self.ctx, element) {
            -1.0
        } else {
            0.0
        }
    }

    /// Dart `inheritanceDistanceFeature`.
    pub fn inheritance_distance_feature(
        &self,
        subclass: EId<InterfaceElement>,
        superclass: EId<InterfaceElement>,
    ) -> f64 {
        let mut visited = Vec::new();
        distance_to_percent(self.inheritance_distance(Some(subclass), superclass, &mut visited))
    }

    /// Dart `_inheritanceDistance`.
    fn inheritance_distance(
        &self,
        subclass: Option<EId<InterfaceElement>>,
        superclass: EId<InterfaceElement>,
        visited: &mut Vec<EId<InterfaceElement>>,
    ) -> i32 {
        let Some(subclass) = subclass else {
            return -1;
        };
        if subclass == superclass {
            return 0;
        }
        if visited.contains(&subclass) {
            return -1;
        }
        visited.push(subclass);
        let ctx = self.ctx;
        let supertype = ctx
            .element_supertype(subclass)
            .and_then(|t| ctx.interface_element(t));
        let mut min_depth = self.inheritance_distance(supertype, superclass, visited);
        let mut visit_types = |types: &[TypeId], min_depth: &mut i32| {
            for &t in types {
                let depth = self.inheritance_distance(ctx.interface_element(t), superclass, visited);
                if *min_depth < 0 || (depth >= 0 && depth < *min_depth) {
                    *min_depth = depth;
                }
            }
        };
        if subclass.raw().tag() == Tag::Mixin {
            let constraints = ctx.element_superclass_constraints(subclass).to_vec();
            visit_types(&constraints, &mut min_depth);
        }
        let mixins = ctx.element_mixins(subclass).to_vec();
        visit_types(&mixins, &mut min_depth);
        let interfaces = ctx.element_interfaces(subclass).to_vec();
        visit_types(&interfaces, &mut min_depth);
        visited.retain(|e| *e != subclass);
        if min_depth < 0 {
            return min_depth;
        }
        min_depth + 1
    }

    /// Dart `isConstantFeature`.
    pub fn is_constant_feature(&self, element: ElementId) -> f64 {
        let ctx = self.ctx;
        let constant = match element.tag() {
            Tag::Constructor => elem::is_const_constructor(ctx, element),
            Tag::Field => elem::is_static(ctx, element) && elem::is_const_variable(ctx, element),
            Tag::TopLevelVariable => elem::is_const_variable(ctx, element),
            Tag::Getter | Tag::Setter if elem::is_origin_variable(ctx, element) => {
                elem::accessor_variable(ctx, element).is_some_and(|v| {
                    (v.tag() == Tag::TopLevelVariable || elem::is_static(ctx, v))
                        && elem::is_const_variable(ctx, v)
                })
            }
            _ => false,
        };
        if constant { 1.0 } else { 0.0 }
    }

    /// Dart `isNoSuchMethodFeature`.
    pub fn is_no_such_method_feature(&self, containing: Option<&str>, proposed: &str) -> f64 {
        if Some(proposed) == containing {
            return 0.0;
        }
        if proposed == "noSuchMethod" { -1.0 } else { 0.0 }
    }

    /// Dart `keywordFeature`.
    pub fn keyword_feature(&self, keyword: &str, location: Option<&str>) -> f64 {
        let Some(location) = location else {
            return 0.0;
        };
        let Some(table) = keyword_table().get(location) else {
            return 0.0;
        };
        let mut range = table.get(keyword);
        if range.is_none() {
            if let Some(index) = keyword.find(|c: char| !c.is_ascii_lowercase()) {
                if index > 0 {
                    range = table.get(&keyword[..index]);
                }
            }
        }
        range.map(|r| r.1).unwrap_or(0.0)
    }

    /// Dart `startsWithDollarFeature`.
    pub fn starts_with_dollar_feature(&self, name: &str) -> f64 {
        if name.starts_with('$') { -1.0 } else { 0.0 }
    }

    /// Dart `superMatchesFeature`.
    pub fn super_matches_feature(&self, containing: Option<&str>, proposed: &str) -> f64 {
        match containing {
            None => 0.0,
            Some(c) => {
                if proposed == c {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Context type

/// Dart `FeatureComputer.computeContextType(node, offset)`.
pub fn compute_context_type(req: &ContextInput<'_, '_>, node: NodeId, offset: u32) -> Option<TypeId> {
    let visitor = ContextTypeVisitor { req, offset };
    let t = visitor.visit(node)?;
    if matches!(req.ctx.ty(t), TypeKind::Dynamic) {
        return None;
    }
    Some(TypeSystem::new(*req.ctx).resolve_to_bound(t))
}

/// The resolved unit that the context type visitor reads.
pub struct ContextInput<'r, 'a> {
    pub ctx: &'r Ctx<'a>,
    pub ast: &'r Ast,
    pub tables: &'r dartr_element::ResolutionTables,
}

struct ContextTypeVisitor<'v, 'r, 'a> {
    req: &'v ContextInput<'r, 'a>,
    offset: u32,
}

/// Dart `range.endStart(a, b).contains(offset)`.
fn end_start_contains(ast: &Ast, a: dartr_syntax::TokenId, b: dartr_syntax::TokenId, o: u32) -> bool {
    ast.t_end(a) <= o && o <= ast.t_offset(b)
}

impl ContextTypeVisitor<'_, '_, '_> {
    fn ast(&self) -> &Ast {
        self.req.ast
    }

    fn ctx(&self) -> &Ctx<'_> {
        self.req.ctx
    }

    fn static_type(&self, node: impl Into<NodeId>) -> Option<TypeId> {
        self.req.tables.static_type.get(node.into()).copied()
    }

    fn tp(&self) -> &dartr_element::TypeProvider {
        self.req.ctx.tp
    }

    fn visit_parent(&self, node: NodeId) -> Option<TypeId> {
        let parent = self.ast().parent(node)?;
        self.visit(parent)
    }

    /// Dart `contains(o)` of the extension: `offset <= o && o <= end`.
    fn contains(&self, node: NodeId) -> bool {
        self.ast().offset(node) <= self.offset && self.offset <= self.ast().end(node)
    }

    /// The type of `Argument.correspondingParameter`.
    fn corresponding_parameter_type(&self, argument: NodeId) -> Option<TypeId> {
        let tables = self.req.tables;
        let found = tables.param_element.get(argument).copied().or_else(|| {
            let named = self.ast().cast::<NamedArgument>(argument)?;
            tables
                .param_element
                .get(self.ast()[named].argument_expression.raw())
                .copied()
        })?;
        Some(member::type_(self.ctx(), found))
    }

    fn function_type_params(&self, t: TypeId) -> Option<Vec<dartr_element::FnParam>> {
        match self.ctx().ty(t) {
            TypeKind::Function(f) => Some(self.ctx().list(f.params).to_vec()),
            _ => None,
        }
    }

    /// Dart `ArgumentList.functionType` (extension in feature_computer).
    fn argument_list_function_type(&self, list: Id<ArgumentList>) -> Option<TypeId> {
        let ast = self.ast();
        let tables = self.req.tables;
        let parent = ast.parent(list)?;
        let is_function = |t: TypeId| matches!(self.ctx().ty(t), TypeKind::Function(_));
        if let Some(i) = ast.cast::<InstanceCreationExpression>(parent) {
            let e = *tables.element.get(ast[i].constructor_name.raw())?;
            return Some(member::type_(self.ctx(), e));
        }
        if ast.is::<MethodInvocation>(parent)
            || ast.is::<FunctionExpressionInvocation>(parent)
            || ast.is::<DotShorthandInvocation>(parent)
        {
            return tables.invoke_type.get(parent).copied().filter(|t| is_function(*t));
        }
        if ast.is::<DotShorthandConstructorInvocation>(parent) {
            let d = ast.cast::<DotShorthandConstructorInvocation>(parent)?;
            let e = tables
                .element
                .get(ast[d].constructor_name.raw())
                .or_else(|| tables.element.get(parent))
                .copied()?;
            if member::base_element(self.ctx(), e).tag() == Tag::Constructor {
                return Some(member::type_(self.ctx(), e));
            }
            return None;
        }
        if ast.is::<SuperConstructorInvocation>(parent) {
            let e = *tables.element.get(parent)?;
            if member::base_element(self.ctx(), e).tag() == Tag::Constructor {
                return Some(member::type_(self.ctx(), e));
            }
            return None;
        }
        if ast.is::<EnumConstantArguments>(parent) {
            let constant = ast.parent(parent)?;
            let e = *tables.element.get(constant)?;
            return Some(member::type_(self.ctx(), e));
        }
        if ast.is::<Annotation>(parent) {
            let e = *tables.element.get(parent)?;
            if super::super::element_locator::is_executable(member::base_element(self.ctx(), e)) {
                return Some(member::type_(self.ctx(), e));
            }
        }
        None
    }

    /// The context type of the body of a function (Dart
    /// `FunctionBody.bodyContext?.contextType`).
    fn body_context_type(&self, body: NodeId) -> Option<TypeId> {
        let ast = self.ast();
        let ctx = self.ctx();
        let parent = ast.parent(body)?;
        let imposed = if let Some(m) = ast.cast::<MethodDeclaration>(parent) {
            let e = super::declared_element(self.req, m.raw())?;
            let t = member::return_type(ctx, ElemRef::Base(e));
            (!matches!(ctx.ty(t), TypeKind::Dynamic)).then_some(t)
        } else if ast.is::<FunctionExpression>(parent) {
            let declaration = ast.parent(parent).filter(|p| ast.is::<FunctionDeclaration>(*p));
            match declaration {
                Some(d) => {
                    let e = super::declared_element(self.req, d)?;
                    let t = member::return_type(ctx, ElemRef::Base(e));
                    (!matches!(ctx.ty(t), TypeKind::Dynamic)).then_some(t)
                }
                None => {
                    let context = compute_context_type(self.req, ast.parent(parent)?, ast.offset(parent));
                    match context.map(|c| ctx.ty(c)) {
                        Some(TypeKind::Function(f)) => {
                            let r = f.ret;
                            (!matches!(ctx.ty(r), TypeKind::Dynamic | TypeKind::Unknown))
                                .then_some(r)
                        }
                        _ => None,
                    }
                }
            }
        } else if ast.is::<ConstructorDeclaration>(parent) {
            None
        } else {
            None
        };
        let is_async = dartr_resolver::ast_ext::function_body_is_asynchronous(ast, body);
        let is_generator = dartr_resolver::ast_ext::function_body_is_generator(ast, body);
        let ts = TypeSystem::new(*ctx);
        dartr_resolver::body_inference_context::BodyInferenceContext::new(
            &ts,
            is_async,
            is_generator,
            imposed,
        )
        .context_type
    }

    fn visit(&self, node: NodeId) -> Option<TypeId> {
        let ast = self.ast();
        let offset = self.offset;
        let tp = self.tp();
        match ast.kind(node) {
            NodeKind::AdjacentStrings => {
                if offset == ast.offset(node) {
                    return self.visit_parent(node);
                }
                Some(tp.string_type())
            }
            NodeKind::ArgumentList => {
                let n = ast.cast::<ArgumentList>(node)?;
                if !end_start_contains(ast, ast[n].left_parenthesis, ast[n].right_parenthesis, offset)
                {
                    return None;
                }
                let function_type = self.argument_list_function_type(n)?;
                let parameters = self.function_type_params(function_type)?;
                let mut index = 0;
                let type_of_index = |index: usize| -> Option<TypeId> {
                    let p = parameters.get(index)?;
                    (!p.kind.is_named()).then_some(p.ty)
                };
                let mut previous: Option<NodeId> = None;
                for &argument in ast.list_raw(ast[n].arguments) {
                    if let Some(named) = ast.cast::<NamedArgument>(argument) {
                        if offset <= ast.offset(argument) {
                            return type_of_index(index);
                        }
                        if self.contains(argument) {
                            if offset >= ast.t_end(ast[named].colon) {
                                return self.corresponding_parameter_type(argument);
                            }
                            return None;
                        }
                    } else {
                        if previous.is_none_or(|p| ast.end(p) < offset) && offset <= ast.end(argument)
                        {
                            return self.corresponding_parameter_type(argument);
                        }
                        previous = Some(argument);
                        index += 1;
                    }
                }
                type_of_index(index)
            }
            NodeKind::AsExpression => {
                let n = ast.cast::<AsExpression>(node)?;
                if ast.t_end(ast[n].as_operator) < offset {
                    return self.static_type(ast[n].expression);
                }
                None
            }
            NodeKind::AssertInitializer => {
                let n = ast.cast::<AssertInitializer>(node)?;
                let end = match ast[n].message {
                    Some(m) => ast.t_prev(ast.begin(m.raw()))?,
                    None => ast[n].right_parenthesis,
                };
                end_start_contains(ast, ast[n].left_parenthesis, end, offset)
                    .then(|| tp.bool_type())
            }
            NodeKind::AssertStatement => {
                let n = ast.cast::<AssertStatement>(node)?;
                let end = match ast[n].message {
                    Some(m) => ast.t_prev(ast.begin(m.raw()))?,
                    None => ast[n].right_parenthesis,
                };
                end_start_contains(ast, ast[n].left_parenthesis, end, offset)
                    .then(|| tp.bool_type())
            }
            NodeKind::AssignmentExpression => {
                let n = ast.cast::<AssignmentExpression>(node)?;
                let op = ast[n].operator;
                if ast.t_end(op) <= offset {
                    let ty = ast.t_ty(op);
                    if ty == TokenType::EQ || ty == TokenType::QUESTION_QUESTION_EQ {
                        return self.req.tables.write_type.get(node).copied();
                    }
                    let method = *self.req.tables.element.get(node)?;
                    let parameters = member::formal_parameters(self.ctx(), method);
                    return parameters.first().map(|p| member::type_(self.ctx(), *p));
                }
                None
            }
            NodeKind::AwaitExpression => self.visit_parent(node),
            NodeKind::BinaryExpression => {
                let n = ast.cast::<BinaryExpression>(node)?;
                let op = ast[n].operator;
                if ast.t_end(op) <= offset {
                    let ty = ast.t_ty(op);
                    if ty == TokenType::EQ_EQ
                        || ty == TokenType::BANG_EQ
                        || ty == TokenType::QUESTION_QUESTION
                    {
                        let right = ast[n].right_operand.raw();
                        if ast.is::<DotShorthandInvocation>(right)
                            || ast.is::<DotShorthandPropertyAccess>(right)
                            || ast.is::<DotShorthandConstructorInvocation>(right)
                        {
                            return self.static_type(ast[n].left_operand);
                        }
                    }
                    return self.corresponding_parameter_type(ast[n].right_operand.raw());
                }
                self.visit_parent(node)
            }
            NodeKind::CascadeExpression => {
                let n = ast.cast::<CascadeExpression>(node)?;
                if offset == ast.offset(ast[n].target) {
                    return self.visit_parent(node);
                }
                None
            }
            NodeKind::ConditionalExpression => {
                let n = ast.cast::<ConditionalExpression>(node)?;
                if offset <= ast.t_offset(ast[n].question) {
                    Some(tp.bool_type())
                } else {
                    self.visit_parent(node)
                }
            }
            NodeKind::ConstantPattern
            | NodeKind::ConstructorName
            | NodeKind::ConstructorReference
            | NodeKind::DotShorthandConstructorInvocation
            | NodeKind::DotShorthandInvocation
            | NodeKind::DotShorthandPropertyAccess
            | NodeKind::InstanceCreationExpression
            | NodeKind::LogicalAndPattern
            | NodeKind::LogicalOrPattern
            | NodeKind::PrefixedIdentifier
            | NodeKind::PropertyAccess
            | NodeKind::SimpleIdentifier => self.visit_parent(node),
            NodeKind::ConstructorFieldInitializer => {
                let n = ast.cast::<ConstructorFieldInitializer>(node)?;
                if ast.t_end(ast[n].equals) <= offset {
                    let e = *self.req.tables.element.get(ast[n].field_name.raw())?;
                    if member::base_element(self.ctx(), e).tag() == Tag::Field {
                        return Some(member::type_(self.ctx(), e));
                    }
                }
                None
            }
            NodeKind::DoStatement => {
                let n = ast.cast::<DoStatement>(node)?;
                end_start_contains(ast, ast[n].left_parenthesis, ast[n].right_parenthesis, offset)
                    .then(|| tp.bool_type())
            }
            NodeKind::ExpressionFunctionBody => {
                let n = ast.cast::<ExpressionFunctionBody>(node)?;
                if ast.t_end(ast[n].function_definition) <= offset && offset <= ast.end(node) {
                    let parent = ast.parent(node)?;
                    if ast.is::<MethodDeclaration>(parent) {
                        return self.body_context_type(node);
                    } else if ast.is::<FunctionExpression>(parent) {
                        let context = self.body_context_type(node);
                        if let Some(c) = context {
                            if !matches!(self.ctx().ty(c), TypeKind::Invalid) {
                                return Some(c);
                            }
                        }
                        if let Some(d) = ast
                            .parent(parent)
                            .and_then(|p| ast.cast::<FunctionDeclaration>(p))
                        {
                            return ast[d]
                                .return_type
                                .and_then(|r| self.req.tables.annotation_type.get(r.raw()).copied());
                        }
                        return self.visit_parent(parent);
                    }
                }
                None
            }
            NodeKind::FieldDeclaration => {
                let n = ast.cast::<FieldDeclaration>(node)?;
                let fields = ast[n].fields.raw();
                if self.contains(fields) {
                    return self.visit(fields);
                }
                None
            }
            NodeKind::ForEachPartsWithDeclaration
            | NodeKind::ForEachPartsWithIdentifier
            | NodeKind::ForEachPartsWithPattern => {
                let in_keyword = if let Some(p) = ast.cast::<ForEachPartsWithDeclaration>(node) {
                    ast[p].in_keyword
                } else if let Some(p) = ast.cast::<ForEachPartsWithIdentifier>(node) {
                    ast[p].in_keyword
                } else {
                    ast[ast.cast::<ForEachPartsWithPattern>(node)?].in_keyword
                };
                if ast.t_end(in_keyword) <= offset && offset <= ast.end(node) {
                    if ast.kind(node) != NodeKind::ForEachPartsWithPattern {
                        let parent = ast.parent(node)?;
                        let is_await = ast
                            .cast::<ForStatement>(parent)
                            .is_some_and(|f| ast[f].await_keyword.is_some())
                            || ast
                                .cast::<ForElement>(parent)
                                .is_some_and(|f| ast[f].await_keyword.is_some());
                        if is_await {
                            return Some(tp.stream_dynamic_type());
                        }
                    }
                    return Some(tp.iterable_dynamic_type());
                }
                None
            }
            NodeKind::ForElement => {
                let n = ast.cast::<ForElement>(node)?;
                if end_start_contains(ast, ast[n].left_parenthesis, ast[n].right_parenthesis, offset)
                {
                    return self.visit(ast[n].for_loop_parts.raw());
                }
                self.visit_parent(node)
            }
            NodeKind::FormalParameterDefaultClause => {
                let n = ast.cast::<FormalParameterDefaultClause>(node)?;
                if ast.t_end(ast[n].separator) <= offset {
                    let parent = ast.parent(node)?;
                    if ast.is::<FormalParameter>(parent) {
                        let e = super::declared_element(self.req, parent)?;
                        return Some(member::type_(self.ctx(), ElemRef::Base(e)));
                    }
                }
                None
            }
            NodeKind::ForPartsWithDeclarations => {
                let n = ast.cast::<ForPartsWithDeclarations>(node)?;
                end_start_contains(ast, ast[n].left_separator, ast[n].right_separator, offset)
                    .then(|| tp.bool_type())
            }
            NodeKind::ForPartsWithExpression => {
                let n = ast.cast::<ForPartsWithExpression>(node)?;
                end_start_contains(ast, ast[n].left_separator, ast[n].right_separator, offset)
                    .then(|| tp.bool_type())
            }
            NodeKind::ForPartsWithPattern => {
                let n = ast.cast::<ForPartsWithPattern>(node)?;
                end_start_contains(ast, ast[n].left_separator, ast[n].right_separator, offset)
                    .then(|| tp.bool_type())
            }
            NodeKind::FunctionExpressionInvocation => {
                let n = ast.cast::<FunctionExpressionInvocation>(node)?;
                if self.contains(ast[n].function.raw()) {
                    return self.visit_parent(node);
                }
                None
            }
            NodeKind::GuardedPattern => {
                let n = ast.cast::<GuardedPattern>(node)?;
                let parent = ast.parent(node)?;
                let switch_expression = if let Some(c) = ast.cast::<SwitchExpressionCase>(parent) {
                    let _ = c;
                    ast.parent(parent)
                        .and_then(|s| ast.cast::<SwitchExpression>(s))
                        .map(|s| ast[s].expression.raw())
                } else if ast.is::<SwitchPatternCase>(parent) {
                    ast.parent(parent)
                        .and_then(|s| ast.cast::<SwitchStatement>(s))
                        .map(|s| ast[s].expression.raw())
                } else {
                    None
                };
                if let Some(expression) = switch_expression {
                    if let Some(when) = ast[n].when_clause {
                        if ast.offset(when) <= offset && offset <= ast.end(node) {
                            return Some(tp.bool_type());
                        }
                    }
                    return self.static_type(expression);
                }
                if ast.is::<CaseClause>(parent) {
                    if let Some(i) = ast.parent(parent).and_then(|p| ast.cast::<IfStatement>(p)) {
                        return self.static_type(ast[i].expression);
                    }
                }
                None
            }
            NodeKind::IfElement => {
                let n = ast.cast::<IfElement>(node)?;
                if end_start_contains(ast, ast[n].left_parenthesis, ast[n].right_parenthesis, offset)
                {
                    return Some(tp.bool_type());
                }
                self.visit_parent(node)
            }
            NodeKind::IfStatement => {
                let n = ast.cast::<IfStatement>(node)?;
                end_start_contains(ast, ast[n].left_parenthesis, ast[n].right_parenthesis, offset)
                    .then(|| tp.bool_type())
            }
            NodeKind::IndexExpression => {
                let n = ast.cast::<IndexExpression>(node)?;
                if end_start_contains(ast, ast[n].left_bracket, ast[n].right_bracket, offset) {
                    let e = *self.req.tables.element.get(node)?;
                    let parameters = member::formal_parameters(self.ctx(), e);
                    return parameters.first().map(|p| member::type_(self.ctx(), *p));
                }
                None
            }
            NodeKind::IsExpression => {
                let n = ast.cast::<IsExpression>(node)?;
                if ast.t_end(ast[n].is_operator) < offset {
                    return self.static_type(ast[n].expression);
                }
                None
            }
            NodeKind::Label => {
                let n = ast.cast::<Label>(node)?;
                if offset == ast.offset(node) || ast.t_end(ast[n].colon) <= offset {
                    return self.visit_parent(node);
                }
                None
            }
            NodeKind::ListLiteral => {
                let n = ast.cast::<ListLiteral>(node)?;
                if end_start_contains(ast, ast[n].left_bracket, ast[n].right_bracket, offset) {
                    let t = self.static_type(node)?;
                    return self.ctx().type_arguments(t).first().copied();
                }
                None
            }
            NodeKind::ListPattern => {
                let n = ast.cast::<ListPattern>(node)?;
                if end_start_contains(ast, ast[n].left_bracket, ast[n].right_bracket, offset) {
                    let t = self.required_type(node)?;
                    return self.ctx().type_arguments(t).first().copied();
                }
                None
            }
            NodeKind::MapLiteralEntry => {
                let n = ast.cast::<MapLiteralEntry>(node)?;
                let literal = ast.this_or_ancestor_of_type::<SetOrMapLiteral>(node)?;
                let t = self.static_type(literal)?;
                if self.ctx().is_dart_core_map(t) {
                    let args = self.ctx().type_arguments(t);
                    if offset <= ast.t_offset(ast[n].separator) {
                        return args.first().copied();
                    }
                    return args.get(1).copied();
                }
                None
            }
            NodeKind::MapPattern => {
                let n = ast.cast::<MapPattern>(node)?;
                if end_start_contains(ast, ast[n].left_bracket, ast[n].right_bracket, offset) {
                    let t = self.required_type(node)?;
                    if self.ctx().is_dart_core_map(t) {
                        return self.ctx().type_arguments(t).first().copied();
                    }
                }
                None
            }
            NodeKind::MapPatternEntry => {
                let n = ast.cast::<MapPatternEntry>(node)?;
                let parent = ast.parent(node).filter(|p| ast.is::<MapPattern>(*p))?;
                let t = self.required_type(parent)?;
                if self.ctx().is_dart_core_map(t) {
                    let args = self.ctx().type_arguments(t);
                    if offset <= ast.t_offset(ast[n].separator) {
                        return args.first().copied();
                    }
                    return args.get(1).copied();
                }
                None
            }
            NodeKind::MethodInvocation => {
                if offset == ast.offset(node) {
                    return self.visit_parent(node);
                }
                None
            }
            NodeKind::NamedArgument => {
                let n = ast.cast::<NamedArgument>(node)?;
                if offset == ast.offset(node) || offset >= ast.t_end(ast[n].colon) {
                    return self.visit_parent(node);
                }
                None
            }
            NodeKind::ParenthesizedExpression | NodeKind::ParenthesizedPattern => {
                let t = self.visit_parent(node)?;
                if let TypeKind::Record { positional, .. } = self.ctx().ty(t) {
                    return self.ctx().list(*positional).first().copied();
                }
                Some(t)
            }
            NodeKind::PatternAssignment => {
                let n = ast.cast::<PatternAssignment>(node)?;
                if offset >= ast.t_end(ast[n].equals) {
                    return self.required_type_of_pattern(ast[n].pattern.raw());
                }
                None
            }
            NodeKind::PatternField => {
                let parent = ast.parent(node)?;
                if let Some(o) = ast.cast::<ObjectPattern>(parent) {
                    return self.field_in_object_pattern(o, node);
                } else if ast.is::<RecordPattern>(parent) {
                    return self.field_in_record_pattern(parent, node);
                }
                None
            }
            NodeKind::PatternVariableDeclaration => {
                let n = ast.cast::<PatternVariableDeclaration>(node)?;
                if offset >= ast.t_end(ast[n].equals) {
                    return self.required_type_of_pattern(ast[n].pattern.raw());
                }
                None
            }
            NodeKind::PostfixExpression => {
                let n = ast.cast::<PostfixExpression>(node)?;
                self.corresponding_parameter_type(ast[n].operand.raw())
            }
            NodeKind::PrefixExpression => {
                let n = ast.cast::<PrefixExpression>(node)?;
                self.corresponding_parameter_type(ast[n].operand.raw())
            }
            NodeKind::RecordLiteral => {
                let n = ast.cast::<RecordLiteral>(node)?;
                let t = self.visit_parent(node)?;
                let TypeKind::Record {
                    positional, named, ..
                } = *self.ctx().ty(t)
                else {
                    return None;
                };
                let positional = self.ctx().list(positional);
                let named = self.ctx().list(named);
                let mut index = 0;
                for &field in ast.list_raw(ast[n].fields) {
                    if let Some(f) = ast.cast::<RecordLiteralNamedField>(field) {
                        if offset <= ast.offset(field) {
                            return positional.get(index).copied();
                        }
                        if self.contains(field) {
                            if offset >= ast.t_end(ast[f].colon) {
                                let name = ast.t_lexeme(ast[f].name);
                                return named
                                    .iter()
                                    .find(|n| self.ctx().name_str(n.name) == name)
                                    .map(|n| n.ty);
                            }
                            return None;
                        }
                    } else {
                        if offset <= ast.end(field) {
                            return positional.get(index).copied();
                        }
                        index += 1;
                    }
                }
                positional.get(index).copied()
            }
            NodeKind::RecordLiteralNamedField => {
                let n = ast.cast::<RecordLiteralNamedField>(node)?;
                if offset == ast.offset(node) || offset >= ast.t_end(ast[n].colon) {
                    return self.visit_parent(node);
                }
                None
            }
            NodeKind::RecordPattern => {
                let n = ast.cast::<RecordPattern>(node)?;
                if !end_start_contains(ast, ast[n].left_parenthesis, ast[n].right_parenthesis, offset)
                {
                    return None;
                }
                let t = self.matched_value_type(node)?;
                let TypeKind::Record { positional, .. } = *self.ctx().ty(t) else {
                    return None;
                };
                let index = self.positional_index(n);
                self.ctx().list(positional).get(index).copied()
            }
            NodeKind::ReturnStatement => {
                let n = ast.cast::<ReturnStatement>(node)?;
                if ast.t_end(ast[n].return_keyword) < offset {
                    let body = ast.this_or_ancestor_of_type::<FunctionBody>(node)?;
                    let invocation_parent = ast
                        .parent(body)
                        .and_then(|p| ast.parent(p))
                        .filter(|p| ast.is::<FunctionExpressionInvocation>(*p))
                        .and_then(|p| ast.parent(p));
                    return self
                        .body_context_type(body.raw())
                        .or_else(|| invocation_parent.and_then(|p| self.visit(p)));
                }
                None
            }
            NodeKind::SetOrMapLiteral => {
                let n = ast.cast::<SetOrMapLiteral>(node)?;
                let t = self.static_type(node)?;
                if end_start_contains(ast, ast[n].left_bracket, ast[n].right_bracket, offset)
                    && (self.ctx().is_dart_core_map(t) || self.ctx().is_dart_core_set(t))
                {
                    return self.ctx().type_arguments(t).first().copied();
                }
                None
            }
            NodeKind::SimpleStringLiteral => None,
            NodeKind::SpreadElement => {
                let n = ast.cast::<SpreadElement>(node)?;
                if ast.t_end(ast[n].spread_operator) <= offset {
                    let mut current = ast.parent(node);
                    while let Some(c) = current {
                        if ast.is::<ListLiteral>(c) {
                            return Some(tp.iterable_dynamic_type());
                        } else if let Some(s) = ast.cast::<SetOrMapLiteral>(c) {
                            let is_set = self
                                .static_type(s)
                                .is_some_and(|t| self.ctx().is_dart_core_set(t));
                            if is_set {
                                return Some(tp.iterable_dynamic_type());
                            }
                            let d = tp.dynamic_type();
                            return Some(tp.map_type(self.ctx(), d, d));
                        }
                        current = ast.parent(c);
                    }
                }
                None
            }
            NodeKind::SwitchCase => {
                let n = ast.cast::<SwitchCase>(node)?;
                if end_start_contains(ast, ast[n].keyword, ast[n].colon, offset) {
                    let parent = ast.parent(node)?;
                    if let Some(s) = ast.cast::<SwitchStatement>(parent) {
                        return self.static_type(ast[s].expression);
                    }
                }
                None
            }
            NodeKind::SwitchExpressionCase => {
                let switch = ast.parent(node)?;
                let parent = ast.parent(switch)?;
                self.visit(parent)
            }
            NodeKind::TopLevelVariableDeclaration => {
                let n = ast.cast::<TopLevelVariableDeclaration>(node)?;
                let variables = ast[n].variables.raw();
                if self.contains(variables) {
                    return self.visit(variables);
                }
                None
            }
            NodeKind::VariableDeclaration => {
                let n = ast.cast::<VariableDeclaration>(node)?;
                if let Some(equals) = ast[n].equals {
                    if ast.t_end(equals) <= offset {
                        let parent = ast.parent(node)?;
                        let list = ast.cast::<VariableDeclarationList>(parent)?;
                        return ast[list]
                            .type_
                            .and_then(|t| self.req.tables.annotation_type.get(t.raw()).copied())
                            .or_else(|| self.implied_type(ast.t_lexeme(ast[n].name)));
                    }
                }
                None
            }
            NodeKind::VariableDeclarationList => {
                let n = ast.cast::<VariableDeclarationList>(node)?;
                for &v in ast.list(ast[n].variables) {
                    if self.contains(v.raw()) {
                        if let Some(equals) = ast[v].equals {
                            if ast.t_end(equals) <= offset {
                                return ast[n]
                                    .type_
                                    .and_then(|t| {
                                        self.req.tables.annotation_type.get(t.raw()).copied()
                                    })
                                    .or_else(|| self.implied_type(ast.t_lexeme(ast[v].name)));
                            }
                        }
                    }
                }
                None
            }
            NodeKind::WhenClause => Some(tp.bool_type()),
            NodeKind::WhileStatement => {
                let n = ast.cast::<WhileStatement>(node)?;
                end_start_contains(ast, ast[n].left_parenthesis, ast[n].right_parenthesis, offset)
                    .then(|| tp.bool_type())
            }
            NodeKind::YieldStatement => {
                let n = ast.cast::<YieldStatement>(node)?;
                if end_start_contains(ast, ast[n].yield_keyword, ast[n].semicolon, offset) {
                    let body = ast.this_or_ancestor_of_type::<FunctionBody>(node)?;
                    return self.body_context_type(body.raw());
                }
                None
            }
            _ => None,
        }
    }

    /// Dart `_impliedDartTypeWithName`.
    fn implied_type(&self, name: &str) -> Option<TypeId> {
        let tp = self.tp();
        let ctx = self.ctx();
        if name.is_empty() {
            return None;
        }
        if ["i", "j", "index", "length"].contains(&name) {
            Some(tp.int_type())
        } else if ["height", "width"].contains(&name) {
            Some(tp.num_type())
        } else if ["list", "items"].contains(&name) {
            Some(tp.list_type(ctx, tp.dynamic_type()))
        } else if ["key", "text", "url", "uri", "name", "str", "string"].contains(&name) {
            Some(tp.string_type())
        } else if name == "iterator" {
            Some(tp.iterable_dynamic_type())
        } else if name == "map" {
            Some(tp.map_type(ctx, tp.dynamic_type(), tp.dynamic_type()))
        } else {
            None
        }
    }

    /// Dart `DartPattern.requiredType` (list and map patterns).
    fn required_type(&self, pattern: NodeId) -> Option<TypeId> {
        self.req
            .tables
            .pattern_info
            .get(pattern)
            .and_then(|i| i.required_type)
    }

    /// Dart `DartPattern.matchedValueType`.
    fn matched_value_type(&self, pattern: NodeId) -> Option<TypeId> {
        self.req
            .tables
            .pattern_info
            .get(pattern)
            .and_then(|i| i.matched_value_type)
    }

    /// Dart `_requiredTypeOfPattern`.
    fn required_type_of_pattern(&self, pattern: NodeId) -> Option<TypeId> {
        let ast = self.ast();
        let mut p = pattern;
        while let Some(pp) = ast.cast::<ParenthesizedPattern>(p) {
            p = ast[pp].pattern.raw();
        }
        let element = if ast.is::<AssignedVariablePattern>(p) {
            self.req.tables.element.get(p).copied()
        } else if ast.is::<DeclaredVariablePattern>(p) {
            super::declared_element(self.req, p).map(ElemRef::Base)
        } else if ast.is::<ListPattern>(p) {
            return self.required_type(p);
        } else {
            None
        }?;
        let base = member::base_element(self.ctx(), element);
        if base.is::<dartr_element::VariableElement>() {
            return Some(member::type_(self.ctx(), element));
        }
        None
    }

    /// Dart `_computePositionalIndex`.
    fn positional_index(&self, node: Id<RecordPattern>) -> usize {
        let ast = self.ast();
        let fields = ast.list(ast[node].fields);
        if fields.is_empty() {
            return 0;
        }
        let mut index = 0;
        for &field in fields {
            let mut right = ast.end_tok(field.raw());
            let next = ast.t_next(right);
            if ast.t_ty(next) == TokenType::COMMA {
                right = next;
            }
            if self.offset <= ast.t_offset(right) {
                return index;
            }
            if ast[field].name.is_none() {
                index += 1;
            }
        }
        index
    }

    /// Dart `_visitFieldInObjectPattern`.
    fn field_in_object_pattern(&self, parent: Id<ObjectPattern>, field: NodeId) -> Option<TypeId> {
        let ast = self.ast();
        let field = ast.cast::<PatternField>(field)?;
        let field_name = ast[field].name?;
        if self.offset < ast.end(field_name) {
            return None;
        }
        let name = ast.t_lexeme(ast[field_name].name?);
        let t = self
            .req
            .tables
            .annotation_type
            .get(ast[parent].type_.raw())
            .copied()?;
        let element = self.ctx().interface_element(t)?;
        let library = self
            .req
            .tables
            .element
            .get(field.raw())
            .and_then(|e| elem::library_of(self.ctx(), member::base_element(self.ctx(), *e)));
        let im = dartr_typesystem::inheritance_manager3::InheritanceManager3::new(*self.ctx());
        let n = dartr_typesystem::inheritance_manager3::Name::new(self.ctx(), library, name);
        let m = im.get_member(element, n)?;
        let base = member::base_element(self.ctx(), m);
        if matches!(base.tag(), Tag::Getter | Tag::Method) {
            return Some(member::return_type(self.ctx(), m));
        }
        None
    }

    /// Dart `_visitFieldInRecordPattern`.
    fn field_in_record_pattern(&self, parent: NodeId, field: NodeId) -> Option<TypeId> {
        let ast = self.ast();
        let record = ast.cast::<RecordPattern>(parent)?;
        let t = self.matched_value_type(parent)?;
        let TypeKind::Record {
            positional, named, ..
        } = *self.ctx().ty(t)
        else {
            return None;
        };
        let field = ast.cast::<PatternField>(field)?;
        let Some(field_name) = ast[field].name else {
            let fields = ast.list(ast[record].fields);
            let index = fields.iter().position(|f| *f == field)?;
            let field_index = fields[..index]
                .iter()
                .filter(|f| ast[**f].name.is_none())
                .count();
            return self.ctx().list(positional).get(field_index).copied();
        };
        if self.offset < ast.end(field_name) {
            return None;
        }
        let name = ast.t_lexeme(ast[field_name].name?);
        self.ctx()
            .list(named)
            .iter()
            .find(|n| self.ctx().name_str(n.name) == name)
            .map(|n| n.ty)
    }
}

// ---------------------------------------------------------------------------
// Relevance

/// Dart `RelevanceComputer`.
pub struct RelevanceComputer<'q, 'r, 'a> {
    pub req: &'q Request<'r, 'a>,
    pub prefer_constants: bool,
    pub completion_location: Option<String>,
    target_prefix: String,
    target_prefix_lower: String,
    containing_member_name: Option<String>,
}

impl<'q, 'r, 'a> RelevanceComputer<'q, 'r, 'a> {
    pub fn new(req: &'q Request<'r, 'a>, target_prefix: &str) -> Self {
        // Dart `_containingMemberName`.
        let ast = req.ast;
        let containing_member_name = req
            .target
            .dot_target(ast)
            .filter(|t| ast.is::<SuperExpression>(*t))
            .and_then(|_| ast.this_or_ancestor_of_type::<MethodDeclaration>(req.target.containing_node))
            .map(|m| ast.t_lexeme(ast[m].name).to_string());
        RelevanceComputer {
            req,
            prefer_constants: false,
            completion_location: None,
            target_prefix: target_prefix.to_string(),
            target_prefix_lower: target_prefix.to_lowercase(),
            containing_member_name,
        }
    }

    fn fc(&self) -> FeatureComputer<'r, 'a> {
        FeatureComputer { ctx: self.req.ctx }
    }

    fn ctx(&self) -> &'r Ctx<'a> {
        self.req.ctx
    }

    fn location(&self) -> Option<&str> {
        self.completion_location.as_deref()
    }

    fn element_kind(&self, element: ElementId, distance: Option<f64>) -> f64 {
        self.fc().element_kind_feature(element, self.location(), distance)
    }

    fn is_constant(&self, element: ElementId) -> f64 {
        if self.prefer_constants {
            self.fc().is_constant_feature(element)
        } else {
            0.0
        }
    }

    fn containing(&self) -> Option<&str> {
        self.containing_member_name.as_deref()
    }

    /// Dart `instantiateInstanceElement(element, neverType)`.
    fn instantiate(&self, element: ElementId) -> Option<TypeId> {
        let interface = element.cast::<InterfaceElement>()?;
        let ctx = self.ctx();
        let count = ctx.interface_type_parameters(interface).len();
        let args = vec![TypeId::NEVER; count];
        Some(ctx.interface_type(interface, &args, Nullability::None))
    }

    /// Dart `_instantiateTypeAlias`.
    fn instantiate_type_alias(&self, element: ElementId) -> Option<TypeId> {
        let alias = element.cast::<dartr_element::TypeAliasElement>()?;
        let ctx = self.ctx();
        let count = ctx.get(alias).type_params.len();
        let args = vec![TypeId::NEVER; count];
        Some(ctx.instantiate_type_alias(alias, &args, Nullability::None))
    }

    /// Dart `computeTopLevelRelevance`.
    fn top_level(&self, element: ElementId, element_type: Option<TypeId>, not_imported: bool) -> i32 {
        let fc = self.fc();
        compute_score(Features {
            context_type: fc.context_type_feature(self.req.context_type, element_type),
            element_kind: self.element_kind(element, None),
            has_deprecated: fc.has_deprecated_feature(element),
            is_constant: self.is_constant(element),
            is_not_imported: if not_imported { -1.0 } else { 0.0 },
            ..Features::default()
        })
    }

    /// Dart `computeFieldElementRelevance`.
    fn field(&self, field: ElemRef, inheritance_distance: f64) -> i32 {
        let ctx = self.ctx();
        let base = member::base_element(ctx, field);
        let fc = self.fc();
        let name = super::candidate::display_name(ctx, base);
        compute_score(Features {
            context_type: fc
                .context_type_feature(self.req.context_type, Some(member::type_(ctx, field))),
            element_kind: self.element_kind(base, Some(inheritance_distance)),
            has_deprecated: fc.has_deprecated_feature(base),
            is_constant: self.is_constant(base),
            starts_with_dollar: fc.starts_with_dollar_feature(&name),
            super_matches: fc.super_matches_feature(self.containing(), &name),
            ..Features::default()
        })
    }

    /// Dart `MemberSuggestion.inheritanceDistance`.
    pub fn inheritance_distance(&self, candidate: &Candidate) -> f64 {
        let ctx = self.ctx();
        let Some(element) = candidate.element() else {
            return 0.0;
        };
        let base = member::base_element(ctx, element);
        if super::candidate::is_enum_constant(ctx, base) {
            return 0.0;
        }
        let Some(Some(referencing)) = candidate.referencing_interface() else {
            return 0.0;
        };
        let Some(declaring) = elem::enclosing(ctx, base).and_then(|e| e.cast::<InterfaceElement>())
        else {
            return 0.0;
        };
        let Some(referencing) = referencing.cast::<InterfaceElement>() else {
            return 0.0;
        };
        self.fc().inheritance_distance_feature(referencing, declaring)
    }

    /// Dart `_computeMethodRelevance`.
    fn method(&self, method: ElemRef, inheritance_distance: f64, not_imported: bool) -> i32 {
        let ctx = self.ctx();
        let base = member::base_element(ctx, method);
        let fc = self.fc();
        let name = super::candidate::display_name(ctx, base);
        compute_score(Features {
            context_type: fc.context_type_feature(
                self.req.context_type,
                Some(member::return_type(ctx, method)),
            ),
            element_kind: self.element_kind(base, Some(inheritance_distance)),
            has_deprecated: fc.has_deprecated_feature(base),
            is_constant: self.is_constant(base),
            is_no_such_method: fc.is_no_such_method_feature(self.containing(), &name),
            is_not_imported: if not_imported { -1.0 } else { 0.0 },
            starts_with_dollar: fc.starts_with_dollar_feature(&name),
            super_matches: fc.super_matches_feature(self.containing(), &name),
            ..Features::default()
        })
    }

    /// Dart `_getPropertyAccessorType`.
    fn accessor_type(&self, accessor: ElemRef) -> Option<TypeId> {
        let ctx = self.ctx();
        let base = member::base_element(ctx, accessor);
        if base.tag() == Tag::Getter {
            return Some(member::return_type(ctx, accessor));
        }
        member::formal_parameters(ctx, accessor)
            .first()
            .map(|p| member::type_(ctx, *p))
    }

    /// Dart `_computePropertyAccessorRelevance`.
    fn property_accessor(&self, accessor: ElemRef, inheritance_distance: f64, not_imported: bool) -> i32 {
        let ctx = self.ctx();
        let base = member::base_element(ctx, accessor);
        if elem::is_origin_variable(ctx, base) {
            if base.tag() == Tag::Getter {
                if let Some(variable) = member::variable(ctx, accessor) {
                    if member::base_element(ctx, variable).tag() == Tag::Field {
                        return self.field(variable, inheritance_distance);
                    }
                }
            }
            return 0;
        }
        let name = super::candidate::display_name(ctx, base);
        let fc = self.fc();
        compute_score(Features {
            context_type: fc.context_type_feature(self.req.context_type, self.accessor_type(accessor)),
            element_kind: self.element_kind(base, Some(inheritance_distance)),
            has_deprecated: fc.has_deprecated_feature(base),
            is_constant: self.is_constant(base),
            is_not_imported: if not_imported { -1.0 } else { 0.0 },
            starts_with_dollar: fc.starts_with_dollar_feature(&name),
            super_matches: fc.super_matches_feature(self.containing(), &name),
            ..Features::default()
        })
    }

    /// Dart `_computeTopLevelPropertyAccessorRelevance`.
    fn top_level_accessor(&self, accessor: ElementId, not_imported: bool) -> i32 {
        let ctx = self.ctx();
        if elem::is_origin_variable(ctx, accessor) {
            if accessor.tag() == Tag::Getter {
                if let Some(v) = elem::accessor_variable(ctx, accessor) {
                    if v.tag() == Tag::TopLevelVariable {
                        let t = member::type_(ctx, ElemRef::Base(v));
                        return self.top_level(v, Some(t), not_imported);
                    }
                }
            }
            return 0;
        }
        let name = super::candidate::display_name(ctx, accessor);
        let fc = self.fc();
        compute_score(Features {
            context_type: fc.context_type_feature(
                self.req.context_type,
                self.accessor_type(ElemRef::Base(accessor)),
            ),
            element_kind: self.element_kind(accessor, None),
            has_deprecated: fc.has_deprecated_feature(accessor),
            is_constant: self.is_constant(accessor),
            is_not_imported: if not_imported { -1.0 } else { 0.0 },
            starts_with_dollar: fc.starts_with_dollar_feature(&name),
            ..Features::default()
        })
    }

    /// Dart `computeRelevance`.
    pub fn compute_relevance(&self, candidate: &Candidate) -> i32 {
        let ctx = self.ctx();
        if !self.target_prefix_lower.is_empty() {
            let completion = candidate.completion(ctx);
            if completion == self.target_prefix {
                return MAXIMUM_RELEVANCE;
            }
            if completion.to_lowercase() == self.target_prefix_lower {
                return MAXIMUM_RELEVANCE - 1;
            }
        }
        let fc = self.fc();
        let not_imported = candidate.is_not_imported();
        match &candidate.kind {
            Kind::Constructor { element, .. } => {
                let base = member::base_element(ctx, *element);
                let class = elem::enclosing(ctx, base);
                let t = class.and_then(|c| self.instantiate(c));
                self.top_level(base, t, not_imported)
            }
            Kind::FunctionCall { .. } => relevance::CALL_FUNCTION,
            Kind::Method { element, .. } | Kind::SetState { element, .. } => {
                self.method(*element, self.inheritance_distance(candidate), not_imported)
            }
            Kind::Getter { element, .. } => {
                self.property_accessor(*element, self.inheritance_distance(candidate), not_imported)
            }
            Kind::Field { element, .. } => {
                let base = member::base_element(ctx, *element);
                if super::candidate::is_enum_constant(ctx, base) {
                    let t = member::type_(ctx, *element);
                    self.top_level(base, Some(t), false)
                } else {
                    self.field(*element, self.inheritance_distance(candidate))
                }
            }
            Kind::RecordField { field_type, .. } => compute_score(Features {
                context_type: fc.context_type_feature(self.req.context_type, Some(*field_type)),
                ..Features::default()
            }),
            Kind::Class(e) => self.top_level(*e, self.instantiate(*e), not_imported),
            Kind::Closure { .. } => relevance::CLOSURE,
            Kind::EnumConstant {
                element,
                include_enum_name,
            } => {
                if *include_enum_name {
                    let t = member::type_(ctx, ElemRef::Base(*element));
                    self.top_level(*element, Some(t), not_imported)
                } else {
                    self.field(ElemRef::Base(*element), 0.0)
                }
            }
            Kind::Enum(e) | Kind::ExtensionType(e) | Kind::Mixin(e) => {
                self.top_level(*e, self.instantiate(*e), not_imported)
            }
            Kind::Extension { element, .. } => {
                let t = element
                    .cast::<dartr_element::ExtensionElement>()
                    .and_then(|x| ctx.get(x).extended_type.get());
                self.top_level(*element, t, not_imported)
            }
            Kind::FormalParameter { element, distance } => {
                let t = member::type_(ctx, ElemRef::Base(*element));
                compute_score(Features {
                    context_type: fc.context_type_feature(self.req.context_type, Some(t)),
                    element_kind: self.element_kind(*element, None),
                    is_constant: self.is_constant(*element),
                    local_variable_distance: distance_to_percent(*distance),
                    ..Features::default()
                })
            }
            Kind::Identifier { .. } | Kind::Name(_) => 500,
            Kind::ImportPrefix { library, .. } => compute_score(Features {
                element_kind: self.element_kind(*library, None),
                ..Features::default()
            }),
            Kind::Keyword { completion, .. } => {
                let element_type = if completion == "null" {
                    Some(ctx.tp.null_type())
                } else if completion == "false" || completion == "true" {
                    Some(ctx.tp.bool_type())
                } else {
                    None
                };
                compute_score(Features {
                    context_type: fc.context_type_feature(self.req.context_type, element_type),
                    keyword: fc.keyword_feature(completion, self.location()),
                    ..Features::default()
                })
            }
            Kind::Label(_) => relevance::LABEL,
            Kind::LoadLibrary { .. } => relevance::LOAD_LIBRARY,
            Kind::LocalFunction { element, .. } => {
                let t = member::return_type(ctx, ElemRef::Base(*element));
                self.top_level(*element, Some(t), not_imported)
            }
            Kind::LocalVariable { element, distance } => {
                let t = member::type_(ctx, ElemRef::Base(*element));
                let d = distance_to_percent(*distance);
                compute_score(Features {
                    context_type: fc.context_type_feature(self.req.context_type, Some(t)),
                    element_kind: self.element_kind(*element, Some(d)),
                    is_constant: self.is_constant(*element),
                    local_variable_distance: d,
                    ..Features::default()
                })
            }
            Kind::NamedArgument { parameter, .. } => {
                let kind = elem::parameter_kind(ctx, *parameter);
                let base = member::base_element(ctx, *parameter);
                if kind.is_required_named() || elem::has_required(ctx, base) {
                    relevance::REQUIRED_NAMED_ARGUMENT
                } else {
                    relevance::NAMED_ARGUMENT
                }
            }
            Kind::Override { .. } => relevance::OVERRIDE,
            Kind::Setter { element, .. } => {
                self.property_accessor(*element, self.inheritance_distance(candidate), not_imported)
            }
            Kind::RecordLiteralNamedField { .. } => relevance::REQUIRED_NAMED_ARGUMENT,
            Kind::StaticField(e) => {
                if elem::is_origin_getter_setter(ctx, *e) {
                    if let Some(getter) = elem::variable_getter(ctx, *e) {
                        if let Some(v) = elem::accessor_variable(ctx, getter) {
                            if v.tag() == Tag::Field {
                                return self.field(ElemRef::Base(v), 0.0);
                            }
                        }
                    }
                    0
                } else {
                    let t = member::type_(ctx, ElemRef::Base(*e));
                    self.top_level(*e, Some(t), not_imported)
                }
            }
            Kind::SuperParameter(_) => relevance::SUPER_FORMAL_PARAMETER,
            Kind::TopLevelFunction { element, .. } => {
                let t = member::return_type(ctx, ElemRef::Base(*element));
                self.top_level(*element, Some(t), not_imported)
            }
            Kind::TopLevelGetter(e) | Kind::TopLevelSetter(e) => {
                self.top_level_accessor(*e, not_imported)
            }
            Kind::TopLevelVariable(e) => {
                let t = member::type_(ctx, ElemRef::Base(*e));
                self.top_level(*e, Some(t), not_imported)
            }
            Kind::TypeAlias(e) => self.top_level(*e, self.instantiate_type_alias(*e), not_imported),
            Kind::TypeParameter(e) => compute_score(Features {
                element_kind: self.element_kind(*e, None),
                is_constant: self.is_constant(*e),
                ..Features::default()
            }),
            Kind::Uri(uri) => {
                if uri == "dart:core" {
                    relevance::IMPORT_DART_CORE
                } else {
                    relevance::IMPORT
                }
            }
        }
    }
}
