// Dart source: pkg/analyzer/lib/src/dart/constant/evaluation.dart
// Dart source: pkg/analyzer/lib/src/dart/element/type_system.dart
//! Constant-value adapter for lint predicates. Unsupported evaluation returns
//! None; the caller must not infer a value from unresolved source text.
use crate::LinterContext;
use dartr_ast::*;
use dartr_constant::{
    BoolState, ConstTypeSystem, DartObjectImpl, DoubleState, InstanceState, IntState, NullState,
    StringState,
};
use dartr_element::{Ctx, EId, ElemRef, LibraryElement, TypeId};
use dartr_typesystem::{TypeSystem, lookup, member};

pub struct ConstantTypeSystem<'a>(pub Ctx<'a>);
impl ConstTypeSystem for ConstantTypeSystem<'_> {
    fn ctx(&self) -> Ctx<'_> {
        self.0
    }
    fn is_subtype_of(&self, a: TypeId, b: TypeId) -> bool {
        TypeSystem::new(self.0).is_subtype_of(a, b)
    }
    fn runtime_types_equal(&self, a: TypeId, b: TypeId) -> bool {
        TypeSystem::new(self.0).runtime_types_equal(a, b)
    }
    fn normalize(&self, t: TypeId) -> TypeId {
        TypeSystem::new(self.0).normalize(t)
    }
    fn types_equal(&self, a: TypeId, b: TypeId) -> bool {
        TypeSystem::new(self.0).dart_eq(a, b)
    }
    fn extension_type_erasure(&self, t: TypeId) -> TypeId {
        TypeSystem::new(self.0).extension_type_erasure(t)
    }
    fn display_string(&self, t: TypeId) -> String {
        dartr_element::display_string::type_display_string_with(&self.0, t, Default::default())
    }
    fn look_up_concrete_method(
        &self,
        t: TypeId,
        n: &str,
        l: EId<LibraryElement>,
    ) -> Option<ElemRef> {
        lookup::type_look_up_method(
            &self.0,
            t,
            n,
            l,
            lookup::LookUpOptions {
                concrete: true,
                ..Default::default()
            },
        )
    }
    fn look_up_concrete_getter(
        &self,
        t: TypeId,
        n: &str,
        l: EId<LibraryElement>,
    ) -> Option<ElemRef> {
        lookup::type_look_up_getter(
            &self.0,
            t,
            n,
            l,
            lookup::LookUpOptions {
                concrete: true,
                ..Default::default()
            },
        )
    }
}

