//! Integration coverage for the port of
//! `pkg/analyzer/lib/src/manifest/manifest_validator.dart`.

use dartr_project::manifest_validator::{validate_content, validate_manifest};
use dartr_project::{AnalysisContextCollection, CollectionOptions};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const UNSUPPORTED_FEATURES: &[&str] = &[
    "android.hardware.camera",
    "android.hardware.camera.autofocus",
    "android.hardware.camera.capability.manual_post_processing",
    "android.hardware.camera.capability.manual_sensor",
    "android.hardware.camera.capability.raw",
    "android.hardware.camera.flash",
    "android.hardware.camera.level.full",
    "android.hardware.consumerir",
    "android.hardware.location.gps",
    "android.hardware.nfc",
    "android.hardware.nfc.hce",
    "android.hardware.sensor.barometer",
    "android.hardware.telephony",
    "android.hardware.telephony.cdma",
    "android.hardware.telephony.gsm",
    "android.hardware.type.automotive",
    "android.hardware.type.television",
    "android.hardware.usb.accessory",
    "android.hardware.usb.host",
    "android.hardware.sensor.accelerometer",
    "android.hardware.sensor.compass",
    "android.hardware.sensor.gyroscope",
    "android.hardware.sensor.light",
    "android.hardware.sensor.proximity",
    "android.hardware.sensor.stepcounter",
    "android.hardware.sensor.stepdetector",
    "android.software.app_widgets",
    "android.software.device_admin",
    "android.software.home_screen",
    "android.software.input_methods",
    "android.software.leanback",
    "android.software.live_wallpaper",
    "android.software.live_tv",
    "android.software.managed_users",
    "android.software.midi",
    "android.software.sip",
    "android.software.sip.voip",
];

const IMPLIED_PERMISSIONS: &[&str] = &[
    "android.permission.CALL_PHONE",
    "android.permission.CALL_PRIVILEGED",
    "android.permission.MODIFY_PHONE_STATE",
    "android.permission.PROCESS_OUTGOING_CALLS",
    "android.permission.READ_SMS",
    "android.permission.RECEIVE_SMS",
    "android.permission.RECEIVE_MMS",
    "android.permission.RECEIVE_WAP_PUSH",
    "android.permission.SEND_SMS",
    "android.permission.WRITE_APN_SETTINGS",
    "android.permission.WRITE_SMS",
];

const UNSUPPORTED_ORIENTATIONS: &[&str] = &[
    "landscape",
    "portrait",
    "reverseLandscape",
    "reversePortrait",
    "sensorLandscape",
    "sensorPortrait",
    "userLandscape",
    "userPortrait",
];

struct TempProject(PathBuf);

impl TempProject {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dartr_manifest_differential_{}_{}",
            std::process::id(),
            unique
        ));
        fs::create_dir(&path).expect("create temporary project");
        Self(path.canonicalize().expect("canonicalize temporary project"))
    }

    fn write(&self, relative: &str, content: &str) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().expect("file has a parent"))
            .expect("create fixture directory");
        fs::write(&path, content).expect("write fixture file");
        path
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove temporary project");
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/diagnostics")
        .join(name)
}

fn utf16_offset(content: &str, needle: &str) -> usize {
    let byte_offset = content.find(needle).expect("fixture contains needle");
    content[..byte_offset].encode_utf16().count()
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

fn optional_touchscreen() -> &'static str {
    r#"<uses-feature android:name="android.hardware.touchscreen" android:required="false" />"#
}

fn dart_executable() -> PathBuf {
    if let Some(path) = std::env::var_os("DARTR_DART") {
        return PathBuf::from(path);
    }
    let sdk = dartr_project::sdk::find_sdk_path().expect("find the Dart SDK on PATH");
    Path::new(&sdk).join("bin/dart")
}

fn diagnostic_json(
    path: &str,
    code: &str,
    severity: &str,
    offset: u64,
    length: u64,
    message: &str,
    correction: Option<&str>,
) -> Value {
    json!({
        "file": path,
        "code": code,
        "severity": severity,
        "offset": offset,
        "length": length,
        "message": message,
        "correction": correction,
    })
}

