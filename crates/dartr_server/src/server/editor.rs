// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_signature_help.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_semantic_tokens.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_inlay_hint.dart

//! The editor requests of the server that work on one resolved unit:
//! `textDocument/signatureHelp`, `textDocument/semanticTokens/{full,range}`
//! and `textDocument/inlayHint`.

use dartr_element::NoopSink;
use serde_json::{Value, json};

use super::Server;
use crate::mapping::ErrorOr;

impl Server {
    /// The client formats of `textDocument.<feature>....` at [pointer]
    /// (Dart `_listToNullableSet`): `None` when the client sends no list.
    pub(crate) fn client_formats(&self, pointer: &str) -> Option<Vec<String>> {
        self.client
            .raw
            .pointer(pointer)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
    }

    /// Dart `asMarkupContentOrString(preferredFormats, content)`.
    pub(crate) fn markup_content_or_string(
        formats: &Option<Vec<String>>,
        content: String,
    ) -> Value {
        match formats {
            None => Value::String(content),
            Some(formats) => {
                let markdown = formats.is_empty() || formats.iter().any(|f| f == "markdown");
                let plain = formats.iter().any(|f| f == "plaintext");
                let kind = if plain && !markdown {
                    "plaintext"
                } else {
                    "markdown"
                };
                json!({"kind": kind, "value": content})
            }
        }
    }

    /// Dart `SignatureHelpHandler.handle`.
    pub(crate) fn signature_help(&mut self, params: &Value) -> ErrorOr<Value> {
        if !super::is_dart_document(params) {
            return Ok(Value::Null);
        }
        let auto_triggered = params
            .pointer("/context/triggerKind")
            .and_then(Value::as_i64)
            == Some(2)
            && params
                .pointer("/context/isRetrigger")
                .and_then(Value::as_bool)
                == Some(false);
        let path = self.path_of_doc(params)?;
        let resolved = self.require_resolved_unit(&path)?;
        let line_info = resolved.line_info().clone();
        let offset = self.position_offset(&line_info, params)?;
        let formats = self
            .client_formats("/textDocument/signatureHelp/signatureInformation/documentationFormat");
        let null_active = self
            .client
            .raw
            .pointer("/textDocument/signatureHelp/signatureInformation/noActiveParameterSupport")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let templates = self.dartdoc_templates(&path);
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let u = crate::element_locator::Unit {
            ctx: &ctx,
            ast: &unit.ast,
            tables: &unit.tables,
        };
        let documentation = |doc: String| Self::markup_content_or_string(&formats, doc);
        if unit.ast.node_covering(unit.unit.raw(), offset, 0).is_none() {
            return Ok(Value::Null);
        }
        if let Some((help, list_offset)) = crate::signature::compute_type_arguments_signature(
            &u,
            unit.unit,
            offset,
            &templates,
            documentation,
            null_active,
        ) {
            if !(auto_triggered && offset != list_offset + 1) {
                return Ok(help);
            }
        }
        let Some(signature) =
            crate::signature::compute_signature(&u, unit.unit, offset, &templates)
        else {
            return Ok(Value::Null);
        };
        if auto_triggered && offset != signature.argument_list_offset + 1 {
            return Ok(Value::Null);
        }
        Ok(crate::signature::to_signature_help(
            &ctx,
            &signature,
            documentation,
            null_active,
        ))
    }

    /// Dart `AbstractSemanticTokensHandler._handleImpl` (no plugins).
    pub(crate) fn semantic_tokens(&mut self, params: &Value, with_range: bool) -> ErrorOr<Value> {
        let path = self.path_of_doc(params)?;
        let resolved = self.require_resolved_unit(&path).ok();
        let line_info = match &resolved {
            Some(r) => r.line_info().clone(),
            None => match self.line_info_of(&path) {
                Some(l) => l,
                None => return Ok(Value::Null),
            },
        };
        let range = if with_range {
            let range = params.get("range").cloned().unwrap_or(Value::Null);
            let start = range
                .get("start")
                .and_then(crate::mapping::read_position)
                .ok_or_else(|| {
                    crate::mapping::ResponseError::new(
                        crate::mapping::codes::INVALID_PARAMS,
                        "Invalid params",
                    )
                })?;
            let end = range
                .get("end")
                .and_then(crate::mapping::read_position)
                .ok_or_else(|| {
                    crate::mapping::ResponseError::new(
                        crate::mapping::codes::INVALID_PARAMS,
                        "Invalid params",
                    )
                })?;
            let start = crate::mapping::to_offset(&line_info, start.0, start.1, false)?;
            let end = crate::mapping::to_offset(&line_info, end.0, end.1, false)?;
            Some((start, end))
        } else {
            None
        };
        let tokens = match &resolved {
            Some(resolved) => {
                let sink = NoopSink;
                let ctx = resolved.ctx(&sink);
                let unit = resolved.unit();
                let u = crate::element_locator::Unit {
                    ctx: &ctx,
                    ast: &unit.ast,
                    tables: &unit.tables,
                };
                crate::semantic_tokens::Computer::new(&u, range).compute(unit.unit)
            }
            None => Vec::new(),
        };
        let data = crate::semantic_tokens::encode(tokens, &line_info, range);
        Ok(json!({"data": data}))
    }

    /// Dart `InlayHintHandler.handle`.
    pub(crate) fn inlay_hints(&mut self, params: &Value) -> ErrorOr<Value> {
        if !super::is_dart_document(params) {
            return Ok(json!([]));
        }
        let path = self.path_of_doc(params)?;
        let resolved = self.require_resolved_unit(&path)?;
        let config = self.client_configuration.global().inlay_hints();
        let line_info = resolved.line_info().clone();
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let u = crate::element_locator::Unit {
            ctx: &ctx,
            ast: &unit.ast,
            tables: &unit.tables,
        };
        let hints = crate::inlay_hints::Computer::new(&u, &line_info, config).compute(unit.unit);
        Ok(Value::Array(hints))
    }
}
