//! Port of `pkg/analyzer/lib/src/manifest/manifest_validator.dart`.

#[path = "manifest_values.rs"]
mod manifest_values;

use crate::{AnalysisContext, non_dart};
use dartr_diagnostics::{Diagnostic, LocatableDiagnostic, diag};
use manifest_values::*;
use std::collections::BTreeMap;

/// Validates an Android manifest using the effective options for [path].
///
/// The analysis server considers every analyzed file whose basename is
/// `AndroidManifest.xml`. The validator emits Chrome OS compatibility warnings
/// only when `analyzer.optional-checks.chrome-os-manifest-checks` is enabled.
pub fn validate_manifest(context: &AnalysisContext, path: &str) -> Vec<Diagnostic> {
    let options = non_dart::options_for_file(context, path);
    if !options.chrome_os_manifest_checks {
        return Vec::new();
    }
    // `File.readAsStringSync()` throws for malformed UTF-8. The analysis
    // server catches that exception and clears diagnostics for the file.
    let Some(content) = crate::fs::read_string_strict(path) else {
        return Vec::new();
    };
    validate_content(&content)
}

/// Validates manifest [content] with Chrome OS checks enabled.
///
/// This is public so differential tests can compare parser behavior without
/// constructing an analysis context.
pub fn validate_content(content: &str) -> Vec<Diagnostic> {
    let mut parser = ManifestParser::new(content);
    let manifest = loop {
        let result = parser.parse_xml_tag();
        if result
            .element
            .as_ref()
            .is_some_and(|e| e.name == MANIFEST_TAG)
        {
            break result.element.expect("element checked above");
        }
        if matches!(result.parse_result, ParseResult::Eof | ParseResult::Error) {
            return Vec::new();
        }
    };

    let features: Vec<&XmlElement> = manifest
        .children
        .iter()
        .filter(|element| element.name == USES_FEATURE_TAG)
        .collect();
    let permissions: Vec<&XmlElement> = manifest
        .children
        .iter()
        .filter(|element| element.name == USES_PERMISSION_TAG)
        .collect();

    let mut diagnostics = Vec::new();
    validate_touch_screen_feature(&features, &manifest, &mut diagnostics);
    validate_features(&features, &mut diagnostics);
    validate_permissions(&permissions, &features, &mut diagnostics);

    if let Some(application) = manifest
        .children
        .iter()
        .find(|element| element.name == APPLICATION_TAG)
    {
        for activity in application
            .children
            .iter()
            .filter(|element| element.name == ACTIVITY_TAG)
        {
            validate_activity(activity, &mut diagnostics);
        }
    }
    diagnostics
}

fn report_error_for_node(
    diagnostics: &mut Vec<Diagnostic>,
    node: &XmlElement,
    key: Option<&str>,
    diagnostic: LocatableDiagnostic,
) {
    let span = match key {
        Some(key) => node.attributes[key].span,
        None => node.span,
    };
    diagnostics.push(diagnostic.to_diagnostic(span.start, span.end - span.start));
}

fn validate_activity(activity: &XmlElement, diagnostics: &mut Vec<Diagnostic>) {
    let attributes = &activity.attributes;
    if let Some(attribute) = attributes.get(ATTRIBUTE_SCREEN_ORIENTATION)
        && UNSUPPORTED_ORIENTATIONS.contains(&attribute.value.as_str())
    {
        report_error_for_node(
            diagnostics,
            activity,
            Some(ATTRIBUTE_SCREEN_ORIENTATION),
            diag::setting_orientation_on_activity(),
        );
    }
    if attributes
        .get(ATTRIBUTE_RESIZABLE_ACTIVITY)
        .is_some_and(|attribute| attribute.value == "false")
    {
        report_error_for_node(
            diagnostics,
            activity,
            Some(ATTRIBUTE_RESIZABLE_ACTIVITY),
            diag::non_resizable_activity(),
        );
    }
}

