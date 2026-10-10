// Dart source: pkg/analysis_server/lib/src/services/correction/bulk_fix_processor.dart (_computeChangesToPubspec, _runPubspecValidatorAndFixGenerator, _PubspecDeps)
// Dart source: pkg/analysis_server/lib/src/services/correction/fix/pubspec/fix_generator.dart (PubspecFixGenerator._addMissingDependency, _getTextAndOffset)

//! The pubspec pass of the bulk fixes (`dart fix` and `edit.bulkFixes`
//! with `updatePubspec`): adds the packages that the Dart files import to
//! the `dependencies` and `dev_dependencies` of their `pubspec.yaml`.

use dartr_ast::*;
use dartr_project::yaml::YamlNode;
use dartr_project::{AnalysisContextCollection, WorkspaceKind};
use indexmap::IndexMap;

use super::bulk_fix_processor::is_generated;
use super::change::{SourceEdit, SourceFileEdit};

/// Dart `_PubspecDeps`, in insertion order.
#[derive(Default)]
struct PubspecDeps {
    packages: Vec<String>,
    dev_packages: Vec<String>,
}

fn add_unique(list: &mut Vec<String>, name: &str) {
    if !list.iter().any(|n| n == name) {
        list.push(name.to_string());
    }
}

/// The UTF-16 offset of the byte offset [byte] of [text].
fn utf16_offset(text: &str, byte: usize) -> u32 {
    text[..byte.min(text.len())].encode_utf16().count() as u32
}

/// The URIs of the import directives of [content].
fn import_uris(content: &str, path: &str) -> Vec<String> {
    let parsed = dartr_ast_builder::parse_string(content, path);
    let ast = &parsed.ast;
    let mut out = Vec::new();
    for &directive in ast.list_raw(ast[parsed.unit].directives) {
        let Some(import) = ast.cast::<ImportDirective>(directive) else {
            continue;
        };
        let Some(literal) = ast.cast::<SimpleStringLiteral>(ast[import].uri) else {
            continue;
        };
        let lexeme = ast.tokens.lexeme(ast[literal].literal);
        let value = lexeme
            .trim_start_matches('r')
            .trim_matches(|c| c == '\'' || c == '"');
        out.push(value.to_string());
    }
    out
}

/// Dart `BulkFixProcessor.fixPubspec`: the edits of the `pubspec.yaml`
/// files and the paths of the changed files.
pub fn pubspec_fixes(collection: &AnalysisContextCollection) -> (Vec<SourceFileEdit>, Vec<String>) {
    let mut edits = Vec::new();
    let mut details = Vec::new();
    for context in &collection.contexts {
        let workspace = &context.root.workspace;
        if !matches!(workspace.kind, WorkspaceKind::PackageConfig { .. }) {
            continue;
        }
        let mut package_to_deps: IndexMap<String, PubspecDeps> = IndexMap::new();
        for path in context.root.analyzed_files() {
            if !path.ends_with(".dart") || is_generated(&path) {
                continue;
            }
            let Some(dartr_project::workspace::WorkspacePackage::Pub { root, .. }) =
                workspace.find_package_for(&path)
            else {
                continue;
            };
            let lib = format!("{root}/lib");
            let bin = format!("{root}/bin");
            let deps = package_to_deps.entry(root.clone()).or_default();
            let Some(content) = dartr_project::fs::read_string(&path) else {
                continue;
            };
            let content = dartr_syntax::strip_bom(&content).to_string();
            for uri in import_uris(&content, &path) {
                let Some(rest) = uri.strip_prefix("package:") else {
                    continue;
                };
                let name = rest.split('/').next().unwrap_or("");
                if dartr_project::paths::is_within(&lib, &path)
                    || dartr_project::paths::is_within(&bin, &path)
                {
                    add_unique(&mut deps.packages, name);
                } else {
                    add_unique(&mut deps.dev_packages, name);
                }
            }
        }
        for (root, deps) in package_to_deps {
            let pubspec = format!("{root}/pubspec.yaml");
            let dev: Vec<String> = deps
                .dev_packages
                .iter()
                .filter(|n| !deps.packages.contains(n))
                .cloned()
                .collect();
            let validation = dartr_project::pubspec_validator::validate_missing_dependencies(
                context,
                &pubspec,
                &deps.packages,
                &dev,
            );
            let Some(data) = validation.data else {
                continue;
            };
            let Some(file_edits) = missing_dependency_edits(&pubspec, &data) else {
                continue;
            };
            if file_edits.is_empty() {
                continue;
            }
            edits.push(SourceFileEdit {
                file: pubspec.clone(),
                file_stamp: 0,
                edits: file_edits,
            });
            details.push(pubspec);
        }
    }
    (edits, details)
}

