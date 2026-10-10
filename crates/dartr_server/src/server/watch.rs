// Dart source: pkg/analysis_server/lib/src/context_manager.dart (ContextManagerImpl: the watchers of the included paths, _handleWatchEvent)

//! The file watcher: Dart's analysis server watches the included paths of
//! its contexts itself (it does not register `workspace/didChangeWatchedFiles`
//! with the client). A change of a Dart file on disk that is not open is
//! analyzed again; a change of `analysis_options.yaml`, `pubspec.yaml` or
//! `package_config.json` creates the contexts again.
//!
//! The watcher thread puts each event on the message queue as the
//! notification [`WATCH_EVENT`], so that it is handled between messages
//! (Dart `WatcherMessage` on the message scheduler).

use std::path::{Path, PathBuf};

use dartr_project::FileKind;
use notify::event::EventKind;
use notify::{RecursiveMode, Watcher};
use serde_json::{Value, json};

use super::Server;

/// The internal notification of a watch event: `{path, type}` with the
/// type `add`, `modify` or `remove` (Dart `ChangeType`).
pub(super) const WATCH_EVENT: &str = "$/dartr/watchEvent";

/// The files under the watched roots and the start of the watch: raw
/// events are checked against the file system, like Dart's
/// `package:watcher` (an event of a file that did not change since the
/// start, a removal of a file that was never seen, and the replayed events
/// of FSEvents are dropped).
struct Known {
    files: std::collections::HashSet<PathBuf>,
    start: std::time::SystemTime,
}

fn list_files(path: &Path, out: &mut std::collections::HashSet<PathBuf>) {
    if path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            for e in entries.flatten() {
                list_files(&e.path(), out);
            }
        }
    } else if path.is_file() {
        out.insert(path.to_path_buf());
    }
}

/// The watch events (path, type) of the raw [paths] of an event.
fn watch_events(known: &mut Known, paths: &[PathBuf]) -> Vec<(PathBuf, &'static str)> {
    let mut out = Vec::new();
    for path in paths {
        if path.is_dir() {
            let mut now = std::collections::HashSet::new();
            list_files(path, &mut now);
            let mut added: Vec<&PathBuf> =
                now.iter().filter(|f| !known.files.contains(*f)).collect();
            added.sort();
            for f in added {
                out.push((f.clone(), "add"));
            }
            let mut removed: Vec<PathBuf> = known
                .files
                .iter()
                .filter(|f| f.starts_with(path) && !now.contains(*f))
                .cloned()
                .collect();
            removed.sort();
            for f in removed {
                out.push((f, "remove"));
            }
        } else if path.is_file() {
            if known.files.contains(path) {
                let changed = std::fs::metadata(path)
                    .and_then(|m| m.modified())
                    .map(|t| t >= known.start)
                    .unwrap_or(true);
                if changed {
                    out.push((path.clone(), "modify"));
                }
            } else {
                out.push((path.clone(), "add"));
            }
        } else {
            // Removed: the known files at or under the path.
            let mut removed: Vec<PathBuf> = known
                .files
                .iter()
                .filter(|f| f.starts_with(path))
                .cloned()
                .collect();
            removed.sort();
            for f in removed {
                out.push((f, "remove"));
            }
        }
    }
    for (p, kind) in &out {
        match *kind {
            "add" => {
                known.files.insert(p.clone());
            }
            "remove" => {
                known.files.remove(p);
            }
            _ => {}
        }
    }
    out
}

