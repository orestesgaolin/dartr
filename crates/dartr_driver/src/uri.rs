// Dart source: sdk/lib/core/uri.dart (`Uri.tryParse`, `_Uri.resolveUri`,
// `_makePath`, `_normalizePath`, `_removeDotSegments`,
// `_normalizeRelativePath`, `_mergePaths`, `_normalizeEscape`), and
// pkg/_fe_analyzer_shared/lib/src/util/resolve_relative_uri.dart

//! [`Uri`]: the subset of Dart `Uri` that the driver uses for directive
//! URIs: parsing with the normalization of `Uri.parse` (the text form is
//! what `Uri.toString()` gives), and relative resolution (`resolveUri`,
//! including the special merge for `package:` URIs).
//!
//! Deviations: `data:` URIs are not special; IPv6 hosts are only checked
//! for the closing `]`; host names are lowercased without further
//! normalization; default ports are not removed.

use std::fmt;

/// A parsed and normalized URI (Dart `Uri`).
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct Uri {
    /// Lowercase; empty for a relative reference.
    pub scheme: String,
    pub user_info: String,
    /// `None` when there is no authority.
    pub host: Option<String>,
    pub port: Option<String>,
    pub path: String,
    pub query: Option<String>,
    pub fragment: Option<String>,
}

impl Uri {
    /// Dart `Uri.tryParse`.
    pub fn try_parse(text: &str) -> Option<Uri> {
        let mut rest = text;
        // Scheme: everything before the first `:` if that comes before any
        // `/`, `?` or `#`.
        let mut scheme = String::new();
        if let Some(colon) = rest.find(':') {
            let before = &rest[..colon];
            if !before.contains(['/', '?', '#']) {
                scheme = make_scheme(before)?;
                rest = &rest[colon + 1..];
            }
        }
        let (rest, fragment) = match rest.find('#') {
            Some(i) => (&rest[..i], Some(&rest[i + 1..])),
            None => (rest, None),
        };
        let (rest, query) = match rest.find('?') {
            Some(i) => (&rest[..i], Some(&rest[i + 1..])),
            None => (rest, None),
        };
        let mut user_info = String::new();
        let mut host = None;
        let mut port = None;
        let mut path = rest;
        if let Some(after) = rest.strip_prefix("//").or_else(|| rest.strip_prefix("\\\\")) {
            let end = after.find(['/', '\\']).unwrap_or(after.len());
            let authority = &after[..end];
            path = &after[end..];
            let host_port = match authority.rfind('@') {
                Some(i) => {
                    user_info = normalize(&authority[..i], USER_INFO_MASK, false);
                    &authority[i + 1..]
                }
                None => authority,
            };
            let (h, p) = if host_port.starts_with('[') {
                let close = host_port.find(']')?;
                let after_close = &host_port[close + 1..];
                match after_close.strip_prefix(':') {
                    Some(p) => (&host_port[..=close], Some(p)),
                    None if after_close.is_empty() => (&host_port[..=close], None),
                    None => return None,
                }
            } else {
                match host_port.rfind(':') {
                    Some(i) => (&host_port[..i], Some(&host_port[i + 1..])),
                    None => (host_port, None),
                }
            };
            if let Some(p) = p {
                if !p.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                if !p.is_empty() {
                    port = Some(p.trim_start_matches('0').to_string()).map(|s| {
                        if s.is_empty() { "0".to_string() } else { s }
                    });
                }
            }
            host = Some(normalize(h, HOST_MASK, false).to_ascii_lowercase());
        }
        if host.is_none() && scheme == "file" {
            host = Some(String::new());
        }
        let path = make_path(path, &scheme, host.is_some());
        Some(Uri {
            scheme,
            user_info,
            host,
            port,
            path,
            query: query.map(|q| normalize(q, QUERY_MASK, false)),
            fragment: fragment.map(|f| normalize(f, QUERY_MASK, false)),
        })
    }

    /// Dart `isAbsolute`: a scheme and no fragment.
    pub fn is_absolute(&self) -> bool {
        !self.scheme.is_empty() && self.fragment.is_none()
    }

    pub fn has_authority(&self) -> bool {
        self.host.is_some()
    }

    pub fn has_absolute_path(&self) -> bool {
        self.path.starts_with('/')
    }

    /// Dart `resolveRelativeUri(base, contained)`: `dart:core` resolves
    /// relative to `dart:core/`.
    pub fn resolve_relative(base: &Uri, contained: &Uri) -> Uri {
        if contained.is_absolute() {
            return contained.clone();
        }
        if base.scheme == "dart" && !base.path.contains('/') {
            let adjusted = Uri {
                path: format!("{}/", base.path),
                ..base.clone()
            };
            return adjusted.resolve_uri(contained);
        }
        base.resolve_uri(contained)
    }

