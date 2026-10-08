//! Context discovery: which context roots analyze the included paths.
//!
//! Ports `pkg/analyzer/lib/src/dart/analysis/context_locator.dart`
//! (`locateContextRoots`, `_ContextLocator`).

use crate::analysis_options::OptionsParseSession;
use crate::context_root::{ContextRoot, LocatedGlob, Resource};
use crate::glob::Glob;
use crate::package_config::{self, Packages};
use crate::pubspec::Pubspec;
use crate::workspace::{DEFAULT_OPTIONS_URI, Workspace};
use crate::{fs, paths};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Returns the context roots that analyze the [included_paths] (absolute,
/// normalized). If [options_file] is given, it is used instead of the
/// `analysis_options.yaml` files in the folders; if [package_config_file] is
/// given, it is used instead of the `.dart_tool/package_config.json` files.
pub fn locate_context_roots(
    included_paths: &[String],
    options_file: Option<&str>,
    package_config_file: Option<&str>,
    session: &OptionsParseSession,
) -> Vec<ContextRoot> {
    let default_options_file = options_file
        .filter(|f| fs::file_exists(f))
        .map(str::to_string);
    let default_package_config_file = package_config_file
        .filter(|f| fs::file_exists(f))
        .map(str::to_string);

    // `_resourcesFromPaths`: unique paths, shorter paths first.
    let mut unique: Vec<String> = Vec::new();
    for path in included_paths {
        if !unique.contains(path) {
            unique.push(path.clone());
        }
    }
    unique.sort_by_key(|p| p.len());
    let mut folders = Vec::new();
    let mut files = Vec::new();
    for path in unique {
        match fs::resource_kind(&path) {
            fs::ResourceKind::Folder => folders.push(path),
            fs::ResourceKind::File => files.push(path),
        }
    }

    let mut locator = ContextLocator {
        session,
        default_options_file,
        default_package_config_file,
        roots: Vec::new(),
        globs: HashMap::new(),
    };
    locator.locate_roots(&folders, &files);
    locator.roots
}

struct RootLocation {
    root_folder: String,
    workspace: Workspace,
    options_file: Option<String>,
    package_config_file: Option<String>,
}

struct ContextLocator<'a> {
    session: &'a OptionsParseSession,
    default_options_file: Option<String>,
    default_package_config_file: Option<String>,
    roots: Vec<ContextRoot>,
    /// Compiled globs, by pattern.
    globs: HashMap<String, Option<Rc<Glob>>>,
}

fn package_config_file_in(folder: &str) -> Option<String> {
    let file = paths::join(folder, ".dart_tool/package_config.json");
    fs::file_exists(&file).then_some(file)
}

fn existing_file(folder: &str, name: &str) -> Option<String> {
    let file = paths::join(folder, name);
    fs::file_exists(&file).then_some(file)
}

fn find_analysis_options_yaml_file(folder: &str) -> Option<String> {
    paths::with_ancestors(folder)
        .find_map(|current| existing_file(current, "analysis_options.yaml"))
}

/// `_lowest`: the deepest of folders on one path to the file system root.
fn lowest(folders: &[Option<String>]) -> Option<String> {
    let mut result: Option<String> = None;
    for folder in folders {
        match (&result, folder) {
            (None, _) => result = folder.clone(),
            (Some(current), Some(folder)) if paths::is_within(current, folder) => {
                result = Some(folder.clone())
            }
            _ => {}
        }
    }
    result
}

