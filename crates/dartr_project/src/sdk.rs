//! The Dart SDK: location, version, and the `dart:` library map.
//!
//! Ports `FolderBasedDartSdk` of `pkg/analyzer/lib/src/dart/sdk/sdk.dart`,
//! `SdkLibrariesReader` of `pkg/analyzer/lib/src/generated/sdk.dart`, and
//! `getImportUriIfMatchesRelativeSdkPath` of
//! `pkg/analyzer/lib/src/dart/sdk/sdk_utils.dart`.
//!
//! The analyzer finds the SDK of the running VM (`getSdkPath`). dartr finds
//! the SDK of the `dart` executable on `PATH`: symbolic links are resolved,
//! and for a Flutter SDK (where `bin/dart` is a wrapper script, also with
//! fvm) the SDK is `bin/cache/dart-sdk`.

use crate::package_config::LanguageVersion;
use crate::{fs, paths};

/// A library of the SDK, from `libraries.dart`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdkLibrary {
    /// For example `dart:core`.
    pub short_name: String,
    /// Path relative to the `lib` folder of the SDK, with `/` separators, or
    /// an absolute path (libraries of an `_embedder.yaml`).
    pub path: String,
    pub documented: bool,
    pub implementation: bool,
}

impl SdkLibrary {
    /// `true` for `dart:_*` libraries.
    pub fn is_internal(&self) -> bool {
        self.short_name.starts_with("dart:_")
    }
}

/// A Dart SDK: an SDK folder (`FolderBasedDartSdk`), or the libraries of an
/// `_embedder.yaml` file (`EmbedderSdk`, used for Flutter's `sky_engine`).
#[derive(Clone, Debug)]
pub struct DartSdk {
    /// The SDK folder, or the folder of `_embedder.yaml`.
    path: String,
    libraries: Vec<SdkLibrary>,
    version: Option<String>,
    /// For an embedder SDK: the path of `_embedder.yaml`.
    embedder_yaml: Option<String>,
    language_version: Option<LanguageVersion>,
}

impl DartSdk {
    /// Opens the SDK at [path] (absolute, normalized).
    pub fn new(path: &str) -> DartSdk {
        let lib = paths::join(path, "lib");
        let locations = [
            paths::join(&lib, "_internal/sdk_library_metadata/lib/libraries.dart"),
            paths::join(&lib, "_internal/libraries.dart"),
        ];
        let mut libraries = Vec::new();
        for location in &locations {
            if let Some(text) = fs::read_string(location) {
                libraries = read_libraries(&text);
                break;
            }
        }
        let version = fs::read_string(&paths::join(path, "version")).map(|v| v.trim().to_string());
        let language_version = version.as_deref().and_then(LanguageVersion::parse_lenient);
        DartSdk {
            path: path.to_string(),
            libraries,
            version,
            embedder_yaml: None,
            language_version,
        }
    }

    /// The embedder SDK for the `_embedder.yaml` in [lib_folder], if the file
    /// exists and is a YAML map (`locateEmbedderYamlFor`, `EmbedderSdk`).
    /// [language_version] is the language version of the folder SDK.
    pub fn embedder(
        lib_folder: &str,
        language_version: Option<LanguageVersion>,
    ) -> Option<DartSdk> {
        let file = paths::join(lib_folder, "_embedder.yaml");
        let text = fs::read_string(&file)?;
        let yaml = crate::yaml::load_yaml_node(&text).ok()?;
        yaml.as_map()?;
        let mut libraries: Vec<SdkLibrary> = Vec::new();
        if let Some(entries) = yaml.value_at("embedded_libs").and_then(|n| n.as_map()) {
            for (key, value) in entries {
                let (Some(name), Some(path)) = (key.string_value(), value.string_value()) else {
                    continue;
                };
                if !name.starts_with("dart:") {
                    continue;
                }
                libraries.retain(|l| l.short_name != name);
                libraries.push(SdkLibrary {
                    short_name: name.to_string(),
                    path: paths::normalize(&paths::join(lib_folder, path)),
                    documented: true,
                    implementation: false,
                });
            }
        }
        Some(DartSdk {
            path: lib_folder.to_string(),
            libraries,
            version: None,
            embedder_yaml: Some(file),
            language_version,
        })
    }

