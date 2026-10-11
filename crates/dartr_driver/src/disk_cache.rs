//! Persistent disk cache for SDK unlinked file summaries and linked SDK library cycles.
//!
//! Controlled by `DARTR_CACHE_DIR`:
//! - Unset: defaults to the platform user cache directory (`~/Library/Caches/dartr` on macOS,
//!   `$XDG_CACHE_HOME/dartr` or `~/.cache/dartr` on Unix, `%LOCALAPPDATA%\dartr\cache` on Windows).
//! - `"none"` (case-insensitive) or empty: disables the disk cache.
//! - Any other path: uses that directory.
//!
//! Every cache file starts with a 40-byte envelope header:
//! - 8-byte magic `b"DRTCACHE"`
//! - 4-byte format version (`FORMAT_VERSION`, little-endian)
//! - 16-byte binary fingerprint (hashes package version, format version, and executable metadata)
//! - 4-byte payload length (`u32`, little-endian)
//! - 8-byte FNV-1a 64-bit checksum of the payload (`u64`, little-endian)
//!
//! Writes are atomic (write to a per-process/counter temporary file in the same directory, then
//! `std::fs::rename`). Any corrupt, truncated, or mismatched entry returns `None` and is rebuilt.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::UNIX_EPOCH;

use dartr_ast::{NodeId, ParameterKind};
use dartr_element::*;
use dartr_link::export::{ExportEntry, ExportLocation};
use dartr_link::link::{ConstExprs, LinkedCycle, LinkedLibrary};
use dartr_link::reference::{
    BuiltInReferenceKind, LibraryReference, MemberReferenceKind, RefId, Reference, ReferenceKind,
    TopLevelReferenceKind,
};
use indexmap::{IndexMap, IndexSet};
use md5::{Digest, Md5};

use crate::api_signature::hex;
use crate::file_state::FileConfig;
use crate::unlinked_data::{
    UnlinkedCombinator, UnlinkedConfiguration, UnlinkedLibraryDirective,
    UnlinkedLibraryExportDirective, UnlinkedLibraryImportDirective, UnlinkedLibraryImportPrefix,
    UnlinkedLibraryImportPrefixName, UnlinkedPartDirective, UnlinkedPartOfNameDirective,
    UnlinkedPartOfUriDirective, UnlinkedSourceRange, UnlinkedUnit,
};

const MAGIC: &[u8; 8] = b"DRTCACHE";
const FORMAT_VERSION: u32 = 1;
const HEADER_LEN: usize = 8 + 4 + 16 + 4 + 8;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

static BINARY_FINGERPRINT: LazyLock<[u8; 16]> = LazyLock::new(|| {
    let mut hasher = Md5::new();
    hasher.update(b"dartr-disk-cache-v1");
    hasher.update(FORMAT_VERSION.to_le_bytes());
    hasher.update(env!("CARGO_PKG_VERSION").as_bytes());
    if let Ok(exe) = std::env::current_exe() {
        hasher.update(exe.to_string_lossy().as_bytes());
        if let Ok(meta) = std::fs::metadata(&exe) {
            hasher.update(meta.len().to_le_bytes());
            if let Ok(modified) = meta.modified()
                && let Ok(dur) = modified.duration_since(UNIX_EPOCH)
            {
                hasher.update(dur.as_secs().to_le_bytes());
                hasher.update(dur.subsec_nanos().to_le_bytes());
            }
        }
    }
    hasher.finalize().into()
});

thread_local! {
    static CACHE_DIR_OVERRIDE: std::cell::RefCell<Option<Option<PathBuf>>> =
        const { std::cell::RefCell::new(None) };
}

/// Overrides the disk cache directory on the current thread for the duration of `f`.
pub fn with_cache_dir_override<R>(dir: Option<PathBuf>, f: impl FnOnce() -> R) -> R {
    struct Reset(Option<Option<PathBuf>>);
    impl Drop for Reset {
        fn drop(&mut self) {
            CACHE_DIR_OVERRIDE.with(|c| {
                c.replace(self.0.take());
            });
        }
    }
    let prev = CACHE_DIR_OVERRIDE.with(|c| c.replace(Some(dir)));
    let _reset = Reset(prev);
    f()
}

/// Parses a `DARTR_CACHE_DIR` value (`"none"` or empty disables the cache).
pub fn parse_cache_dir_value(val: &str) -> Option<PathBuf> {
    let trimmed = val.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

/// Returns the active disk cache directory, or `None` if disabled (`DARTR_CACHE_DIR=none`).
pub fn cache_dir() -> Option<PathBuf> {
    if let Some(overridden) = CACHE_DIR_OVERRIDE.with(|c| c.borrow().clone()) {
        return overridden;
    }
    if let Ok(val) = std::env::var("DARTR_CACHE_DIR") {
        return parse_cache_dir_value(&val);
    }
    #[cfg(test)]
    {
        None
    }
    #[cfg(not(test))]
    {
        default_cache_dir()
    }
}

#[allow(dead_code)]
fn default_cache_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME")
            && !home.is_empty()
        {
            return Some(PathBuf::from(home).join("Library/Caches/dartr"));
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME")
            && !xdg.is_empty()
        {
            return Some(PathBuf::from(xdg).join("dartr"));
        }
        if let Some(home) = std::env::var_os("HOME")
            && !home.is_empty()
        {
            return Some(PathBuf::from(home).join(".cache/dartr"));
        }
    }
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA")
            && !local.is_empty()
        {
            return Some(PathBuf::from(local).join("dartr\\cache"));
        }
    }
    Some(std::env::temp_dir().join("dartr-cache"))
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn write_envelope(dir: &Path, filename: &str, payload: &[u8]) {
    if payload.len() > u32::MAX as usize {
        return;
    }
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let pid = std::process::id();
    let seq = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_name = format!(".tmp.{filename}.{pid}.{seq}");
    let tmp_path = dir.join(tmp_name);
    let final_path = dir.join(filename);

    let mut buf = Vec::with_capacity(HEADER_LEN + payload.len());
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    buf.extend_from_slice(&*BINARY_FINGERPRINT);
    buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    buf.extend_from_slice(&fnv1a_64(payload).to_le_bytes());
    buf.extend_from_slice(payload);

    if std::fs::write(&tmp_path, &buf).is_ok() {
        if std::fs::rename(&tmp_path, &final_path).is_err() {
            let _ = std::fs::remove_file(&tmp_path);
        }
    } else {
        let _ = std::fs::remove_file(&tmp_path);
    }
}

fn read_envelope(dir: &Path, filename: &str) -> Option<Vec<u8>> {
    let path = dir.join(filename);
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() < HEADER_LEN {
        return None;
    }
    if &bytes[0..8] != MAGIC {
        return None;
    }
    let version = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
    if version != FORMAT_VERSION {
        return None;
    }
    if bytes[12..28] != *BINARY_FINGERPRINT {
        return None;
    }
    let payload_len = u32::from_le_bytes(bytes[28..32].try_into().ok()?) as usize;
    let expected_checksum = u64::from_le_bytes(bytes[32..40].try_into().ok()?);
    let payload = &bytes[HEADER_LEN..];
    if payload.len() != payload_len {
        return None;
    }
    if fnv1a_64(payload) != expected_checksum {
        return None;
    }
    Some(payload.to_vec())
}

// ----------------------------------------------------------------------------
// Primitive binary Writer / Reader
// ----------------------------------------------------------------------------

struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    fn with_capacity(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap),
        }
    }

    #[inline]
    fn write_u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    #[inline]
    fn write_bool(&mut self, v: bool) {
        self.buf.push(u8::from(v));
    }

    #[inline]
    fn write_u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    #[inline]
    fn write_u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    #[inline]
    fn write_i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    #[inline]
    fn write_u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    #[inline]
    fn write_bytes(&mut self, bytes: &[u8]) {
        self.write_u32(bytes.len() as u32);
        self.buf.extend_from_slice(bytes);
    }

    #[inline]
    fn write_fixed_16(&mut self, bytes: &[u8; 16]) {
        self.buf.extend_from_slice(bytes);
    }

    #[inline]
    fn write_str(&mut self, s: &str) {
        self.write_bytes(s.as_bytes());
    }

    #[inline]
    fn write_opt_str(&mut self, s: Option<&str>) {
        match s {
            Some(s) => {
                self.write_u8(1);
                self.write_str(s);
            }
            None => self.write_u8(0),
        }
    }

    #[inline]
    fn write_opt_u32(&mut self, v: Option<u32>) {
        match v {
            Some(x) => {
                self.write_u8(1);
                self.write_u32(x);
            }
            None => self.write_u8(0),
        }
    }
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    #[inline]
    fn read_exact(&mut self, len: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(len)?;
        if end > self.buf.len() {
            return None;
        }
        let slice = &self.buf[self.pos..end];
        self.pos = end;
        Some(slice)
    }

    #[inline]
    fn read_u8(&mut self) -> Option<u8> {
        Some(self.read_exact(1)?[0])
    }

    #[inline]
    fn read_bool(&mut self) -> Option<bool> {
        match self.read_u8()? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }

    #[inline]
    fn read_u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.read_exact(2)?.try_into().ok()?))
    }

    #[inline]
    fn read_u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.read_exact(4)?.try_into().ok()?))
    }

    #[inline]
    fn read_i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.read_exact(4)?.try_into().ok()?))
    }

    #[inline]
    fn read_u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.read_exact(8)?.try_into().ok()?))
    }

    #[inline]
    fn read_bytes(&mut self) -> Option<&'a [u8]> {
        let len = self.read_u32()? as usize;
        self.read_exact(len)
    }

    #[inline]
    fn read_fixed_16(&mut self) -> Option<[u8; 16]> {
        self.read_exact(16)?.try_into().ok()
    }

    #[inline]
    fn read_str(&mut self) -> Option<&'a str> {
        std::str::from_utf8(self.read_bytes()?).ok()
    }

    #[inline]
    fn read_string(&mut self) -> Option<String> {
        Some(self.read_str()?.to_string())
    }

    #[inline]
    fn read_arc_str(&mut self) -> Option<Arc<str>> {
        Some(Arc::from(self.read_str()?))
    }

    #[inline]
    fn read_opt_string(&mut self) -> Option<Option<String>> {
        match self.read_u8()? {
            0 => Some(None),
            1 => Some(Some(self.read_string()?)),
            _ => None,
        }
    }

    #[inline]
    fn read_opt_arc_str(&mut self) -> Option<Option<Arc<str>>> {
        match self.read_u8()? {
            0 => Some(None),
            1 => Some(Some(self.read_arc_str()?)),
            _ => None,
        }
    }

    #[inline]
    fn read_opt_u32(&mut self) -> Option<Option<u32>> {
        match self.read_u8()? {
            0 => Some(None),
            1 => Some(Some(self.read_u32()?)),
            _ => None,
        }
    }

    #[inline]
    fn check_len(&mut self, min_elem_bytes: usize) -> Option<usize> {
        let count = self.read_u32()? as usize;
        let needed = count.checked_mul(min_elem_bytes)?;
        if self.pos.checked_add(needed)? > self.buf.len() {
            return None;
        }
        Some(count)
    }
}

// ----------------------------------------------------------------------------
// Unlinked SDK File Summary Bundle
// ----------------------------------------------------------------------------

/// Cached unlinked summary of one SDK file on disk.
#[derive(Clone)]
pub struct CachedSdkFileSummary {
    pub path: Arc<str>,
    pub is_dart_core: bool,
    pub mtime_secs: u64,
    pub mtime_nanos: u32,
    pub file_size: u64,
    pub exists: bool,
    pub content_hash: String,
    pub unlinked_signature: [u8; 16],
    pub unlinked: UnlinkedUnit,
}

/// Returns `(mtime_secs, mtime_nanos, file_size)` for a file on disk.
pub fn file_stamp(path: &str) -> Option<(u64, u32, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some((modified.as_secs(), modified.subsec_nanos(), meta.len()))
}

/// Computes the bundle filename for a `(config, salt)` pair.
pub fn unlinked_bundle_filename(config: &FileConfig, salt: &[u32]) -> String {
    let mut hasher = Md5::new();
    hasher.update(b"unlinked-bundle-v1");
    hasher.update(BINARY_FINGERPRINT.as_slice());
    hasher.update(config.package_language_version.0.to_le_bytes());
    hasher.update(config.package_language_version.1.to_le_bytes());
    hasher.update((config.experiments.len() as u32).to_le_bytes());
    for exp in &config.experiments {
        let s = exp.name();
        hasher.update((s.len() as u32).to_le_bytes());
        hasher.update(s.as_bytes());
    }
    hasher.update((salt.len() as u32).to_le_bytes());
    for &v in salt {
        hasher.update(v.to_le_bytes());
    }
    format!("unlinked_{}.bin", hex(&hasher.finalize()))
}

pub fn load_unlinked_bundle(
    dir: &Path,
    config: &FileConfig,
    salt: &[u32],
) -> Option<HashMap<Arc<str>, CachedSdkFileSummary>> {
    let filename = unlinked_bundle_filename(config, salt);
    let payload = read_envelope(dir, &filename)?;
    let mut r = Reader::new(&payload);
    let major = r.read_u32()?;
    let minor = r.read_u32()?;
    if (major, minor) != config.package_language_version {
        return None;
    }
    let exp_len = r.check_len(1)?;
    if exp_len != config.experiments.len() {
        return None;
    }
    for exp in &config.experiments {
        if r.read_str()? != exp.name() {
            return None;
        }
    }
    let salt_len = r.check_len(4)?;
    if salt_len != salt.len() {
        return None;
    }
    for &expected in salt {
        if r.read_u32()? != expected {
            return None;
        }
    }
    let count = r.check_len(24)?;
    let mut map = HashMap::with_capacity(count);
    for _ in 0..count {
        let entry = read_cached_sdk_file(&mut r)?;
        map.insert(entry.path.clone(), entry);
    }
    if r.pos != r.buf.len() {
        return None;
    }
    Some(map)
}

pub fn save_unlinked_bundle(
    dir: &Path,
    config: &FileConfig,
    salt: &[u32],
    entries: &HashMap<Arc<str>, CachedSdkFileSummary>,
) {
    let filename = unlinked_bundle_filename(config, salt);
    let mut w = Writer::with_capacity(entries.len() * 1024 + 128);
    w.write_u32(config.package_language_version.0);
    w.write_u32(config.package_language_version.1);
    w.write_u32(config.experiments.len() as u32);
    for exp in &config.experiments {
        w.write_str(exp.name());
    }
    w.write_u32(salt.len() as u32);
    for &v in salt {
        w.write_u32(v);
    }
    let mut sorted: Vec<&CachedSdkFileSummary> = entries.values().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    w.write_u32(sorted.len() as u32);
    for entry in sorted {
        write_cached_sdk_file(&mut w, entry);
    }
    write_envelope(dir, &filename, &w.buf);
}

fn write_cached_sdk_file(w: &mut Writer, e: &CachedSdkFileSummary) {
    w.write_str(&e.path);
    w.write_bool(e.is_dart_core);
    w.write_u64(e.mtime_secs);
    w.write_u32(e.mtime_nanos);
    w.write_u64(e.file_size);
    w.write_bool(e.exists);
    w.write_str(&e.content_hash);
    w.write_fixed_16(&e.unlinked_signature);
    write_unlinked_unit(w, &e.unlinked);
}

fn read_cached_sdk_file(r: &mut Reader<'_>) -> Option<CachedSdkFileSummary> {
    let path = r.read_arc_str()?;
    let is_dart_core = r.read_bool()?;
    let mtime_secs = r.read_u64()?;
    let mtime_nanos = r.read_u32()?;
    let file_size = r.read_u64()?;
    let exists = r.read_bool()?;
    let content_hash = r.read_string()?;
    let unlinked_signature = r.read_fixed_16()?;
    let unlinked = read_unlinked_unit(r)?;
    Some(CachedSdkFileSummary {
        path,
        is_dart_core,
        mtime_secs,
        mtime_nanos,
        file_size,
        exists,
        content_hash,
        unlinked_signature,
        unlinked,
    })
}

