// Dart source: pkg/analysis_server/lib/src/plugin/plugin_watcher.dart
// (PluginWatcher.addedDriver, _addPlugins)
// Dart source: pkg/analysis_server/lib/src/plugin/plugin_manager.dart
// (pluginStateFolder, _computeFiles, _runPubCommand)
// Dart source: pkg/analysis_server/lib/src/plugin/plugin_isolate.dart
// (PluginSession.start, PluginIsolate._updatePluginRoots, reportException)
// Dart source: pkg/analysis_server/lib/src/plugin/notification_manager.dart
// (handlePluginNotification, recordAnalysisErrors, sendPluginError)

//! Analyzer plugins (the new `plugins:` section of `analysis_options.yaml`)
//! for `dartr analyze`.
//!
//! dartr does what the analysis server does for each context root whose
//! root options file configures plugins: it writes the synthetic plugin
//! package ([`generator`]) into a state folder, runs `dart pub upgrade`
//! there, and starts the plugin isolate. A plugin runs in a Dart isolate
//! and talks JSON maps over a `SendPort`, so a small Dart program
//! (`bridge/plugin_bridge.dart`, run with the `dart` of the SDK) spawns
//! the isolate and relays the messages as JSON lines over stdio. The
//! plugin reads and analyzes the files by itself (its own analyzer); dartr
//! sends only the roots, waits until the plugin is idle and collects the
//! `analysis.errors` notifications. The plugin applies its ignore comments
//! (`// ignore: plugin/code`) and its severities (`diagnostics:`) itself;
//! the server only merges the errors (design `docs/research/analyzer-plugins.md`).
//!
//! Not supported yet: legacy plugins (`analyzer: plugins:`), the language
//! server (overlays, fixes, assists).

pub mod generator;

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

use dartr_project::AnalysisContextCollection;
use dartr_project::analysis_options::PluginsOptions;
use serde_json::{Value, json};

/// The source of the bridge program.
const BRIDGE_SOURCE: &str = include_str!("../bridge/plugin_bridge.dart");

/// The plugins of one context root (Dart `addedDriver` with
/// `driver.pluginsOptions`).
#[derive(Clone, Debug)]
pub struct PluginJob {
    /// The root folder of the context.
    pub context_root: String,
    pub excluded: Vec<String>,
    pub options_file: Option<String>,
    /// The SDK path sent in `plugin.versionCheck`.
    pub sdk_path: String,
    pub plugins: PluginsOptions,
}

/// The plugin jobs of [collection]: one per context whose root options file
/// has plugin configurations (Dart `pluginsOptions` reads only the root
/// options file of a context).
pub fn plugin_jobs(collection: &AnalysisContextCollection) -> Vec<PluginJob> {
    let mut jobs = Vec::new();
    for context in &collection.contexts {
        let Some(options_file) = &context.root.options_file else {
            continue;
        };
        let options = collection.options_for(context, options_file);
        if options.plugins.configurations.is_empty() {
            continue;
        }
        // Dart `PluginWatcher._getSdkPath` (the folder above the `lib`
        // folder of `dart:core`). The plugin builds a folder SDK from it
        // (it reads `<sdk>/version`), so this is the folder SDK, also for a
        // Flutter context whose `dart:core` comes from the `sky_engine`
        // embedder.
        let sdk_path = collection
            .sdk
            .as_ref()
            .or(context.sdk.as_ref())
            .map(|s| s.path().to_string())
            .unwrap_or_default();
        jobs.push(PluginJob {
            context_root: context.root.root.clone(),
            excluded: context.root.excluded.clone(),
            options_file: Some(options_file.clone()),
            sdk_path,
            plugins: options.plugins.clone(),
        });
    }
    jobs
}

/// Protocol `Location`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Location {
    pub file: String,
    pub offset: usize,
    pub length: usize,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// Protocol `DiagnosticMessage`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContextMessage {
    pub message: String,
    pub location: Location,
}

/// Protocol `AnalysisError` from a plugin.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PluginError {
    /// `ERROR`, `WARNING` or `INFO`.
    pub severity: String,
    /// `AnalysisErrorType` name, for example `STATIC_WARNING`.
    pub type_: String,
    pub location: Location,
    pub message: String,
    pub correction: Option<String>,
    pub code: String,
    pub url: Option<String>,
    pub context_messages: Vec<ContextMessage>,
}