    /// Dart `_Uri.resolveUri` (RFC 3986 with the Dart extensions).
    pub fn resolve_uri(&self, reference: &Uri) -> Uri {
        let target_scheme;
        let mut target_user_info = String::new();
        let target_host;
        let mut target_port = None;
        let target_path;
        let mut target_query = None;
        if !reference.scheme.is_empty() {
            target_scheme = reference.scheme.clone();
            if reference.has_authority() {
                target_user_info = reference.user_info.clone();
                target_port = reference.port.clone();
            }
            target_host = reference.host.clone();
            target_path = remove_dot_segments(&reference.path);
            target_query = reference.query.clone();
        } else {
            target_scheme = self.scheme.clone();
            if reference.has_authority() {
                target_user_info = reference.user_info.clone();
                target_host = reference.host.clone();
                target_port = reference.port.clone();
                target_path = remove_dot_segments(&reference.path);
                target_query = reference.query.clone();
            } else {
                target_user_info = self.user_info.clone();
                target_host = self.host.clone();
                target_port = self.port.clone();
                if reference.path.is_empty() {
                    target_path = self.path.clone();
                    target_query = if reference.query.is_some() {
                        reference.query.clone()
                    } else {
                        self.query.clone()
                    };
                } else {
                    let base_path = &self.path;
                    let package_name_end = self.package_name_end();
                    if package_name_end > 0 {
                        let package_name = &base_path[..package_name_end];
                        if reference.has_absolute_path() {
                            target_path =
                                format!("{package_name}{}", remove_dot_segments(&reference.path));
                        } else {
                            target_path = format!(
                                "{package_name}{}",
                                remove_dot_segments(&merge_paths(
                                    &base_path[package_name.len()..],
                                    &reference.path
                                ))
                            );
                        }
                    } else if reference.has_absolute_path() {
                        target_path = remove_dot_segments(&reference.path);
                    } else if self.path.is_empty() {
                        if !self.has_authority() {
                            if self.scheme.is_empty() {
                                target_path = reference.path.clone();
                            } else {
                                target_path = remove_dot_segments(&reference.path);
                            }
                        } else {
                            target_path = remove_dot_segments(&format!("/{}", reference.path));
                        }
                    } else {
                        let merged = merge_paths(&self.path, &reference.path);
                        if !self.scheme.is_empty()
                            || self.has_authority()
                            || self.has_absolute_path()
                        {
                            target_path = remove_dot_segments(&merged);
                        } else {
                            target_path = normalize_relative_path(
                                &merged,
                                !self.scheme.is_empty() || self.has_authority(),
                            );
                        }
                    }
                    if reference.query.is_some() {
                        target_query = reference.query.clone();
                    }
                }
            }
        }
        Uri {
            scheme: target_scheme,
            user_info: target_user_info,
            host: target_host,
            port: target_port,
            path: target_path,
            query: target_query,
            fragment: reference.fragment.clone(),
        }
    }

    /// Dart `_packageNameEnd`: the index of the `/` after the package name
    /// of a `package:` URI without authority, or -1.
    fn package_name_end(&self) -> isize {
        if self.scheme == "package" && !self.has_authority() {
            skip_package_name_chars(&self.path)
        } else {
            -1
        }
    }

    /// The first path segment of a `package:` URI (Dart
    /// `pathSegments.first`), when the URI has at least two segments.
    pub fn package_name(&self) -> Option<&str> {
        if self.scheme != "package" {
            return None;
        }
        let (name, _) = self.path.split_once('/')?;
        Some(name)
    }
}

impl fmt::Display for Uri {
    /// Dart `Uri.toString()`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.scheme.is_empty() {
            write!(f, "{}:", self.scheme)?;
        }
        if let Some(host) = &self.host {
            f.write_str("//")?;
            if !self.user_info.is_empty() {
                write!(f, "{}@", self.user_info)?;
            }
            f.write_str(host)?;
            if let Some(port) = &self.port {
                write!(f, ":{port}")?;
            }
        }
        f.write_str(&self.path)?;
        if let Some(query) = &self.query {
            write!(f, "?{query}")?;
        }
        if let Some(fragment) = &self.fragment {
            write!(f, "#{fragment}")?;
        }
        Ok(())
    }
}