fn sort_diagnostics(diagnostics: &mut [Value]) {
    diagnostics.sort_by_key(|diagnostic| {
        (
            diagnostic["file"].as_str().unwrap().to_owned(),
            diagnostic["offset"].as_u64().unwrap(),
            diagnostic["code"].as_str().unwrap().to_owned(),
            diagnostic.to_string(),
        )
    });
}

#[test]
fn reports_manifest_diagnostics_in_analyzer_order_and_on_attribute_spans() {
    let content = concat!(
        "<manifest xmlns:android=\"http://schemas.android.com/apk/res/android\">",
        "<uses-feature android:name=\"android.hardware.touchscreen\" android:required=\"false\" />",
        "<uses-feature android:name=\"android.hardware.camera\" />",
        "<uses-permission android:name=\"android.permission.CAMERA\" />",
        "<uses-permission android:name=\"android.permission.READ_SMS\" />",
        "<application><activity android:name=\"MainActivity\" ",
        "android:screenOrientation=\"portrait\" android:resizeableActivity=\"false\" />",
        "</application></manifest>",
    );

    let diagnostics = validate_content(content);
    let codes: Vec<_> = diagnostics.iter().map(|d| d.code.name).collect();
    assert_eq!(
        codes,
        [
            "unsupported_chrome_os_hardware",
            "camera_permissions_incompatible",
            "permission_implies_unsupported_hardware",
            "setting_orientation_on_activity",
            "non_resizable_activity",
        ]
    );

    let targets = [
        "android:name=\"android.hardware.camera",
        "android:name=\"android.permission.CAMERA",
        "android:name=\"android.permission.READ_SMS",
        "android:screenOrientation=\"portrait",
        "android:resizeableActivity=\"false",
    ];
    for (diagnostic, target) in diagnostics.iter().zip(targets) {
        assert_eq!(diagnostic.offset, utf16_offset(content, target));
        assert_eq!(diagnostic.length, utf16_len(target));
    }
}

#[test]
fn uses_utf16_offsets_and_silently_ignores_malformed_xml() {
    let content = concat!(
        "<!-- 👋 -->",
        "<manifest><uses-feature android:name=\"android.hardware.camera\" ",
        "android:required=\"true\" /></manifest>",
    );
    let diagnostics = validate_content(content);
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].code.name, "no_touchscreen_feature");
    assert_eq!(diagnostics[0].offset, 11);
    assert_eq!(diagnostics[1].code.name, "unsupported_chrome_os_feature");
    assert_eq!(diagnostics[1].offset, 35);
    assert_eq!(diagnostics[1].length, 37);

    assert!(validate_content("<manifest><uses-feature android:name=\"unterminated").is_empty());
}

#[test]
fn public_api_uses_effective_chrome_os_manifest_option() {
    let disabled_root = fixture("manifest_disabled");
    let disabled_manifest = disabled_root.join("AndroidManifest.xml");
    let collection = AnalysisContextCollection::new(
        &[disabled_root.to_string_lossy().into_owned()],
        &CollectionOptions::default(),
    );
    let context = collection
        .context_for(&disabled_manifest.to_string_lossy())
        .expect("manifest has an analysis context");
    assert!(validate_manifest(context, &disabled_manifest.to_string_lossy()).is_empty());

    let enabled_root = fixture("manifest_required");
    let enabled_manifest = enabled_root.join("AndroidManifest.xml");
    let collection = AnalysisContextCollection::new(
        &[enabled_root.to_string_lossy().into_owned()],
        &CollectionOptions::default(),
    );
    let context = collection
        .context_for(&enabled_manifest.to_string_lossy())
        .expect("manifest has an analysis context");
    let diagnostics = validate_manifest(context, &enabled_manifest.to_string_lossy());
    assert_eq!(
        diagnostics
            .iter()
            .map(|d| (d.code.name, d.offset, d.length))
            .collect::<Vec<_>>(),
        [
            ("no_touchscreen_feature", 0, 334),
            ("unsupported_chrome_os_hardware", 83, 37),
            ("camera_permissions_incompatible", 141, 39),
            ("setting_orientation_on_activity", 235, 35),
            ("non_resizable_activity", 272, 33),
        ]
    );
}

