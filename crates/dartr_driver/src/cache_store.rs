// Dart source: pkg/analyzer/lib/src/dart/analysis/file_byte_store.dart
// (FileByteStore, EvictingFileByteStore, FileByteStoreValidator)
// Design: docs/design/result-cache.md §4 (Cache Keys and Invalidation),
// §5 (On-Disk Layout, Atomic Writes, Size Limits, and Eviction)

//! Content-addressed on-disk result cache (`DiskCache`) for analysis artifacts.
//!
//! Layout under `<cache_dir>`:
//! ```text
//! <cache_dir>/
//!   v1/
//!     unlinked/
//!       <2-hex-shard>/
//!         <32-hex-key>.unlinked
//!     resolved/
//!       <2-hex-shard>/
//!         <32-hex-key>.resolved
//! ```
//!
//! Each file is framed with a 16-byte header (`b"DRTC"`, `CACHE_FORMAT_VERSION: u32`,
//! `payload_len: u64` in little-endian), the payload bytes, and an 8-byte trailer
//! holding a 64-bit SipHash-2-4 checksum over `header + payload` (keyed by the
//! cache version salt). Writes are atomic via a per-process/per-thread temporary
//! file in the same shard directory followed by `std::fs::rename`. Read errors,
//! framing mismatches, and checksum failures are treated as cache misses (and
//! unlink the corrupted file).

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_project::{DartSdk, paths};
use md5::{Digest, Md5};

use crate::api_signature::{ApiSignature, hex};

/// Magic bytes at the start of every cache file (`0..4`).
pub const CACHE_MAGIC: &[u8; 4] = b"DRTC";

/// Format version of the on-disk cache (`4..8`). Bump whenever the serialized
/// layout of cached entries changes.
pub const CACHE_FORMAT_VERSION: u32 = 1;

/// Fixed header size in bytes (`magic: 4` + `version: 4` + `payload_len: 8`).
pub const HEADER_LEN: usize = 16;

/// Fixed trailer size in bytes (`checksum: 8`).
pub const TRAILER_LEN: usize = 8;

/// Minimum valid framed file size (`HEADER_LEN + TRAILER_LEN`).
pub const MIN_FRAMED_LEN: usize = HEADER_LEN + TRAILER_LEN;

/// Default maximum disk cache size (`512 MiB`).
pub const DEFAULT_MAX_SIZE_BYTES: u64 = 512 * 1024 * 1024;

/// Age after which leftover `.tmp.*` files are deleted during cleanup (`10 minutes`).
pub const STALE_TEMP_AGE: Duration = Duration::from_secs(10 * 60);

/// A 128-bit hash / signature (`[u8; 16]`).
pub type Hash128 = [u8; 16];

/// Monotonic counter for unique `.tmp.<key>.<pid>.<counter>` filenames.
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Monotonic clock for deterministic LRU ordering within the process.
static LRU_TICK_COUNTER: AtomicU64 = AtomicU64::new(1);

/// The tier/kind of a cache entry, determining its subdirectory and file extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CacheKind {
    Unlinked,
    Resolved,
}

impl CacheKind {
    /// Subdirectory name under `<cache_dir>/v1/`.
    pub fn subdir(self) -> &'static str {
        match self {
            CacheKind::Unlinked => "unlinked",
            CacheKind::Resolved => "resolved",
        }
    }

    /// File extension (without leading dot).
    pub fn extension(self) -> &'static str {
        match self {
            CacheKind::Unlinked => "unlinked",
            CacheKind::Resolved => "resolved",
        }
    }
}

/// Computes the 128-bit MD5 content hash of UTF-8 source text after stripping a
/// leading `\u{feff}` BOM (matching `file_state.rs`).
pub fn content_hash(source: &str) -> Hash128 {
    let stripped = source.strip_prefix('\u{feff}').unwrap_or(source);
    content_hash_bytes(stripped.as_bytes())
}

/// Computes the 128-bit MD5 hash of raw bytes.
pub fn content_hash_bytes(bytes: &[u8]) -> Hash128 {
    let digest = Md5::digest(bytes);
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest);
    out
}

/// Computes a 128-bit fingerprint of the current `dartr` executable.
///
/// Hashes the binary's size, modification timestamp, and up to the first and
/// last 64 KiB of the binary so recompiling `dartr` automatically invalidates
/// cache entries from older builds. When `DARTR_CACHE_SALT` is set to a
/// non-empty string in the environment, it is mixed into the fingerprint to
/// allow tests to simulate a different `dartr` build.
pub fn executable_fingerprint() -> Hash128 {
    let base = match std::env::current_exe() {
        Ok(path) => executable_fingerprint_for_path(&path),
        Err(_) => {
            let mut sig = ApiSignature::new();
            sig.add_string("dartr-exe-unknown");
            sig.add_string(env!("CARGO_PKG_VERSION"));
            sig.to_byte_list()
        }
    };
    if let Ok(extra) = std::env::var("DARTR_CACHE_SALT")
        && !extra.is_empty()
    {
        let mut sig = ApiSignature::new();
        sig.add_bytes(&base);
        sig.add_string(&extra);
        return sig.to_byte_list();
    }
    base
}

/// Computes a 128-bit fingerprint for an executable file at [path].
pub fn executable_fingerprint_for_path(path: &Path) -> Hash128 {
    let mut sig = ApiSignature::new();
    sig.add_string(env!("CARGO_PKG_VERSION"));
    let Ok(meta) = fs::metadata(path) else {
        sig.add_string(&path.to_string_lossy());
        return sig.to_byte_list();
    };
    let len = meta.len();
    sig.add_bytes(&len.to_le_bytes());
    if let Ok(modified) = meta.modified()
        && let Ok(dur) = modified.duration_since(UNIX_EPOCH)
    {
        sig.add_bytes(&dur.as_secs().to_le_bytes());
        sig.add_int(dur.subsec_nanos());
    }
    if let Ok(mut file) = File::open(path) {
        const CHUNK: usize = 64 * 1024;
        let mut buf = vec![0u8; CHUNK.min(len as usize)];
        if file.read_exact(&mut buf).is_ok() {
            sig.add_bytes(&buf);
        }
        if len > CHUNK as u64 {
            let tail_offset = len - CHUNK as u64;
            if file.seek(SeekFrom::Start(tail_offset)).is_ok() {
                let mut tail = vec![0u8; CHUNK];
                if file.read_exact(&mut tail).is_ok() {
                    sig.add_bytes(&tail);
                }
            }
        }
    }
    sig.to_byte_list()
}

/// Computes a 128-bit fingerprint of the Dart SDK (canonical path, `version`
/// content, `libraries.dart` / `libraries.json` content, and library table).
pub fn sdk_fingerprint(sdk: Option<&DartSdk>) -> Hash128 {
    let mut sig = ApiSignature::new();
    let Some(sdk) = sdk else {
        sig.add_bool(false);
        return sig.to_byte_list();
    };
    sig.add_bool(true);
    let canonical_root = fs::canonicalize(sdk.path())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| paths::normalize(sdk.path()));
    sig.add_string(&canonical_root);
    sig.add_string(sdk.sdk_version());
    if let Some(embedder) = sdk.embedder_yaml() {
        sig.add_string(embedder);
        if let Ok(bytes) = fs::read(embedder) {
            sig.add_bytes(&content_hash_bytes(&bytes));
        }
    }
    let version_path = paths::join(sdk.path(), "version");
    if let Ok(bytes) = fs::read(&version_path) {
        sig.add_bytes(&content_hash_bytes(&bytes));
    }
    let lib_dir = sdk.lib_path();
    for rel in [
        "_internal/sdk_library_metadata/lib/libraries.dart",
        "_internal/libraries.dart",
        "libraries.json",
    ] {
        let candidate = paths::join(&lib_dir, rel);
        if let Ok(bytes) = fs::read(&candidate) {
            sig.add_string(rel);
            sig.add_bytes(&content_hash_bytes(&bytes));
            break;
        }
    }
    sig.add_int(sdk.libraries().len() as u32);
    for lib in sdk.libraries() {
        sig.add_string(&lib.short_name);
        sig.add_string(&lib.path);
        sig.add_bool(lib.documented);
        sig.add_bool(lib.implementation);
    }
    sig.to_byte_list()
}