fn validate_features(features: &[&XmlElement], diagnostics: &mut Vec<Diagnostic>) {
    for feature in features {
        let Some(name) = feature
            .attributes
            .get(ANDROID_NAME)
            .map(|a| a.value.as_str())
        else {
            continue;
        };
        if !UNSUPPORTED_HARDWARE_FEATURES.contains(&name) {
            continue;
        }
        match feature.attributes.get(ANDROID_REQUIRED) {
            None => report_error_for_node(
                diagnostics,
                feature,
                Some(ANDROID_NAME),
                diag::unsupported_chrome_os_hardware(name),
            ),
            Some(required) if required.value == "true" => report_error_for_node(
                diagnostics,
                feature,
                Some(ANDROID_NAME),
                diag::unsupported_chrome_os_feature(name),
            ),
            Some(_) => {}
        }
    }
}

fn validate_permissions(
    permissions: &[&XmlElement],
    features: &[&XmlElement],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for permission in permissions {
        let name = permission
            .attributes
            .get(ANDROID_NAME)
            .map(|a| a.value.as_str());
        if name == Some(ANDROID_PERMISSION_CAMERA) {
            let has_camera = has_feature(features, HARDWARE_FEATURE_CAMERA);
            let has_autofocus = has_feature(features, HARDWARE_FEATURE_CAMERA_AUTOFOCUS);
            if !has_camera || !has_autofocus {
                report_error_for_node(
                    diagnostics,
                    permission,
                    Some(ANDROID_NAME),
                    diag::camera_permissions_incompatible(),
                );
            }
        } else if let Some(feature_name) = get_implied_unsupported_hardware(name) {
            report_error_for_node(
                diagnostics,
                permission,
                Some(ANDROID_NAME),
                diag::permission_implies_unsupported_hardware(feature_name),
            );
        }
    }
}

fn has_feature(features: &[&XmlElement], name: &str) -> bool {
    features.iter().any(|feature| {
        feature
            .attributes
            .get(ANDROID_NAME)
            .is_some_and(|attribute| attribute.value == name)
    })
}

fn validate_touch_screen_feature(
    features: &[&XmlElement],
    manifest: &XmlElement,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let feature = features.iter().find(|feature| {
        feature
            .attributes
            .get(ANDROID_NAME)
            .is_some_and(|attribute| attribute.value == HARDWARE_FEATURE_TOUCHSCREEN)
    });
    if let Some(feature) = feature {
        match feature.attributes.get(ANDROID_REQUIRED) {
            None => report_error_for_node(
                diagnostics,
                feature,
                Some(ANDROID_NAME),
                diag::unsupported_chrome_os_hardware(HARDWARE_FEATURE_TOUCHSCREEN),
            ),
            Some(required) if required.value == "true" => report_error_for_node(
                diagnostics,
                feature,
                Some(ANDROID_NAME),
                diag::unsupported_chrome_os_feature(HARDWARE_FEATURE_TOUCHSCREEN),
            ),
            Some(_) => {}
        }
    } else {
        report_error_for_node(diagnostics, manifest, None, diag::no_touchscreen_feature());
    }
}

/// A rudimentary parser for the subset of XML used by manifest validation.
///
/// Offsets index UTF-16 code units, matching Dart `String` offsets and the
/// analyzer diagnostic protocol.
struct ManifestParser {
    content: Vec<u16>,
    pos: usize,
}

impl ManifestParser {
    fn new(content: &str) -> Self {
        Self {
            content: content.encode_utf16().collect(),
            pos: 0,
        }
    }

    fn is_closing(&self) -> bool {
        self.content.get(self.pos) == Some(&(b'>' as u16))
    }

    fn is_comment_closing(&self) -> bool {
        self.content.get(self.pos..self.pos + 3) == Some(&[b'-' as u16, b'-' as u16, b'>' as u16])
    }

    fn is_comment_opening(&self) -> bool {
        self.content.get(self.pos..self.pos + 4)
            == Some(&[b'<' as u16, b'!' as u16, b'-' as u16, b'-' as u16])
    }

    fn is_declaration_opening(&self) -> bool {
        self.content.get(self.pos + 1) == Some(&(b'!' as u16))
    }