impl ContextLocator<'_> {
    /// `_contextRootLocation`.
    fn context_root_location(&self, parent: &str, default_root_folder: &str) -> RootLocation {
        let mut options_folder_to_choose_root = None;
        let options_file = match &self.default_options_file {
            Some(file) => Some(file.clone()),
            None => {
                let file = find_analysis_options_yaml_file(parent);
                options_folder_to_choose_root =
                    file.as_deref().map(|f| paths::dirname(f).to_string());
                file
            }
        };

        let mut package_config_file = None;
        let mut packages_folder_to_choose_root = None;
        if let Some(file) = &self.default_package_config_file {
            package_config_file = Some(file.clone());
            let parent = paths::dirname(file);
            packages_folder_to_choose_root = Some(
                paths::with_ancestors(parent)
                    .find(|current| package_config_file_in(current).is_some())
                    .unwrap_or(parent)
                    .to_string(),
            );
        }

        let build_gn_file =
            paths::with_ancestors(parent).find_map(|current| existing_file(current, "BUILD.gn"));

        let mut workspace = self.create_workspace(
            parent,
            package_config_file.as_deref(),
            build_gn_file.as_deref(),
        );

        let mut root_folder = lowest(&[
            options_folder_to_choose_root,
            build_gn_file
                .as_deref()
                .map(|f| paths::dirname(f).to_string()),
            (!workspace.is_basic()).then(|| workspace.root.clone()),
        ]);

        if let Some(file) = workspace.package_config_file() {
            if package_config_file.is_none() {
                package_config_file = Some(file.to_string());
            }
            if let (Some(root), Some(packages_folder)) =
                (&root_folder, &packages_folder_to_choose_root)
                && paths::is_within(packages_folder, root)
            {
                root_folder = Some(packages_folder.clone());
            }
        }

        let root_folder = match root_folder {
            Some(folder) => folder,
            None => {
                if workspace.is_basic() {
                    workspace = self.create_workspace(
                        default_root_folder,
                        package_config_file.as_deref(),
                        build_gn_file.as_deref(),
                    );
                }
                default_root_folder.to_string()
            }
        };

        RootLocation {
            root_folder,
            workspace,
            options_file,
            package_config_file,
        }
    }

    /// `_createContextRoot`.
    fn create_context_root(
        &mut self,
        root_folder: &str,
        options_file: Option<String>,
        location: &RootLocation,
    ) -> usize {
        let mut options_file = options_file;
        if location.workspace.has_default_analysis_options() && options_file.is_none() {
            options_file = location
                .workspace
                .resolve_package_uri(DEFAULT_OPTIONS_URI)
                .filter(|path| fs::file_exists(path));
        }
        let mut root = ContextRoot::new(
            root_folder,
            location.workspace.clone(),
            options_file.clone(),
            location.package_config_file.clone(),
        );
        if let Some(file) = &options_file {
            root.map_options_file(file, root_folder);
        }
        let globs = self.excluded_globs(options_file.as_deref(), &location.workspace);
        root.excluded_globs.extend(globs);
        self.roots.push(root);
        self.roots.len() - 1
    }

    /// `_createContextRoots`. Returns `true` if [folder] stays in the
    /// containing root, `false` if it got a new root.
    fn create_context_roots(
        &mut self,
        visited: &mut HashSet<String>,
        folder: &str,
        containing_root: usize,
        containing_root_plugins: &HashSet<String>,
        options_file_from_parent_in_same_root: Option<&str>,
    ) -> bool {
        let mut containing_root = containing_root;
        let mut containing_root_plugins = containing_root_plugins.clone();
        let package_config_file_to_use = self
            .default_package_config_file
            .clone()
            .or_else(|| package_config_file_in(folder))
            .or_else(|| self.roots[containing_root].packages_file.clone());
        let build_gn_file = existing_file(folder, "BUILD.gn");

        let mut options_file_to_use = self.default_options_file.clone();
        if options_file_to_use.is_none() {
            options_file_to_use = existing_file(folder, "analysis_options.yaml");
            if options_file_to_use.is_none() {
                options_file_to_use = options_file_from_parent_in_same_root.map(str::to_string);
            }
            if options_file_to_use.is_none() {
                let root = self.roots[containing_root].root.clone();
                let mut parent = paths::dirname(folder).to_string();
                while parent != root {
                    options_file_to_use = existing_file(&parent, "analysis_options.yaml");
                    if options_file_to_use.is_some() || parent == "/" {
                        break;
                    }
                    parent = paths::dirname(&parent).to_string();
                }
            }
        }

        let local_plugins = self.enabled_legacy_plugins(
            &self.roots[containing_root].workspace.clone(),
            options_file_to_use.as_deref(),
        );
        let plugins_differ =
            options_file_to_use.is_some() && containing_root_plugins != local_plugins;

        let mut used_this_root = true;
        if plugins_differ
            || package_config_file_to_use != self.roots[containing_root].packages_file
            || build_gn_file.is_some()
        {
            let workspace = self.create_workspace(
                folder,
                package_config_file_to_use.as_deref(),
                build_gn_file.as_deref(),
            );
            let options_file = options_file_to_use
                .clone()
                .or_else(|| self.roots[containing_root].options_file.clone());
            let mut root = ContextRoot::new(
                folder,
                workspace.clone(),
                options_file.clone(),
                package_config_file_to_use,
            );
            root.included.push(Resource::Folder(folder.to_string()));
            self.roots[containing_root]
                .excluded
                .push(folder.to_string());
            let globs = self.excluded_globs(options_file.as_deref(), &workspace);
            root.excluded_globs.extend(globs);
            self.roots.push(root);
            containing_root = self.roots.len() - 1;
            containing_root_plugins = local_plugins;
            used_this_root = false;
        }

        if let Some(options_file) = &options_file_to_use
            && Some(options_file.as_str()) != options_file_from_parent_in_same_root
        {
            self.roots[containing_root].map_options_file(options_file, folder);
            let workspace = self.roots[containing_root].workspace.clone();
            let globs = self.excluded_globs(Some(options_file), &workspace);
            self.roots[containing_root].excluded_globs.extend(globs);
        }

        let options_for_folder = if used_this_root {
            options_file_to_use.clone()
        } else {
            None
        };
        self.create_context_roots_in(
            visited,
            folder,
            containing_root,
            &containing_root_plugins,
            options_for_folder.as_deref(),
        );
        used_this_root
    }

    /// `_createContextRootsIn`.
    fn create_context_roots_in(
        &mut self,
        visited: &mut HashSet<String>,
        folder: &str,
        containing_root: usize,
        containing_root_plugins: &HashSet<String>,
        options_file_to_use_for_folder: Option<&str>,
    ) {
        let Some(canonical) = fs::canonicalize(folder) else {
            return;
        };
        if !visited.insert(canonical) {
            return;
        }
        let Some(children) = fs::children(folder) else {
            return;
        };
        for child in children {
            if child.kind != fs::ResourceKind::Folder {
                continue;
            }
            let excluded = paths::basename(&child.path).starts_with('.')
                || self.roots[containing_root]
                    .excluded_globs
                    .iter()
                    .any(|g| g.matches(&child.path));
            if !excluded {
                self.create_context_roots(
                    visited,
                    &child.path,
                    containing_root,
                    containing_root_plugins,
                    options_file_to_use_for_folder,
                );
            }
        }
    }

    /// `_createWorkspace`.
    fn create_workspace(
        &self,
        folder: &str,
        package_config_file: Option<&str>,
        build_gn_file: Option<&str>,
    ) -> Workspace {
        if let Some(build_gn_file) = build_gn_file
            && let Some(workspace) = Workspace::find_gn(build_gn_file)
        {
            return workspace;
        }
        // `Packages.empty` (the identity) when there is no file, or when it
        // cannot be parsed; a package config workspace then reads its own file.
        let packages: Option<Packages> = package_config_file.and_then(|file| {
            let content = fs::read_string(file)?;
            package_config::parse_package_config(&content, file).ok()
        });
        let blaze = Workspace::find_blaze(folder);
        let package_config = Workspace::find_package_config(packages.clone(), folder);
        let most_specific = match (blaze, package_config) {
            (None, second) => second,
            (first, None) => first,
            (Some(first), Some(second)) => {
                if paths::is_within(&first.root, &second.root) {
                    Some(second)
                } else {
                    Some(first)
                }
            }
        };
        most_specific.unwrap_or_else(|| Workspace::basic(packages.unwrap_or_default(), folder))
    }

    /// `_getEnabledLegacyPlugins`.
    fn enabled_legacy_plugins(
        &self,
        workspace: &Workspace,
        options_file: Option<&str>,
    ) -> HashSet<String> {
        let Some(options_file) = options_file else {
            return HashSet::new();
        };
        let result = self.session.parse(workspace, options_file);
        result
            .options
            .enabled_legacy_plugin_names
            .into_iter()
            .collect()
    }

    fn glob(&mut self, pattern: &str) -> Option<Rc<Glob>> {
        self.globs
            .entry(pattern.to_string())
            .or_insert_with(|| Glob::new(pattern).ok().map(Rc::new))
            .clone()
    }

    /// `_getExcludedGlobs`: the `exclude:` patterns of [options_file]; a
    /// pattern that ends with `/**` also excludes the folder itself.
    fn excluded_globs(
        &mut self,
        options_file: Option<&str>,
        workspace: &Workspace,
    ) -> Vec<LocatedGlob> {
        let Some(options_file) = options_file else {
            return Vec::new();
        };
        let result = self.session.parse(workspace, options_file);
        let parent = paths::dirname(options_file).to_string();
        let mut globs = Vec::new();
        for excluded_path in &result.options.exclude_patterns {
            let components = posix_split(excluded_path);
            if let Some(glob) = self.glob(&posix_join(&components)) {
                globs.push(LocatedGlob {
                    parent: parent.clone(),
                    glob,
                });
            }
            if components.len() > 1 && components.last().map(String::as_str) == Some("**") {
                let trimmed = &components[..components.len() - 1];
                if let Some(glob) = self.glob(&posix_join(trimmed)) {
                    globs.push(LocatedGlob {
                        parent: parent.clone(),
                        glob,
                    });
                }
            }
        }
        globs
    }

    /// `_loadWorkspaceDetailsFromPubspec`: the existing folders listed in the
    /// `workspace:` entry of the `pubspec.yaml` in [root].
    fn workspace_folders_from_pubspec(&self, root: &str) -> Vec<String> {
        let mut result = Vec::new();
        let pubspec = paths::join(root, "pubspec.yaml");
        if let Some(pubspec) = fs::file_exists(&pubspec)
            .then(|| Pubspec::read(&pubspec))
            .flatten()
        {
            for entry in pubspec.workspace.unwrap_or_default().into_iter().flatten() {
                let child = paths::normalize(&paths::join(root, &entry));
                if fs::folder_exists(&child) && !result.contains(&child) {
                    result.push(child);
                }
            }
        }
        result
    }

    /// `_sortIncludedFoldersIntoWorkspaceResolutions`.
    fn sort_into_workspace_resolutions(
        &self,
        included_folders: &[String],
    ) -> (Vec<(String, Vec<String>)>, Vec<String>) {
        let mut workspace_map: Vec<(String, Vec<String>)> = Vec::new();
        let mut non_workspace: Vec<String> = Vec::new();
        let mut known_by_root: HashMap<String, Vec<String>> = HashMap::new();
        for folder in included_folders {
            let location = self.context_root_location(folder, folder);
            let workspace_root = location.workspace.root.clone();
            let mut added = false;
            if *folder == workspace_root {
                known_by_root.insert(workspace_root.clone(), Vec::new());
                if let Some((_, folders)) = workspace_map
                    .iter()
                    .find(|(root, _)| *root == workspace_root)
                {
                    non_workspace.extend(folders.iter().cloned());
                }
            } else {
                let pubspec = paths::join(folder, "pubspec.yaml");
                if fs::file_exists(&pubspec) {
                    let pubspec = Pubspec::read(&pubspec).unwrap_or_default();
                    if pubspec.resolution_text() == Some("workspace") {
                        let known =
                            known_by_root
                                .entry(workspace_root.clone())
                                .or_insert_with(|| {
                                    self.workspace_folders_from_pubspec(&workspace_root)
                                });
                        if known.contains(folder) {
                            match workspace_map
                                .iter_mut()
                                .find(|(root, _)| *root == workspace_root)
                            {
                                Some((_, folders)) => folders.push(folder.clone()),
                                None => workspace_map
                                    .push((workspace_root.clone(), vec![folder.clone()])),
                            }
                            added = true;
                        }
                    }
                }
            }
            if !added {
                non_workspace.push(folder.clone());
            }
        }
        (workspace_map, non_workspace)
    }

    fn match_root_with_location(root: &ContextRoot, location: &RootLocation) -> bool {
        if root.options_file != location.options_file {
            return false;
        }
        if root.packages_file != location.package_config_file {
            return false;
        }
        if !location.workspace.is_basic() && root.workspace.root != location.workspace.root {
            return false;
        }
        true
    }

    /// `_locateRoots`.
    fn locate_roots(&mut self, included_folders: &[String], included_files: &[String]) {
        let (workspace_map, non_workspace) = self.sort_into_workspace_resolutions(included_folders);

        for (workspace_root, workspace_folders) in workspace_map {
            let location = self.context_root_location(&workspace_root, &workspace_root);
            let root =
                self.create_context_root(&workspace_root, location.options_file.clone(), &location);
            let root_plugins =
                self.enabled_legacy_plugins(&location.workspace, location.options_file.as_deref());
            let mut visited = HashSet::new();
            let mut used_root = false;
            for folder in &workspace_folders {
                if !self.roots[root].is_analyzed(folder) {
                    self.roots[root]
                        .included
                        .push(Resource::Folder(folder.clone()));
                }
                used_root |=
                    self.create_context_roots(&mut visited, folder, root, &root_plugins, None);
            }
            if !used_root {
                self.roots.remove(root);
            }
        }

        for folder in &non_workspace {
            let location = self.context_root_location(folder, folder);
            let mut root: Option<usize> = None;
            for index in 0..self.roots.len() {
                let existing = &self.roots[index];
                if paths::is_or_within(&existing.root, folder) {
                    if Self::match_root_with_location(existing, &location) {
                        root = Some(index);
                        break;
                    } else if !self.roots[index].excluded.contains(folder) {
                        self.roots[index].excluded.push(folder.clone());
                    }
                }
            }
            let root = match root {
                Some(root) => root,
                None => self.create_context_root(folder, location.options_file.clone(), &location),
            };
            if !self.roots[root].is_analyzed(folder) {
                self.roots[root]
                    .included
                    .push(Resource::Folder(folder.clone()));
            }
            let root_plugins =
                self.enabled_legacy_plugins(&location.workspace, location.options_file.as_deref());
            self.create_context_roots_in(&mut HashSet::new(), folder, root, &root_plugins, None);
        }

        for file in included_files {
            let parent = paths::dirname(file).to_string();
            let location = self.context_root_location(&parent, "/");
            let root = self.roots.iter().position(|existing| {
                paths::is_or_within(&existing.root, file)
                    && Self::match_root_with_location(existing, &location)
            });
            let root = match root {
                Some(root) => root,
                None => {
                    let root_folder = location.root_folder.clone();
                    self.create_context_root(&root_folder, location.options_file.clone(), &location)
                }
            };
            if !self.roots[root].is_analyzed(file) {
                self.roots[root].included.push(Resource::File(file.clone()));
            }
        }
    }
}

/// `posix.split` of `package:path`.
fn posix_split(path: &str) -> Vec<String> {
    paths::split(path).into_iter().map(str::to_string).collect()
}

/// `posix.joinAll` of `package:path`.
fn posix_join(components: &[String]) -> String {
    let mut result = String::new();
    for component in components {
        if paths::is_absolute(component) {
            result = component.clone();
        } else if result.is_empty() || result.ends_with('/') {
            result.push_str(component);
        } else {
            result.push('/');
            result.push_str(component);
        }
    }
    result
}
