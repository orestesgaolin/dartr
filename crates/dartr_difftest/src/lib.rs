//! Differential tests: runs the Dart oracle (`tools/oracle`) and
//! `dartr dump <mode>` on the same files and compares the JSON Lines output
//! per file.
//!
//! Both programs read the file paths from stdin (one per line) and write one
//! JSON object per file with a `path` key. The comparison is generic over
//! the mode: the raw lines are compared first, and for a difference the
//! first differing JSON path is reported.
//!
//! With [`Options::mask_inferred`], the `"type"` of every JSON object that
//! has `"inf": true` is replaced by `"<inferred>"` on both sides before the
//! comparison (the `elements` mode before top-level inference exists).
//!
//! Modes `resolved` and `resolved-el` (lines with `"units"`, see
//! `tools/oracle/bin/resolved_el.dart`):
//! - [`Options::kinds`] keeps only the entries of the given node kinds
//!   (`"k"`) in the `"types"` and `"nodes"` lists before the comparison;
//! - [`Options::no_diagnostics`] removes the `"diagnostics"` lists;
//! - the report has a parity per node kind ([`Report::kinds`]): for each
//!   kind, the number of oracle entries and the number of them that dartr
//!   has with the same value (same offsets, kind, type or element), summed
//!   over all files.

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde_json::Value;

/// Options of a difftest run.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// `tokens`, `events`, `ast`, `resolved` or `elements`.
    pub mode: String,
    /// Files or directories (searched recursively for `.dart` files).
    pub inputs: Vec<PathBuf>,
    /// Number of batches that run at the same time.
    pub jobs: usize,
    /// Number of files per batch (0: split the files evenly over `jobs`).
    pub batch_size: usize,
    /// When set, the oracle and dartr output of each differing file are
    /// written to this directory.
    pub write_failures: Option<PathBuf>,
    /// The `dartr` binary.
    pub dartr: PathBuf,
    /// The oracle command (program and leading arguments); the mode is
    /// appended. Empty: use [`ensure_oracle`].
    pub oracle: Vec<String>,
    /// Replace the `"type"` of each JSON object with `"inf": true` by
    /// `"<inferred>"` on both sides before the comparison (see
    /// [`mask_inferred`]).
    pub mask_inferred: bool,
    /// When not empty: compare only the entries of these node kinds in the
    /// `"types"` and `"nodes"` lists (see [`filter_value`]).
    pub kinds: Vec<String>,
    /// Remove the `"diagnostics"` lists before the comparison.
    pub no_diagnostics: bool,
    /// The time limit per input file, in seconds: a batch is killed after
    /// `60 + files * timeout_per_file` seconds, so a hang in one input can
    /// not block the run. Its missing lines count as differences. `0`: no
    /// limit.
    pub timeout_per_file: u64,
}

impl Options {
    /// Whether the lines are changed before the comparison.
    fn normalizes(&self) -> bool {
        self.mask_inferred || !self.kinds.is_empty() || self.no_diagnostics
    }

    /// [line] after [mask_inferred] and [filter_value], serialized again
    /// (keys keep their order). A line that is not valid JSON is returned
    /// unchanged.
    fn normalize_line(&self, line: &str) -> String {
        match parse_deep(line) {
            Ok(mut v) => {
                self.normalize_value(&mut v);
                v.to_string()
            }
            Err(_) => line.to_string(),
        }
    }

    fn normalize_value(&self, v: &mut Value) {
        if self.mask_inferred {
            mask_inferred(v);
        }
        if !self.kinds.is_empty() || self.no_diagnostics {
            filter_value(v, &self.kinds, self.no_diagnostics);
        }
    }
}

/// Removes, in every JSON object of [value], the entries of the `"types"`
/// and `"nodes"` lists whose `"k"` is not in [kinds] (when [kinds] is not
/// empty), and the `"diagnostics"` key (when [no_diagnostics]).
pub fn filter_value(value: &mut Value, kinds: &[String], no_diagnostics: bool) {
    match value {
        Value::Object(map) => {
            if no_diagnostics {
                map.shift_remove("diagnostics");
            }
            for (key, v) in map.iter_mut() {
                if !kinds.is_empty()
                    && (key == "types" || key == "nodes")
                    && let Value::Array(items) = v
                {
                    items.retain(|item| {
                        item.get("k")
                            .and_then(Value::as_str)
                            .is_some_and(|k| kinds.iter().any(|x| x == k))
                    });
                    continue;
                }
                filter_value(v, kinds, no_diagnostics);
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|v| filter_value(v, kinds, no_diagnostics)),
        _ => {}
    }
}

/// The parity of one node kind (modes `resolved`, `resolved-el`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KindStats {
    /// Entries of this kind in the oracle output.
    pub oracle: usize,
    /// Oracle entries that dartr has with the same value.
    pub matched: usize,
    /// Entries of this kind in the dartr output.
    pub dartr: usize,
}

