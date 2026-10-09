// Dart source: pkg/analyzer/lib/src/dart/resolver/record_literal_resolver.dart

//! `RecordLiteralResolver`: resolves [RecordLiteral]s.

use dartr_ast::{Expression, Id, RecordLiteral, RecordLiteralField, RecordLiteralNamedField};
use dartr_diagnostics::{DiagnosticMessage, diag};
use dartr_element::{NamedType, Nullability, TypeId, TypeKind};
use dartr_typesystem::TypeExt;
use indexmap::IndexMap;

use crate::record_type_annotation_resolver::{
    is_forbidden_name_for_record_field, positional_field_index,
};
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitRecordLiteral(node, contextType: contextType)`:
/// `checkUnreachableNode(node)`, then `RecordLiteralResolver.resolve(node,
/// contextType:)`.
pub fn visit_record_literal(
    rv: &mut ResolverVisitor<'_>,
    node: Id<RecordLiteral>,
    context_type: TypeId,
) {
    rv.check_unreachable_node(node);
    resolve(rv, node, context_type);
}

/// Dart `resolve`.
fn resolve(rv: &mut ResolverVisitor<'_>, node: Id<RecordLiteral>, context_type: TypeId) {
    resolve_fields(rv, node, context_type);

    report_duplicate_field_definitions(rv, node);
    report_invalid_field_names(rv, node);
}

/// The named field of [field], if it is one (Dart `field is
/// RecordLiteralNamedField`).
fn named_field(
    rv: &ResolverVisitor<'_>,
    field: Id<RecordLiteralField>,
) -> Option<Id<RecordLiteralNamedField>> {
    rv.ast.cast::<RecordLiteralNamedField>(field.raw())
}

/// Dart `_matchContextType`: [context_type] when it is a record type with
/// the shape of [node] (up to the order of the named fields).
fn match_context_type(
    rv: &ResolverVisitor<'_>,
    node: Id<RecordLiteral>,
    context_type: TypeId,
) -> Option<TypeId> {
    let TypeKind::Record {
        positional, named, ..
    } = *rv.ctx.ty(context_type)
    else {
        return None;
    };
    let positional = rv.ctx.list(positional);
    let named = rv.ctx.list(named);
    let fields = rv.ast.list(rv.ast[node].fields);
    if named.len() + positional.len() != fields.len() {
        return None;
    }
    let mut num_positional_fields = 0;
    for &field in fields {
        if let Some(named_field) = named_field(rv, field) {
            let name = rv.lexeme(rv.ast[named_field].name);
            if !named.iter().any(|f| rv.ctx.name_str(f.name) == name) {
                return None;
            }
        } else {
            num_positional_fields += 1;
        }
    }
    if positional.len() != num_positional_fields {
        return None;
    }
    Some(context_type)
}

/// Dart `_reportDuplicateFieldDefinitions`.
fn report_duplicate_field_definitions(rv: &mut ResolverVisitor<'_>, node: Id<RecordLiteral>) {
    let fields = rv.ast.list(rv.ast[node].fields).to_vec();
    let mut used_names: IndexMap<String, Id<RecordLiteralNamedField>> = IndexMap::new();
    for field in fields {
        let Some(field) = named_field(rv, field) else {
            continue;
        };
        let name = rv.lexeme(rv.ast[field].name).to_string();
        match used_names.get(&name) {
            Some(&previous_field) => {
                // Dart `DiagnosticFactory.duplicateFieldDefinitionInLiteral`.
                let source = rv.ctx.fragment(rv.unit.fragment).source.clone();
                let previous_name = rv.ast[previous_field].name;
                let d =
                    diag::duplicate_field_name(&name).with_context_messages([DiagnosticMessage {
                        file_path: source.path.to_string(),
                        offset: rv.ast.tokens.offset(previous_name) as i64,
                        length: name.encode_utf16().count() as i64,
                        message: "The first ".to_string(),
                        url: Some(source.uri.to_string()),
                    }]);
                let d = rv.at_token(d, rv.ast[field].name);
                rv.report(d);
            }
            None => {
                used_names.insert(name, field);
            }
        }
    }
}

/// Dart `_reportInvalidFieldNames`.
fn report_invalid_field_names(rv: &mut ResolverVisitor<'_>, node: Id<RecordLiteral>) {
    let fields = rv.ast.list(rv.ast[node].fields).to_vec();
    let positional_count = fields
        .iter()
        .filter(|&&f| named_field(rv, f).is_none())
        .count();
    for field in fields {
        let Some(field) = named_field(rv, field) else {
            continue;
        };
        let name_token = rv.ast[field].name;
        let name = rv.lexeme(name_token).to_string();
        if name.starts_with('_') {
            let d = rv.at_token(diag::invalid_field_name_private(), name_token);
            rv.report(d);
        } else if let Some(index) = positional_field_index(&name) {
            if index < positional_count {
                let d = rv.at_token(diag::invalid_field_name_positional(), name_token);
                rv.report(d);
            }
        } else if is_forbidden_name_for_record_field(&name) {
            let d = rv.at_token(diag::invalid_field_name_from_object(), name_token);
            rv.report(d);
        }
    }
}

/// Dart `_resolveField`: resolves the expression of [field] in
/// [context_type] and returns the type of the record field.
fn resolve_field(
    rv: &mut ResolverVisitor<'_>,
    field: Id<RecordLiteralField>,
    expression: Id<Expression>,
    context_type: TypeId,
) -> TypeId {
    let static_type = rv
        .analyze_expression_node(expression, context_type)
        .type_
        .unwrap_type_view();
    rv.pop_rewrite();

    // Implicit cast from `dynamic`.
    if context_type != TypeId::UNKNOWN && matches!(rv.ctx.ty(static_type), TypeKind::Dynamic) {
        let greatest_closure_of_schema = rv.type_system.greatest_closure_of_schema(context_type);
        if !rv
            .type_system
            .is_subtype_of(static_type, greatest_closure_of_schema)
        {
            return greatest_closure_of_schema;
        }
    }

    if matches!(rv.ctx.ty(static_type), TypeKind::Void) {
        let d = rv.at(diag::use_of_void_result(), field.raw());
        rv.report(d);
    }

    static_type
}

/// Dart `_resolveFields`.
fn resolve_fields(rv: &mut ResolverVisitor<'_>, node: Id<RecordLiteral>, context_type: TypeId) {
    let mut positional_fields: Vec<TypeId> = Vec::new();
    let mut named_fields: Vec<NamedType> = Vec::new();
    let context_type_as_record = match_context_type(rv, node, context_type);
    let (context_positional, context_named) = match context_type_as_record.map(|t| *rv.ctx.ty(t)) {
        Some(TypeKind::Record {
            positional, named, ..
        }) => (rv.ctx.list(positional), rv.ctx.list(named)),
        _ => (&[][..], &[][..]),
    };
    let mut index = 0;
    let fields = rv.ast.list(rv.ast[node].fields).to_vec();
    for field in fields {
        if let Some(named) = named_field(rv, field) {
            let name = rv.lexeme(rv.ast[named].name).to_string();
            let field_context_type = context_named
                .iter()
                .find(|f| rv.ctx.name_str(f.name) == name)
                .map(|f| f.ty)
                .unwrap_or(TypeId::UNKNOWN);
            let expression = rv.ast[named].field_expression;
            let ty = resolve_field(rv, field, expression, field_context_type);
            named_fields.push(NamedType {
                name: rv.ctx.name(&name),
                ty,
            });
        } else {
            let field_context_type = match context_positional.get(index) {
                Some(&t) if context_type_as_record.is_some() => {
                    index += 1;
                    t
                }
                _ => TypeId::UNKNOWN,
            };
            let expression: Id<Expression> = Id::from_raw(field.raw());
            let ty = resolve_field(rv, field, expression, field_context_type);
            positional_fields.push(ty);
        }
    }

    let record_type =
        rv.ctx
            .record_type(&positional_fields, &named_fields, Nullability::None, None);
    rv.record_static_type(node, record_type);
}
