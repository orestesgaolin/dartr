//! Path helpers with the semantics of `package:path` (posix style), which the
//! analyzer uses for all path computations.
//!
//! Paths are plain strings. The analyzer requires absolute, normalized paths
//! for every resource, and this crate keeps the same rule.

/// The path separator.
pub const SEPARATOR: char = '/';

/// Returns `true` if [path] is absolute.
pub fn is_absolute(path: &str) -> bool {
    path.starts_with('/')
}

/// Normalizes [path]: removes `.` segments, resolves `..` segments, removes
/// duplicate and trailing separators. Same as `p.normalize`.
pub fn normalize(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let absolute = is_absolute(path);
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if let Some(last) = parts.last()
                    && *last != ".."
                {
                    parts.pop();
                    continue;
                }
                if !absolute {
                    parts.push("..");
                }
            }
            _ => parts.push(part),
        }
    }
    let joined = parts.join("/");
    if absolute {
        format!("/{joined}")
    } else if joined.is_empty() {
        ".".to_string()
    } else {
        joined
    }
}

/// Joins [base] and [child]. If [child] is absolute, returns [child].
pub fn join(base: &str, child: &str) -> String {
    if is_absolute(child) || base.is_empty() {
        child.to_string()
    } else if base.ends_with('/') {
        format!("{base}{child}")
    } else {
        format!("{base}/{child}")
    }
}

/// Returns the absolute normalized form of [path], relative to the current
/// directory. Same as `p.normalize(p.absolute(path))`.
pub fn absolute_normalized(path: &str) -> String {
    if is_absolute(path) {
        normalize(path)
    } else {
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "/".to_string());
        normalize(&join(&cwd, path))
    }
}

/// Returns the parent directory of [path]. The parent of `/` is `/`.
pub fn dirname(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return if path.starts_with('/') { "/" } else { "." };
    }
    match trimmed.rfind('/') {
        Some(0) => "/",
        Some(index) => trimmed[..index].trim_end_matches('/'),
        None => ".",
    }
}

/// Returns the last component of [path]. Same as `p.basename`.
pub fn basename(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return if path.starts_with('/') { "/" } else { "" };
    }
    match trimmed.rfind('/') {
        Some(index) => &trimmed[index + 1..],
        None => trimmed,
    }
}

/// Returns the extension of the last component of [path], including the dot,
/// or an empty string. Same as `p.extension`.
pub fn extension(path: &str) -> &str {
    let name = basename(path);
    if name == ".." {
        return "";
    }
    match name.rfind('.') {
        Some(index) if index > 0 => &name[index..],
        _ => "",
    }
}

/// Splits [path] into components. The root of an absolute path is the first
/// component. Same as `p.split`.
pub fn split(path: &str) -> Vec<&str> {
    let mut result = Vec::new();
    if is_absolute(path) {
        result.push("/");
    }
    result.extend(path.split('/').filter(|part| !part.is_empty()));
    result
}

/// Returns `true` if [child] is strictly inside [parent]. Both paths must be
/// absolute and normalized. Same as `p.isWithin` for such paths.
pub fn is_within(parent: &str, child: &str) -> bool {
    if parent == "/" {
        return child.len() > 1 && child.starts_with('/');
    }
    child.len() > parent.len()
        && child.starts_with(parent)
        && child.as_bytes()[parent.len()] == b'/'
}

/// Returns `true` if [path] is [parent] or inside it.
pub fn is_or_within(parent: &str, path: &str) -> bool {
    parent == path || is_within(parent, path)
}

/// Returns [path] relative to [from]. Both must be absolute and normalized.
/// Same as `p.relative(path, from: from)`.
pub fn relative(path: &str, from: &str) -> String {
    let path_parts = split(path);
    let from_parts = split(from);
    let mut common = 0;
    while common < path_parts.len()
        && common < from_parts.len()
        && path_parts[common] == from_parts[common]
    {
        common += 1;
    }
    let mut parts: Vec<&str> = vec![".."; from_parts.len() - common];
    parts.extend(&path_parts[common..]);
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

/// Returns the path relative to [parent] if [path] is strictly inside it.
/// Same as `Folder.relativeIfContains`.
pub fn relative_if_within<'a>(parent: &str, path: &'a str) -> Option<&'a str> {
    if is_within(parent, path) {
        if parent == "/" {
            Some(&path[1..])
        } else {
            Some(&path[parent.len() + 1..])
        }
    } else {
        None
    }
}

/// Iterates [folder] and its ancestors up to the root. Same as
/// `Folder.withAncestors`.
pub fn with_ancestors(folder: &str) -> impl Iterator<Item = &str> {
    let mut current = Some(folder);
    std::iter::from_fn(move || {
        let result = current?;
        current = if result == "/" {
            None
        } else {
            Some(dirname(result))
        };
        Some(result)
    })
}

/// Converts an absolute path to a `file:` URI string, with the escaping of
/// Dart's `Uri.file`.
pub fn to_file_uri(path: &str) -> String {
    let mut result = String::from("file://");
    for segment in path.split('/').skip(1) {
        result.push('/');
        result.push_str(&encode_path_segment(segment));
    }
    if path == "/" {
        result.push('/');
    }
    result
}

/// Percent-encodes a URI path segment like Dart's `Uri` does: unreserved
/// characters, sub-delimiters, `:` and `@` stay as they are.
pub fn encode_path_segment(segment: &str) -> String {
    let mut result = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        let keep = byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'.'
                    | b'_'
                    | b'~'
                    | b'!'
                    | b'$'
                    | b'&'
                    | b'\''
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b'='
                    | b':'
                    | b'@'
            );
        if keep {
            result.push(byte as char);
        } else {
            result.push_str(&format!("%{byte:02X}"));
        }
    }
    result
}

/// Decodes `%XX` escapes in [text]. Invalid escapes are kept as they are.
pub fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 3 <= bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(value) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(value);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Converts a `file:` URI to a normalized absolute path, or `None` if [uri]
/// is not a `file:` URI with an absolute path.
pub fn file_uri_to_path(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://")?;
    // Skip an authority, if any (only the empty authority and `localhost`).
    let path = if rest.starts_with('/') {
        rest
    } else {
        let slash = rest.find('/')?;
        &rest[slash..]
    };
    let path = path.split(['?', '#']).next().unwrap_or("");
    Some(normalize(&percent_decode(path)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_and_relative_match_package_path() {
        assert_eq!(normalize("/a/./b/../c//d/"), "/a/c/d");
        assert_eq!(normalize("a/../../b"), "../b");
        assert_eq!(normalize("/.."), "/");
        assert_eq!(relative("/a/b/c", "/a/d"), "../b/c");
        assert_eq!(relative("/a", "/a"), ".");
        assert_eq!(extension("/a/b.g.dart"), ".dart");
        assert_eq!(extension("/a/.dart"), "");
        assert_eq!(dirname("/a"), "/");
        assert_eq!(
            with_ancestors("/a/b").collect::<Vec<_>>(),
            ["/a/b", "/a", "/"]
        );
        assert_eq!(to_file_uri("/a b/c"), "file:///a%20b/c");
        assert_eq!(
            file_uri_to_path("file:///a%20b/./c").as_deref(),
            Some("/a b/c")
        );
    }
}
