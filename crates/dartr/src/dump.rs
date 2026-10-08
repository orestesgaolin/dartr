//! `dartr dump <mode>`: writes the same JSON Lines as `tools/oracle`
//! (`dart run tools/oracle/bin/oracle.dart <mode>`), one object per file.

use std::fmt::Write as _;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use dartr_syntax::{Diagnostic, TokenId, Tokens, scan_for_analyzer, strip_bom};
use rayon::prelude::*;

use crate::json::write_string;

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum DumpMode {
    /// Token stream and scanner diagnostics.
    Tokens,
    /// Unresolved AST and parse diagnostics (not implemented yet).
    Ast,
    /// Resolved diagnostics and expression types (not implemented yet).
    Resolved,
    /// Element model of each library (docs/design/semantics.md §5.1).
    /// Skeleton: every path gives `{"path":..,"error":"not implemented"}`.
    Elements,
}

/// Runs `dump` for [files], or for the paths on stdin (one per line) when
/// [files] is empty.
pub fn run(mode: DumpMode, files: Vec<PathBuf>) -> anyhow::Result<()> {
    let paths: Vec<PathBuf> = if files.is_empty() {
        io::stdin()
            .lock()
            .lines()
            .map_while(Result::ok)
            .filter(|l| !l.is_empty())
            .map(PathBuf::from)
            .collect()
    } else {
        files
    };
    let cwd = std::env::current_dir()?;
    // Dart `File(p).absolute.path`: the current directory is prepended to a
    // relative path, without normalization.
    let paths: Vec<String> = paths
        .iter()
        .map(|p| {
            // A `dart:` library URI (mode `elements`) is kept as it is.
            if p.is_absolute() || p.to_str().is_some_and(|s| s.starts_with("dart:")) {
                p.to_string_lossy().into_owned()
            } else {
                cwd.join(p).to_string_lossy().into_owned()
            }
        })
        .collect();

    let dump: fn(&str) -> String = match mode {
        DumpMode::Tokens => dump_tokens,
        DumpMode::Elements => dump_elements,
        DumpMode::Ast | DumpMode::Resolved => {
            anyhow::bail!("dump mode {mode:?} is not implemented yet")
        }
    };

    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    // Bounded chunks keep the output order and limit memory.
    for chunk in paths.chunks(512) {
        let lines: Vec<String> = chunk.par_iter().map(|p| dump(p)).collect();
        for line in lines {
            out.write_all(line.as_bytes())?;
            out.write_all(b"\n")?;
        }
    }
    out.flush()?;
    Ok(())
}

/// Reads a file like Dart `File.readAsStringSync()` (UTF-8, a leading byte
/// order mark is removed). On failure, returns the Dart exception type.
fn read_source(path: &str) -> Result<String, &'static str> {
    let bytes = match std::fs::read(Path::new(path)) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err("PathNotFoundException"),
        Err(_) => return Err("FileSystemException"),
    };
    let text = String::from_utf8(bytes).map_err(|_| "FileSystemException")?;
    if strip_bom(&text).len() == text.len() {
        Ok(text)
    } else {
        Ok(strip_bom(&text).to_string())
    }
}

fn error_json(path: &str, error: &str) -> String {
    let mut out = String::new();
    out.push_str("{\"path\":");
    write_string(&mut out, path);
    out.push_str(",\"error\":");
    write_string(&mut out, error);
    out.push('}');
    out
}

/// One line of `dump elements`. Not implemented yet: the element model is
/// built in `dartr_element` (phase 5).
pub fn dump_elements(path: &str) -> String {
    error_json(path, "not implemented")
}

/// One line of `dump tokens`.
pub fn dump_tokens(path: &str) -> String {
    let source = match read_source(path) {
        Ok(s) => s,
        Err(e) => return error_json(path, e),
    };
    let result = scan_for_analyzer(&source);
    let tokens = result.tokens();
    let mut out = String::with_capacity(source.len() * 4 + 64);
    out.push_str("{\"path\":");
    write_string(&mut out, path);
    out.push_str(",\"tokens\":[");
    let mut first = true;
    for id in tokens.iter_from(result.first) {
        if !first {
            out.push(',');
        }
        first = false;
        write_token(&mut out, tokens, id, true);
    }
    out.push_str("],\"diagnostics\":[");
    for (i, d) in result.diagnostics.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_diagnostic(&mut out, d);
    }
    out.push_str("]}");
    out
}

/// `tokenJson` of the oracle.
fn write_token(out: &mut String, tokens: &Tokens, id: TokenId, with_comments: bool) {
    let t = tokens.get(id);
    out.push_str("{\"k\":\"");
    out.push_str(t.ty.name());
    let _ = write!(out, "\",\"o\":{},\"l\":{},\"x\":", t.offset, t.length);
    write_string(out, tokens.lexeme(id));
    if t.is_synthetic() {
        out.push_str(",\"syn\":true");
    }
    if with_comments && t.preceding_comments.is_some() {
        out.push_str(",\"c\":[");
        for (i, c) in tokens.comments(id).enumerate() {
            if i > 0 {
                out.push(',');
            }
            write_token(out, tokens, c, false);
        }
        out.push(']');
    }
    out.push('}');
}

/// `diagnosticJson` of the oracle.
fn write_diagnostic(out: &mut String, d: &Diagnostic) {
    out.push_str("{\"code\":");
    write_string(out, d.code.lower_case_name());
    out.push_str(",\"severity\":");
    write_string(out, d.severity().lower_name());
    let _ = write!(out, ",\"o\":{},\"l\":{},\"msg\":", d.offset, d.length);
    write_string(out, &d.message);
    out.push('}');
}
