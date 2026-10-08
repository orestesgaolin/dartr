// Dart source: pkg/analyzer/lib/src/dart/resolver/record_type_annotation_resolver.dart
// Dart source: pkg/analyzer/lib/src/diagnostic/diagnostic_factory.dart
// (duplicateFieldDefinitionInType)
// Dart source: pkg/analyzer/lib/src/dart/element/extensions.dart
// (RecordTypeExtension.positionalFieldIndex)

//! `RecordTypeAnnotationResolver`: the type of a record type annotation
//! (`ResolutionTables.annotation_type`) and the diagnostics of its field
//! names.

use dartr_ast::{Ast, Id, NodeId, RecordTypeAnnotation, RecordTypeAnnotationPositionalField};
use dartr_diagnostics::{Diagnostic, DiagnosticMessage, diag};
use dartr_element::{Ctx, Nullability, ResolutionTables, TypeId};
use dartr_syntax::TokenId;
use dartr_typesystem::TypeExt;
use indexmap::IndexMap;

use crate::ast_ext::token_end;

/// Dart `RecordTypeAnnotationResolver`.
pub struct RecordTypeAnnotationResolver<'r, 'a> {
    pub ctx: Ctx<'a>,
    /// `libraryElement.featureSet.isEnabled(Feature.wildcard_variables)`.
    pub wildcard_variables: bool,
    /// The path and the URI of the unit (Dart `diagnosticReporter.source`).
    pub source_path: &'r str,
    pub source_uri: &'r str,
    pub tables: &'r mut ResolutionTables,
    pub diagnostics: &'r mut Vec<Diagnostic>,
}

/// The name token of a record type field.
fn field_name(ast: &Ast, field: NodeId) -> Option<TokenId> {
    if let Some(f) = ast.cast::<RecordTypeAnnotationPositionalField>(field) {
        ast[f].name
    } else {
        ast.cast::<dartr_ast::RecordTypeAnnotationNamedField>(field)
            .map(|f| ast[f].name)
    }
}

/// Dart `RecordTypeExtension.positionalFieldIndex(name)`: the index of
/// `$1`, `$2`, ... (`^\$[1-9]\d*$`).
pub fn positional_field_index(name: &str) -> Option<usize> {
    let digits = name.strip_prefix('$')?;
    let mut chars = digits.chars();
    let first = chars.next()?;
    if !('1'..='9').contains(&first) || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let position: usize = digits.parse().ok()?;
    Some(position - 1)
}

/// Dart `RecordLiteralResolver.isForbiddenNameForRecordField`.
pub fn is_forbidden_name_for_record_field(name: &str) -> bool {
    matches!(name, "hashCode" | "runtimeType" | "noSuchMethod" | "toString")
}

impl RecordTypeAnnotationResolver<'_, '_> {
    /// Dart `isWildCardVariablesEnabled`.
    fn is_wild_card_variables_enabled(&self) -> bool {
        self.wildcard_variables
    }

    /// Dart `isPositionalWildCard`.
    fn is_positional_wild_card(&self, ast: &Ast, field: NodeId, name: &str) -> bool {
        ast.is::<RecordTypeAnnotationPositionalField>(field)
            && name == "_"
            && self.is_wild_card_variables_enabled()
    }

    fn report_at_token(&mut self, ast: &Ast, d: dartr_diagnostics::LocatableDiagnostic, token: TokenId) {
        let offset = ast.tokens.offset(token);
        let length = token_end(ast, token) - offset;
        self.diagnostics
            .push(d.at_offset(offset as usize, length as usize).into_diagnostic());
    }

    /// Dart `reportDuplicateFieldDefinitions`.
    pub fn report_duplicate_field_definitions(&mut self, ast: &Ast, node: Id<RecordTypeAnnotation>) {
        let mut used_names: IndexMap<String, NodeId> = IndexMap::new();
        for field in ast.record_type_fields(node) {
            let Some(name_token) = field_name(ast, field) else {
                continue;
            };
            let name = ast.tokens.lexeme(name_token).to_string();
            // Multiple positional `_`s are legal with wildcards.
            if self.is_positional_wild_card(ast, field, &name) {
                continue;
            }
            match used_names.get(&name) {
                Some(&previous_field) => {
                    // Dart `DiagnosticFactory.duplicateFieldDefinitionInType`.
                    let previous_name = field_name(ast, previous_field).expect("named field");
                    let d = diag::duplicate_field_name(&name).with_context_messages([
                        DiagnosticMessage {
                            file_path: self.source_path.to_string(),
                            offset: ast.tokens.offset(previous_name) as i64,
                            length: name.encode_utf16().count() as i64,
                            message: "The first ".to_string(),
                            url: Some(self.source_uri.to_string()),
                        },
                    ]);
                    self.report_at_token(ast, d, name_token);
                }
                None => {
                    used_names.insert(name, field);
                }
            }
        }
    }

    /// Dart `reportInvalidFieldNames`.
    pub fn report_invalid_field_names(&mut self, ast: &Ast, node: Id<RecordTypeAnnotation>) {
        let positional_fields = ast.list(ast[node].positional_fields).to_vec();
        let positional_count = positional_fields.len();
        for field in ast.record_type_fields(node) {
            let Some(name_token) = field_name(ast, field) else {
                continue;
            };
            let name = ast.tokens.lexeme(name_token).to_string();
            if name.starts_with('_') {
                // Positional record fields named `_` are legal with
                // wildcards.
                if !self.is_positional_wild_card(ast, field, &name) {
                    self.report_at_token(ast, diag::invalid_field_name_private(), name_token);
                }
            } else if let Some(index) = positional_field_index(&name) {
                let position = positional_fields.iter().position(|f| f.raw() == field);
                if index < positional_count && position != Some(index) {
                    self.report_at_token(ast, diag::invalid_field_name_positional(), name_token);
                }
            } else if is_forbidden_name_for_record_field(&name) {
                self.report_at_token(ast, diag::invalid_field_name_from_object(), name_token);
            }
        }
    }

    /// Dart `resolve(node)`.
    pub fn resolve(&mut self, ast: &Ast, node: Id<RecordTypeAnnotation>) {
        self.build_type(ast, node);
        self.report_duplicate_field_definitions(ast, node);
        self.report_invalid_field_names(ast, node);
    }

    /// Dart `_buildType`.
    fn build_type(&mut self, ast: &Ast, node: Id<RecordTypeAnnotation>) {
        let type_of = |tables: &ResolutionTables, n: NodeId| {
            tables
                .annotation_type
                .get(n)
                .copied()
                .unwrap_or(TypeId::INVALID)
        };
        let positional: Vec<TypeId> = ast
            .list(ast[node].positional_fields)
            .iter()
            .map(|&f| type_of(self.tables, ast[f].type_.raw()))
            .collect();
        let named: Vec<dartr_element::NamedType> = match ast[node].named_fields {
            Some(named_fields) => ast
                .list(ast[named_fields].fields)
                .iter()
                .map(|&f| dartr_element::NamedType {
                    name: self.ctx.name(ast.tokens.lexeme(ast[f].name)),
                    ty: type_of(self.tables, ast[f].type_.raw()),
                })
                .collect(),
            None => Vec::new(),
        };
        let nullability = if ast[node].question.is_some() {
            Nullability::Question
        } else {
            Nullability::None
        };
        let t = self.ctx.record_type(&positional, &named, nullability, None);
        self.tables.annotation_type.insert(node, t);
    }
}
