// Dart source: pkg/analysis_server/lib/src/lsp/server_capabilities_computer.dart
// Dart source: pkg/analysis_server/lib/src/lsp/registration/feature_registration.dart
// Dart source: pkg/analysis_server/lib/src/lsp/client_capabilities.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_*.dart (the *Registrations classes)

//! Client capabilities, the static server capabilities of `initialize`, and
//! the dynamic registrations (`client/registerCapability`).
//!
//! dartr advertises only the features it implements, with the same options
//! as the Dart server. The features of the Dart server are listed in
//! [`DART_FEATURES`] in the order of Dart `LspFeatures.allFeatures`, so that
//! registrations are sent in the same order; `implemented` says which ones
//! dartr has. The capability test (`tests/capabilities.rs`) compares the
//! result with `dart language-server` and prints the missing ones.

use serde_json::{Map, Value, json};

use crate::client_configuration::LspClientConfiguration;
use crate::formatting::DART_TYPE_FORMATTING_CHARACTERS;
use crate::mapping::{DEFAULT_SYMBOL_KINDS, DiagnosticOptions};

/// The client capabilities of `initialize` (Dart `LspClientCapabilities`):
/// the raw JSON and accessors for the values that the server reads.
#[derive(Clone, Debug, Default)]
pub struct ClientCapabilities {
    pub raw: Value,
}

impl ClientCapabilities {
    pub fn new(raw: Value) -> Self {
        ClientCapabilities { raw }
    }

    fn get(&self, path: &[&str]) -> Option<&Value> {
        let mut v = &self.raw;
        for p in path {
            v = v.get(p)?;
        }
        Some(v)
    }

    fn flag(&self, path: &[&str]) -> bool {
        self.get(path).and_then(Value::as_bool).unwrap_or(false)
    }

    /// `textDocument.<feature>.dynamicRegistration`.
    pub fn text_document_dynamic(&self, feature: &str) -> bool {
        self.flag(&["textDocument", feature, "dynamicRegistration"])
    }

    /// `workspace.<feature>.dynamicRegistration`.
    pub fn workspace_dynamic(&self, feature: &str) -> bool {
        self.flag(&["workspace", feature, "dynamicRegistration"])
    }

    /// `workspace.configuration`.
    pub fn configuration(&self) -> bool {
        self.flag(&["workspace", "configuration"])
    }

    /// `window.workDoneProgress`.
    pub fn work_done_progress(&self) -> bool {
        self.flag(&["window", "workDoneProgress"])
    }

    /// `textDocument.documentSymbol.hierarchicalDocumentSymbolSupport`.
    pub fn hierarchical_symbols(&self) -> bool {
        self.flag(&[
            "textDocument",
            "documentSymbol",
            "hierarchicalDocumentSymbolSupport",
        ])
    }

    /// `textDocument.documentSymbol.symbolKind.valueSet`, or the defaults.
    pub fn document_symbol_kinds(&self) -> Vec<i64> {
        match self
            .get(&["textDocument", "documentSymbol", "symbolKind", "valueSet"])
            .and_then(Value::as_array)
        {
            Some(list) => list.iter().filter_map(Value::as_i64).collect(),
            None => DEFAULT_SYMBOL_KINDS.collect(),
        }
    }

    /// `textDocument.foldingRange.lineFoldingOnly`.
    pub fn line_folding_only(&self) -> bool {
        self.flag(&["textDocument", "foldingRange", "lineFoldingOnly"])
    }

    /// The options of the diagnostic conversion.
    pub fn diagnostic_options(&self) -> DiagnosticOptions {
        let tags = self
            .get(&[
                "textDocument",
                "publishDiagnostics",
                "tagSupport",
                "valueSet",
            ])
            .and_then(Value::as_array)
            .map(|l| l.iter().filter_map(Value::as_i64).collect())
            .unwrap_or_default();
        DiagnosticOptions {
            supported_tags: Some(tags),
            code_description: self.flag(&[
                "textDocument",
                "publishDiagnostics",
                "codeDescriptionSupport",
            ]),
        }
    }
}

/// A Dart LSP feature (Dart `FeatureRegistration`).
pub struct Feature {
    /// The Dart registration class.
    pub name: &'static str,
    /// The key in `ServerCapabilities` for static registration.
    pub capability: Option<&'static str>,
    /// Whether dartr implements the feature.
    pub implemented: bool,
}

