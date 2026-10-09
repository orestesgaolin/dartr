//! Compares the capabilities of `dartr language-server` with `dart
//! language-server` (3.13.3, needs `dart` on `PATH`) for two clients:
//! Dart-Code (dynamic registration) and a client without dynamic
//! registration (static capabilities only).
//!
//! - Every capability and registration that dartr advertises must be equal
//!   to the one of the Dart server.
//! - The capabilities that the Dart server advertises and dartr does not are
//!   the to-do list of phase 10b. The test prints them and compares them with
//!   `lsp_fixtures/missing_capabilities.txt`; when a feature is added, remove
//!   it from that file (`DARTR_UPDATE_MISSING=1` rewrites it).

mod lsp_support;

use std::collections::BTreeMap;

use lsp_support::*;
use serde_json::{Value, json};

/// The capabilities of a server: flattened static capabilities (path to
/// value) and dynamic registrations (key to options).
#[derive(Debug, Default)]
struct Capabilities {
    statics: BTreeMap<String, Value>,
    registrations: BTreeMap<String, Value>,
}

fn flatten(prefix: &str, v: &Value, out: &mut BTreeMap<String, Value>) {
    match v {
        Value::Object(m) if !m.is_empty() => {
            for (k, v) in m {
                let path = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(&path, v, out);
            }
        }
        Value::Array(list) if prefix == "executeCommandProvider.commands" => {
            for c in list {
                out.insert(format!("command {}", c.as_str().unwrap()), json!(true));
            }
        }
        _ => {
            out.insert(prefix.to_string(), v.clone());
        }
    }
}

/// A readable key of a registration: the method and the languages or
/// patterns of its document selector.
fn registration_key(r: &Value) -> String {
    let method = r["method"].as_str().unwrap();
    let selector = r["registerOptions"]["documentSelector"]
        .as_array()
        .map(|s| {
            s.iter()
                .map(|f| {
                    f["pattern"]
                        .as_str()
                        .or(f["language"].as_str())
                        .unwrap_or("?")
                        .to_string()
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    if selector.is_empty() {
        method.to_string()
    } else {
        format!("{method} [{selector}]")
    }
}

fn capabilities_of(program: &str, args: &[&str], init: Value) -> Capabilities {
    let mut c = LspClient::spawn(program, args, &[]);
    let result = c.request("initialize", init);
    c.notify("initialized", json!({}));
    c.settle(false);
    let mut caps = Capabilities::default();
    flatten("", &result["result"]["capabilities"], &mut caps.statics);
    for (method, params) in &c.server_requests {
        if method == "client/registerCapability" {
            for r in params["registrations"].as_array().unwrap() {
                let options = r.get("registerOptions").cloned().unwrap_or(Value::Null);
                caps.registrations.insert(registration_key(r), options);
            }
        }
    }
    let _ = c.shutdown_and_exit();
    caps
}

/// Removes every `dynamicRegistration` from client capabilities.
fn without_dynamic_registration(v: &mut Value) {
    if let Value::Object(m) = v {
        m.remove("dynamicRegistration");
        for v in m.values_mut() {
            without_dynamic_registration(v);
        }
    }
}

#[test]
fn capabilities_match_dart_language_server() {
    if !dart_available() {
        eprintln!("skipped: `dart` is not on PATH");
        return;
    }
    let root = fixtures().join("lsp_project");
    let dynamic_init = dart_code_initialize_params(&root);
    let mut static_init = dynamic_init.clone();
    without_dynamic_registration(&mut static_init["capabilities"]);

    let mut problems = Vec::new();
    let mut missing = Vec::new();
    for (client, init) in [("dart-code", &dynamic_init), ("static", &static_init)] {
        let dart = capabilities_of("dart", &["language-server", "--protocol=lsp"], init.clone());
        let dartr = capabilities_of(
            env!("CARGO_BIN_EXE_dartr"),
            &["language-server", "--protocol=lsp"],
            init.clone(),
        );
        for (kind, d, r) in [
            ("static", &dart.statics, &dartr.statics),
            ("dynamic", &dart.registrations, &dartr.registrations),
        ] {
            for (key, value) in r {
                match d.get(key) {
                    None => problems.push(format!("{client} {kind} {key}: not advertised by dart")),
                    Some(v) if v != value => problems.push(format!(
                        "{client} {kind} {key}: dart={v} dartr={value}"
                    )),
                    Some(_) => {}
                }
            }
            for key in d.keys().filter(|k| !r.contains_key(*k)) {
                missing.push(format!("{client} {kind} {key}"));
            }
        }
    }

    println!("Capabilities of dart language-server that dartr does not advertise:");
    for m in &missing {
        println!("  {m}");
    }
    assert!(problems.is_empty(), "capability differences:\n{}", problems.join("\n"));

    let path = fixtures().join("missing_capabilities.txt");
    let actual = missing.join("\n") + "\n";
    if std::env::var_os("DARTR_UPDATE_MISSING").is_some() {
        std::fs::write(&path, &actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(
        expected, actual,
        "the missing capabilities changed: update {} (DARTR_UPDATE_MISSING=1)",
        path.display()
    );
}
