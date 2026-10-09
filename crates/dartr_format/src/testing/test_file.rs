// Dart source: dart_style lib/src/testing/test_file.dart
// Dart source: dart_style test/utils.dart (_versionedTestEntries, _validateFormat)

//! The `.unit` / `.stmt` test files of dart_style (`test/tall/**`,
//! `test/short/**`) and the logic of its test runner.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::dart_formatter::{DartFormatter, TrailingCommas};
use crate::dart_version_history::{DartVersionHistory, Version};
use crate::source_code::SourceCode;
use crate::text::utf16_len;

const UNSUPPORTED_MARKER: &str = "(unsupported)";

/// A file containing a series of formatting tests.
#[derive(Clone, Debug)]
pub struct TestFile {
    /// The path to the test file, relative to the `test/` directory.
    pub path: String,

    /// The page width for tests in this file or `None` if the default should
    /// be used.
    pub page_width: Option<usize>,

    /// The default options used by all tests in this file.
    pub options: TestOptions,

    /// The `###` comment lines at the beginning of the test file before any
    /// tests.
    pub comments: Vec<String>,

    /// The tests in this file.
    pub tests: Vec<FormatTest>,
}

/// A single formatting test inside a [TestFile].
#[derive(Clone, Debug)]
pub struct FormatTest {
    /// The 1-based index of the line where this test begins.
    pub line: usize,

    /// The options specific to this test.
    pub options: TestOptions,

    /// The unformatted input.
    pub input: TestEntry,

    /// The expected output by version (see dart_style `FormatTest.outputs`).
    pub outputs: BTreeMap<Version, TestEntry>,

    /// The language version at which point this test is no longer supported,
    /// or `None` if the test has no upper bound for language support.
    pub unsupported_version: Option<Version>,
}

impl FormatTest {
    /// The line and description of the test.
    pub fn label(&self) -> String {
        if self.input.description.is_empty() {
            format!("line {}", self.line)
        } else {
            format!("line {}: {}", self.line, self.input.description)
        }
    }
}

/// A single test input or output.
#[derive(Clone, Debug)]
pub struct TestEntry {
    /// Any remark on the "<<<" or ">>>" line.
    pub description: String,

    /// The `###` comment lines appearing after the header line before the code.
    pub comments: Vec<String>,

    pub code: SourceCode,
}

/// Options for configuring all tests in a file or an individual test.
#[derive(Clone, Debug, Default)]
pub struct TestOptions {
    /// The number of spaces of leading indentation that should be added to each
    /// line.
    pub leading_indent: Option<usize>,

    /// The trailing comma handling configuration.
    pub trailing_commas: Option<TrailingCommas>,

    /// Experiments that should be enabled when running this test.
    pub experiment_flags: Vec<String>,
}

/// Dart `File.readAsLinesSync` (`LineSplitter`): splits on `\n`, `\r\n` and
/// `\r`; a final line terminator does not start a new line.
fn split_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\n' => lines.push(std::mem::take(&mut current)),
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                lines.push(std::mem::take(&mut current));
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Dart `String.indexOf` with a UTF-16 result.
fn index_of_utf16(s: &str, pattern: char) -> Option<usize> {
    s.find(pattern).map(|byte| utf16_len(&s[..byte]))
}

/// Parses `>>> 1.2 description` / `<<< 3.7 description` headers.
fn parse_output_header(line: &str) -> Option<(Version, String)> {
    // Dart `RegExp(r'<<<\s*(\d+)\.(\d+)(.*)')` (firstMatch, not anchored at
    // the end).
    let start = line.find("<<<")?;
    let rest = line[start + 3..].trim_start();
    let major_end = rest.find(|c: char| !c.is_ascii_digit())?;
    if major_end == 0 {
        return None;
    }
    let major: u32 = rest[..major_end].parse().ok()?;
    let rest = rest[major_end..].strip_prefix('.')?;
    let minor_end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    if minor_end == 0 {
        return None;
    }
    let minor: u32 = rest[..minor_end].parse().ok()?;
    Some((Version::new(major, minor), rest[minor_end..].to_string()))
}