fn write_unlinked_combinator(w: &mut Writer, c: &UnlinkedCombinator) {
    w.write_u32(c.keyword_offset);
    w.write_u32(c.end_offset);
    w.write_bool(c.is_show);
    w.write_u32(c.names.len() as u32);
    for name in &c.names {
        w.write_str(name);
    }
}

fn read_unlinked_combinator(r: &mut Reader<'_>) -> Option<UnlinkedCombinator> {
    let keyword_offset = r.read_u32()?;
    let end_offset = r.read_u32()?;
    let is_show = r.read_bool()?;
    let len = r.check_len(4)?;
    let mut names = Vec::with_capacity(len);
    for _ in 0..len {
        names.push(r.read_string()?);
    }
    Some(UnlinkedCombinator {
        keyword_offset,
        end_offset,
        is_show,
        names,
    })
}

fn write_unlinked_configuration(w: &mut Writer, c: &UnlinkedConfiguration) {
    w.write_str(&c.name);
    w.write_str(&c.value);
    w.write_opt_str(c.uri.as_deref());
}

fn read_unlinked_configuration(r: &mut Reader<'_>) -> Option<UnlinkedConfiguration> {
    let name = r.read_string()?;
    let value = r.read_string()?;
    let uri = r.read_opt_string()?;
    Some(UnlinkedConfiguration { name, value, uri })
}

fn write_unlinked_import(w: &mut Writer, d: &UnlinkedLibraryImportDirective) {
    w.write_u32(d.combinators.len() as u32);
    for c in &d.combinators {
        write_unlinked_combinator(w, c);
    }
    w.write_u32(d.configurations.len() as u32);
    for c in &d.configurations {
        write_unlinked_configuration(w, c);
    }
    w.write_i32(d.import_keyword_offset);
    w.write_bool(d.is_doc_import);
    w.write_bool(d.is_synthetic_dart_core);
    match &d.prefix {
        None => w.write_u8(0),
        Some(p) => {
            w.write_u8(1);
            w.write_opt_u32(p.deferred_offset);
            w.write_u32(p.as_offset);
            w.write_u32(p.name_offset);
            match &p.name {
                None => w.write_u8(0),
                Some(n) => {
                    w.write_u8(1);
                    w.write_str(&n.name);
                    w.write_u32(n.name_offset);
                }
            }
        }
    }
    w.write_opt_str(d.uri.as_deref());
}

fn read_unlinked_import(r: &mut Reader<'_>) -> Option<UnlinkedLibraryImportDirective> {
    let comb_len = r.check_len(9)?;
    let mut combinators = Vec::with_capacity(comb_len);
    for _ in 0..comb_len {
        combinators.push(read_unlinked_combinator(r)?);
    }
    let conf_len = r.check_len(9)?;
    let mut configurations = Vec::with_capacity(conf_len);
    for _ in 0..conf_len {
        configurations.push(read_unlinked_configuration(r)?);
    }
    let import_keyword_offset = r.read_i32()?;
    let is_doc_import = r.read_bool()?;
    let is_synthetic_dart_core = r.read_bool()?;
    let prefix = match r.read_u8()? {
        0 => None,
        1 => {
            let deferred_offset = r.read_opt_u32()?;
            let as_offset = r.read_u32()?;
            let name_offset = r.read_u32()?;
            let name = match r.read_u8()? {
                0 => None,
                1 => Some(UnlinkedLibraryImportPrefixName {
                    name: r.read_string()?,
                    name_offset: r.read_u32()?,
                }),
                _ => return None,
            };
            Some(UnlinkedLibraryImportPrefix {
                deferred_offset,
                as_offset,
                name_offset,
                name,
            })
        }
        _ => return None,
    };
    let uri = r.read_opt_string()?;
    Some(UnlinkedLibraryImportDirective {
        combinators,
        configurations,
        import_keyword_offset,
        is_doc_import,
        is_synthetic_dart_core,
        prefix,
        uri,
    })
}

fn write_unlinked_export(w: &mut Writer, d: &UnlinkedLibraryExportDirective) {
    w.write_u32(d.combinators.len() as u32);
    for c in &d.combinators {
        write_unlinked_combinator(w, c);
    }
    w.write_u32(d.configurations.len() as u32);
    for c in &d.configurations {
        write_unlinked_configuration(w, c);
    }
    w.write_u32(d.export_keyword_offset);
    w.write_opt_str(d.uri.as_deref());
}

fn read_unlinked_export(r: &mut Reader<'_>) -> Option<UnlinkedLibraryExportDirective> {
    let comb_len = r.check_len(9)?;
    let mut combinators = Vec::with_capacity(comb_len);
    for _ in 0..comb_len {
        combinators.push(read_unlinked_combinator(r)?);
    }
    let conf_len = r.check_len(9)?;
    let mut configurations = Vec::with_capacity(conf_len);
    for _ in 0..conf_len {
        configurations.push(read_unlinked_configuration(r)?);
    }
    let export_keyword_offset = r.read_u32()?;
    let uri = r.read_opt_string()?;
    Some(UnlinkedLibraryExportDirective {
        combinators,
        configurations,
        export_keyword_offset,
        uri,
    })
}

fn write_unlinked_part(w: &mut Writer, d: &UnlinkedPartDirective) {
    w.write_u32(d.configurations.len() as u32);
    for c in &d.configurations {
        write_unlinked_configuration(w, c);
    }
    w.write_u32(d.part_keyword_offset);
    w.write_opt_str(d.uri.as_deref());
}

fn read_unlinked_part(r: &mut Reader<'_>) -> Option<UnlinkedPartDirective> {
    let conf_len = r.check_len(9)?;
    let mut configurations = Vec::with_capacity(conf_len);
    for _ in 0..conf_len {
        configurations.push(read_unlinked_configuration(r)?);
    }
    let part_keyword_offset = r.read_u32()?;
    let uri = r.read_opt_string()?;
    Some(UnlinkedPartDirective {
        configurations,
        part_keyword_offset,
        uri,
    })
}

fn write_unlinked_unit(w: &mut Writer, u: &UnlinkedUnit) {
    w.write_fixed_16(&u.api_signature);
    w.write_u32(u.exports.len() as u32);
    for e in &u.exports {
        write_unlinked_export(w, e);
    }
    w.write_bool(u.has_dart_core_import);
    w.write_u32(u.imports.len() as u32);
    for i in &u.imports {
        write_unlinked_import(w, i);
    }
    w.write_bool(u.is_dart_core);
    match &u.library_directive {
        None => w.write_u8(0),
        Some(l) => {
            w.write_u8(1);
            w.write_u32(l.doc_imports.len() as u32);
            for di in &l.doc_imports {
                write_unlinked_import(w, di);
            }
            w.write_opt_str(l.name.as_deref());
        }
    }
    w.write_u32(u.line_starts.len() as u32);
    for &ls in &u.line_starts {
        w.write_u32(ls);
    }
    w.write_u32(u.parts.len() as u32);
    for p in &u.parts {
        write_unlinked_part(w, p);
    }
    match &u.part_of_name_directive {
        None => w.write_u8(0),
        Some(p) => {
            w.write_u8(1);
            w.write_u32(p.doc_imports.len() as u32);
            for di in &p.doc_imports {
                write_unlinked_import(w, di);
            }
            w.write_str(&p.name);
            w.write_u32(p.name_range.offset);
            w.write_u32(p.name_range.length);
        }
    }
    match &u.part_of_uri_directive {
        None => w.write_u8(0),
        Some(p) => {
            w.write_u8(1);
            w.write_u32(p.doc_imports.len() as u32);
            for di in &p.doc_imports {
                write_unlinked_import(w, di);
            }
            w.write_opt_str(p.uri.as_deref());
            w.write_u32(p.uri_range.offset);
            w.write_u32(p.uri_range.length);
        }
    }
    w.write_u32(u.top_level_declarations.len() as u32);
    for decl in &u.top_level_declarations {
        w.write_str(decl);
    }
}

fn read_unlinked_unit(r: &mut Reader<'_>) -> Option<UnlinkedUnit> {
    let api_signature = r.read_fixed_16()?;
    let exp_len = r.check_len(9)?;
    let mut exports = Vec::with_capacity(exp_len);
    for _ in 0..exp_len {
        exports.push(read_unlinked_export(r)?);
    }
    let has_dart_core_import = r.read_bool()?;
    let imp_len = r.check_len(11)?;
    let mut imports = Vec::with_capacity(imp_len);
    for _ in 0..imp_len {
        imports.push(read_unlinked_import(r)?);
    }
    let is_dart_core = r.read_bool()?;
    let library_directive = match r.read_u8()? {
        0 => None,
        1 => {
            let di_len = r.check_len(11)?;
            let mut doc_imports = Vec::with_capacity(di_len);
            for _ in 0..di_len {
                doc_imports.push(read_unlinked_import(r)?);
            }
            let name = r.read_opt_string()?;
            Some(UnlinkedLibraryDirective { doc_imports, name })
        }
        _ => return None,
    };
    let ls_len = r.check_len(4)?;
    let mut line_starts = Vec::with_capacity(ls_len);
    for _ in 0..ls_len {
        line_starts.push(r.read_u32()?);
    }
    let parts_len = r.check_len(9)?;
    let mut parts = Vec::with_capacity(parts_len);
    for _ in 0..parts_len {
        parts.push(read_unlinked_part(r)?);
    }
    let part_of_name_directive = match r.read_u8()? {
        0 => None,
        1 => {
            let di_len = r.check_len(11)?;
            let mut doc_imports = Vec::with_capacity(di_len);
            for _ in 0..di_len {
                doc_imports.push(read_unlinked_import(r)?);
            }
            let name = r.read_string()?;
            let offset = r.read_u32()?;
            let length = r.read_u32()?;
            Some(UnlinkedPartOfNameDirective {
                doc_imports,
                name,
                name_range: UnlinkedSourceRange { offset, length },
            })
        }
        _ => return None,
    };
    let part_of_uri_directive = match r.read_u8()? {
        0 => None,
        1 => {
            let di_len = r.check_len(11)?;
            let mut doc_imports = Vec::with_capacity(di_len);
            for _ in 0..di_len {
                doc_imports.push(read_unlinked_import(r)?);
            }
            let uri = r.read_opt_string()?;
            let offset = r.read_u32()?;
            let length = r.read_u32()?;
            Some(UnlinkedPartOfUriDirective {
                doc_imports,
                uri,
                uri_range: UnlinkedSourceRange { offset, length },
            })
        }
        _ => return None,
    };
    let decl_len = r.check_len(4)?;
    let mut top_level_declarations = IndexSet::with_capacity(decl_len);
    for _ in 0..decl_len {
        top_level_declarations.insert(r.read_string()?);
    }
    Some(UnlinkedUnit {
        api_signature,
        exports,
        has_dart_core_import,
        imports,
        is_dart_core,
        library_directive,
        line_starts,
        parts,
        part_of_name_directive,
        part_of_uri_directive,
        top_level_declarations,
    })
}

// ----------------------------------------------------------------------------
// Linked SDK Cycle Serialization / Deserialization
// ----------------------------------------------------------------------------

/// Disk cache key for an SDK `LinkedCycle`.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct DiskCycleKey {
    pub sdk_path: String,
    pub api_signature: String,
    pub manifest_signature: String,
    pub non_transitive_api_signature: String,
    pub unlinked_signatures: Vec<[u8; 16]>,
}

impl DiskCycleKey {
    pub fn filename(&self) -> String {
        let mut hasher = Md5::new();
        hasher.update(b"linked-cycle-v1");
        hasher.update(BINARY_FINGERPRINT.as_slice());
        hasher.update((self.sdk_path.len() as u32).to_le_bytes());
        hasher.update(self.sdk_path.as_bytes());
        hasher.update((self.api_signature.len() as u32).to_le_bytes());
        hasher.update(self.api_signature.as_bytes());
        hasher.update((self.manifest_signature.len() as u32).to_le_bytes());
        hasher.update(self.manifest_signature.as_bytes());
        hasher.update((self.non_transitive_api_signature.len() as u32).to_le_bytes());
        hasher.update(self.non_transitive_api_signature.as_bytes());
        hasher.update((self.unlinked_signatures.len() as u32).to_le_bytes());
        for sig in &self.unlinked_signatures {
            hasher.update(sig);
        }
        format!("cycle_{}.bin", hex(&hasher.finalize()))
    }
}

pub fn save_linked_cycle(
    dir: &Path,
    key: &DiskCycleKey,
    world: &WorldSnapshot,
    cycle: &LinkedCycle,
) {
    let mut enc = CycleEncoder::new(world, cycle.store.id);
    let mut body = Writer::with_capacity(64 * 1024);
    enc.write_cycle_body(&mut body, cycle);

    let mut w = Writer::with_capacity(body.buf.len() + 4096);
    w.write_str(&key.sdk_path);
    w.write_str(&key.api_signature);
    w.write_str(&key.manifest_signature);
    w.write_str(&key.non_transitive_api_signature);
    w.write_u32(key.unlinked_signatures.len() as u32);
    for sig in &key.unlinked_signatures {
        w.write_fixed_16(sig);
    }
    enc.write_tables(&mut w);
    w.write_bytes(&body.buf);

    let filename = key.filename();
    write_envelope(dir, &filename, &w.buf);
}

pub fn load_linked_cycle(
    dir: &Path,
    key: &DiskCycleKey,
    world: &WorldSnapshot,
) -> Option<LinkedCycle> {
    let filename = key.filename();
    let payload = read_envelope(dir, &filename)?;
    let mut r = Reader::new(&payload);
    if r.read_str()? != key.sdk_path {
        return None;
    }
    if r.read_str()? != key.api_signature {
        return None;
    }
    if r.read_str()? != key.manifest_signature {
        return None;
    }
    if r.read_str()? != key.non_transitive_api_signature {
        return None;
    }
    let sig_len = r.check_len(16)?;
    if sig_len != key.unlinked_signatures.len() {
        return None;
    }
    for expected in &key.unlinked_signatures {
        if &r.read_fixed_16()? != expected {
            return None;
        }
    }
    let new_store = world.generation.new_cycle_store();
    let dec = CycleDecoder::read_tables(&mut r, world, new_store.id)?;
    let body_bytes = r.read_bytes()?;
    if r.pos != r.buf.len() {
        return None;
    }
    let mut body_r = Reader::new(body_bytes);
    let cycle = dec.read_cycle_body(&mut body_r, new_store)?;
    if body_r.pos != body_r.buf.len() {
        return None;
    }
    Some(cycle)
}

#[derive(Clone)]
struct SerializedAlias {
    element: ElementId,
    args: Vec<u32>,
    nullability: Nullability,
}

#[derive(Clone)]
enum SerializedElemRef {
    Base(ElementId),
    Member {
        base: ElementId,
        subst: Vec<(ElementId, u32)>,
    },
}

#[derive(Clone)]
struct SerializedFnParam {
    name: Option<u32>,
    kind: ParameterKind,
    ty: u32,
    covariant: bool,
    element: Option<SerializedElemRef>,
}

#[derive(Clone)]
enum SerializedType {
    Dynamic,
    Void,
    Invalid,
    Unknown,
    Never(Nullability),
    Interface {
        element: ElementId,
        args: Vec<u32>,
        nullability: Nullability,
        alias: Option<SerializedAlias>,
    },
    Function {
        type_params: Vec<ElementId>,
        params: Vec<SerializedFnParam>,
        required_positional: u16,
        ret: u32,
        nullability: Nullability,
        alias: Option<SerializedAlias>,
    },
    Record {
        positional: Vec<u32>,
        named: Vec<(u32, u32)>,
        nullability: Nullability,
        alias: Option<SerializedAlias>,
    },
    TypeParameter {
        param: ElementId,
        nullability: Nullability,
        promoted_bound: Option<u32>,
        alias: Option<SerializedAlias>,
    },
}