/// Computes the global `cache_version_salt` from `CACHE_FORMAT_VERSION`, the
/// current executable fingerprint, and the Dart SDK fingerprint (design §4.1).
pub fn cache_version_salt(sdk: Option<&DartSdk>) -> Hash128 {
    let exe_fp = executable_fingerprint();
    let sdk_fp = sdk_fingerprint(sdk);
    cache_version_salt_with_parts(CACHE_FORMAT_VERSION, &exe_fp, &sdk_fp)
}

/// Computes `cache_version_salt` from explicit format version, executable
/// fingerprint, and SDK fingerprint components.
pub fn cache_version_salt_with_parts(
    format_version: u32,
    exe_fingerprint: &Hash128,
    sdk_fingerprint: &Hash128,
) -> Hash128 {
    let mut sig = ApiSignature::new();
    sig.add_int(format_version);
    sig.add_bytes(exe_fingerprint);
    sig.add_bytes(sdk_fingerprint);
    sig.to_byte_list()
}

/// Computes the Tier A `UnlinkedKey` (`Hash128`) for a single file (design §4.2).
pub fn unlinked_key(
    salt: &Hash128,
    file_content_hash: &Hash128,
    package_language_version: (u32, u32),
    experiments: &[ExperimentalFlag],
    is_dart_core: bool,
) -> Hash128 {
    let mut sig = ApiSignature::new();
    sig.add_bytes(salt);
    sig.add_bytes(file_content_hash);
    sig.add_language_version(package_language_version.0, package_language_version.1);
    let mut sorted_exps: Vec<&str> = experiments.iter().map(|e| e.name()).collect();
    sorted_exps.sort_unstable();
    sig.add_string_list(&sorted_exps);
    sig.add_bool(is_dart_core);
    sig.to_byte_list()
}

/// Input describing one compilation unit in a library for [`library_result_key`].
#[derive(Clone, Debug)]
pub struct LibraryUnitKeyInput<'a> {
    pub path: &'a str,
    pub uri_str: &'a str,
    pub content_hash: Hash128,
    pub unlinked_signature: Hash128,
    pub directives_signature: Hash128,
    pub unit_options_signature: Hash128,
    pub is_generated: bool,
}

/// Input describing analysis options, lints, and package metadata for [`library_result_key`].
#[derive(Clone, Debug, Default)]
pub struct LibraryOptionsKeyInput<'a> {
    pub strict_casts: bool,
    pub strict_inference: bool,
    pub strict_raw_types: bool,
    pub experiments: &'a [ExperimentalFlag],
    pub lint_rules: &'a [&'a str],
    pub unimplemented_lint_rules: &'a [String],
    pub unignorable_names: &'a [String],
    /// Optional precomputed signature of (`strict_*`, `experiments`,
    /// `lint_rules`, `unimplemented_lint_rules`, `unignorable_names`) from
    /// [`library_options_core_signature`].
    pub precomputed_core_signature: Option<Hash128>,
    pub pubspec_content_hash: Option<Hash128>,
    pub package_names: &'a [String],
    pub packages_signature: Hash128,
}

/// Precomputes the 128-bit signature of the analysis-options portion of
/// [`LibraryOptionsKeyInput`] so it can be computed once per
/// `analysis_options.yaml` folder in a context instead of re-sorting per
/// library.
pub fn library_options_core_signature(
    strict_casts: bool,
    strict_inference: bool,
    strict_raw_types: bool,
    experiments: &[ExperimentalFlag],
    lint_rules: &[&str],
    unimplemented_lint_rules: &[String],
    unignorable_names: &[String],
) -> Hash128 {
    let mut sig = ApiSignature::new();
    sig.add_bool(strict_casts);
    sig.add_bool(strict_inference);
    sig.add_bool(strict_raw_types);

    let mut exps: Vec<&str> = experiments.iter().map(|e| e.name()).collect();
    exps.sort_unstable();
    sig.add_string_list(&exps);

    let mut lints: Vec<&str> = lint_rules.to_vec();
    lints.sort_unstable();
    sig.add_string_list(&lints);

    let mut unimpl: Vec<&str> = unimplemented_lint_rules
        .iter()
        .map(String::as_str)
        .collect();
    unimpl.sort_unstable();
    sig.add_string_list(&unimpl);

    let mut unignorable: Vec<&str> = unignorable_names.iter().map(String::as_str).collect();
    unignorable.sort_unstable();
    sig.add_string_list(&unignorable);

    sig.to_byte_list()
}

/// Computes the Tier B `LibraryResultKey` (`Hash128`) for a defining library (design §4.4).
pub fn library_result_key(
    salt: &Hash128,
    cycle_api_signature: &Hash128,
    units: &[LibraryUnitKeyInput<'_>],
    options: &LibraryOptionsKeyInput<'_>,
) -> Hash128 {
    let mut sig = ApiSignature::new();
    sig.add_bytes(salt);
    sig.add_bytes(cycle_api_signature);
    sig.add_int(units.len() as u32);
    for u in units {
        sig.add_string(u.path);
        sig.add_string(u.uri_str);
        sig.add_bytes(&u.content_hash);
        sig.add_bytes(&u.unlinked_signature);
        sig.add_bytes(&u.directives_signature);
        sig.add_bytes(&u.unit_options_signature);
        sig.add_bool(u.is_generated);
    }
    let core_sig = options.precomputed_core_signature.unwrap_or_else(|| {
        library_options_core_signature(
            options.strict_casts,
            options.strict_inference,
            options.strict_raw_types,
            options.experiments,
            options.lint_rules,
            options.unimplemented_lint_rules,
            options.unignorable_names,
        )
    });
    sig.add_bytes(&core_sig);

    match &options.pubspec_content_hash {
        Some(hash) => {
            sig.add_bool(true);
            sig.add_bytes(hash);
        }
        None => sig.add_bool(false),
    }

    if !options.package_names.is_empty() {
        let mut pkgs: Vec<&str> = options.package_names.iter().map(String::as_str).collect();
        pkgs.sort_unstable();
        sig.add_string_list(&pkgs);
    } else {
        sig.add_int(0);
    }
    sig.add_bytes(&options.packages_signature);

    sig.to_byte_list()
}

/// Cached diagnostics and line starts for one compilation unit of a library
/// (design `docs/design/result-cache.md` §4.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedUnitDiagnostics {
    pub path: String,
    pub line_info: dartr_syntax::LineInfo,
    pub diagnostics: Vec<dartr_diagnostics::Diagnostic>,
}

/// Tier B cache value (`<key>.resolved`) for a defining library and all of its
/// compilation units (design `docs/design/result-cache.md` §4.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryAnalysisCacheEntry {
    pub units: Vec<CachedUnitDiagnostics>,
}

