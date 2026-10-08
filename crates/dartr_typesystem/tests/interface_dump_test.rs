// Dart source: tools/oracle/bin/interface.dart (oracle mode `interface`)

//! Differential test of `dartr_typesystem::interface_dump` against the
//! oracle: `tests/fixtures/interface/sample.dart` is built with the test
//! declaration builder (`support`) and dumped; the result must equal the
//! committed oracle output `sample.oracle.jsonl` byte for byte.
//!
//! The library URI is `file:///sample.dart` in both: the oracle output has
//! the absolute `file:` URI of the checkout replaced by it. A `file:` prefix
//! keeps the sort order of private names (`Name.toString()`).
//!
//! Regenerate the oracle output (from the repository root):
//!
//! ```sh
//! ROOT=$PWD F=crates/dartr_typesystem/tests/fixtures/interface
//! dart run tools/oracle/bin/oracle.dart interface "$ROOT/$F/sample.dart" \
//!   | sed -e "s#file://$ROOT/$F/sample.dart#file:///sample.dart#g" \
//!         -e "s#$ROOT/$F/sample.dart#sample.dart#g" > $F/sample.oracle.jsonl
//! ```

mod support;

use dartr_typesystem::interface_dump::interface_library_json;
use support::SourceTest;

const URI: &str = "file:///sample.dart";

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/interface");

#[test]
fn sample_matches_oracle() {
    let source = std::fs::read_to_string(format!("{FIXTURES}/sample.dart")).unwrap();
    let expected = std::fs::read_to_string(format!("{FIXTURES}/sample.oracle.jsonl")).unwrap();
    let r = SourceTest::with_files(&[(URI, &source)]);
    let actual = interface_library_json(&r.ctx(), "sample.dart", r.library);
    if actual != expected.trim_end() {
        // Show the first differing interface.
        let a: Vec<&str> = actual.split("{\"n\":").collect();
        let e: Vec<&str> = expected.trim_end().split("{\"n\":").collect();
        for (x, y) in a.iter().zip(&e) {
            assert_eq!(x, y, "first differing interface");
        }
        assert_eq!(a.len(), e.len(), "number of interfaces");
    }
}

#[test]
fn sample_is_deterministic() {
    let source = std::fs::read_to_string(format!("{FIXTURES}/sample.dart")).unwrap();
    let first = {
        let r = SourceTest::with_files(&[(URI, &source)]);
        interface_library_json(&r.ctx(), "sample.dart", r.library)
    };
    for _ in 0..3 {
        let r = SourceTest::with_files(&[(URI, &source)]);
        assert_eq!(
            interface_library_json(&r.ctx(), "sample.dart", r.library),
            first
        );
    }
}
