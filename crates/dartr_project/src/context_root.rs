//! A context root: the folder of an analysis context, the included and
//! excluded resources, and the analyzed files.
//!
//! Ports `pkg/analyzer/lib/src/dart/analysis/context_root.dart`
//! (`ContextRootImpl`, `LocatedGlob`).

use crate::glob::Glob;
use crate::workspace::Workspace;
use crate::{fs, paths};
use std::collections::HashSet;
use std::rc::Rc;

/// A file or a folder.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Resource {
    File(String),
    Folder(String),
}

impl Resource {
    pub fn path(&self) -> &str {
        match self {
            Resource::File(path) | Resource::Folder(path) => path,
        }
    }
}

/// A glob that applies to the resources inside [parent] (`LocatedGlob`).
#[derive(Clone, Debug)]
pub struct LocatedGlob {
    pub parent: String,
    pub glob: Rc<Glob>,
}

impl LocatedGlob {
    /// Matches the path relative to [parent]; `false` if [path] is not inside.
    pub fn matches(&self, path: &str) -> bool {
        match paths::relative_if_within(&self.parent, path) {
            Some(relative) => self.glob.matches(relative),
            None => false,
        }
    }
}

/// A context root (`ContextRootImpl`).
#[derive(Clone, Debug)]
pub struct ContextRoot {
    /// The root folder.
    pub root: String,
    pub workspace: Workspace,
    /// Included files and folders.
    pub included: Vec<Resource>,
    /// Excluded folders (absolute paths).
    pub excluded: Vec<String>,
    /// Globs of the `exclude:` patterns of the options files.
    pub excluded_globs: Vec<LocatedGlob>,
    /// The options file of the root, if any.
    pub options_file: Option<String>,
    /// For each options file, the folders that use it, in insertion order.
    pub options_file_map: Vec<(String, Vec<String>)>,
    /// The package config file, if any.
    pub packages_file: Option<String>,
}

impl ContextRoot {
    pub fn new(
        root: &str,
        workspace: Workspace,
        options_file: Option<String>,
        packages_file: Option<String>,
    ) -> Self {
        ContextRoot {
            root: root.to_string(),
            workspace,
            included: Vec::new(),
            excluded: Vec::new(),
            excluded_globs: Vec::new(),
            options_file,
            options_file_map: Vec::new(),
            packages_file,
        }
    }

    /// Adds [folder] to the folders that use [options_file].
    pub fn map_options_file(&mut self, options_file: &str, folder: &str) {
        match self
            .options_file_map
            .iter_mut()
            .find(|(file, _)| file == options_file)
        {
            Some((_, folders)) => {
                if !folders.iter().any(|f| f == folder) {
                    folders.push(folder.to_string());
                }
            }
            None => self
                .options_file_map
                .push((options_file.to_string(), vec![folder.to_string()])),
        }
    }

    pub fn included_paths(&self) -> impl Iterator<Item = &str> {
        self.included.iter().map(Resource::path)
    }

    /// The absolute paths of all analyzed files, in directory order
    /// (`ContextRootImpl.analyzedFiles`). Contains every file, not only Dart
    /// files.
    pub fn analyzed_files(&self) -> Vec<String> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        for included in &self.included {
            let path = included.path();
            match fs::resource_kind(path) {
                fs::ResourceKind::File => result.push(path.to_string()),
                fs::ResourceKind::Folder => {
                    self.included_files_in_folder(&mut visited, path, path, &mut result)
                }
            }
        }
        result
    }

    fn included_files_in_folder(
        &self,
        visited: &mut HashSet<String>,
        folder: &str,
        included_path: &str,
        result: &mut Vec<String>,
    ) {
        let Some(children) = fs::children(folder) else {
            return;
        };
        for child in children {
            if self.is_excluded(&child.path, included_path) {
                continue;
            }
            match child.kind {
                fs::ResourceKind::File => result.push(child.path),
                fs::ResourceKind::Folder => {
                    let Some(canonical) = fs::canonicalize(&child.path) else {
                        return;
                    };
                    if visited.insert(canonical.clone()) {
                        self.included_files_in_folder(visited, &child.path, included_path, result);
                        visited.remove(&canonical);
                    }
                }
            }
        }
    }

    /// Returns `true` if [path] is analyzed in this context
    /// (`ContextRootImpl.isAnalyzed`).
    pub fn is_analyzed(&self, path: &str) -> bool {
        for included in &self.included {
            match included {
                Resource::File(file) => {
                    if file == path {
                        return true;
                    }
                }
                Resource::Folder(folder) => {
                    if paths::is_or_within(folder, path) && !self.is_excluded(path, folder) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// `ContextRootImpl._isExcluded`: hidden (dot) names below the root,
    /// excluded folders, and exclude globs (except globs that match the
    /// explicitly [included_path]).
    pub fn is_excluded(&self, path: &str, included_path: &str) -> bool {
        let mut current = path;
        while paths::is_within(&self.root, current) {
            if paths::basename(current).starts_with('.') {
                return true;
            }
            current = paths::dirname(current);
        }
        for excluded in &self.excluded {
            if path == excluded || paths::is_within(excluded, path) {
                return true;
            }
        }
        for pattern in &self.excluded_globs {
            if !pattern.matches(included_path) && pattern.matches(path) {
                return true;
            }
        }
        false
    }
}