impl LibraryAnalysisCacheEntry {
    /// Serializes this entry into a deterministic little-endian byte vector.
    pub fn encode(&self) -> Vec<u8> {
        let mut w = ByteWriter::new();
        w.write_u32(self.units.len() as u32);
        for u in &self.units {
            w.write_str(&u.path);
            w.write_u32_slice(&u.line_info.line_starts);
            w.write_u32(u.diagnostics.len() as u32);
            for d in &u.diagnostics {
                w.write_str(d.code.unique_name);
                w.write_u64(d.offset as u64);
                w.write_u64(d.length as u64);
                w.write_str(&d.message);
                w.write_opt_str(d.correction.as_deref());
                let sev_u8 = match d.severity {
                    dartr_diagnostics::DiagnosticSeverity::None => 0u8,
                    dartr_diagnostics::DiagnosticSeverity::Info => 1u8,
                    dartr_diagnostics::DiagnosticSeverity::Warning => 2u8,
                    dartr_diagnostics::DiagnosticSeverity::Error => 3u8,
                };
                w.write_u8(sev_u8);
                w.write_u32(d.context_messages.len() as u32);
                for cm in &d.context_messages {
                    w.write_str(&cm.file_path);
                    w.write_i64(cm.offset);
                    w.write_i64(cm.length);
                    w.write_str(&cm.message);
                    w.write_opt_str(cm.url.as_deref());
                }
            }
        }
        w.into_bytes()
    }

    /// Deserializes a [`LibraryAnalysisCacheEntry`] from [bytes], returning
    /// `None` if the bytes are truncated, malformed, or reference an unknown
    /// diagnostic code.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let mut r = ByteReader::new(bytes);
        let unit_count = r.read_u32()? as usize;
        if r.remaining() < unit_count.checked_mul(4)? {
            return None;
        }
        let mut units = Vec::with_capacity(unit_count);
        for _ in 0..unit_count {
            let path = r.read_string()?;
            let line_starts = r.read_u32_vec()?;
            if line_starts.is_empty() {
                return None;
            }
            let line_info = dartr_syntax::LineInfo::new(line_starts);
            let diag_count = r.read_u32()? as usize;
            if r.remaining() < diag_count.checked_mul(4)? {
                return None;
            }
            let mut diagnostics = Vec::with_capacity(diag_count);
            for _ in 0..diag_count {
                let unique_name = r.read_string()?;
                let code = dartr_diagnostics::code_by_unique_name(&unique_name)?;
                let offset = usize::try_from(r.read_u64()?).ok()?;
                let length = usize::try_from(r.read_u64()?).ok()?;
                let message = r.read_string()?;
                let correction = r.read_opt_string()?;
                let severity = match r.read_u8()? {
                    0 => dartr_diagnostics::DiagnosticSeverity::None,
                    1 => dartr_diagnostics::DiagnosticSeverity::Info,
                    2 => dartr_diagnostics::DiagnosticSeverity::Warning,
                    3 => dartr_diagnostics::DiagnosticSeverity::Error,
                    _ => return None,
                };
                let cm_count = r.read_u32()? as usize;
                if r.remaining() < cm_count.checked_mul(4)? {
                    return None;
                }
                let mut context_messages = Vec::with_capacity(cm_count);
                for _ in 0..cm_count {
                    let file_path = r.read_string()?;
                    let cm_offset = r.read_i64()?;
                    let cm_length = r.read_i64()?;
                    let cm_message = r.read_string()?;
                    let url = r.read_opt_string()?;
                    context_messages.push(dartr_diagnostics::DiagnosticMessage {
                        file_path,
                        offset: cm_offset,
                        length: cm_length,
                        message: cm_message,
                        url,
                    });
                }
                diagnostics.push(dartr_diagnostics::Diagnostic {
                    code,
                    offset,
                    length,
                    message,
                    correction,
                    severity,
                    context_messages,
                });
            }
            units.push(CachedUnitDiagnostics {
                path,
                line_info,
                diagnostics,
            });
        }
        if !r.is_empty() {
            return None;
        }
        Some(Self { units })
    }
}

/// Reads `DARTR_CACHE_MAX_MB` from the environment, falling back to
/// [`DEFAULT_MAX_SIZE_BYTES`].
pub fn configured_max_size_bytes() -> u64 {
    std::env::var("DARTR_CACHE_MAX_MB")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|&mb| mb > 0)
        .map(|mb| mb.saturating_mul(1024 * 1024))
        .unwrap_or(DEFAULT_MAX_SIZE_BYTES)
}

/// Content-addressed disk store with 2-hex-char sharding, header + 64-bit
/// checksum validation, atomic writes, and size-bounded LRU eviction.
pub struct DiskCache {
    root: PathBuf,
    v1_root: PathBuf,
    max_size_bytes: u64,
    salt: Hash128,
    initial_sweep_done: AtomicBool,
    bytes_written_since_cleanup: AtomicU64,
    estimated_total_bytes: AtomicU64,
    cleanup_lock: Mutex<()>,
    /// Monotonic access timestamps (`(epoch_nanos, tick)`) for files read or
    /// written by this `DiskCache` instance, ensuring exact LRU ordering even
    /// on filesystems with coarse `mtime` granularity or `noatime`.
    access_times: Mutex<HashMap<PathBuf, (u128, u64)>>,
}

impl std::fmt::Debug for DiskCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiskCache")
            .field("root", &self.root)
            .field("max_size_bytes", &self.max_size_bytes)
            .field("salt", &hex(&self.salt))
            .finish()
    }
}

impl DiskCache {
    /// Creates a `DiskCache` at [cache_dir] using [`configured_max_size_bytes`]
    /// and [`cache_version_salt`] for [sdk].
    pub fn new(cache_dir: impl AsRef<Path>, sdk: Option<&DartSdk>) -> Self {
        Self::with_salt(
            cache_dir,
            configured_max_size_bytes(),
            cache_version_salt(sdk),
        )
    }

    /// Creates a `DiskCache` at [cache_dir] with an explicit [max_size_bytes]
    /// limit and [salt].
    pub fn with_salt(cache_dir: impl AsRef<Path>, max_size_bytes: u64, salt: Hash128) -> Self {
        let root = cache_dir.as_ref().to_path_buf();
        let v1_root = root.join(format!("v{CACHE_FORMAT_VERSION}"));
        Self {
            root,
            v1_root,
            max_size_bytes,
            salt,
            initial_sweep_done: AtomicBool::new(false),
            bytes_written_since_cleanup: AtomicU64::new(0),
            estimated_total_bytes: AtomicU64::new(0),
            cleanup_lock: Mutex::new(()),
            access_times: Mutex::new(HashMap::new()),
        }
    }

    /// Root directory passed when creating this cache.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Versioned root directory (`<cache_dir>/v1`).
    pub fn v1_root(&self) -> &Path {
        &self.v1_root
    }

    /// Maximum size in bytes before LRU eviction is triggered.
    pub fn max_size_bytes(&self) -> u64 {
        self.max_size_bytes
    }

    /// The `cache_version_salt` of this store.
    pub fn salt(&self) -> &Hash128 {
        &self.salt
    }

    /// Mixes [`Self::salt`] with [key] to produce the effective on-disk key.
    pub fn salted_key(&self, key: &Hash128) -> Hash128 {
        if self.salt == [0u8; 16] {
            return *key;
        }
        let mut sig = ApiSignature::new();
        sig.add_bytes(&self.salt);
        sig.add_bytes(key);
        sig.to_byte_list()
    }

    /// Returns the 32-character lowercase hex string for [key] (after mixing
    /// with [`Self::salt`]).
    pub fn salted_key_hex(&self, key: &Hash128) -> String {
        hex(&self.salted_key(key))
    }

    /// Returns the shard directory (`<cache_dir>/v1/<kind>/<2-hex-shard>`) for
    /// [kind] and [key].
    pub fn shard_dir(&self, kind: CacheKind, key: &Hash128) -> PathBuf {
        let key_hex = self.salted_key_hex(key);
        let shard = &key_hex[0..2];
        self.v1_root.join(kind.subdir()).join(shard)
    }

