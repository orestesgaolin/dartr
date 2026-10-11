// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/move_file.dart

use dartr_ast::*;
use dartr_project::paths;
use indexmap::IndexMap;
use std::collections::HashSet;

use super::host::RefactoringHost;
use super::rename::ChangeAcc;
use super::status::RefactoringStatus;
use crate::correction::change::SourceChange;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct FileReference {
    offset: u32,
    length: u32,
    source_file: String,
    target_file: String,
    quoted_uri_value: String,
}

pub struct MoveFile {
    pub rename_mapping: IndexMap<String, String>,
}

impl MoveFile {
    pub fn new(old_file: impl Into<String>, new_file: impl Into<String>) -> Self {
        let mut rename_mapping = IndexMap::new();
        rename_mapping.insert(old_file.into(), new_file.into());
        Self { rename_mapping }
    }

    pub fn new_multi(rename_mapping: IndexMap<String, String>) -> Self {
        Self { rename_mapping }
    }

    pub fn refactoring_name(&self) -> &'static str {
        "Move File"
    }

    pub fn check_initial_conditions(&self) -> RefactoringStatus {
        RefactoringStatus::new()
    }

    pub fn check_final_conditions(&self, host: &mut dyn RefactoringHost) -> RefactoringStatus {
        let Some(collection) = host.collection() else {
            return RefactoringStatus::new();
        };

        let mut sessions = HashSet::new();
        for old_file in self.rename_mapping.keys() {
            for ctx in &collection.contexts {
                if ctx.root.root == *old_file {
                    return RefactoringStatus::fatal(format!(
                        "Renaming an analysis root is not supported ({old_file})"
                    ));
                }
            }

            let matching_contexts: Vec<usize> = collection
                .contexts
                .iter()
                .enumerate()
                .filter(|(_, ctx)| ctx.root.is_analyzed(old_file))
                .map(|(i, _)| i)
                .collect();

            if matching_contexts.len() != 1 {
                return RefactoringStatus::fatal(format!(
                    "{old_file} does not belong to an analysis root."
                ));
            }
            sessions.insert(matching_contexts[0]);

            if !(dartr_project::fs::file_exists(old_file)
                || dartr_project::fs::folder_exists(old_file))
            {
                return RefactoringStatus::fatal(format!("{old_file} does not exist."));
            }
        }

        if sessions.len() > 1 {
            return RefactoringStatus::fatal("Cannot move files from multiple analysis sessions");
        }

        RefactoringStatus::new()
    }

    pub fn check_all_conditions(&self, host: &mut dyn RefactoringHost) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        result.add_status(self.check_initial_conditions());
        if result.has_fatal_error() {
            return result;
        }
        result.add_status(self.check_final_conditions(host));
        result
    }

    pub fn create_change(&self, host: &mut dyn RefactoringHost) -> SourceChange {
        let scope = host.search_scope();
        let all_files: Vec<(usize, String)> = scope
            .iter()
            .flat_map(|(ctx_idx, files)| files.iter().map(move |f| (*ctx_idx, f.clone())))
            .collect();

        // 1. Expand folder mappings to flat file mappings
        let mut resolved_mapping: IndexMap<String, String> = IndexMap::new();
        for (old_path, new_path) in &self.rename_mapping {
            let norm_old = paths::normalize(old_path);
            let norm_new = paths::normalize(new_path);
            let mut matched_children = false;
            for (_, f) in &all_files {
                if let Some(rel) = paths::relative_if_within(&norm_old, f) {
                    matched_children = true;
                    resolved_mapping
                        .insert(f.clone(), paths::normalize(&paths::join(&norm_new, rel)));
                }
            }
            if !matched_children {
                resolved_mapping.insert(norm_old, norm_new);
            }
        }

        // 2. Collect all source references
        let mut references_to_update: Vec<FileReference> = Vec::new();
        let mut seen_refs: HashSet<FileReference> = HashSet::new();

        let mut record_ref = |r: FileReference| {
            if seen_refs.insert(r.clone()) {
                references_to_update.push(r);
            }
        };

        for (old_path, new_path) in &resolved_mapping {
            let old_dir = paths::dirname(old_path).to_string();
            let new_dir = paths::dirname(new_path).to_string();

            if let Some(resolved_old) = host.resolved_unit_in(old_path, None) {
                // If defining unit of a library, check inbound relative `part of` directives in parts
                if resolved_old.index == 0 {
                    for (idx, part_unit) in resolved_old.library.library.units.iter().enumerate() {
                        if idx == 0 {
                            continue;
                        }
                        let part_path = resolved_old.library.inputs[idx].path.to_string();
                        let ast = &part_unit.ast;
                        for &d in ast.list_raw(ast[part_unit.unit].directives) {
                            if let Some(pod) = ast.cast::<PartOfDirective>(d)
                                && let Some(uri_lit) = ast[pod].uri
                                && let Some(ssl) = ast.cast::<SimpleStringLiteral>(uri_lit.raw())
                            {
                                let uri_val = &*ast[ssl].value;
                                if is_relative_uri(uri_val) {
                                    let quoted = ast.tokens.lexeme(ast[ssl].literal).to_string();
                                    record_ref(FileReference {
                                        offset: ast.offset(ssl.raw()),
                                        length: ast.length(ssl.raw()),
                                        source_file: part_path.clone(),
                                        target_file: old_path.clone(),
                                        quoted_uri_value: quoted,
                                    });
                                }
                            }
                        }
                    }
                }

                // If folder changes, outbound relative directives in old_path need updating
                if new_dir != old_dir {
                    let unit_old = resolved_old.unit();
                    let ast = &unit_old.ast;
                    for &d in ast.list_raw(ast[unit_old.unit].directives) {
                        if let Some(ssl) = directive_simple_uri(ast, d) {
                            let uri_val = &*ast[ssl].value;
                            if is_relative_uri(uri_val) {
                                let target = paths::normalize(&paths::join(&old_dir, uri_val));
                                let quoted = ast.tokens.lexeme(ast[ssl].literal).to_string();
                                record_ref(FileReference {
                                    offset: ast.offset(ssl.raw()),
                                    length: ast.length(ssl.raw()),
                                    source_file: old_path.clone(),
                                    target_file: target,
                                    quoted_uri_value: quoted,
                                });
                            }
                        }
                    }
                }
            }

            // Locate incoming references across workspace files
            for (ctx_idx, f) in &all_files {
                let Some(resolved_f) = host.resolved_unit_in(f, Some(*ctx_idx)) else {
                    continue;
                };
                let f_dir = paths::dirname(f);
                let pkg_resolved = |uri_val: &str| -> Option<String> {
                    if uri_val.starts_with("package:") {
                        host.collection()
                            .and_then(|c| c.contexts.get(*ctx_idx))
                            .and_then(|ctx| ctx.packages.resolve_package_uri(uri_val))
                    } else if is_relative_uri(uri_val) {
                        Some(paths::normalize(&paths::join(f_dir, uri_val)))
                    } else {
                        None
                    }
                };

                let unit_f = resolved_f.unit();
                let ast_f = &unit_f.ast;
                for &d in ast_f.list_raw(ast_f[unit_f.unit].directives) {
                    if ast_f.is::<PartOfDirective>(d) {
                        continue;
                    }
                    let Some(ssl) = directive_simple_uri(ast_f, d) else {
                        continue;
                    };
                    let uri_val = &*ast_f[ssl].value;
                    if pkg_resolved(uri_val).as_deref() == Some(old_path.as_str()) {
                        let quoted = ast_f.tokens.lexeme(ast_f[ssl].literal).to_string();
                        record_ref(FileReference {
                            offset: ast_f.offset(ssl.raw()),
                            length: ast_f.length(ssl.raw()),
                            source_file: f.clone(),
                            target_file: old_path.clone(),
                            quoted_uri_value: quoted,
                        });
                    }
                }
            }
        }

        // 3. Apply updates per source file
        let mut acc = ChangeAcc::default();
        for r in &references_to_update {
            let new_source = resolved_mapping
                .get(&r.source_file)
                .unwrap_or(&r.source_file);
            let new_target = resolved_mapping
                .get(&r.target_file)
                .unwrap_or(&r.target_file);
            let (start_quote, end_quote, uri_value) = extract_quotes(&r.quoted_uri_value);
            let new_uri = compute_new_uri(host.collection(), new_source, new_target, uri_value);
            if new_uri != uri_value {
                acc.add(
                    &r.source_file,
                    r.offset,
                    r.length,
                    &format!("{start_quote}{new_uri}{end_quote}"),
                );
            }
        }

        acc.into_change(self.refactoring_name().to_string())
    }
}