struct CycleEncoder<'a> {
    world: &'a WorldSnapshot,
    cycle_store_id: StoreId,
    external_stores: Vec<Arc<str>>,
    store_map: HashMap<StoreId, u16>,
    names: Vec<&'a str>,
    name_map: HashMap<Name, u32>,
    types: Vec<SerializedType>,
    type_map: HashMap<TypeId, u32>,
}

impl<'a> CycleEncoder<'a> {
    fn new(world: &'a WorldSnapshot, cycle_store_id: StoreId) -> Self {
        let mut store_map = HashMap::new();
        store_map.insert(StoreId::SYNTHETIC, 0);
        store_map.insert(cycle_store_id, 1);

        let mut types = Vec::with_capacity(256);
        let mut type_map = HashMap::with_capacity(256);
        let fixed = [
            (TypeId::DYNAMIC, SerializedType::Dynamic),
            (TypeId::VOID, SerializedType::Void),
            (TypeId::INVALID, SerializedType::Invalid),
            (TypeId::UNKNOWN, SerializedType::Unknown),
            (TypeId::NEVER, SerializedType::Never(Nullability::None)),
            (
                TypeId::NEVER_QUESTION,
                SerializedType::Never(Nullability::Question),
            ),
        ];
        for (i, (id, st)) in fixed.into_iter().enumerate() {
            types.push(st);
            type_map.insert(id, i as u32);
        }

        Self {
            world,
            cycle_store_id,
            external_stores: Vec::new(),
            store_map,
            names: Vec::new(),
            name_map: HashMap::new(),
            types,
            type_map,
        }
    }

    fn map_store(&mut self, store: StoreId) -> u16 {
        if let Some(&idx) = self.store_map.get(&store) {
            return idx;
        }
        let ext_store = self
            .world
            .store(store)
            .expect("external dependency store must exist in world");
        let lib0 = ext_store.elements.libraries.get(0);
        let uri = ext_store
            .fragments
            .units
            .get(lib0.first_fragment().index())
            .source
            .uri
            .clone();
        let idx = (2 + self.external_stores.len()) as u16;
        self.external_stores.push(uri);
        self.store_map.insert(store, idx);
        idx
    }

    fn map_name(&mut self, name: Name) -> u32 {
        if let Some(&idx) = self.name_map.get(&name) {
            return idx;
        }
        let s = self.world.generation.names.get(name);
        let idx = self.names.len() as u32;
        self.names.push(s);
        self.name_map.insert(name, idx);
        idx
    }

    fn map_alias(&mut self, alias: Option<AliasId>) -> Option<SerializedAlias> {
        let alias_id = alias?;
        let a = self.world.generation.interner.alias(alias_id);
        let _ = self.map_store(a.element.store());
        let args = self
            .world
            .generation
            .interner
            .list(a.args)
            .iter()
            .map(|&t| self.map_type(t))
            .collect();
        Some(SerializedAlias {
            element: a.element.raw(),
            args,
            nullability: a.nullability,
        })
    }

    fn map_elem_ref(&mut self, r: ElemRef) -> SerializedElemRef {
        match r {
            ElemRef::Base(e) => {
                let _ = self.map_store(e.store());
                SerializedElemRef::Base(e)
            }
            ElemRef::Member(m) => {
                let member = self.world.generation.interner.member(m);
                let _ = self.map_store(member.base.store());
                let subst = self
                    .world
                    .generation
                    .interner
                    .subst(member.subst)
                    .iter()
                    .map(|&(param, ty)| {
                        let _ = self.map_store(param.store());
                        let ty_idx = self.map_type(ty);
                        (param.raw(), ty_idx)
                    })
                    .collect();
                SerializedElemRef::Member {
                    base: member.base,
                    subst,
                }
            }
        }
    }

    fn map_type(&mut self, ty: TypeId) -> u32 {
        if let Some(&idx) = self.type_map.get(&ty) {
            return idx;
        }
        let interner = &self.world.generation.interner;
        let kind = *interner.get(ty);
        let serialized = match kind {
            TypeKind::Dynamic => SerializedType::Dynamic,
            TypeKind::Void => SerializedType::Void,
            TypeKind::Invalid => SerializedType::Invalid,
            TypeKind::Unknown => SerializedType::Unknown,
            TypeKind::Never(n) => SerializedType::Never(n),
            TypeKind::Interface {
                element,
                args,
                nullability,
                alias,
            } => {
                let _ = self.map_store(element.store());
                let args = interner
                    .list(args)
                    .iter()
                    .map(|&t| self.map_type(t))
                    .collect();
                let alias = self.map_alias(alias);
                SerializedType::Interface {
                    element: element.raw(),
                    args,
                    nullability,
                    alias,
                }
            }
            TypeKind::Function(f) => {
                let type_params: Vec<ElementId> = interner
                    .list(f.type_params)
                    .iter()
                    .map(|&p| {
                        let _ = self.map_store(p.store());
                        p.raw()
                    })
                    .collect();
                let params: Vec<SerializedFnParam> = interner
                    .list(f.params)
                    .iter()
                    .map(|p| {
                        let name = p.name.map(|n| self.map_name(n));
                        let ty = self.map_type(p.ty);
                        let element = p.element.map(|e| self.map_elem_ref(e));
                        SerializedFnParam {
                            name,
                            kind: p.kind,
                            ty,
                            covariant: p.covariant,
                            element,
                        }
                    })
                    .collect();
                let ret = self.map_type(f.ret);
                let alias = self.map_alias(f.alias);
                SerializedType::Function {
                    type_params,
                    params,
                    required_positional: f.required_positional,
                    ret,
                    nullability: f.nullability,
                    alias,
                }
            }
            TypeKind::Record {
                positional,
                named,
                nullability,
                alias,
            } => {
                let positional = interner
                    .list(positional)
                    .iter()
                    .map(|&t| self.map_type(t))
                    .collect();
                let named = interner
                    .list(named)
                    .iter()
                    .map(|nt| {
                        let n = self.map_name(nt.name);
                        let t = self.map_type(nt.ty);
                        (n, t)
                    })
                    .collect();
                let alias = self.map_alias(alias);
                SerializedType::Record {
                    positional,
                    named,
                    nullability,
                    alias,
                }
            }
            TypeKind::TypeParameter {
                param,
                nullability,
                promoted_bound,
                alias,
            } => {
                let _ = self.map_store(param.store());
                let promoted_bound = promoted_bound.map(|b| self.map_type(b));
                let alias = self.map_alias(alias);
                SerializedType::TypeParameter {
                    param: param.raw(),
                    nullability,
                    promoted_bound,
                    alias,
                }
            }
        };
        let idx = self.types.len() as u32;
        self.types.push(serialized);
        self.type_map.insert(ty, idx);
        idx
    }

    #[inline]
    fn write_element_id(&mut self, w: &mut Writer, id: ElementId) {
        let store_idx = self.map_store(id.store());
        w.write_u16(store_idx);
        w.write_u8(id.tag() as u8);
        w.write_u32(id.index());
    }

    #[inline]
    fn write_opt_element_id(&mut self, w: &mut Writer, id: Option<ElementId>) {
        match id {
            Some(id) => {
                w.write_u8(1);
                self.write_element_id(w, id);
            }
            None => w.write_u8(0),
        }
    }

    #[inline]
    fn write_eid<T: ?Sized>(&mut self, w: &mut Writer, id: EId<T>) {
        self.write_element_id(w, id.raw());
    }

    #[inline]
    fn write_opt_eid<T: ?Sized>(&mut self, w: &mut Writer, id: Option<EId<T>>) {
        self.write_opt_element_id(w, id.map(|x| x.raw()));
    }

    #[inline]
    fn write_eid_vec<T: ?Sized>(&mut self, w: &mut Writer, list: &[EId<T>]) {
        w.write_u32(list.len() as u32);
        for &item in list {
            self.write_eid(w, item);
        }
    }

    #[inline]
    fn write_fragment_id(&mut self, w: &mut Writer, id: FragmentId) {
        let store_idx = self.map_store(id.store());
        w.write_u16(store_idx);
        w.write_u8(id.tag() as u8);
        w.write_u32(id.index());
    }

    #[inline]
    fn write_opt_fragment_id(&mut self, w: &mut Writer, id: Option<FragmentId>) {
        match id {
            Some(id) => {
                w.write_u8(1);
                self.write_fragment_id(w, id);
            }
            None => w.write_u8(0),
        }
    }

    #[inline]
    fn write_fid<T: ?Sized>(&mut self, w: &mut Writer, id: FId<T>) {
        self.write_fragment_id(w, id.raw());
    }

    #[inline]
    fn write_opt_fid<T: ?Sized>(&mut self, w: &mut Writer, id: Option<FId<T>>) {
        self.write_opt_fragment_id(w, id.map(|x| x.raw()));
    }

    #[inline]
    fn write_fid_vec<T: ?Sized>(&mut self, w: &mut Writer, list: &[FId<T>]) {
        w.write_u32(list.len() as u32);
        for &item in list {
            self.write_fid(w, item);
        }
    }

    #[inline]
    fn write_name(&mut self, w: &mut Writer, name: Name) {
        let idx = self.map_name(name);
        w.write_u32(idx);
    }

    #[inline]
    fn write_opt_name(&mut self, w: &mut Writer, name: Option<Name>) {
        match name {
            Some(n) => {
                w.write_u8(1);
                self.write_name(w, n);
            }
            None => w.write_u8(0),
        }
    }

    #[inline]
    fn write_type_id(&mut self, w: &mut Writer, ty: TypeId) {
        let idx = self.map_type(ty);
        w.write_u32(idx);
    }

    #[inline]
    fn write_opt_type_id(&mut self, w: &mut Writer, ty: Option<TypeId>) {
        match ty {
            Some(t) => {
                w.write_u8(1);
                self.write_type_id(w, t);
            }
            None => w.write_u8(0),
        }
    }

    #[inline]
    fn write_type_list(&mut self, w: &mut Writer, list: TypeList) {
        let slice = self.world.generation.interner.list(list);
        w.write_u32(slice.len() as u32);
        for &t in slice {
            self.write_type_id(w, t);
        }
    }

    #[inline]
    fn write_opt_type_list(&mut self, w: &mut Writer, list: Option<TypeList>) {
        match list {
            Some(l) => {
                w.write_u8(1);
                self.write_type_list(w, l);
            }
            None => w.write_u8(0),
        }
    }

    fn write_serialized_elem_ref(&self, w: &mut Writer, r: &SerializedElemRef) {
        match r {
            SerializedElemRef::Base(e) => {
                w.write_u8(0);
                let store_idx = self.store_map[&e.store()];
                w.write_u16(store_idx);
                w.write_u8(e.tag() as u8);
                w.write_u32(e.index());
            }
            SerializedElemRef::Member { base, subst } => {
                w.write_u8(1);
                let store_idx = self.store_map[&base.store()];
                w.write_u16(store_idx);
                w.write_u8(base.tag() as u8);
                w.write_u32(base.index());
                w.write_u32(subst.len() as u32);
                for &(param, ty_idx) in subst {
                    let p_store = self.store_map[&param.store()];
                    w.write_u16(p_store);
                    w.write_u8(param.tag() as u8);
                    w.write_u32(param.index());
                    w.write_u32(ty_idx);
                }
            }
        }
    }

    fn write_elem_ref(&mut self, w: &mut Writer, r: ElemRef) {
        let sr = self.map_elem_ref(r);
        self.write_serialized_elem_ref(w, &sr);
    }

    fn write_opt_elem_ref(&mut self, w: &mut Writer, r: Option<ElemRef>) {
        match r {
            Some(r) => {
                w.write_u8(1);
                self.write_elem_ref(w, r);
            }
            None => w.write_u8(0),
        }
    }

    fn write_serialized_alias(&self, w: &mut Writer, alias: Option<&SerializedAlias>) {
        match alias {
            None => w.write_u8(0),
            Some(a) => {
                w.write_u8(1);
                let store_idx = self.store_map[&a.element.store()];
                w.write_u16(store_idx);
                w.write_u8(a.element.tag() as u8);
                w.write_u32(a.element.index());
                w.write_u32(a.args.len() as u32);
                for &arg in &a.args {
                    w.write_u32(arg);
                }
                write_nullability(w, a.nullability);
            }
        }
    }

    fn write_tables(&self, w: &mut Writer) {
        let _ = self.cycle_store_id;
        w.write_u32(self.external_stores.len() as u32);
        for uri in &self.external_stores {
            w.write_str(uri);
        }
        w.write_u32(self.names.len() as u32);
        for name in &self.names {
            w.write_str(name);
        }
        // Types 0..6 are fixed; write types[6..]
        let custom_types = &self.types[6..];
        w.write_u32(custom_types.len() as u32);
        for ty in custom_types {
            match ty {
                SerializedType::Dynamic => w.write_u8(0),
                SerializedType::Void => w.write_u8(1),
                SerializedType::Invalid => w.write_u8(2),
                SerializedType::Unknown => w.write_u8(3),
                SerializedType::Never(n) => {
                    w.write_u8(4);
                    write_nullability(w, *n);
                }
                SerializedType::Interface {
                    element,
                    args,
                    nullability,
                    alias,
                } => {
                    w.write_u8(5);
                    let store_idx = self.store_map[&element.store()];
                    w.write_u16(store_idx);
                    w.write_u8(element.tag() as u8);
                    w.write_u32(element.index());
                    w.write_u32(args.len() as u32);
                    for &arg in args {
                        w.write_u32(arg);
                    }
                    write_nullability(w, *nullability);
                    self.write_serialized_alias(w, alias.as_ref());
                }
                SerializedType::Function {
                    type_params,
                    params,
                    required_positional,
                    ret,
                    nullability,
                    alias,
                } => {
                    w.write_u8(6);
                    w.write_u32(type_params.len() as u32);
                    for &tp in type_params {
                        let store_idx = self.store_map[&tp.store()];
                        w.write_u16(store_idx);
                        w.write_u8(tp.tag() as u8);
                        w.write_u32(tp.index());
                    }
                    w.write_u32(params.len() as u32);
                    for p in params {
                        w.write_opt_u32(p.name);
                        write_parameter_kind(w, p.kind);
                        w.write_u32(p.ty);
                        w.write_bool(p.covariant);
                        match &p.element {
                            None => w.write_u8(0),
                            Some(er) => {
                                w.write_u8(1);
                                self.write_serialized_elem_ref(w, er);
                            }
                        }
                    }
                    w.write_u16(*required_positional);
                    w.write_u32(*ret);
                    write_nullability(w, *nullability);
                    self.write_serialized_alias(w, alias.as_ref());
                }
                SerializedType::Record {
                    positional,
                    named,
                    nullability,
                    alias,
                } => {
                    w.write_u8(7);
                    w.write_u32(positional.len() as u32);
                    for &p in positional {
                        w.write_u32(p);
                    }
                    w.write_u32(named.len() as u32);
                    for &(n, t) in named {
                        w.write_u32(n);
                        w.write_u32(t);
                    }
                    write_nullability(w, *nullability);
                    self.write_serialized_alias(w, alias.as_ref());
                }
                SerializedType::TypeParameter {
                    param,
                    nullability,
                    promoted_bound,
                    alias,
                } => {
                    w.write_u8(8);
                    let store_idx = self.store_map[&param.store()];
                    w.write_u16(store_idx);
                    w.write_u8(param.tag() as u8);
                    w.write_u32(param.index());
                    write_nullability(w, *nullability);
                    w.write_opt_u32(*promoted_bound);
                    self.write_serialized_alias(w, alias.as_ref());
                }
            }
        }
    }

    fn write_metadata(&mut self, w: &mut Writer, m: &Metadata) {
        w.write_u32(m.annotations.len() as u32);
        for a in &m.annotations {
            self.write_fid(w, a.library_fragment);
            w.write_u32(a.annotation_ast.0.raw());
        }
        w.write_opt_u32(m.metadata_flags.try_get().copied());
    }

