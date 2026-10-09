// Dart source: dart_style lib/src/config_cache.dart
// Dart source: package:package_config lib/src/discovery.dart (findPackageConfig)

//! Caches the nearest surrounding package config file for files in
//! directories, and the language versions and formatter options inferred from
//! the package config and "analysis_options.yaml" files.

use rustc_hash::{FxHashMap, FxHashSet};

use dartr_project::package_config::{Packages, parse_package_config};
use dartr_project::paths;

use crate::analysis_options::{AnalysisOptionsError, Value, find_analysis_options};
use crate::dart_formatter::TrailingCommas;
use crate::dart_version_history::Version;

/// Caches the nearest surrounding package config file for files in directories.
///
/// The formatter reads `.dart_tool/package_config.json` files in order to
/// determine the default language version of files in that package and to
/// resolve "package:" URIs in "analysis_options.yaml" files.
///
/// Walking the file system to find the package config and then reading it off
/// disk is very slow. We know that every formatted file in the same directory
/// will share the same package config, so this caches a previously read
/// config for each directory.
///
/// This class also directly caches the language versions and page widths that
/// are then inferred from the package config and analysis_options.yaml files.
///
/// Messages that Dart writes to stderr are appended to the `stderr` buffer
/// that each method takes, so that the caller can print them in file order
/// when files are formatted in parallel.
#[derive(Default)]
pub struct ConfigCache {
    /// The previously cached package config for all files immediately within a
    /// given directory.
    directory_configs: FxHashMap<String, Option<Packages>>,

    /// The previously cached default language version for all files immediately
    /// within a given directory.
    ///
    /// The version may be `None` if we formatted a file in that directory and
    /// discovered that there is no surrounding package.
    directory_versions: FxHashMap<String, Option<Version>>,

    /// The previously cached configured options for all files immediately within
    /// a given directory.
    directory_options: FxHashMap<String, FormatterConfig>,

    /// Paths to files that we have failed to read.
    ///
    /// These are cached so that the warning is only printed once per file.
    failed_read_paths: FxHashSet<String>,
}

/// The formatter options that can be configured in the "analysis_options.yaml"
/// file (Dart `_FormatterOptions`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FormatterConfig {
    /// The configured page width, or `None` if there is no options file or the
    /// options file doesn't specify it.
    pub page_width: Option<usize>,

    /// The configured comma handling, or `None` if there is no options file or
    /// the options file doesn't specify it.
    pub trailing_commas: Option<TrailingCommas>,
}

/// Dart `File.parent.path`.
fn parent_path(file: &str) -> String {
    match file.rfind('/') {
        None => ".".to_string(),
        Some(0) => "/".to_string(),
        Some(index) => file[..index].to_string(),
    }
}

impl ConfigCache {
    pub fn new() -> ConfigCache {
        ConfigCache::default()
    }

    /// Looks for a package surrounding [file] and, if found, returns the default
    /// language version specified by that package.
    pub fn find_language_version(
        &mut self,
        file: &str,
        display_path: &str,
        stderr: &mut String,
    ) -> Option<Version> {
        // Use the cached version (which may be `None`) if present.
        let directory = parent_path(file);
        if let Some(version) = self.directory_versions.get(&directory) {
            return *version;
        }

        // Otherwise, walk the file system and look for it.
        let absolute = paths::absolute_normalized(file);
        let version = self
            .find_package_config(file, display_path, true, stderr)
            .and_then(|config| config.package_for_path(&absolute).cloned())
            .and_then(|package| package.language_version)
            .map(|v| Version::new(v.major, v.minor));

        // Cache the version, or `None` if we weren't able to resolve this
        // file's version, so we don't try again.
        self.directory_versions.insert(directory, version);
        version
    }