impl TestFile {
    /// Finds all test files in [dir] (recursively), sorted by path, with paths
    /// relative to [test_dir].
    pub fn list_directory(test_dir: &Path, name: &str) -> Result<Vec<TestFile>, String> {
        let mut entries = Vec::new();
        collect_files(&test_dir.join(name), &mut entries);
        entries.sort();
        let mut result = Vec::new();
        for entry in entries {
            let path = entry.to_string_lossy();
            if path.ends_with(".stmt") || path.ends_with(".unit") {
                let relative = entry
                    .strip_prefix(test_dir)
                    .unwrap_or(&entry)
                    .to_string_lossy()
                    .to_string();
                result.push(TestFile::load(&entry, relative)?);
            }
        }
        Ok(result)
    }

    /// Reads the test file from [file].
    pub fn load(file: &Path, relative_path: String) -> Result<TestFile, String> {
        let content = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
        TestFile::parse(&content, relative_path)
    }

    /// Parses the content of a test file.
    pub fn parse(content: &str, relative_path: String) -> Result<TestFile, String> {
        let lines = split_lines(content);
        let is_compilation_unit = relative_path.ends_with(".unit");

        // The first line may have a "|" to indicate the page width.
        let mut i = 0;
        let mut page_width = None;
        if lines[i].ends_with('|') {
            page_width = index_of_utf16(&lines[i], '|');
            i += 1;
        }

        // Optional line to configure options for all tests in the file.
        let file_options;
        if !lines[i].starts_with("###") && !lines[i].starts_with(">>>") {
            file_options = parse_options(&lines[i]).0;
            i += 1;
        } else {
            file_options = TestOptions::default();
        }

        let mut tests = Vec::new();

        let read_comments = |i: &mut usize| {
            let mut comments = Vec::new();
            while *i < lines.len() && lines[*i].starts_with("###") {
                comments.push(lines[*i].clone());
                *i += 1;
            }
            comments
        };

        let file_comments = read_comments(&mut i);

        while i < lines.len() {
            let line_number = i + 1;
            let line = lines[i].replace(">>>", "");
            i += 1;
            let (options, description) = parse_options(&line);
            let description = description.trim().to_string();

            let input_comments = read_comments(&mut i);
            let mut input_buffer = String::new();
            while i < lines.len() && !lines[i].starts_with("<<<") {
                input_buffer.push_str(&lines[i]);
                input_buffer.push('\n');
                i += 1;
            }

            let input_code =
                extract_selection(&unescape_unicode(&input_buffer), is_compilation_unit);
            let input = TestEntry {
                description,
                comments: input_comments,
                code: input_code,
            };

            // Read the outputs. A single test should have outputs in one of two
            // forms:
            //
            // - One single unversioned output which is the expected output across
            //   all supported versions.
            // - One or more versioned outputs, each of which defines the expected
            //   output at that language version or later until reaching the next
            //   output's version.
            let fail = |error: &str| {
                format!("Test format error in {relative_path}, line {line_number}: {error}")
            };

            let mut outputs = BTreeMap::new();
            let mut unsupported_version = None;
            while i < lines.len() && lines[i].starts_with("<<<") {
                let (output_version, output_description) =
                    parse_output_header(&lines[i]).ok_or_else(|| fail("Bad output header."))?;
                i += 1;
                let output_description = output_description.trim().to_string();

                if output_description.ends_with(UNSUPPORTED_MARKER) {
                    unsupported_version = Some(output_version);

                    // There should not be any lines after an (unsupported) marker.
                    if i < lines.len()
                        && !lines[i].starts_with(">>>")
                        && !lines[i].starts_with("<<<")
                    {
                        return Err(fail(
                            "Shouldn't have any output lines after \"(unsupported)\".",
                        ));
                    }
                    continue;
                }

                let output_comments = read_comments(&mut i);

                let mut output_buffer = String::new();
                while i < lines.len()
                    && !lines[i].starts_with(">>>")
                    && !lines[i].starts_with("<<<")
                {
                    output_buffer.push_str(&lines[i]);
                    output_buffer.push('\n');
                    i += 1;
                }

                // The output always has a trailing newline. When formatting a
                // statement, the formatter (correctly) doesn't output trailing
                // newlines when formatting a statement, so remove it from the
                // expectation to match.
                let mut output_text = output_buffer;
                if !is_compilation_unit {
                    output_text.pop();
                }
                let output_code =
                    extract_selection(&unescape_unicode(&output_text), is_compilation_unit);

                if outputs.contains_key(&output_version) {
                    return Err(fail(&format!(
                        "Multiple outputs with the same version {output_version}."
                    )));
                }

                outputs.insert(
                    output_version,
                    TestEntry {
                        description: output_description,
                        comments: output_comments,
                        code: output_code,
                    },
                );
            }

            if outputs.is_empty() {
                return Err(fail("Test must have at least one output."));
            }

            tests.push(FormatTest {
                line: line_number,
                options,
                input,
                outputs,
                unsupported_version,
            });
        }

        Ok(TestFile {
            path: relative_path,
            page_width,
            options: file_options,
            comments: file_comments,
            tests,
        })
    }