fn directive_simple_uri(ast: &Ast, d: NodeId) -> Option<Id<SimpleStringLiteral>> {
    let uri_node = if let Some(imp) = ast.cast::<ImportDirective>(d) {
        Some(ast[imp].uri.raw())
    } else if let Some(exp) = ast.cast::<ExportDirective>(d) {
        Some(ast[exp].uri.raw())
    } else if let Some(part) = ast.cast::<PartDirective>(d) {
        Some(ast[part].uri.raw())
    } else if let Some(pod) = ast.cast::<PartOfDirective>(d) {
        ast[pod].uri.map(|u| u.raw())
    } else {
        None
    }?;
    ast.cast::<SimpleStringLiteral>(uri_node)
}

fn is_relative_uri(uri: &str) -> bool {
    if uri.is_empty() || uri.starts_with('/') {
        return false;
    }
    if let Some((scheme, _)) = uri.split_once(':')
        && !scheme.is_empty()
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
    {
        return false;
    }
    true
}

fn compute_new_uri(
    collection: Option<&dartr_project::AnalysisContextCollection>,
    source_file: &str,
    target_file: &str,
    current_uri_value: &str,
) -> String {
    if current_uri_value.starts_with("package:")
        && let Some(coll) = collection
        && let Some(ctx) = coll
            .context_for(source_file)
            .or_else(|| coll.contexts.first())
        && let Some(pkg_uri) = ctx.packages.path_to_package_uri(target_file)
    {
        return pkg_uri;
    }
    paths::relative(target_file, paths::dirname(source_file))
}

fn extract_quotes(quoted: &str) -> (&str, &str, &str) {
    let start_len = if quoted.starts_with("r'''") || quoted.starts_with("r\"\"\"") {
        4
    } else if quoted.starts_with("'''") || quoted.starts_with("\"\"\"") {
        3
    } else if quoted.starts_with("r'") || quoted.starts_with("r\"") {
        2
    } else if quoted.starts_with('\'') || quoted.starts_with('"') {
        1
    } else {
        0
    };
    let end_len = if quoted.ends_with("'''") || quoted.ends_with("\"\"\"") {
        3
    } else if quoted.ends_with('\'') || quoted.ends_with('"') {
        1
    } else {
        0
    };
    let start_idx = start_len.min(quoted.len());
    let end_idx = quoted.len().saturating_sub(end_len).max(start_idx);
    (
        &quoted[..start_idx],
        &quoted[end_idx..],
        &quoted[start_idx..end_idx],
    )
}
