// Dart source: pkg/analysis_server/lib/src/channel/byte_stream_channel.dart
// Dart source: pkg/analysis_server/lib/protocol/protocol.dart

//! One UTF-8 JSON envelope per line. Protocol logging never writes to stdout.

use serde_json::Value;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Channel<W: Write> {
    output: W,
    log: Option<File>,
}

/// Dart LineSplitter accepts LF, CRLF, CR, and a final unterminated line.
pub struct LineReader<R: BufRead> {
    input: R,
    skip_lf: bool,
}

impl<R: BufRead> LineReader<R> {
    pub fn new(input: R) -> Self {
        Self {
            input,
            skip_lf: false,
        }
    }

    pub fn next_line(&mut self) -> io::Result<Option<String>> {
        let mut line = Vec::new();
        loop {
            let buffer = self.input.fill_buf()?;
            if buffer.is_empty() {
                if line.is_empty() {
                    return Ok(None);
                }
                break;
            }
            if self.skip_lf {
                self.skip_lf = false;
                if buffer[0] == b'\n' {
                    self.input.consume(1);
                    continue;
                }
            }
            if let Some(end) = buffer.iter().position(|byte| matches!(byte, b'\r' | b'\n')) {
                line.extend_from_slice(&buffer[..end]);
                self.skip_lf = buffer[end] == b'\r';
                self.input.consume(end + 1);
                break;
            }
            let length = buffer.len();
            line.extend_from_slice(buffer);
            self.input.consume(length);
        }
        String::from_utf8(line)
            .map(Some)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }
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