    pub fn is_compilation_unit(&self) -> bool {
        self.path.ends_with(".unit")
    }

    /// Whether the test uses the tall or short style.
    pub fn is_tall(&self) -> bool {
        Path::new(&self.path)
            .components()
            .any(|c| c.as_os_str() == "tall")
    }

    /// Creates a [DartFormatter] configured with all of the options that should
    /// be applied for [test] in this test file.
    ///
    /// If [version] is given, then it specifies the language version to run the
    /// test at. Otherwise, the test's default version is used.
    pub fn formatter_for_test(&self, test: &FormatTest, version: Option<Version>) -> DartFormatter {
        let default_language_version = if self.is_tall() {
            DartFormatter::LATEST_LANGUAGE_VERSION
        } else {
            DartFormatter::LATEST_SHORT_STYLE_LANGUAGE_VERSION
        };

        let mut experiment_flags = self.options.experiment_flags.clone();
        experiment_flags.extend(test.options.experiment_flags.iter().cloned());

        DartFormatter {
            language_version: version.unwrap_or(default_language_version),
            line_ending: None,
            page_width: self.page_width.unwrap_or(DartFormatter::DEFAULT_PAGE_WIDTH),
            indent: test
                .options
                .leading_indent
                .or(self.options.leading_indent)
                .unwrap_or(0),
            trailing_commas: test
                .options
                .trailing_commas
                .or(self.options.trailing_commas)
                .unwrap_or(TrailingCommas::Automate),
            experiment_flags,
        }
    }

    /// Dart `_testFile` (`test/utils.dart`): the language versions to run
    /// [test] at, with the expected output at each version.
    pub fn versions_to_test<'a>(&self, test: &'a FormatTest) -> Vec<(Version, &'a TestEntry)> {
        // Find the upper end of the range of versions to test.
        let upper_bound = match test.unsupported_version {
            Some(unsupported) => DartVersionHistory::before(unsupported),
            None if self.is_tall() => DartVersionHistory::LATEST,
            None => DartVersionHistory::LATEST_SHORT_STYLE,
        };
        versioned_test_entries(&test.outputs, upper_bound)
    }
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_files(&path, out);
        } else if file_type.is_file() {
            out.push(path);
        }
    }
}

