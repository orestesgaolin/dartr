//! Integration coverage for the port of
//! `pkg/analyzer/lib/src/manifest/manifest_validator.dart`.

use dartr_project::manifest_validator::{validate_content, validate_manifest};
use dartr_project::{AnalysisContextCollection, CollectionOptions};
use std::path::{Path, PathBuf};

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