/// The features of the Dart server in `LspFeatures.allFeatures` order.
pub const DART_FEATURES: &[Feature] = &[
    feature(
        "CallHierarchyRegistrations",
        Some("callHierarchyProvider"),
        true,
    ),
    feature("ChangeWorkspaceFoldersRegistrations", None, true),
    feature("CodeActionRegistrations", Some("codeActionProvider"), false),
    feature("CodeLensRegistrations", Some("codeLensProvider"), false),
    feature("CompletionRegistrations", Some("completionProvider"), false),
    feature("DefinitionRegistrations", Some("definitionProvider"), true),
    feature(
        "DocumentLinkRegistrations",
        Some("documentLinkProvider"),
        false,
    ),
    feature("DocumentColorRegistrations", Some("colorProvider"), false),
    feature(
        "DocumentHighlightsRegistrations",
        Some("documentHighlightProvider"),
        true,
    ),
    feature(
        "DocumentSymbolsRegistrations",
        Some("documentSymbolProvider"),
        true,
    ),
    feature(
        "ExecuteCommandRegistrations",
        Some("executeCommandProvider"),
        false,
    ),
    feature("FoldingRegistrations", Some("foldingRangeProvider"), true),
    feature(
        "FormatOnTypeRegistrations",
        Some("documentOnTypeFormattingProvider"),
        true,
    ),
    feature(
        "FormatRangeRegistrations",
        Some("documentRangeFormattingProvider"),
        true,
    ),
    feature(
        "FormattingRegistrations",
        Some("documentFormattingProvider"),
        true,
    ),
    feature("HoverRegistrations", Some("hoverProvider"), true),
    feature(
        "ImplementationRegistrations",
        Some("implementationProvider"),
        true,
    ),
    feature("InlayHintRegistrations", Some("inlayHintProvider"), true),
    feature(
        "InlineValueRegistrations",
        Some("inlineValueProvider"),
        false,
    ),
    feature("ReferencesRegistrations", Some("referencesProvider"), true),
    feature("RenameRegistrations", Some("renameProvider"), false),
    feature(
        "SelectionRangeRegistrations",
        Some("selectionRangeProvider"),
        true,
    ),
    feature(
        "SemanticTokensRegistrations",
        Some("semanticTokensProvider"),
        true,
    ),
    feature(
        "SignatureHelpRegistrations",
        Some("signatureHelpProvider"),
        true,
    ),
    feature("TextDocumentRegistrations", Some("textDocumentSync"), true),
    feature(
        "TypeDefinitionRegistrations",
        Some("typeDefinitionProvider"),
        true,
    ),
    feature(
        "TypeHierarchyRegistrations",
        Some("typeHierarchyProvider"),
        true,
    ),
    feature("WillRenameFilesRegistrations", None, false),
    feature("WorkspaceDidChangeConfigurationRegistrations", None, true),
    feature(
        "WorkspaceSymbolRegistrations",
        Some("workspaceSymbolProvider"),
        true,
    ),
];

const fn feature(
    name: &'static str,
    capability: Option<&'static str>,
    implemented: bool,
) -> Feature {
    Feature {
        name,
        capability,
        implemented,
    }
}

/// Dart `RegistrationContext.dartFilters` (only `file`: dartr has no
/// custom URI schemes).
/// Dart `SemanticTokensOptions` (the legend, full without delta, range).
fn semantic_tokens_options() -> Value {
    json!({
        "legend": {
            "tokenTypes": crate::semantic_tokens::TOKEN_TYPES,
            "tokenModifiers": crate::semantic_tokens::TOKEN_MODIFIERS,
        },
        "full": {"delta": false},
        "range": true,
    })
}

fn dart_files() -> Value {
    json!([{"language": "dart", "scheme": "file"}])
}

/// Dart `TextDocumentRegistrations.synchronisedTypes`.
fn synchronised_types() -> Value {
    json!([
        {"language": "dart", "scheme": "file"},
        {"language": "yaml", "pattern": "**/pubspec.yaml", "scheme": "file"},
        {"language": "yaml", "pattern": "**/analysis_options.yaml", "scheme": "file"},
        {"language": "yaml", "pattern": "**/lib/{fix_data.yaml,fix_data/**.yaml}", "scheme": "file"},
    ])
}