/// Dart `_versionedTestEntries` (`test/utils.dart`): given a set of versions
/// that have specific test expectations, determine what set of versions
/// should be tested against which outputs.
///
/// For each output, test the language version at each end of the range it
/// covers.
pub fn versioned_test_entries(
    expected_outputs: &BTreeMap<Version, TestEntry>,
    upper_bound: Version,
) -> Vec<(Version, &TestEntry)> {
    let output_versions: Vec<Version> = expected_outputs.keys().copied().collect();
    // Dart uses an insertion-ordered map: a later insert of the same key
    // replaces the value but keeps the position.
    let mut tested: Vec<(Version, &TestEntry)> = Vec::new();
    fn put<'a>(tested: &mut Vec<(Version, &'a TestEntry)>, version: Version, entry: &'a TestEntry) {
        if let Some(slot) = tested.iter_mut().find(|(v, _)| *v == version) {
            slot.1 = entry;
        } else {
            tested.push((version, entry));
        }
    }
    for (i, &version) in output_versions.iter().enumerate() {
        // The output specifies the low end of its range.
        put(&mut tested, version, &expected_outputs[&version]);

        // Find the version at the high end of the range.
        let range_end = if i < output_versions.len() - 1 {
            // The end of this version's range is one version lower than the next
            // output's version.
            DartVersionHistory::before(output_versions[i + 1])
        } else {
            // This is the last specified version, so it's range goes up to the
            // upper bound.
            upper_bound
        };

        put(&mut tested, range_end, &expected_outputs[&version]);
    }
    tested
}

/// Parses all of the test option syntax like `(indent 3)` from [line].
///
/// Returns the options and the text remaining on the line after the options
/// are removed.
fn parse_options(line: &str) -> (TestOptions, String) {
    let mut line = line.to_string();

    // Let the test specify a leading indentation. This is handy for
    // regression tests which often come from a chunk of nested code.
    // Dart `RegExp(r'\(indent (\d+)\)')`.
    let mut leading_indent = None;
    while let Some((start, end, value)) = find_option(&line, "(indent ", |c| c.is_ascii_digit()) {
        leading_indent = value.parse().ok();
        line.replace_range(start..end, "");
    }

    // Let the test enable experiments for features that are supported but not
    // released yet. Dart `RegExp(r'\(experiment ([a-z-]+)\)')`.
    let mut experiments = Vec::new();
    while let Some((start, end, value)) = find_option(&line, "(experiment ", |c| {
        c.is_ascii_lowercase() || c == '-'
    }) {
        experiments.push(value);
        line.replace_range(start..end, "");
    }

    let mut trailing_commas = None;
    while let Some(start) = line.find("(trailing_commas preserve)") {
        trailing_commas = Some(TrailingCommas::Preserve);
        line.replace_range(start..start + "(trailing_commas preserve)".len(), "");
    }

    (
        TestOptions {
            leading_indent,
            trailing_commas,
            experiment_flags: experiments,
        },
        line,
    )
}

/// Finds `<prefix><one or more chars matching accept>)` in [line]. Returns
/// the byte range of the match and the captured value.
fn find_option(
    line: &str,
    prefix: &str,
    accept: impl Fn(char) -> bool,
) -> Option<(usize, usize, String)> {
    let mut search = 0;
    while let Some(found) = line[search..].find(prefix) {
        let start = search + found;
        let value_start = start + prefix.len();
        let value_len = line[value_start..]
            .chars()
            .take_while(|&c| accept(c))
            .map(char::len_utf8)
            .sum::<usize>();
        if value_len > 0 && line[value_start + value_len..].starts_with(')') {
            let end = value_start + value_len + 1;
            return Some((
                start,
                end,
                line[value_start..value_start + value_len].to_string(),
            ));
        }
        search = start + 1;
    }
    None
}

