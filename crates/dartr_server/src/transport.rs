// Dart source: pkg/analysis_server/lib/src/lsp/lsp_packet_transformer.dart
// Dart source: pkg/analysis_server/lib/src/lsp/channel/lsp_byte_stream_channel.dart

//! JSON-RPC over stdio with LSP framing (`Content-Length` headers).
//!
//! The messages are plain [`serde_json::Value`]s: the Dart LSP extensions
//! (custom methods, `experimental` capabilities, initialization options) do
//! not fit typed LSP crates, and plain values keep the JSON that dartr sends
//! under control (no `null` fields that Dart omits).

use std::fs::File;
use std::io::{self, BufRead, Write};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

/// Reads one message. Returns `Ok(None)` at the end of the input.
pub fn read_message(input: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut content_length: Option<usize> = None;
    let mut line = String::new();
    loop {
        line.clear();
        if input.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            if content_length.is_some() {
                break;
            }
            // Blank lines before the headers are skipped.
            continue;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().ok();
            }
        }
    }
    let length = content_length.unwrap_or(0);
    let mut body = vec![0u8; length];
    input.read_exact(&mut body)?;
    match serde_json::from_slice(&body) {
        Ok(v) => Ok(Some(v)),
        Err(e) => Err(io::Error::new(io::ErrorKind::InvalidData, e)),
    }
}

/// Writes one framed message.
pub fn write_message(output: &mut impl Write, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message)?;
    write!(output, "Content-Length: {}\r\n\r\n", body.len())?;
    output.write_all(&body)?;
    output.flush()
}

/// The output channel: framed messages to stdout, and a copy to the protocol
/// traffic log (`--protocol-traffic-log`) when there is one. Shared by the
/// reader thread (for the log) and the main loop.
#[derive(Clone)]
pub struct Channel {
    inner: Arc<Mutex<ChannelInner>>,
}

struct ChannelInner {
    out: Box<dyn Write + Send>,
    log: Option<File>,
}

impl Channel {
    pub fn new(out: Box<dyn Write + Send>, log_path: Option<&str>) -> Channel {
        let log = log_path.and_then(|p| File::create(p).ok());
        Channel {
            inner: Arc::new(Mutex::new(ChannelInner { out, log })),
        }
    }

    /// Sends [message] to the client.
    pub fn send(&self, message: &Value) {
        let mut inner = self.inner.lock().unwrap();
        log_line(&mut inner.log, "<=", message);
        // A closed output means the client is gone; the main loop exits when
        // the input closes.
        let _ = write_message(&mut inner.out, message);
    }

    /// Records an incoming [message] in the traffic log.
    pub fn log_incoming(&self, message: &Value) {
        let mut inner = self.inner.lock().unwrap();
        log_line(&mut inner.log, "=>", message);
    }
}

fn log_line(log: &mut Option<File>, direction: &str, message: &Value) {
    if let Some(file) = log {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let _ = writeln!(file, "{millis}:{direction}:{message}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut buf = Vec::new();
        let m = serde_json::json!({"jsonrpc": "2.0", "method": "x", "params": {"a": "ü"}});
        write_message(&mut buf, &m).unwrap();
        write_message(&mut buf, &m).unwrap();
        let mut input = io::Cursor::new(buf);
        assert_eq!(read_message(&mut input).unwrap(), Some(m.clone()));
        assert_eq!(read_message(&mut input).unwrap(), Some(m));
        assert_eq!(read_message(&mut input).unwrap(), None);
    }

    #[test]
    fn extra_headers() {
        let body = r#"{"id":1}"#;
        let raw = format!(
            "Content-Type: application/vscode-jsonrpc; charset=utf-8\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let mut input = io::Cursor::new(raw.into_bytes());
        assert_eq!(
            read_message(&mut input).unwrap(),
            Some(serde_json::json!({"id": 1}))
        );
    }
}
