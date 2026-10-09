//! A small LSP client for the differential tests of the language server: it
//! starts a server process, sends a scripted session, answers the requests
//! of the server like Dart-Code, and keeps the last state of the
//! notifications per document.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// The time without messages after which a session step is complete.
const QUIET: Duration = Duration::from_millis(400);
/// The longest time that one step can take.
const STEP_TIMEOUT: Duration = Duration::from_secs(60);

/// The root of the workspace (the repository).
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/lsp_fixtures")
}

/// Whether `dart` is on `PATH`.
pub fn dart_available() -> bool {
    Command::new("dart")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Dart `Uri.file(path).toString()` for the paths of the fixtures.
pub fn file_uri(path: &Path) -> String {
    format!("file://{}", path.display()).replace(' ', "%20")
}

/// The Dart-Code `initialize` parameters for [root]
/// (`lsp_fixtures/dart_code_initialize.json`).
pub fn dart_code_initialize_params(root: &Path) -> Value {
    let text = std::fs::read_to_string(fixtures().join("dart_code_initialize.json")).unwrap();
    let text = text
        .replace("${ROOT_PATH}", &root.display().to_string())
        .replace("${ROOT_URI}", &file_uri(root))
        .replace(
            "${ROOT_NAME}",
            &root.file_name().unwrap().to_string_lossy(),
        );
    serde_json::from_str(&text).unwrap()
}

/// The notifications of the server, last value per document.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocumentState {
    pub diagnostics: BTreeMap<String, Vec<Value>>,
    pub closing_labels: BTreeMap<String, Value>,
    pub outlines: BTreeMap<String, Value>,
    pub flutter_outlines: BTreeMap<String, Value>,
}

/// A running server and the state of the session.
pub struct LspClient {
    child: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<Option<Value>>,
    next_id: i64,
    /// Every message from the server, in order.
    pub log: Vec<Value>,
    pub state: DocumentState,
    /// `$/progress` begin events without an end.
    open_progress: i64,
    /// Number of `$/progress` end events (and `$/analyzerStatus` false).
    pub progress_ends: usize,
    /// The requests of the server: method and params.
    pub server_requests: Vec<(String, Value)>,
    /// The `window/showMessage` and `window/logMessage` messages.
    pub messages: Vec<Value>,
    /// The `dart` settings that `workspace/configuration` returns.
    pub configuration: Value,
    /// The program that was started (for failure messages).
    program: String,
}