/// Given a source string that contains ‹ and › to indicate a selection, returns
/// a [SourceCode] with the text (with the selection markers removed) and the
/// correct selection range.
fn extract_selection(source: &str, is_compilation_unit: bool) -> SourceCode {
    let start = index_of_utf16(source, '‹');
    let source = source.replace('‹', "");

    let end = index_of_utf16(&source, '›');
    let source = source.replace('›', "");

    SourceCode {
        uri: None,
        text: source,
        is_compilation_unit,
        selection_start: start,
        selection_length: match (start, end) {
            (_, None) => None,
            (Some(start), Some(end)) => Some(end - start),
            (None, Some(_)) => None,
        },
    }
}

/// Turn the special Unicode escape marker syntax used in the tests into real
/// Unicode characters.
///
/// This does not use Dart's own string escape sequences so that we don't
/// accidentally modify the Dart code being formatted.
fn unescape_unicode(input: &str) -> String {
    // Dart `RegExp(r'×([0-9a-fA-F]{2,4})')`.
    let mut result = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(index) = rest.find('×') {
        result.push_str(&rest[..index]);
        let after = &rest[index + '×'.len_utf8()..];
        let hex_len = after
            .bytes()
            .take(4)
            .take_while(|b| b.is_ascii_hexdigit())
            .count();
        if hex_len >= 2 {
            let code_point = u32::from_str_radix(&after[..hex_len], 16).unwrap();
            // Dart `String.fromCharCode`: a lone surrogate cannot be
            // represented in Rust; use the replacement character.
            result.push(char::from_u32(code_point).unwrap_or('\u{FFFD}'));
            rest = &after[hex_len..];
        } else {
            result.push('×');
            rest = after;
        }
    }
    result.push_str(rest);
    result
}

/// The result of running one test at one version.
#[derive(Clone, Debug)]
pub struct TestFailure {
    pub file: String,
    pub label: String,
    pub version: Version,
    pub message: String,
}

/// Dart `_runTestAtVersion` + `_validateFormat`: formats the input, compares
/// it with the expected output (text and selection), then checks that
/// formatting the output again does not change it.
pub fn run_test_at_version(
    file: &TestFile,
    test: &FormatTest,
    output: &TestEntry,
    version: Version,
) -> Result<(), String> {
    let formatter = file.formatter_for_test(test, Some(version));
    let actual = validate_format(
        &formatter,
        &test.input.code,
        &output.code,
        "did not match expectation",
    )?;

    // Make sure that formatting is idempotent. Format the output and make
    // sure we get the same result.
    validate_format(&formatter, &actual, &actual, "was not idempotent")?;
    Ok(())
}

fn validate_format(
    formatter: &DartFormatter,
    input: &SourceCode,
    expected: &SourceCode,
    reason: &str,
) -> Result<SourceCode, String> {
    let actual = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        formatter.format_source(input)
    }));
    let actual = match actual {
        Ok(Ok(actual)) => actual,
        Ok(Err(error)) => return Err(format!("Formatting {reason}: error: {error}")),
        Err(panic) => {
            let message = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            return Err(format!("Formatting {reason}: panic: {message}"));
        }
    };
    if actual.text != expected.text {
        return Err(format!(
            "Formatting {reason}. Expected:\n{}\nActual:\n{}",
            expected.text, actual.text
        ));
    } else if actual.selection_start != expected.selection_start
        || actual.selection_length != expected.selection_length
    {
        return Err(format!(
            "Selection {reason}. Expected:\n{}\nActual:\n{}",
            expected.text_with_selection_markers(),
            actual.text_with_selection_markers()
        ));
    }
    Ok(actual)
}

/// Runs every test of every file and returns the number of tests run and the
/// failures.
pub fn run_test_files(files: &[TestFile]) -> (usize, Vec<TestFailure>) {
    let mut count = 0;
    let mut failures = Vec::new();
    for file in files {
        for test in &file.tests {
            for (version, output) in file.versions_to_test(test) {
                count += 1;
                if let Err(message) = run_test_at_version(file, test, output, version) {
                    failures.push(TestFailure {
                        file: file.path.clone(),
                        label: test.label(),
                        version,
                        message,
                    });
                }
            }
        }
    }
    (count, failures)
}