pub fn constant_value(c: &LinterContext<'_>, node: NodeId) -> Option<DartObjectImpl> {
    evaluate(c, node, 0)
}
fn evaluate(c: &LinterContext<'_>, node: NodeId, depth: usize) -> Option<DartObjectImpl> {
    if depth >= 64 {
        return None;
    }
    let r = c.resolved?;
    let ts = ConstantTypeSystem(r.ctx);
    let make = |ty, state| Some(DartObjectImpl::new(&ts, ty, state));
    let recur = |n| evaluate(c, n, depth + 1);
    match c.ast.kind(node) {
        NodeKind::IntegerLiteral => make(
            r.ctx.tp.int_type(),
            InstanceState::Int(IntState::new(
                c.ast[Id::<IntegerLiteral>::from_raw(node)].value,
            )),
        ),
        NodeKind::DoubleLiteral => make(
            r.ctx.tp.double_type(),
            InstanceState::Double(DoubleState::new(Some(
                c.ast[Id::<DoubleLiteral>::from_raw(node)].value,
            ))),
        ),
        NodeKind::BooleanLiteral => make(
            r.ctx.tp.bool_type(),
            InstanceState::Bool(BoolState::new(Some(
                c.ast[Id::<BooleanLiteral>::from_raw(node)].value,
            ))),
        ),
        NodeKind::NullLiteral => make(
            r.ctx.tp.null_type(),
            InstanceState::Null(NullState::NULL_STATE),
        ),
        NodeKind::SimpleStringLiteral => make(
            r.ctx.tp.string_type(),
            InstanceState::String(StringState::new(
                c.ast[Id::<SimpleStringLiteral>::from_raw(node)]
                    .value
                    .clone(),
            )),
        ),
        NodeKind::AdjacentStrings => {
            let n = &c.ast[Id::<AdjacentStrings>::from_raw(node)];
            let mut value = String::new();
            for &s in c.ast.list(n.strings) {
                value.push_str(recur(s.raw())?.to_string_value()?);
            }
            make(
                r.ctx.tp.string_type(),
                InstanceState::String(StringState::new(value)),
            )
        }
        NodeKind::StringInterpolation => {
            let n = &c.ast[Id::<StringInterpolation>::from_raw(node)];
            let mut value = String::new();
            for &part in c.ast.list(n.elements) {
                if let Some(s) = c.ast.cast::<InterpolationString>(part) {
                    value.push_str(&c.ast[s].value);
                } else if let Some(e) = c.ast.cast::<InterpolationExpression>(part) {
                    let object = recur(c.ast[e].expression.raw())?;
                    if !object.is_bool_num_string_or_null() {
                        return None;
                    }
                    value.push_str(object.perform_to_string(&ts).ok()?.to_string_value()?);
                }
            }
            make(
                r.ctx.tp.string_type(),
                InstanceState::String(StringState::new(value)),
            )
        }
        NodeKind::ParenthesizedExpression => recur(
            c.ast[Id::<ParenthesizedExpression>::from_raw(node)]
                .expression
                .raw(),
        ),
        NodeKind::PrefixExpression => {
            let n = &c.ast[Id::<PrefixExpression>::from_raw(node)];
            let value = recur(n.operand.raw())?;
            match c.ast.tokens.lexeme(n.operator) {
                "-" => value.negated(&ts).ok(),
                "!" => value.logical_not(&ts).ok(),
                "~" => value.bit_not(&ts).ok(),
                _ => None,
            }
        }
        NodeKind::ConditionalExpression => {
            let n = &c.ast[Id::<ConditionalExpression>::from_raw(node)];
            recur(if recur(n.condition.raw())?.to_bool_value()? {
                n.then_expression.raw()
            } else {
                n.else_expression.raw()
            })
        }
        NodeKind::BinaryExpression => {
            let n = &c.ast[Id::<BinaryExpression>::from_raw(node)];
            let left = recur(n.left_operand.raw())?;
            let op = c.ast.tokens.lexeme(n.operator);
            if op == "??" {
                return if left.is_null() {
                    recur(n.right_operand.raw())
                } else {
                    Some(left)
                };
            }
            if op == "&&" && left.to_bool_value() == Some(false) {
                return Some(left);
            }
            if op == "||" && left.to_bool_value() == Some(true) {
                return Some(left);
            }
            let right = recur(n.right_operand.raw())?;
            match op {
                "+" => left.add(&ts, &right).ok(),
                "-" => left.minus(&ts, &right).ok(),
                "*" => left.times(&ts, &right).ok(),
                "/" => left.divide(&ts, &right).ok(),
                "~/" => left.integer_divide(&ts, &right).ok(),
                "%" => left.remainder(&ts, &right).ok(),
                "&" => left.eager_and(&ts, &right).ok(),
                "|" => left.eager_or(&ts, &right).ok(),
                "^" => left.eager_xor(&ts, &right).ok(),
                "<<" => left.shift_left(&ts, &right).ok(),
                ">>" => left.shift_right(&ts, &right).ok(),
                ">>>" => left.logical_shift_right(&ts, &right).ok(),
                "==" => left.equal_equal(&ts, r.ctx.features, &right).ok(),
                "!=" => left.not_equal(&ts, r.ctx.features, &right).ok(),
                "<" => left.less_than(&ts, &right).ok(),
                "<=" => left.less_than_or_equal(&ts, &right).ok(),
                ">" => left.greater_than(&ts, &right).ok(),
                ">=" => left.greater_than_or_equal(&ts, &right).ok(),
                "&&" => make(
                    r.ctx.tp.bool_type(),
                    InstanceState::Bool(BoolState::new(Some(
                        left.to_bool_value()? && right.to_bool_value()?,
                    ))),
                ),
                "||" => make(
                    r.ctx.tp.bool_type(),
                    InstanceState::Bool(BoolState::new(Some(
                        left.to_bool_value()? || right.to_bool_value()?,
                    ))),
                ),
                _ => None,
            }
        }
        NodeKind::SimpleIdentifier | NodeKind::PrefixedIdentifier | NodeKind::PropertyAccess => {
            let identifier = if let Some(p) = c.ast.cast::<PrefixedIdentifier>(node) {
                c.ast[p].identifier.raw()
            } else if let Some(p) = c.ast.cast::<PropertyAccess>(node) {
                c.ast[p].property_name.raw()
            } else {
                node
            };
            let element = member::base_element(&r.ctx, c.element(identifier)?);
            let variable = if let Some(g) = element.cast::<dartr_element::GetterElement>() {
                r.ctx.get(g).variable.get()?.raw()
            } else {
                element
            };
            let data = r.ctx.element_data(variable)?;
            if !r
                .ctx
                .fragment_data(data.first_fragment)?
                .flags
                .get()
                .contains(dartr_element::FragmentFlags::VARIABLE_FRAGMENT_IS_CONST)
            {
                return None;
            }
            let (unit, declaration) = declaration_context(c, variable)?;
            let n = unit.ast.cast::<VariableDeclaration>(declaration)?;
            evaluate(&unit, unit.ast[n].initializer?.raw(), depth + 1)
        }
        _ => None, // Constructors/collections await the shared wave-D evaluator.
    }
}

