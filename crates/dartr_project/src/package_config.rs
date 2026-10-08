//! `package_config.json` parsing and `package:` URI resolution.
//!
//! Ports `pkg/analyzer/lib/src/context/packages.dart` (`Packages`,
//! `parsePackageConfigJsonFile`) and the validation of
//! `package:package_config` (`PackageConfig.parseString`). As in the
//! analyzer, any error in the file makes the whole file empty.

use crate::{fs, paths};
use serde_json::Value;

/// A language version: `major.minor`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LanguageVersion {
    pub major: u32,
    pub minor: u32,
}

impl LanguageVersion {
    pub const fn new(major: u32, minor: u32) -> Self {
        LanguageVersion { major, minor }
    }

    /// Parses `major.minor[.patch[-pre][+build]]`, keeping major and minor.
    pub fn parse_lenient(text: &str) -> Option<Self> {
        let mut parts = text.trim().split('.');
        let major = parts.next()?.parse().ok()?;
        let minor_part = parts.next()?;
        let digits: String = minor_part
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        Some(LanguageVersion {
            major,
            minor: digits.parse().ok()?,
        })
    }
}

impl std::fmt::Display for LanguageVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// A package of a package config.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    /// Absolute normalized path of the package root.
    pub root: String,
    /// Absolute normalized path of the folder that `package:<name>/` maps to.
    pub lib: String,
    /// The language version of the package, `None` if not specified.
    pub language_version: Option<LanguageVersion>,
}

/// The packages of a package config, in file order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Packages {
    packages: Vec<Package>,
}

impl Packages {
    pub fn empty() -> Self {
        Packages::default()
    }

    pub fn new(packages: Vec<Package>) -> Self {
        Packages { packages }
    }

    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }

    pub fn packages(&self) -> &[Package] {
        &self.packages
    }

    /// The package with the given [name].
    pub fn get(&self, name: &str) -> Option<&Package> {
        self.packages.iter().find(|p| p.name == name)
    }

    /// The innermost package whose root strictly contains [path]
    /// (`Packages.packageForPath`).
    pub fn package_for_path(&self, path: &str) -> Option<&Package> {
        let mut result: Option<&Package> = None;
        for package in &self.packages {
            if paths::is_within(&package.root, path) {
                match result {
                    Some(current) if current.root.len() >= package.root.len() => {}
                    _ => result = Some(package),
                }
            }
        }
        result
    }

    /// Resolves `package:<name>/<path>` to a file path, without checking that
    /// the file exists (`PackageMapUriResolver.resolveAbsolute`).
    pub fn resolve_package_uri(&self, uri: &str) -> Option<String> {
        let (name, rel) = split_package_uri(uri)?;
        let package = self.get(&name)?;
        Some(paths::normalize(&paths::join(&package.lib, &rel)))
    }

    /// Converts a file path to a `package:` URI, using the package with the
    /// longest `lib` folder that contains [path]
    /// (`PackageMapUriResolver.pathToUri`).
    pub fn path_to_package_uri(&self, path: &str) -> Option<String> {
        let mut best: Option<(usize, String)> = None;
        for package in &self.packages {
            if let Some(rel) = paths::relative_if_within(&package.lib, path)
                && best
                    .as_ref()
                    .is_none_or(|(len, _)| package.lib.len() > *len)
            {
                let encoded: Vec<String> = rel.split('/').map(paths::encode_path_segment).collect();
                best = Some((
                    package.lib.len(),
                    format!("package:{}/{}", package.name, encoded.join("/")),
                ));
            }
        }
        best.map(|(_, uri)| uri)
    }
}

/// Splits `package:name/rest` into the package name and the (decoded)
/// relative path. Returns `None` if there are less than two path segments.
pub fn split_package_uri(uri: &str) -> Option<(String, String)> {
    let rest = uri.strip_prefix("package:")?;
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let segments: Vec<String> = rest.split('/').map(paths::percent_decode).collect();
    if segments.len() < 2 {
        return None;
    }
    Some((segments[0].clone(), segments[1..].join("/")))
}

