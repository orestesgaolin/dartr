//! Workspaces: how files map to packages and URIs.
//!
//! Ports `pkg/analyzer/lib/src/workspace/` (`workspace.dart`, `simple.dart`,
//! `basic.dart`, `pub.dart`, and the discovery parts of `blaze.dart` and
//! `gn.dart`) and the URI resolvers that the workspaces use
//! (`PackageMapUriResolver`, `PackageConfigPackageUriResolver`,
//! `PackageConfigFileUriResolver`, `ResourceUriResolver`, `DartUriResolver`).
//!
//! Blaze and GN workspaces are detected, so that context roots match the
//! analyzer, but their package resolution is limited: a Blaze workspace
//! resolves `package:a.b/x` to `<root>/a/b/lib/x` and
//! `package:name/x` to `<root>/third_party/dart/name/lib/x`; a GN workspace
//! has no packages.

use crate::analysis_options::IncludeResolver;
use crate::package_config::{self, Packages};
use crate::pubspec::Pubspec;
use crate::sdk::DartSdk;
use crate::{fs, paths};

/// The kind of a workspace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceKind {
    /// No build system: one package (`BasicWorkspace`).
    Basic,
    /// Rooted at the folder that contains `.dart_tool/package_config.json`
    /// (`PackageConfigWorkspace`).
    PackageConfig {
        package_config_file: String,
        /// `true` if the `pubspec.yaml` in the root has a `workspace:` entry.
        is_pub_workspace: bool,
    },
    /// `BlazeWorkspace` (only discovery).
    Blaze,
    /// `GnWorkspace` (only discovery).
    Gn,
}

/// A workspace.
#[derive(Clone, Debug, PartialEq)]
pub struct Workspace {
    pub kind: WorkspaceKind,
    /// The absolute root path.
    pub root: String,
    /// The packages available in the workspace.
    pub packages: Packages,
}

/// A package of a workspace (`WorkspacePackageImpl`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspacePackage {
    /// A folder with a `pubspec.yaml` (`PubPackage`).
    Pub {
        root: String,
        name: Option<String>,
        /// The text of `environment: sdk:`.
        sdk_constraint: Option<String>,
    },
    /// The whole basic workspace, or the root of a package config workspace
    /// without `pubspec.yaml` (`BasicWorkspacePackage`).
    Basic { root: String },
}

impl WorkspacePackage {
    pub fn root(&self) -> &str {
        match self {
            WorkspacePackage::Pub { root, .. } | WorkspacePackage::Basic { root } => root,
        }
    }
}

/// The default options file of workspaces with default analysis options.
pub const DEFAULT_OPTIONS_URI: &str = "package:dart.analysis_options/default.yaml";

impl Workspace {
    /// A basic workspace rooted at [path], or at its parent if [path] is a
    /// file (`BasicWorkspace.find`).
    pub fn basic(packages: Packages, path: &str) -> Workspace {
        let root = if fs::folder_exists(path) {
            path.to_string()
        } else {
            paths::dirname(path).to_string()
        };
        Workspace {
            kind: WorkspaceKind::Basic,
            root,
            packages,
        }
    }

    /// A package config workspace (`PackageConfigWorkspace`). [packages] are
    /// the packages that the caller already parsed, or `None` to parse
    /// [package_config_file].
    pub fn package_config(
        root: &str,
        package_config_file: &str,
        packages: Option<Packages>,
    ) -> Workspace {
        let packages = packages
            .unwrap_or_else(|| package_config::parse_package_config_file(package_config_file));
        let pubspec = paths::join(root, "pubspec.yaml");
        let is_pub_workspace = fs::file_exists(&pubspec)
            && Pubspec::read(&pubspec).is_some_and(|p| p.workspace.is_some());
        Workspace {
            kind: WorkspaceKind::PackageConfig {
                package_config_file: package_config_file.to_string(),
                is_pub_workspace,
            },
            root: root.to_string(),
            packages,
        }
    }

    /// Finds the package config workspace that contains [path]: rooted at
    /// the innermost folder with `.dart_tool/package_config.json`
    /// (`PackageConfigWorkspace.find`).
    pub fn find_package_config(packages: Option<Packages>, path: &str) -> Option<Workspace> {
        for current in paths::with_ancestors(path) {
            let file = paths::join(current, ".dart_tool/package_config.json");
            if fs::file_exists(&file) {
                return Some(Workspace::package_config(current, &file, packages));
            }
        }
        None
    }

