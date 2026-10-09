//! `dartr dump <mode>`: writes the same JSON Lines as `tools/oracle`
//! (`dart run tools/oracle/bin/oracle.dart <mode>`), one object per file.

use std::fmt::Write as _;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use dartr_parser::event_recorder::EventRecorder;
use dartr_parser::parse_for_analyzer;
use dartr_syntax::{
    Diagnostic, TokenId, Tokens, scan_for_analyzer, severity_lower_name, strip_bom,
};
use rayon::prelude::*;

use crate::json::write_string;

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum DumpMode {
    /// Token stream and scanner diagnostics.
    Tokens,
    /// Parser events: every listener call of the parser, the recoverable
    /// errors and the token stream after parsing.
    Events,
    /// Unresolved AST (child entities) and parse diagnostics.
    Ast,
    /// Diagnostics and static types of expressions of each resolved
    /// library (docs/design/semantics.md §5.4,
    /// `tools/oracle/bin/resolved_el.dart`).
    Resolved,
    /// The elements of the identifiers, named types and constructor names of
    /// each resolved library (docs/design/semantics.md §5.4).
    ResolvedEl,
    /// Element model of each library (docs/design/semantics.md §5.1).
    Elements,
    /// Interfaces of the classes of each library (docs/design/semantics.md
    /// §5.2, `tools/oracle/bin/interface.dart`). Skeleton: every path gives
    /// `{"path":..,"error":"not implemented"}` until the linker builds the
    /// library elements; then each line is
    /// `dartr_typesystem::interface_dump::interface_library_json`.
    Interface,
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
            // A `dart:` library URI (modes `elements`, `interface`) is kept
            // as it is.
            if p.is_absolute() || p.to_str().is_some_and(|s| s.starts_with("dart:")) {
                p.to_string_lossy().into_owned()
            } else {
                cwd.join(p).to_string_lossy().into_owned()
            }
        })
        .collect();

    let dump: fn(&str) -> String = match mode {
        DumpMode::Tokens => dump_tokens,
        DumpMode::Events => dump_events,
        DumpMode::Elements | DumpMode::Interface => {
            // All inputs at once: they share the analysis contexts and the
            // linked library cycles.
            let pool = rayon::ThreadPoolBuilder::new()
                .stack_size(256 << 20)
                .build()?;
            let lines = pool.install(|| {
                crate::elements::dump_elements_all(&paths, mode == DumpMode::Interface)
            });
            let stdout = io::stdout();
            let mut out = io::BufWriter::new(stdout.lock());
            for line in lines {
                out.write_all(line.as_bytes())?;
                out.write_all(b"\n")?;
            }
            out.flush()?;
            return Ok(());
        }
        DumpMode::Ast => dump_ast,
        DumpMode::Resolved | DumpMode::ResolvedEl => {
            let pool = rayon::ThreadPoolBuilder::new()
                .stack_size(256 << 20)
                .build()?;
            let resolved_mode = if mode == DumpMode::Resolved {
                crate::resolved::ResolvedMode::Types
            } else {
                crate::resolved::ResolvedMode::Elements
            };
            let lines = pool.install(|| crate::resolved::dump_resolved_all(&paths, resolved_mode));
            let stdout = io::stdout();
            let mut out = io::BufWriter::new(stdout.lock());
            for line in lines {
                out.write_all(line.as_bytes())?;
                out.write_all(b"\n")?;
            }
            out.flush()?;
            return Ok(());
        }
    };

    // The parser recurses deeply on deeply nested code; use large stacks.
    let pool = rayon::ThreadPoolBuilder::new()
        .stack_size(256 << 20)
        .build()?;
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    // Bounded chunks keep the output order and limit memory.
    for chunk in paths.chunks(64) {
        let lines: Vec<String> = pool.install(|| chunk.par_iter().map(|p| dump(p)).collect());
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

/// One line of `dump events` (`tools/oracle/bin/events.dart`).
pub fn dump_events(path: &str) -> String {
    let source = match read_source(path) {
        Ok(s) => s,
        Err(e) => return error_json(path, e),
    };
    let result = parse_for_analyzer(&source, EventRecorder::new());
    let tokens = &result.tokens;
    let recorder = &result.listener;
    let mut out = String::with_capacity(recorder.out.len() + source.len() * 2 + 64);
    out.push_str("{\"path\":");
    write_string(&mut out, path);
    out.push_str(",\"events\":[");
    out.push_str(&recorder.out);
    out.push_str("],\"errors\":[");
    out.push_str(&recorder.errors);
    out.push_str("],\"stream\":[");
    let mut first = true;
    for id in tokens.iter_from(tokens.next(result.before_first)) {
        if !first {
            out.push(',');
        }
        first = false;
        let t = tokens.get(id);
        let _ = write!(out, "[{},", t.offset as i32);
        write_string(&mut out, tokens.lexeme(id));
        if t.is_synthetic() {
            out.push_str(",\"");
            out.push_str(t.ty.name());
            out.push('"');
        }
        out.push(']');
    }
    out.push_str("]}");
    out
}

/// One line of `dump ast` (oracle `dumpAst`): the AST of `parseString` as
/// child entities, and the parse diagnostics.
pub fn dump_ast(path: &str) -> String {
    let source = match read_source(path) {
        Ok(s) => s,
        Err(e) => return error_json(path, e),
    };
    let parsed = dartr_ast_builder::parse_string(&source, path);
    let mut out = String::with_capacity(source.len() * 8 + 64);
    out.push_str("{\"path\":");
    write_string(&mut out, path);
    out.push_str(",\"ast\":");
    dartr_ast::dump::write_node_json(&mut out, &parsed.ast, parsed.unit.raw());
    out.push_str(",\"diagnostics\":[");
    for (i, d) in parsed.diagnostics.iter().enumerate() {
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
    write_string(out, severity_lower_name(d.severity));
    let _ = write!(out, ",\"o\":{},\"l\":{},\"msg\":", d.offset, d.length);
    write_string(out, &d.message);
    out.push('}');
}