/// Parses the package config file at [path]. Returns empty packages if the
/// file cannot be read or is not valid (`parsePackageConfigJsonFile`).
pub fn parse_package_config_file(path: &str) -> Packages {
    let Some(content) = fs::read_string(path) else {
        return Packages::empty();
    };
    parse_package_config(&content, path).unwrap_or_default()
}

/// Parses [content] of a package config file at [path], or returns an error
/// message for the first error (`PackageConfig.parseString`).
pub fn parse_package_config(content: &str, path: &str) -> Result<Packages, String> {
    let json: Value = serde_json::from_str(content).map_err(|e| e.to_string())?;
    let map = json.as_object().ok_or("value is not a JSON object")?;
    let base_dir = paths::dirname(path).to_string();

    let config_version = match map.get("configVersion") {
        None => return Err("Missing configVersion entry".into()),
        Some(value) => value.as_i64().ok_or("configVersion is not a JSON int")?,
    };
    if !(0..=2).contains(&config_version) {
        return Err("Must be in the range 1 to 2".into());
    }
    let package_array = match map.get("packages") {
        None => return Err("Missing packages list".into()),
        Some(value) => value.as_array().ok_or("packages is not a JSON array")?,
    };

    struct Parsed {
        name: String,
        root: String,
        lib: String,
        language_version: Option<LanguageVersion>,
    }
    let mut parsed: Vec<Parsed> = Vec::new();
    for entry in package_array {
        let entry = entry
            .as_object()
            .ok_or("package entry is not a JSON object")?;
        let name = match entry.get("name") {
            None => return Err("Missing name entry".into()),
            Some(v) => v.as_str().ok_or("name is not a JSON string")?.to_string(),
        };
        let root_uri = match entry.get("rootUri") {
            None => return Err("Missing rootUri entry".into()),
            Some(v) => v
                .as_str()
                .ok_or("rootUri is not a JSON string")?
                .to_string(),
        };
        let package_uri = match entry.get("packageUri") {
            None => None,
            Some(v) => Some(
                v.as_str()
                    .ok_or("packageUri is not a JSON string")?
                    .to_string(),
            ),
        };
        let language_version = match entry.get("languageVersion") {
            None => None,
            Some(v) => {
                let text = v.as_str().ok_or("languageVersion is not a JSON string")?;
                Some(parse_language_version(text)?)
            }
        };
        if !is_valid_package_name(&name) {
            return Err(format!("Not a valid package name: {name}"));
        }
        let root = resolve_directory_uri(&base_dir, &root_uri)?;
        let lib = match &package_uri {
            None => root.clone(),
            Some(uri) => {
                let lib = resolve_directory_uri(&root, uri)?;
                if !paths::is_or_within(&root, &lib) {
                    return Err("The package URI root is not below the package root".into());
                }
                lib
            }
        };
        parsed.push(Parsed {
            name,
            root,
            lib,
            language_version,
        });
    }

    // `SimplePackageConfig._validatePackages`: sorted by root, no duplicate
    // names, no same roots, no interleaving roots.
    let mut sorted: Vec<&Parsed> = parsed.iter().collect();
    sorted.sort_by(|a, b| format!("{}/", a.root).cmp(&format!("{}/", b.root)));
    let mut names = std::collections::HashSet::new();
    let mut added: Vec<&Parsed> = Vec::new();
    for package in sorted {
        if !names.insert(package.name.clone()) {
            return Err(format!("Duplicate package name '{}'", package.name));
        }
        for existing in &added {
            if paths::is_or_within(&existing.root, &package.root) {
                if existing.root == package.root {
                    return Err(format!(
                        "Packages {} and {} have the same root directory",
                        package.name, existing.name
                    ));
                }
                if paths::is_or_within(&package.root, &existing.lib) {
                    return Err(format!(
                        "Package {} is inside the root of package {}",
                        package.name, existing.name
                    ));
                }
            }
        }
        added.push(package);
    }

    Ok(Packages::new(
        parsed
            .into_iter()
            .map(|p| Package {
                name: p.name,
                root: p.root,
                lib: p.lib,
                language_version: p.language_version,
            })
            .collect(),
    ))
}