impl KindStats {
    pub fn parity(&self) -> f64 {
        if self.oracle == 0 {
            100.0
        } else {
            self.matched as f64 * 100.0 / self.oracle as f64
        }
    }

    fn add(&mut self, other: &KindStats) {
        self.oracle += other.oracle;
        self.matched += other.matched;
        self.dartr += other.dartr;
    }
}

/// The number of `"panic"` keys of a line: on the line itself (a panic
/// outside the units) and in its units.
pub fn count_panics(line: &Value) -> usize {
    let units = line
        .get("units")
        .and_then(Value::as_array)
        .map_or(0, |units| {
            units.iter().filter(|u| u.get("panic").is_some()).count()
        });
    usize::from(line.get("panic").is_some()) + units
}

/// Adds the per-kind counts of one file to [stats]. The units of the two
/// lines are paired by `"path"`; in each pair, an oracle entry of the
/// `"types"` or `"nodes"` list is matched when the dartr list of the same
/// unit has an equal entry that is not matched yet.
pub fn count_kinds(oracle: &Value, dartr: Option<&Value>, stats: &mut BTreeMap<String, KindStats>) {
    fn units(v: &Value) -> Vec<&Value> {
        v.get("units")
            .and_then(Value::as_array)
            .map(|a| a.iter().collect())
            .unwrap_or_default()
    }
    fn entries(unit: &Value) -> impl Iterator<Item = &Value> {
        ["types", "nodes"]
            .into_iter()
            .filter_map(|k| unit.get(k).and_then(Value::as_array))
            .flatten()
    }
    fn kind(entry: &Value) -> String {
        entry
            .get("k")
            .and_then(Value::as_str)
            .unwrap_or("<no kind>")
            .to_string()
    }
    let dartr_units = dartr.map(units).unwrap_or_default();
    for unit in units(oracle) {
        let path = unit.get("path");
        let other = dartr_units.iter().find(|u| u.get("path") == path);
        let mut available: HashMap<String, usize> = HashMap::new();
        if let Some(other) = other {
            for e in entries(other) {
                stats.entry(kind(e)).or_default().dartr += 1;
                *available.entry(e.to_string()).or_default() += 1;
            }
        }
        for e in entries(unit) {
            let s = stats.entry(kind(e)).or_default();
            s.oracle += 1;
            if let Some(n) = available.get_mut(&e.to_string())
                && *n > 0
            {
                *n -= 1;
                s.matched += 1;
            }
        }
    }
}

/// The first difference of one file.
#[derive(Clone, Debug)]
pub struct Difference {
    pub file: String,
    /// JSON path of the first difference, for example `tokens[12].x`.
    pub json_path: String,
    pub oracle: String,
    pub dartr: String,
}

#[derive(Clone, Debug, Default)]
pub struct Report {
    pub mode: String,
    pub files: usize,
    pub identical: usize,
    pub differences: Vec<Difference>,
    pub oracle_time: Duration,
    pub dartr_time: Duration,
    pub elapsed: Duration,
    /// Per node kind parity (modes `resolved`, `resolved-el`), by kind name.
    pub kinds: BTreeMap<String, KindStats>,
    /// Lines and units with a `"panic"` key in the dartr output (modes
    /// `resolved`, `resolved-el`).
    pub panics: usize,
}

impl Report {
    pub fn different(&self) -> usize {
        self.differences.len()
    }

    /// Percentage of identical files.
    pub fn parity(&self) -> f64 {
        if self.files == 0 {
            100.0
        } else {
            self.identical as f64 * 100.0 / self.files as f64
        }
    }

    /// The summary block (files, identical, different, parity).
    pub fn summary(&self) -> String {
        format!(
            "mode:      {}\nfiles:     {}\nidentical: {}\ndifferent: {}\nparity:    {:.2}%\ntime:      {:.1}s (oracle {:.1}s, dartr {:.1}s, summed over batches)\n",
            self.mode,
            self.files,
            self.identical,
            self.different(),
            self.parity(),
            self.elapsed.as_secs_f64(),
            self.oracle_time.as_secs_f64(),
            self.dartr_time.as_secs_f64(),
        )
    }