    fn is_two_char_closing(&self) -> bool {
        matches!(self.content.get(self.pos), Some(c) if *c == b'?' as u16 || *c == b'/' as u16)
            && self.content.get(self.pos + 1) == Some(&(b'>' as u16))
    }

    fn is_whitespace(&self) -> bool {
        matches!(self.content.get(self.pos), Some(c) if matches!(*c, 0x09 | 0x0a | 0x0d | 0x20))
    }

    fn text(&self, start: usize, end: usize) -> String {
        String::from_utf16_lossy(&self.content[start..end])
    }

    fn parse_xml_tag(&mut self) -> ParseTagResult {
        while self.pos < self.content.len() && self.content[self.pos] != b'<' as u16 {
            self.pos += 1;
        }
        if self.pos >= self.content.len() {
            return ParseTagResult::new(ParseResult::Eof, None);
        }
        if self.is_comment_opening() {
            return self.parse_comment();
        }
        if self.is_declaration_opening() {
            return self.parse_declaration();
        }
        self.parse_normal_tag()
    }

    fn parse_any_whitespace(&mut self) -> Result<(), ()> {
        if self.pos >= self.content.len() {
            return Err(());
        }
        while self.is_whitespace() {
            self.pos += 1;
            if self.pos >= self.content.len() {
                return Err(());
            }
        }
        Ok(())
    }

    fn parse_attributes(
        &mut self,
        is_relevant: bool,
    ) -> Result<(bool, BTreeMap<String, XmlAttribute>), ()> {
        let mut attributes = BTreeMap::new();
        let is_empty_element;
        loop {
            if self.pos >= self.content.len() {
                return Err(());
            }
            if self.is_closing() {
                is_empty_element = false;
                break;
            } else if self.is_two_char_closing() {
                is_empty_element = true;
                self.pos += 1;
                break;
            } else if !self.is_whitespace() {
                return Err(());
            }

            self.parse_any_whitespace()?;
            if self.is_closing() {
                is_empty_element = false;
                break;
            } else if self.is_two_char_closing() {
                is_empty_element = true;
                self.pos += 1;
                break;
            }

            let attribute_name_pos = self.pos;
            self.pos += 1;
            if self.pos >= self.content.len() {
                return Err(());
            }
            while self.content[self.pos] != b'=' as u16 {
                if self.is_whitespace() || self.is_closing() || self.is_two_char_closing() {
                    return Err(());
                }
                self.pos += 1;
                if self.pos >= self.content.len() {
                    return Err(());
                }
            }
            let attribute_name =
                is_relevant.then(|| self.text(attribute_name_pos, self.pos).to_lowercase());
            self.pos += 1;
            if self.pos >= self.content.len() {
                return Err(());
            }

            let quote = self.content[self.pos];
            if quote != b'\'' as u16 && quote != b'"' as u16 {
                return Err(());
            }
            self.pos += 1;
            let attribute_value_pos = self.pos;
            while self.content.get(self.pos) != Some(&quote) {
                self.pos += 1;
                if self.pos >= self.content.len() {
                    return Err(());
                }
            }
            if let Some(name) = attribute_name {
                attributes.insert(
                    name,
                    XmlAttribute {
                        value: self.text(attribute_value_pos, self.pos),
                        span: Span {
                            start: attribute_name_pos,
                            end: self.pos,
                        },
                    },
                );
            }
            self.pos += 1;
        }
        Ok((is_empty_element, attributes))
    }

    fn parse_comment(&mut self) -> ParseTagResult {
        self.pos += 4;
        if self.pos >= self.content.len() {
            return ParseTagResult::new(ParseResult::Error, None);
        }
        while !self.is_comment_closing() {
            self.pos += 1;
            if self.pos >= self.content.len() {
                return ParseTagResult::new(ParseResult::Error, None);
            }
        }
        self.pos += 2;
        ParseTagResult::new(ParseResult::Element, None)
    }