/// The result of the plugins.
#[derive(Debug, Default)]
pub struct PluginOutput {
    /// The plugin errors by file (the last `analysis.errors` notification of
    /// each file, merged over the plugin isolates without duplicates).
    pub errors: Vec<PluginError>,
    /// `server.pluginError` messages (a plugin could not be set up or
    /// started, or crashed).
    pub plugin_errors: Vec<String>,
    /// `server.error` notifications made from `plugin.error` (message and
    /// stack trace).
    pub server_errors: Vec<(String, String)>,
}

/// The state folder of dartr (`DARTR_STATE_DIR`, or `~/.dartr`), like the
/// `.dartServer` folder of the analysis server.
fn state_location() -> PathBuf {
    if let Some(dir) = std::env::var_os("DARTR_STATE_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME").unwrap_or_else(|| ".".into());
    PathBuf::from(home).join(".dartr")
}

/// Dart `PluginManager.pluginStateFolder(path)`:
/// `<state>/.plugin_manager/<md5 of the path>`.
fn plugin_state_folder(path: &str) -> PathBuf {
    use md5::{Digest, Md5};
    // Dart `md5.convert(path.codeUnits)`: the UTF-16 code units as bytes.
    let units: Vec<u8> = path.encode_utf16().map(|u| u as u8).collect();
    let digest = Md5::digest(&units);
    let name: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    state_location().join(".plugin_manager").join(name)
}

/// Writes [content] to [path] when it differs (Dart keeps the modification
/// time of unchanged files).
fn write_if_changed(path: &Path, content: &str) -> std::io::Result<()> {
    if std::fs::read_to_string(path).ok().as_deref() == Some(content) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)
}

/// Dart `PluginIsolate.reportException` message for an exception without
/// a Dart stack trace.
fn plugin_error_message(message: &str) -> String {
    format!("An error occurred while executing an analyzer plugin: {message}\n")
}

/// Runs the plugins of [jobs] for the analysis roots [roots] with the `dart`
/// executable [dart] and returns their errors.
pub fn run_plugins(jobs: &[PluginJob], roots: &[String], dart: &str) -> PluginOutput {
    let mut output = PluginOutput::default();
    if jobs.is_empty() {
        return output;
    }
    let bridge = state_location()
        .join(".plugin_manager")
        .join("plugin_bridge.dart");
    if let Err(e) = write_if_changed(&bridge, BRIDGE_SOURCE) {
        output.plugin_errors.push(plugin_error_message(&format!(
            "cannot write {}: {e}",
            bridge.display()
        )));
        return output;
    }
    // The isolates run in parallel, like the server's.
    let results: Vec<Result<PluginSessionOutput, String>> = std::thread::scope(|scope| {
        let handles: Vec<_> = jobs
            .iter()
            .map(|job| scope.spawn(|| run_job(job, roots, dart, &bridge)))
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err("plugin thread panicked".into()))
            })
            .collect()
    });
    // Dart `ResultMerger.mergeAnalysisErrors`: all errors of all plugins,
    // one copy of an error that several plugins report.
    let mut by_file: indexmap_like::OrderedMap = indexmap_like::OrderedMap::default();
    for result in results {
        match result {
            Ok(session) => {
                for (file, errors) in session.errors {
                    by_file.extend(file, errors);
                }
                output.server_errors.extend(session.server_errors);
                output.plugin_errors.extend(session.plugin_errors);
            }
            Err(message) => output.plugin_errors.push(plugin_error_message(&message)),
        }
    }
    output.errors = by_file.into_errors();
    output
}

/// A tiny insertion-ordered map of file errors (no extra dependency).
mod indexmap_like {
    use super::PluginError;

    #[derive(Default)]
    pub struct OrderedMap {
        files: Vec<(String, Vec<PluginError>)>,
    }

    impl OrderedMap {
        pub fn extend(&mut self, file: String, errors: Vec<PluginError>) {
            let index = match self.files.iter().position(|(f, _)| *f == file) {
                Some(i) => i,
                None => {
                    self.files.push((file, Vec::new()));
                    self.files.len() - 1
                }
            };
            let list = &mut self.files[index].1;
            for error in errors {
                if !list.contains(&error) {
                    list.push(error);
                }
            }
        }

        pub fn into_errors(self) -> Vec<PluginError> {
            self.files.into_iter().flat_map(|(_, e)| e).collect()
        }
    }
}

struct PluginSessionOutput {
    errors: Vec<(String, Vec<PluginError>)>,
    plugin_errors: Vec<String>,
    server_errors: Vec<(String, String)>,
}