    /// The per-kind parity table, sorted by the number of oracle entries
    /// (empty when there are no per-kind counts).
    pub fn kind_report(&self) -> String {
        if self.kinds.is_empty() {
            return String::new();
        }
        let mut rows: Vec<(&String, &KindStats)> = self.kinds.iter().collect();
        rows.sort_by(|a, b| b.1.oracle.cmp(&a.1.oracle).then(a.0.cmp(b.0)));
        let mut total = KindStats::default();
        let mut s = format!(
            "{:<40} {:>9} {:>9} {:>8} {:>9}\n",
            "kind", "oracle", "matched", "parity", "dartr"
        );
        for (kind, k) in rows {
            total.add(k);
            s.push_str(&format!(
                "{:<40} {:>9} {:>9} {:>7.2}% {:>9}\n",
                kind,
                k.oracle,
                k.matched,
                k.parity(),
                k.dartr
            ));
        }
        s.push_str(&format!(
            "{:<40} {:>9} {:>9} {:>7.2}% {:>9}\n",
            "TOTAL",
            total.oracle,
            total.matched,
            total.parity(),
            total.dartr
        ));
        s.push_str(&format!(
            "dartr panics (lines and units): {}\n",
            self.panics
        ));
        s
    }

    /// The first-difference report of the first [max] differing files.
    pub fn difference_report(&self, max: usize) -> String {
        let mut s = String::new();
        for d in self.differences.iter().take(max) {
            s.push_str(&format!(
                "DIFF {}\n  at:     {}\n  oracle: {}\n  dartr:  {}\n",
                d.file,
                if d.json_path.is_empty() {
                    "<root>"
                } else {
                    &d.json_path
                },
                d.oracle,
                d.dartr
            ));
        }
        if self.differences.len() > max {
            s.push_str(&format!("... and {} more\n", self.differences.len() - max));
        }
        s
    }
}

/// Parses a JSON line without the recursion limit of `serde_json` (the
/// `ast` dump of deeply nested code).
fn parse_deep(s: &str) -> serde_json::Result<Value> {
    use serde::Deserialize;
    let mut de = serde_json::Deserializer::from_str(s);
    de.disable_recursion_limit();
    Value::deserialize(&mut de)
}

/// The `path` of a JSON line `{"path":"...",...}`, read without parsing the
/// whole line (deeply nested ASTs exceed the recursion limit of
/// `serde_json`).
fn path_of_line(line: &str) -> Option<String> {
    let rest = line.strip_prefix("{\"path\":")?;
    let bytes = rest.as_bytes();
    if bytes.first() != Some(&b'"') {
        return None;
    }
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return serde_json::from_str::<String>(&rest[..=i]).ok(),
            _ => i += 1,
        }
    }
    None
}

/// The repository root (from the location of this crate).
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

/// Collects the `.dart` files of [inputs] (sorted, absolute paths). An
/// input that starts with `dart:` (a library URI, mode `elements`) is passed
/// through unchanged.
pub fn collect_dart_files(inputs: &[PathBuf]) -> Result<Vec<String>> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
            let entry = entry?;
            let path = entry.path();
            let ty = entry.file_type()?;
            if ty.is_dir() {
                walk(&path, out)?;
            } else if ty.is_file() && path.extension().is_some_and(|e| e == "dart") {
                out.push(path);
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    let mut uris = Vec::new();
    for input in inputs {
        if let Some(uri) = input.to_str().filter(|s| s.starts_with("dart:")) {
            uris.push(uri.to_string());
            continue;
        }
        let input = std::path::absolute(input)?;
        if input.is_dir() {
            walk(&input, &mut files)?;
        } else if input.is_file() {
            files.push(input);
        } else {
            bail!("no such file or directory: {}", input.display());
        }
    }
    files.sort();
    files.dedup();
    let mut out: Vec<String> = files
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    uris.sort();
    uris.dedup();
    out.extend(uris);
    Ok(out)
}