    /// Finds the Blaze workspace that contains [path] (`BlazeWorkspace.find`).
    pub fn find_blaze(path: &str) -> Option<Workspace> {
        for folder in paths::with_ancestors(path) {
            let parent = paths::dirname(folder);
            if fs::folder_exists(&paths::join(parent, "blaze-out")) {
                return Some(Workspace {
                    kind: WorkspaceKind::Blaze,
                    root: parent.to_string(),
                    packages: Packages::empty(),
                });
            }
            if fs::file_exists(&paths::join(folder, "dart/config/ide/flutter.json")) {
                return Some(Workspace {
                    kind: WorkspaceKind::Blaze,
                    root: folder.to_string(),
                    packages: Packages::empty(),
                });
            }
        }
        None
    }

    /// Finds the GN workspace for the `BUILD.gn` file [build_gn_file]: an
    /// ancestor folder with `.jiri_root` (`GnWorkspace.find`). The analyzer
    /// also requires package config files in the build output; dartr does not
    /// read them and returns a workspace without packages.
    pub fn find_gn(build_gn_file: &str) -> Option<Workspace> {
        for folder in paths::with_ancestors(paths::dirname(build_gn_file)) {
            if fs::folder_exists(&paths::join(folder, ".jiri_root")) {
                return Some(Workspace {
                    kind: WorkspaceKind::Gn,
                    root: folder.to_string(),
                    packages: Packages::empty(),
                });
            }
        }
        None
    }

    /// `true` for workspaces that have a default analysis options file
    /// (`WorkspaceWithDefaultAnalysisOptions`).
    pub fn has_default_analysis_options(&self) -> bool {
        self.kind == WorkspaceKind::Blaze
    }

    /// `true` if this is a [WorkspaceKind::Basic] workspace.
    pub fn is_basic(&self) -> bool {
        self.kind == WorkspaceKind::Basic
    }

    /// The package config file of a package config workspace.
    pub fn package_config_file(&self) -> Option<&str> {
        match &self.kind {
            WorkspaceKind::PackageConfig {
                package_config_file,
                ..
            } => Some(package_config_file),
            _ => None,
        }
    }

    /// Finds the workspace package that contains [path]
    /// (`Workspace.findPackageFor`).
    pub fn find_package_for(&self, path: &str) -> Option<WorkspacePackage> {
        match &self.kind {
            WorkspaceKind::Basic => {
                paths::is_within(&self.root, path).then(|| WorkspacePackage::Basic {
                    root: self.root.clone(),
                })
            }
            WorkspaceKind::PackageConfig { .. } => {
                if !paths::is_within(&self.root, path) {
                    return None;
                }
                for current in paths::with_ancestors(paths::dirname(path)) {
                    let pubspec = paths::join(current, "pubspec.yaml");
                    if fs::file_exists(&pubspec) {
                        if is_in_third_party_dart(&pubspec) {
                            return None;
                        }
                        let parsed = Pubspec::read(&pubspec).unwrap_or_default();
                        return Some(WorkspacePackage::Pub {
                            root: current.to_string(),
                            name: parsed.name_text().map(str::to_string),
                            sdk_constraint: parsed.environment_sdk.clone().flatten(),
                        });
                    }
                    if current == self.root {
                        return Some(WorkspacePackage::Basic {
                            root: current.to_string(),
                        });
                    }
                }
                None
            }
            WorkspaceKind::Blaze | WorkspaceKind::Gn => None,
        }
    }

    /// Resolves an absolute `package:` URI to a file path. The file can be
    /// missing.
    pub fn resolve_package_uri(&self, uri: &str) -> Option<String> {
        match &self.kind {
            WorkspaceKind::Blaze => {
                let (name, rel) = package_config::split_package_uri(uri)?;
                let base = if name.contains('.') {
                    paths::join(&self.root, &name.replace('.', "/"))
                } else {
                    paths::join(&self.root, &format!("third_party/dart/{name}"))
                };
                Some(paths::normalize(&paths::join(&base, &format!("lib/{rel}"))))
            }
            WorkspaceKind::PackageConfig { .. } => {
                let basic = self.packages.resolve_package_uri(uri);
                if let Some(path) = &basic
                    && fs::file_exists(path)
                {
                    return basic;
                }
                // `PackageConfigPackageUriResolver`: the built file in
                // `.dart_tool/build/generated/<package>/lib/`.
                let (name, rel) = package_config::split_package_uri(uri)?;
                if self.packages.get(&name).is_some() {
                    let built = paths::normalize(&paths::join(
                        &self.root,
                        &format!(".dart_tool/build/generated/{name}/lib/{rel}"),
                    ));
                    if fs::file_exists(&built) {
                        return Some(built);
                    }
                }
                basic
            }
            _ => self.packages.resolve_package_uri(uri),
        }
    }

