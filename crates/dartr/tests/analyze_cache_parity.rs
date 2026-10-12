//! End-to-end integration tests for the Tier B persistent per-library
//! diagnostics cache (`--cache` / `DARTR_CACHE_DIR`) in `dartr analyze`.
//!
//! Verifies:
//! 1. Cold and warm cache runs on a fixture project produce output and exit
//!    codes identical to `dart analyze` (and no-cache `dartr analyze`).
//! 2. All 8 required invalidation / resilience mutations between two cached
//!    runs match `dart analyze`:
//!    (1) edit a library body,
//!    (2) change a dependency's API,
//!    (3) change `analysis_options.yaml` lints,
//!    (4) change `pubspec`/`package_config` (a package version),
//!    (5) delete a file,
//!    (6) add a part file,
//!    (7) corrupt a cache file,
//!    (8) different `dartr` binary salt (simulated via the salt input).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use dartr_cli::driver_provider::DriverProvider;
use serde_json::{Value, json};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "dartr_analyze_cache_parity_{label}_{}_{}",
            std::process::id(),
            id
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let canonical = dir.canonicalize().unwrap_or(dir);
        Self(canonical)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn find_dart() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    for candidate in [
        format!("{home}/fvm/default/bin/dart"),
        format!("{home}/fvm/versions/stable/bin/dart"),
    ] {
        let path = PathBuf::from(&candidate);
        if path.is_file() {
            return path;
        }
    }
    if let Ok(output) = Command::new("which").arg("dart").output()
        && output.status.success()
    {
        let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !s.is_empty() {
            return PathBuf::from(s);
        }
    }
    panic!("could not find `dart` executable");
}

fn dart_analyze_json(dart: &Path, dir: &Path) -> (i32, Value) {
    let output = Command::new(dart)
        .args(["analyze", "--format=json"])
        .arg(dir)
        .output()
        .expect("run dart analyze");
    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_line = stdout
        .lines()
        .find(|l| l.starts_with('{'))
        .unwrap_or_else(|| panic!("no JSON line in dart analyze output: {stdout}"));
    let value: Value = serde_json::from_str(json_line).expect("parse dart analyze JSON");
    (code, normalize_json(value))
}

fn dartr_analyze_json_with_provider(
    dir: &Path,
    cache_dir: Option<&Path>,
    provider: &DriverProvider,
) -> (i32, Value) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut args: Vec<String> = vec!["--format=json".to_string()];
    if let Some(cache) = cache_dir {
        args.push("--cache".to_string());
        args.push(cache.to_string_lossy().into_owned());
    }
    args.push(dir.to_string_lossy().into_owned());
    let code = dartr_cli::run(
        &args,
        provider,
        dartr_cli::Terminal::detect(),
        &mut out,
        &mut err,
    );
    assert!(
        err.is_empty(),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&err)
    );
    let stdout = String::from_utf8(out).unwrap();
    let value: Value = serde_json::from_str(stdout.trim()).expect("parse dartr analyze JSON");
    (code, normalize_json(value))
}

fn dartr_analyze_json(dir: &Path, cache_dir: Option<&Path>) -> (i32, Value) {
    dartr_analyze_json_with_provider(dir, cache_dir, &DriverProvider::new(None))
}

/// Normalizes the `diagnostics` array of `--format=json` so diagnostic order
/// differences do not hide or cause false mismatches.
fn normalize_json(mut value: Value) -> Value {
    if let Some(arr) = value.get_mut("diagnostics").and_then(Value::as_array_mut) {
        arr.sort_by_key(|a| a.to_string());
    }
    value
}

fn collect_resolved_cache_files(dir: &Path) -> Vec<PathBuf> {
    collect_cache_files_with_ext(dir, "resolved")
}

fn collect_unlinked_cache_files(dir: &Path) -> Vec<PathBuf> {
    collect_cache_files_with_ext(dir, "unlinked")
}