    /// For an embedder SDK, the path of its `_embedder.yaml`.
    pub fn embedder_yaml(&self) -> Option<&str> {
        self.embedder_yaml.as_deref()
    }

    /// Finds the SDK of the `dart` executable on `PATH` and opens it.
    pub fn find() -> Option<DartSdk> {
        find_sdk_path().map(|path| DartSdk::new(&path))
    }

    /// The SDK folder.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The `lib` folder.
    pub fn lib_path(&self) -> String {
        paths::join(&self.path, "lib")
    }

    /// The content of the `version` file, trimmed, or `0` (`sdkVersion`).
    pub fn sdk_version(&self) -> &str {
        self.version.as_deref().unwrap_or("0")
    }

    /// The language version of the SDK: major and minor of [sdk_version].
    pub fn language_version(&self) -> Option<LanguageVersion> {
        self.language_version
    }

    /// The libraries, in the order of `libraries.dart`.
    pub fn libraries(&self) -> &[SdkLibrary] {
        &self.libraries
    }

    /// The library with the short name [name] (for example `dart:core`).
    pub fn library(&self, name: &str) -> Option<&SdkLibrary> {
        self.libraries.iter().find(|l| l.short_name == name)
    }

    /// Maps `dart:name` or `dart:name/part.dart` to a file path
    /// (`FolderBasedDartSdk.internalMapDartUri`). The file can be missing.
    pub fn map_dart_uri(&self, uri: &str) -> Option<String> {
        let (name, relative) = match uri.find('/') {
            Some(index) => (&uri[..index], &uri[index + 1..]),
            None => (uri, ""),
        };
        let library = self.library(name)?;
        let file = if paths::is_absolute(&library.path) {
            library.path.clone()
        } else {
            paths::normalize(&paths::join(&self.lib_path(), &library.path))
        };
        if relative.is_empty() {
            return Some(file);
        }
        Some(paths::normalize(&paths::join(
            paths::dirname(&file),
            relative,
        )))
    }

    /// Maps a file path inside the SDK to a `dart:` URI
    /// (`FolderBasedDartSdk.pathToUri`).
    pub fn path_to_uri(&self, path: &str) -> Option<String> {
        for library in &self.libraries {
            if paths::is_absolute(&library.path) && library.path == path {
                return Some(library.short_name.clone());
            }
        }
        for library in &self.libraries {
            if paths::is_absolute(&library.path)
                && let Some(inside) = relative_path_if_inside(&library.path, path)
            {
                return Some(format!("{}/{}", library.short_name, inside));
            }
        }
        let lib = self.lib_path();
        let relative = if self.embedder_yaml.is_some() {
            path
        } else {
            path.strip_prefix(&format!("{lib}/"))?
        };
        for library in &self.libraries {
            if library.path == relative {
                return Some(library.short_name.clone());
            }
        }
        for library in &self.libraries {
            if let Some(inside) = relative_path_if_inside(&library.path, relative) {
                return Some(format!("{}/{}", library.short_name, inside));
            }
        }
        None
    }
}

/// `getRelativePathIfInside` of `sdk_utils.dart`.
fn relative_path_if_inside<'a>(library_path: &str, file_path: &'a str) -> Option<&'a str> {
    let lib = library_path.as_bytes();
    let file = file_path.as_bytes();
    let min_length = lib.len().min(file.len());
    let mut same = 0;
    for i in 0..min_length {
        if lib[i] == file[i] || (is_sep(lib[i]) && is_sep(file[i])) {
            same += 1;
        } else {
            break;
        }
    }
    if lib[same..].iter().any(|c| is_sep(*c)) {
        return None;
    }
    let mut i = same.min(lib.len().saturating_sub(1)) as isize;
    while i >= 0 {
        if is_sep(lib[i as usize]) {
            return Some(&file_path[i as usize + 1..]);
        }
        i -= 1;
    }
    None
}

fn is_sep(c: u8) -> bool {
    c == b'/' || c == b'\\'
}

/// Returns `true` if [path] looks like a Dart SDK folder.
fn is_sdk(path: &str) -> bool {
    fs::file_exists(&paths::join(path, "version")) && fs::folder_exists(&paths::join(path, "lib"))
}

