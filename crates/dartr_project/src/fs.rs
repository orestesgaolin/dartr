//! Physical file system access with the semantics of the analyzer's
//! `PhysicalResourceProvider` (`package:analyzer/file_system/physical_file_system.dart`).
//!
//! - `File.exists` is `true` only for files (symbolic links are followed).
//! - `Folder.exists` is `true` only for directories.
//! - `getResource` returns a folder for an existing directory, otherwise a
//!   file (also for paths that do not exist).
//! - `getChildren` lists in directory order, follows links, and skips broken
//!   links and other entities.

use std::fs;

/// The kind of a resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceKind {
    File,
    Folder,
}

/// A child of a folder.
#[derive(Clone, Debug)]
pub struct Child {
    pub path: String,
    pub kind: ResourceKind,
}

/// Returns `true` if [path] is an existing file.
pub fn file_exists(path: &str) -> bool {
    fs::metadata(path).map(|m| m.is_file()).unwrap_or(false)
}

/// Returns `true` if [path] is an existing directory.
pub fn folder_exists(path: &str) -> bool {
    fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
}

/// Returns the kind of the resource at [path], like `getResource`.
pub fn resource_kind(path: &str) -> ResourceKind {
    if folder_exists(path) {
        ResourceKind::Folder
    } else {
        ResourceKind::File
    }
}

/// Reads the file at [path] as a string.
pub fn read_string(path: &str) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    Some(match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => String::from_utf8_lossy(error.as_bytes()).into_owned(),
    })
}

/// Lists the children of the folder at [path], or `None` if it cannot be
/// read.
pub fn children(path: &str) -> Option<Vec<Child>> {
    let entries = fs::read_dir(path).ok()?;
    let mut result = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let child_path = crate::paths::join(path, &name);
        // `Directory.listSync()` follows links: the entity type is the type of
        // the link target. Broken links are `Link` entities and are skipped.
        let Ok(metadata) = fs::metadata(&child_path) else {
            continue;
        };
        if metadata.is_dir() {
            result.push(Child {
                path: child_path,
                kind: ResourceKind::Folder,
            });
        } else if metadata.is_file() {
            result.push(Child {
                path: child_path,
                kind: ResourceKind::File,
            });
        }
    }
    Some(result)
}

/// Resolves symbolic links in [path], like `resolveSymbolicLinksSync`.
pub fn canonicalize(path: &str) -> Option<String> {
    fs::canonicalize(path)
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}