    fn write_directive_uri(&mut self, w: &mut Writer, uri: &DirectiveUri) {
        match uri {
            DirectiveUri::None => w.write_u8(0),
            DirectiveUri::RelativeUriString {
                relative_uri_string,
            } => {
                w.write_u8(1);
                w.write_str(relative_uri_string);
            }
            DirectiveUri::RelativeUri {
                relative_uri_string,
                relative_uri,
            } => {
                w.write_u8(2);
                w.write_str(relative_uri_string);
                w.write_str(relative_uri);
            }
            DirectiveUri::Source {
                relative_uri_string,
                relative_uri,
                source,
            } => {
                w.write_u8(3);
                w.write_str(relative_uri_string);
                w.write_str(relative_uri);
                w.write_str(&source.path);
                w.write_str(&source.uri);
            }
            DirectiveUri::Library {
                relative_uri_string,
                relative_uri,
                source,
                library,
            } => {
                w.write_u8(4);
                w.write_str(relative_uri_string);
                w.write_str(relative_uri);
                w.write_str(&source.path);
                w.write_str(&source.uri);
                self.write_eid(w, *library);
            }
            DirectiveUri::Unit {
                relative_uri_string,
                relative_uri,
                library_fragment,
            } => {
                w.write_u8(5);
                w.write_str(relative_uri_string);
                w.write_str(relative_uri);
                self.write_fid(w, *library_fragment);
            }
        }
    }

    fn write_combinators(&mut self, w: &mut Writer, combinators: &[NamespaceCombinator]) {
        w.write_u32(combinators.len() as u32);
        for c in combinators {
            match c {
                NamespaceCombinator::Hide {
                    hidden_names,
                    offset,
                    end,
                } => {
                    w.write_u8(0);
                    w.write_u32(hidden_names.len() as u32);
                    for &n in hidden_names {
                        self.write_name(w, n);
                    }
                    w.write_u32(*offset);
                    w.write_i32(*end);
                }
                NamespaceCombinator::Show {
                    shown_names,
                    offset,
                    end,
                } => {
                    w.write_u8(1);
                    w.write_u32(shown_names.len() as u32);
                    for &n in shown_names {
                        self.write_name(w, n);
                    }
                    w.write_u32(*offset);
                    w.write_i32(*end);
                }
            }
        }
    }

    fn write_namespace(&mut self, w: &mut Writer, ns: &Namespace) {
        w.write_u32(ns.defined_names.len() as u32);
        for (&name, &elem) in &ns.defined_names {
            self.write_name(w, name);
            self.write_element_id(w, elem);
        }
    }

    fn write_opt_namespace(&mut self, w: &mut Writer, slot: &OnceSlot<Arc<Namespace>>) {
        match slot.try_get() {
            None => w.write_u8(0),
            Some(ns) => {
                w.write_u8(1);
                self.write_namespace(w, ns);
            }
        }
    }

    fn write_top_level_inference_error(
        &mut self,
        w: &mut Writer,
        slot: &OnceSlot<TopLevelInferenceError>,
    ) {
        match slot.try_get() {
            None => w.write_u8(0),
            Some(TopLevelInferenceError::DependencyCycle { cycle }) => {
                w.write_u8(1);
                w.write_u32(cycle.len() as u32);
                for &n in cycle {
                    self.write_name(w, n);
                }
            }
            Some(TopLevelInferenceError::OverrideNoCombinedSuperSignature {
                candidate_signatures,
            }) => {
                w.write_u8(2);
                w.write_str(candidate_signatures);
            }
        }
    }

    fn write_element_data(&mut self, w: &mut Writer, d: &ElementData) {
        self.write_opt_name(w, d.name);
        self.write_opt_eid(w, d.library);
        self.write_opt_element_id(w, d.enclosing);
        self.write_fragment_id(w, d.first_fragment);
        w.write_u32(d.flags.get().bits());
        self.write_opt_fragment_id(w, d.previous_fragment_of_different_kind);
    }

    fn write_instance_element_data(&mut self, w: &mut Writer, d: &InstanceElementData) {
        self.write_element_data(w, &d.element);
        self.write_eid_vec(w, &d.type_params);
        self.write_eid_vec(w, &d.fields);
        self.write_eid_vec(w, &d.getters);
        self.write_eid_vec(w, &d.setters);
        self.write_eid_vec(w, &d.methods);
    }

    fn write_interface_element_data(&mut self, w: &mut Writer, d: &InterfaceElementData) {
        self.write_instance_element_data(w, &d.instance);
        self.write_opt_type_id(w, d.non_nullable_instance.try_get().copied());
        self.write_opt_type_id(w, d.nullable_instance.try_get().copied());
        self.write_opt_type_id(w, d.supertype.get());
        self.write_opt_type_list(w, d.mixins.get());
        self.write_opt_type_list(w, d.interfaces.get());
        self.write_opt_type_id(w, d.this_type.try_get().copied());
        match d.interface_cycle.try_get() {
            None => w.write_u8(0),
            Some(cycle) => {
                w.write_u8(1);
                self.write_eid_vec(w, cycle);
            }
        }
        self.write_opt_type_list(w, d.all_supertypes.try_get().copied());
        self.write_eid_vec(w, &d.constructors);
        w.write_bool(d.has_non_final_field.get());
    }

    fn write_executable_element_data(&mut self, w: &mut Writer, d: &ExecutableElementData) {
        self.write_element_data(w, &d.element);
        self.write_eid_vec(w, &d.type_params);
        self.write_eid_vec(w, &d.formal_params);
        self.write_opt_type_id(w, d.return_type.get());
        self.write_opt_type_id(w, d.type_.get());
    }

    fn write_variable_element_data(&mut self, w: &mut Writer, d: &VariableElementData) {
        self.write_element_data(w, &d.element);
        match d.constant_initializer.try_get() {
            None => w.write_u8(0),
            Some(None) => w.write_u8(1),
            Some(Some(ci)) => {
                w.write_u8(2);
                self.write_fragment_id(w, ci.fragment);
                w.write_u32(ci.expression.0.raw());
            }
        }
    }

    fn write_property_inducing_element_data(
        &mut self,
        w: &mut Writer,
        d: &PropertyInducingElementData,
    ) {
        self.write_variable_element_data(w, &d.variable);
        self.write_opt_eid(w, d.getter);
        self.write_opt_eid(w, d.setter);
        self.write_opt_type_id(w, d.type_.get());
        self.write_top_level_inference_error(w, &d.type_inference_error);
    }

    fn write_fragment_data(&mut self, w: &mut Writer, d: &FragmentData) {
        self.write_opt_name(w, d.name);
        w.write_opt_u32(d.name_offset);
        self.write_opt_element_id(w, d.element.try_get().copied());
        self.write_opt_fragment_id(w, d.enclosing_fragment);
        self.write_opt_fragment_id(w, d.previous_fragment);
        self.write_opt_fragment_id(w, d.next_fragment);
        w.write_opt_u32(d.first_token_offset);
        w.write_u64(d.flags.get().bits());
        w.write_opt_str(d.documentation_comment.as_deref());
        self.write_metadata(w, &d.metadata);
        w.write_opt_u32(d.code_offset);
        w.write_opt_u32(d.code_length);
    }

    fn write_instance_fragment_data(&mut self, w: &mut Writer, d: &InstanceFragmentData) {
        self.write_fragment_data(w, &d.fragment);
        self.write_fid_vec(w, &d.type_params);
        self.write_fid_vec(w, &d.fields);
        self.write_fid_vec(w, &d.getters);
        self.write_fid_vec(w, &d.setters);
        self.write_fid_vec(w, &d.methods);
    }

    fn write_interface_fragment_data(&mut self, w: &mut Writer, d: &InterfaceFragmentData) {
        self.write_instance_fragment_data(w, &d.instance);
        self.write_fid_vec(w, &d.constructors);
    }

    fn write_executable_fragment_data(&mut self, w: &mut Writer, d: &ExecutableFragmentData) {
        self.write_fragment_data(w, &d.fragment);
        self.write_fid_vec(w, &d.type_params);
        self.write_fid_vec(w, &d.formal_params);
    }

    fn write_variable_fragment_data(&mut self, w: &mut Writer, d: &VariableFragmentData) {
        self.write_fragment_data(w, &d.fragment);
        w.write_opt_u32(d.constant_initializer.map(|c| c.0.raw()));
    }

    fn write_property_inducing_fragment_data(
        &mut self,
        w: &mut Writer,
        d: &PropertyInducingFragmentData,
    ) {
        self.write_variable_fragment_data(w, &d.variable);
        self.write_opt_fid(w, d.induced_getter);
        self.write_opt_fid(w, d.induced_setter);
    }

