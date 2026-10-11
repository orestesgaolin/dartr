// Dart source: pkg/analysis_server/lib/src/plugin/plugin_watcher.dart
// (PluginWatcher.addedDriver, _addPlugins)
// Dart source: pkg/analysis_server/lib/src/plugin/plugin_manager.dart
// (pluginStateFolder, _computeFiles, _runPubCommand)
// Dart source: pkg/analysis_server/lib/src/plugin/plugin_isolate.dart
// (PluginSession.start, PluginIsolate._updatePluginRoots, reportException)
// Dart source: pkg/analysis_server/lib/src/plugin/notification_manager.dart
// (handlePluginNotification, recordAnalysisErrors, sendPluginError)

//! Analyzer plugins (both the new `plugins:` section and legacy
//! `analyzer: plugins:` in `analysis_options.yaml`) for `dartr analyze`
//! and `dartr language-server`.
//!
//! dartr does what the analysis server does for each context root whose
//! root options file configures plugins: for new plugins it writes the
//! synthetic plugin package ([`generator`]) into a state folder, runs
//! `dart pub upgrade` there, and starts the plugin isolate; for legacy
//! plugins it resolves `package:<name>/<name>.dart` to
//! `<packageRoot>/tools/analyzer_plugin`, copies it into `.plugin_manager`
//! (when `pubspec.yaml` exists) and runs `dart pub get` or `upgrade`.
//! A plugin runs in a Dart isolate and talks JSON maps over a `SendPort`,
//! so a small Dart program (`bridge/plugin_bridge.dart`, run with the
//! `dart` of the SDK) spawns the isolate and relays the messages as JSON
//! lines over stdio.

pub mod generator;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError, channel};
use std::time::{Duration, Instant};

use dartr_project::AnalysisContextCollection;
use dartr_project::analysis_options::PluginsOptions;
use dartr_project::glob::Glob;
use dartr_project::paths;
use serde_json::{Value, json};

/// The source of the bridge program.
const BRIDGE_SOURCE: &str = include_str!("../bridge/plugin_bridge.dart");

/// The plugins of one context root (Dart `addedDriver` with
/// `driver.enabledLegacyPluginNames` and `driver.pluginsOptions`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginJob {
    /// The root folder of the context.
    pub context_root: String,
    pub excluded: Vec<String>,
    pub options_file: Option<String>,
    /// The SDK path sent in `plugin.versionCheck`.
    pub sdk_path: String,
    pub plugins: PluginsOptions,
    /// For a legacy plugin (`analyzer: plugins:`), `<packageRoot>/tools/analyzer_plugin`.
    pub legacy_plugin_path: Option<String>,
}

impl PluginJob {
    /// The plugin ID used as key in `PluginManager._pluginMap` and
    /// `ResultCollector`: for a legacy plugin, its `<packageRoot>/tools/analyzer_plugin`
    /// path; for a new plugin, its `.plugin_manager/<md5(context_root)>` folder.
    pub fn plugin_id(&self) -> String {
        match &self.legacy_plugin_path {
            Some(path) => path.clone(),
            None => plugin_state_folder(&self.context_root)
                .to_string_lossy()
                .into_owned(),
        }
    }

    pub fn is_legacy(&self) -> bool {
        self.legacy_plugin_path.is_some()
    }
}