pub fn default_value(c: &LinterContext<'_>, parameter: ElemRef) -> Option<DartObjectImpl> {
    default_value_with_depth(c, parameter, 0)
}
fn default_value_with_depth(
    c: &LinterContext<'_>,
    parameter: ElemRef,
    depth: usize,
) -> Option<DartObjectImpl> {
    if depth >= 64 {
        return None;
    }
    let r = c.resolved?;
    let element = member::base_element(&r.ctx, parameter);
    let (unit, node) = declaration_context(c, element)?;
    let c = &unit;
    let clause = match c.ast.kind(node) {
        NodeKind::RegularFormalParameter => {
            c.ast[Id::<RegularFormalParameter>::from_raw(node)].default_clause
        }
        NodeKind::FieldFormalParameter => {
            c.ast[Id::<FieldFormalParameter>::from_raw(node)].default_clause
        }
        NodeKind::SuperFormalParameter => {
            c.ast[Id::<SuperFormalParameter>::from_raw(node)].default_clause
        }
        _ => None,
    };
    if let Some(clause) = clause {
        constant_value(c, c.ast[clause].value.raw())
    } else if element.tag() == dartr_element::Tag::SuperFormalParameter {
        let inherited =
            dartr_link::outline::super_constructor_parameter(&r.ctx, EId::from_raw(element))?;
        default_value_with_depth(c, ElemRef::Base(inherited.raw()), depth + 1)
    } else {
        Some(DartObjectImpl::new(
            &ConstantTypeSystem(r.ctx),
            r.ctx.tp.null_type(),
            InstanceState::Null(NullState::NULL_STATE),
        ))
    }
}

fn declaration_context<'a>(
    c: &LinterContext<'a>,
    element: dartr_element::ElementId,
) -> Option<(LinterContext<'a>, NodeId)> {
    for unit in std::iter::once(*c)
        .chain((0..c.resolved_units.len()).filter_map(|index| c.resolved_unit(index)))
    {
        if let Some(node) = (0..unit.ast.node_count())
            .map(NodeId::from_index)
            .find(|&node| unit.declared_element(node) == Some(element))
        {
            return Some((unit, node));
        }
    }
    None
}