    fn write_cycle_body(&mut self, w: &mut Writer, cycle: &LinkedCycle) {
        let mut const_ast_bytes = Vec::new();
        cycle.const_exprs.ast.encode(&mut const_ast_bytes);
        w.write_bytes(&const_ast_bytes);

        let e = &cycle.store.elements;
        // 1. classes
        w.write_u32(e.classes.len() as u32);
        for (_, item) in e.classes.iter() {
            self.write_interface_element_data(w, &item.interface);
        }
        // 2. enums
        w.write_u32(e.enums.len() as u32);
        for (_, item) in e.enums.iter() {
            self.write_interface_element_data(w, &item.interface);
        }
        // 3. mixins
        w.write_u32(e.mixins.len() as u32);
        for (_, item) in e.mixins.iter() {
            self.write_interface_element_data(w, &item.interface);
            self.write_opt_type_list(w, item.superclass_constraints.get());
        }
        // 4. extensions
        w.write_u32(e.extensions.len() as u32);
        for (_, item) in e.extensions.iter() {
            self.write_instance_element_data(w, &item.instance);
            self.write_opt_type_id(w, item.extended_type.get());
        }
        // 5. extension_types
        w.write_u32(e.extension_types.len() as u32);
        for (_, item) in e.extension_types.iter() {
            self.write_interface_element_data(w, &item.interface);
            w.write_bool(item.has_representation_self_reference.get());
            w.write_bool(item.has_implements_self_reference.get());
            self.write_opt_type_id(w, item.type_erasure.try_get().copied());
        }
        // 6. fields
        w.write_u32(e.fields.len() as u32);
        for (_, item) in e.fields.iter() {
            self.write_property_inducing_element_data(w, &item.property);
        }
        // 7. getters
        w.write_u32(e.getters.len() as u32);
        for (_, item) in e.getters.iter() {
            self.write_executable_element_data(w, &item.accessor.executable);
            self.write_opt_eid(w, item.accessor.variable.get());
        }
        // 8. setters
        w.write_u32(e.setters.len() as u32);
        for (_, item) in e.setters.iter() {
            self.write_executable_element_data(w, &item.accessor.executable);
            self.write_opt_eid(w, item.accessor.variable.get());
        }
        // 9. methods
        w.write_u32(e.methods.len() as u32);
        for (_, item) in e.methods.iter() {
            self.write_executable_element_data(w, &item.executable);
            w.write_bool(item.is_operator_equal_with_parameter_type_from_object.get());
            self.write_top_level_inference_error(w, &item.type_inference_error);
        }
        // 10. constructors
        w.write_u32(e.constructors.len() as u32);
        for (_, item) in e.constructors.iter() {
            self.write_executable_element_data(w, &item.executable);
            self.write_opt_elem_ref(w, item.redirected_constructor.get());
            self.write_opt_elem_ref(w, item.super_constructor.get());
        }
        // 11. functions
        w.write_u32(e.functions.len() as u32);
        for (_, item) in e.functions.iter() {
            self.write_executable_element_data(w, &item.executable);
        }
        // 12. variables
        w.write_u32(e.variables.len() as u32);
        for (_, item) in e.variables.iter() {
            self.write_property_inducing_element_data(w, &item.property);
        }
        // 13. type_aliases
        w.write_u32(e.type_aliases.len() as u32);
        for (_, item) in e.type_aliases.iter() {
            self.write_element_data(w, &item.element);
            self.write_eid_vec(w, &item.type_params);
            self.write_opt_type_id(w, item.aliased_type.get());
        }
        // 14. type_params
        w.write_u32(e.type_params.len() as u32);
        for (_, item) in e.type_params.iter() {
            self.write_element_data(w, &item.element);
            write_opt_variance(w, item.variance);
            self.write_opt_type_id(w, item.bound.get());
            self.write_opt_type_id(w, item.default_type.get());
        }
        // 15. params
        w.write_u32(e.params.len() as u32);
        for (_, item) in e.params.iter() {
            self.write_variable_element_data(w, &item.variable);
            write_parameter_kind(w, item.kind);
            self.write_opt_type_id(w, item.type_.get());
            self.write_opt_eid(w, item.base_formal_parameter);
            self.write_opt_eid(w, item.field.get());
        }
        // 16. prefixes
        w.write_u32(e.prefixes.len() as u32);
        for (_, item) in e.prefixes.iter() {
            self.write_element_data(w, &item.element);
            self.write_name(w, item.local_id);
            self.write_fid(w, item.last_fragment);
        }
        // 17. libraries
        w.write_u32(e.libraries.len() as u32);
        for (_, item) in e.libraries.iter() {
            self.write_element_data(w, &item.element);
            self.write_metadata(w, &item.metadata);
            w.write_opt_str(item.documentation_comment.as_deref());
            w.write_u32(item.language_version.package.major);
            w.write_u32(item.language_version.package.minor);
            match item.language_version.override_ {
                None => w.write_u8(0),
                Some(v) => {
                    w.write_u8(1);
                    w.write_u32(v.major);
                    w.write_u32(v.minor);
                }
            }
            let enabled = item.feature_set.enabled();
            w.write_u32(enabled.len() as u32);
            for feat in enabled {
                w.write_str(feat);
            }
            match item.entry_point.try_get() {
                None => w.write_u8(0),
                Some(None) => w.write_u8(1),
                Some(Some(ep)) => {
                    w.write_u8(2);
                    self.write_eid(w, *ep);
                }
            }
            self.write_opt_eid(w, item.load_library_function.try_get().copied());
            w.write_i32(item.name_offset);
            w.write_u32(item.name_length);
            self.write_eid_vec(w, &item.classes);
            self.write_eid_vec(w, &item.enums);
            self.write_eid_vec(w, &item.extensions);
            self.write_eid_vec(w, &item.extension_types);
            self.write_eid_vec(w, &item.getters);
            self.write_eid_vec(w, &item.setters);
            self.write_eid_vec(w, &item.mixins);
            self.write_eid_vec(w, &item.top_level_functions);
            self.write_eid_vec(w, &item.top_level_variables);
            self.write_eid_vec(w, &item.type_aliases);
            self.write_opt_namespace(w, &item.export_namespace);
            self.write_opt_namespace(w, &item.public_namespace);
            match item.field_name_non_promotability_info.try_get() {
                None => w.write_u8(0),
                Some(map) => {
                    w.write_u8(1);
                    w.write_u32(map.len() as u32);
                    for (&name, info) in map {
                        self.write_name(w, name);
                        self.write_eid_vec(w, &info.conflicting_fields);
                        self.write_eid_vec(w, &info.conflicting_getters);
                        self.write_eid_vec(w, &info.conflicting_nsm_classes);
                    }
                }
            }
        }
        // 18. generic_function_types
        w.write_u32(e.generic_function_types.len() as u32);
        for (_, item) in e.generic_function_types.iter() {
            self.write_element_data(w, &item.element);
            self.write_eid_vec(w, &item.type_params);
            self.write_eid_vec(w, &item.formal_params);
            self.write_opt_type_id(w, item.return_type.get());
            self.write_opt_type_id(w, item.type_.get());
        }
        // 19. locals
        w.write_u32(e.locals.len() as u32);
        for (_, item) in e.locals.iter() {
            self.write_variable_element_data(w, &item.variable);
            self.write_opt_type_id(w, item.type_.get());
        }
        // 20. local_functions
        w.write_u32(e.local_functions.len() as u32);
        for (_, item) in e.local_functions.iter() {
            self.write_executable_element_data(w, &item.executable);
        }
        // 21. labels
        w.write_u32(e.labels.len() as u32);
        for (_, item) in e.labels.iter() {
            self.write_element_data(w, &item.element);
        }
        // 22. multiply_defined
        w.write_u32(e.multiply_defined.len() as u32);
        for (_, item) in e.multiply_defined.iter() {
            self.write_element_data(w, &item.element);
            self.write_fid(w, item.library_fragment);
            w.write_u32(item.conflicting_elements.len() as u32);
            for &ce in &item.conflicting_elements {
                self.write_element_id(w, ce);
            }
        }

        // Fragments
        let f = &cycle.store.fragments;
        // 1. classes
        w.write_u32(f.classes.len() as u32);
        for (_, item) in f.classes.iter() {
            self.write_interface_fragment_data(w, &item.interface);
        }
        // 2. enums
        w.write_u32(f.enums.len() as u32);
        for (_, item) in f.enums.iter() {
            self.write_interface_fragment_data(w, &item.interface);
        }
        // 3. mixins
        w.write_u32(f.mixins.len() as u32);
        for (_, item) in f.mixins.iter() {
            self.write_interface_fragment_data(w, &item.interface);
            match item.super_invoked_names.try_get() {
                None => w.write_u8(0),
                Some(names) => {
                    w.write_u8(1);
                    w.write_u32(names.len() as u32);
                    for &n in names {
                        self.write_name(w, n);
                    }
                }
            }
        }
        // 4. extensions
        w.write_u32(f.extensions.len() as u32);
        for (_, item) in f.extensions.iter() {
            self.write_instance_fragment_data(w, &item.instance);
        }
        // 5. extension_types
        w.write_u32(f.extension_types.len() as u32);
        for (_, item) in f.extension_types.iter() {
            self.write_interface_fragment_data(w, &item.interface);
        }
        // 6. fields
        w.write_u32(f.fields.len() as u32);
        for (_, item) in f.fields.iter() {
            self.write_property_inducing_fragment_data(w, &item.property);
            w.write_bool(item.inherits_covariant.get());
        }
        // 7. getters
        w.write_u32(f.getters.len() as u32);
        for (_, item) in f.getters.iter() {
            self.write_executable_fragment_data(w, &item.accessor.executable);
            self.write_opt_fid(w, item.accessor.inducing_variable);
        }
        // 8. setters
        w.write_u32(f.setters.len() as u32);
        for (_, item) in f.setters.iter() {
            self.write_executable_fragment_data(w, &item.accessor.executable);
            self.write_opt_fid(w, item.accessor.inducing_variable);
        }
        // 9. methods
        w.write_u32(f.methods.len() as u32);
        for (_, item) in f.methods.iter() {
            self.write_executable_fragment_data(w, &item.executable);
        }
        // 10. constructors
        w.write_u32(f.constructors.len() as u32);
        for (_, item) in f.constructors.iter() {
            self.write_executable_fragment_data(w, &item.executable);
            match item.constant_initializers.try_get() {
                None => w.write_u8(0),
                Some(inits) => {
                    w.write_u8(1);
                    w.write_u32(inits.len() as u32);
                    for &ci in inits {
                        w.write_u32(ci.0.raw());
                    }
                }
            }
            w.write_opt_u32(item.new_keyword_offset);
            w.write_opt_u32(item.factory_keyword_offset);
            self.write_opt_name(w, item.type_name);
            w.write_opt_u32(item.type_name_offset);
            w.write_opt_u32(item.period_offset);
            w.write_opt_u32(item.name_end);
            w.write_opt_u32(item.this_keyword_offset);
        }
        // 11. functions
        w.write_u32(f.functions.len() as u32);
        for (_, item) in f.functions.iter() {
            self.write_executable_fragment_data(w, &item.executable);
        }
        // 12. variables
        w.write_u32(f.variables.len() as u32);
        for (_, item) in f.variables.iter() {
            self.write_property_inducing_fragment_data(w, &item.property);
        }
        // 13. type_aliases
        w.write_u32(f.type_aliases.len() as u32);
        for (_, item) in f.type_aliases.iter() {
            self.write_fragment_data(w, &item.fragment);
            self.write_fid_vec(w, &item.type_params);
            w.write_bool(item.has_self_reference.get());
        }
        // 14. type_params
        w.write_u32(f.type_params.len() as u32);
        for (_, item) in f.type_params.iter() {
            self.write_fragment_data(w, &item.fragment);
        }
        // 15. params
        w.write_u32(f.params.len() as u32);
        for (_, item) in f.params.iter() {
            self.write_variable_fragment_data(w, &item.variable);
            write_parameter_kind(w, item.parameter_kind);
            self.write_opt_name(w, item.private_name);
        }
        // 16. prefixes
        w.write_u32(f.prefixes.len() as u32);
        for (_, item) in f.prefixes.iter() {
            self.write_fragment_data(w, &item.fragment);
            w.write_u32(item.offset);
            w.write_bool(item.is_deferred);
        }
        // 17. units
        w.write_u32(f.units.len() as u32);
        for (_, item) in f.units.iter() {
            self.write_fragment_data(w, &item.fragment);
            w.write_str(&item.source.path);
            w.write_str(&item.source.uri);
            w.write_u32(item.line_starts.len() as u32);
            for &ls in item.line_starts.iter() {
                w.write_u32(ls);
            }
            self.write_eid(w, item.library);
            w.write_u32(item.library_exports.len() as u32);
            for exp in &item.library_exports {
                self.write_fid(w, exp.directive.library_fragment);
                self.write_directive_uri(w, &exp.directive.uri);
                self.write_metadata(w, &exp.directive.metadata);
                self.write_combinators(w, &exp.combinators);
                w.write_i32(exp.export_keyword_offset);
            }
            w.write_u32(item.library_imports.len() as u32);
            for imp in &item.library_imports {
                self.write_fid(w, imp.directive.library_fragment);
                self.write_directive_uri(w, &imp.directive.uri);
                self.write_metadata(w, &imp.directive.metadata);
                w.write_bool(imp.is_synthetic);
                self.write_combinators(w, &imp.combinators);
                w.write_i32(imp.import_keyword_offset);
                self.write_opt_fid(w, imp.prefix);
                self.write_opt_namespace(w, &imp.namespace);
            }
            self.write_eid_vec(w, &item.library_import_prefixes);
            w.write_u32(item.library_import_prefixes_by_id.len() as u32);
            for (&name, &pref) in &item.library_import_prefixes_by_id {
                self.write_name(w, name);
                self.write_eid(w, pref);
            }
            w.write_u32(item.parts.len() as u32);
            for part in &item.parts {
                self.write_fid(w, part.directive.library_fragment);
                self.write_directive_uri(w, &part.directive.uri);
                self.write_metadata(w, &part.directive.metadata);
                w.write_i32(part.part_keyword_offset);
            }
            self.write_fid_vec(w, &item.classes);
            self.write_fid_vec(w, &item.enums);
            self.write_fid_vec(w, &item.extensions);
            self.write_fid_vec(w, &item.extension_types);
            self.write_fid_vec(w, &item.functions);
            self.write_fid_vec(w, &item.getters);
            self.write_fid_vec(w, &item.setters);
            self.write_fid_vec(w, &item.mixins);
            self.write_fid_vec(w, &item.type_aliases);
            self.write_fid_vec(w, &item.variables);
        }
        // 18. generic_function_types
        w.write_u32(f.generic_function_types.len() as u32);
        for (_, item) in f.generic_function_types.iter() {
            self.write_fragment_data(w, &item.fragment);
            self.write_fid_vec(w, &item.type_params);
            self.write_fid_vec(w, &item.formal_params);
            w.write_bool(item.is_nullable);
        }
        // 19. locals
        w.write_u32(f.locals.len() as u32);
        for (_, item) in f.locals.iter() {
            self.write_variable_fragment_data(w, &item.variable);
            self.write_opt_fid(w, item.pattern.join.get());
            w.write_bool(item.pattern.is_visiting_when_clause.get());
            w.write_opt_u32(item.pattern.node.map(|n| n.raw()));
            w.write_bool(item.pattern.is_duplicate.get());
            self.write_fid_vec(w, &item.pattern.variables);
            match item.pattern.inconsistency.get() {
                None => w.write_u8(0),
                Some(inc) => {
                    w.write_u8(1);
                    let code = match inc {
                        JoinedPatternVariableInconsistency::None => 0,
                        JoinedPatternVariableInconsistency::LogicalOr => 1,
                        JoinedPatternVariableInconsistency::SharedCaseAbsent => 2,
                        JoinedPatternVariableInconsistency::SharedCaseHasLabel => 3,
                        JoinedPatternVariableInconsistency::DifferentFinalityOrType => 4,
                    };
                    w.write_u8(code);
                }
            }
            let refs = item.pattern.references.lock();
            w.write_u32(refs.len() as u32);
            for &r_id in refs.iter() {
                w.write_u32(r_id.raw());
            }
        }
        // 20. local_functions
        w.write_u32(f.local_functions.len() as u32);
        for (_, item) in f.local_functions.iter() {
            self.write_executable_fragment_data(w, &item.executable);
        }
        // 21. labels
        w.write_u32(f.labels.len() as u32);
        for (_, item) in f.labels.iter() {
            self.write_fragment_data(w, &item.fragment);
            w.write_bool(item.on_switch_member);
        }
        // 22. multiply_defined
        w.write_u32(f.multiply_defined.len() as u32);
        for (_, item) in f.multiply_defined.iter() {
            self.write_fragment_data(w, &item.fragment);
        }

        // LinkedLibraries
        w.write_u32(cycle.libraries.len() as u32);
        for lib in &cycle.libraries {
            w.write_str(&lib.uri);
            self.write_eid(w, lib.element);
            w.write_u32(lib.export_entries.len() as u32);
            for exp in &lib.export_entries {
                w.write_str(&exp.name);
                self.write_element_id(w, exp.element);
                w.write_u32(exp.locations.len() as u32);
                for loc in &exp.locations {
                    w.write_u32(loc.fragment_index);
                    w.write_u32(loc.export_index);
                }
            }
            let (ref_uri, references, _top_levels_by_kind, _members, built_ins) =
                lib.reference.raw_parts();
            w.write_str(ref_uri);
            for b in built_ins {
                match b {
                    None => w.write_u8(0),
                    Some(id) => {
                        w.write_u8(1);
                        w.write_u32(id.0);
                    }
                }
            }
            w.write_u32(references.len() as u32);
            for r_entry in references {
                match &r_entry.kind {
                    ReferenceKind::Library => w.write_u8(0),
                    ReferenceKind::TopLevel { kind, key } => {
                        w.write_u8(1);
                        w.write_u8(top_level_ref_kind_to_u8(*kind));
                        w.write_str(key);
                    }
                    ReferenceKind::Member {
                        container,
                        kind,
                        key,
                    } => {
                        w.write_u8(2);
                        w.write_u32(container.0);
                        w.write_u8(member_ref_kind_to_u8(*kind));
                        w.write_str(key);
                    }
                    ReferenceKind::BuiltIn(BuiltInReferenceKind::Dynamic) => w.write_u8(3),
                    ReferenceKind::BuiltIn(BuiltInReferenceKind::Never) => w.write_u8(4),
                }
                self.write_opt_element_id(w, r_entry.element);
            }
        }
    }
}

struct CycleDecoder<'a> {
    world: &'a WorldSnapshot,
    stores: Vec<StoreId>,
    names: Vec<Name>,
    types: Vec<TypeId>,
}

impl<'a> CycleDecoder<'a> {
    fn read_tables(
        r: &mut Reader<'_>,
        world: &'a WorldSnapshot,
        new_store_id: StoreId,
    ) -> Option<Self> {
        let ext_len = r.check_len(4)?;
        let mut stores = Vec::with_capacity(2 + ext_len);
        stores.push(StoreId::SYNTHETIC);
        stores.push(new_store_id);
        for _ in 0..ext_len {
            let uri = r.read_str()?;
            let lib_eid = *world.libraries.get(uri)?;
            stores.push(lib_eid.store());
        }

        let names_len = r.check_len(4)?;
        let mut names = Vec::with_capacity(names_len);
        for _ in 0..names_len {
            let s = r.read_str()?;
            names.push(world.generation.names.intern(s));
        }

        let custom_types_len = r.check_len(1)?;
        let mut types = Vec::with_capacity(6 + custom_types_len);
        types.extend_from_slice(&[
            TypeId::DYNAMIC,
            TypeId::VOID,
            TypeId::INVALID,
            TypeId::UNKNOWN,
            TypeId::NEVER,
            TypeId::NEVER_QUESTION,
        ]);

        let mut dec = Self {
            world,
            stores,
            names,
            types,
        };

        let interner = &world.generation.interner;
        for _ in 0..custom_types_len {
            let tag = r.read_u8()?;
            let kind = match tag {
                0 => TypeKind::Dynamic,
                1 => TypeKind::Void,
                2 => TypeKind::Invalid,
                3 => TypeKind::Unknown,
                4 => TypeKind::Never(read_nullability(r)?),
                5 => {
                    let element = dec.read_eid::<InterfaceElement>(r)?;
                    let args_len = r.check_len(4)?;
                    let mut arg_ids = Vec::with_capacity(args_len);
                    for _ in 0..args_len {
                        arg_ids.push(dec.read_type_id(r)?);
                    }
                    let args = interner.intern_list(&arg_ids);
                    let nullability = read_nullability(r)?;
                    let alias = dec.read_alias(r)?;
                    TypeKind::Interface {
                        element,
                        args,
                        nullability,
                        alias,
                    }
                }
                6 => {
                    let tp_len = r.check_len(7)?;
                    let mut tp_ids = Vec::with_capacity(tp_len);
                    for _ in 0..tp_len {
                        tp_ids.push(dec.read_eid::<TypeParameterElement>(r)?);
                    }
                    let type_params = interner.intern_list(&tp_ids);
                    let params_len = r.check_len(7)?;
                    let mut fn_params = Vec::with_capacity(params_len);
                    for _ in 0..params_len {
                        let name_idx = r.read_opt_u32()?;
                        let name = match name_idx {
                            Some(idx) => Some(*dec.names.get(idx as usize)?),
                            None => None,
                        };
                        let kind = read_parameter_kind(r)?;
                        let ty = dec.read_type_id(r)?;
                        let covariant = r.read_bool()?;
                        let element = match r.read_u8()? {
                            0 => None,
                            1 => Some(dec.read_elem_ref(r)?),
                            _ => return None,
                        };
                        fn_params.push(FnParam {
                            name,
                            kind,
                            ty,
                            covariant,
                            element,
                        });
                    }
                    let params = interner.intern_list(&fn_params);
                    let required_positional = r.read_u16()?;
                    let ret = dec.read_type_id(r)?;
                    let nullability = read_nullability(r)?;
                    let alias = dec.read_alias(r)?;
                    TypeKind::Function(FunctionTypeData {
                        type_params,
                        params,
                        required_positional,
                        ret,
                        nullability,
                        alias,
                    })
                }
                7 => {
                    let pos_len = r.check_len(4)?;
                    let mut pos_ids = Vec::with_capacity(pos_len);
                    for _ in 0..pos_len {
                        pos_ids.push(dec.read_type_id(r)?);
                    }
                    let positional = interner.intern_list(&pos_ids);
                    let named_len = r.check_len(8)?;
                    let mut named_fields = Vec::with_capacity(named_len);
                    for _ in 0..named_len {
                        let name = dec.read_name(r)?;
                        let ty = dec.read_type_id(r)?;
                        named_fields.push(NamedType { name, ty });
                    }
                    let named = interner.intern_list(&named_fields);
                    let nullability = read_nullability(r)?;
                    let alias = dec.read_alias(r)?;
                    TypeKind::Record {
                        positional,
                        named,
                        nullability,
                        alias,
                    }
                }
                8 => {
                    let param = dec.read_eid::<TypeParameterElement>(r)?;
                    let nullability = read_nullability(r)?;
                    let promoted_bound = dec.read_opt_type_id(r)?;
                    let alias = dec.read_alias(r)?;
                    TypeKind::TypeParameter {
                        param,
                        nullability,
                        promoted_bound,
                        alias,
                    }
                }
                _ => return None,
            };
            dec.types.push(interner.intern(kind));
        }

        Some(dec)
    }