/// Dart `_addPlugins` + `PluginManager.addPluginToContextRoot` + a plugin
/// session for one context root.
fn run_job(
    job: &PluginJob,
    roots: &[String],
    dart: &str,
    bridge: &Path,
) -> Result<PluginSessionOutput, String> {
    let folder = plugin_state_folder(&job.context_root);
    let configurations = &job.plugins.configurations;
    let pubspec =
        generator::generate_pubspec(configurations, job.plugins.dependency_overrides.as_deref());
    let entrypoint = generator::generate_entrypoint(configurations);
    let entrypoint_path = folder.join("bin").join("plugin.dart");
    write_if_changed(&folder.join("pubspec.yaml"), &pubspec)
        .and_then(|_| write_if_changed(&entrypoint_path, &entrypoint))
        .map_err(|e| {
            format!(
                "cannot write the plugin package at {}: {e}",
                folder.display()
            )
        })?;

    // Dart `_computeFiles(pubCommand: 'upgrade')`.
    let pub_result = Command::new(dart)
        .args(["pub", "upgrade"])
        .current_dir(&folder)
        .env("PUB_ENVIRONMENT", "analysis_server.plugin_manager")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run {dart} pub upgrade: {e}"))?;
    let package_config = folder.join(".dart_tool").join("package_config.json");
    if !pub_result.status.success() {
        // Without network, an earlier resolution can still run.
        if !package_config.is_file() {
            return Err(format!(
                "An error occurred while setting up the analyzer plugin package at '{}'. \
                 The `dart pub upgrade` command failed:\n  exitCode = {}\n  stdout = {}\n  stderr = {}\n",
                folder.display(),
                pub_result.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&pub_result.stdout),
                String::from_utf8_lossy(&pub_result.stderr),
            ));
        }
    }
    if !package_config.is_file() {
        return Err(format!(
            "File \"{}\" does not exist.",
            package_config.display()
        ));
    }

    let mut session = Session::start(dart, bridge, &entrypoint_path, &package_config)?;
    let result = session.analyze(job, roots);
    session.shutdown();
    result
}

/// One message from the bridge.
enum Incoming {
    Message(Value),
    /// stdout closed.
    Closed,
}

/// A running plugin isolate behind the bridge process.
struct Session {
    child: Child,
    stdin: Option<ChildStdin>,
    incoming: Receiver<Incoming>,
    next_id: u64,
    errors: HashMap<String, Vec<PluginError>>,
    file_order: Vec<String>,
    analyzing: Option<bool>,
    plugin_errors: Vec<String>,
    server_errors: Vec<(String, String)>,
    exited: bool,
}

/// Appends [line] to the file `$DARTR_PLUGIN_LOG` (the plugin messages,
/// for debugging; stderr may be locked by the caller).
fn debug_log(line: &str) {
    if let Some(path) = std::env::var_os("DARTR_PLUGIN_LOG")
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
    {
        let elapsed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let text: String = line.chars().take(400).collect();
        let _ = writeln!(file, "{elapsed} {text}");
    }
}