/// Returns the oracle command: the AOT-compiled oracle in
/// `target/oracle/oracle`, compiled first when it is missing or older than
/// one of its sources (a file in `tools/oracle/bin/`, or
/// `tools/oracle/pubspec.lock`).
pub fn ensure_oracle() -> Result<Vec<String>> {
    let root = repo_root();
    let oracle_dir = root.join("tools/oracle");
    let source = oracle_dir.join("bin/oracle.dart");
    let exe = root.join("target/oracle/oracle");
    let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    // The oracle sources: every file in bin/ (oracle.dart imports the
    // other modes) and the package lock.
    let mut inputs = vec![oracle_dir.join("pubspec.lock")];
    if let Ok(entries) = std::fs::read_dir(oracle_dir.join("bin")) {
        inputs.extend(entries.filter_map(|e| e.ok()).map(|e| e.path()));
    }
    let stale = match modified(&exe) {
        None => true,
        Some(t) => inputs.iter().any(|p| modified(p).is_some_and(|s| s > t)),
    };
    if stale {
        if !oracle_dir.join(".dart_tool/package_config.json").exists() {
            let status = Command::new("dart")
                .arg("pub")
                .arg("get")
                .current_dir(&oracle_dir)
                .status()
                .context("running `dart pub get` for tools/oracle (is `dart` on PATH?)")?;
            if !status.success() {
                bail!("`dart pub get` failed in {}", oracle_dir.display());
            }
        }
        std::fs::create_dir_all(exe.parent().unwrap())?;
        // Compile to a unique file and rename, so that parallel runs do not
        // see a partial file.
        let tmp = exe.with_extension(format!("tmp{}", std::process::id()));
        let output = Command::new("dart")
            .arg("compile")
            .arg("exe")
            .arg(&source)
            .arg("-o")
            .arg(&tmp)
            .output()
            .context("running `dart compile exe` for the oracle (is `dart` on PATH?)")?;
        if !output.status.success() {
            bail!(
                "compiling the oracle failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::fs::rename(&tmp, &exe)?;
    }
    Ok(vec![exe.to_string_lossy().into_owned()])
}

/// The output of one program for one batch: path -> line.
struct BatchOutput {
    lines: HashMap<String, String>,
    stderr: String,
    time: Duration,
}

fn run_batch(
    program: &[String],
    mode: &str,
    files: &[String],
    timeout_per_file: u64,
) -> Result<BatchOutput> {
    let start = Instant::now();
    let mut child = Command::new(&program[0])
        .args(&program[1..])
        .arg(mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("starting {}", program.join(" ")))?;
    let mut stdin = child.stdin.take().unwrap();
    let input: String = files.iter().map(|f| format!("{f}\n")).collect();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes());
    });
    let pid = child.id();
    let (tx, rx) = std::sync::mpsc::channel();
    let waiter = std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    let limit = (timeout_per_file > 0)
        .then(|| std::time::Duration::from_secs(60 + files.len() as u64 * timeout_per_file));
    let mut timed_out = false;
    let output = match limit {
        None => rx.recv().expect("batch waiter")?,
        Some(limit) => match rx.recv_timeout(limit) {
            Ok(output) => output?,
            Err(_) => {
                timed_out = true;
                let _ = Command::new("kill").arg("-9").arg(pid.to_string()).status();
                rx.recv().expect("batch waiter")?
            }
        },
    };
    let _ = waiter.join();
    let _ = writer.join();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = HashMap::new();
    for line in stdout.lines() {
        if line.is_empty() {
            continue;
        }
        let path = path_of_line(line);
        if let Some(path) = path {
            lines.insert(path, line.to_string());
        }
    }
    let mut stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if timed_out {
        stderr.push_str(&format!(
            "\ntimeout: `{} {mode}` killed after the time limit of the batch ({} files)\n",
            program.join(" "),
            files.len()
        ));
    }
    Ok(BatchOutput {
        lines,
        stderr,
        time: start.elapsed(),
    })
}

fn short(v: &Value) -> String {
    let s = v.to_string();
    if s.chars().count() > 300 {
        format!("{}...", s.chars().take(300).collect::<String>())
    } else {
        s
    }
}