    #[inline]
    fn read_element_id(&self, r: &mut Reader<'_>) -> Option<ElementId> {
        let store_idx = r.read_u16()? as usize;
        let store = *self.stores.get(store_idx)?;
        let tag = Tag::from_u8(r.read_u8()?)?;
        let index = r.read_u32()?;
        Some(ElementId::new(store, tag, index))
    }

    #[inline]
    fn read_opt_element_id(&self, r: &mut Reader<'_>) -> Option<Option<ElementId>> {
        match r.read_u8()? {
            0 => Some(None),
            1 => Some(Some(self.read_element_id(r)?)),
            _ => None,
        }
    }

    #[inline]
    fn read_eid<T: ElementType + ?Sized>(&self, r: &mut Reader<'_>) -> Option<EId<T>> {
        self.read_element_id(r)?.cast::<T>()
    }

    #[inline]
    fn read_opt_eid<T: ElementType + ?Sized>(&self, r: &mut Reader<'_>) -> Option<Option<EId<T>>> {
        match self.read_opt_element_id(r)? {
            None => Some(None),
            Some(id) => Some(Some(id.cast::<T>()?)),
        }
    }

    #[inline]
    fn read_eid_vec<T: ElementType + ?Sized>(&self, r: &mut Reader<'_>) -> Option<Vec<EId<T>>> {
        let len = r.check_len(7)?;
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            out.push(self.read_eid::<T>(r)?);
        }
        Some(out)
    }

    #[inline]
    fn read_fragment_id(&self, r: &mut Reader<'_>) -> Option<FragmentId> {
        let store_idx = r.read_u16()? as usize;
        let store = *self.stores.get(store_idx)?;
        let tag = Tag::from_u8(r.read_u8()?)?;
        let index = r.read_u32()?;
        Some(FragmentId::new(store, tag, index))
    }

    #[inline]
    fn read_opt_fragment_id(&self, r: &mut Reader<'_>) -> Option<Option<FragmentId>> {
        match r.read_u8()? {
            0 => Some(None),
            1 => Some(Some(self.read_fragment_id(r)?)),
            _ => None,
        }
    }

    #[inline]
    fn read_fid<T: FragmentType + ?Sized>(&self, r: &mut Reader<'_>) -> Option<FId<T>> {
        self.read_fragment_id(r)?.cast::<T>()
    }

    #[inline]
    fn read_opt_fid<T: FragmentType + ?Sized>(&self, r: &mut Reader<'_>) -> Option<Option<FId<T>>> {
        match self.read_opt_fragment_id(r)? {
            None => Some(None),
            Some(id) => Some(Some(id.cast::<T>()?)),
        }
    }

    #[inline]
    fn read_fid_vec<T: FragmentType + ?Sized>(&self, r: &mut Reader<'_>) -> Option<Vec<FId<T>>> {
        let len = r.check_len(7)?;
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            out.push(self.read_fid::<T>(r)?);
        }
        Some(out)
    }

    #[inline]
    fn read_name(&self, r: &mut Reader<'_>) -> Option<Name> {
        let idx = r.read_u32()? as usize;
        self.names.get(idx).copied()
    }

    #[inline]
    fn read_opt_name(&self, r: &mut Reader<'_>) -> Option<Option<Name>> {
        match r.read_u8()? {
            0 => Some(None),
            1 => Some(Some(self.read_name(r)?)),
            _ => None,
        }
    }

    #[inline]
    fn read_type_id(&self, r: &mut Reader<'_>) -> Option<TypeId> {
        let idx = r.read_u32()? as usize;
        self.types.get(idx).copied()
    }

    #[inline]
    fn read_opt_type_id(&self, r: &mut Reader<'_>) -> Option<Option<TypeId>> {
        match r.read_u8()? {
            0 => Some(None),
            1 => Some(Some(self.read_type_id(r)?)),
            _ => None,
        }
    }

    #[inline]
    fn read_type_list(&self, r: &mut Reader<'_>) -> Option<TypeList> {
        let len = r.check_len(4)?;
        let mut ids = Vec::with_capacity(len);
        for _ in 0..len {
            ids.push(self.read_type_id(r)?);
        }
        Some(self.world.generation.interner.intern_list(&ids))
    }

    #[inline]
    fn read_opt_type_list(&self, r: &mut Reader<'_>) -> Option<Option<TypeList>> {
        match r.read_u8()? {
            0 => Some(None),
            1 => Some(Some(self.read_type_list(r)?)),
            _ => None,
        }
    }

    fn read_alias(&self, r: &mut Reader<'_>) -> Option<Option<AliasId>> {
        match r.read_u8()? {
            0 => Some(None),
            1 => {
                let element = self.read_eid::<TypeAliasElement>(r)?;
                let args = self.read_type_list(r)?;
                let nullability = read_nullability(r)?;
                let id = self.world.generation.interner.intern_alias(AliasRef {
                    element,
                    args,
                    nullability,
                });
                Some(Some(id))
            }
            _ => None,
        }
    }

    fn read_elem_ref(&self, r: &mut Reader<'_>) -> Option<ElemRef> {
        match r.read_u8()? {
            0 => Some(ElemRef::Base(self.read_element_id(r)?)),
            1 => {
                let base = self.read_element_id(r)?;
                let len = r.check_len(11)?;
                let mut pairs = Vec::with_capacity(len);
                for _ in 0..len {
                    let param = self.read_eid::<TypeParameterElement>(r)?;
                    let ty = self.read_type_id(r)?;
                    pairs.push((param, ty));
                }
                let subst = self.world.generation.interner.intern_subst(&mut pairs);
                let member = self
                    .world
                    .generation
                    .interner
                    .intern_member(Member { base, subst });
                Some(ElemRef::Member(member))
            }
            _ => None,
        }
    }

    fn read_opt_elem_ref(&self, r: &mut Reader<'_>) -> Option<Option<ElemRef>> {
        match r.read_u8()? {
            0 => Some(None),
            1 => Some(Some(self.read_elem_ref(r)?)),
            _ => None,
        }
    }

    fn read_metadata(&self, r: &mut Reader<'_>) -> Option<Metadata> {
        let len = r.check_len(11)?;
        let mut annotations = Vec::with_capacity(len);
        for _ in 0..len {
            let library_fragment = self.read_fid::<LibraryFragment>(r)?;
            let annotation_ast = ConstExprId(NodeId::from_raw(r.read_u32()?)?);
            annotations.push(ElementAnnotation {
                library_fragment,
                annotation_ast,
            });
        }
        let metadata_flags = match r.read_opt_u32()? {
            Some(v) => OnceSlot::with(v),
            None => OnceSlot::new(),
        };
        Some(Metadata {
            annotations,
            metadata_flags,
        })
    }

    fn read_directive_uri(&self, r: &mut Reader<'_>) -> Option<DirectiveUri> {
        match r.read_u8()? {
            0 => Some(DirectiveUri::None),
            1 => Some(DirectiveUri::RelativeUriString {
                relative_uri_string: r.read_arc_str()?,
            }),
            2 => Some(DirectiveUri::RelativeUri {
                relative_uri_string: r.read_arc_str()?,
                relative_uri: r.read_arc_str()?,
            }),
            3 => Some(DirectiveUri::Source {
                relative_uri_string: r.read_arc_str()?,
                relative_uri: r.read_arc_str()?,
                source: SourceRef {
                    path: r.read_arc_str()?,
                    uri: r.read_arc_str()?,
                },
            }),
            4 => Some(DirectiveUri::Library {
                relative_uri_string: r.read_arc_str()?,
                relative_uri: r.read_arc_str()?,
                source: SourceRef {
                    path: r.read_arc_str()?,
                    uri: r.read_arc_str()?,
                },
                library: self.read_eid::<LibraryElement>(r)?,
            }),
            5 => Some(DirectiveUri::Unit {
                relative_uri_string: r.read_arc_str()?,
                relative_uri: r.read_arc_str()?,
                library_fragment: self.read_fid::<LibraryFragment>(r)?,
            }),
            _ => None,
        }
    }

    fn read_combinators(&self, r: &mut Reader<'_>) -> Option<Vec<NamespaceCombinator>> {
        let len = r.check_len(13)?;
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            match r.read_u8()? {
                0 => {
                    let n_len = r.check_len(4)?;
                    let mut hidden_names = Vec::with_capacity(n_len);
                    for _ in 0..n_len {
                        hidden_names.push(self.read_name(r)?);
                    }
                    let offset = r.read_u32()?;
                    let end = r.read_i32()?;
                    out.push(NamespaceCombinator::Hide {
                        hidden_names,
                        offset,
                        end,
                    });
                }
                1 => {
                    let n_len = r.check_len(4)?;
                    let mut shown_names = Vec::with_capacity(n_len);
                    for _ in 0..n_len {
                        shown_names.push(self.read_name(r)?);
                    }
                    let offset = r.read_u32()?;
                    let end = r.read_i32()?;
                    out.push(NamespaceCombinator::Show {
                        shown_names,
                        offset,
                        end,
                    });
                }
                _ => return None,
            }
        }
        Some(out)
    }

    fn read_namespace(&self, r: &mut Reader<'_>) -> Option<Namespace> {
        let len = r.check_len(11)?;
        let mut defined_names = IndexMap::with_capacity(len);
        for _ in 0..len {
            let name = self.read_name(r)?;
            let elem = self.read_element_id(r)?;
            defined_names.insert(name, elem);
        }
        Some(Namespace { defined_names })
    }

    fn read_opt_namespace(&self, r: &mut Reader<'_>) -> Option<OnceSlot<Arc<Namespace>>> {
        match r.read_u8()? {
            0 => Some(OnceSlot::new()),
            1 => Some(OnceSlot::with(Arc::new(self.read_namespace(r)?))),
            _ => None,
        }
    }

    fn read_top_level_inference_error(
        &self,
        r: &mut Reader<'_>,
    ) -> Option<OnceSlot<TopLevelInferenceError>> {
        match r.read_u8()? {
            0 => Some(OnceSlot::new()),
            1 => {
                let len = r.check_len(4)?;
                let mut cycle = Vec::with_capacity(len);
                for _ in 0..len {
                    cycle.push(self.read_name(r)?);
                }
                Some(OnceSlot::with(TopLevelInferenceError::DependencyCycle {
                    cycle,
                }))
            }
            2 => Some(OnceSlot::with(
                TopLevelInferenceError::OverrideNoCombinedSuperSignature {
                    candidate_signatures: r.read_arc_str()?,
                },
            )),
            _ => None,
        }
    }

    fn read_element_data(&self, r: &mut Reader<'_>) -> Option<ElementData> {
        let name = self.read_opt_name(r)?;
        let library = self.read_opt_eid::<LibraryElement>(r)?;
        let enclosing = self.read_opt_element_id(r)?;
        let first_fragment = self.read_fragment_id(r)?;
        let flags = ElementFlagCell::new(ElementFlags::from_bits(r.read_u32()?));
        let previous_fragment_of_different_kind = self.read_opt_fragment_id(r)?;
        Some(ElementData {
            name,
            library,
            enclosing,
            first_fragment,
            flags,
            previous_fragment_of_different_kind,
        })
    }

    fn read_instance_element_data(&self, r: &mut Reader<'_>) -> Option<InstanceElementData> {
        let element = self.read_element_data(r)?;
        let type_params = self.read_eid_vec(r)?;
        let fields = self.read_eid_vec(r)?;
        let getters = self.read_eid_vec(r)?;
        let setters = self.read_eid_vec(r)?;
        let methods = self.read_eid_vec(r)?;
        Some(InstanceElementData {
            element,
            type_params,
            fields,
            getters,
            setters,
            methods,
        })
    }

    fn read_interface_element_data(&self, r: &mut Reader<'_>) -> Option<InterfaceElementData> {
        let instance = self.read_instance_element_data(r)?;
        let non_nullable_instance = opt_to_once_slot(self.read_opt_type_id(r)?);
        let nullable_instance = opt_to_once_slot(self.read_opt_type_id(r)?);
        let supertype = opt_to_var_slot(self.read_opt_type_id(r)?);
        let mixins = opt_to_var_slot(self.read_opt_type_list(r)?);
        let interfaces = opt_to_var_slot(self.read_opt_type_list(r)?);
        let this_type = opt_to_once_slot(self.read_opt_type_id(r)?);
        let interface_cycle = match r.read_u8()? {
            0 => OnceSlot::new(),
            1 => OnceSlot::with(self.read_eid_vec(r)?),
            _ => return None,
        };
        let all_supertypes = opt_to_once_slot(self.read_opt_type_list(r)?);
        let constructors = self.read_eid_vec(r)?;
        let has_non_final_field = BoolSlot::new(r.read_bool()?);
        Some(InterfaceElementData {
            instance,
            non_nullable_instance,
            nullable_instance,
            supertype,
            mixins,
            interfaces,
            this_type,
            interface_cycle,
            all_supertypes,
            constructors,
            has_non_final_field,
            inheritance: dartr_element::slot::ElementCache::new(),
        })
    }

    fn read_executable_element_data(&self, r: &mut Reader<'_>) -> Option<ExecutableElementData> {
        let element = self.read_element_data(r)?;
        let type_params = self.read_eid_vec(r)?;
        let formal_params = self.read_eid_vec(r)?;
        let return_type = opt_to_var_slot(self.read_opt_type_id(r)?);
        let type_ = opt_to_var_slot(self.read_opt_type_id(r)?);
        Some(ExecutableElementData {
            element,
            type_params,
            formal_params,
            return_type,
            type_,
        })
    }

    fn read_variable_element_data(&self, r: &mut Reader<'_>) -> Option<VariableElementData> {
        let element = self.read_element_data(r)?;
        let constant_initializer = match r.read_u8()? {
            0 => OnceSlot::new(),
            1 => OnceSlot::with(None),
            2 => {
                let fragment = self.read_fragment_id(r)?;
                let expression = ConstExprId(NodeId::from_raw(r.read_u32()?)?);
                OnceSlot::with(Some(ConstantInitializer {
                    fragment,
                    expression,
                }))
            }
            _ => return None,
        };
        Some(VariableElementData {
            element,
            constant_initializer,
        })
    }

    fn read_property_inducing_element_data(
        &self,
        r: &mut Reader<'_>,
    ) -> Option<PropertyInducingElementData> {
        let variable = self.read_variable_element_data(r)?;
        let getter = self.read_opt_eid(r)?;
        let setter = self.read_opt_eid(r)?;
        let type_ = opt_to_var_slot(self.read_opt_type_id(r)?);
        let type_inference_error = self.read_top_level_inference_error(r)?;
        Some(PropertyInducingElementData {
            variable,
            getter,
            setter,
            type_,
            type_inference_error,
        })
    }

    fn read_fragment_data(&self, r: &mut Reader<'_>) -> Option<FragmentData> {
        let name = self.read_opt_name(r)?;
        let name_offset = r.read_opt_u32()?;
        let element = opt_to_once_slot(self.read_opt_element_id(r)?);
        let enclosing_fragment = self.read_opt_fragment_id(r)?;
        let previous_fragment = self.read_opt_fragment_id(r)?;
        let next_fragment = self.read_opt_fragment_id(r)?;
        let first_token_offset = r.read_opt_u32()?;
        let flags = FragmentFlagCell::new(FragmentFlags::from_bits(r.read_u64()?));
        let documentation_comment = r.read_opt_arc_str()?;
        let metadata = self.read_metadata(r)?;
        let code_offset = r.read_opt_u32()?;
        let code_length = r.read_opt_u32()?;
        Some(FragmentData {
            name,
            name_offset,
            element,
            enclosing_fragment,
            previous_fragment,
            next_fragment,
            first_token_offset,
            flags,
            documentation_comment,
            metadata,
            code_offset,
            code_length,
        })
    }

    fn read_instance_fragment_data(&self, r: &mut Reader<'_>) -> Option<InstanceFragmentData> {
        let fragment = self.read_fragment_data(r)?;
        let type_params = self.read_fid_vec(r)?;
        let fields = self.read_fid_vec(r)?;
        let getters = self.read_fid_vec(r)?;
        let setters = self.read_fid_vec(r)?;
        let methods = self.read_fid_vec(r)?;
        Some(InstanceFragmentData {
            fragment,
            type_params,
            fields,
            getters,
            setters,
            methods,
        })
    }

    fn read_interface_fragment_data(&self, r: &mut Reader<'_>) -> Option<InterfaceFragmentData> {
        let instance = self.read_instance_fragment_data(r)?;
        let constructors = self.read_fid_vec(r)?;
        Some(InterfaceFragmentData {
            instance,
            constructors,
        })
    }

    fn read_executable_fragment_data(&self, r: &mut Reader<'_>) -> Option<ExecutableFragmentData> {
        let fragment = self.read_fragment_data(r)?;
        let type_params = self.read_fid_vec(r)?;
        let formal_params = self.read_fid_vec(r)?;
        Some(ExecutableFragmentData {
            fragment,
            type_params,
            formal_params,
        })
    }

    fn read_variable_fragment_data(&self, r: &mut Reader<'_>) -> Option<VariableFragmentData> {
        let fragment = self.read_fragment_data(r)?;
        let constant_initializer = match r.read_opt_u32()? {
            None => None,
            Some(raw) => Some(ConstExprId(NodeId::from_raw(raw)?)),
        };
        Some(VariableFragmentData {
            fragment,
            constant_initializer,
        })
    }

    fn read_property_inducing_fragment_data(
        &self,
        r: &mut Reader<'_>,
    ) -> Option<PropertyInducingFragmentData> {
        let variable = self.read_variable_fragment_data(r)?;
        let induced_getter = self.read_opt_fid(r)?;
        let induced_setter = self.read_opt_fid(r)?;
        Some(PropertyInducingFragmentData {
            variable,
            induced_getter,
            induced_setter,
        })
    }

    fn read_cycle_body(&self, r: &mut Reader<'_>, store: ElementStore) -> Option<LinkedCycle> {
        let mut const_ast_bytes = r.read_bytes()?;
        let const_ast = dartr_ast::Ast::decode(&mut const_ast_bytes)?;
        if !const_ast_bytes.is_empty() {
            return None;
        }
        let const_exprs = Arc::new(ConstExprs { ast: const_ast });

        let e = &store.elements;
        // 1. classes
        let len = r.check_len(20)?;
        for _ in 0..len {
            e.classes.push(ClassElement {
                interface: self.read_interface_element_data(r)?,
            });
        }
        // 2. enums
        let len = r.check_len(20)?;
        for _ in 0..len {
            e.enums.push(EnumElement {
                interface: self.read_interface_element_data(r)?,
            });
        }
        // 3. mixins
        let len = r.check_len(20)?;
        for _ in 0..len {
            let interface = self.read_interface_element_data(r)?;
            let superclass_constraints = opt_to_var_slot(self.read_opt_type_list(r)?);
            e.mixins.push(MixinElement {
                interface,
                superclass_constraints,
            });
        }
        // 4. extensions
        let len = r.check_len(20)?;
        for _ in 0..len {
            let instance = self.read_instance_element_data(r)?;
            let extended_type = opt_to_var_slot(self.read_opt_type_id(r)?);
            e.extensions.push(ExtensionElement {
                instance,
                extended_type,
            });
        }
        // 5. extension_types
        let len = r.check_len(20)?;
        for _ in 0..len {
            let interface = self.read_interface_element_data(r)?;
            let has_representation_self_reference = BoolSlot::new(r.read_bool()?);
            let has_implements_self_reference = BoolSlot::new(r.read_bool()?);
            let type_erasure = opt_to_once_slot(self.read_opt_type_id(r)?);
            e.extension_types.push(ExtensionTypeElement {
                interface,
                has_representation_self_reference,
                has_implements_self_reference,
                type_erasure,
            });
        }
        // 6. fields
        let len = r.check_len(15)?;
        for _ in 0..len {
            e.fields.push(FieldElement {
                property: self.read_property_inducing_element_data(r)?,
            });
        }
        // 7. getters
        let len = r.check_len(15)?;
        for _ in 0..len {
            let executable = self.read_executable_element_data(r)?;
            let variable = opt_to_var_slot(self.read_opt_eid(r)?);
            e.getters.push(GetterElement {
                accessor: PropertyAccessorElementData {
                    executable,
                    variable,
                },
            });
        }
        // 8. setters
        let len = r.check_len(15)?;
        for _ in 0..len {
            let executable = self.read_executable_element_data(r)?;
            let variable = opt_to_var_slot(self.read_opt_eid(r)?);
            e.setters.push(SetterElement {
                accessor: PropertyAccessorElementData {
                    executable,
                    variable,
                },
            });
        }
        // 9. methods
        let len = r.check_len(15)?;
        for _ in 0..len {
            let executable = self.read_executable_element_data(r)?;
            let is_operator_equal_with_parameter_type_from_object = BoolSlot::new(r.read_bool()?);
            let type_inference_error = self.read_top_level_inference_error(r)?;
            e.methods.push(MethodElement {
                executable,
                is_operator_equal_with_parameter_type_from_object,
                type_inference_error,
            });
        }
        // 10. constructors
        let len = r.check_len(15)?;
        for _ in 0..len {
            let executable = self.read_executable_element_data(r)?;
            let redirected_constructor = opt_to_var_slot(self.read_opt_elem_ref(r)?);
            let super_constructor = opt_to_var_slot(self.read_opt_elem_ref(r)?);
            e.constructors.push(ConstructorElement {
                executable,
                redirected_constructor,
                super_constructor,
            });
        }
        // 11. functions
        let len = r.check_len(15)?;
        for _ in 0..len {
            e.functions.push(TopLevelFunctionElement {
                executable: self.read_executable_element_data(r)?,
            });
        }
        // 12. variables
        let len = r.check_len(15)?;
        for _ in 0..len {
            e.variables.push(TopLevelVariableElement {
                property: self.read_property_inducing_element_data(r)?,
            });
        }
        // 13. type_aliases
        let len = r.check_len(15)?;
        for _ in 0..len {
            let element = self.read_element_data(r)?;
            let type_params = self.read_eid_vec(r)?;
            let aliased_type = opt_to_var_slot(self.read_opt_type_id(r)?);
            e.type_aliases.push(TypeAliasElement {
                element,
                type_params,
                aliased_type,
            });
        }
        // 14. type_params
        let len = r.check_len(15)?;
        for _ in 0..len {
            let element = self.read_element_data(r)?;
            let variance = read_opt_variance(r)?;
            let bound = opt_to_var_slot(self.read_opt_type_id(r)?);
            let default_type = opt_to_var_slot(self.read_opt_type_id(r)?);
            e.type_params.push(TypeParameterElement {
                element,
                variance,
                bound,
                default_type,
            });
        }
        // 15. params
        let len = r.check_len(15)?;
        for _ in 0..len {
            let variable = self.read_variable_element_data(r)?;
            let kind = read_parameter_kind(r)?;
            let type_ = opt_to_var_slot(self.read_opt_type_id(r)?);
            let base_formal_parameter = self.read_opt_eid(r)?;
            let field = opt_to_var_slot(self.read_opt_eid(r)?);
            e.params.push(FormalParameterElement {
                variable,
                kind,
                type_,
                base_formal_parameter,
                field,
            });
        }
        // 16. prefixes
        let len = r.check_len(15)?;
        for _ in 0..len {
            let element = self.read_element_data(r)?;
            let local_id = self.read_name(r)?;
            let last_fragment = self.read_fid(r)?;
            e.prefixes.push(PrefixElement {
                element,
                local_id,
                last_fragment,
            });
        }
        // 17. libraries
        let len = r.check_len(30)?;
        for _ in 0..len {
            let element = self.read_element_data(r)?;
            let metadata = self.read_metadata(r)?;
            let documentation_comment = r.read_opt_arc_str()?;
            let pkg_major = r.read_u32()?;
            let pkg_minor = r.read_u32()?;
            let override_ = match r.read_u8()? {
                0 => None,
                1 => Some(Version {
                    major: r.read_u32()?,
                    minor: r.read_u32()?,
                }),
                _ => return None,
            };
            let language_version = LibraryLanguageVersion {
                package: Version {
                    major: pkg_major,
                    minor: pkg_minor,
                },
                override_,
            };
            let feat_len = r.check_len(4)?;
            let mut feat_vec = Vec::with_capacity(feat_len);
            for _ in 0..feat_len {
                feat_vec.push(r.read_arc_str()?);
            }
            let feature_set = FeatureSet::new(feat_vec);
            let entry_point = match r.read_u8()? {
                0 => OnceSlot::new(),
                1 => OnceSlot::with(None),
                2 => OnceSlot::with(Some(self.read_eid::<TopLevelFunctionElement>(r)?)),
                _ => return None,
            };
            let load_library_function = opt_to_once_slot(self.read_opt_eid(r)?);
            let name_offset = r.read_i32()?;
            let name_length = r.read_u32()?;
            let classes = self.read_eid_vec(r)?;
            let enums = self.read_eid_vec(r)?;
            let extensions = self.read_eid_vec(r)?;
            let extension_types = self.read_eid_vec(r)?;
            let getters = self.read_eid_vec(r)?;
            let setters = self.read_eid_vec(r)?;
            let mixins = self.read_eid_vec(r)?;
            let top_level_functions = self.read_eid_vec(r)?;
            let top_level_variables = self.read_eid_vec(r)?;
            let type_aliases = self.read_eid_vec(r)?;
            let export_namespace = self.read_opt_namespace(r)?;
            let public_namespace = self.read_opt_namespace(r)?;
            let field_name_non_promotability_info = match r.read_u8()? {
                0 => OnceSlot::new(),
                1 => {
                    let map_len = r.check_len(16)?;
                    let mut map = IndexMap::with_capacity(map_len);
                    for _ in 0..map_len {
                        let name = self.read_name(r)?;
                        let conflicting_fields = self.read_eid_vec(r)?;
                        let conflicting_getters = self.read_eid_vec(r)?;
                        let conflicting_nsm_classes = self.read_eid_vec(r)?;
                        map.insert(
                            name,
                            FieldNameNonPromotabilityInfo {
                                conflicting_fields,
                                conflicting_getters,
                                conflicting_nsm_classes,
                            },
                        );
                    }
                    OnceSlot::with(map)
                }
                _ => return None,
            };
            e.libraries.push(LibraryElement {
                element,
                metadata,
                documentation_comment,
                language_version,
                feature_set,
                entry_point,
                load_library_function,
                name_offset,
                name_length,
                classes,
                enums,
                extensions,
                extension_types,
                getters,
                setters,
                mixins,
                top_level_functions,
                top_level_variables,
                type_aliases,
                export_namespace,
                public_namespace,
                field_name_non_promotability_info,
            });
        }
        // 18. generic_function_types
        let len = r.check_len(15)?;
        for _ in 0..len {
            let element = self.read_element_data(r)?;
            let type_params = self.read_eid_vec(r)?;
            let formal_params = self.read_eid_vec(r)?;
            let return_type = opt_to_var_slot(self.read_opt_type_id(r)?);
            let type_ = opt_to_var_slot(self.read_opt_type_id(r)?);
            e.generic_function_types.push(GenericFunctionTypeElement {
                element,
                type_params,
                formal_params,
                return_type,
                type_,
            });
        }
        // 19. locals
        let len = r.check_len(15)?;
        for _ in 0..len {
            let variable = self.read_variable_element_data(r)?;
            let type_ = opt_to_var_slot(self.read_opt_type_id(r)?);
            e.locals.push(LocalVariableElement { variable, type_ });
        }
        // 20. local_functions
        let len = r.check_len(15)?;
        for _ in 0..len {
            e.local_functions.push(LocalFunctionElement {
                executable: self.read_executable_element_data(r)?,
            });
        }
        // 21. labels
        let len = r.check_len(15)?;
        for _ in 0..len {
            e.labels.push(LabelElement {
                element: self.read_element_data(r)?,
            });
        }
        // 22. multiply_defined
        let len = r.check_len(15)?;
        for _ in 0..len {
            let element = self.read_element_data(r)?;
            let library_fragment = self.read_fid(r)?;
            let c_len = r.check_len(7)?;
            let mut conflicting_elements = Vec::with_capacity(c_len);
            for _ in 0..c_len {
                conflicting_elements.push(self.read_element_id(r)?);
            }
            e.multiply_defined.push(MultiplyDefinedElement {
                element,
                library_fragment,
                conflicting_elements,
            });
        }

        // Fragments
        let f = &store.fragments;
        // 1. classes
        let len = r.check_len(20)?;
        for _ in 0..len {
            f.classes.push(ClassFragment {
                interface: self.read_interface_fragment_data(r)?,
            });
        }
        // 2. enums
        let len = r.check_len(20)?;
        for _ in 0..len {
            f.enums.push(EnumFragment {
                interface: self.read_interface_fragment_data(r)?,
            });
        }
        // 3. mixins
        let len = r.check_len(20)?;
        for _ in 0..len {
            let interface = self.read_interface_fragment_data(r)?;
            let super_invoked_names = match r.read_u8()? {
                0 => OnceSlot::new(),
                1 => {
                    let n_len = r.check_len(4)?;
                    let mut names = Vec::with_capacity(n_len);
                    for _ in 0..n_len {
                        names.push(self.read_name(r)?);
                    }
                    OnceSlot::with(names)
                }
                _ => return None,
            };
            f.mixins.push(MixinFragment {
                interface,
                super_invoked_names,
            });
        }
        // 4. extensions
        let len = r.check_len(20)?;
        for _ in 0..len {
            f.extensions.push(ExtensionFragment {
                instance: self.read_instance_fragment_data(r)?,
            });
        }
        // 5. extension_types
        let len = r.check_len(20)?;
        for _ in 0..len {
            f.extension_types.push(ExtensionTypeFragment {
                interface: self.read_interface_fragment_data(r)?,
            });
        }
        // 6. fields
        let len = r.check_len(15)?;
        for _ in 0..len {
            let property = self.read_property_inducing_fragment_data(r)?;
            let inherits_covariant = BoolSlot::new(r.read_bool()?);
            f.fields.push(FieldFragment {
                property,
                inherits_covariant,
            });
        }
        // 7. getters
        let len = r.check_len(15)?;
        for _ in 0..len {
            let executable = self.read_executable_fragment_data(r)?;
            let inducing_variable = self.read_opt_fid(r)?;
            f.getters.push(GetterFragment {
                accessor: PropertyAccessorFragmentData {
                    executable,
                    inducing_variable,
                },
            });
        }
        // 8. setters
        let len = r.check_len(15)?;
        for _ in 0..len {
            let executable = self.read_executable_fragment_data(r)?;
            let inducing_variable = self.read_opt_fid(r)?;
            f.setters.push(SetterFragment {
                accessor: PropertyAccessorFragmentData {
                    executable,
                    inducing_variable,
                },
            });
        }
        // 9. methods
        let len = r.check_len(15)?;
        for _ in 0..len {
            f.methods.push(MethodFragment {
                executable: self.read_executable_fragment_data(r)?,
            });
        }
        // 10. constructors
        let len = r.check_len(15)?;
        for _ in 0..len {
            let executable = self.read_executable_fragment_data(r)?;
            let constant_initializers = match r.read_u8()? {
                0 => OnceSlot::new(),
                1 => {
                    let ci_len = r.check_len(4)?;
                    let mut inits = Vec::with_capacity(ci_len);
                    for _ in 0..ci_len {
                        inits.push(ConstExprId(NodeId::from_raw(r.read_u32()?)?));
                    }
                    OnceSlot::with(inits)
                }
                _ => return None,
            };
            let new_keyword_offset = r.read_opt_u32()?;
            let factory_keyword_offset = r.read_opt_u32()?;
            let type_name = self.read_opt_name(r)?;
            let type_name_offset = r.read_opt_u32()?;
            let period_offset = r.read_opt_u32()?;
            let name_end = r.read_opt_u32()?;
            let this_keyword_offset = r.read_opt_u32()?;
            f.constructors.push(ConstructorFragment {
                executable,
                constant_initializers,
                new_keyword_offset,
                factory_keyword_offset,
                type_name,
                type_name_offset,
                period_offset,
                name_end,
                this_keyword_offset,
            });
        }
        // 11. functions
        let len = r.check_len(15)?;
        for _ in 0..len {
            f.functions.push(TopLevelFunctionFragment {
                executable: self.read_executable_fragment_data(r)?,
            });
        }
        // 12. variables
        let len = r.check_len(15)?;
        for _ in 0..len {
            f.variables.push(TopLevelVariableFragment {
                property: self.read_property_inducing_fragment_data(r)?,
            });
        }
        // 13. type_aliases
        let len = r.check_len(15)?;
        for _ in 0..len {
            let fragment = self.read_fragment_data(r)?;
            let type_params = self.read_fid_vec(r)?;
            let has_self_reference = BoolSlot::new(r.read_bool()?);
            f.type_aliases.push(TypeAliasFragment {
                fragment,
                type_params,
                has_self_reference,
            });
        }
        // 14. type_params
        let len = r.check_len(15)?;
        for _ in 0..len {
            f.type_params.push(TypeParameterFragment {
                fragment: self.read_fragment_data(r)?,
            });
        }
        // 15. params
        let len = r.check_len(15)?;
        for _ in 0..len {
            let variable = self.read_variable_fragment_data(r)?;
            let parameter_kind = read_parameter_kind(r)?;
            let private_name = self.read_opt_name(r)?;
            f.params.push(FormalParameterFragment {
                variable,
                parameter_kind,
                private_name,
            });
        }
        // 16. prefixes
        let len = r.check_len(15)?;
        for _ in 0..len {
            let fragment = self.read_fragment_data(r)?;
            let offset = r.read_u32()?;
            let is_deferred = r.read_bool()?;
            f.prefixes.push(PrefixFragment {
                fragment,
                offset,
                is_deferred,
            });
        }
        // 17. units
        let len = r.check_len(30)?;
        for _ in 0..len {
            let fragment = self.read_fragment_data(r)?;
            let source = SourceRef {
                path: r.read_arc_str()?,
                uri: r.read_arc_str()?,
            };
            let ls_len = r.check_len(4)?;
            let mut ls = Vec::with_capacity(ls_len);
            for _ in 0..ls_len {
                ls.push(r.read_u32()?);
            }
            let line_starts: Arc<[u32]> = ls.into();
            let library = self.read_eid(r)?;
            let exp_len = r.check_len(15)?;
            let mut library_exports = Vec::with_capacity(exp_len);
            for _ in 0..exp_len {
                let library_fragment = self.read_fid(r)?;
                let uri = self.read_directive_uri(r)?;
                let metadata = self.read_metadata(r)?;
                let combinators = self.read_combinators(r)?;
                let export_keyword_offset = r.read_i32()?;
                library_exports.push(LibraryExport {
                    directive: ElementDirective {
                        library_fragment,
                        uri,
                        metadata,
                    },
                    combinators,
                    export_keyword_offset,
                });
            }
            let imp_len = r.check_len(15)?;
            let mut library_imports = Vec::with_capacity(imp_len);
            for _ in 0..imp_len {
                let library_fragment = self.read_fid(r)?;
                let uri = self.read_directive_uri(r)?;
                let metadata = self.read_metadata(r)?;
                let is_synthetic = r.read_bool()?;
                let combinators = self.read_combinators(r)?;
                let import_keyword_offset = r.read_i32()?;
                let prefix = self.read_opt_fid(r)?;
                let namespace = self.read_opt_namespace(r)?;
                library_imports.push(LibraryImport {
                    directive: ElementDirective {
                        library_fragment,
                        uri,
                        metadata,
                    },
                    is_synthetic,
                    combinators,
                    import_keyword_offset,
                    prefix,
                    namespace,
                });
            }
            let library_import_prefixes = self.read_eid_vec(r)?;
            let pref_map_len = r.check_len(11)?;
            let mut library_import_prefixes_by_id = IndexMap::with_capacity(pref_map_len);
            for _ in 0..pref_map_len {
                let name = self.read_name(r)?;
                let pref = self.read_eid(r)?;
                library_import_prefixes_by_id.insert(name, pref);
            }
            let parts_len = r.check_len(15)?;
            let mut parts = Vec::with_capacity(parts_len);
            for _ in 0..parts_len {
                let library_fragment = self.read_fid(r)?;
                let uri = self.read_directive_uri(r)?;
                let metadata = self.read_metadata(r)?;
                let part_keyword_offset = r.read_i32()?;
                parts.push(PartInclude {
                    directive: ElementDirective {
                        library_fragment,
                        uri,
                        metadata,
                    },
                    part_keyword_offset,
                });
            }
            let classes = self.read_fid_vec(r)?;
            let enums = self.read_fid_vec(r)?;
            let extensions = self.read_fid_vec(r)?;
            let extension_types = self.read_fid_vec(r)?;
            let functions = self.read_fid_vec(r)?;
            let getters = self.read_fid_vec(r)?;
            let setters = self.read_fid_vec(r)?;
            let mixins = self.read_fid_vec(r)?;
            let type_aliases = self.read_fid_vec(r)?;
            let variables = self.read_fid_vec(r)?;
            f.units.push(LibraryFragment {
                fragment,
                source,
                line_starts,
                library,
                library_exports,
                library_imports,
                library_import_prefixes,
                library_import_prefixes_by_id,
                parts,
                classes,
                enums,
                extensions,
                extension_types,
                functions,
                getters,
                setters,
                mixins,
                type_aliases,
                variables,
            });
        }
        // 18. generic_function_types
        let len = r.check_len(15)?;
        for _ in 0..len {
            let fragment = self.read_fragment_data(r)?;
            let type_params = self.read_fid_vec(r)?;
            let formal_params = self.read_fid_vec(r)?;
            let is_nullable = r.read_bool()?;
            f.generic_function_types.push(GenericFunctionTypeFragment {
                fragment,
                type_params,
                formal_params,
                is_nullable,
            });
        }
        // 19. locals
        let len = r.check_len(15)?;
        for _ in 0..len {
            let variable = self.read_variable_fragment_data(r)?;
            let join = opt_to_var_slot(self.read_opt_fid(r)?);
            let is_visiting_when_clause = BoolSlot::new(r.read_bool()?);
            let node = match r.read_opt_u32()? {
                None => None,
                Some(raw) => Some(NodeId::from_raw(raw)?),
            };
            let is_duplicate = BoolSlot::new(r.read_bool()?);
            let variables = self.read_fid_vec(r)?;
            let inconsistency = match r.read_u8()? {
                0 => VarSlot::new(),
                1 => {
                    let inc = match r.read_u8()? {
                        0 => JoinedPatternVariableInconsistency::None,
                        1 => JoinedPatternVariableInconsistency::LogicalOr,
                        2 => JoinedPatternVariableInconsistency::SharedCaseAbsent,
                        3 => JoinedPatternVariableInconsistency::SharedCaseHasLabel,
                        4 => JoinedPatternVariableInconsistency::DifferentFinalityOrType,
                        _ => return None,
                    };
                    VarSlot::with(inc)
                }
                _ => return None,
            };
            let refs_len = r.check_len(4)?;
            let mut refs = Vec::with_capacity(refs_len);
            for _ in 0..refs_len {
                refs.push(NodeId::from_raw(r.read_u32()?)?);
            }
            let pattern = PatternVariableFragmentData {
                join,
                is_visiting_when_clause,
                node,
                is_duplicate,
                variables,
                inconsistency,
                ..PatternVariableFragmentData::default()
            };
            *pattern.references.lock() = refs;
            f.locals.push(LocalVariableFragment { variable, pattern });
        }
        // 20. local_functions
        let len = r.check_len(15)?;
        for _ in 0..len {
            f.local_functions.push(LocalFunctionFragment {
                executable: self.read_executable_fragment_data(r)?,
            });
        }
        // 21. labels
        let len = r.check_len(15)?;
        for _ in 0..len {
            let fragment = self.read_fragment_data(r)?;
            let on_switch_member = r.read_bool()?;
            f.labels.push(LabelFragment {
                fragment,
                on_switch_member,
            });
        }
        // 22. multiply_defined
        let len = r.check_len(15)?;
        for _ in 0..len {
            f.multiply_defined.push(MultiplyDefinedFragment {
                fragment: self.read_fragment_data(r)?,
            });
        }

        // LinkedLibraries
        let libs_len = r.check_len(15)?;
        let mut libraries = Vec::with_capacity(libs_len);
        for _ in 0..libs_len {
            let uri = r.read_arc_str()?;
            let element = self.read_eid::<LibraryElement>(r)?;
            let exp_len = r.check_len(15)?;
            let mut export_entries = Vec::with_capacity(exp_len);
            for _ in 0..exp_len {
                let name = r.read_arc_str()?;
                let exp_elem = self.read_element_id(r)?;
                let loc_len = r.check_len(8)?;
                let mut locations = Vec::with_capacity(loc_len);
                for _ in 0..loc_len {
                    locations.push(ExportLocation {
                        fragment_index: r.read_u32()?,
                        export_index: r.read_u32()?,
                    });
                }
                export_entries.push(ExportEntry {
                    name,
                    element: exp_elem,
                    locations,
                });
            }
            let ref_uri = r.read_arc_str()?;
            let mut built_ins = [None; 2];
            for slot in &mut built_ins {
                *slot = match r.read_u8()? {
                    0 => None,
                    1 => Some(RefId(r.read_u32()?)),
                    _ => return None,
                };
            }
            let ref_len = r.check_len(2)?;
            let mut references = Vec::with_capacity(ref_len);
            for _ in 0..ref_len {
                let kind = match r.read_u8()? {
                    0 => ReferenceKind::Library,
                    1 => ReferenceKind::TopLevel {
                        kind: u8_to_top_level_ref_kind(r.read_u8()?)?,
                        key: r.read_arc_str()?,
                    },
                    2 => ReferenceKind::Member {
                        container: RefId(r.read_u32()?),
                        kind: u8_to_member_ref_kind(r.read_u8()?)?,
                        key: r.read_arc_str()?,
                    },
                    3 => ReferenceKind::BuiltIn(BuiltInReferenceKind::Dynamic),
                    4 => ReferenceKind::BuiltIn(BuiltInReferenceKind::Never),
                    _ => return None,
                };
                let element = self.read_opt_element_id(r)?;
                references.push(Reference { kind, element });
            }
            let reference = LibraryReference::from_raw_parts(
                ref_uri,
                references,
                Vec::new(),
                IndexMap::new(),
                built_ins,
            );
            libraries.push(Arc::new(LinkedLibrary {
                uri,
                element,
                export_entries,
                reference,
            }));
        }

        store.const_ast.set_once(ConstAst(const_exprs.clone()));
        Some(LinkedCycle {
            store: Arc::new(store),
            const_exprs,
            libraries,
        })
    }
}