/// The longest time to wait for the plugin to start analyzing after
/// `analysis.setAnalysisRoots` (a plugin without files to analyze never
/// reports `isAnalyzing: true`). The first run of a JIT plugin compiles the
/// analyzer, which takes seconds.
fn idle_start_timeout() -> Duration {
    let seconds = std::env::var("DARTR_PLUGIN_START_TIMEOUT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    Duration::from_secs(seconds)
}

/// The longest time to wait for one plugin answer or status change.
fn timeout() -> Duration {
    let seconds = std::env::var("DARTR_PLUGIN_TIMEOUT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(600);
    Duration::from_secs(seconds)
}

impl Session {
    fn start(
        dart: &str,
        bridge: &Path,
        entrypoint: &Path,
        package_config: &Path,
    ) -> Result<Session, String> {
        let mut child = Command::new(dart)
            .arg(bridge)
            .arg(entrypoint)
            .arg(package_config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("cannot start the plugin bridge with {dart}: {e}"))?;
        let stdout = child.stdout.take().expect("piped stdout");
        let stdin = child.stdin.take();
        let (sender, incoming) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if let Ok(value) = serde_json::from_str::<Value>(&line)
                    && sender.send(Incoming::Message(value)).is_err()
                {
                    return;
                }
            }
            let _ = sender.send(Incoming::Closed);
        });
        let mut session = Session {
            child,
            stdin,
            incoming,
            next_id: 0,
            errors: HashMap::new(),
            file_order: Vec::new(),
            analyzing: None,
            plugin_errors: Vec::new(),
            server_errors: Vec::new(),
            exited: false,
        };
        // Wait for the isolate (`{"bridge": "ready"}`).
        loop {
            match session.next_message(timeout())? {
                Some(v) if v.get("bridge").and_then(Value::as_str) == Some("ready") => break,
                Some(v) => session.handle(v),
                None => {
                    return Err(session
                        .plugin_errors
                        .pop()
                        .unwrap_or_else(|| "Unrecorded error while starting the plugin.".into()));
                }
            }
        }
        Ok(session)
    }

    /// The next message, `None` when the bridge exited.
    fn next_message(&mut self, wait: Duration) -> Result<Option<Value>, String> {
        if self.exited {
            return Ok(None);
        }
        match self.incoming.recv_timeout(wait) {
            Ok(Incoming::Message(v)) => {
                debug_log(&format!("<== {v}"));
                if v.get("bridge").and_then(Value::as_str) == Some("exit") {
                    self.exited = true;
                    return Ok(None);
                }
                Ok(Some(v))
            }
            Ok(Incoming::Closed) | Err(RecvTimeoutError::Disconnected) => {
                self.exited = true;
                Ok(None)
            }
            Err(RecvTimeoutError::Timeout) => Err("the analyzer plugin did not answer".into()),
        }
    }

    fn send(&mut self, method: &str, params: Value) -> String {
        let id = self.next_id.to_string();
        self.next_id += 1;
        let request = json!({"id": id, "method": method, "params": params});
        debug_log(&format!("==> {request}"));
        if let Some(stdin) = &mut self.stdin {
            let _ = writeln!(stdin, "{request}");
            let _ = stdin.flush();
        }
        id
    }

    /// Sends a request and waits for its response (handling notifications
    /// meanwhile). Returns the `result`.
    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.send(method, params);
        loop {
            let Some(message) = self.next_message(timeout())? else {
                return Err(self
                    .plugin_errors
                    .last()
                    .cloned()
                    .unwrap_or_else(|| format!("the analyzer plugin exited during '{method}'")));
            };
            if message.get("id").and_then(Value::as_str) == Some(&id) {
                if let Some(error) = message.get("error").filter(|e| !e.is_null())
                    // Dart: a plugin with an older `analysis_server_plugin`
                    // may not support the request; only logged.
                    && error.get("code").and_then(Value::as_str) != Some("UNKNOWN_REQUEST")
                {
                    let text = error.get("message").and_then(Value::as_str).unwrap_or("");
                    let stack = error
                        .get("stackTrace")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    // Dart `PluginSession.sendRequest`: `reportException`.
                    self.plugin_errors
                        .push(plugin_error_message(&format!("{text}\n{stack}")));
                }
                return Ok(message.get("result").cloned().unwrap_or(Value::Null));
            }
            self.handle(message);
        }
    }

    /// Dart `NotificationManager.handlePluginNotification` (and the bridge
    /// error lines).
    fn handle(&mut self, message: Value) {
        if message.get("bridge").and_then(Value::as_str) == Some("error") {
            // An uncaught error of the isolate (`onError`).
            let text = message.get("message").and_then(Value::as_str).unwrap_or("");
            let stack = message
                .get("stackTrace")
                .and_then(Value::as_str)
                .unwrap_or("");
            self.plugin_errors
                .push(plugin_error_message(&format!("{text}\n{stack}")));
            return;
        }
        let Some(event) = message.get("event").and_then(Value::as_str) else {
            return;
        };
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        match event {
            "analysis.errors" => {
                let Some(file) = params.get("file").and_then(Value::as_str) else {
                    return;
                };
                let errors = params
                    .get("errors")
                    .and_then(Value::as_array)
                    .map(|list| list.iter().filter_map(parse_error).collect())
                    .unwrap_or_default();
                if !self.errors.contains_key(file) {
                    self.file_order.push(file.to_string());
                }
                self.errors.insert(file.to_string(), errors);
            }
            "plugin.status" => {
                if let Some(analyzing) = params
                    .get("analysis")
                    .and_then(|a| a.get("isAnalyzing"))
                    .and_then(Value::as_bool)
                {
                    self.analyzing = Some(analyzing);
                }
            }
            "plugin.error" => {
                // Dart `sendPluginErrorNotification`: a `server.error`.
                let text = params.get("message").and_then(Value::as_str).unwrap_or("");
                let stack = params
                    .get("stackTrace")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                self.server_errors
                    .push((text.to_string(), stack.to_string()));
            }
            _ => {}
        }
    }

    /// `plugin.versionCheck`, the roots, then wait until the plugin is idle.
    fn analyze(
        &mut self,
        job: &PluginJob,
        roots: &[String],
    ) -> Result<PluginSessionOutput, String> {
        let byte_store = state_location().join(".plugin_manager").join("byte_store");
        let result = self.request(
            "plugin.versionCheck",
            json!({
                "byteStorePath": byte_store.to_string_lossy(),
                "sdkPath": job.sdk_path,
                "version": "1.0.0-alpha.0",
            }),
        )?;
        if result.get("isCompatible").and_then(Value::as_bool) == Some(false) {
            return Err("Plugin is not compatible.".into());
        }
        // Dart `PluginIsolate._updatePluginRoots`: the context roots, then
        // the analysis roots of the client (new plugins use only these).
        let mut context_root = json!({
            "root": job.context_root,
            "exclude": job.excluded,
        });
        if let Some(options) = &job.options_file {
            context_root["optionsFile"] = json!(options);
        }
        self.send("analysis.setContextRoots", json!({"roots": [context_root]}));
        self.request(
            "analysis.setAnalysisRoots",
            json!({"included": roots, "excluded": []}),
        )?;
        // Wait for the idle status after the analysis started. The plugin
        // reports `isAnalyzing: true` when its driver starts working.
        let started = Instant::now();
        let mut seen_busy = self.analyzing == Some(true);
        loop {
            if seen_busy && self.analyzing == Some(false) {
                break;
            }
            // No status at all for a while: nothing to analyze.
            let wait = if seen_busy {
                timeout()
            } else {
                idle_start_timeout().saturating_sub(started.elapsed())
            };
            if !seen_busy && wait.is_zero() {
                break;
            }
            match self.next_message(wait) {
                Ok(Some(message)) => {
                    self.handle(message);
                    if self.analyzing == Some(true) {
                        seen_busy = true;
                    }
                }
                Ok(None) => break,
                Err(e) if seen_busy => return Err(e),
                Err(_) => break,
            }
        }
        let mut errors = Vec::new();
        for file in std::mem::take(&mut self.file_order) {
            let list = self.errors.remove(&file).unwrap_or_default();
            errors.push((file, list));
        }
        Ok(PluginSessionOutput {
            errors,
            plugin_errors: std::mem::take(&mut self.plugin_errors),
            server_errors: std::mem::take(&mut self.server_errors),
        })
    }

    /// Dart `PluginSession.stop`: `plugin.shutdown`, then kill after a
    /// while.
    fn shutdown(&mut self) {
        if !self.exited {
            self.send("plugin.shutdown", json!({}));
            let deadline = Instant::now() + Duration::from_secs(5);
            while !self.exited && Instant::now() < deadline {
                let left = deadline.saturating_duration_since(Instant::now());
                match self.next_message(left) {
                    Ok(Some(_)) => {}
                    _ => break,
                }
            }
        }
        self.stdin = None;
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20))
                }
                _ => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    return;
                }
            }
        }
    }
}