#[test]
fn all_manifest_values_match_pinned_dart_analyzer() {
    let project = TempProject::new();
    project.write(
        "analysis_options.yaml",
        "analyzer:\n  optional-checks:\n    chrome-os-manifest-checks: true\n",
    );
    project.write(
        "pubspec.yaml",
        "name: manifest_differential\nenvironment:\n  sdk: ^3.11.0\n",
    );
    project.write("lib/main.dart", "void main() {}\n");

    let mut manifests = Vec::new();
    let mut write_manifest = |folder: &str, content: String| {
        manifests.push(project.write(&format!("{folder}/AndroidManifest.xml"), &content));
    };

    let feature_tags = |required: Option<&str>| {
        let mut tags = String::from(optional_touchscreen());
        for feature in UNSUPPORTED_FEATURES {
            tags.push_str(&format!(
                "<uses-feature android:name=\"{feature}\"{} />",
                required
                    .map(|value| format!(" android:required=\"{value}\""))
                    .unwrap_or_default()
            ));
        }
        tags
    };
    write_manifest(
        "features_missing_required",
        format!("<!-- 👋 -->\n<manifest>{}</manifest>", feature_tags(None)),
    );
    write_manifest(
        "features_required_true",
        format!("<manifest>{}</manifest>", feature_tags(Some("true"))),
    );
    write_manifest(
        "features_optional_controls",
        format!(
            "<manifest>{}<uses-feature android:name=\"android.hardware.bluetooth\" /></manifest>",
            feature_tags(Some("false"))
        ),
    );

    let mut permission_tags = format!(
        "{}<uses-permission android:name=\"android.permission.CAMERA\" />",
        optional_touchscreen()
    );
    for permission in IMPLIED_PERMISSIONS {
        permission_tags.push_str(&format!(
            "<uses-permission android:name=\"{permission}\" />"
        ));
    }
    write_manifest(
        "permissions",
        format!("<manifest>{permission_tags}</manifest>"),
    );
    write_manifest(
        "permission_controls",
        format!(
            concat!(
                "<manifest>{}",
                "<uses-feature android:name=\"android.hardware.camera\" android:required=\"false\" />",
                "<uses-feature android:name=\"android.hardware.camera.autofocus\" android:required=\"false\" />",
                "<uses-permission android:name=\"android.permission.CAMERA\" />",
                "<uses-permission android:name=\"android.permission.INTERNET\" />",
                "</manifest>"
            ),
            optional_touchscreen()
        ),
    );

    let mut activities = String::new();
    for orientation in UNSUPPORTED_ORIENTATIONS {
        activities.push_str(&format!(
            "<activity android:screenOrientation=\"{orientation}\" />"
        ));
    }
    write_manifest(
        "orientations",
        format!(
            "<manifest>{}<application>{activities}</application></manifest>",
            optional_touchscreen()
        ),
    );
    write_manifest(
        "orientation_controls",
        format!(
            concat!(
                "<manifest>{}<application>",
                "<activity android:screenOrientation=\"unspecified\" />",
                "<activity android:screenOrientation=\"fullSensor\" />",
                "<activity android:resizeableActivity=\"true\" />",
                "</application></manifest>"
            ),
            optional_touchscreen()
        ),
    );
    write_manifest(
        "non_resizable",
        format!(
            concat!(
                "<manifest>{}<application>",
                "<activity android:resizeableActivity=\"false\" />",
                "</application></manifest>"
            ),
            optional_touchscreen()
        ),
    );

    write_manifest(
        "touchscreen_missing",
        "<!-- 👋 -->\n<manifest />".to_string(),
    );
    write_manifest(
        "touchscreen_required_missing",
        "<manifest><uses-feature android:name=\"android.hardware.touchscreen\" /></manifest>"
            .to_string(),
    );
    write_manifest(
        "touchscreen_required_true",
        concat!(
            "<manifest><uses-feature android:name=\"android.hardware.touchscreen\" ",
            "android:required=\"true\" /></manifest>"
        )
        .to_string(),
    );
    write_manifest(
        "parser_declarations",
        format!(
            concat!(
                "<?xml version=\"1.0\" encoding=\"utf-8\"?>",
                "<!DOCTYPE greeting SYSTEM \"hello.dtd\">",
                "<!-- parsed comment -->",
                "<manifest>{}</manifest>"
            ),
            optional_touchscreen()
        ),
    );

    for (name, content) in [
        ("unterminated_comment", "<!-- unterminated"),
        (
            "unterminated_attribute",
            "<manifest><uses-feature android:name=\"unterminated",
        ),
        ("unquoted_attribute", "<manifest android:name=value />"),
        ("whitespace_after_lt", "< manifest />"),
        ("crossed_tags", "<manifest><application></manifest>"),
    ] {
        write_manifest(name, content.to_string());
    }

    let dart = dart_executable();
    let dart_text = dart.to_string_lossy();
    let sdk_path = dartr_project::sdk::sdk_path_for_executable(&dart_text)
        .expect("Dart executable belongs to an SDK");
    let version =
        fs::read_to_string(Path::new(&sdk_path).join("version")).expect("read Dart SDK version");
    assert_eq!(version.trim(), "3.13.3", "test requires the pinned SDK");

    let output = Command::new(&dart)
        .args(["analyze", "--format=json"])
        .arg(&project.0)
        .output()
        .expect("run pinned Dart analyzer");
    assert!(
        matches!(output.status.code(), Some(0..=3)),
        "Dart analyzer failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let oracle_output: Value =
        serde_json::from_slice(&output.stdout).expect("parse Dart analyzer JSON output");
    let mut oracle: Vec<Value> = oracle_output["diagnostics"]
        .as_array()
        .expect("Dart diagnostics array")
        .iter()
        .filter(|diagnostic| {
            Path::new(diagnostic["location"]["file"].as_str().unwrap())
                .file_name()
                .is_some_and(|name| name == "AndroidManifest.xml")
        })
        .map(|diagnostic| {
            let start = diagnostic["location"]["range"]["start"]["offset"]
                .as_u64()
                .unwrap();
            let end = diagnostic["location"]["range"]["end"]["offset"]
                .as_u64()
                .unwrap();
            diagnostic_json(
                diagnostic["location"]["file"].as_str().unwrap(),
                diagnostic["code"].as_str().unwrap(),
                diagnostic["severity"].as_str().unwrap(),
                start,
                end - start,
                diagnostic["problemMessage"].as_str().unwrap(),
                diagnostic.get("correctionMessage").and_then(Value::as_str),
            )
        })
        .collect();

    let collection = AnalysisContextCollection::new(
        &[project.0.to_string_lossy().into_owned()],
        &CollectionOptions {
            sdk_path: Some(sdk_path),
            ..CollectionOptions::default()
        },
    );
    let mut actual = Vec::new();
    for manifest in manifests {
        let path = manifest.to_string_lossy();
        let context = collection
            .context_for(&path)
            .expect("manifest has an analysis context");
        for diagnostic in validate_manifest(context, &path) {
            actual.push(diagnostic_json(
                &path,
                diagnostic.code.name,
                diagnostic.severity.name(),
                diagnostic.offset as u64,
                diagnostic.length as u64,
                &diagnostic.message,
                diagnostic.correction.as_deref(),
            ));
        }
    }

    sort_diagnostics(&mut oracle);
    sort_diagnostics(&mut actual);
    assert_eq!(oracle.len(), 98, "oracle covers every manifest value");
    assert_eq!(actual, oracle);
}