/// The options of on-type formatting (Dart
/// `DocumentOnTypeFormattingOptions` of `FormatOnTypeRegistrations`).
fn on_type_formatting_options() -> Map<String, Value> {
    let mut m = Map::new();
    m.insert(
        "firstTriggerCharacter".into(),
        json!(DART_TYPE_FORMATTING_CHARACTERS[0]),
    );
    m.insert(
        "moreTriggerCharacter".into(),
        json!(DART_TYPE_FORMATTING_CHARACTERS[1..]),
    );
    m
}

/// Dart `ServerCapabilitiesComputer.computeServerCapabilities` for the
/// implemented features. The formatting features depend on the global
/// `dart.enableSdkFormatter` setting of [config].
pub fn server_capabilities(client: &ClientCapabilities, config: &LspClientConfiguration) -> Value {
    let enable_formatter = config.global().enable_sdk_formatter();
    let mut c = Map::new();
    if !client.text_document_dynamic("callHierarchy") {
        c.insert("callHierarchyProvider".into(), json!(true));
    }
    if !client.text_document_dynamic("inlayHint") {
        c.insert("inlayHintProvider".into(), json!({"resolveProvider": false}));
    }
    if !client.text_document_dynamic("semanticTokens") {
        c.insert("semanticTokensProvider".into(), semantic_tokens_options());
    }
    if !client.text_document_dynamic("signatureHelp") {
        c.insert(
            "signatureHelpProvider".into(),
            json!({"triggerCharacters": ["("], "retriggerCharacters": [","]}),
        );
    }
    if !client.text_document_dynamic("typeHierarchy") {
        c.insert("typeHierarchyProvider".into(), json!(true));
    }
    // Dart `WorkspaceSymbolRegistrations`: static only.
    c.insert("workspaceSymbolProvider".into(), json!(true));
    for (feature, key) in [
        ("definition", "definitionProvider"),
        ("documentHighlight", "documentHighlightProvider"),
        ("hover", "hoverProvider"),
        ("implementation", "implementationProvider"),
        ("references", "referencesProvider"),
        ("typeDefinition", "typeDefinitionProvider"),
    ] {
        if !client.text_document_dynamic(feature) {
            c.insert(key.into(), json!(true));
        }
    }
    if !client.text_document_dynamic("synchronization") {
        c.insert(
            "textDocumentSync".into(),
            json!({"openClose": true, "change": 2, "willSave": false, "willSaveWaitUntil": false}),
        );
    }
    if !client.text_document_dynamic("documentSymbol") {
        c.insert("documentSymbolProvider".into(), json!(true));
    }
    if !client.text_document_dynamic("foldingRange") {
        c.insert("foldingRangeProvider".into(), json!(true));
    }
    if enable_formatter && !client.text_document_dynamic("onTypeFormatting") {
        c.insert(
            "documentOnTypeFormattingProvider".into(),
            Value::Object(on_type_formatting_options()),
        );
    }
    if enable_formatter && !client.text_document_dynamic("rangeFormatting") {
        c.insert("documentRangeFormattingProvider".into(), json!(true));
    }
    if enable_formatter && !client.text_document_dynamic("formatting") {
        c.insert("documentFormattingProvider".into(), json!(true));
    }
    if !client.text_document_dynamic("selectionRange") {
        c.insert("selectionRangeProvider".into(), json!(true));
    }
    // `fileOperations` is only for `willRenameFiles`, which dartr does not
    // implement.
    c.insert(
        "workspace".into(),
        json!({"workspaceFolders": {"supported": true, "changeNotifications": true}}),
    );
    Value::Object(c)
}

/// A dynamic registration: the method and its options.
#[derive(Clone, Debug, PartialEq)]
pub struct DynamicRegistration {
    pub method: &'static str,
    pub options: Option<Value>,
}