fn parse_location(value: &Value) -> Option<Location> {
    let int = |key: &str| value.get(key).and_then(Value::as_u64);
    Some(Location {
        file: value.get("file")?.as_str()?.to_string(),
        offset: int("offset")? as usize,
        length: int("length")? as usize,
        start_line: int("startLine").unwrap_or(0) as u32,
        start_column: int("startColumn").unwrap_or(0) as u32,
        end_line: int("endLine").unwrap_or(0) as u32,
        end_column: int("endColumn").unwrap_or(0) as u32,
    })
}

/// Protocol `AnalysisError.fromJson`.
fn parse_error(value: &Value) -> Option<PluginError> {
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_string);
    Some(PluginError {
        severity: text("severity")?,
        type_: text("type")?,
        location: parse_location(value.get("location")?)?,
        message: text("message")?,
        correction: text("correction"),
        code: text("code")?,
        url: text("url"),
        context_messages: value
            .get("contextMessages")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|m| {
                        Some(ContextMessage {
                            message: m.get("message")?.as_str()?.to_string(),
                            location: parse_location(m.get("location")?)?,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// The `dart` executable of [sdk_path] (`<sdk>/bin/dart`).
pub fn dart_executable(sdk_path: &str) -> String {
    Path::new(sdk_path)
        .join("bin")
        .join("dart")
        .to_string_lossy()
        .into_owned()
}