impl LspClient {
    /// Starts [program] with [args].
    pub fn spawn(program: &str, args: &[&str], env: &[(&str, &str)]) -> LspClient {
        let mut command = Command::new(program);
        command
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in env {
            command.env(k, v);
        }
        let mut child = command
            .spawn()
            .unwrap_or_else(|e| panic!("cannot start {program}: {e}"));
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                match read_message(&mut reader) {
                    Some(m) => {
                        if tx.send(Some(m)).is_err() {
                            return;
                        }
                    }
                    None => {
                        let _ = tx.send(None);
                        return;
                    }
                }
            }
        });
        thread::spawn(move || {
            let mut s = String::new();
            let _ = BufReader::new(stderr).read_to_string(&mut s);
            if !s.trim().is_empty() {
                eprintln!("[server stderr] {s}");
            }
        });
        LspClient {
            child,
            stdin,
            rx,
            next_id: 0,
            log: Vec::new(),
            state: DocumentState::default(),
            open_progress: 0,
            progress_ends: 0,
            server_requests: Vec::new(),
            messages: Vec::new(),
            configuration: json!({}),
            program: program.to_string(),
        }
    }

    /// The process id of the server.
    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// Records a marker in the log.
    pub fn mark(&mut self, step: &str) {
        self.log.push(json!({"step": step}));
    }

    pub fn send(&mut self, message: &Value) {
        let body = serde_json::to_vec(message).unwrap();
        let stdin = self.stdin.as_mut().expect("stdin closed");
        write!(stdin, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        stdin.write_all(&body).unwrap();
        stdin.flush().unwrap();
    }

    pub fn notify(&mut self, method: &str, params: Value) {
        self.send(&json!({"jsonrpc": "2.0", "method": method, "params": params}));
    }

    /// Sends a request and returns its id.
    pub fn send_request(&mut self, method: &str, params: Value) -> i64 {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        id
    }

    /// Sends a request and waits for its response (`result` or `error`).
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.send_request(method, params);
        self.wait_for_response(id)
    }

    pub fn wait_for_response(&mut self, id: i64) -> Value {
        let deadline = Instant::now() + STEP_TIMEOUT;
        loop {
            let m = self
                .next_message(deadline)
                .unwrap_or_else(|| panic!("no response for request {id}"));
            if m.get("method").is_none() && m.get("id") == Some(&json!(id)) {
                let mut response = m.clone();
                if let Some(o) = response.as_object_mut() {
                    o.remove("id");
                    o.remove("jsonrpc");
                }
                return response;
            }
        }
    }

    /// Waits until the server is quiet and analysis is not running. With
    /// [expect_analysis], also waits for the end of an analysis that
    /// started after the call.
    #[track_caller]
    pub fn settle(&mut self, expect_analysis: bool) {
        let ends_before = self.progress_ends;
        let deadline = Instant::now() + STEP_TIMEOUT;
        loop {
            match self.next_message(Instant::now() + QUIET) {
                Some(_) => {}
                None => {
                    let analysis_done = !expect_analysis || self.progress_ends > ends_before;
                    if self.open_progress == 0 && analysis_done {
                        self.log.push(json!({"settled": self.progress_ends}));
                        return;
                    }
                }
            }
            assert!(
                Instant::now() < deadline,
                "the server did not settle: program={} caller={} expect_analysis={expect_analysis} \
                 open_progress={} progress_ends={} (before: {ends_before})",
                self.program,
                std::panic::Location::caller(),
                self.open_progress,
                self.progress_ends,
            );
        }
    }

    /// The next message (handled), or `None` at [deadline].
    fn next_message(&mut self, deadline: Instant) -> Option<Value> {
        let timeout = deadline.saturating_duration_since(Instant::now());
        let m = match self.rx.recv_timeout(timeout) {
            Ok(Some(m)) => m,
            Ok(None) => {
                // The server closed its output.
                thread::sleep(timeout.min(Duration::from_millis(50)));
                return None;
            }
            Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => return None,
        };
        if std::env::var_os("LSP_TRACE").is_some() {
            eprintln!("[{}] {}", self.program, m.to_string().chars().take(300).collect::<String>());
        }
        self.handle(&m);
        self.log.push(m.clone());
        Some(m)
    }

    fn handle(&mut self, m: &Value) {
        let Some(method) = m.get("method").and_then(Value::as_str) else {
            return;
        };
        let params = m.get("params").cloned().unwrap_or(Value::Null);
        if let Some(id) = m.get("id") {
            // A request of the server: answer like Dart-Code.
            self.server_requests.push((method.to_string(), params.clone()));
            let result = match method {
                "workspace/configuration" => {
                    let n = params["items"].as_array().map(|a| a.len()).unwrap_or(0);
                    Value::Array(vec![self.configuration.clone(); n])
                }
                _ => Value::Null,
            };
            self.send(&json!({"jsonrpc": "2.0", "id": id, "result": result}));
            return;
        }
        let uri = params
            .get("uri")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        match method {
            "textDocument/publishDiagnostics" => {
                let list = params["diagnostics"].as_array().cloned().unwrap_or_default();
                self.state.diagnostics.insert(uri, list);
            }
            "dart/textDocument/publishClosingLabels" => {
                self.state.closing_labels.insert(uri, params["labels"].clone());
            }
            "dart/textDocument/publishOutline" => {
                self.state.outlines.insert(uri, params["outline"].clone());
            }
            "dart/textDocument/publishFlutterOutline" => {
                self.state
                    .flutter_outlines
                    .insert(uri, params["outline"].clone());
            }
            "$/progress" => match params["value"]["kind"].as_str() {
                Some("begin") => self.open_progress += 1,
                Some("end") => {
                    self.open_progress -= 1;
                    self.progress_ends += 1;
                }
                _ => {}
            },
            "$/analyzerStatus" => {
                if params["isAnalyzing"] == json!(false) {
                    self.progress_ends += 1;
                }
            }
            "window/showMessage" | "window/logMessage" => self.messages.push(params),
            _ => {}
        }
    }

    /// Sends `shutdown` and `exit` and returns the exit code.
    pub fn shutdown_and_exit(mut self) -> (Value, i32) {
        let response = self.request("shutdown", Value::Null);
        self.notify("exit", Value::Null);
        drop(self.stdin.take());
        let code = wait_with_timeout(&mut self.child, Duration::from_secs(30));
        (response, code)
    }

    /// Closes stdin without `shutdown` and returns the exit code.
    pub fn close(mut self) -> i32 {
        drop(self.stdin.take());
        wait_with_timeout(&mut self.child, Duration::from_secs(30))
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn wait_with_timeout(child: &mut Child, timeout: Duration) -> i32 {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status.code().unwrap_or(-1);
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("the server did not exit");
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn read_message(reader: &mut impl BufRead) -> Option<Value> {
    let mut length = None;
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let t = line.trim_end();
        if t.is_empty() {
            if length.is_some() {
                break;
            }
            continue;
        }
        if let Some(v) = t.strip_prefix("Content-Length:") {
            length = v.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; length?];
    reader.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}
