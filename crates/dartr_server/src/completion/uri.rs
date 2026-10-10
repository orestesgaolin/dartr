// Dart source: pkg/analysis_server/lib/src/services/completion/dart/uri_helper.dart

//! The URI suggestions in directives (Dart `UriHelper`).

use dartr_ast::*;
#[allow(unused_imports)]
use dartr_typesystem::TypeExt;

use super::candidate::{Candidate, Kind};
use super::target::{TokenExt, string_contents_range};
use super::{Out, Request};

/// Dart `UriHelper.addSuggestions`.
pub fn add_uri_suggestions(q: &Request<'_, '_>, out: &mut Out, node: NodeId) {
    let ast = q.ast;
    let Some(s) = ast.cast::<SimpleStringLiteral>(node) else {
        return;
    };
    let offset = q.offset;
    let start = ast.offset(node);
    let end = ast.end(node);
    let content: Vec<u16> = q.content.encode_utf16().collect();
    let last_is_quote = |end: u32| {
        content
            .get(end as usize - 1)
            .is_some_and(|c| *c == b'"' as u16 || *c == b'\'' as u16)
    };
    if offset > start {
        if offset < end {
            simple_string_literal(q, out, s);
        } else if offset == end {
            if end == start + 1 {
                simple_string_literal(q, out, s);
            } else if end as usize == content.len() && !last_is_quote(end) {
                simple_string_literal(q, out, s);
            }
        }
    } else if offset == start
        && offset == end
        && end as usize == content.len()
        && end > 0
        && last_is_quote(end)
    {
        simple_string_literal(q, out, s);
    }
}

fn suggest_uri(out: &mut Out, uri: &str) {
    let score = out.score(uri);
    if score != -1.0 {
        out.add(Candidate::new(Kind::Uri(uri.to_string()), score));
    }
}

/// Dart `_extractPartialUri`.
fn partial_uri(q: &Request<'_, '_>, s: Id<SimpleStringLiteral>) -> Option<String> {
    let ast = q.ast;
    let (contents, _) = string_contents_range(ast, s);
    if q.offset < contents {
        return None;
    }
    let lexeme = ast.t_lexeme(ast[s].literal);
    let node_offset = ast.offset(s);
    let units: Vec<u16> = lexeme.encode_utf16().collect();
    let from = (contents - node_offset) as usize;
    let to = ((q.offset - node_offset) as usize).min(units.len());
    Some(String::from_utf16_lossy(&units[from.min(to)..to]))
}

/// Dart `_simpleStringLiteral`.
fn simple_string_literal(q: &Request<'_, '_>, out: &mut Out, s: Id<SimpleStringLiteral>) {
    let ast = q.ast;
    let Some(parent) = ast.parent(s) else {
        return;
    };
    if ast.is::<Configuration>(parent) || ast.is::<NamespaceDirective>(parent) {
        if let Some(partial) = partial_uri(q, s) {
            add_dart_suggestions(q, out);
            add_package_suggestions(q, out, &partial);
            add_file_suggestions(q, out, &partial);
        }
    } else if ast.is::<PartDirective>(parent) || ast.is::<PartOfDirective>(parent) {
        if let Some(partial) = partial_uri(q, s) {
            add_file_suggestions(q, out, &partial);
        }
    }
}

/// Dart `_addDartSuggestions`.
fn add_dart_suggestions(q: &Request<'_, '_>, out: &mut Out) {
    suggest_uri(out, "dart:");
    for (name, internal, implementation) in &q.sdk_libraries {
        if !internal && !implementation && !name.starts_with("dart:_") {
            suggest_uri(out, name);
        }
    }
}

/// The children of a folder (Dart `Folder.getChildren`).
fn children(folder: &str) -> Vec<dartr_project::fs::Child> {
    dartr_project::fs::children(folder).unwrap_or_default()
}

/// Dart `_addPackageFolderSuggestions`.
fn add_package_folder_suggestions(out: &mut Out, partial: &str, prefix: &str, folder: &str) {
    for child in children(folder) {
        let short = child.path.rsplit('/').next().unwrap_or("").to_string();
        match child.kind {
            dartr_project::fs::ResourceKind::Folder => {
                let child_prefix = format!("{prefix}{short}/");
                suggest_uri(out, &child_prefix);
                if partial.starts_with(&child_prefix) {
                    add_package_folder_suggestions(out, partial, &child_prefix, &child.path);
                }
            }
            dartr_project::fs::ResourceKind::File => {
                if child.path.ends_with(".dart") {
                    suggest_uri(out, &format!("{prefix}{short}"));
                }
            }
        }
    }
}

/// Dart `_addPackageSuggestions`.
///
/// The package map of the source factory of Dart is never null for a
/// context of the server (a context without a package config has an empty
/// map), so `package:` is always suggested.
fn add_package_suggestions(q: &Request<'_, '_>, out: &mut Out, partial: &str) {
    suggest_uri(out, "package:");
    for (name, lib) in &q.packages {
        let prefix = format!("package:{name}/");
        suggest_uri(out, &prefix);
        if std::path::Path::new(lib).is_dir() {
            add_package_folder_suggestions(out, partial, &prefix, lib);
        }
    }
}

/// Dart `_addFileSuggestions`.
fn add_file_suggestions(q: &Request<'_, '_>, out: &mut Out, partial: &str) {
    let parent_uri = if partial.ends_with('/') {
        partial.to_string()
    } else {
        let dir = match partial.rfind('/') {
            Some(0) => "/".to_string(),
            Some(i) => partial[..i].to_string(),
            None => ".".to_string(),
        };
        if dir != "." && !dir.ends_with('/') {
            format!("{dir}/")
        } else {
            dir
        }
    };
    let uri_prefix = if parent_uri == "." {
        String::new()
    } else {
        parent_uri.clone()
    };
    let has_scheme = parent_uri.find(':').is_some_and(|i| {
        parent_uri[..i]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
            && i > 0
    });
    if !parent_uri.starts_with("file://") && has_scheme {
        return;
    }
    let source_dir = std::path::Path::new(q.path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    let raw = parent_uri.strip_prefix("file://").unwrap_or(&parent_uri);
    let mut dir = std::path::PathBuf::from(raw);
    if dir.is_relative() {
        dir = normalize(&source_dir.join(&dir));
        let src_in_lib = source_dir.components().any(|c| c.as_os_str() == "lib");
        let dst_in_lib = dir.components().any(|c| c.as_os_str() == "lib");
        if src_in_lib && !dst_in_lib {
            return;
        }
    } else {
        dir = normalize(&dir);
    }
    let source_short = std::path::Path::new(q.path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let Some(dir) = dir.to_str() else {
        return;
    };
    if !std::path::Path::new(dir).is_dir() {
        return;
    }
    for child in children(dir) {
        let short = child.path.rsplit('/').next().unwrap_or("").to_string();
        let completion = match child.kind {
            dartr_project::fs::ResourceKind::Folder => {
                (!short.starts_with('.')).then(|| format!("{uri_prefix}{short}/"))
            }
            dartr_project::fs::ResourceKind::File => short
                .ends_with(".dart")
                .then(|| format!("{uri_prefix}{short}")),
        };
        if let Some(c) = completion {
            if c != source_short {
                suggest_uri(out, &c);
            }
        }
    }
}

/// `path.normalize`.
fn normalize(path: &std::path::Path) -> std::path::PathBuf {
    let mut out = std::path::PathBuf::new();
    for c in path.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}