    /// Looks for an "analysis_options.yaml" file surrounding [file] and, if
    /// found and valid, returns the configured options.
    ///
    /// If no options file could be found or it doesn't contain a "formatter"
    /// key whose value is a map, returns a default set of options where all
    /// settings are `None`.
    ///
    /// Returns an error only for a YAML syntax error, which Dart does not
    /// catch (the format command crashes).
    pub fn find_formatter_options(
        &mut self,
        file: &str,
        stderr: &mut String,
    ) -> Result<FormatterConfig, String> {
        // Use the cached version if present.
        let directory = parent_path(file);
        if let Some(options) = self.directory_options.get(&directory) {
            return Ok(*options);
        }

        let mut page_width = None;
        let mut trailing_commas = None;

        // Look for a surrounding "analysis_options.yaml" file.
        let mut failed_reads: Vec<String> = Vec::new();
        let result = {
            let mut report_failed_read = |path: &str| failed_reads.push(path.to_string());
            let mut resolve_package_uri =
                |uri: &str| -> Option<String> { self.resolve_package_uri(file, uri) };
            find_analysis_options(file, &mut report_failed_read, &mut resolve_package_uri)
        };
        for path in failed_reads {
            self.report_failed_read(&path, stderr);
        }

        match result {
            Ok(options_file) => {
                if let Some(formatter @ Value::Map(_)) = options_file.get("formatter") {
                    if let Some(Value::Int(width)) = formatter.get("page_width") {
                        page_width = Some((*width).max(0) as usize);
                    }

                    if let Some(commas) = formatter.get("trailing_commas") {
                        match commas {
                            Value::String(s) if s == "automate" => {
                                trailing_commas = Some(TrailingCommas::Automate)
                            }
                            Value::String(s) if s == "preserve" => {
                                trailing_commas = Some(TrailingCommas::Preserve)
                            }
                            _ => {
                                stderr.push_str(&format!(
                                    "Warning: \"trailing_commas\" option should be \"automate\" or \"preserve\", but was \"{}\".\n",
                                    commas.to_dart_string()
                                ));
                            }
                        }
                    }
                }
            }
            Err(AnalysisOptionsError::PackageResolution(exception)) => {
                // Report the error, but use the default settings and keep going.
                stderr.push_str(&format!(
                    "Warning: Package resolution error when reading \"analysis_options.yaml\" file for \"{file}\":\n{}\n",
                    exception.0
                ));
            }
            Err(AnalysisOptionsError::Yaml(text)) => return Err(text),
        }

        // Cache whichever options we found (or `None` if we didn't find them).
        let options = FormatterConfig {
            page_width,
            trailing_commas,
        };
        self.directory_options.insert(directory, options);
        Ok(options)
    }

    /// Look for and cache the nearest package surrounding [file].
    fn find_package_config(
        &mut self,
        file: &str,
        display_path: &str,
        for_language_version: bool,
        stderr: &mut String,
    ) -> Option<&Packages> {
        // Use the cached one (which might be `None`) if we have it.
        let directory = parent_path(file);
        if self.directory_configs.contains_key(&directory) {
            return self.directory_configs[&directory].as_ref();
        }

        // Otherwise, walk the file system and look for it. If we fail to find
        // it, store `None` so that we don't look again in that same directory.
        match find_package_config(&directory) {
            Ok(config) => {
                self.directory_configs.insert(directory.clone(), config);
                self.directory_configs[&directory].as_ref()
            }
            Err(error) => {
                // We need a language version, so report an error if we can't
                // find one. We don't need a page width because we happily use
                // the default, so say nothing in that case.
                if for_language_version {
                    stderr.push_str(&format!(
                        "Could not read package configuration for {display_path}:\n{error}\n"
                    ));
                    stderr.push_str(
                        "To avoid searching for a package configuration, specify a language version using \"--language-version\".\n",
                    );
                }
                None
            }
        }
    }

    /// Resolves a "package:" [package_uri] using the nearest package config
    /// file surrounding [file].
    ///
    /// If there is no package config file around [file], or the package config
    /// doesn't contain the package for [package_uri], returns `None`.
    /// Otherwise, returns an absolute file path for where [package_uri] can be
    /// found on disk.
    fn resolve_package_uri(&mut self, file: &str, package_uri: &str) -> Option<String> {
        let mut ignored = String::new();
        let config = self.find_package_config(file, file, false, &mut ignored)?;
        config.resolve_package_uri(package_uri)
    }

    fn report_failed_read(&mut self, path: &str, stderr: &mut String) {
        if self.failed_read_paths.insert(path.to_string()) {
            stderr.push_str(&format!("Warning: Couldn't read file \"{path}\".\n"));
        }
    }
}

/// Dart `findPackageConfig(directory)` of `package:package_config`: walks
/// [directory] and its parents and reads the first
/// `.dart_tool/package_config.json`. Returns `Ok(None)` if there is none,
/// and an error message if the file cannot be read or is invalid.
pub fn find_package_config(directory: &str) -> Result<Option<Packages>, String> {
    let directory = paths::absolute_normalized(directory);
    if !std::fs::metadata(&directory).is_ok_and(|m| m.is_dir()) {
        return Ok(None);
    }
    for folder in paths::with_ancestors(&directory) {
        let path = paths::join(folder, ".dart_tool/package_config.json");
        if std::fs::metadata(&path).is_ok_and(|m| m.is_file()) {
            let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            return parse_package_config(&content, &path).map(Some);
        }
    }
    Ok(None)
}
