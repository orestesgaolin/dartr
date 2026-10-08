use dartr_project::{AnalysisContextCollection, CollectionOptions, pubspec_validator};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "dartr-pubspec-validation-{}-{id}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, contents).unwrap();
        path
    }

    fn collection(&self) -> AnalysisContextCollection {
        AnalysisContextCollection::new(
            &[self.root.to_string_lossy().into_owned()],
            &CollectionOptions::default(),
        )
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn code_names(diagnostics: &[dartr_diagnostics::Diagnostic]) -> Vec<&str> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.name)
        .collect()
}

fn utf16_offset(text: &str, needle: &str) -> usize {
    text[..text.find(needle).unwrap()].encode_utf16().count()
}

#[test]
fn validates_fields_and_physical_paths_in_upstream_order() {
    let fixture = Fixture::new();
    std::fs::create_dir_all(fixture.root.join("local_dep")).unwrap();
    fixture.write("assets/2.0x/icon.png", "");
    let text = r#"description: 😀
name: 42
author: somebody
version: 1.0.0
dependencies:
  local:
    path: local_dep
  absent:
    path: missing_dep
dev_dependencies:
  local: any
flutter:
  assets:
    - assets/icon.png
    - path: 3
screenshots:
  - path: screenshots/missing.png
platforms:
  browser: chrome
workspace:
  - ../outside
"#;
    let pubspec = fixture.write("pubspec.yaml", text);
    let collection = fixture.collection();
    let diagnostics =
        pubspec_validator::validate_pubspec(&collection.contexts[0], pubspec.to_str().unwrap());

    assert_eq!(
        code_names(&diagnostics),
        [
            "path_pubspec_does_not_exist",
            "invalid_dependency",
            "path_does_not_exist",
            "invalid_dependency",
            "unnecessary_dev_dependency",
            "deprecated_field",
            "asset_not_string",
            "name_not_string",
            "path_does_not_exist",
            "unknown_platform",
            "platform_value_disallowed",
            "workspace_value_not_subdirectory",
        ]
    );
    let name_error = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.name == "name_not_string")
        .unwrap();
    assert_eq!(name_error.offset, utf16_offset(text, "42"));
    assert_eq!(name_error.length, 2);
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.name == "asset_does_not_exist")
    );
}

#[test]
fn applies_yaml_ignores_and_enabled_pubspec_lints() {
    let fixture = Fixture::new();
    fixture.write(
        "analysis_options.yaml",
        r#"linter:
  rules:
    - package_names
    - secure_pubspec_urls
    - sort_pub_dependencies
"#,
    );
    let text = r#"name: BadName
homepage: http://example.com
dependencies:
  zed: any
  alpha:
    # ignore: secure_pubspec_urls
    git: git://example.com/repo.git
  beta:
    git: git://example.com/repo.git # ignore: secure_pubspec_urls
"#;
    let pubspec = fixture.write("pubspec.yaml", text);
    let collection = fixture.collection();
    let diagnostics =
        pubspec_validator::validate_pubspec(&collection.contexts[0], pubspec.to_str().unwrap());

    assert_eq!(
        code_names(&diagnostics),
        [
            "package_names",
            "secure_pubspec_urls",
            "sort_pub_dependencies"
        ]
    );
    assert_eq!(
        diagnostics[1].message,
        "The 'http' protocol shouldn't be used because it isn't secure."
    );
    assert_eq!(diagnostics[2].offset, utf16_offset(text, "alpha:"));
}

#[test]
fn malformed_yaml_has_no_diagnostics() {
    let fixture = Fixture::new();
    let pubspec = fixture.write("pubspec.yaml", "name: [unterminated\n");
    let collection = fixture.collection();

    assert!(
        pubspec_validator::validate_pubspec(&collection.contexts[0], pubspec.to_str().unwrap(),)
            .is_empty()
    );
}

#[test]
fn computes_missing_dependency_edits_for_bulk_fixes() {
    let fixture = Fixture::new();
    let pubspec = fixture.write(
        "pubspec.yaml",
        r#"name: sample
dependencies:
  present: any
dev_dependencies:
  moved: any
  dev_present: any
"#,
    );
    let collection = fixture.collection();
    let used = ["sample", "flutter_gen", "present", "moved", "absent"].map(str::to_string);
    let used_dev = ["dev_present", "new_dev"].map(str::to_string);

    let result = pubspec_validator::validate_missing_dependencies(
        &collection.contexts[0],
        pubspec.to_str().unwrap(),
        &used,
        &used_dev,
    );

    assert_eq!(code_names(&result.diagnostics), ["missing_dependency"]);
    assert_eq!(
        result.data.unwrap(),
        pubspec_validator::MissingDependencyData {
            add_dependencies: vec!["moved".into(), "absent".into()],
            add_dev_dependencies: vec!["new_dev".into()],
            remove_dev_dependencies: vec!["moved".into()],
        }
    );
    assert_eq!(
        result.diagnostics[0].message,
        "Missing a dependency on imported packages 'moved', 'absent' in 'dependencies', and package 'new_dev' in 'dev_dependencies'."
    );
}