/// Finds the first difference between [a] (oracle) and [b] (dartr).
fn first_difference(a: &Value, b: &Value, path: &str) -> Option<(String, String, String)> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, va) in x {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                match y.get(k) {
                    None => return Some((p, short(va), "<missing>".into())),
                    Some(vb) => {
                        if let Some(d) = first_difference(va, vb, &p) {
                            return Some(d);
                        }
                    }
                }
            }
            for (k, vb) in y {
                if !x.contains_key(k) {
                    let p = if path.is_empty() {
                        k.clone()
                    } else {
                        format!("{path}.{k}")
                    };
                    return Some((p, "<missing>".into(), short(vb)));
                }
            }
            let ka: Vec<&String> = x.keys().collect();
            let kb: Vec<&String> = y.keys().collect();
            if ka != kb {
                return Some((
                    path.to_string(),
                    format!("keys {ka:?}"),
                    format!("keys {kb:?}"),
                ));
            }
            None
        }
        (Value::Array(x), Value::Array(y)) => {
            for (i, (va, vb)) in x.iter().zip(y.iter()).enumerate() {
                if let Some(d) = first_difference(va, vb, &format!("{path}[{i}]")) {
                    return Some(d);
                }
            }
            if x.len() != y.len() {
                let i = x.len().min(y.len());
                let p = format!("{path}[{i}]");
                let va = x.get(i).map(short).unwrap_or_else(|| "<missing>".into());
                let vb = y.get(i).map(short).unwrap_or_else(|| "<missing>".into());
                return Some((p, va, vb));
            }
            None
        }
        _ => {
            if a != b {
                Some((path.to_string(), short(a), short(b)))
            } else {
                None
            }
        }
    }
}

/// Replaces, in every JSON object of [value] that has `"inf": true`, the
/// value of its `"type"` key by the string `"<inferred>"`. Nested objects
/// (members, parameters) are masked too.
pub fn mask_inferred(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if map.get("inf") == Some(&Value::Bool(true))
                && let Some(ty) = map.get_mut("type")
            {
                *ty = Value::String("<inferred>".to_string());
            }
            for v in map.values_mut() {
                mask_inferred(v);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(mask_inferred),
        _ => {}
    }
}

/// Compares the output lines of one file. When [options] change the lines
/// ([`Options::normalize_line`]), both lines are changed first; the text
/// comparison then sees the re-serialized lines, so escaping differences
/// are not reported.
fn compare(
    file: &str,
    oracle: Option<&String>,
    dartr: Option<&String>,
    options: &Options,
) -> Option<Difference> {
    let normalized;
    let (oracle, dartr) = if options.normalizes() {
        normalized = (
            oracle.map(|l| options.normalize_line(l)),
            dartr.map(|l| options.normalize_line(l)),
        );
        (normalized.0.as_ref(), normalized.1.as_ref())
    } else {
        (oracle, dartr)
    };
    let diff = |json_path: &str, o: String, d: String| {
        Some(Difference {
            file: file.to_string(),
            json_path: json_path.to_string(),
            oracle: o,
            dartr: d,
        })
    };
    match (oracle, dartr) {
        (Some(o), Some(d)) if o == d => None,
        (None, None) => diff("", "<no output>".into(), "<no output>".into()),
        (None, Some(_)) => diff("", "<no output>".into(), "<output>".into()),
        (Some(_), None) => diff("", "<output>".into(), "<no output>".into()),
        (Some(o), Some(d)) => {
            let (vo, vd) = match (parse_deep(o), parse_deep(d)) {
                (Ok(vo), Ok(vd)) => (vo, vd),
                _ => return diff("", "<invalid JSON>".into(), "<invalid JSON>".into()),
            };
            match first_difference(&vo, &vd, "") {
                Some((p, a, b)) => diff(&p, a, b),
                None => {
                    // Same values, different text (escaping or number format).
                    let i = o.bytes().zip(d.bytes()).take_while(|(a, b)| a == b).count();
                    let ctx = |s: &str| {
                        s.get(i.saturating_sub(20)..(i + 40).min(s.len()))
                            .unwrap_or("")
                            .to_string()
                    };
                    diff(&format!("<text, byte {i}>"), ctx(o), ctx(d))
                }
            }
        }
    }
}

fn sanitize(file: &str) -> String {
    file.trim_start_matches('/').replace(['/', '\\'], "__")
}