    fn parse_declaration(&mut self) -> ParseTagResult {
        self.pos += 2;
        if self.pos >= self.content.len() {
            return ParseTagResult::new(ParseResult::Error, None);
        }
        while !self.is_closing() {
            self.pos += 1;
            if self.pos >= self.content.len() {
                return ParseTagResult::new(ParseResult::Error, None);
            }
        }
        ParseTagResult::new(ParseResult::Element, None)
    }

    fn parse_normal_tag(&mut self) -> ParseTagResult {
        let start_pos = self.pos;
        self.pos += 1;
        if self.pos >= self.content.len() || self.is_whitespace() {
            return ParseTagResult::new(ParseResult::Error, None);
        }

        let is_end_tag = self.content[self.pos] == b'/' as u16;
        if is_end_tag {
            self.pos += 1;
        }
        let mut closing_state = TagClosingState::NotClosed;
        let name_pos = self.pos;
        while !self.is_closing() && !self.is_two_char_closing() && !self.is_whitespace() {
            self.pos += 1;
            if self.pos >= self.content.len() {
                return ParseTagResult::new(ParseResult::Error, None);
            }
        }

        let name = self.text(name_pos, self.pos).to_lowercase();
        if self.is_closing() {
            closing_state = TagClosingState::Closed;
        } else if self.is_two_char_closing() {
            closing_state = TagClosingState::ClosedEmptyElement;
            self.pos += 1;
        }

        if is_end_tag {
            if self.parse_any_whitespace().is_err() {
                return ParseTagResult::new(ParseResult::Error, None);
            }
            return if self.is_closing() {
                ParseTagResult::new(ParseResult::EndTag, None)
            } else {
                ParseTagResult::new(ParseResult::Error, None)
            };
        }

        let is_relevant = is_relevant_element(&name);
        let (mut is_empty_element, attributes) = match closing_state {
            TagClosingState::NotClosed => match self.parse_attributes(is_relevant) {
                Ok(result) => result,
                Err(()) => return ParseTagResult::new(ParseResult::Error, None),
            },
            TagClosingState::Closed => (false, BTreeMap::new()),
            TagClosingState::ClosedEmptyElement => (true, BTreeMap::new()),
        };
        if name.starts_with('!') {
            is_empty_element = true;
        }

        let mut children = Vec::new();
        if !is_empty_element {
            self.pos += 1;
            loop {
                let child = self.parse_xml_tag();
                if child.parse_result == ParseResult::EndTag {
                    break;
                }
                if matches!(child.parse_result, ParseResult::Eof | ParseResult::Error) {
                    return child;
                }
                if let Some(element) = child.element {
                    children.push(element);
                    self.pos += 1;
                }
            }
        }

        if is_relevant {
            ParseTagResult::new(
                ParseResult::RelevantElement,
                Some(XmlElement {
                    name,
                    attributes,
                    children,
                    span: Span {
                        start: start_pos,
                        end: self.pos + 1,
                    },
                }),
            )
        } else {
            ParseTagResult::new(ParseResult::Element, None)
        }
    }
}

fn is_relevant_element(name: &str) -> bool {
    matches!(
        name,
        ACTIVITY_TAG | APPLICATION_TAG | MANIFEST_TAG | USES_FEATURE_TAG | USES_PERMISSION_TAG
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ParseResult {
    Element,
    EndTag,
    Eof,
    Error,
    RelevantElement,
}

struct ParseTagResult {
    parse_result: ParseResult,
    element: Option<XmlElement>,
}

impl ParseTagResult {
    fn new(parse_result: ParseResult, element: Option<XmlElement>) -> Self {
        Self {
            parse_result,
            element,
        }
    }
}

struct XmlAttribute {
    value: String,
    span: Span,
}

struct XmlElement {
    name: String,
    attributes: BTreeMap<String, XmlAttribute>,
    children: Vec<XmlElement>,
    span: Span,
}

#[derive(Clone, Copy)]
struct Span {
    start: usize,
    end: usize,
}

#[derive(Clone, Copy)]
enum TagClosingState {
    NotClosed,
    Closed,
    ClosedEmptyElement,
}
