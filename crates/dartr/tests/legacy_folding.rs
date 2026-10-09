// Dart source: pkg/analysis_server/lib/src/computer/computer_folding.dart

//! Differential coverage for legacy `analysis.folding` notifications.

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

const TIMEOUT: Duration = Duration::from_secs(60);
const QUIET: Duration = Duration::from_millis(300);

struct Client {
    child: Child,
    input: ChildStdin,
    messages: Receiver<Option<Value>>,
    next_id: u32,
    responses: BTreeMap<String, Value>,
    folding: BTreeMap<String, Vec<Value>>,
    analyzing: bool,
    completions: usize,
}

impl Client {
    fn start(program: &Path, arguments: &[&str]) -> Self {
        let mut child = Command::new(program)
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap_or_else(|error| panic!("cannot start {}: {error}", program.display()));
        let input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        let (sender, messages) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let line = line.unwrap();
                if !line.trim().is_empty() {
                    sender
                        .send(Some(serde_json::from_str(&line).unwrap()))
                        .unwrap();
                }
            }
            let _ = sender.send(None);
        });
        Self {
            child,
            input,
            messages,
            next_id: 0,
            responses: BTreeMap::new(),
            folding: BTreeMap::new(),
            analyzing: false,
            completions: 0,
        }
    }

    fn request(&mut self, method: &str, params: Value) {
        self.next_id += 1;
        let id = self.next_id.to_string();
        serde_json::to_writer(
            &mut self.input,
            &json!({"id": id, "method": method, "params": params}),
        )
        .unwrap();
        self.input.write_all(b"\n").unwrap();
        self.input.flush().unwrap();
        let deadline = Instant::now() + TIMEOUT;
        while !self.responses.contains_key(&id) {
            self.receive(deadline);
        }
        let response = self.responses.remove(&id).unwrap();
        assert!(response.get("error").is_none(), "{method}: {response}");
    }

    fn receive(&mut self, deadline: Instant) -> bool {
        match self
            .messages
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        {
            Ok(Some(message)) => {
                self.handle(message);
                true
            }
            Ok(None) => panic!("legacy server exited unexpectedly"),
            Err(RecvTimeoutError::Timeout) => false,
            Err(RecvTimeoutError::Disconnected) => panic!("legacy server output disconnected"),
        }
    }

    fn handle(&mut self, message: Value) {
        if let Some(id) = message.get("id").and_then(Value::as_str) {
            self.responses.insert(id.to_string(), message);
            return;
        }
        match message.get("event").and_then(Value::as_str) {
            Some("server.status") => {
                if let Some(value) = message["params"]["analysis"]["isAnalyzing"].as_bool() {
                    self.analyzing = value;
                    if !value {
                        self.completions += 1;
                    }
                }
            }
            Some("analysis.folding") => {
                let params = &message["params"];
                self.folding.insert(
                    params["file"].as_str().unwrap().to_string(),
                    params["regions"].as_array().unwrap().clone(),
                );
            }
            _ => {}
        }
    }

    fn settle(&mut self, previous_completions: usize) {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            if !self.receive(Instant::now() + QUIET)
                && !self.analyzing
                && self.completions > previous_completions
            {
                return;
            }
            assert!(Instant::now() < deadline, "legacy analysis did not settle");
        }
    }

    fn finish(mut self) {
        self.request("server.shutdown", json!({}));
        drop(self.input);
        assert!(self.child.wait().unwrap().success());
    }
}

fn source(label: &str) -> String {
    format!(
        "// Header one\n// Header two\n\nimport 'dart:async';\nimport 'dart:collection';\n\n/// Documentation\nclass {label} {{\n  void method() {{\n    final values = <int>[\n      1,\n      2,\n    ];\n    if (values.isNotEmpty) {{\n      print(values);\n    }}\n  }}\n}}\n"
    )
}

fn dart_binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("DART_BIN") {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    let output = Command::new("sh")
        .args(["-c", "command -v dart"])
        .output()
        .ok()?;
    let wrapper = PathBuf::from(String::from_utf8(output.stdout).ok()?.trim());
    let direct = wrapper.parent()?.join("cache/dart-sdk/bin/dart");
    if direct.is_file() {
        Some(direct)
    } else {
        Some(wrapper)
    }
}

fn run(program: &Path, arguments: &[&str], root: &Path, file: &Path, outside: &Path) -> Vec<Value> {
    let mut client = Client::start(program, arguments);
    client.request(
        "server.setSubscriptions",
        json!({"subscriptions":["STATUS"]}),
    );
    client.request(
        "analysis.setSubscriptions",
        json!({"subscriptions":{"FOLDING":[file, outside]}}),
    );
    let completed = client.completions;
    client.request(
        "analysis.setAnalysisRoots",
        json!({"included":[root],"excluded":[]}),
    );
    client.settle(completed);
    let disk = client.folding.remove(file.to_str().unwrap()).unwrap();
    let outside_regions = client.folding.remove(outside.to_str().unwrap()).unwrap();

    let completed = client.completions;
    client.request(
        "analysis.updateContent",
        json!({"files":{(file.to_str().unwrap()):{"type":"add","content":source("Overlay")}}}),
    );
    client.settle(completed);
    let overlay = client.folding.remove(file.to_str().unwrap()).unwrap();
    client.finish();
    vec![
        Value::Array(disk),
        Value::Array(outside_regions),
        Value::Array(overlay),
    ]
}

#[test]
fn legacy_folding_matches_dart_for_disk_overlay_and_root_scope() {
    let Some(dart_binary) = dart_binary() else {
        eprintln!("skipped: Dart is not available");
        return;
    };
    let version = Command::new(&dart_binary)
        .arg("--version")
        .output()
        .unwrap();
    let version = format!(
        "{}{}",
        String::from_utf8_lossy(&version.stdout),
        String::from_utf8_lossy(&version.stderr)
    );
    assert!(
        version.contains("3.13.3"),
        "requires Dart 3.13.3: {version}"
    );
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!("dartr-legacy-folding-{unique}"));
    let root = base.join("project");
    fs::create_dir_all(&root).unwrap();
    let file = root.join("main.dart");
    let outside = base.join("outside.dart");
    fs::write(&file, source("Disk")).unwrap();
    fs::write(&outside, source("Outside")).unwrap();

    let dart = run(
        &dart_binary,
        &["language-server", "--protocol=analyzer"],
        &root,
        &file,
        &outside,
    );
    let dartr = run(
        Path::new(env!("CARGO_BIN_EXE_dartr")),
        &["language-server", "--protocol=analyzer"],
        &root,
        &file,
        &outside,
    );
    assert_eq!(dartr, dart);
    fs::remove_dir_all(base).unwrap();
}