/// Dart `_makeScheme`: `None` for an invalid scheme (Dart throws a
/// `FormatException`, so `tryParse` returns `null`).
fn make_scheme(scheme: &str) -> Option<String> {
    let bytes = scheme.as_bytes();
    let first = *bytes.first()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if !bytes
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
    {
        return None;
    }
    Some(scheme.to_ascii_lowercase())
}

/// Dart `_makePath` + `_normalizePath`.
fn make_path(path: &str, scheme: &str, has_authority: bool) -> String {
    let is_file = scheme == "file";
    let ensure_leading_slash = is_file || has_authority;
    let mut result = normalize(path, PATH_MASK, true);
    if result.is_empty() {
        if is_file {
            return "/".to_string();
        }
    } else if ensure_leading_slash && !result.starts_with('/') {
        result.insert(0, '/');
    }
    if scheme.is_empty() && !has_authority && !result.starts_with('/') {
        normalize_relative_path(&result, false)
    } else {
        remove_dot_segments(&result)
    }
}

const UNRESERVED: u8 = 1;
const SUB_DELIM: u8 = 2;
const COLON_AT: u8 = 4;
const SLASH: u8 = 8;
const QUESTION: u8 = 16;

const PATH_MASK: u8 = UNRESERVED | SUB_DELIM | COLON_AT | SLASH;
const QUERY_MASK: u8 = PATH_MASK | QUESTION;
const USER_INFO_MASK: u8 = UNRESERVED | SUB_DELIM;
const HOST_MASK: u8 = UNRESERVED | SUB_DELIM | COLON_AT;

fn char_class(b: u8) -> u8 {
    match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => UNRESERVED,
        b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'=' => SUB_DELIM,
        b':' | b'@' => COLON_AT,
        b'/' => SLASH,
        b'?' => QUESTION,
        _ => 0,
    }
}

/// Dart `_normalizeOrSubstring` with `_normalizeEscape` and `_escapeChar`:
/// keeps the characters of [mask], normalizes valid `%XX` escapes (an
/// unreserved character is decoded, hex digits are uppercased), escapes an
/// invalid `%` as `%25` and every other character as UTF-8 `%XX`.
fn normalize(text: &str, mask: u8, replace_backslash: bool) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' {
            let hex = |c: u8| (c as char).to_digit(16);
            match (bytes.get(i + 1), bytes.get(i + 2)) {
                (Some(&h1), Some(&h2)) if hex(h1).is_some() && hex(h2).is_some() => {
                    let value = (hex(h1).unwrap() * 16 + hex(h2).unwrap()) as u8;
                    if char_class(value) == UNRESERVED {
                        out.push(value as char);
                    } else {
                        out.push('%');
                        out.push(h1.to_ascii_uppercase() as char);
                        out.push(h2.to_ascii_uppercase() as char);
                    }
                    i += 3;
                }
                _ => {
                    out.push_str("%25");
                    i += 1;
                }
            }
            continue;
        }
        if b == b'\\' && replace_backslash {
            out.push('/');
        } else if b < 0x80 && char_class(b) & mask != 0 {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
        i += 1;
    }
    out
}

/// Dart `_mayContainDotSegments`.
fn may_contain_dot_segments(path: &str) -> bool {
    path.starts_with('.') || path.contains("/.")
}

/// Dart `_removeDotSegments`.
pub fn remove_dot_segments(path: &str) -> String {
    if !may_contain_dot_segments(path) {
        return path.to_string();
    }
    let mut output: Vec<&str> = Vec::new();
    let mut append_slash = false;
    for segment in path.split('/') {
        append_slash = false;
        if segment == ".." {
            if !output.is_empty() {
                output.pop();
                if output.is_empty() {
                    output.push("");
                }
            }
            append_slash = true;
        } else if segment == "." {
            append_slash = true;
        } else {
            output.push(segment);
        }
    }
    if append_slash {
        output.push("");
    }
    output.join("/")
}

/// Dart `_normalizeRelativePath`.
fn normalize_relative_path(path: &str, allow_scheme: bool) -> String {
    if !may_contain_dot_segments(path) {
        return if allow_scheme {
            path.to_string()
        } else {
            escape_scheme(path)
        };
    }
    let mut output: Vec<String> = Vec::new();
    let mut append_slash = false;
    for segment in path.split('/') {
        if segment == ".." {
            append_slash = true;
            if output.last().is_some_and(|l| l != "..") {
                output.pop();
            } else {
                output.push("..".to_string());
            }
        } else if segment == "." {
            append_slash = true;
        } else {
            append_slash = false;
            if segment.is_empty() && output.is_empty() {
                output.push("./".to_string());
            } else {
                output.push(segment.to_string());
            }
        }
    }
    if output.is_empty() {
        return "./".to_string();
    }
    if append_slash {
        output.push(String::new());
    }
    if !allow_scheme {
        output[0] = escape_scheme(&output[0]);
    }
    output.join("/")
}