/// Runs the difftest.
pub fn run(options: &Options) -> Result<Report> {
    let start = Instant::now();
    let files = collect_dart_files(&options.inputs)?;
    let oracle = if options.oracle.is_empty() {
        ensure_oracle()?
    } else {
        options.oracle.clone()
    };
    let dartr = vec![
        options.dartr.to_string_lossy().into_owned(),
        "dump".to_string(),
    ];
    let jobs = options.jobs.max(1);
    let batch_size = if options.batch_size > 0 {
        options.batch_size
    } else {
        files.len().div_ceil(jobs).max(1)
    };
    let batches: Vec<&[String]> = files.chunks(batch_size).collect();

    struct BatchResult {
        index: usize,
        differences: Vec<Difference>,
        identical: usize,
        oracle_time: Duration,
        dartr_time: Duration,
        failures: Vec<(String, Option<String>, Option<String>)>,
        kinds: BTreeMap<String, KindStats>,
        panics: usize,
    }
    let per_kind = options.mode.starts_with("resolved");
    let next = Mutex::new(0usize);
    let results: Mutex<Vec<Result<BatchResult>>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..jobs.min(batches.len().max(1)) {
            scope.spawn(|| {
                loop {
                    let index = {
                        let mut n = next.lock().unwrap();
                        let i = *n;
                        *n += 1;
                        i
                    };
                    if index >= batches.len() {
                        break;
                    }
                    let batch = batches[index];
                    let result = (|| -> Result<BatchResult> {
                        let (o, d) = std::thread::scope(|s| {
                            let o = s.spawn(|| {
                                run_batch(&oracle, &options.mode, batch, options.timeout_per_file)
                            });
                            let d = s.spawn(|| {
                                run_batch(&dartr, &options.mode, batch, options.timeout_per_file)
                            });
                            (o.join().unwrap(), d.join().unwrap())
                        });
                        let (o, d) = (o?, d?);
                        let mut differences = Vec::new();
                        let mut failures = Vec::new();
                        let mut identical = 0;
                        let mut kinds = BTreeMap::new();
                        let mut panics = 0;
                        for file in batch {
                            let (lo, ld) = (o.lines.get(file), d.lines.get(file));
                            if per_kind && let Some(lo) = lo {
                                let parse = |l: &String| {
                                    parse_deep(l).ok().map(|mut v| {
                                        options.normalize_value(&mut v);
                                        v
                                    })
                                };
                                let vd = ld.and_then(parse);
                                if let Some(vd) = &vd {
                                    panics += count_panics(vd);
                                }
                                if let Some(vo) = parse(lo) {
                                    count_kinds(&vo, vd.as_ref(), &mut kinds);
                                }
                            }
                            match compare(file, lo, ld, options) {
                                None => identical += 1,
                                Some(mut diff) => {
                                    if lo.is_none() && !o.stderr.is_empty() {
                                        diff.oracle = format!(
                                            "{} (stderr: {})",
                                            diff.oracle,
                                            tail(&o.stderr)
                                        );
                                    }
                                    if ld.is_none() && !d.stderr.is_empty() {
                                        diff.dartr =
                                            format!("{} (stderr: {})", diff.dartr, tail(&d.stderr));
                                    }
                                    differences.push(diff);
                                    failures.push((file.clone(), lo.cloned(), ld.cloned()));
                                }
                            }
                        }
                        Ok(BatchResult {
                            index,
                            differences,
                            identical,
                            oracle_time: o.time,
                            dartr_time: d.time,
                            failures,
                            kinds,
                            panics,
                        })
                    })();
                    results.lock().unwrap().push(result);
                }
            });
        }
    });

    let mut results: Vec<BatchResult> = results
        .into_inner()
        .unwrap()
        .into_iter()
        .collect::<Result<_>>()?;
    results.sort_by_key(|r| r.index);
    let mut report = Report {
        mode: options.mode.clone(),
        files: files.len(),
        ..Default::default()
    };
    if let Some(dir) = &options.write_failures {
        std::fs::create_dir_all(dir)?;
    }
    for r in results {
        report.identical += r.identical;
        report.oracle_time += r.oracle_time;
        report.dartr_time += r.dartr_time;
        report.differences.extend(r.differences);
        report.panics += r.panics;
        for (kind, k) in &r.kinds {
            report.kinds.entry(kind.clone()).or_default().add(k);
        }
        if let Some(dir) = &options.write_failures {
            for (file, o, d) in r.failures {
                let name = sanitize(&file);
                let pretty = |line: Option<String>| match line {
                    None => "<no output>\n".to_string(),
                    Some(l) => serde_json::from_str::<Value>(&l)
                        .and_then(|v| serde_json::to_string_pretty(&v))
                        .map(|s| s + "\n")
                        .unwrap_or(l),
                };
                std::fs::write(dir.join(format!("{name}.oracle.json")), pretty(o))?;
                std::fs::write(dir.join(format!("{name}.dartr.json")), pretty(d))?;
            }
        }
    }
    if let Some(dir) = &options.write_failures {
        std::fs::write(
            dir.join("summary.txt"),
            report.summary() + &report.kind_report() + &report.difference_report(usize::MAX),
        )?;
    }
    report.elapsed = start.elapsed();
    Ok(report)
}