/// The plugin jobs of [collection]: legacy plugins (`analyzer: plugins:`)
/// and new plugins (`plugins:`) from each context's root options file
/// (Dart `PluginWatcher.addedDriver`).
pub fn plugin_jobs(collection: &AnalysisContextCollection) -> Vec<PluginJob> {
    let mut jobs = Vec::new();
    for context in &collection.contexts {
        let Some(options_file) = &context.root.options_file else {
            continue;
        };
        let options = collection.options_for(context, options_file);
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

        // Dart `PluginWatcher._addLegacyPlugins`.
        let mut seen_legacy = HashSet::new();
        for host_package_name in &options.enabled_legacy_plugin_names {
            if !seen_legacy.insert(host_package_name.as_str()) {
                continue;
            }
            let uri = format!("package:{host_package_name}/{host_package_name}.dart");
            let Some(full_path) = context.packages.resolve_package_uri(&uri) else {
                continue;
            };
            let lib_dir = paths::dirname(&full_path);
            if paths::basename(lib_dir) != "lib" {
                continue;
            }
            let package_root = paths::dirname(lib_dir);
            let plugin_dir = paths::join(package_root, "tools/analyzer_plugin");
            if !Path::new(&plugin_dir).is_dir() {
                continue;
            }
            jobs.push(PluginJob {
                context_root: context.root.root.clone(),
                excluded: context.root.excluded.clone(),
                options_file: Some(options_file.clone()),
                sdk_path: sdk_path.clone(),
                plugins: PluginsOptions::default(),
                legacy_plugin_path: Some(plugin_dir),
            });
        }

        // Dart `PluginWatcher._addPlugins`.
        if !options.plugins.configurations.is_empty() {
            jobs.push(PluginJob {
                context_root: context.root.root.clone(),
                excluded: context.root.excluded.clone(),
                options_file: Some(options_file.clone()),
                sdk_path,
                plugins: options.plugins.clone(),
                legacy_plugin_path: None,
            });
        }
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

/// Protocol `PrioritizedSourceChange` from a plugin (`edit.getFixes` / `edit.getAssists`).
#[derive(Clone, Debug)]
pub struct PrioritizedSourceChangeJson {
    pub priority: i32,
    pub change: Value,
}

/// Protocol `AnalysisErrorFixes` from a plugin (`edit.getFixes`).
#[derive(Clone, Debug)]
pub struct PluginErrorFixes {
    pub error: PluginError,
    pub fixes: Vec<PrioritizedSourceChangeJson>,
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
/// time of unchanged files). Returns `true` when the file was written.
fn write_if_changed(path: &Path, content: &str) -> std::io::Result<bool> {
    if std::fs::read_to_string(path).ok().as_deref() == Some(content) {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(true)
}

/// Recursively copies [src] directory to [dst].
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &dest_path)?;
        } else if file_type.is_file() {
            std::fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}

/// Dart `PluginIsolate.reportException` message for an exception without
/// a Dart stack trace.
fn plugin_error_message(message: &str) -> String {
    format!("An error occurred while executing an analyzer plugin: {message}\n")
}

/// Groups [jobs] so that contexts sharing the same legacy plugin path or the
/// same new-plugin configuration (`sdk_path` and `plugins`) share one
/// prepared plugin package and one plugin isolate.
fn group_jobs(jobs: &[PluginJob]) -> Vec<(String, Vec<&PluginJob>)> {
    let mut grouped: Vec<(String, Vec<&PluginJob>)> = Vec::new();
    for job in jobs {
        if let Some((_, list)) = grouped.iter_mut().find(|(id, list)| {
            let first = list[0];
            if job.is_legacy() || first.is_legacy() {
                *id == job.plugin_id()
            } else {
                first.sdk_path == job.sdk_path && first.plugins == job.plugins
            }
        }) {
            list.push(job);
        } else {
            grouped.push((job.plugin_id(), vec![job]));
        }
    }
    grouped
}

/// Prepares the plugin execution entrypoint and `package_config.json` for [job]
/// (Dart `_addPlugins` + `PluginManager.filesFor` + `_computeFiles`).
/// When `force_pub` is `false` and the prepared package already exists with
/// unchanged `pubspec.yaml`, `bin/plugin.dart`, and `.dart_tool/package_config.json`,
/// reuses it without running `dart pub upgrade`.
fn prepare_plugin_files(
    job: &PluginJob,
    dart: &str,
    force_pub: bool,
) -> Result<(PathBuf, PathBuf), String> {
    if let Some(plugin_path) = &job.legacy_plugin_path {
        let plugin_folder = Path::new(plugin_path);
        let pubspec_file = plugin_folder.join("pubspec.yaml");
        if !pubspec_file.is_file() {
            let entrypoint = plugin_folder.join("bin").join("plugin.dart");
            let package_config = plugin_folder.join(".dart_tool").join("package_config.json");
            if !entrypoint.is_file() {
                return Err(format!("File \"{}\" does not exist.", entrypoint.display()));
            }
            if !package_config.is_file() {
                return Err(format!(
                    "File \"{}\" does not exist.",
                    package_config.display()
                ));
            }
            return Ok((entrypoint, package_config));
        }
        let parent_folder = plugin_state_folder(plugin_path);
        let short_name = plugin_folder
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("analyzer_plugin");
        let execution_folder = parent_folder.join(short_name);
        let entrypoint_path = execution_folder.join("bin").join("plugin.dart");
        let package_config = execution_folder
            .join(".dart_tool")
            .join("package_config.json");
        if !force_pub && entrypoint_path.is_file() && package_config.is_file() {
            return Ok((entrypoint_path, package_config));
        }
        let pub_command = if parent_folder.is_dir() {
            "upgrade"
        } else {
            copy_dir_all(plugin_folder, &execution_folder).map_err(|e| {
                format!(
                    "cannot copy legacy plugin from {} to {}: {e}",
                    plugin_folder.display(),
                    execution_folder.display()
                )
            })?;
            "get"
        };
        return run_pub_and_check(&execution_folder, dart, pub_command);
    }

    let folder = plugin_state_folder(&job.context_root);
    let configurations = &job.plugins.configurations;
    let pubspec =
        generator::generate_pubspec(configurations, job.plugins.dependency_overrides.as_deref());
    let entrypoint = generator::generate_entrypoint(configurations);
    let pubspec_path = folder.join("pubspec.yaml");
    let entrypoint_path = folder.join("bin").join("plugin.dart");
    let package_config = folder.join(".dart_tool").join("package_config.json");
    let pubspec_changed = write_if_changed(&pubspec_path, &pubspec)
        .and_then(|changed| {
            write_if_changed(&entrypoint_path, &entrypoint)?;
            Ok(changed)
        })
        .map_err(|e| {
            format!(
                "cannot write the plugin package at {}: {e}",
                folder.display()
            )
        })?;
    if !force_pub && !pubspec_changed && entrypoint_path.is_file() && package_config.is_file() {
        return Ok((entrypoint_path, package_config));
    }
    run_pub_and_check(&folder, dart, "upgrade")
}

fn run_pub_and_check(
    folder: &Path,
    dart: &str,
    pub_command: &str,
) -> Result<(PathBuf, PathBuf), String> {
    let entrypoint_path = folder.join("bin").join("plugin.dart");
    let pub_result = Command::new(dart)
        .args(["pub", pub_command])
        .current_dir(folder)
        .env("PUB_ENVIRONMENT", "analysis_server.plugin_manager")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run {dart} pub {pub_command}: {e}"))?;
    let package_config = folder.join(".dart_tool").join("package_config.json");
    if !pub_result.status.success() && !package_config.is_file() {
        return Err(format!(
            "An error occurred while setting up the analyzer plugin package at '{}'. \
             The `dart pub {pub_command}` command failed:\n  exitCode = {}\n  stdout = {}\n  stderr = {}\n",
            folder.display(),
            pub_result.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&pub_result.stdout),
            String::from_utf8_lossy(&pub_result.stderr),
        ));
    }
    if !entrypoint_path.is_file() {
        return Err(format!(
            "File \"{}\" does not exist.",
            entrypoint_path.display()
        ));
    }
    if !package_config.is_file() {
        return Err(format!(
            "File \"{}\" does not exist.",
            package_config.display()
        ));
    }
    Ok((entrypoint_path, package_config))
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
    let grouped = group_jobs(jobs);
    // The isolates run in parallel, like the server's.
    let results: Vec<Result<PluginSessionOutput, String>> = std::thread::scope(|scope| {
        let handles: Vec<_> = grouped
            .iter()
            .map(|(_, group)| scope.spawn(|| run_job_group(group, roots, dart, &bridge)))
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
/// session for one plugin isolate across its context roots.
fn run_job_group(
    group: &[&PluginJob],
    roots: &[String],
    dart: &str,
    bridge: &Path,
) -> Result<PluginSessionOutput, String> {
    let first = group[0];
    let (entrypoint_path, package_config) = prepare_plugin_files(first, dart, false)?;
    let mut session = Session::start(dart, bridge, &entrypoint_path, &package_config, None)?;
    let result = session.analyze(group, roots);
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
        let text: String = line.chars().take(400).collect();
        let _ = writeln!(file, "{text}");
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
        wakeup: Option<Sender<Option<Value>>>,
    ) -> Result<Session, String> {
        let mut child = Command::new(dart)
            .arg(bridge)
            .arg(entrypoint)
            .arg(package_config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("cannot start the plugin bridge with {dart}: {e}"))?;
        let stdout = child.stdout.take().expect("piped stdout");
        let stdin = child.stdin.take();
        let (sender, incoming) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if let Ok(value) = serde_json::from_str::<Value>(&line) {
                    let is_event = value.get("event").is_some();
                    if sender.send(Incoming::Message(value)).is_err() {
                        return;
                    }
                    if is_event && let Some(w) = &wakeup {
                        let _ = w.send(Some(json!({"method": "$/dartr/pluginNotification"})));
                    }
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

    /// Drains any already-received messages without blocking.
    fn poll_nonblocking(&mut self) {
        if self.exited {
            return;
        }
        loop {
            match self.incoming.try_recv() {
                Ok(Incoming::Message(v)) => {
                    debug_log(&format!("<== {v}"));
                    if v.get("bridge").and_then(Value::as_str) == Some("exit") {
                        self.exited = true;
                        return;
                    }
                    self.handle(v);
                }
                Ok(Incoming::Closed) | Err(TryRecvError::Disconnected) => {
                    self.exited = true;
                    return;
                }
                Err(TryRecvError::Empty) => return,
            }
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
        self.request_with_timeout(method, params, timeout())
    }

    fn request_with_timeout(
        &mut self,
        method: &str,
        params: Value,
        max_wait: Duration,
    ) -> Result<Value, String> {
        let id = self.send(method, params);
        let deadline = Instant::now() + max_wait;
        loop {
            let wait = deadline.saturating_duration_since(Instant::now());
            let Some(message) = self.next_message(wait)? else {
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

    /// Waits for `plugin.status` (`isAnalyzing: true` -> `isAnalyzing: false`).
    /// Returns immediately if `self.analyzing` is already `Some(false)` (the
    /// plugin already started and finished analyzing before this wait began).
    fn wait_for_analysis_idle(&mut self, start_wait: Duration) -> Result<(), String> {
        self.poll_nonblocking();
        if self.analyzing == Some(false) {
            return Ok(());
        }
        let started = Instant::now();
        loop {
            if self.analyzing == Some(false) {
                break;
            }
            let seen_busy = self.analyzing == Some(true);
            let wait = if seen_busy {
                timeout()
            } else {
                start_wait.saturating_sub(started.elapsed())
            };
            if !seen_busy && wait.is_zero() {
                break;
            }
            match self.next_message(wait) {
                Ok(Some(message)) => {
                    self.handle(message);
                }
                Ok(None) => break,
                Err(e) if seen_busy => return Err(e),
                Err(_) => break,
            }
        }
        Ok(())
    }

    fn take_errors(&mut self) -> Vec<(String, Vec<PluginError>)> {
        let mut errors = Vec::new();
        for file in std::mem::take(&mut self.file_order) {
            let list = self.errors.remove(&file).unwrap_or_default();
            errors.push((file, list));
        }
        errors
    }

    /// `plugin.versionCheck`, the roots, then wait until the plugin is idle.
    fn analyze(
        &mut self,
        group: &[&PluginJob],
        roots: &[String],
    ) -> Result<PluginSessionOutput, String> {
        let first = group[0];
        let byte_store = state_location().join(".plugin_manager").join("byte_store");
        let result = self.request(
            "plugin.versionCheck",
            json!({
                "byteStorePath": byte_store.to_string_lossy(),
                "sdkPath": first.sdk_path,
                "version": "1.0.0-alpha.0",
            }),
        )?;
        if result.get("isCompatible").and_then(Value::as_bool) == Some(false) {
            return Err("Plugin is not compatible.".into());
        }
        // Dart `PluginIsolate._updatePluginRoots`: the context roots, then
        // the analysis roots of the client (new plugins use only these).
        let context_roots: Vec<Value> = group
            .iter()
            .map(|job| {
                let mut context_root = json!({
                    "root": job.context_root,
                    "exclude": job.excluded,
                });
                if let Some(options) = &job.options_file {
                    context_root["optionsFile"] = json!(options);
                }
                context_root
            })
            .collect();
        self.analyzing = None;
        if first.is_legacy() {
            self.request("analysis.setContextRoots", json!({"roots": context_roots}))?;
            if self.analyzing == Some(true) {
                self.wait_for_analysis_idle(Duration::from_millis(200))?;
            }
        } else {
            self.send("analysis.setContextRoots", json!({"roots": context_roots}));
            self.send(
                "analysis.setAnalysisRoots",
                json!({"included": roots, "excluded": []}),
            );
            self.wait_for_analysis_idle(idle_start_timeout())?;
        }
        Ok(PluginSessionOutput {
            errors: self.take_errors(),
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

/// One context root associated with a running plugin isolate.
#[derive(Clone, Debug, PartialEq, Eq)]
struct IsolateContextRoot {
    root: String,
    excluded: Vec<String>,
    options_file: Option<String>,
}

impl IsolateContextRoot {
    fn is_analyzed(&self, file_path: &str) -> bool {
        if !paths::is_within(&self.root, file_path) {
            return false;
        }
        for excluded in &self.excluded {
            if paths::is_within(excluded, file_path) {
                return false;
            }
        }
        true
    }

    fn to_json(&self) -> Value {
        let mut obj = json!({
            "root": self.root,
            "exclude": self.excluded,
        });
        if let Some(options) = &self.options_file {
            obj["optionsFile"] = json!(options);
        }
        obj
    }
}

struct ManagedIsolate {
    plugin_id: String,
    is_legacy: bool,
    sdk_path: String,
    plugins_options: PluginsOptions,
    context_roots: Vec<IsolateContextRoot>,
    interesting_globs: Vec<String>,
    session: Session,
    pending_analysis: bool,
    initial_pending: bool,
}

impl ManagedIsolate {
    fn is_analyzing(&self, file_path: &str) -> bool {
        self.context_roots.iter().any(|r| r.is_analyzed(file_path))
    }

    fn has_context_root(&self, root: &str) -> bool {
        self.context_roots.iter().any(|r| r.root == root)
    }

    fn matches_interesting_file(&self, file_path: &str) -> bool {
        self.interesting_globs
            .iter()
            .any(|g| Glob::new(g).is_ok_and(|glob| glob.matches(file_path)))
    }
}

/// A plugin isolate being prepared and started on a background thread in
/// parallel with the server's own analysis.
struct StartingIsolate {
    plugin_id: String,
    is_legacy: bool,
    sdk_path: String,
    plugins_options: PluginsOptions,
    context_roots: Vec<IsolateContextRoot>,
    pending_analysis: bool,
    initial_pending: bool,
    handle: std::thread::JoinHandle<Option<(Session, Vec<String>)>>,
}

/// Persistent plugin manager for `dartr language-server` (ports Dart's
/// `PluginManager` and `PluginWatcher`).
#[derive(Default)]
pub struct PluginManager {
    wakeup: Option<Sender<Option<Value>>>,
    isolates: Vec<ManagedIsolate>,
    starting: Vec<StartingIsolate>,
    overlays: BTreeMap<String, String>,
    priority_files: Vec<String>,
    included_roots: Vec<String>,
    excluded_roots: Vec<String>,
    force_restart: bool,
}

impl PluginManager {
    pub fn new(wakeup: Option<Sender<Option<Value>>>) -> Self {
        let mut manager = PluginManager::default();
        manager.wakeup = wakeup;
        manager
    }

    pub fn is_empty(&self) -> bool {
        self.isolates.is_empty() && self.starting.is_empty()
    }

    pub fn has_pending(&self) -> bool {
        self.isolates.iter().any(|i| i.pending_analysis)
            || self.starting.iter().any(|s| s.pending_analysis)
    }

    /// Marks all running plugins to be restarted on the next `sync_contexts`
    /// (when `analysis_options.yaml` or `package_config.json` changed).
    pub fn mark_restart_needed(&mut self) {
        self.force_restart = true;
    }

    /// Waits for any background startup threads to finish and moves their
    /// started sessions into `self.isolates`.
    fn finish_starting(&mut self) {
        for starting in self.starting.drain(..) {
            if let Ok(Some((session, interesting_globs))) = starting.handle.join() {
                self.isolates.push(ManagedIsolate {
                    plugin_id: starting.plugin_id,
                    is_legacy: starting.is_legacy,
                    sdk_path: starting.sdk_path,
                    plugins_options: starting.plugins_options,
                    context_roots: starting.context_roots,
                    interesting_globs,
                    session,
                    pending_analysis: starting.pending_analysis,
                    initial_pending: starting.initial_pending,
                });
            }
        }
    }

    /// Synchronizes the running plugin isolates with [jobs] for the current
    /// analysis contexts. Returns the `plugin_id`s of any stopped or
    /// restarted plugins so the server can clear their recorded errors.
    pub fn sync_contexts(
        &mut self,
        jobs: &[PluginJob],
        dart: Option<&str>,
        included: &[String],
        excluded: &[String],
        priority_files: &[String],
        has_analyzed_dart_files: &dyn Fn(&str) -> bool,
    ) -> Vec<String> {
        self.finish_starting();
        self.included_roots = included.to_vec();
        self.excluded_roots = excluded.to_vec();
        self.priority_files = priority_files.to_vec();
        let force_restart = std::mem::replace(&mut self.force_restart, false);

        let grouped = group_jobs(jobs);

        let mut removed_ids = Vec::new();
        let Some(dart) = dart else {
            for mut isolate in self.isolates.drain(..) {
                isolate.session.shutdown();
                removed_ids.push(isolate.plugin_id);
            }
            return removed_ids;
        };

        // Stop isolates that are no longer needed or whose configuration changed.
        let mut kept = Vec::new();
        for mut isolate in self.isolates.drain(..) {
            let matching = grouped.iter().find(|(id, _)| *id == isolate.plugin_id);
            let should_keep = if force_restart {
                false
            } else if let Some((_, group)) = matching {
                let first = group[0];
                let expected_roots: Vec<IsolateContextRoot> = group
                    .iter()
                    .map(|j| IsolateContextRoot {
                        root: j.context_root.clone(),
                        excluded: j.excluded.clone(),
                        options_file: j.options_file.clone(),
                    })
                    .collect();
                isolate.is_legacy == first.is_legacy()
                    && isolate.sdk_path == first.sdk_path
                    && isolate.plugins_options == first.plugins
                    && isolate.context_roots == expected_roots
                    && !isolate.session.exited
            } else {
                false
            };
            if should_keep {
                kept.push(isolate);
            } else {
                isolate.session.shutdown();
                removed_ids.push(isolate.plugin_id);
            }
        }
        self.isolates = kept;

        if grouped.is_empty() {
            return removed_ids;
        }

        let bridge = state_location()
            .join(".plugin_manager")
            .join("plugin_bridge.dart");
        if write_if_changed(&bridge, BRIDGE_SOURCE).is_err() {
            return removed_ids;
        }

        for (plugin_id, group) in grouped {
            if let Some(existing) = self.isolates.iter_mut().find(|i| i.plugin_id == plugin_id) {
                if !existing.is_legacy {
                    let has_files = existing
                        .context_roots
                        .iter()
                        .any(|r| has_analyzed_dart_files(&r.root));
                    if has_files {
                        existing.session.analyzing = None;
                        existing.pending_analysis = true;
                        existing.initial_pending = true;
                    }
                    existing.session.send(
                        "analysis.setAnalysisRoots",
                        json!({"included": included, "excluded": excluded}),
                    );
                }
                continue;
            }

            let first = group[0].clone();
            let context_roots: Vec<IsolateContextRoot> = group
                .iter()
                .map(|j| IsolateContextRoot {
                    root: j.context_root.clone(),
                    excluded: j.excluded.clone(),
                    options_file: j.options_file.clone(),
                })
                .collect();
            let roots_json: Vec<Value> = context_roots.iter().map(|r| r.to_json()).collect();
            let has_files = context_roots
                .iter()
                .any(|r| has_analyzed_dart_files(&r.root));
            let dart_owned = dart.to_string();
            let bridge_owned = bridge.clone();
            let wakeup = self.wakeup.clone();
            let overlays = self.overlays.clone();
            let priority_files_owned = self.priority_files.clone();
            let included_owned = included.to_vec();
            let excluded_owned = excluded.to_vec();
            let is_legacy = first.is_legacy();
            let sdk_path = first.sdk_path.clone();
            let plugins_options = first.plugins.clone();

            let handle = std::thread::spawn(move || -> Option<(Session, Vec<String>)> {
                let Ok((entrypoint, package_config)) =
                    prepare_plugin_files(&first, &dart_owned, force_restart)
                else {
                    return None;
                };
                let Ok(mut session) = Session::start(
                    &dart_owned,
                    &bridge_owned,
                    &entrypoint,
                    &package_config,
                    wakeup,
                ) else {
                    return None;
                };

                let byte_store = state_location().join(".plugin_manager").join("byte_store");
                let Ok(vc) = session.request(
                    "plugin.versionCheck",
                    json!({
                        "byteStorePath": byte_store.to_string_lossy(),
                        "sdkPath": first.sdk_path,
                        "version": "1.0.0-alpha.0",
                    }),
                ) else {
                    session.shutdown();
                    return None;
                };
                if vc.get("isCompatible").and_then(Value::as_bool) == Some(false) {
                    session.shutdown();
                    return None;
                }
                let interesting_globs: Vec<String> = vc
                    .get("interestingFiles")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(Value::as_str)
                            .map(|g| {
                                if g.starts_with("**/") {
                                    g.to_string()
                                } else {
                                    format!("**/{g}")
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_else(|| vec!["**/*.dart".to_string()]);

                // Send existing overlays first so the initial context collection
                // in the plugin sees the open document contents immediately.
                if !overlays.is_empty() {
                    let mut files = serde_json::Map::new();
                    for (path, content) in &overlays {
                        files.insert(path.clone(), json!({"type": "add", "content": content}));
                    }
                    session.send("analysis.updateContent", json!({"files": files}));
                }

                session.analyzing = None;
                if first.is_legacy() {
                    session.send("analysis.setContextRoots", json!({"roots": roots_json}));
                } else {
                    session.send("analysis.setContextRoots", json!({"roots": roots_json}));
                    session.send(
                        "analysis.setAnalysisRoots",
                        json!({"included": included_owned, "excluded": excluded_owned}),
                    );
                }

                if !priority_files_owned.is_empty() {
                    session.send(
                        "analysis.setSubscriptions",
                        subscriptions_json(&priority_files_owned),
                    );
                    session.send(
                        "analysis.setPriorityFiles",
                        json!({"files": priority_files_owned}),
                    );
                }
                Some((session, interesting_globs))
            });

            self.starting.push(StartingIsolate {
                plugin_id,
                is_legacy,
                sdk_path,
                plugins_options,
                context_roots,
                pending_analysis: has_files,
                initial_pending: has_files,
                handle,
            });
        }

        removed_ids
    }

    /// Updates priority files and subscriptions on all running plugins
    /// (Dart `_updateDriversAndPluginsPriorityFiles`).
    pub fn set_priority_files(&mut self, priority_files: &[String]) {
        self.finish_starting();
        self.priority_files = priority_files.to_vec();
        if self.isolates.is_empty() {
            return;
        }
        let subs = subscriptions_json(priority_files);
        let prio = json!({"files": priority_files});
        for isolate in &mut self.isolates {
            isolate
                .session
                .send("analysis.setSubscriptions", subs.clone());
            isolate
                .session
                .send("analysis.setPriorityFiles", prio.clone());
        }
    }

    /// Sends `AddContentOverlay` to all running plugins (`onOverlayCreated`).
    pub fn overlay_added(&mut self, path: &str, content: &str, changed_from_disk: bool) {
        self.finish_starting();
        self.overlays.insert(path.to_string(), content.to_string());
        if self.isolates.is_empty() {
            return;
        }
        let params = json!({
            "files": {
                path: {
                    "type": "add",
                    "content": content,
                }
            }
        });
        for isolate in &mut self.isolates {
            let will_analyze = isolate.is_analyzing(path)
                && path.ends_with(".dart")
                && (changed_from_disk || isolate.is_legacy);
            if will_analyze {
                isolate.session.analyzing = None;
                isolate.pending_analysis = true;
            }
            let _ = isolate
                .session
                .request("analysis.updateContent", params.clone());
        }
    }

    /// Sends `ChangeContentOverlay` to all running plugins (`onOverlayUpdated`).
    pub fn overlay_changed(&mut self, path: &str, new_content: &str, edits: Vec<Value>) {
        self.finish_starting();
        self.overlays
            .insert(path.to_string(), new_content.to_string());
        if self.isolates.is_empty() {
            return;
        }
        let params = json!({
            "files": {
                path: {
                    "type": "change",
                    "edits": edits,
                }
            }
        });
        for isolate in &mut self.isolates {
            let will_analyze = isolate.is_analyzing(path) && path.ends_with(".dart");
            if will_analyze {
                isolate.session.analyzing = None;
                isolate.pending_analysis = true;
            }
            let _ = isolate
                .session
                .request("analysis.updateContent", params.clone());
        }
    }

    /// Sends `RemoveContentOverlay` to all running plugins (`onOverlayDestroyed`).
    pub fn overlay_removed(&mut self, path: &str, changed_from_disk: bool) {
        self.finish_starting();
        self.overlays.remove(path);
        if self.isolates.is_empty() {
            return;
        }
        let params = json!({
            "files": {
                path: {
                    "type": "remove",
                }
            }
        });
        for isolate in &mut self.isolates {
            let will_analyze = isolate.is_analyzing(path)
                && path.ends_with(".dart")
                && (changed_from_disk || isolate.is_legacy);
            if will_analyze {
                isolate.session.analyzing = None;
                isolate.pending_analysis = true;
            }
            let _ = isolate
                .session
                .request("analysis.updateContent", params.clone());
        }
    }

    /// Broadcasts a file system watch event (`add`, `modify`, `remove`) to
    /// matching plugins (Dart `PluginManager.broadcastWatchEvent`).
    pub fn broadcast_watch_event(&mut self, kind: &str, path: &str) {
        self.finish_starting();
        if self.isolates.is_empty() {
            return;
        }
        let event_type = match kind {
            "add" => "ADD",
            "modify" => "MODIFY",
            "remove" => "REMOVE",
            _ => return,
        };
        let params = json!({
            "events": [{
                "type": event_type,
                "path": path,
            }]
        });
        for isolate in &mut self.isolates {
            if !isolate.is_analyzing(path) || !isolate.matches_interesting_file(path) {
                continue;
            }
            let will_analyze = path.ends_with(".dart") && kind != "remove" && !isolate.is_legacy;
            if will_analyze {
                isolate.session.analyzing = None;
                isolate.pending_analysis = true;
            }
            let _ = isolate
                .session
                .request("analysis.handleWatchEvents", params.clone());
        }
    }

    /// Waits for any pending plugin analysis passes and drains all
    /// `(plugin_id, file_path, errors)` notifications collected so far.
    pub fn wait_for_pending(&mut self) -> Vec<(String, String, Vec<PluginError>)> {
        self.finish_starting();
        for isolate in &mut self.isolates {
            if isolate.pending_analysis {
                isolate.pending_analysis = false;
                let is_initial = std::mem::replace(&mut isolate.initial_pending, false);
                if !isolate.is_legacy {
                    let start_wait = if is_initial {
                        idle_start_timeout()
                    } else {
                        Duration::from_millis(500)
                    };
                    let _ = isolate.session.wait_for_analysis_idle(start_wait);
                } else if isolate.session.analyzing == Some(true) {
                    let _ = isolate
                        .session
                        .wait_for_analysis_idle(Duration::from_millis(200));
                }
            }
        }
        self.drain_errors()
    }

    /// Non-blockingly polls all running plugin isolates and drains any new
    /// `(plugin_id, file_path, errors)` notifications.
    pub fn poll(&mut self) -> Vec<(String, String, Vec<PluginError>)> {
        let mut still_starting = Vec::new();
        for starting in self.starting.drain(..) {
            if starting.handle.is_finished() {
                if let Ok(Some((session, interesting_globs))) = starting.handle.join() {
                    self.isolates.push(ManagedIsolate {
                        plugin_id: starting.plugin_id,
                        is_legacy: starting.is_legacy,
                        sdk_path: starting.sdk_path,
                        plugins_options: starting.plugins_options,
                        context_roots: starting.context_roots,
                        interesting_globs,
                        session,
                        pending_analysis: starting.pending_analysis,
                        initial_pending: starting.initial_pending,
                    });
                }
            } else {
                still_starting.push(starting);
            }
        }
        self.starting = still_starting;
        for isolate in &mut self.isolates {
            isolate.session.poll_nonblocking();
        }
        self.drain_errors()
    }

    fn drain_errors(&mut self) -> Vec<(String, String, Vec<PluginError>)> {
        let mut out = Vec::new();
        for isolate in &mut self.isolates {
            for (file, errors) in isolate.session.take_errors() {
                out.push((isolate.plugin_id.clone(), file, errors));
            }
        }
        out
    }

    /// Sends `edit.getFixes` to all plugins associated with [context_root]
    /// (Dart `PluginCodeActionsProducer.getFixActions`).
    pub fn get_fixes(
        &mut self,
        context_root: Option<&str>,
        file: &str,
        offset: u32,
    ) -> Vec<PluginErrorFixes> {
        self.finish_starting();
        let mut out = Vec::new();
        let params = json!({"file": file, "offset": offset});
        for isolate in &mut self.isolates {
            if let Some(root) = context_root
                && !isolate.has_context_root(root)
            {
                continue;
            }
            if let Ok(result) = isolate.session.request_with_timeout(
                "edit.getFixes",
                params.clone(),
                Duration::from_secs(5),
            ) && let Some(fixes_list) = result.get("fixes").and_then(Value::as_array)
            {
                for item in fixes_list {
                    let Some(error) = item.get("error").and_then(parse_error) else {
                        continue;
                    };
                    let fixes = item
                        .get("fixes")
                        .and_then(Value::as_array)
                        .map(|arr| {
                            arr.iter()
                                .filter_map(parse_prioritized_source_change)
                                .collect()
                        })
                        .unwrap_or_default();
                    out.push(PluginErrorFixes { error, fixes });
                }
            }
        }
        out
    }

    /// Sends `edit.getAssists` to all plugins associated with [context_root]
    /// (Dart `PluginCodeActionsProducer.getAssistActions`).
    pub fn get_assists(
        &mut self,
        context_root: Option<&str>,
        file: &str,
        offset: u32,
        length: u32,
    ) -> Vec<PrioritizedSourceChangeJson> {
        self.finish_starting();
        let mut out = Vec::new();
        let params = json!({"file": file, "offset": offset, "length": length});
        for isolate in &mut self.isolates {
            if let Some(root) = context_root
                && !isolate.has_context_root(root)
            {
                continue;
            }
            if let Ok(result) = isolate.session.request_with_timeout(
                "edit.getAssists",
                params.clone(),
                Duration::from_secs(5),
            ) && let Some(assists_list) = result.get("assists").and_then(Value::as_array)
            {
                for item in assists_list {
                    if let Some(change) = parse_prioritized_source_change(item) {
                        out.push(change);
                    }
                }
            }
        }
        out
    }

    /// Stops all running plugin isolates.
    pub fn stop_all(&mut self) {
        self.finish_starting();
        for mut isolate in self.isolates.drain(..) {
            isolate.session.shutdown();
        }
    }
}

impl Drop for PluginManager {
    fn drop(&mut self) {
        self.stop_all();
    }
}

fn subscriptions_json(priority_files: &[String]) -> Value {
    json!({
        "subscriptions": {
            "FOLDING": priority_files,
            "HIGHLIGHTS": priority_files,
            "NAVIGATION": priority_files,
            "OCCURRENCES": priority_files,
            "OUTLINE": priority_files,
        }
    })
}

fn parse_prioritized_source_change(value: &Value) -> Option<PrioritizedSourceChangeJson> {
    Some(PrioritizedSourceChangeJson {
        priority: value.get("priority")?.as_i64()? as i32,
        change: value.get("change")?.clone(),
    })
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