#[inline]
fn opt_to_once_slot<T>(opt: Option<T>) -> OnceSlot<T> {
    match opt {
        Some(v) => OnceSlot::with(v),
        None => OnceSlot::new(),
    }
}

#[inline]
fn opt_to_var_slot<T: Copy>(opt: Option<T>) -> VarSlot<T> {
    match opt {
        Some(v) => VarSlot::with(v),
        None => VarSlot::new(),
    }
}

#[inline]
fn write_nullability(w: &mut Writer, n: Nullability) {
    let code = match n {
        Nullability::None => 0,
        Nullability::Question => 1,
        Nullability::Star => 2,
    };
    w.write_u8(code);
}

#[inline]
fn read_nullability(r: &mut Reader<'_>) -> Option<Nullability> {
    match r.read_u8()? {
        0 => Some(Nullability::None),
        1 => Some(Nullability::Question),
        2 => Some(Nullability::Star),
        _ => None,
    }
}

#[inline]
fn write_opt_variance(w: &mut Writer, v: Option<Variance>) {
    let code = match v {
        None => 0,
        Some(Variance::Unrelated) => 1,
        Some(Variance::Covariant) => 2,
        Some(Variance::Contravariant) => 3,
        Some(Variance::Invariant) => 4,
    };
    w.write_u8(code);
}