fn tail(s: &str) -> String {
    let lines: Vec<&str> = s.lines().collect();
    lines[lines.len().saturating_sub(3)..].join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diff(o: &str, d: &str) -> Option<Difference> {
        compare(
            "f.dart",
            Some(&o.to_string()),
            Some(&d.to_string()),
            &Options::default(),
        )
    }

    fn masked_diff(o: &str, d: &str) -> Option<Difference> {
        let options = Options {
            mask_inferred: true,
            ..Default::default()
        };
        compare(
            "f.dart",
            Some(&o.to_string()),
            Some(&d.to_string()),
            &options,
        )
    }

    #[test]
    fn mask_inferred_hides_inferred_types_only() {
        // Only an inferred type differs: equal with the mask, different
        // without it.
        let o = r#"{"path":"f","elements":[{"k":"topVar","n":"v","type":"int","inf":true}]}"#;
        let d = r#"{"path":"f","elements":[{"k":"topVar","n":"v","type":"dynamic","inf":true}]}"#;
        assert!(masked_diff(o, d).is_none());
        let unmasked = diff(o, d).unwrap();
        assert_eq!(unmasked.json_path, "elements[0].type");

        // A declared type (`"inf": false`) is still compared, and the JSON
        // path of the difference is reported.
        let o = r#"{"path":"f","elements":[{"k":"topVar","n":"v","type":"int","inf":false}]}"#;
        let d = r#"{"path":"f","elements":[{"k":"topVar","n":"v","type":"num","inf":false}]}"#;
        let m = masked_diff(o, d).unwrap();
        assert_eq!(
            (m.json_path.as_str(), m.oracle.as_str(), m.dartr.as_str()),
            ("elements[0].type", "\"int\"", "\"num\"")
        );

        // A different `inf` value is a difference, not masked away.
        let d = r#"{"path":"f","elements":[{"k":"topVar","n":"v","type":"num","inf":true}]}"#;
        let m = masked_diff(o, d).unwrap();
        assert_eq!(m.json_path, "elements[0].type");
        assert_eq!(m.dartr, "\"<inferred>\"");
    }

    #[test]
    fn mask_inferred_masks_nested_objects() {
        // A method with an inferred parameter type inside a class: both the
        // method type and the parameter type are masked; the declared
        // parameter type next to it is still compared.
        let line = |method: &str, p0: &str, p1: &str| {
            format!(
                r#"{{"path":"f","elements":[{{"k":"class","n":"A","members":[{{"k":"method","n":"m","type":"{method}","inf":true,"params":[{{"n":"x","type":"{p0}","inf":true}},{{"n":"y","type":"{p1}","inf":false}}]}}]}}]}}"#
            )
        };
        let o = line("int Function(int, String)", "int", "String");
        let d = line("dynamic Function(dynamic, String)", "dynamic", "String");
        assert!(masked_diff(&o, &d).is_none());

        let d = line("dynamic Function(dynamic, Object)", "dynamic", "Object");
        let m = masked_diff(&o, &d).unwrap();
        assert_eq!(m.json_path, "elements[0].members[0].params[1].type");

        let mut v: Value = serde_json::from_str(&o).unwrap();
        mask_inferred(&mut v);
        assert_eq!(
            v.to_string(),
            line("<inferred>", "<inferred>", "String"),
            "key order is kept"
        );
    }

    #[test]
    fn dart_uris_pass_through_collect() {
        let files =
            collect_dart_files(&[PathBuf::from("dart:core"), PathBuf::from("dart:_internal")])
                .unwrap();
        assert_eq!(files, ["dart:_internal", "dart:core"]);
    }

    #[test]
    fn mask_keeps_key_order_differences() {
        let d = masked_diff(
            r#"{"k":"field","type":"int","inf":true}"#,
            r#"{"type":"int","k":"field","inf":true}"#,
        )
        .unwrap();
        assert_eq!(d.json_path, "");
        assert!(d.oracle.starts_with("keys"), "{}", d.oracle);
    }

    #[test]
    fn reports_the_json_path_of_the_first_difference() {
        let d = diff(
            r#"{"path":"f","tokens":[{"k":"A","o":0},{"k":"B","o":1}],"diagnostics":[]}"#,
            r#"{"path":"f","tokens":[{"k":"A","o":0},{"k":"B","o":2}],"diagnostics":[1]}"#,
        )
        .unwrap();
        assert_eq!(
            (d.json_path.as_str(), d.oracle.as_str(), d.dartr.as_str()),
            ("tokens[1].o", "1", "2")
        );

        let d = diff(r#"{"t":[1,2,3]}"#, r#"{"t":[1,2]}"#).unwrap();
        assert_eq!(
            (d.json_path.as_str(), d.oracle.as_str(), d.dartr.as_str()),
            ("t[2]", "3", "<missing>")
        );

        let d = diff(r#"{"t":{"syn":true}}"#, r#"{"t":{}}"#).unwrap();
        assert_eq!(
            (d.json_path.as_str(), d.dartr.as_str()),
            ("t.syn", "<missing>")
        );
    }

    #[test]
    fn key_order_and_escaping_are_differences() {
        let d = diff(r#"{"a":1,"b":2}"#, r#"{"b":2,"a":1}"#).unwrap();
        assert_eq!(d.json_path, "");
        assert!(d.oracle.contains("\"a\", \"b\""), "{}", d.oracle);

        // Same value, different text.
        let d = diff(r#"{"x":"a/b"}"#, r#"{"x":"a\/b"}"#).unwrap();
        assert!(d.json_path.starts_with("<text"), "{}", d.json_path);

        assert!(diff(r#"{"a":1}"#, r#"{"a":1}"#).is_none());
    }

    #[test]
    fn kinds_and_no_diagnostics_filter_the_lists() {
        let options = Options {
            kinds: vec!["IntegerLiteral".to_string()],
            no_diagnostics: true,
            ..Default::default()
        };
        let o = r#"{"path":"f","units":[{"path":"f","diagnostics":[{"code":"x"}],"types":[{"o":0,"k":"IntegerLiteral","type":"int"},{"o":2,"k":"SimpleIdentifier","type":"int"}]}]}"#;
        let d = r#"{"path":"f","units":[{"path":"f","diagnostics":[],"types":[{"o":0,"k":"IntegerLiteral","type":"int"},{"o":2,"k":"SimpleIdentifier","type":"dynamic"}]}]}"#;
        assert!(compare("f", Some(&o.to_string()), Some(&d.to_string()), &options).is_none());
        assert_eq!(
            options.normalize_line(o),
            r#"{"path":"f","units":[{"path":"f","types":[{"o":0,"k":"IntegerLiteral","type":"int"}]}]}"#
        );
        // Without the filter, the identifier type is the first difference.
        let options = Options {
            no_diagnostics: true,
            ..Default::default()
        };
        let diff = compare("f", Some(&o.to_string()), Some(&d.to_string()), &options).unwrap();
        assert_eq!(diff.json_path, "units[0].types[1].type");
    }

    #[test]
    fn count_kinds_matches_entries_per_unit() {
        let o: Value = serde_json::from_str(
            r#"{"path":"a","units":[
                {"path":"a","nodes":[{"o":0,"k":"SimpleIdentifier","el":"x"},{"o":4,"k":"SimpleIdentifier","el":"y"},{"o":8,"k":"NamedType","el":"T"}]},
                {"path":"b","nodes":[{"o":0,"k":"SimpleIdentifier","el":"z"}]}]}"#,
        )
        .unwrap();
        // Unit b is missing (for example a panic), y has another element.
        let d: Value = serde_json::from_str(
            r#"{"path":"a","units":[{"path":"a","nodes":[{"o":0,"k":"SimpleIdentifier","el":"x"},{"o":4,"k":"SimpleIdentifier","el":null},{"o":8,"k":"NamedType","el":"T"}]}]}"#,
        )
        .unwrap();
        let mut stats = BTreeMap::new();
        count_kinds(&o, Some(&d), &mut stats);
        assert_eq!(
            stats["SimpleIdentifier"],
            KindStats {
                oracle: 3,
                matched: 1,
                dartr: 2
            }
        );
        assert_eq!(
            stats["NamedType"],
            KindStats {
                oracle: 1,
                matched: 1,
                dartr: 1
            }
        );
        // No dartr line: nothing is matched.
        let mut stats = BTreeMap::new();
        count_kinds(&o, None, &mut stats);
        assert_eq!(stats["SimpleIdentifier"].matched, 0);
        assert_eq!(stats["SimpleIdentifier"].oracle, 3);
    }
}