    /// Resolves an absolute URI (`dart:`, `package:` or `file:`) to a file
    /// path, with the source factory of this workspace (`SourceFactory.forUri`).
    pub fn resolve_uri(&self, uri: &str, sdk: Option<&DartSdk>) -> Option<String> {
        let (scheme, _) = uri.split_once(':')?;
        match scheme {
            "dart" => sdk?.map_dart_uri(uri),
            "package" => self.resolve_package_uri(uri),
            "file" => paths::file_uri_to_path(uri),
            _ => None,
        }
    }

    /// Converts a file path to its URI: `dart:`, `package:` or `file:`
    /// (`SourceFactory.pathToUri`).
    pub fn path_to_uri(&self, path: &str, sdk: Option<&DartSdk>) -> String {
        if let Some(uri) = sdk.and_then(|sdk| sdk.path_to_uri(path)) {
            return uri;
        }
        if let WorkspaceKind::PackageConfig { .. } = &self.kind
            && paths::is_within(&self.root, path)
        {
            let relative = paths::relative(path, &self.root);
            let parts: Vec<&str> = relative.split('/').collect();
            if parts.len() > 5
                && parts[0] == ".dart_tool"
                && parts[1] == "build"
                && parts[2] == "generated"
                && parts[4] == "lib"
            {
                return format!("package:{}/{}", parts[3], parts[5..].join("/"));
            }
        }
        if let Some(uri) = self.packages.path_to_package_uri(path) {
            return uri;
        }
        if let WorkspaceKind::PackageConfig { .. } = &self.kind {
            // `PackageConfigFileUriResolver.pathToUri`: a generated file of
            // the package is mapped to its source path.
            if paths::is_within(&self.root, path)
                && let Some(WorkspacePackage::Pub {
                    name: Some(name), ..
                }) = self.find_package_for(path)
            {
                let relative = paths::relative(path, &self.root);
                let parts: Vec<&str> = relative.split('/').collect();
                if parts.len() > 4
                    && parts[0] == ".dart_tool"
                    && parts[1] == "build"
                    && parts[2] == "generated"
                    && parts[3] == name
                {
                    let canonical = paths::join(&self.root, &parts[4..].join("/"));
                    return paths::to_file_uri(&canonical);
                }
            }
        }
        paths::to_file_uri(path)
    }
}

/// `PackageConfigWorkspace._isInThirdPartyDart`.
fn is_in_third_party_dart(pubspec: &str) -> bool {
    let parts = paths::split(pubspec);
    parts.len() > 4 && parts[parts.len() - 3] == "dart" && parts[parts.len() - 4] == "third_party"
}

impl IncludeResolver for Workspace {
    /// `workspace.partialSourceFactory.resolveUri(FileSource(file), uri)`:
    /// relative URIs resolve against the file, `package:` URIs with the
    /// packages of the workspace, `file:` URIs to paths; `dart:` URIs do not
    /// resolve (the partial source factory has no SDK).
    fn resolve_include(&self, containing_file: &str, uri: &str) -> Option<String> {
        if uri.is_empty() {
            return Some(containing_file.to_string());
        }
        let scheme_end = uri.find(':').filter(|&i| {
            i > 0
                && uri[..i]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic())
                && uri[..i]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
        });
        match scheme_end {
            Some(end) => match &uri[..end] {
                "package" => self.resolve_package_uri(uri),
                "file" => paths::file_uri_to_path(uri),
                _ => None,
            },
            None => {
                let reference = uri.split(['?', '#']).next().unwrap_or("");
                let decoded = paths::percent_decode(reference);
                let base = paths::dirname(containing_file);
                Some(paths::normalize(&paths::join(base, &decoded)))
            }
        }
    }
}