/// Dart `_escapeScheme`: escapes a `:` in a first segment that looks like
/// a scheme.
fn escape_scheme(path: &str) -> String {
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() {
        for (i, &c) in bytes.iter().enumerate().skip(1) {
            if c == b':' {
                return format!("{}%3A{}", &path[..i], &path[i + 1..]);
            }
            if !(c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.')) {
                break;
            }
        }
    }
    path.to_string()
}

/// Dart `_mergePaths`.
fn merge_paths(base: &str, reference: &str) -> String {
    let mut back_count = 0usize;
    let mut ref_start = 0usize;
    while reference[ref_start..].starts_with("../") {
        ref_start += 3;
        back_count += 1;
    }
    let bytes = base.as_bytes();
    let mut base_end = base.rfind('/').map(|i| i as isize).unwrap_or(-1);
    while base_end > 0 && back_count > 0 {
        let new_end = match base[..base_end as usize].rfind('/') {
            Some(i) => i as isize,
            None => break,
        };
        let delta = base_end - new_end;
        if (delta == 2 || delta == 3)
            && bytes[(new_end + 1) as usize] == b'.'
            && (delta == 2 || bytes[(new_end + 2) as usize] == b'.')
        {
            break;
        }
        base_end = new_end;
        back_count -= 1;
    }
    format!(
        "{}{}",
        &base[..(base_end + 1) as usize],
        &reference[ref_start - 3 * back_count..]
    )
}

/// Dart `_skipPackageNameChars`: the index of the `/` that ends a valid
/// package name at the start of [path], or -1.
fn skip_package_name_chars(path: &str) -> isize {
    let mut dots = true;
    for (i, c) in path.bytes().enumerate() {
        if c == b'/' {
            return if dots || i == 0 { -1 } else { i as isize };
        }
        if c == b'%' || c == b':' {
            return -1;
        }
        dots &= c == b'.';
    }
    -1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Option<String> {
        Uri::try_parse(s).map(|u| u.to_string())
    }

    fn resolve(base: &str, r: &str) -> String {
        let base = Uri::try_parse(base).unwrap();
        let r = Uri::try_parse(r).unwrap();
        Uri::resolve_relative(&base, &r).to_string()
    }

    /// Expected values from `Uri.tryParse(s).toString()` and
    /// `base.resolveUri(...)` of the Dart 3.13 VM.
    #[test]
    fn parse_and_resolve_like_dart() {
        assert_eq!(parse("%zz").as_deref(), Some("%25zz"));
        assert_eq!(parse("a b.dart").as_deref(), Some("a%20b.dart"));
        assert_eq!(parse("Package:Foo/../x.dart").as_deref(), Some("package:/x.dart"));
        assert_eq!(parse("http://[::1"), None);
        assert_eq!(parse(":foo"), None);
        assert_eq!(parse("1a:b"), None);
        assert_eq!(parse("a:b:c").as_deref(), Some("a:b:c"));
        assert_eq!(parse("foo\\bar.dart").as_deref(), Some("foo/bar.dart"));
        assert_eq!(parse("é.dart").as_deref(), Some("%C3%A9.dart"));
        assert_eq!(parse("%41.dart").as_deref(), Some("A.dart"));
        assert_eq!(parse("a/./b/../c.dart").as_deref(), Some("a/c.dart"));
        assert_eq!(parse("file:///a/../b.dart").as_deref(), Some("file:///b.dart"));
        assert_eq!(parse("package:").as_deref(), Some("package:"));
        assert_eq!(parse("#x").as_deref(), Some("#x"));
        let base = "package:foo/src/a.dart";
        assert_eq!(resolve(base, "b.dart"), "package:foo/src/b.dart");
        assert_eq!(resolve(base, "../../../b.dart"), "package:foo/b.dart");
        assert_eq!(resolve(base, "/b.dart"), "package:foo/b.dart");
        assert_eq!(resolve(base, "."), "package:foo/src/");
        assert_eq!(resolve("dart:core", "list.dart"), "dart:core/list.dart");
        assert_eq!(
            resolve("file:///a/b/c.dart", "../d e.dart"),
            "file:///a/d%20e.dart"
        );
    }
}
