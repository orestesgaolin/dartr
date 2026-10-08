//! The JSON dump of an [AnalysisContextCollection], in the format of
//! `tools/oracle/bin/contexts.dart`.

use crate::analysis_options::{AnalysisOptions, PluginSource};
use crate::collection::{AnalysisContext, AnalysisContextCollection, FileKind};
use crate::sdk::DartSdk;
use crate::workspace::WorkspaceKind;
use serde_json::{Map, Value, json};

/// The JSON of the whole collection.
pub fn collection_json(collection: &AnalysisContextCollection) -> Value {
    json!({
        "sdk": collection.sdk.as_deref().map(sdk_json),
        "contexts": collection.contexts.iter().map(|c| context_json(collection, c)).collect::<Vec<_>>(),
    })
}

fn libraries_json(sdk: &DartSdk) -> Value {
    let mut names: Vec<&str> = sdk
        .libraries()
        .iter()
        .map(|l| l.short_name.as_str())
        .collect();
    names.sort();
    let mut map = Map::new();
    for name in names {
        map.insert(name.to_string(), json!(sdk.map_dart_uri(name)));
    }
    Value::Object(map)
}

fn sdk_json(sdk: &DartSdk) -> Value {
    json!({
        "path": sdk.path(),
        "version": sdk.sdk_version(),
        "language_version": sdk.language_version().map(|v| v.to_string()),
        "libraries": libraries_json(sdk),
    })
}

fn context_json(collection: &AnalysisContextCollection, context: &AnalysisContext) -> Value {
    let root = &context.root;
    let workspace = &root.workspace;
    let mut workspace_json = Map::new();
    let kind = match &workspace.kind {
        WorkspaceKind::Basic => "basic",
        WorkspaceKind::PackageConfig {
            is_pub_workspace, ..
        } => {
            workspace_json.insert("is_pub_workspace".into(), json!(is_pub_workspace));
            "package_config"
        }
        WorkspaceKind::Blaze => "blaze",
        WorkspaceKind::Gn => "gn",
    };
    workspace_json.insert("kind".into(), json!(kind));
    workspace_json.insert("root".into(), json!(workspace.root));

    let mut packages: Vec<_> = workspace.packages.packages().iter().collect();
    packages.sort_by(|a, b| a.name.cmp(&b.name));

    let options_map = collection.options_map(context);
    let mut options = Vec::new();
    if options_map.entries().is_empty() {
        let mut entry = options_json(options_map.default_options());
        entry.insert("folder".into(), Value::Null);
        options.push(Value::Object(entry));
    } else {
        let mut entries: Vec<_> = options_map.entries().iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (folder, folder_options) in entries {
            let mut entry = options_json(folder_options);
            entry.insert("folder".into(), json!(folder));
            options.push(Value::Object(entry));
        }
    }

    let mut analyzed = root.analyzed_files();
    analyzed.sort();
    let files: Vec<Value> = analyzed
        .iter()
        .map(|path| {
            let kind = FileKind::of(path);
            let mut entry = Map::new();
            entry.insert("path".into(), json!(path));
            entry.insert("kind".into(), json!(kind.name()));
            if kind == FileKind::Dart {
                let info = context.file_info(path);
                entry.insert("uri".into(), json!(info.uri));
                entry.insert(
                    "language_version".into(),
                    json!(info.language_version.to_string()),
                );
                entry.insert(
                    "options_file".into(),
                    json!(collection.options_for(context, path).file),
                );
            }
            Value::Object(entry)
        })
        .collect();

    let mut fix_data = context.fix_data_files();
    fix_data.sort();

    let embedder_libraries = context
        .sdk
        .as_deref()
        .filter(|s| s.embedder_yaml().is_some())
        .map(libraries_json);

    json!({
        "root": root.root,
        "workspace": workspace_json,
        "included": root.included_paths().collect::<Vec<_>>(),
        "excluded": root.excluded,
        "options_file": root.options_file,
        "package_config": root.packages_file,
        "packages": packages.iter().map(|p| json!({
            "name": p.name,
            "root": p.root,
            "lib": p.lib,
            "language_version": p.language_version.map(|v| v.to_string()),
        })).collect::<Vec<_>>(),
        "embedder_libraries": embedder_libraries,
        "options": options,
        "files": files,
        "fix_data": fix_data,
    })
}

fn plugin_source_json(source: &PluginSource) -> Value {
    match source {
        PluginSource::Versioned {
            constraint,
            hosted_url,
        } => {
            json!({"kind": "version", "constraint": constraint, "hosted": hosted_url})
        }
        PluginSource::Git {
            url,
            path,
            git_ref,
            tag_pattern,
        } => {
            json!({"kind": "git", "url": url, "path": path, "ref": git_ref, "tag_pattern": tag_pattern})
        }
        PluginSource::Path { path } => json!({"kind": "path", "path": path}),
    }
}

/// The JSON of effective analysis options. `cannot_ignore` only contains the
/// explicitly named codes (the severity entries need the diagnostic codes).
pub fn options_json(options: &AnalysisOptions) -> Map<String, Value> {
    let value = json!({
        "file": options.file,
        "errors": options.error_processors.iter().map(|p| json!({
            "code": p.code,
            "severity": p.severity.map(|s| s.name()),
        })).collect::<Vec<_>>(),
        "exclude": options.exclude_patterns,
        "strict_casts": options.strict_casts,
        "strict_inference": options.strict_inference,
        "strict_raw_types": options.strict_raw_types,
        "experiments": options.enabled_experiments(),
        "lint": options.lint,
        "lint_rules": options.lint_rules,
        "legacy_plugins": options.enabled_legacy_plugin_names,
        "plugins": options.plugins.configurations.iter().map(|c| {
            let diagnostics: Map<String, Value> = c
                .diagnostic_configs
                .iter()
                .map(|(name, config)| (name.clone(), json!(config.severity.name())))
                .collect();
            json!({"name": c.name, "source": plugin_source_json(&c.source), "diagnostics": diagnostics})
        }).collect::<Vec<_>>(),
        "plugin_dependency_overrides": options.plugins.dependency_overrides.as_ref().map(|overrides| {
            overrides.iter().map(|(name, source)| (name.clone(), plugin_source_json(source))).collect::<Map<_, _>>()
        }),
        "cannot_ignore": options.unignorable_code_names(&[]),
        "formatter_page_width": options.formatter_page_width,
        "formatter_trailing_commas": options.formatter_trailing_commas.map(|t| t.name()),
        "code_style_format": options.code_style_use_formatter,
        "chrome_os_manifest_checks": options.chrome_os_manifest_checks,
        "propagate_linter_exceptions": options.propagate_linter_exceptions,
    });
    match value {
        Value::Object(map) => map,
        _ => unreachable!(),
    }
}