#[inline]
fn read_opt_variance(r: &mut Reader<'_>) -> Option<Option<Variance>> {
    match r.read_u8()? {
        0 => Some(None),
        1 => Some(Some(Variance::Unrelated)),
        2 => Some(Some(Variance::Covariant)),
        3 => Some(Some(Variance::Contravariant)),
        4 => Some(Some(Variance::Invariant)),
        _ => None,
    }
}

#[inline]
fn write_parameter_kind(w: &mut Writer, k: ParameterKind) {
    let code = match k {
        ParameterKind::Required => 0,
        ParameterKind::Positional => 1,
        ParameterKind::NamedRequired => 2,
        ParameterKind::Named => 3,
    };
    w.write_u8(code);
}

#[inline]
fn read_parameter_kind(r: &mut Reader<'_>) -> Option<ParameterKind> {
    match r.read_u8()? {
        0 => Some(ParameterKind::Required),
        1 => Some(ParameterKind::Positional),
        2 => Some(ParameterKind::NamedRequired),
        3 => Some(ParameterKind::Named),
        _ => None,
    }
}

#[inline]
fn top_level_ref_kind_to_u8(k: TopLevelReferenceKind) -> u8 {
    match k {
        TopLevelReferenceKind::Class => 0,
        TopLevelReferenceKind::Enum => 1,
        TopLevelReferenceKind::Extension => 2,
        TopLevelReferenceKind::ExtensionType => 3,
        TopLevelReferenceKind::Function => 4,
        TopLevelReferenceKind::Getter => 5,
        TopLevelReferenceKind::Mixin => 6,
        TopLevelReferenceKind::Setter => 7,
        TopLevelReferenceKind::TypeAlias => 8,
        TopLevelReferenceKind::TopLevelVariable => 9,
    }
}

#[inline]
fn u8_to_top_level_ref_kind(v: u8) -> Option<TopLevelReferenceKind> {
    match v {
        0 => Some(TopLevelReferenceKind::Class),
        1 => Some(TopLevelReferenceKind::Enum),
        2 => Some(TopLevelReferenceKind::Extension),
        3 => Some(TopLevelReferenceKind::ExtensionType),
        4 => Some(TopLevelReferenceKind::Function),
        5 => Some(TopLevelReferenceKind::Getter),
        6 => Some(TopLevelReferenceKind::Mixin),
        7 => Some(TopLevelReferenceKind::Setter),
        8 => Some(TopLevelReferenceKind::TypeAlias),
        9 => Some(TopLevelReferenceKind::TopLevelVariable),
        _ => None,
    }
}

#[inline]
fn member_ref_kind_to_u8(k: MemberReferenceKind) -> u8 {
    match k {
        MemberReferenceKind::Constructor => 0,
        MemberReferenceKind::Field => 1,
        MemberReferenceKind::Getter => 2,
        MemberReferenceKind::Method => 3,
        MemberReferenceKind::Setter => 4,
    }
}

#[inline]
fn u8_to_member_ref_kind(v: u8) -> Option<MemberReferenceKind> {
    match v {
        0 => Some(MemberReferenceKind::Constructor),
        1 => Some(MemberReferenceKind::Field),
        2 => Some(MemberReferenceKind::Getter),
        3 => Some(MemberReferenceKind::Method),
        4 => Some(MemberReferenceKind::Setter),
        _ => None,
    }
}