fn key_is(entry: &(YamlNode, YamlNode), key: &str) -> bool {
    entry.0.string_value() == Some(key)
}

/// Dart `PubspecFixGenerator._addMissingDependency`: the edits, from the
/// last to the first.
fn missing_dependency_edits(
    pubspec: &str,
    data: &dartr_project::pubspec_validator::MissingDependencyData,
) -> Option<Vec<SourceEdit>> {
    let content = dartr_project::fs::read_string(pubspec)?;
    let node = dartr_project::yaml::load_yaml_node(&content).ok()?;
    let entries = node.as_map()?;
    let eol = super::change_builder::end_of_line(&content).unwrap_or("\n");
    let line_starts: Vec<u32> = dartr_syntax::LineInfo::from_content(&content)
        .line_starts
        .clone();
    let off = |byte: usize| utf16_offset(&content, byte);
    // (offset, length, text), in the order Dart adds them.
    let mut list: Vec<(u32, u32, String)> = Vec::new();
    let text_and_offset = |section: &str, names: &[String]| {
        let value = entries.iter().find(|e| key_is(e, section)).map(|e| &e.1);
        let mut text = String::new();
        if value.is_none() {
            text.push_str(&format!("{section}:{eol}"));
        }
        for name in names {
            text.push_str(&format!("  {name}: any{eol}"));
        }
        let offset = match value {
            None => off(node.span.end),
            Some(v) => off(v.span.end),
        };
        (text, offset)
    };
    if !data.add_dependencies.is_empty() {
        let (text, offset) = text_and_offset("dependencies", &data.add_dependencies);
        list.push((offset, 0, text));
    }
    if !data.add_dev_dependencies.is_empty() {
        let (text, offset) = text_and_offset("dev_dependencies", &data.add_dev_dependencies);
        list.push((offset, 0, text));
    }
    let remove = &data.remove_dev_dependencies;
    if !remove.is_empty() {
        let index = entries.iter().position(|e| key_is(e, "dev_dependencies"))?;
        let section = entries[index].1.as_map()?;
        if remove.len() == section.len() {
            if index > 0 {
                let start = off(entries[index - 1].1.span.end);
                let end = off(entries[index].1.span.end);
                list.push((start, end - start, String::new()));
            }
        } else {
            let mut edit: Option<(u32, u32)> = None;
            for dep in remove {
                let Some(i) = section.iter().position(|e| key_is(e, dep)) else {
                    continue;
                };
                let current = &section[i];
                let prev = (i > 0).then(|| &section[i - 1]);
                let next = section.get(i + 1);
                let mut start = match prev {
                    Some(p) => off(p.1.span.end),
                    None => off(current.0.span.start),
                };
                let mut end = match next {
                    None => off(current.1.span.end),
                    Some(n) => off(n.0.span.start),
                };
                if let (Some(p), Some(n)) = (prev, next) {
                    let removed = |e: &(YamlNode, YamlNode)| {
                        e.0.string_value()
                            .is_some_and(|k| remove.iter().any(|r| r == k))
                    };
                    if !removed(p) && !removed(n) {
                        start = line_starts[current.0.span.line];
                        end = line_starts[n.0.span.line];
                    }
                }
                match edit {
                    None => edit = Some((start, end)),
                    Some((s, e)) if e > start => edit = Some((s, end)),
                    Some((s, e)) => {
                        list.push((s, e - s, String::new()));
                        edit = Some((start, end));
                    }
                }
            }
            if let Some((s, e)) = edit {
                list.push((s, e - s, String::new()));
            }
        }
    }
    // Dart `FileEdit.add`: the edits sorted from the last to the first; an
    // edit at the same offset goes before the earlier ones.
    let mut file_edit = SourceFileEdit::new(pubspec, 0);
    for (offset, length, replacement) in list {
        let _ = file_edit.add(
            SourceEdit {
                offset,
                length,
                replacement,
                ..Default::default()
            },
            false,
        );
    }
    Some(file_edit.edits)
}