/// Dart `ServerCapabilitiesComputer.performDynamicRegistration`: the
/// registrations of the implemented features that the client registers
/// dynamically, in Dart order. The formatting features depend on the
/// global `dart.enableSdkFormatter` setting of [config].
pub fn dynamic_registrations(
    client: &ClientCapabilities,
    config: &LspClientConfiguration,
) -> Vec<DynamicRegistration> {
    let enable_formatter = config.global().enable_sdk_formatter();
    let mut out = Vec::new();
    let reg = |method, options| DynamicRegistration {
        method,
        options: Some(options),
    };
    // Dart `fullySupportedTypes`: the Dart files and the types of plugins
    // (the language server of dartr has no plugins).
    if client.text_document_dynamic("callHierarchy") {
        out.push(reg(
            "textDocument/prepareCallHierarchy",
            json!({"documentSelector": dart_files()}),
        ));
    }
    if client.text_document_dynamic("definition") {
        out.push(reg(
            "textDocument/definition",
            json!({"documentSelector": dart_files()}),
        ));
    }
    if client.text_document_dynamic("documentHighlight") {
        out.push(reg(
            "textDocument/documentHighlight",
            json!({"documentSelector": dart_files()}),
        ));
    }
    if client.text_document_dynamic("documentSymbol") {
        out.push(reg(
            "textDocument/documentSymbol",
            json!({"documentSelector": dart_files()}),
        ));
    }
    if client.text_document_dynamic("foldingRange") {
        out.push(reg(
            "textDocument/foldingRange",
            json!({"documentSelector": dart_files()}),
        ));
    }
    if enable_formatter && client.text_document_dynamic("onTypeFormatting") {
        let mut options = on_type_formatting_options();
        options.insert("documentSelector".into(), dart_files());
        out.push(reg("textDocument/onTypeFormatting", Value::Object(options)));
    }
    if enable_formatter && client.text_document_dynamic("rangeFormatting") {
        out.push(reg(
            "textDocument/rangeFormatting",
            json!({"documentSelector": dart_files()}),
        ));
    }
    // Dart `fullySupportedTypes`: the Dart files and the types of plugins
    // (dartr has no plugins).
    if enable_formatter && client.text_document_dynamic("formatting") {
        out.push(reg(
            "textDocument/formatting",
            json!({"documentSelector": dart_files()}),
        ));
    }
    for (feature, method) in [
        ("hover", "textDocument/hover"),
        ("implementation", "textDocument/implementation"),
    ] {
        if client.text_document_dynamic(feature) {
            out.push(reg(method, json!({"documentSelector": dart_files()})));
        }
    }
    if client.text_document_dynamic("inlayHint") {
        out.push(reg(
            "textDocument/inlayHint",
            json!({"documentSelector": dart_files(), "resolveProvider": false}),
        ));
    }
    if client.text_document_dynamic("references") {
        out.push(reg(
            "textDocument/references",
            json!({"documentSelector": dart_files()}),
        ));
    }
    if client.text_document_dynamic("selectionRange") {
        out.push(reg(
            "textDocument/selectionRange",
            json!({"documentSelector": dart_files()}),
        ));
    }
    if client.text_document_dynamic("semanticTokens") {
        let mut options = semantic_tokens_options();
        options["documentSelector"] = dart_files();
        // Dart `CustomMethods.semanticTokenDynamicRegistration`.
        out.push(reg("textDocument/semanticTokens", options));
    }
    if client.text_document_dynamic("signatureHelp") {
        out.push(reg(
            "textDocument/signatureHelp",
            json!({
                "documentSelector": dart_files(),
                "triggerCharacters": ["("],
                "retriggerCharacters": [","],
            }),
        ));
    }
    if client.text_document_dynamic("synchronization") {
        out.push(reg(
            "textDocument/didOpen",
            json!({"documentSelector": synchronised_types()}),
        ));
        out.push(reg(
            "textDocument/didClose",
            json!({"documentSelector": synchronised_types()}),
        ));
        out.push(reg(
            "textDocument/didChange",
            json!({"documentSelector": synchronised_types(), "syncKind": 2}),
        ));
    }
    if client.text_document_dynamic("typeDefinition") {
        out.push(reg(
            "textDocument/typeDefinition",
            json!({"documentSelector": dart_files()}),
        ));
    }
    if client.text_document_dynamic("typeHierarchy") {
        out.push(reg(
            "textDocument/prepareTypeHierarchy",
            json!({"documentSelector": dart_files()}),
        ));
    }
    if client.workspace_dynamic("didChangeConfiguration") {
        out.push(DynamicRegistration {
            method: "workspace/didChangeConfiguration",
            options: None,
        });
    }
    out
}