fn collect_cache_files_with_ext(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if let Ok(meta) = entry.metadata() {
            if meta.is_dir() {
                out.extend(collect_cache_files_with_ext(&path, ext));
            } else if meta.is_file() && path.extension().and_then(|e| e.to_str()) == Some(ext) {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Sets up a multi-file fixture workspace under `ws_root`:
/// - `ws_root/dep_v1/lib/dep.dart`
/// - `ws_root/dep_v2/lib/dep.dart`
/// - `ws_root/pkg/.dart_tool/package_config.json`
/// - `ws_root/pkg/pubspec.yaml`
/// - `ws_root/pkg/analysis_options.yaml`
/// - `ws_root/pkg/lib/helper.dart`
/// - `ws_root/pkg/lib/a.dart`
/// - `ws_root/pkg/lib/b.dart`
fn write_base_fixture(ws_root: &Path) -> PathBuf {
    let dep_v1_lib = ws_root.join("dep_v1").join("lib");
    let dep_v2_lib = ws_root.join("dep_v2").join("lib");
    let pkg_dir = ws_root.join("pkg");
    let dot_dart_tool = pkg_dir.join(".dart_tool");
    let pkg_lib = pkg_dir.join("lib");

    fs::create_dir_all(&dep_v1_lib).unwrap();
    fs::create_dir_all(&dep_v2_lib).unwrap();
    fs::create_dir_all(&dot_dart_tool).unwrap();
    fs::create_dir_all(&pkg_lib).unwrap();

    fs::write(
        dep_v1_lib.join("dep.dart"),
        "int computeDep(int x) => x + 1;\n",
    )
    .unwrap();
    fs::write(
        dep_v2_lib.join("dep.dart"),
        "String computeDep(String x) => x;\n",
    )
    .unwrap();

    let pkg_config = json!({
        "configVersion": 2,
        "packages": [
            {
                "name": "pkg",
                "rootUri": "../",
                "packageUri": "lib/",
                "languageVersion": "3.7"
            },
            {
                "name": "dep",
                "rootUri": "../../dep_v1",
                "packageUri": "lib/",
                "languageVersion": "3.7"
            }
        ]
    });
    fs::write(
        dot_dart_tool.join("package_config.json"),
        serde_json::to_string_pretty(&pkg_config).unwrap(),
    )
    .unwrap();

    fs::write(
        pkg_dir.join("pubspec.yaml"),
        "name: pkg\nversion: 1.0.0\nenvironment:\n  sdk: ^3.7.0\ndependencies:\n  dep: ^1.0.0\n",
    )
    .unwrap();

    fs::write(
        pkg_dir.join("analysis_options.yaml"),
        "linter:\n  rules:\n    - prefer_final_locals\n",
    )
    .unwrap();

    fs::write(pkg_lib.join("helper.dart"), "int helperValue() => 10;\n").unwrap();

    fs::write(
        pkg_lib.join("a.dart"),
        "import 'package:dep/dep.dart';\nimport 'helper.dart';\n\nint runA() {\n  var local = computeDep(helperValue());\n  return local;\n}\n",
    )
    .unwrap();

    fs::write(
        pkg_lib.join("b.dart"),
        "int runB() {\n  final int ok = 42;\n  return ok;\n}\n",
    )
    .unwrap();

    pkg_dir
}

#[test]
fn cold_and_warm_cache_match_dart_analyze() {
    let dart = find_dart();
    let ws = TempDir::new("cold_warm");
    let cache_dir = TempDir::new("cold_warm_cache");
    let pkg_dir = write_base_fixture(ws.path());

    let expected = dart_analyze_json(&dart, &pkg_dir);
    let no_cache = dartr_analyze_json(&pkg_dir, None);
    let cold = dartr_analyze_json(&pkg_dir, Some(cache_dir.path()));
    let resolved_files = collect_resolved_cache_files(cache_dir.path());
    assert!(
        !resolved_files.is_empty(),
        "expected cold run to populate .resolved cache entries"
    );
    let warm = dartr_analyze_json(&pkg_dir, Some(cache_dir.path()));

    assert_eq!(no_cache, expected, "no-cache run must match dart analyze");
    assert_eq!(cold, expected, "cold cache run must match dart analyze");
    assert_eq!(warm, expected, "warm cache run must match dart analyze");
    assert_eq!(cold, warm, "cold and warm cache runs must be identical");
}

#[test]
fn cache_invalidation_all_eight_mutations_match_dart_analyze() {
    let dart = find_dart();

    // (1) Edit a library body.
    {
        let ws = TempDir::new("mut1_body");
        let cache = TempDir::new("mut1_cache");
        let pkg_dir = write_base_fixture(ws.path());

        let run1 = dartr_analyze_json(&pkg_dir, Some(cache.path()));
        assert_eq!(run1, dart_analyze_json(&dart, &pkg_dir));

        fs::write(
            pkg_dir.join("lib").join("b.dart"),
            "int runB() {\n  final int broken = 'not_an_int';\n  return broken;\n}\n",
        )
        .unwrap();

        let expected = dart_analyze_json(&dart, &pkg_dir);
        let run2 = dartr_analyze_json(&pkg_dir, Some(cache.path()));
        assert_ne!(run1, run2, "body edit introducing error must change output");
        assert_eq!(
            run2, expected,
            "mutation (1) edit library body must match dart analyze"
        );
    }

    // (2) Change a dependency's API.
    {
        let ws = TempDir::new("mut2_dep_api");
        let cache = TempDir::new("mut2_cache");
        let pkg_dir = write_base_fixture(ws.path());

        let run1 = dartr_analyze_json(&pkg_dir, Some(cache.path()));
        assert_eq!(run1, dart_analyze_json(&dart, &pkg_dir));

        // Change `helperValue()` return type from `int` to `String` without
        // touching `lib/a.dart`. `lib/a.dart` passes `helperValue()` to
        // `computeDep(int)`, so `lib/a.dart` must be invalidated and report a
        // type error.
        fs::write(
            pkg_dir.join("lib").join("helper.dart"),
            "String helperValue() => 'ten';\n",
        )
        .unwrap();

        let expected = dart_analyze_json(&dart, &pkg_dir);
        let run2 = dartr_analyze_json(&pkg_dir, Some(cache.path()));
        assert_ne!(
            run1, run2,
            "dependency API change must change diagnostics in importer"
        );
        assert_eq!(
            run2, expected,
            "mutation (2) change dependency API must match dart analyze"
        );
    }

    // (3) Change `analysis_options.yaml` lints.
    {
        let ws = TempDir::new("mut3_lints");
        let cache = TempDir::new("mut3_cache");
        let pkg_dir = write_base_fixture(ws.path());

        let run1 = dartr_analyze_json(&pkg_dir, Some(cache.path()));
        assert_eq!(run1, dart_analyze_json(&dart, &pkg_dir));
        assert_eq!(
            run1.0, 0,
            "initial prefer_final_locals lint has info severity (exit 0)"
        );
        assert!(
            !run1.1["diagnostics"].as_array().unwrap().is_empty(),
            "expected prefer_final_locals diagnostic on run 1"
        );

        // Disable `prefer_final_locals` and enable `empty_catches`, and verify
        // cached `prefer_final_locals` is invalidated.
        fs::write(
            pkg_dir.join("analysis_options.yaml"),
            "linter:\n  rules:\n    - empty_catches\n",
        )
        .unwrap();

        let expected = dart_analyze_json(&dart, &pkg_dir);
        let run2 = dartr_analyze_json(&pkg_dir, Some(cache.path()));
        assert_ne!(run1, run2, "changing lints must change output");
        assert_eq!(
            run2, expected,
            "mutation (3) change analysis_options.yaml lints must match dart analyze"
        );
    }

    // (4) Change `pubspec` / `package_config` (a package version).
    {
        let ws = TempDir::new("mut4_pkg_config");
        let cache = TempDir::new("mut4_cache");
        let pkg_dir = write_base_fixture(ws.path());

        let run1 = dartr_analyze_json(&pkg_dir, Some(cache.path()));
        assert_eq!(run1, dart_analyze_json(&dart, &pkg_dir));

        // Point package `dep` to `dep_v2` (version 2.0.0) in package_config.json
        // and pubspec.yaml without editing `lib/a.dart`.
        let pkg_config_v2 = json!({
            "configVersion": 2,
            "packages": [
                {
                    "name": "pkg",
                    "rootUri": "../",
                    "packageUri": "lib/",
                    "languageVersion": "3.7"
                },
                {
                    "name": "dep",
                    "rootUri": "../../dep_v2",
                    "packageUri": "lib/",
                    "languageVersion": "3.7"
                }
            ]
        });
        fs::write(
            pkg_dir.join(".dart_tool").join("package_config.json"),
            serde_json::to_string_pretty(&pkg_config_v2).unwrap(),
        )
        .unwrap();
        fs::write(
            pkg_dir.join("pubspec.yaml"),
            "name: pkg\nversion: 1.0.0\nenvironment:\n  sdk: ^3.7.0\ndependencies:\n  dep: ^2.0.0\n",
        )
        .unwrap();

        let expected = dart_analyze_json(&dart, &pkg_dir);
        let run2 = dartr_analyze_json(&pkg_dir, Some(cache.path()));
        assert_ne!(
            run1, run2,
            "package_config version change must invalidate cache"
        );
        assert_eq!(
            run2, expected,
            "mutation (4) change pubspec/package_config must match dart analyze"
        );
    }

    // (5) Delete a file.
    {
        let ws = TempDir::new("mut5_delete_file");
        let cache = TempDir::new("mut5_cache");
        let pkg_dir = write_base_fixture(ws.path());

        let run1 = dartr_analyze_json(&pkg_dir, Some(cache.path()));
        assert_eq!(run1, dart_analyze_json(&dart, &pkg_dir));

        // Delete `lib/helper.dart` without modifying `lib/a.dart`.
        fs::remove_file(pkg_dir.join("lib").join("helper.dart")).unwrap();

        let expected = dart_analyze_json(&dart, &pkg_dir);
        let run2 = dartr_analyze_json(&pkg_dir, Some(cache.path()));
        assert_ne!(run1, run2, "deleting imported file must report errors");
        assert_eq!(
            run2, expected,
            "mutation (5) delete a file must match dart analyze"
        );
    }

    // (6) Add a part file.
    {
        let ws = TempDir::new("mut6_add_part");
        let cache = TempDir::new("mut6_cache");
        let pkg_dir = write_base_fixture(ws.path());

        // Also include a library `lib/c.dart` that already has `part 'c_part.dart';`
        // while `c_part.dart` does not exist yet on Run 1, then create `c_part.dart`
        // AND add a new `part 'b_part.dart';` to `lib/b.dart` before Run 2.
        fs::write(
            pkg_dir.join("lib").join("c.dart"),
            "part 'c_part.dart';\n\nint runC() => 1;\n",
        )
        .unwrap();

        let run1 = dartr_analyze_json(&pkg_dir, Some(cache.path()));
        assert_eq!(run1, dart_analyze_json(&dart, &pkg_dir));

        // Create `c_part.dart` (without touching `c.dart`) and add `b_part.dart` to `b.dart`.
        fs::write(
            pkg_dir.join("lib").join("c_part.dart"),
            "part of 'c.dart';\n\nint fromCPart() {\n  String bad = 123;\n  return bad.length;\n}\n",
        )
        .unwrap();
        fs::write(
            pkg_dir.join("lib").join("b.dart"),
            "part 'b_part.dart';\n\nint runB() => partValue();\n",
        )
        .unwrap();
        fs::write(
            pkg_dir.join("lib").join("b_part.dart"),
            "part of 'b.dart';\n\nint partValue() => 99;\n",
        )
        .unwrap();

        let expected = dart_analyze_json(&dart, &pkg_dir);
        let run2 = dartr_analyze_json(&pkg_dir, Some(cache.path()));
        assert_ne!(run1, run2, "adding part files must update diagnostics");
        assert_eq!(
            run2, expected,
            "mutation (6) add a part file must match dart analyze"
        );
    }

    // (7) Corrupt a cache file.
    {
        let ws = TempDir::new("mut7_corrupt");
        let cache = TempDir::new("mut7_cache");
        let pkg_dir = write_base_fixture(ws.path());

        let run1 = dartr_analyze_json(&pkg_dir, Some(cache.path()));
        let expected = dart_analyze_json(&dart, &pkg_dir);
        assert_eq!(run1, expected);

        let files = collect_resolved_cache_files(cache.path());
        assert!(!files.is_empty(), "expected resolved cache files to exist");
        for (i, path) in files.iter().enumerate() {
            if i % 2 == 0 {
                let mut raw = fs::read(path).unwrap();
                if let Some(last) = raw.last_mut() {
                    *last ^= 0xff;
                }
                fs::write(path, &raw).unwrap();
            } else {
                fs::write(path, b"corrupted_short").unwrap();
            }
        }

        let run2 = dartr_analyze_json(&pkg_dir, Some(cache.path()));
        assert_eq!(
            run2, expected,
            "mutation (7) corrupting cache files must recover cleanly and match dart analyze"
        );
        let files_after = collect_resolved_cache_files(cache.path());
        assert_eq!(
            files_after.len(),
            files.len(),
            "corrupted cache files must be replaced with valid entries"
        );
    }

    // (8) Different `dartr` binary salt (simulated via the salt input).
    {
        let ws = TempDir::new("mut8_salt");
        let cache = TempDir::new("mut8_cache");
        let pkg_dir = write_base_fixture(ws.path());

        let provider_salt_1 = DriverProvider::with_salt(None, Some([0x11u8; 16]));
        let provider_salt_2 = DriverProvider::with_salt(None, Some([0x22u8; 16]));

        let expected = dart_analyze_json(&dart, &pkg_dir);
        let run1 = dartr_analyze_json_with_provider(&pkg_dir, Some(cache.path()), &provider_salt_1);
        assert_eq!(run1, expected);

        let files_salt_1 = collect_resolved_cache_files(cache.path());
        assert!(!files_salt_1.is_empty());

        let run2 = dartr_analyze_json_with_provider(&pkg_dir, Some(cache.path()), &provider_salt_2);
        assert_eq!(
            run2, expected,
            "mutation (8) different binary salt must match dart analyze"
        );

        let files_salt_2 = collect_resolved_cache_files(cache.path());
        assert_eq!(
            files_salt_2.len(),
            files_salt_1.len() * 2,
            "different binary salt must miss old entries and write new salted entries"
        );
    }
}

#[test]
fn partial_hit_edit_one_library_others_hit_matches_dart_analyze() {
    let dart = find_dart();
    let ws = TempDir::new("partial_hit");
    let cache = TempDir::new("partial_hit_cache");
    let pkg_dir = write_base_fixture(ws.path());

    // Add an extra independent library `c.dart` that also has a lint diagnostic
    // so multiple libraries with and without diagnostics hit Tier A and Tier B.
    fs::write(
        pkg_dir.join("lib").join("c.dart"),
        "int runC() {\n  var cLocal = 7;\n  return cLocal;\n}\n",
    )
    .unwrap();

    // Cold run: populates Tier A (.unlinked) and Tier B (.resolved) for all files/libraries.
    let run1 = dartr_analyze_json(&pkg_dir, Some(cache.path()));
    let expected1 = dart_analyze_json(&dart, &pkg_dir);
    assert_eq!(run1, expected1, "cold run must match dart analyze");

    let unlinked_before = collect_unlinked_cache_files(cache.path());
    let resolved_before = collect_resolved_cache_files(cache.path());
    assert!(
        !unlinked_before.is_empty(),
        "expected cold run to populate .unlinked cache entries"
    );
    assert_eq!(
        resolved_before.len(),
        4,
        "expected 4 analyzed libraries (helper, a, b, c) in .resolved cache"
    );

    // Edit only `b.dart` (one leaf library); `helper.dart`, `a.dart`, `c.dart`,
    // `dep.dart`, and SDK files remain unchanged and must hit Tier A & Tier B.
    fs::write(
        pkg_dir.join("lib").join("b.dart"),
        "int runB() {\n  var nonFinal = 99;\n  final String bad = 123;\n  return nonFinal + bad.length;\n}\n",
    )
    .unwrap();

    let expected2 = dart_analyze_json(&dart, &pkg_dir);
    let run2 = dartr_analyze_json(&pkg_dir, Some(cache.path()));
    assert_ne!(run1, run2, "editing b.dart must change diagnostics");
    assert_eq!(
        run2, expected2,
        "partial-hit run after editing one library must match dart analyze"
    );

    let unlinked_after = collect_unlinked_cache_files(cache.path());
    let resolved_after = collect_resolved_cache_files(cache.path());
    assert_eq!(
        unlinked_after.len(),
        unlinked_before.len() + 1,
        "only the edited file should write a new .unlinked entry; all other files must hit Tier A"
    );
    assert_eq!(
        resolved_after.len(),
        resolved_before.len() + 1,
        "only the edited library should write a new .resolved entry; all other libraries must hit Tier B"
    );
}

#[test]
fn cached_unlinked_unit_used_when_library_misses_tier_b_matches_dart_analyze() {
    let dart = find_dart();

    // Case A: Warm Tier A (.unlinked) cache with all Tier B (.resolved) entries
    // removed. Every file hits Tier A in `discover()` (skipping parsing for
    // unlinked serialization), then every library misses Tier B, links and
    // resolves using the cached `UnlinkedUnit`s + lazy parsing, and matches
    // `dart analyze`.
    {
        let ws = TempDir::new("tier_a_only");
        let cache = TempDir::new("tier_a_only_cache");
        let pkg_dir = write_base_fixture(ws.path());

        let expected = dart_analyze_json(&dart, &pkg_dir);
        let run1 = dartr_analyze_json(&pkg_dir, Some(cache.path()));
        assert_eq!(run1, expected);

        let unlinked_before = collect_unlinked_cache_files(cache.path());
        let resolved_before = collect_resolved_cache_files(cache.path());
        assert!(!unlinked_before.is_empty());
        assert!(!resolved_before.is_empty());

        for path in &resolved_before {
            fs::remove_file(path).unwrap();
        }
        assert!(collect_resolved_cache_files(cache.path()).is_empty());

        let run2 = dartr_analyze_json(&pkg_dir, Some(cache.path()));
        assert_eq!(
            run2, expected,
            "run with warm .unlinked and missing .resolved must match dart analyze"
        );
        let unlinked_after = collect_unlinked_cache_files(cache.path());
        let resolved_after = collect_resolved_cache_files(cache.path());
        assert_eq!(
            unlinked_after, unlinked_before,
            "all files must hit existing .unlinked entries without writing new ones"
        );
        assert_eq!(
            resolved_after.len(),
            resolved_before.len(),
            "missing .resolved entries must be recomputed and restored"
        );
    }

    // Case B: Dependency API change invalidates Tier B for `a.dart` while
    // `a.dart` itself is unedited and hits its cached Tier A `UnlinkedUnit`.
    {
        let ws = TempDir::new("tier_a_dep_miss_b");
        let cache = TempDir::new("tier_a_dep_miss_b_cache");
        let pkg_dir = write_base_fixture(ws.path());

        let run1 = dartr_analyze_json(&pkg_dir, Some(cache.path()));
        assert_eq!(run1, dart_analyze_json(&dart, &pkg_dir));

        let unlinked_before = collect_unlinked_cache_files(cache.path());
        let resolved_before = collect_resolved_cache_files(cache.path());

        // Edit `helper.dart`'s API without touching `a.dart`.
        // `helper.dart` misses Tier A and Tier B; `a.dart` hits Tier A (using its
        // cached UnlinkedUnit) and misses Tier B (because its dependency cycle's
        // API signature changed); `b.dart` hits both Tier A and Tier B.
        fs::write(
            pkg_dir.join("lib").join("helper.dart"),
            "String helperValue() => 'changed';\n",
        )
        .unwrap();

        let expected2 = dart_analyze_json(&dart, &pkg_dir);
        let run2 = dartr_analyze_json(&pkg_dir, Some(cache.path()));
        assert_ne!(run1, run2);
        assert_eq!(
            run2, expected2,
            "library hitting Tier A while missing Tier B due to dependency API change must match dart analyze"
        );

        let unlinked_after = collect_unlinked_cache_files(cache.path());
        let resolved_after = collect_resolved_cache_files(cache.path());
        assert_eq!(
            unlinked_after.len(),
            unlinked_before.len() + 1,
            "only helper.dart should write a new .unlinked entry; a.dart must reuse its cached Tier A UnlinkedUnit"
        );
        assert_eq!(
            resolved_after.len(),
            resolved_before.len() + 2,
            "both helper.dart and a.dart must miss Tier B and write new .resolved entries"
        );
    }
}