    /// Returns the final on-disk file path
    /// (`<cache_dir>/v1/<kind>/<2-hex-shard>/<32-hex-key>.<ext>`) for [kind]
    /// and [key].
    pub fn entry_path(&self, kind: CacheKind, key: &Hash128) -> PathBuf {
        let key_hex = self.salted_key_hex(key);
        let shard = &key_hex[0..2];
        let filename = format!("{key_hex}.{}", kind.extension());
        self.v1_root.join(kind.subdir()).join(shard).join(filename)
    }

    /// Reads and validates the cached payload for [kind] and [key].
    ///
    /// Returns `None` on missing file, I/O error, header mismatch, or checksum
    /// failure. Corrupted files are unlinked automatically.
    pub fn get(&self, kind: CacheKind, key: &Hash128) -> Option<Vec<u8>> {
        let path = self.entry_path(kind, key);
        let raw = fs::read(&path).ok()?;
        match unwrap_payload(&raw, &self.salt) {
            Some(payload) => {
                self.record_access(&path, false);
                Some(payload.to_vec())
            }
            None => {
                let _ = fs::remove_file(&path);
                if let Ok(mut map) = self.access_times.lock() {
                    map.remove(&path);
                }
                None
            }
        }
    }

    /// Reads and decodes the Tier B [`LibraryAnalysisCacheEntry`] for [key].
    /// If the payload fails to decode, the corrupted file is unlinked and `None`
    /// is returned.
    pub fn get_library_diagnostics(&self, key: &Hash128) -> Option<LibraryAnalysisCacheEntry> {
        let payload = self.get(CacheKind::Resolved, key)?;
        match LibraryAnalysisCacheEntry::decode(&payload) {
            Some(entry) => Some(entry),
            None => {
                let path = self.entry_path(CacheKind::Resolved, key);
                let _ = fs::remove_file(&path);
                if let Ok(mut map) = self.access_times.lock() {
                    map.remove(&path);
                }
                None
            }
        }
    }

    /// Encodes and atomically writes the Tier B [`LibraryAnalysisCacheEntry`]
    /// for [key].
    pub fn put_library_diagnostics(&self, key: &Hash128, entry: &LibraryAnalysisCacheEntry) {
        let payload = entry.encode();
        self.put(CacheKind::Resolved, key, &payload);
    }

    /// Atomically writes [payload] for [kind] and [key] using a temporary file
    /// in the same shard directory and `std::fs::rename`, then triggers LRU
    /// eviction if the write threshold or size limit is exceeded.
    pub fn put(&self, kind: CacheKind, key: &Hash128, payload: &[u8]) {
        if !self.initial_sweep_done.swap(true, Ordering::Relaxed) {
            self.evict_if_needed();
        }
        let key_hex = self.salted_key_hex(key);
        let shard = &key_hex[0..2];
        let shard_dir = self.v1_root.join(kind.subdir()).join(shard);
        if fs::create_dir_all(&shard_dir).is_err() {
            return;
        }
        let final_path = shard_dir.join(format!("{key_hex}.{}", kind.extension()));
        let pid = std::process::id();
        let seq = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp_path = shard_dir.join(format!(".tmp.{key_hex}.{pid}.{seq}"));

        let framed = wrap_payload(payload, &self.salt);
        let framed_len = framed.len() as u64;

        let write_ok = (|| -> std::io::Result<()> {
            let mut file = File::create(&tmp_path)?;
            file.write_all(&framed)?;
            Ok(())
        })();
        if write_ok.is_err() {
            let _ = fs::remove_file(&tmp_path);
            return;
        }
        if fs::rename(&tmp_path, &final_path).is_err() {
            let _ = fs::remove_file(&tmp_path);
            return;
        }

        self.record_access(&final_path, true);

        let written = self
            .bytes_written_since_cleanup
            .fetch_add(framed_len, Ordering::Relaxed)
            + framed_len;
        let est_total = self
            .estimated_total_bytes
            .fetch_add(framed_len, Ordering::Relaxed)
            + framed_len;
        let threshold = (self.max_size_bytes / 8).max(1);
        if written >= threshold || est_total > self.max_size_bytes {
            self.evict_if_needed();
        }
    }

    /// Sweeps `<cache_dir>/v1/{unlinked,resolved}/*/*`, deletes stale `.tmp.*`
    /// files older than [`STALE_TEMP_AGE`], and if total bytes exceed
    /// `max_size_bytes`, deletes the least-recently-used files until total
    /// size is `<= max_size_bytes * 3 / 4`.
    pub fn evict_if_needed(&self) {
        let Ok(_guard) = self.cleanup_lock.lock() else {
            return;
        };
        self.initial_sweep_done.store(true, Ordering::Relaxed);
        self.bytes_written_since_cleanup.store(0, Ordering::Relaxed);

        if !self.v1_root.exists() {
            self.estimated_total_bytes.store(0, Ordering::Relaxed);
            return;
        }

        let now = SystemTime::now();
        let mut entries: Vec<(PathBuf, u64, (u128, u64))> = Vec::new();
        let mut total_bytes: u64 = 0;

        for kind in [CacheKind::Unlinked, CacheKind::Resolved] {
            let kind_dir = self.v1_root.join(kind.subdir());
            let Ok(shards) = fs::read_dir(&kind_dir) else {
                continue;
            };
            for shard_entry in shards.flatten() {
                let shard_path = shard_entry.path();
                let Ok(files) = fs::read_dir(&shard_path) else {
                    continue;
                };
                for file_entry in files.flatten() {
                    let path = file_entry.path();
                    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                        continue;
                    };
                    let Ok(meta) = file_entry.metadata() else {
                        continue;
                    };
                    if !meta.is_file() {
                        continue;
                    }
                    if name.starts_with(".tmp.") {
                        let is_stale = meta
                            .modified()
                            .ok()
                            .and_then(|m| now.duration_since(m).ok())
                            .is_some_and(|age| age >= STALE_TEMP_AGE);
                        if is_stale {
                            let _ = fs::remove_file(&path);
                        }
                        continue;
                    }
                    let size = meta.len();
                    let disk_nanos = meta
                        .modified()
                        .or_else(|_| meta.accessed())
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_nanos())
                        .unwrap_or(0);
                    let stamp = self
                        .access_times
                        .lock()
                        .ok()
                        .and_then(|map| map.get(&path).copied())
                        .unwrap_or((disk_nanos, 0));
                    total_bytes = total_bytes.saturating_add(size);
                    entries.push((path, size, stamp));
                }
            }
        }

        if total_bytes > self.max_size_bytes {
            // Sort by access timestamp ascending (oldest first), breaking ties by path.
            entries.sort_by(|a, b| a.2.cmp(&b.2).then_with(|| a.0.cmp(&b.0)));
            let target_bytes = self.max_size_bytes * 3 / 4;
            for (path, size, _) in entries {
                if total_bytes <= target_bytes {
                    break;
                }
                if fs::remove_file(&path).is_ok() {
                    total_bytes = total_bytes.saturating_sub(size);
                    if let Ok(mut map) = self.access_times.lock() {
                        map.remove(&path);
                    }
                }
            }
        }

        self.estimated_total_bytes
            .store(total_bytes, Ordering::Relaxed);
    }

    fn record_access(&self, path: &Path, is_write: bool) {
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tick = LRU_TICK_COUNTER.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut map) = self.access_times.lock() {
            // Refresh disk mtime when an existing entry is older than 1 hour so
            // cross-process sweeps also observe recent usage without write
            // amplification on every read.
            if !is_write
                && let Some(&(prev_nanos, _)) = map.get(path)
                && now_nanos.saturating_sub(prev_nanos) > 3_600_000_000_000
                && let Ok(file) = File::options().write(true).open(path)
            {
                let _ = file.set_times(fs::FileTimes::new().set_modified(SystemTime::now()));
            }
            map.insert(path.to_path_buf(), (now_nanos, tick));
        }
    }
}

