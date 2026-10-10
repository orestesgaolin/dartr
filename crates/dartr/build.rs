// `dartr --version` shows the release version: the release workflow sets DARTR_RELEASE_VERSION
// to the tag without the leading `v` (for example 0.1.0-preview.4). Local builds show the crate
// version.
fn main() {
    println!("cargo:rerun-if-env-changed=DARTR_RELEASE_VERSION");
    let version = std::env::var("DARTR_RELEASE_VERSION")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| std::env::var("CARGO_PKG_VERSION").unwrap());
    println!("cargo:rustc-env=DARTR_VERSION={version}");
}
