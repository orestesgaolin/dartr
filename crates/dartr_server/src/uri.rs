// Dart source: pkg/analysis_server/lib/src/lsp/client_uri_converter.dart (file URIs only)

//! Conversion between file paths and `file:` URIs, like Dart `Uri.file`
//! and `Uri.toFilePath` on POSIX systems.

/// Dart `Uri.file(path).toString()` for an absolute POSIX path.
pub fn path_to_uri(path: &str) -> String {
    let mut out = String::from("file://");
    for b in path.bytes() {
        // The characters that Dart's path encoding keeps (`_pathCharTable`).
        let keep = b.is_ascii_alphanumeric()
            || matches!(
                b,
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
                    | b'/'
            );
        if keep {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The reason why a URI is not a usable file URI (Dart `pathOfUri`).
#[derive(Debug, PartialEq, Eq)]
pub enum UriError {
    /// No scheme: `URI is not a valid file:// URI`.
    NoScheme,
    /// Another scheme than `file`.
    UnsupportedScheme(String),
    /// A file URI that does not map to an absolute path.
    Invalid,
}

/// Dart `Uri.parse(uri).toFilePath()` for `file:` URIs.
pub fn uri_to_path(uri: &str) -> Result<String, UriError> {
    let Some(colon) = uri.find(':') else {
        return Err(UriError::NoScheme);
    };
    let scheme = &uri[..colon];
    if scheme.is_empty() || scheme.contains('/') {
        return Err(UriError::NoScheme);
    }
    if !scheme.eq_ignore_ascii_case("file") {
        return Err(UriError::UnsupportedScheme(scheme.to_string()));
    }
    let rest = &uri[colon + 1..];
    if rest.contains('?') || rest.contains('#') {
        return Err(UriError::Invalid);
    }
    let path = match rest.strip_prefix("//") {
        Some(after) => {
            // Skip the authority (empty or `localhost`).
            let slash = after.find('/').unwrap_or(after.len());
            let authority = &after[..slash];
            if !authority.is_empty() && authority != "localhost" {
                return Err(UriError::Invalid);
            }
            &after[slash..]
        }
        None => rest,
    };
    if !path.starts_with('/') {
        return Err(UriError::Invalid);
    }
    let decoded = percent_decode(path).ok_or(UriError::Invalid)?;
    Ok(normalize(&decoded))
}

fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Dart `pathContext.normalize` for absolute POSIX paths: removes `.`, `..`,
/// duplicate and trailing separators.
pub fn normalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    format!("/{}", parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        for p in ["/a/b c/d.dart", "/x/ü/%.dart", "/a/b"] {
            assert_eq!(uri_to_path(&path_to_uri(p)).unwrap(), p);
        }
        assert_eq!(path_to_uri("/a/b c.dart"), "file:///a/b%20c.dart");
    }

    #[test]
    fn errors() {
        assert_eq!(uri_to_path("a/b.dart"), Err(UriError::NoScheme));
        assert_eq!(
            uri_to_path("untitled:Untitled-1"),
            Err(UriError::UnsupportedScheme("untitled".into()))
        );
        assert_eq!(uri_to_path("file:///a/../b/./c/"), Ok("/b/c".into()));
    }
}