/// `checkPackageName` of `package:package_config`.
fn is_valid_package_name(name: &str) -> bool {
    const VALID: &str =
        "!$&'()*+,-.0123456789;=@ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz~";
    !name.is_empty() && name.chars().all(|c| VALID.contains(c)) && !name.chars().all(|c| c == '.')
}

/// `parseLanguageVersion` of `package:package_config`: `^(0|[1-9]\d*)\.(0|[1-9]\d*)$`.
fn parse_language_version(text: &str) -> Result<LanguageVersion, String> {
    let numeral = |part: &str| -> Result<u32, String> {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!("Invalid language version: {text}"));
        }
        if part.len() > 1 && part.starts_with('0') {
            return Err(format!("Leading zero not allowed: {text}"));
        }
        part.parse::<u32>()
            .ok()
            .filter(|v| *v <= 0x7FFF_FFFF)
            .ok_or_else(|| format!("Number too large: {text}"))
    };
    let (major, minor) = text
        .split_once('.')
        .ok_or_else(|| format!("Missing '.': {text}"))?;
    Ok(LanguageVersion::new(numeral(major)?, numeral(minor)?))
}

/// Resolves a URI reference [uri] against the directory [base] (an absolute
/// path) and returns the normalized absolute path of the directory.
fn resolve_directory_uri(base: &str, uri: &str) -> Result<String, String> {
    let scheme_end = uri.find(':').filter(|&i| {
        i > 0
            && uri[..i]
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    });
    if let Some(end) = scheme_end {
        let scheme = &uri[..end];
        if scheme == "package" {
            return Err("Must not be a package URI".into());
        }
        if scheme != "file" {
            return Err(format!("Unsupported URI scheme: {uri}"));
        }
        if uri.contains(['?', '#']) {
            return Err("Not an absolute URI with no query or fragment".into());
        }
        return paths::file_uri_to_path(uri).ok_or_else(|| format!("Invalid file URI: {uri}"));
    }
    if uri.contains(['?', '#']) {
        return Err("Not an absolute URI with no query or fragment".into());
    }
    let decoded = paths::percent_decode(uri);
    Ok(paths::normalize(&paths::join(base, &decoded)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_files_are_empty_and_resolution_uses_innermost_package() {
        let config = r#"{"configVersion":2,"packages":[
            {"name":"app","rootUri":"../","packageUri":"lib/","languageVersion":"3.4"},
            {"name":"dep","rootUri":"file:///cache/dep-1.0","packageUri":"lib/"},
            {"name":"inner","rootUri":"../tool/inner/","packageUri":"lib/"}]}"#;
        let packages =
            parse_package_config(config, "/w/app/.dart_tool/package_config.json").unwrap();
        assert_eq!(packages.get("app").unwrap().lib, "/w/app/lib");
        assert_eq!(
            packages.get("app").unwrap().language_version,
            Some(LanguageVersion::new(3, 4))
        );
        assert_eq!(packages.get("dep").unwrap().root, "/cache/dep-1.0");
        assert_eq!(
            packages
                .package_for_path("/w/app/tool/inner/a.dart")
                .unwrap()
                .name,
            "inner"
        );
        assert_eq!(
            packages
                .resolve_package_uri("package:dep/src/a.dart")
                .unwrap(),
            "/cache/dep-1.0/lib/src/a.dart"
        );
        assert_eq!(
            packages.path_to_package_uri("/w/app/lib/a b.dart").unwrap(),
            "package:app/a%20b.dart"
        );

        let duplicate = r#"{"configVersion":2,"packages":[{"name":"a","rootUri":"../"},{"name":"a","rootUri":"../x/"}]}"#;
        assert!(parse_package_config(duplicate, "/w/.dart_tool/package_config.json").is_err());
        let bad_version = r#"{"configVersion":2,"packages":[{"name":"a","rootUri":"../","languageVersion":"3.04"}]}"#;
        assert!(parse_package_config(bad_version, "/w/.dart_tool/package_config.json").is_err());
    }
}