/// Frames [payload] with the 16-byte header (`CACHE_MAGIC`,
/// `CACHE_FORMAT_VERSION`, `payload.len() as u64`) and 8-byte 64-bit checksum
/// trailer.
pub fn wrap_payload(payload: &[u8], salt: &Hash128) -> Vec<u8> {
    let len = payload.len();
    let mut buf = Vec::with_capacity(MIN_FRAMED_LEN + len);
    buf.extend_from_slice(CACHE_MAGIC);
    buf.extend_from_slice(&CACHE_FORMAT_VERSION.to_le_bytes());
    buf.extend_from_slice(&(len as u64).to_le_bytes());
    buf.extend_from_slice(payload);
    let checksum = compute_checksum(salt, &buf);
    buf.extend_from_slice(&checksum.to_le_bytes());
    buf
}

/// Validates the header, length, and 64-bit checksum of [raw] and returns the
/// inner payload slice, or `None` if corrupted or mismatched.
pub fn unwrap_payload<'a>(raw: &'a [u8], salt: &Hash128) -> Option<&'a [u8]> {
    if raw.len() < MIN_FRAMED_LEN {
        return None;
    }
    if &raw[0..4] != CACHE_MAGIC {
        return None;
    }
    let version = u32::from_le_bytes(raw[4..8].try_into().ok()?);
    if version != CACHE_FORMAT_VERSION {
        return None;
    }
    let payload_len = u64::from_le_bytes(raw[8..16].try_into().ok()?) as usize;
    if raw.len() != MIN_FRAMED_LEN.checked_add(payload_len)? {
        return None;
    }
    let header_and_payload = &raw[..HEADER_LEN + payload_len];
    let expected_checksum = u64::from_le_bytes(
        raw[HEADER_LEN + payload_len..MIN_FRAMED_LEN + payload_len]
            .try_into()
            .ok()?,
    );
    let actual_checksum = compute_checksum(salt, header_and_payload);
    if actual_checksum != expected_checksum {
        return None;
    }
    Some(&raw[HEADER_LEN..HEADER_LEN + payload_len])
}

/// Deterministic 64-bit SipHash-2-4 checksum of [data] keyed by [salt].
pub fn compute_checksum(salt: &Hash128, data: &[u8]) -> u64 {
    let k0 = u64::from_le_bytes(salt[0..8].try_into().unwrap());
    let k1 = u64::from_le_bytes(salt[8..16].try_into().unwrap());
    siphash24(k0, k1, data)
}

fn siphash24(k0: u64, k1: u64, data: &[u8]) -> u64 {
    let mut v0: u64 = 0x736f6d6570736575 ^ k0;
    let mut v1: u64 = 0x646f72616e646f6d ^ k1;
    let mut v2: u64 = 0x6c7967656e657261 ^ k0;
    let mut v3: u64 = 0x7465646279746573 ^ k1;

    macro_rules! sipround {
        () => {
            v0 = v0.wrapping_add(v1);
            v1 = v1.rotate_left(13);
            v1 ^= v0;
            v0 = v0.rotate_left(32);
            v2 = v2.wrapping_add(v3);
            v3 = v3.rotate_left(16);
            v3 ^= v2;
            v0 = v0.wrapping_add(v3);
            v3 = v3.rotate_left(21);
            v3 ^= v0;
            v2 = v2.wrapping_add(v1);
            v1 = v1.rotate_left(17);
            v1 ^= v2;
            v2 = v2.rotate_left(32);
        };
    }

    let len = data.len();
    let blocks = len / 8;
    for i in 0..blocks {
        let m = u64::from_le_bytes(data[i * 8..i * 8 + 8].try_into().unwrap());
        v3 ^= m;
        sipround!();
        sipround!();
        v0 ^= m;
    }

    let mut last: u64 = (len as u64 & 0xff) << 56;
    let rem = &data[blocks * 8..];
    for (shift, &byte) in rem.iter().enumerate() {
        last |= (byte as u64) << (shift * 8);
    }
    v3 ^= last;
    sipround!();
    sipround!();
    v0 ^= last;

    v2 ^= 0xff;
    sipround!();
    sipround!();
    sipround!();
    sipround!();

    v0 ^ v1 ^ v2 ^ v3
}

/// Deterministic little-endian binary writer for cache payloads (design §5.2).
#[derive(Clone, Debug, Default)]
pub struct ByteWriter {
    buf: Vec<u8>,
}

impl ByteWriter {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    pub fn write_u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    pub fn write_bool(&mut self, v: bool) {
        self.buf.push(v as u8);
    }

