// Dart source: pkg/analysis_server/lib/src/services/correction/dart/replace_with_is_empty.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_map_literal.dart

//! Producers for collection idioms: `isEmpty` and map literals.

use dartr_ast::*;
use dartr_element::{Nullability, TypeKind};
use dartr_typesystem::type_ext::TypeExt;

use super::super::change_builder::ChangeBuilder;
use super::super::dart_edit::WriteType;
use super::super::fix_kind::FixKind;
use super::super::generated::fix_kinds as k;
use super::super::producer::*;
use super::create::static_type;
use super::simple::producer;

/// Dart `ReplaceWithIsEmpty`: the kind and the replacement are computed
/// when the producer is created.
pub struct ReplaceWithIsEmpty {
    binary: Option<NodeId>,
    /// The getter (`isEmpty`/`isNotEmpty`) and the length target.
    replacement: Option<(&'static str, NodeId)>,
}

/// Dart `_getIntValue`.
fn int_value(c: &ProducerContext<'_>, e: NodeId) -> Option<i64> {
    let ast = c.ast;
    if let Some(i) = ast.cast::<IntegerLiteral>(e) {
        return ast[i].value;
    }
    if let Some(p) = ast.cast::<PrefixExpression>(e) {
        if c.lexeme(ast[p].operator) == "-" {
            if let Some(i) = ast.cast::<IntegerLiteral>(ast[p].operand) {
                return ast[i].value.map(|v| -v);
            }
        }
    }
    None
}

/// Dart `_getLengthTarget`.
fn length_target(c: &ProducerContext<'_>, e: NodeId) -> Option<NodeId> {
    let ast = c.ast;
    if let Some(p) = ast.cast::<PropertyAccess>(e) {
        if c.lexeme(ast[ast[p].property_name].token) == "length" {
            return ast[p].target.map(|t| t.raw());
        }
    } else if let Some(p) = ast.cast::<PrefixedIdentifier>(e) {
        if c.lexeme(ast[ast[p].identifier].token) == "length" {
            return Some(ast[p].prefix.raw());
        }
    }
    None
}

impl ReplaceWithIsEmpty {
    pub fn new(c: &ProducerContext<'_>) -> Self {
        let binary = c
            .ast
            .this_or_ancestor_of_type::<BinaryExpression>(c.node)
            .map(|b| b.raw());
        let replacement = binary.and_then(|b| Self::analyze(c, b));
        ReplaceWithIsEmpty {
            binary,
            replacement,
        }
    }

    /// Dart `_analyzeBinaryExpression`.
    fn analyze(c: &ProducerContext<'_>, binary: NodeId) -> Option<(&'static str, NodeId)> {
        let ast = c.ast;
        let b = ast.cast::<BinaryExpression>(binary)?;
        let op = c.lexeme(ast[b].operator);
        let (left, right) = (ast[b].left_operand.raw(), ast[b].right_operand.raw());
        if let Some(v) = int_value(c, right) {
            let target = length_target(c, left)?;
            return match (v, op) {
                (0, "==" | "<=") => Some(("isEmpty", target)),
                (0, ">" | "!=") => Some(("isNotEmpty", target)),
                (1, ">=") => Some(("isNotEmpty", target)),
                (1, "<") => Some(("isEmpty", target)),
                _ => None,
            };
        }
        let v = int_value(c, left)?;
        let target = length_target(c, right)?;
        match (v, op) {
            (0, "==" | ">=") => Some(("isEmpty", target)),
            (0, "<" | "!=") => Some(("isNotEmpty", target)),
            (1, "<=") => Some(("isNotEmpty", target)),
            (1, ">") => Some(("isEmpty", target)),
            _ => None,
        }
    }
}

impl CorrectionProducer for ReplaceWithIsEmpty {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(match self.replacement {
            Some(("isNotEmpty", _)) => &k::REPLACE_WITH_IS_NOT_EMPTY,
            _ => &k::REPLACE_WITH_IS_EMPTY,
        })
    }

    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        Some(match self.replacement {
            Some(("isNotEmpty", _)) => &k::REPLACE_WITH_IS_NOT_EMPTY_MULTI,
            _ => &k::REPLACE_WITH_IS_EMPTY_MULTI,
        })
    }

    fn applicability(&self) -> Applicability {
        Applicability::Automatically
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let (Some(binary), Some((getter, target))) = (self.binary, self.replacement) else {
            return;
        };
        if let Some(ty) = static_type(c, target) {
            let nullable = match *c.ctx.ty(ty) {
                TypeKind::Interface { nullability, .. }
                | TypeKind::TypeParameter { nullability, .. } => {
                    nullability == Nullability::Question
                }
                _ => false,
            };
            if nullable {
                return;
            }
        }
        let text = c.utils.get_node_text(target);
        let range = c.range().node(binary);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(range.offset, range.length, &format!("{text}.{getter}"))
        });
    }
}

producer!(
    ConvertToMapLiteral,
    k::CONVERT_TO_MAP_LITERAL,
    Some(&k::CONVERT_TO_MAP_LITERAL_MULTI),
    Automatically,
    assist: Some(&crate::correction::generated::assist_kinds::CONVERT_TO_MAP_LITERAL),
    |c, builder| {
        let ast = c.ast;
        let ctx = c.ctx;
        let Some(creation) = ast.this_or_ancestor_of_type::<InstanceCreationExpression>(c.node)
        else {
            return;
        };
        let Some(ty) = static_type(c, creation.raw()) else {
            return;
        };
        let constructor_name = ast[creation].constructor_name;
        let argument_list = ast[creation].argument_list;
        let TypeKind::Interface { element, args, .. } = *ctx.ty(ty) else {
            return;
        };
        let is_map = ctx.is_element(element.raw(), "dart.core", "Map")
            || ctx.is_element(element.raw(), "dart.collection", "LinkedHashMap");
        if ast.offset(c.node) > ast.offset(argument_list)
            || ast[constructor_name].name.is_some()
            || !ast.list_raw(ast[argument_list].arguments).is_empty()
            || !is_map
        {
            return;
        }
        let constructor_type_arguments = ast[ast[constructor_name].type_].type_arguments;
        let mut static_type_arguments: Option<Vec<dartr_element::TypeId>> = None;
        if constructor_type_arguments.is_none() {
            let list = ast.this_or_ancestor_of_type::<VariableDeclarationList>(creation);
            if list.is_none_or(|l| ast[l].type_.is_none()) {
                let a = ctx.list(args).to_vec();
                let all_dynamic = a
                    .first()
                    .is_some_and(|t| matches!(ctx.ty(*t), TypeKind::Dynamic))
                    && a.last()
                        .is_some_and(|t| matches!(ctx.ty(*t), TypeKind::Dynamic));
                if !all_dynamic {
                    static_type_arguments = Some(a);
                }
            }
        }
        let type_arguments_text = constructor_type_arguments.map(|t| c.utils.get_node_text(t));
        let range = c.range().node(creation);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_replacement(range.offset, range.length, |e| {
                if let Some(text) = &type_arguments_text {
                    e.write(text);
                } else if let Some(types) = static_type_arguments.as_ref().filter(|t| !t.is_empty())
                {
                    e.write("<");
                    for (i, t) in types.iter().enumerate() {
                        if i > 0 {
                            e.write(", ");
                        }
                        e.write_type(Some(*t), &WriteType::default());
                    }
                    e.write(">");
                }
                e.write("{}");
            });
        });
    }
);
