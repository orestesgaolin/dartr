// Dart source: pkg/analysis_server/lib/src/channel/byte_stream_channel.dart
// Dart source: pkg/analysis_server/lib/protocol/protocol.dart

//! One UTF-8 JSON envelope per line. Protocol logging never writes to stdout.

use serde_json::Value;
use std::fs::File;
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Channel<W: Write> {
    output: W,
    log: Option<File>,
}

impl<W: Write> Channel<W> {
    pub fn new(output: W, log: Option<&str>) -> io::Result<Self> {
        Ok(Self {
            output,
            log: log.map(File::create).transpose()?,
        })
    }

    pub fn log_incoming(&mut self, line: &str) {
        self.log("=>", line.trim_end_matches(['\r', '\n']));
    }

    pub fn send(&mut self, message: &Value) -> io::Result<()> {
        let line = serde_json::to_string(message)?;
        self.log("<=", &line);
        writeln!(self.output, "{line}")?;
        self.output.flush()
    }

    fn log(&mut self, direction: &str, line: &str) {
        if let Some(log) = &mut self.log {
            let millis = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let _ = writeln!(log, "{millis}:{direction}:{line}");
            let _ = log.flush();
        }
    }
}