    pub fn write_u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn write_i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn write_u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn write_i64(&mut self, v: i64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    pub fn write_hash128(&mut self, v: &Hash128) {
        self.buf.extend_from_slice(v);
    }

    pub fn write_raw_bytes(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.write_u32(bytes.len() as u32);
        self.buf.extend_from_slice(bytes);
    }

    pub fn write_str(&mut self, s: &str) {
        self.write_bytes(s.as_bytes());
    }

    pub fn write_opt_str(&mut self, s: Option<&str>) {
        match s {
            Some(v) => {
                self.write_bool(true);
                self.write_str(v);
            }
            None => self.write_bool(false),
        }
    }

    pub fn write_opt_u32(&mut self, v: Option<u32>) {
        match v {
            Some(val) => {
                self.write_bool(true);
                self.write_u32(val);
            }
            None => self.write_bool(false),
        }
    }

    pub fn write_u32_slice(&mut self, values: &[u32]) {
        self.write_u32(values.len() as u32);
        for &v in values {
            self.write_u32(v);
        }
    }

    pub fn write_string_slice(&mut self, values: &[String]) {
        self.write_u32(values.len() as u32);
        for s in values {
            self.write_str(s);
        }
    }
}

/// Deterministic little-endian binary reader for cache payloads (design §5.2).
#[derive(Clone, Copy, Debug)]
pub struct ByteReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> ByteReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }

    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    pub fn read_raw_bytes(&mut self, len: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(len)?;
        if end > self.bytes.len() {
            return None;
        }
        let slice = &self.bytes[self.pos..end];
        self.pos = end;
        Some(slice)
    }

    pub fn read_u8(&mut self) -> Option<u8> {
        let b = *self.bytes.get(self.pos)?;
        self.pos += 1;
        Some(b)
    }

    pub fn read_bool(&mut self) -> Option<bool> {
        match self.read_u8()? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }

    pub fn read_u32(&mut self) -> Option<u32> {
        let raw = self.read_raw_bytes(4)?;
        Some(u32::from_le_bytes(raw.try_into().ok()?))
    }

    pub fn read_i32(&mut self) -> Option<i32> {
        let raw = self.read_raw_bytes(4)?;
        Some(i32::from_le_bytes(raw.try_into().ok()?))
    }

    pub fn read_u64(&mut self) -> Option<u64> {
        let raw = self.read_raw_bytes(8)?;
        Some(u64::from_le_bytes(raw.try_into().ok()?))
    }

    pub fn read_i64(&mut self) -> Option<i64> {
        let raw = self.read_raw_bytes(8)?;
        Some(i64::from_le_bytes(raw.try_into().ok()?))
    }

    pub fn read_hash128(&mut self) -> Option<Hash128> {
        let raw = self.read_raw_bytes(16)?;
        raw.try_into().ok()
    }

    pub fn read_bytes(&mut self) -> Option<&'a [u8]> {
        let len = self.read_u32()? as usize;
        self.read_raw_bytes(len)
    }

    pub fn read_string(&mut self) -> Option<String> {
        let raw = self.read_bytes()?;
        std::str::from_utf8(raw).ok().map(str::to_owned)
    }

    pub fn read_opt_string(&mut self) -> Option<Option<String>> {
        if self.read_bool()? {
            Some(Some(self.read_string()?))
        } else {
            Some(None)
        }
    }

    pub fn read_opt_u32(&mut self) -> Option<Option<u32>> {
        if self.read_bool()? {
            Some(Some(self.read_u32()?))
        } else {
            Some(None)
        }
    }

    pub fn read_u32_vec(&mut self) -> Option<Vec<u32>> {
        let len = self.read_u32()? as usize;
        if self.remaining() < len.checked_mul(4)? {
            return None;
        }
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            out.push(self.read_u32()?);
        }
        Some(out)
    }

    pub fn read_string_vec(&mut self) -> Option<Vec<String>> {
        let len = self.read_u32()? as usize;
        if self.remaining() < len.checked_mul(4)? {
            return None;
        }
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            out.push(self.read_string()?);
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::Arc;

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(1);

    /// RAII temporary cache directory under `$TMPDIR` (`std::env::temp_dir()`)
    /// that removes itself on drop.
    struct TempCacheDir {
        path: PathBuf,
    }

    impl TempCacheDir {
        fn new(label: &str) -> Self {
            let id = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
            let pid = std::process::id();
            let path = std::env::temp_dir().join(format!("dartr_cache_test_{label}_{pid}_{id}"));
            if path.exists() {
                let _ = fs::remove_dir_all(&path);
            }
            fs::create_dir_all(&path).expect("create temp cache dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempCacheDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn collect_files_recursively(dir: &Path) -> Vec<(PathBuf, u64)> {
        let mut out = Vec::new();
        let Ok(entries) = fs::read_dir(dir) else {
            return out;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    out.extend(collect_files_recursively(&path));
                } else if meta.is_file() {
                    out.push((path, meta.len()));
                }
            }
        }
        out
    }

    #[test]
    fn round_trip_and_on_disk_layout() {
        let tmp = TempCacheDir::new("round_trip");
        let salt = [0x11u8; 16];
        let cache = DiskCache::with_salt(tmp.path(), DEFAULT_MAX_SIZE_BYTES, salt);

        let key_u = content_hash("library a;\nint x = 1;\n");
        let key_r = content_hash("resolved_library_a");
        let payload_u = b"unlinked-payload-bytes-12345";
        let payload_r = b"resolved-payload-bytes-67890";

        // Missing before write.
        assert_eq!(cache.get(CacheKind::Unlinked, &key_u), None);
        assert_eq!(cache.get(CacheKind::Resolved, &key_r), None);

        // Write both kinds and read back.
        cache.put(CacheKind::Unlinked, &key_u, payload_u);
        cache.put(CacheKind::Resolved, &key_r, payload_r);

        assert_eq!(
            cache.get(CacheKind::Unlinked, &key_u).as_deref(),
            Some(&payload_u[..])
        );
        assert_eq!(
            cache.get(CacheKind::Resolved, &key_r).as_deref(),
            Some(&payload_r[..])
        );

        // Cross-kind lookup with same key must be a miss.
        assert_eq!(cache.get(CacheKind::Resolved, &key_u), None);
        assert_eq!(cache.get(CacheKind::Unlinked, &key_r), None);

        // Verify on-disk layout: <cache_dir>/v1/<kind>/<2-hex-shard>/<32-hex-key>.<ext>
        let path_u = cache.entry_path(CacheKind::Unlinked, &key_u);
        let path_r = cache.entry_path(CacheKind::Resolved, &key_r);
        assert!(path_u.is_file(), "expected file at {}", path_u.display());
        assert!(path_r.is_file(), "expected file at {}", path_r.display());

        let hex_u = cache.salted_key_hex(&key_u);
        assert_eq!(hex_u.len(), 32);
        assert_eq!(
            path_u,
            tmp.path()
                .join("v1")
                .join("unlinked")
                .join(&hex_u[0..2])
                .join(format!("{hex_u}.unlinked"))
        );

        // Verify framing on disk: magic, version, length, payload, and 64-bit checksum.
        let raw_u = fs::read(&path_u).expect("read raw cache file");
        assert_eq!(raw_u.len(), MIN_FRAMED_LEN + payload_u.len());
        assert_eq!(&raw_u[0..4], b"DRTC");
        assert_eq!(
            u32::from_le_bytes(raw_u[4..8].try_into().unwrap()),
            CACHE_FORMAT_VERSION
        );
        assert_eq!(
            u64::from_le_bytes(raw_u[8..16].try_into().unwrap()),
            payload_u.len() as u64
        );
        assert_eq!(&raw_u[16..16 + payload_u.len()], payload_u);
        let expected_crc = compute_checksum(&salt, &raw_u[..16 + payload_u.len()]);
        let stored_crc = u64::from_le_bytes(raw_u[16 + payload_u.len()..].try_into().unwrap());
        assert_eq!(stored_crc, expected_crc);
    }

    #[test]
    fn corruption_gives_miss_and_removes_corrupted_file() {
        let tmp = TempCacheDir::new("corruption");
        let salt = [0x22u8; 16];
        let cache = DiskCache::with_salt(tmp.path(), DEFAULT_MAX_SIZE_BYTES, salt);
        let key = content_hash("corrupt_test_key");
        let payload = b"valid-payload-for-corruption-checks";

        // 1. Truncated file (< 24 bytes).
        cache.put(CacheKind::Unlinked, &key, payload);
        let path = cache.entry_path(CacheKind::Unlinked, &key);
        fs::write(&path, b"DRTCshort").unwrap();
        assert_eq!(cache.get(CacheKind::Unlinked, &key), None);
        assert!(!path.exists(), "truncated file must be unlinked on miss");

        // 2. Bad magic bytes.
        cache.put(CacheKind::Unlinked, &key, payload);
        let mut raw = fs::read(&path).unwrap();
        raw[0..4].copy_from_slice(b"BAD!");
        fs::write(&path, &raw).unwrap();
        assert_eq!(cache.get(CacheKind::Unlinked, &key), None);
        assert!(!path.exists(), "bad-magic file must be unlinked on miss");

        // 3. Wrong format version in header.
        cache.put(CacheKind::Unlinked, &key, payload);
        let mut raw = fs::read(&path).unwrap();
        raw[4..8].copy_from_slice(&(CACHE_FORMAT_VERSION + 99).to_le_bytes());
        fs::write(&path, &raw).unwrap();
        assert_eq!(cache.get(CacheKind::Unlinked, &key), None);
        assert!(
            !path.exists(),
            "wrong-version file must be unlinked on miss"
        );

        // 4. Payload length mismatch in header.
        cache.put(CacheKind::Unlinked, &key, payload);
        let mut raw = fs::read(&path).unwrap();
        raw[8..16].copy_from_slice(&(payload.len() as u64 + 5).to_le_bytes());
        fs::write(&path, &raw).unwrap();
        assert_eq!(cache.get(CacheKind::Unlinked, &key), None);
        assert!(
            !path.exists(),
            "length-mismatched file must be unlinked on miss"
        );

        // 5. Bit flip inside payload (checksum mismatch).
        cache.put(CacheKind::Unlinked, &key, payload);
        let mut raw = fs::read(&path).unwrap();
        raw[HEADER_LEN + 3] ^= 0x01;
        fs::write(&path, &raw).unwrap();
        assert_eq!(cache.get(CacheKind::Unlinked, &key), None);
        assert!(
            !path.exists(),
            "payload-corrupted file must be unlinked on miss"
        );

        // 6. Bit flip inside 64-bit checksum trailer.
        cache.put(CacheKind::Unlinked, &key, payload);
        let mut raw = fs::read(&path).unwrap();
        let last = raw.len() - 1;
        raw[last] ^= 0x80;
        fs::write(&path, &raw).unwrap();
        assert_eq!(cache.get(CacheKind::Unlinked, &key), None);
        assert!(
            !path.exists(),
            "checksum-corrupted file must be unlinked on miss"
        );

        // Re-populating after corruption works cleanly.
        cache.put(CacheKind::Unlinked, &key, payload);
        assert_eq!(
            cache.get(CacheKind::Unlinked, &key).as_deref(),
            Some(&payload[..])
        );
    }

    #[test]
    fn concurrent_writers_same_key_from_threads() {
        let tmp = TempCacheDir::new("threads");
        let salt = [0x33u8; 16];
        let cache = Arc::new(DiskCache::with_salt(
            tmp.path(),
            DEFAULT_MAX_SIZE_BYTES,
            salt,
        ));
        let key = content_hash("shared_concurrent_key");
        let payload = Arc::new(vec![0x5au8; 4096]);

        std::thread::scope(|s| {
            for _ in 0..8 {
                let cache = Arc::clone(&cache);
                let payload = Arc::clone(&payload);
                s.spawn(move || {
                    for _ in 0..60 {
                        cache.put(CacheKind::Resolved, &key, &payload);
                        if let Some(read_back) = cache.get(CacheKind::Resolved, &key) {
                            assert_eq!(read_back, *payload);
                        }
                    }
                });
            }
        });

        // Final file must exist, read back identically, and leave no `.tmp.` files.
        let final_path = cache.entry_path(CacheKind::Resolved, &key);
        assert!(final_path.is_file());
        assert_eq!(
            cache.get(CacheKind::Resolved, &key).as_deref(),
            Some(payload.as_slice())
        );
        let files = collect_files_recursively(tmp.path());
        assert_eq!(files.len(), 1);
        assert!(!files[0].0.to_string_lossy().contains(".tmp."));
    }

    #[test]
    fn concurrent_writers_same_key_from_two_processes() {
        const CHILD_ENV: &str = "DARTR_CACHE_TEST_CHILD_DIR";
        let salt = [0x44u8; 16];
        let key = content_hash("shared_multiprocess_key");
        let payload: Vec<u8> = (0..8192).map(|i| (i % 251) as u8).collect();

        if let Ok(dir) = std::env::var(CHILD_ENV) {
            let cache = DiskCache::with_salt(Path::new(&dir), DEFAULT_MAX_SIZE_BYTES, salt);
            for _ in 0..80 {
                cache.put(CacheKind::Unlinked, &key, &payload);
                if let Some(got) = cache.get(CacheKind::Unlinked, &key) {
                    assert_eq!(got, payload);
                }
            }
            return;
        }

        let tmp = TempCacheDir::new("multiprocess");
        let exe = std::env::current_exe().expect("current test executable");

        let spawn_worker = || {
            Command::new(&exe)
                .args([
                    "--exact",
                    "cache_store::tests::concurrent_writers_same_key_from_two_processes",
                    "--nocapture",
                ])
                .env(CHILD_ENV, tmp.path())
                .output()
                .expect("spawn child test process")
        };

        let (out1, out2) = std::thread::scope(|s| {
            let h1 = s.spawn(spawn_worker);
            let h2 = s.spawn(spawn_worker);
            (h1.join().unwrap(), h2.join().unwrap())
        });

        assert!(
            out1.status.success(),
            "child 1 failed: {}",
            String::from_utf8_lossy(&out1.stderr)
        );
        assert!(
            out2.status.success(),
            "child 2 failed: {}",
            String::from_utf8_lossy(&out2.stderr)
        );

        let cache = DiskCache::with_salt(tmp.path(), DEFAULT_MAX_SIZE_BYTES, salt);
        let final_path = cache.entry_path(CacheKind::Unlinked, &key);
        assert!(final_path.is_file());
        assert_eq!(
            cache.get(CacheKind::Unlinked, &key).as_deref(),
            Some(payload.as_slice())
        );

        let files = collect_files_recursively(tmp.path());
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, final_path);
    }

    #[test]
    fn eviction_keeps_size_under_limit_and_preserves_recently_used() {
        let tmp = TempCacheDir::new("eviction");
        let salt = [0x55u8; 16];
        // Each entry has 200-byte payload + 24-byte framing = 224 bytes on disk.
        // With max_size_bytes = 1000, at most 4 files (896 bytes) fit under the
        // limit, and eviction trims down to <= 750 bytes (3 files = 672 bytes).
        let max_size: u64 = 1000;
        let cache = DiskCache::with_salt(tmp.path(), max_size, salt);

        let make_key = |i: u32| content_hash(&format!("evict_key_{i}"));
        let make_payload = |i: u8| vec![i; 200];

        // Write 4 entries (4 * 224 = 896 bytes <= 1000).
        for i in 0..4u8 {
            cache.put(CacheKind::Unlinked, &make_key(i as u32), &make_payload(i));
        }

        // Touch key 0 via `get` so key 0 becomes the most recently accessed among 0..4,
        // leaving key 1 as the oldest (LRU).
        assert_eq!(
            cache.get(CacheKind::Unlinked, &make_key(0)).as_deref(),
            Some(make_payload(0).as_slice())
        );

        // Also plant a stale `.tmp.` file older than 10 minutes to verify cleanup removes it.
        let shard_dir = cache.shard_dir(CacheKind::Unlinked, &make_key(0));
        let stale_tmp = shard_dir.join(".tmp.stale.99999.1");
        {
            let f = File::create(&stale_tmp).unwrap();
            let old_time = SystemTime::now() - Duration::from_secs(3600);
            let _ = f.set_times(fs::FileTimes::new().set_modified(old_time));
        }
        assert!(stale_tmp.exists());

        // Write key 4 (5th entry -> 1120 bytes > 1000), triggering LRU eviction
        // down to <= 750 bytes (3 entries = 672 bytes).
        cache.put(CacheKind::Unlinked, &make_key(4), &make_payload(4));

        assert!(
            !stale_tmp.exists(),
            "stale .tmp. file must be deleted by cleanup"
        );

        let files_after_5 = collect_files_recursively(tmp.path());
        let total_after_5: u64 = files_after_5.iter().map(|(_, len)| *len).sum();
        assert!(
            total_after_5 <= max_size,
            "total bytes {total_after_5} must be <= max_size {max_size}"
        );
        assert!(
            total_after_5 <= max_size * 3 / 4,
            "total bytes {total_after_5} must be <= 75% target {}",
            max_size * 3 / 4
        );

        // Keys 1 and 2 were the oldest untouched entries and must have been evicted;
        // key 0 (recently read), key 3, and key 4 must still be present on disk.
        assert_eq!(cache.get(CacheKind::Unlinked, &make_key(1)), None);
        assert_eq!(cache.get(CacheKind::Unlinked, &make_key(2)), None);
        assert!(!cache.entry_path(CacheKind::Unlinked, &make_key(1)).exists());
        assert!(!cache.entry_path(CacheKind::Unlinked, &make_key(2)).exists());
        assert_eq!(
            cache.get(CacheKind::Unlinked, &make_key(0)).as_deref(),
            Some(make_payload(0).as_slice())
        );
        assert_eq!(
            cache.get(CacheKind::Unlinked, &make_key(3)).as_deref(),
            Some(make_payload(3).as_slice())
        );
        assert_eq!(
            cache.get(CacheKind::Unlinked, &make_key(4)).as_deref(),
            Some(make_payload(4).as_slice())
        );

        // Write 10 more entries and verify the on-disk size stays strictly under max_size.
        for i in 5..15u8 {
            cache.put(CacheKind::Resolved, &make_key(i as u32), &make_payload(i));
            let files = collect_files_recursively(tmp.path());
            let total: u64 = files.iter().map(|(_, len)| *len).sum();
            assert!(
                total <= max_size,
                "after writing entry {i}, total disk usage {total} exceeded limit {max_size}"
            );
        }
    }

    #[test]
    fn different_salt_gives_miss() {
        let tmp = TempCacheDir::new("salt_miss");
        let exe_fp_1 = [0xaa; 16];
        let exe_fp_2 = [0xbb; 16];
        let sdk_fp_1 = [0xcc; 16];
        let sdk_fp_2 = [0xdd; 16];

        let salt_base = cache_version_salt_with_parts(CACHE_FORMAT_VERSION, &exe_fp_1, &sdk_fp_1);
        let salt_diff_ver =
            cache_version_salt_with_parts(CACHE_FORMAT_VERSION + 1, &exe_fp_1, &sdk_fp_1);
        let salt_diff_exe =
            cache_version_salt_with_parts(CACHE_FORMAT_VERSION, &exe_fp_2, &sdk_fp_1);
        let salt_diff_sdk =
            cache_version_salt_with_parts(CACHE_FORMAT_VERSION, &exe_fp_1, &sdk_fp_2);

        assert_ne!(salt_base, salt_diff_ver);
        assert_ne!(salt_base, salt_diff_exe);
        assert_ne!(salt_base, salt_diff_sdk);

        let key = content_hash("some_file_content");
        let payload = b"cached_diagnostics_for_salt_base";

        let cache_base = DiskCache::with_salt(tmp.path(), DEFAULT_MAX_SIZE_BYTES, salt_base);
        cache_base.put(CacheKind::Unlinked, &key, payload);
        assert_eq!(
            cache_base.get(CacheKind::Unlinked, &key).as_deref(),
            Some(&payload[..])
        );

        // Every store with a different salt must miss on the same directory and key.
        for other_salt in [salt_diff_ver, salt_diff_exe, salt_diff_sdk] {
            let other_cache = DiskCache::with_salt(tmp.path(), DEFAULT_MAX_SIZE_BYTES, other_salt);
            assert_eq!(other_cache.get(CacheKind::Unlinked, &key), None);
            // Also verify that even raw payload unwrapping with a different salt fails checksum.
            let raw = fs::read(cache_base.entry_path(CacheKind::Unlinked, &key)).unwrap();
            assert_eq!(unwrap_payload(&raw, &other_salt), None);
        }

        // Also verify SDK fingerprint changes when SDK version or libraries.dart changes on disk.
        let sdk_dir_1 = TempCacheDir::new("sdk1");
        let sdk_dir_2 = TempCacheDir::new("sdk2");
        for (dir, ver) in [(sdk_dir_1.path(), "3.13.3"), (sdk_dir_2.path(), "3.14.0")] {
            let meta_dir = dir.join("lib/_internal/sdk_library_metadata/lib");
            fs::create_dir_all(&meta_dir).unwrap();
            fs::write(dir.join("version"), ver).unwrap();
            fs::write(
                meta_dir.join("libraries.dart"),
                "const Map<String, LibraryInfo> libraries = {'core': const LibraryInfo('core/core.dart')};",
            )
            .unwrap();
        }
        let sdk1 = DartSdk::new(&sdk_dir_1.path().to_string_lossy());
        let sdk2 = DartSdk::new(&sdk_dir_2.path().to_string_lossy());
        assert_ne!(sdk_fingerprint(Some(&sdk1)), sdk_fingerprint(Some(&sdk2)));
        assert_ne!(
            cache_version_salt(Some(&sdk1)),
            cache_version_salt(Some(&sdk2))
        );
    }

    #[test]
    fn byte_writer_and_reader_round_trip() {
        let mut w = ByteWriter::new();
        w.write_u8(42);
        w.write_bool(true);
        w.write_bool(false);
        w.write_u32(0xdead_beef);
        w.write_i32(-12345);
        w.write_u64(0x0123_4567_89ab_cdef);
        w.write_hash128(&[7u8; 16]);
        w.write_str("hello dartr");
        w.write_opt_str(Some("opt"));
        w.write_opt_str(None);
        w.write_opt_u32(Some(99));
        w.write_opt_u32(None);
        w.write_u32_slice(&[1, 10, 100]);
        w.write_string_slice(&["a".to_string(), "b".to_string()]);

        let bytes = w.into_bytes();
        let mut r = ByteReader::new(&bytes);
        assert_eq!(r.read_u8(), Some(42));
        assert_eq!(r.read_bool(), Some(true));
        assert_eq!(r.read_bool(), Some(false));
        assert_eq!(r.read_u32(), Some(0xdead_beef));
        assert_eq!(r.read_i32(), Some(-12345));
        assert_eq!(r.read_u64(), Some(0x0123_4567_89ab_cdef));
        assert_eq!(r.read_hash128(), Some([7u8; 16]));
        assert_eq!(r.read_string().as_deref(), Some("hello dartr"));
        assert_eq!(r.read_opt_string(), Some(Some("opt".to_string())));
        assert_eq!(r.read_opt_string(), Some(None));
        assert_eq!(r.read_opt_u32(), Some(Some(99)));
        assert_eq!(r.read_opt_u32(), Some(None));
        assert_eq!(r.read_u32_vec(), Some(vec![1, 10, 100]));
        assert_eq!(
            r.read_string_vec(),
            Some(vec!["a".to_string(), "b".to_string()])
        );
        assert!(r.is_empty());
        assert_eq!(r.read_u8(), None);
    }

    #[test]
    fn library_analysis_cache_entry_round_trip() {
        let tmp = TempCacheDir::new("lib_diag_rt");
        let salt = [0x77u8; 16];
        let cache = DiskCache::with_salt(tmp.path(), DEFAULT_MAX_SIZE_BYTES, salt);
        let key = content_hash("lib_diag_key");

        let code =
            dartr_diagnostics::code_by_unique_name("CompileTimeErrorCode.UNDEFINED_IDENTIFIER")
                .unwrap_or_else(|| dartr_diagnostics::all_codes()[0]);
        let entry = LibraryAnalysisCacheEntry {
            units: vec![
                CachedUnitDiagnostics {
                    path: "/pkg/lib/a.dart".to_string(),
                    line_info: dartr_syntax::LineInfo::new(vec![0, 15, 42]),
                    diagnostics: vec![dartr_diagnostics::Diagnostic {
                        code,
                        offset: 18,
                        length: 3,
                        message: "Undefined name 'foo'.".to_string(),
                        correction: Some("Try correcting the name.".to_string()),
                        severity: dartr_diagnostics::DiagnosticSeverity::Error,
                        context_messages: vec![dartr_diagnostics::DiagnosticMessage {
                            file_path: "/pkg/lib/a.dart".to_string(),
                            offset: 5,
                            length: 4,
                            message: "See declaration here.".to_string(),
                            url: Some(
                                "https://dart.dev/diagnostics/undefined_identifier".to_string(),
                            ),
                        }],
                    }],
                },
                CachedUnitDiagnostics {
                    path: "/pkg/lib/a_part.dart".to_string(),
                    line_info: dartr_syntax::LineInfo::new(vec![0, 20]),
                    diagnostics: Vec::new(),
                },
            ],
        };

        assert_eq!(cache.get_library_diagnostics(&key), None);
        cache.put_library_diagnostics(&key, &entry);
        assert_eq!(cache.get_library_diagnostics(&key), Some(entry));
    }
}