/// Finds the SDK folder of the `dart` executable on `PATH`.
pub fn find_sdk_path() -> Option<String> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join("dart");
        let Some(candidate) = candidate.to_str() else {
            continue;
        };
        if !fs::file_exists(candidate) {
            continue;
        }
        if let Some(sdk) = sdk_path_for_executable(candidate) {
            return Some(sdk);
        }
    }
    None
}

/// Returns the SDK folder for the `dart` executable at [executable].
pub fn sdk_path_for_executable(executable: &str) -> Option<String> {
    let resolved = fs::canonicalize(executable)?;
    let bin = paths::dirname(&resolved);
    let root = paths::dirname(bin);
    if is_sdk(root) {
        return Some(root.to_string());
    }
    // A Flutter SDK: `bin/dart` is a wrapper script for the SDK in
    // `bin/cache/dart-sdk`.
    let flutter_sdk = paths::join(bin, "cache/dart-sdk");
    if is_sdk(&flutter_sdk) {
        return fs::canonicalize(&flutter_sdk);
    }
    None
}

/// Reads the library map of `libraries.dart` (`SdkLibrariesReader`): every
/// map literal entry `'name': const LibraryInfo('path', ...)`.
pub fn read_libraries(text: &str) -> Vec<SdkLibrary> {
    let tokens = tokenize(text);
    let mut result: Vec<SdkLibrary> = Vec::new();
    let mut i = 0;
    while i + 4 < tokens.len() {
        let is_entry = matches!(&tokens[i], Token::String(_))
            && tokens[i + 1] == Token::Punct(':')
            && matches!(&tokens[i + 2], Token::Ident(k) if k == "const" || k == "new")
            && matches!(&tokens[i + 3], Token::Ident(_))
            && tokens[i + 4] == Token::Punct('(');
        if !is_entry {
            i += 1;
            continue;
        }
        let Token::String(key) = &tokens[i] else {
            unreachable!()
        };
        let mut library = SdkLibrary {
            short_name: format!("dart:{key}"),
            path: String::new(),
            documented: true,
            implementation: false,
        };
        // Arguments, split at top-level commas.
        let mut j = i + 5;
        let mut depth = 0;
        let mut argument: Vec<&Token> = Vec::new();
        let mut arguments: Vec<Vec<&Token>> = Vec::new();
        while j < tokens.len() {
            match &tokens[j] {
                Token::Punct('(') | Token::Punct('[') | Token::Punct('{') => depth += 1,
                Token::Punct(')') | Token::Punct(']') | Token::Punct('}') if depth > 0 => {
                    depth -= 1
                }
                Token::Punct(')') => break,
                Token::Punct(',') if depth == 0 => {
                    arguments.push(std::mem::take(&mut argument));
                    j += 1;
                    continue;
                }
                _ => {}
            }
            argument.push(&tokens[j]);
            j += 1;
        }
        if !argument.is_empty() {
            arguments.push(argument);
        }
        for argument in arguments {
            match argument.as_slice() {
                [Token::String(value)] => library.path = value.clone(),
                [Token::Ident(name), Token::Punct(':'), value] => match (name.as_str(), value) {
                    ("implementation", Token::Ident(b)) => library.implementation = b == "true",
                    ("documented", Token::Ident(b)) => library.documented = b == "true",
                    ("dart2jsPath", Token::String(path)) => library.path = path.clone(),
                    _ => {}
                },
                _ => {}
            }
        }
        result.retain(|l| l.short_name != library.short_name);
        result.push(library);
        i = j;
    }
    result
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    String(String),
    Ident(String),
    Punct(char),
}

/// A small tokenizer for the subset of Dart in `libraries.dart`.
fn tokenize(text: &str) -> Vec<Token> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else if c == '\'' || c == '"' {
            let quote = c;
            let mut value = String::new();
            i += 1;
            while i < chars.len() && chars[i] != quote {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 1;
                }
                value.push(chars[i]);
                i += 1;
            }
            i += 1;
            tokens.push(Token::String(value));
        } else if c.is_alphanumeric() || c == '_' || c == '$' {
            let start = i;
            while i < chars.len()
                && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '$')
            {
                i += 1;
            }
            tokens.push(Token::Ident(chars[start..i].iter().collect()));
        } else {
            tokens.push(Token::Punct(c));
            i += 1;
        }
    }
    tokens
}