impl Server {
    /// Watches the included paths of the contexts (Dart: a watcher for each
    /// included resource of each context root), when they changed.
    pub(super) fn watch_roots(&mut self) {
        let mut roots: Vec<String> = Vec::new();
        if let Some(collection) = &self.collection {
            for context in &collection.contexts {
                for included in &context.root.included {
                    let path = included.path().to_string();
                    if !roots.contains(&path) {
                        roots.push(path);
                    }
                }
            }
        }
        if roots == self.watched && (self.watcher.is_some() || roots.is_empty()) {
            return;
        }
        self.watcher = None;
        self.watched = roots.clone();
        let Some(tx) = self.watch_tx.clone() else {
            return;
        };
        let mut known = Known {
            files: std::collections::HashSet::new(),
            start: std::time::SystemTime::now(),
        };
        for root in &roots {
            list_files(Path::new(root), &mut known.files);
        }
        // FSEvents reports real paths: map them to the watched roots.
        let real_roots: Vec<(PathBuf, PathBuf)> = roots
            .iter()
            .filter_map(|r| Some((std::fs::canonicalize(r).ok()?, PathBuf::from(r))))
            .collect();
        let watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
            let Ok(event) = result else { return };
            if matches!(event.kind, EventKind::Access(_)) {
                return;
            }
            let paths: Vec<PathBuf> = event
                .paths
                .iter()
                .map(|p| {
                    for (real, root) in &real_roots {
                        if let Ok(rest) = p.strip_prefix(real) {
                            return root.join(rest);
                        }
                    }
                    p.clone()
                })
                .collect();
            for (path, kind) in watch_events(&mut known, &paths) {
                let Some(path) = path.to_str() else { continue };
                let message = json!({
                    "jsonrpc": "2.0",
                    "method": WATCH_EVENT,
                    "params": {"path": path, "type": kind},
                });
                if tx.send(Some(message)).is_err() {
                    return;
                }
            }
        });
        let Ok(mut watcher) = watcher else { return };
        for root in &roots {
            let _ = watcher.watch(Path::new(root), RecursiveMode::Recursive);
        }
        self.watcher = Some(watcher);
    }

    /// Dart `ContextManagerImpl._handleWatchEvent`.
    pub(super) fn handle_watch_event(&mut self, params: &Value) {
        let Some(path) = params.get("path").and_then(Value::as_str) else {
            return;
        };
        let path = self.event_path(path);
        let kind = params
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("modify");
        if std::env::var_os("DARTR_DEBUG_WATCH").is_some() {
            eprintln!("WATCH {kind} {path}");
        }
        let file_kind = FileKind::of(&path);
        let is_package_config = path.ends_with("/.dart_tool/package_config.json");
        if matches!(file_kind, FileKind::AnalysisOptions | FileKind::Pubspec) || is_package_config {
            // `_createAnalysisContexts`.
            self.plugins.mark_restart_needed();
            self.roots_dirty = true;
            return;
        }
        let Some(collection) = &self.collection else {
            return;
        };
        if path.ends_with(".dart") && !self.overlays.contains_key(&path) {
            let analyzed_by_context = collection
                .contexts
                .iter()
                .any(|c| c.root.is_analyzed(&path));
            match kind {
                "add" => {
                    // `driver.addFile` for an analyzed file, else
                    // `changeFile`.
                    if analyzed_by_context && !self.is_excluded(&path) {
                        self.analyzed.insert(path.clone());
                    }
                    self.file_changed(&path);
                }
                "modify" => self.file_changed(&path),
                _ => {
                    // `driver.removeFile` and `applyFileRemoved`.
                    self.file_changed(&path);
                    self.clear_results_for_file(&path);
                    if self.analyzed.remove(&path) {
                        self.dirty.remove(&path);
                        self.publish_diagnostics(&path, Vec::new());
                    }
                }
            }
        }
        self.plugins.broadcast_watch_event(kind, &path);
        // `_checkForAndroidManifestXmlUpdate`.
        if kind != "remove"
            && file_kind == FileKind::AndroidManifest
            && self.non_dart_analyzed.contains(&path)
        {
            let files = self.non_dart_analyzed.clone();
            self.publish_non_dart_diagnostics(files);
        }
    }
}

impl Server {
    /// The path of an event in the form of the watched roots: FSEvents
    /// reports the real path (`/private/var/...` for `/var/...`).
    fn event_path(&self, path: &str) -> String {
        for root in &self.watched {
            if path.starts_with(root.as_str()) {
                return path.to_string();
            }
            if let Ok(real) = std::fs::canonicalize(root)
                && let Some(real) = real.to_str()
                && real != root
                && let Some(rest) = path.strip_prefix(real)
            {
                return format!("{root}{rest}");
            }
        }
        path.to_string()
    }
}
