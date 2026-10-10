// Dart source: pkg/analysis_server/lib/src/channel/byte_stream_channel.dart
// Dart source: pkg/analysis_server/lib/protocol/protocol.dart

//! Executable transport tests for both legacy entry points and traffic logs.

use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn legacy_aliases_recover_after_malformed_lines_and_keep_logs_off_stdout() {
    for args in [
        vec!["analysis-server"],
        vec!["language-server", "--protocol=analyzer"],
    ] {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let log = std::env::temp_dir().join(format!("dartr-legacy-traffic-{nonce}.log"));
        let mut child = Command::new(env!("CARGO_BIN_EXE_dartr"))
            .args(&args)
            .arg("--protocol-traffic-log")
            .arg(&log)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        // Split writes exercise buffering independently of pipe write boundaries.
        input
            .write_all(b"not JSON\n[]\n{\"id\":123,\"method\":\"server.getVersion\"}\n")
            .unwrap();
        input
            .write_all(b"{\"id\":\"version\",\"method\":\"server.get")
            .unwrap();
        input
            .write_all(b"Version\",\"params\":null,\"clientRequestTime\":null}\r\n")
            .unwrap();
        writeln!(
            input,
            "{}",
            json!({"id":"unsupported","method":"completion.registerLibraryPaths"})
        )
        .unwrap();
        writeln!(input, "{}", json!({"id":"stop","method":"server.shutdown"})).unwrap();
        drop(input);
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let lines = String::from_utf8(output.stdout).unwrap();
        let messages: Vec<Value> = lines
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(messages[0]["event"], "server.connected");
        assert_eq!(messages[0]["params"]["version"], "1.40.1");
        for message in &messages[1..4] {
            assert_eq!(
                *message,
                json!({"id":"","error":{"code":"INVALID_REQUEST","message":"Invalid request"}})
            );
        }
        assert_eq!(
            messages[4],
            json!({"id":"version","result":{"version":"1.40.1"}})
        );
        assert_eq!(
            messages[5],
            json!({"id":"unsupported","error":{"code":"UNKNOWN_REQUEST","message":"Unknown request"}})
        );
        assert_eq!(messages[6], json!({"id":"stop"}));
        assert_eq!(messages.len(), 7);
        let traffic = std::fs::read_to_string(&log).unwrap();
        let incoming: Vec<_> = traffic
            .lines()
            .filter_map(|line| line.split_once(":=>:"))
            .collect();
        let outgoing: Vec<Value> = traffic
            .lines()
            .filter_map(|line| line.split_once(":<=:"))
            .map(|(_, line)| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(incoming.len(), 6);
        assert!(
            incoming
                .iter()
                .all(|(time, _)| time.parse::<u128>().is_ok())
        );
        assert_eq!(outgoing, messages);
        std::fs::remove_file(log).unwrap();
    }
}

#[test]
fn line_splitter_accepts_lf_crlf_cr_and_unterminated_final_requests() {
    for separator in ["\n", "\r\n", "\r"] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_dartr"))
            .arg("analysis-server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        write!(
            input,
            "{}{}{}",
            json!({"id":"version","method":"server.getVersion"}),
            separator,
            json!({"id":"shutdown","method":"server.shutdown"})
        )
        .unwrap();
        drop(input);
        let result = child.wait_with_output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let messages: Vec<Value> = String::from_utf8(result.stdout)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(messages.len(), 3, "separator {separator:?}: {messages:?}");
        assert_eq!(
            messages[1],
            json!({"id":"version","result":{"version":"1.40.1"}})
        );
        assert_eq!(messages[2], json!({"id":"shutdown"}));
    }
}
