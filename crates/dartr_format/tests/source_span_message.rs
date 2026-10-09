// Dart source: none (fixtures captured from Dart SDK 3.13.3: dart_style 3.1.13,
// source_span 1.10.2, path 1.9.1, term_glyph 1.2.2)

//! Byte-exact comparison of the `package:source_span` message port with
//! output captured from Dart.
//!
//! `ERROR_CASES` were captured from `FormatterException.message(color:)` of
//! dart_style 3.1.13 (offsets, lengths and messages of the analyzer
//! diagnostics included). The `plain` strings are equal to the stderr of
//! `dart format -o none lib/<name>.dart` without its trailing newline, and
//! the `color` strings are equal to the stderr of the same command on a
//! terminal. The `source` of each case is the text that the formatter
//! got: `File.readAsStringSync` drops a leading BOM. `SPAN_CASES` were captured from
//! `SourceFile.fromString(text, url: url).span(start, end).message(message,
//! color:)` and `SourceSpanException.toString()`.

use dartr_format::exceptions::{FormatError, FormatterError, FormatterException};
use dartr_format::source_span::{pretty_uri_from, span_exception_text, span_message};
use dartr_format::{DartFormatter, SourceCode, Version};

struct ErrorCase {
    name: &'static str,
    path: &'static str,
    source: &'static str,
    /// (offset, length, message), offsets in UTF-16 code units.
    errors: &'static [(usize, usize, &'static str)],
    plain: &'static str,
    color: &'static str,
}

struct SpanCase {
    name: &'static str,
    text: &'static str,
    url: Option<&'static str>,
    start: usize,
    end: usize,
    message: &'static str,
    plain: &'static str,
    color: &'static str,
    exception: &'static str,
}

const ERROR_CASES: &[ErrorCase] = &[
    ErrorCase {
        name: "bad_token",
        path: "lib/bad_token.dart",
        source: "void main() {\n  var x = 1 # 2;\n}\n",
        errors: &[
            (24, 1, "Expected to find ';'."),
            (28, 1, "Expected an identifier."),
            (26, 1, "Expected to find ';'."),
        ],
        plain: "Could not format because the source could not be parsed:\n\nline 2, column 11 of lib/bad_token.dart: Expected to find ';'.\n  ╷\n2 │   var x = 1 # 2;\n  │           ^\n  ╵\nline 2, column 15 of lib/bad_token.dart: Expected an identifier.\n  ╷\n2 │   var x = 1 # 2;\n  │               ^\n  ╵\nline 2, column 13 of lib/bad_token.dart: Expected to find ';'.\n  ╷\n2 │   var x = 1 # 2;\n  │             ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 2, column 11 of lib/bad_token.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m   var x = \u{1b}[31m1\u{1b}[0m # 2;\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m          ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 2, column 15 of lib/bad_token.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m   var x = 1 # \u{1b}[31m2\u{1b}[0m;\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m              ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 2, column 13 of lib/bad_token.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m   var x = 1 \u{1b}[31m#\u{1b}[0m 2;\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m            ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "blank_line_eof",
        path: "lib/blank_line_eof.dart",
        source: "class A {\n\n\n",
        errors: &[(12, 1, "Expected to find '}'.")],
        plain: "Could not format because the source could not be parsed:\n\nline 4, column 1 of lib/blank_line_eof.dart: Expected to find '}'.\n  ╷\n4 │  \n  │ ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 4, column 1 of lib/blank_line_eof.dart: Expected to find '}'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m4 │\u{1b}[0m \u{1b}[31m \u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "bom",
        path: "lib/bom.dart",
        source: "void main() { var x = 1 }\n",
        errors: &[(22, 1, "Expected to find ';'.")],
        plain: "Could not format because the source could not be parsed:\n\nline 1, column 23 of lib/bom.dart: Expected to find ';'.\n  ╷\n1 │ void main() { var x = 1 }\n  │                       ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 1, column 23 of lib/bom.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void main() { var x = \u{1b}[31m1\u{1b}[0m }\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m                      ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "class_eof_spaces",
        path: "lib/class_eof_spaces.dart",
        source: "class A {   ",
        errors: &[(12, 1, "Expected to find '}'.")],
        plain: "Could not format because the source could not be parsed:\n\nline 1, column 13 of lib/class_eof_spaces.dart: Expected to find '}'.\n  ╷\n1 │ class A {    \n  │             ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 1, column 13 of lib/class_eof_spaces.dart: Expected to find '}'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m class A {   \u{1b}[31m \u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m            ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "cr_only",
        path: "lib/cr_only.dart",
        source: "void main() {\r  var x = 1\r}\r",
        errors: &[(24, 1, "Expected to find ';'.")],
        plain: "Could not format because the source could not be parsed:\n\nline 2, column 11 of lib/cr_only.dart: Expected to find ';'.\n  ╷\n2 │   var x = 1\r\n  │           ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 2, column 11 of lib/cr_only.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m   var x = \u{1b}[31m1\u{1b}[0m\r\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m          ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "crlf",
        path: "lib/crlf.dart",
        source: "void main() {\r\n  var x = 1\r\n}\r\n",
        errors: &[(25, 1, "Expected to find ';'.")],
        plain: "Could not format because the source could not be parsed:\n\nline 2, column 11 of lib/crlf.dart: Expected to find ';'.\n  ╷\n2 │   var x = 1\r\n  │           ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 2, column 11 of lib/crlf.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m   var x = \u{1b}[31m1\u{1b}[0m\r\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m          ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "crlf_eof",
        path: "lib/crlf_eof.dart",
        source: "void main() {\r\n  var x = 1;\r\n",
        errors: &[(29, 1, "Expected to find '}'.")],
        plain: "Could not format because the source could not be parsed:\n\nline 3, column 1 of lib/crlf_eof.dart: Expected to find '}'.\n  ╷\n3 │  \n  │ ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 3, column 1 of lib/crlf_eof.dart: Expected to find '}'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m \u{1b}[31m \u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "eleven",
        path: "lib/eleven.dart",
        source: "int a = ;\nint b = ;\nint c = ;\nint d = ;\nint e = ;\nint f = ;\nint g = ;\nint h = ;\nint i = ;\nint j = ;\nint k = ;\n",
        errors: &[
            (8, 1, "Expected an identifier."),
            (18, 1, "Expected an identifier."),
            (28, 1, "Expected an identifier."),
            (38, 1, "Expected an identifier."),
            (48, 1, "Expected an identifier."),
            (58, 1, "Expected an identifier."),
            (68, 1, "Expected an identifier."),
            (78, 1, "Expected an identifier."),
            (88, 1, "Expected an identifier."),
            (98, 1, "Expected an identifier."),
            (108, 1, "Expected an identifier."),
        ],
        plain: "Could not format because the source could not be parsed:\n\nline 1, column 9 of lib/eleven.dart: Expected an identifier.\n  ╷\n1 │ int a = ;\n  │         ^\n  ╵\nline 2, column 9 of lib/eleven.dart: Expected an identifier.\n  ╷\n2 │ int b = ;\n  │         ^\n  ╵\nline 3, column 9 of lib/eleven.dart: Expected an identifier.\n  ╷\n3 │ int c = ;\n  │         ^\n  ╵\nline 4, column 9 of lib/eleven.dart: Expected an identifier.\n  ╷\n4 │ int d = ;\n  │         ^\n  ╵\nline 5, column 9 of lib/eleven.dart: Expected an identifier.\n  ╷\n5 │ int e = ;\n  │         ^\n  ╵\nline 6, column 9 of lib/eleven.dart: Expected an identifier.\n  ╷\n6 │ int f = ;\n  │         ^\n  ╵\nline 7, column 9 of lib/eleven.dart: Expected an identifier.\n  ╷\n7 │ int g = ;\n  │         ^\n  ╵\nline 8, column 9 of lib/eleven.dart: Expected an identifier.\n  ╷\n8 │ int h = ;\n  │         ^\n  ╵\nline 9, column 9 of lib/eleven.dart: Expected an identifier.\n  ╷\n9 │ int i = ;\n  │         ^\n  ╵\nline 10, column 9 of lib/eleven.dart: Expected an identifier.\n   ╷\n10 │ int j = ;\n   │         ^\n   ╵\n(1 more errors...)",
        color: "Could not format because the source could not be parsed:\n\nline 1, column 9 of lib/eleven.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m int a = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 2, column 9 of lib/eleven.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m int b = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 3, column 9 of lib/eleven.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m int c = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 4, column 9 of lib/eleven.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m4 │\u{1b}[0m int d = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 5, column 9 of lib/eleven.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m5 │\u{1b}[0m int e = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 6, column 9 of lib/eleven.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m6 │\u{1b}[0m int f = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 7, column 9 of lib/eleven.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m7 │\u{1b}[0m int g = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 8, column 9 of lib/eleven.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m8 │\u{1b}[0m int h = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 9, column 9 of lib/eleven.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m9 │\u{1b}[0m int i = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 10, column 9 of lib/eleven.dart: Expected an identifier.\n\u{1b}[34m   ╷\u{1b}[0m\n\u{1b}[34m10 │\u{1b}[0m int j = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m   │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m   ╵\u{1b}[0m\n(1 more errors...)",
    },
    ErrorCase {
        name: "emoji",
        path: "lib/emoji.dart",
        source: "void main() {\n  var s = '😀é😀'; var x = 1\n}\n",
        errors: &[(41, 1, "Expected to find ';'.")],
        plain: "Could not format because the source could not be parsed:\n\nline 2, column 28 of lib/emoji.dart: Expected to find ';'.\n  ╷\n2 │   var s = '😀é😀'; var x = 1\n  │                            ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 2, column 28 of lib/emoji.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m   var s = '😀é😀'; var x = \u{1b}[31m1\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m                           ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "emoji_in_span",
        path: "lib/emoji_in_span.dart",
        source: "void main() {\n  var x = 1 '😀';\n}\n",
        errors: &[(24, 1, "Expected to find ';'.")],
        plain: "Could not format because the source could not be parsed:\n\nline 2, column 11 of lib/emoji_in_span.dart: Expected to find ';'.\n  ╷\n2 │   var x = 1 '😀';\n  │           ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 2, column 11 of lib/emoji_in_span.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m   var x = \u{1b}[31m1\u{1b}[0m '😀';\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m          ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "empty_lines_crlf_eof",
        path: "lib/empty_lines_crlf_eof.dart",
        source: "class A {\r\n\r\n",
        errors: &[(13, 1, "Expected to find '}'.")],
        plain: "Could not format because the source could not be parsed:\n\nline 3, column 1 of lib/empty_lines_crlf_eof.dart: Expected to find '}'.\n  ╷\n3 │  \n  │ ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 3, column 1 of lib/empty_lines_crlf_eof.dart: Expected to find '}'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m \u{1b}[31m \u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "eof_newline",
        path: "lib/eof_newline.dart",
        source: "void main() {\n",
        errors: &[(14, 1, "Expected to find '}'.")],
        plain: "Could not format because the source could not be parsed:\n\nline 2, column 1 of lib/eof_newline.dart: Expected to find '}'.\n  ╷\n2 │  \n  │ ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 2, column 1 of lib/eof_newline.dart: Expected to find '}'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m \u{1b}[31m \u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "eof_no_newline",
        path: "lib/eof_no_newline.dart",
        source: "void main() {",
        errors: &[(13, 1, "Expected to find '}'.")],
        plain: "Could not format because the source could not be parsed:\n\nline 1, column 14 of lib/eof_no_newline.dart: Expected to find '}'.\n  ╷\n1 │ void main() { \n  │              ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 1, column 14 of lib/eof_no_newline.dart: Expected to find '}'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void main() {\u{1b}[31m \u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m             ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "eof_trailing_spaces",
        path: "lib/eof_trailing_spaces.dart",
        source: "void main() {\n   ",
        errors: &[(17, 1, "Expected to find '}'.")],
        plain: "Could not format because the source could not be parsed:\n\nline 2, column 4 of lib/eof_trailing_spaces.dart: Expected to find '}'.\n  ╷\n2 │     \n  │    ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 2, column 4 of lib/eof_trailing_spaces.dart: Expected to find '}'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m    \u{1b}[31m \u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m   ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "eof_two_newlines",
        path: "lib/eof_two_newlines.dart",
        source: "void main() {\n\n",
        errors: &[(15, 1, "Expected to find '}'.")],
        plain: "Could not format because the source could not be parsed:\n\nline 3, column 1 of lib/eof_two_newlines.dart: Expected to find '}'.\n  ╷\n3 │  \n  │ ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 3, column 1 of lib/eof_two_newlines.dart: Expected to find '}'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m \u{1b}[31m \u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "error_first_col",
        path: "lib/error_first_col.dart",
        source: "void main() {\nfoo bar\n}\n",
        errors: &[(18, 3, "Expected to find ';'.")],
        plain: "Could not format because the source could not be parsed:\n\nline 2, column 5 of lib/error_first_col.dart: Expected to find ';'.\n  ╷\n2 │ foo bar\n  │     ^^^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 2, column 5 of lib/error_first_col.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m foo \u{1b}[31mbar\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m    ^^^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "exactly_ten",
        path: "lib/exactly_ten.dart",
        source: "int a = ;\nint b = ;\nint c = ;\nint d = ;\nint e = ;\nint f = ;\nint g = ;\nint h = ;\nint i = ;\nint j = ;\n",
        errors: &[
            (8, 1, "Expected an identifier."),
            (18, 1, "Expected an identifier."),
            (28, 1, "Expected an identifier."),
            (38, 1, "Expected an identifier."),
            (48, 1, "Expected an identifier."),
            (58, 1, "Expected an identifier."),
            (68, 1, "Expected an identifier."),
            (78, 1, "Expected an identifier."),
            (88, 1, "Expected an identifier."),
            (98, 1, "Expected an identifier."),
        ],
        plain: "Could not format because the source could not be parsed:\n\nline 1, column 9 of lib/exactly_ten.dart: Expected an identifier.\n  ╷\n1 │ int a = ;\n  │         ^\n  ╵\nline 2, column 9 of lib/exactly_ten.dart: Expected an identifier.\n  ╷\n2 │ int b = ;\n  │         ^\n  ╵\nline 3, column 9 of lib/exactly_ten.dart: Expected an identifier.\n  ╷\n3 │ int c = ;\n  │         ^\n  ╵\nline 4, column 9 of lib/exactly_ten.dart: Expected an identifier.\n  ╷\n4 │ int d = ;\n  │         ^\n  ╵\nline 5, column 9 of lib/exactly_ten.dart: Expected an identifier.\n  ╷\n5 │ int e = ;\n  │         ^\n  ╵\nline 6, column 9 of lib/exactly_ten.dart: Expected an identifier.\n  ╷\n6 │ int f = ;\n  │         ^\n  ╵\nline 7, column 9 of lib/exactly_ten.dart: Expected an identifier.\n  ╷\n7 │ int g = ;\n  │         ^\n  ╵\nline 8, column 9 of lib/exactly_ten.dart: Expected an identifier.\n  ╷\n8 │ int h = ;\n  │         ^\n  ╵\nline 9, column 9 of lib/exactly_ten.dart: Expected an identifier.\n  ╷\n9 │ int i = ;\n  │         ^\n  ╵\nline 10, column 9 of lib/exactly_ten.dart: Expected an identifier.\n   ╷\n10 │ int j = ;\n   │         ^\n   ╵",
        color: "Could not format because the source could not be parsed:\n\nline 1, column 9 of lib/exactly_ten.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m int a = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 2, column 9 of lib/exactly_ten.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m int b = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 3, column 9 of lib/exactly_ten.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m int c = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 4, column 9 of lib/exactly_ten.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m4 │\u{1b}[0m int d = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 5, column 9 of lib/exactly_ten.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m5 │\u{1b}[0m int e = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 6, column 9 of lib/exactly_ten.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m6 │\u{1b}[0m int f = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 7, column 9 of lib/exactly_ten.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m7 │\u{1b}[0m int g = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 8, column 9 of lib/exactly_ten.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m8 │\u{1b}[0m int h = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 9, column 9 of lib/exactly_ten.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m9 │\u{1b}[0m int i = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 10, column 9 of lib/exactly_ten.dart: Expected an identifier.\n\u{1b}[34m   ╷\u{1b}[0m\n\u{1b}[34m10 │\u{1b}[0m int j = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m   │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m   ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "last_line_no_newline",
        path: "lib/last_line_no_newline.dart",
        source: "void main() {\n  var x = 1\n}\nint y = ",
        errors: &[
            (24, 1, "Expected to find ';'."),
            (36, 0, "Expected an identifier."),
            (34, 1, "Expected to find ';'."),
        ],
        plain: "Could not format because the source could not be parsed:\n\nline 2, column 11 of lib/last_line_no_newline.dart: Expected to find ';'.\n  ╷\n2 │   var x = 1\n  │           ^\n  ╵\nline 4, column 9 of lib/last_line_no_newline.dart: Expected an identifier.\n  ╷\n4 │ int y = \n  │         ^\n  ╵\nline 4, column 7 of lib/last_line_no_newline.dart: Expected to find ';'.\n  ╷\n4 │ int y = \n  │       ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 2, column 11 of lib/last_line_no_newline.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m   var x = \u{1b}[31m1\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m          ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 4, column 9 of lib/last_line_no_newline.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m4 │\u{1b}[0m int y = \u{1b}[31m\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 4, column 7 of lib/last_line_no_newline.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m4 │\u{1b}[0m int y \u{1b}[31m=\u{1b}[0m \n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m      ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "line10",
        path: "lib/line10.dart",
        source: "//\n//\n//\n//\n//\n//\n//\n//\n//\nint x = ;\n",
        errors: &[(35, 1, "Expected an identifier.")],
        plain: "Could not format because the source could not be parsed:\n\nline 10, column 9 of lib/line10.dart: Expected an identifier.\n   ╷\n10 │ int x = ;\n   │         ^\n   ╵",
        color: "Could not format because the source could not be parsed:\n\nline 10, column 9 of lib/line10.dart: Expected an identifier.\n\u{1b}[34m   ╷\u{1b}[0m\n\u{1b}[34m10 │\u{1b}[0m int x = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m   │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m   ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "line100",
        path: "lib/line100.dart",
        source: "//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\nint x = ;\n",
        errors: &[(305, 1, "Expected an identifier.")],
        plain: "Could not format because the source could not be parsed:\n\nline 100, column 9 of lib/line100.dart: Expected an identifier.\n    ╷\n100 │ int x = ;\n    │         ^\n    ╵",
        color: "Could not format because the source could not be parsed:\n\nline 100, column 9 of lib/line100.dart: Expected an identifier.\n\u{1b}[34m    ╷\u{1b}[0m\n\u{1b}[34m100 │\u{1b}[0m int x = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m    │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m    ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "line9",
        path: "lib/line9.dart",
        source: "//\n//\n//\n//\n//\n//\n//\n//\nint x = ;\n",
        errors: &[(32, 1, "Expected an identifier.")],
        plain: "Could not format because the source could not be parsed:\n\nline 9, column 9 of lib/line9.dart: Expected an identifier.\n  ╷\n9 │ int x = ;\n  │         ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 9, column 9 of lib/line9.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m9 │\u{1b}[0m int x = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "line99",
        path: "lib/line99.dart",
        source: "//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\n//\nint x = ;\n",
        errors: &[(302, 1, "Expected an identifier.")],
        plain: "Could not format because the source could not be parsed:\n\nline 99, column 9 of lib/line99.dart: Expected an identifier.\n   ╷\n99 │ int x = ;\n   │         ^\n   ╵",
        color: "Could not format because the source could not be parsed:\n\nline 99, column 9 of lib/line99.dart: Expected an identifier.\n\u{1b}[34m   ╷\u{1b}[0m\n\u{1b}[34m99 │\u{1b}[0m int x = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m   │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m   ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "long_token",
        path: "lib/long_token.dart",
        source: "void main() {\n  var x = 1 abcdefghijklmnop;\n}\n",
        errors: &[(24, 1, "Expected to find ';'.")],
        plain: "Could not format because the source could not be parsed:\n\nline 2, column 11 of lib/long_token.dart: Expected to find ';'.\n  ╷\n2 │   var x = 1 abcdefghijklmnop;\n  │           ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 2, column 11 of lib/long_token.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m   var x = \u{1b}[31m1\u{1b}[0m abcdefghijklmnop;\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m          ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "many_errors",
        path: "lib/many_errors.dart",
        source: "void main() { a b c d e f g h i j k l m n o p }\n",
        errors: &[
            (16, 1, "Expected to find ';'."),
            (20, 1, "Expected to find ';'."),
            (24, 1, "Expected to find ';'."),
            (28, 1, "Expected to find ';'."),
            (32, 1, "Expected to find ';'."),
            (36, 1, "Expected to find ';'."),
            (40, 1, "Expected to find ';'."),
            (44, 1, "Expected to find ';'."),
        ],
        plain: "Could not format because the source could not be parsed:\n\nline 1, column 17 of lib/many_errors.dart: Expected to find ';'.\n  ╷\n1 │ void main() { a b c d e f g h i j k l m n o p }\n  │                 ^\n  ╵\nline 1, column 21 of lib/many_errors.dart: Expected to find ';'.\n  ╷\n1 │ void main() { a b c d e f g h i j k l m n o p }\n  │                     ^\n  ╵\nline 1, column 25 of lib/many_errors.dart: Expected to find ';'.\n  ╷\n1 │ void main() { a b c d e f g h i j k l m n o p }\n  │                         ^\n  ╵\nline 1, column 29 of lib/many_errors.dart: Expected to find ';'.\n  ╷\n1 │ void main() { a b c d e f g h i j k l m n o p }\n  │                             ^\n  ╵\nline 1, column 33 of lib/many_errors.dart: Expected to find ';'.\n  ╷\n1 │ void main() { a b c d e f g h i j k l m n o p }\n  │                                 ^\n  ╵\nline 1, column 37 of lib/many_errors.dart: Expected to find ';'.\n  ╷\n1 │ void main() { a b c d e f g h i j k l m n o p }\n  │                                     ^\n  ╵\nline 1, column 41 of lib/many_errors.dart: Expected to find ';'.\n  ╷\n1 │ void main() { a b c d e f g h i j k l m n o p }\n  │                                         ^\n  ╵\nline 1, column 45 of lib/many_errors.dart: Expected to find ';'.\n  ╷\n1 │ void main() { a b c d e f g h i j k l m n o p }\n  │                                             ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 1, column 17 of lib/many_errors.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void main() { a \u{1b}[31mb\u{1b}[0m c d e f g h i j k l m n o p }\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m                ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 1, column 21 of lib/many_errors.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void main() { a b c \u{1b}[31md\u{1b}[0m e f g h i j k l m n o p }\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m                    ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 1, column 25 of lib/many_errors.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void main() { a b c d e \u{1b}[31mf\u{1b}[0m g h i j k l m n o p }\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m                        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 1, column 29 of lib/many_errors.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void main() { a b c d e f g \u{1b}[31mh\u{1b}[0m i j k l m n o p }\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m                            ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 1, column 33 of lib/many_errors.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void main() { a b c d e f g h i \u{1b}[31mj\u{1b}[0m k l m n o p }\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m                                ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 1, column 37 of lib/many_errors.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void main() { a b c d e f g h i j k \u{1b}[31ml\u{1b}[0m m n o p }\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m                                    ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 1, column 41 of lib/many_errors.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void main() { a b c d e f g h i j k l m \u{1b}[31mn\u{1b}[0m o p }\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m                                        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 1, column 45 of lib/many_errors.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void main() { a b c d e f g h i j k l m n o \u{1b}[31mp\u{1b}[0m }\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m                                            ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "missing_paren_eof_crlf",
        path: "lib/missing_paren_eof_crlf.dart",
        source: "void main() {\r\n  foo(\r\n",
        errors: &[
            (20, 1, "Expected to find ';'."),
            (23, 1, "Expected to find ')'."),
            (23, 1, "Expected to find '}'."),
        ],
        plain: "Could not format because the source could not be parsed:\n\nline 2, column 6 of lib/missing_paren_eof_crlf.dart: Expected to find ';'.\n  ╷\n2 │   foo(\r\n  │      ^\n  ╵\nline 3, column 1 of lib/missing_paren_eof_crlf.dart: Expected to find ')'.\n  ╷\n3 │  \n  │ ^\n  ╵\nline 3, column 1 of lib/missing_paren_eof_crlf.dart: Expected to find '}'.\n  ╷\n3 │  \n  │ ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 2, column 6 of lib/missing_paren_eof_crlf.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m   foo\u{1b}[31m(\u{1b}[0m\r\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m     ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 3, column 1 of lib/missing_paren_eof_crlf.dart: Expected to find ')'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m \u{1b}[31m \u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 3, column 1 of lib/missing_paren_eof_crlf.dart: Expected to find '}'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m \u{1b}[31m \u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "missing_semicolon",
        path: "lib/missing_semicolon.dart",
        source: "void main() {\n  var x = 1\n}\n",
        errors: &[(24, 1, "Expected to find ';'.")],
        plain: "Could not format because the source could not be parsed:\n\nline 2, column 11 of lib/missing_semicolon.dart: Expected to find ';'.\n  ╷\n2 │   var x = 1\n  │           ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 2, column 11 of lib/missing_semicolon.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m   var x = \u{1b}[31m1\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m          ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "multiline_args",
        path: "lib/multiline_args.dart",
        source: "void main() {\n  foo(1,\n      2\n      3);\n}\n",
        errors: &[(37, 1, "Expected to find ','.")],
        plain: "Could not format because the source could not be parsed:\n\nline 4, column 7 of lib/multiline_args.dart: Expected to find ','.\n  ╷\n4 │       3);\n  │       ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 4, column 7 of lib/multiline_args.dart: Expected to find ','.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m4 │\u{1b}[0m       \u{1b}[31m3\u{1b}[0m);\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m      ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "nul",
        path: "lib/nul.dart",
        source: "void main() { var x = 1\u{0} }\n",
        errors: &[
            (22, 1, "Expected to find ';'."),
            (23, 1, "Illegal character '0'."),
        ],
        plain: "Could not format because the source could not be parsed:\n\nline 1, column 23 of lib/nul.dart: Expected to find ';'.\n  ╷\n1 │ void main() { var x = 1\u{0} }\n  │                       ^\n  ╵\nline 1, column 24 of lib/nul.dart: Illegal character '0'.\n  ╷\n1 │ void main() { var x = 1\u{0} }\n  │                        ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 1, column 23 of lib/nul.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void main() { var x = \u{1b}[31m1\u{1b}[0m\u{0} }\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m                      ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 1, column 24 of lib/nul.dart: Illegal character '0'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void main() { var x = 1\u{1b}[31m\u{0}\u{1b}[0m }\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m                       ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "only_paren",
        path: "lib/only_paren.dart",
        source: "(",
        errors: &[
            (
                0,
                1,
                "Expected a method, getter, setter or operator declaration.",
            ),
            (
                1,
                0,
                "Expected a method, getter, setter or operator declaration.",
            ),
            (1, 1, "Expected to find ')'."),
        ],
        plain: "Could not format because the source could not be parsed:\n\nline 1, column 1 of lib/only_paren.dart: Expected a method, getter, setter or operator declaration.\n  ╷\n1 │ (\n  │ ^\n  ╵\nline 1, column 2 of lib/only_paren.dart: Expected a method, getter, setter or operator declaration.\n  ╷\n1 │ (\n  │  ^\n  ╵\nline 1, column 2 of lib/only_paren.dart: Expected to find ')'.\n  ╷\n1 │ ( \n  │  ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 1, column 1 of lib/only_paren.dart: Expected a method, getter, setter or operator declaration.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m \u{1b}[31m(\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 1, column 2 of lib/only_paren.dart: Expected a method, getter, setter or operator declaration.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m (\u{1b}[31m\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 1, column 2 of lib/only_paren.dart: Expected to find ')'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m (\u{1b}[31m \u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "string_interp",
        path: "lib/string_interp.dart",
        source: "void main() {\n  var s = '${1 +}';\n}\n",
        errors: &[(30, 1, "Expected an identifier.")],
        plain: "Could not format because the source could not be parsed:\n\nline 2, column 17 of lib/string_interp.dart: Expected an identifier.\n  ╷\n2 │   var s = '${1 +}';\n  │                 ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 2, column 17 of lib/string_interp.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m   var s = '${1 +\u{1b}[31m}\u{1b}[0m';\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m                ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "tab_two",
        path: "lib/tab_two.dart",
        source: "class A {\n\t\tint\tx y;\n}\n",
        errors: &[
            (16, 1, "Expected to find ';'."),
            (
                18,
                1,
                "Variables must be declared using the keywords 'const', 'final', 'var' or a type name.",
            ),
        ],
        plain: "Could not format because the source could not be parsed:\n\nline 2, column 7 of lib/tab_two.dart: Expected to find ';'.\n  ╷\n2 │         int    x y;\n  │                ^\n  ╵\nline 2, column 9 of lib/tab_two.dart: Variables must be declared using the keywords 'const', 'final', 'var' or a type name.\n  ╷\n2 │         int    x y;\n  │                  ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 2, column 7 of lib/tab_two.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m         int    \u{1b}[31mx\u{1b}[0m y;\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m               ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 2, column 9 of lib/tab_two.dart: Variables must be declared using the keywords 'const', 'final', 'var' or a type name.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m         int    x \u{1b}[31my\u{1b}[0m;\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m                 ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "tabs_before",
        path: "lib/tabs_before.dart",
        source: "void main() {\n\tvar x = 1\n}\n",
        errors: &[(23, 1, "Expected to find ';'.")],
        plain: "Could not format because the source could not be parsed:\n\nline 2, column 10 of lib/tabs_before.dart: Expected to find ';'.\n  ╷\n2 │     var x = 1\n  │             ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 2, column 10 of lib/tabs_before.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m     var x = \u{1b}[31m1\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m            ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "tabs_inside",
        path: "lib/tabs_inside.dart",
        source: "void main() {\n\tvar\tx = 1 2;\n}\n",
        errors: &[(23, 1, "Expected to find ';'.")],
        plain: "Could not format because the source could not be parsed:\n\nline 2, column 10 of lib/tabs_inside.dart: Expected to find ';'.\n  ╷\n2 │     var    x = 1 2;\n  │                ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 2, column 10 of lib/tabs_inside.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m     var    x = \u{1b}[31m1\u{1b}[0m 2;\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m               ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "two_errors_same_line",
        path: "lib/two_errors_same_line.dart",
        source: "int a = ; int b = ;\n",
        errors: &[
            (8, 1, "Expected an identifier."),
            (18, 1, "Expected an identifier."),
        ],
        plain: "Could not format because the source could not be parsed:\n\nline 1, column 9 of lib/two_errors_same_line.dart: Expected an identifier.\n  ╷\n1 │ int a = ; int b = ;\n  │         ^\n  ╵\nline 1, column 19 of lib/two_errors_same_line.dart: Expected an identifier.\n  ╷\n1 │ int a = ; int b = ;\n  │                   ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 1, column 9 of lib/two_errors_same_line.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m int a = \u{1b}[31m;\u{1b}[0m int b = ;\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 1, column 19 of lib/two_errors_same_line.dart: Expected an identifier.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m int a = ; int b = \u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m                  ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "unexpected_close",
        path: "lib/unexpected_close.dart",
        source: "}\n",
        errors: &[(
            0,
            1,
            "Expected a method, getter, setter or operator declaration.",
        )],
        plain: "Could not format because the source could not be parsed:\n\nline 1, column 1 of lib/unexpected_close.dart: Expected a method, getter, setter or operator declaration.\n  ╷\n1 │ }\n  │ ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 1, column 1 of lib/unexpected_close.dart: Expected a method, getter, setter or operator declaration.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m \u{1b}[31m}\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "unicode_ident",
        path: "lib/unicode_ident.dart",
        source: "void main() { var żółw = 1 }\n",
        errors: &[
            (25, 1, "Expected to find ';'."),
            (18, 1, "Illegal character '380'."),
            (19, 1, "Illegal character '243'."),
            (20, 1, "Illegal character '322'."),
        ],
        plain: "Could not format because the source could not be parsed:\n\nline 1, column 26 of lib/unicode_ident.dart: Expected to find ';'.\n  ╷\n1 │ void main() { var żółw = 1 }\n  │                          ^\n  ╵\nline 1, column 19 of lib/unicode_ident.dart: Illegal character '380'.\n  ╷\n1 │ void main() { var żółw = 1 }\n  │                   ^\n  ╵\nline 1, column 20 of lib/unicode_ident.dart: Illegal character '243'.\n  ╷\n1 │ void main() { var żółw = 1 }\n  │                    ^\n  ╵\nline 1, column 21 of lib/unicode_ident.dart: Illegal character '322'.\n  ╷\n1 │ void main() { var żółw = 1 }\n  │                     ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 1, column 26 of lib/unicode_ident.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void main() { var żółw = \u{1b}[31m1\u{1b}[0m }\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m                         ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 1, column 19 of lib/unicode_ident.dart: Illegal character '380'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void main() { var \u{1b}[31mż\u{1b}[0mółw = 1 }\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m                  ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 1, column 20 of lib/unicode_ident.dart: Illegal character '243'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void main() { var ż\u{1b}[31mó\u{1b}[0młw = 1 }\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m                   ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 1, column 21 of lib/unicode_ident.dart: Illegal character '322'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void main() { var żó\u{1b}[31mł\u{1b}[0mw = 1 }\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m                    ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "unterminated_comment",
        path: "lib/unterminated_comment.dart",
        source: "void main() {}\n/* abc\n def\n",
        errors: &[(26, 1, "Unterminated multi-line comment.")],
        plain: "Could not format because the source could not be parsed:\n\nline 3, column 5 of lib/unterminated_comment.dart: Unterminated multi-line comment.\n  ╷\n3 │  def\n  │     ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 3, column 5 of lib/unterminated_comment.dart: Unterminated multi-line comment.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m  def\u{1b}[31m\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m    ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "unterminated_comment_noeol",
        path: "lib/unterminated_comment_noeol.dart",
        source: "void main() {}\n/* abc\n def",
        errors: &[(25, 1, "Unterminated multi-line comment.")],
        plain: "Could not format because the source could not be parsed:\n\nline 3, column 4 of lib/unterminated_comment_noeol.dart: Unterminated multi-line comment.\n  ╷\n3 │  def\n  │    ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 3, column 4 of lib/unterminated_comment_noeol.dart: Unterminated multi-line comment.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m  de\u{1b}[31mf\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m   ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "unterminated_multiline_string",
        path: "lib/unterminated_multiline_string.dart",
        source: "void main() {\n  var s = '''abc\ndef\n",
        errors: &[
            (24, 11, "Expected to find ';'."),
            (34, 1, "Unterminated string literal."),
            (35, 1, "Expected to find '}'."),
        ],
        plain: "Could not format because the source could not be parsed:\n\nline 2, column 11 of lib/unterminated_multiline_string.dart: Expected to find ';'.\n  ╷\n2 │     var s = '''abc\n  │ ┌───────────^\n3 │ └ def\n  ╵\nline 3, column 4 of lib/unterminated_multiline_string.dart: Unterminated string literal.\n  ╷\n3 │ def\n  │    ^\n  ╵\nline 4, column 1 of lib/unterminated_multiline_string.dart: Expected to find '}'.\n  ╷\n4 │  \n  │ ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 2, column 11 of lib/unterminated_multiline_string.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m     var s = \u{1b}[31m'''abc\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m┌\u{1b}[0m\u{1b}[31m───────────^\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m \u{1b}[31m└\u{1b}[0m \u{1b}[31mdef\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 3, column 4 of lib/unterminated_multiline_string.dart: Unterminated string literal.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m def\u{1b}[31m\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m   ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 4, column 1 of lib/unterminated_multiline_string.dart: Expected to find '}'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m4 │\u{1b}[0m \u{1b}[31m \u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
    ErrorCase {
        name: "unterminated_string",
        path: "lib/unterminated_string.dart",
        source: "void main() {\n  var s = 'abc;\n}\n",
        errors: &[
            (24, 5, "Expected to find ';'."),
            (28, 1, "Unterminated string literal."),
        ],
        plain: "Could not format because the source could not be parsed:\n\nline 2, column 11 of lib/unterminated_string.dart: Expected to find ';'.\n  ╷\n2 │   var s = 'abc;\n  │           ^^^^^\n  ╵\nline 2, column 15 of lib/unterminated_string.dart: Unterminated string literal.\n  ╷\n2 │   var s = 'abc;\n  │               ^\n  ╵",
        color: "Could not format because the source could not be parsed:\n\nline 2, column 11 of lib/unterminated_string.dart: Expected to find ';'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m   var s = \u{1b}[31m'abc;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m          ^^^^^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m\nline 2, column 15 of lib/unterminated_string.dart: Unterminated string literal.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m   var s = 'abc\u{1b}[31m;\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m              ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
    },
];

const SPAN_CASES: &[SpanCase] = &[
    SpanCase {
        name: "multi_mid",
        text: "void main() {\n  foo(1,\n      2);\n}\n",
        url: Some("lib/a.dart"),
        start: 19,
        end: 31,
        message: "Boom.",
        plain: "line 2, column 6 of lib/a.dart: Boom.\n  ╷\n2 │     foo(1,\n  │ ┌──────^\n3 │ │       2);\n  │ └────────^\n  ╵",
        color: "line 2, column 6 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m     foo\u{1b}[31m(1,\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m┌\u{1b}[0m\u{1b}[31m──────^\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m \u{1b}[31m│\u{1b}[0m \u{1b}[31m      2)\u{1b}[0m;\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m└\u{1b}[0m\u{1b}[31m────────^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 2, column 6 of lib/a.dart: Boom.\n  ╷\n2 │     foo(1,\n  │ ┌──────^\n3 │ │       2);\n  │ └────────^\n  ╵",
    },
    SpanCase {
        name: "multi_from_line_start",
        text: "void main() {\n  foo(1,\n      2);\n}\n",
        url: Some("lib/a.dart"),
        start: 14,
        end: 33,
        message: "Boom.",
        plain: "line 2, column 1 of lib/a.dart: Boom.\n  ╷\n2 │ ┌   foo(1,\n3 │ └       2);\n  ╵",
        color: "line 2, column 1 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m \u{1b}[31m┌\u{1b}[0m \u{1b}[31m  foo(1,\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m \u{1b}[31m└\u{1b}[0m \u{1b}[31m      2);\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 2, column 1 of lib/a.dart: Boom.\n  ╷\n2 │ ┌   foo(1,\n3 │ └       2);\n  ╵",
    },
    SpanCase {
        name: "multi_ws_start",
        text: "void main() {\n  foo(1,\n      2);\n}\n",
        url: Some("lib/a.dart"),
        start: 16,
        end: 32,
        message: "Boom.",
        plain: "line 2, column 3 of lib/a.dart: Boom.\n  ╷\n2 │ ┌   foo(1,\n3 │ └       2);\n  ╵",
        color: "line 2, column 3 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m \u{1b}[31m┌\u{1b}[0m   \u{1b}[31mfoo(1,\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m \u{1b}[31m└\u{1b}[0m \u{1b}[31m      2);\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 2, column 3 of lib/a.dart: Boom.\n  ╷\n2 │ ┌   foo(1,\n3 │ └       2);\n  ╵",
    },
    SpanCase {
        name: "multi_end_col0",
        text: "void main() {\n  foo(1,\n      2);\n}\n",
        url: Some("lib/a.dart"),
        start: 16,
        end: 23,
        message: "Boom.",
        plain: "line 2, column 3 of lib/a.dart: Boom.\n  ╷\n2 │   foo(1,\n  │   ^^^^^^\n  ╵",
        color: "line 2, column 3 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m   \u{1b}[31mfoo(1,\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m  ^^^^^^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 2, column 3 of lib/a.dart: Boom.\n  ╷\n2 │   foo(1,\n  │   ^^^^^^\n  ╵",
    },
    SpanCase {
        name: "multi_whole_lines",
        text: "void main() {\n  foo(1,\n      2);\n}\n",
        url: Some("lib/a.dart"),
        start: 14,
        end: 35,
        message: "Boom.",
        plain: "line 2, column 1 of lib/a.dart: Boom.\n  ╷\n2 │ ┌   foo(1,\n3 │ │       2);\n4 │ └ }\n  ╵",
        color: "line 2, column 1 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m \u{1b}[31m┌\u{1b}[0m \u{1b}[31m  foo(1,\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m \u{1b}[31m│\u{1b}[0m \u{1b}[31m      2);\u{1b}[0m\n\u{1b}[34m4 │\u{1b}[0m \u{1b}[31m└\u{1b}[0m \u{1b}[31m}\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 2, column 1 of lib/a.dart: Boom.\n  ╷\n2 │ ┌   foo(1,\n3 │ │       2);\n4 │ └ }\n  ╵",
    },
    SpanCase {
        name: "whole_file",
        text: "void main() {\n  foo(1,\n      2);\n}\n",
        url: Some("lib/a.dart"),
        start: 0,
        end: 35,
        message: "Boom.",
        plain: "line 1, column 1 of lib/a.dart: Boom.\n  ╷\n1 │ ┌ void main() {\n2 │ │   foo(1,\n3 │ │       2);\n4 │ └ }\n  ╵",
        color: "line 1, column 1 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m \u{1b}[31m┌\u{1b}[0m \u{1b}[31mvoid main() {\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m \u{1b}[31m│\u{1b}[0m \u{1b}[31m  foo(1,\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m \u{1b}[31m│\u{1b}[0m \u{1b}[31m      2);\u{1b}[0m\n\u{1b}[34m4 │\u{1b}[0m \u{1b}[31m└\u{1b}[0m \u{1b}[31m}\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 1, column 1 of lib/a.dart: Boom.\n  ╷\n1 │ ┌ void main() {\n2 │ │   foo(1,\n3 │ │       2);\n4 │ └ }\n  ╵",
    },
    SpanCase {
        name: "empty_at_eof",
        text: "void main() {\n  foo(1,\n      2);\n}\n",
        url: Some("lib/a.dart"),
        start: 35,
        end: 35,
        message: "Boom.",
        plain: "line 5, column 1 of lib/a.dart: Boom.\n  ╷\n5 │ \n  │ ^\n  ╵",
        color: "line 5, column 1 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m5 │\u{1b}[0m \u{1b}[31m\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 5, column 1 of lib/a.dart: Boom.\n  ╷\n5 │ \n  │ ^\n  ╵",
    },
    SpanCase {
        name: "empty_at_line_start",
        text: "void main() {\n  foo(1,\n      2);\n}\n",
        url: Some("lib/a.dart"),
        start: 33,
        end: 33,
        message: "Boom.",
        plain: "line 4, column 1 of lib/a.dart: Boom.\n  ╷\n4 │ }\n  │ ^\n  ╵",
        color: "line 4, column 1 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m4 │\u{1b}[0m \u{1b}[31m\u{1b}[0m}\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 4, column 1 of lib/a.dart: Boom.\n  ╷\n4 │ }\n  │ ^\n  ╵",
    },
    SpanCase {
        name: "empty_mid",
        text: "void main() {\n  foo(1,\n      2);\n}\n",
        url: Some("lib/a.dart"),
        start: 3,
        end: 3,
        message: "Boom.",
        plain: "line 1, column 4 of lib/a.dart: Boom.\n  ╷\n1 │ void main() {\n  │    ^\n  ╵",
        color: "line 1, column 4 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m voi\u{1b}[31m\u{1b}[0md main() {\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m   ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 1, column 4 of lib/a.dart: Boom.\n  ╷\n1 │ void main() {\n  │    ^\n  ╵",
    },
    SpanCase {
        name: "newline_only",
        text: "void main() {\n  foo(1,\n      2);\n}\n",
        url: Some("lib/a.dart"),
        start: 13,
        end: 14,
        message: "Boom.",
        plain: "line 1, column 14 of lib/a.dart: Boom.\n  ╷\n1 │ void main() {\n  │              ^\n  ╵",
        color: "line 1, column 14 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void main() {\u{1b}[31m\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m             ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 1, column 14 of lib/a.dart: Boom.\n  ╷\n1 │ void main() {\n  │              ^\n  ╵",
    },
    SpanCase {
        name: "no_url",
        text: "void main() {\n  foo(1,\n      2);\n}\n",
        url: None,
        start: 5,
        end: 9,
        message: "Boom.",
        plain: "line 1, column 6: Boom.\n  ╷\n1 │ void main() {\n  │      ^^^^\n  ╵",
        color: "line 1, column 6: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void \u{1b}[31mmain\u{1b}[0m() {\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m     ^^^^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 1, column 6: Boom.\n  ╷\n1 │ void main() {\n  │      ^^^^\n  ╵",
    },
    SpanCase {
        name: "empty_url",
        text: "void main() {\n  foo(1,\n      2);\n}\n",
        url: Some(""),
        start: 5,
        end: 9,
        message: "Boom.",
        plain: "line 1, column 6 of .: Boom.\n  ╷\n1 │ void main() {\n  │      ^^^^\n  ╵",
        color: "line 1, column 6 of .: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void \u{1b}[31mmain\u{1b}[0m() {\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m     ^^^^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 1, column 6 of .: Boom.\n  ╷\n1 │ void main() {\n  │      ^^^^\n  ╵",
    },
    SpanCase {
        name: "abs_url",
        text: "void main() {\n  foo(1,\n      2);\n}\n",
        url: Some("/tmp/x.dart"),
        start: 5,
        end: 9,
        message: "Boom.",
        plain: "line 1, column 6 of /tmp/x.dart: Boom.\n  ╷\n1 │ void main() {\n  │      ^^^^\n  ╵",
        color: "line 1, column 6 of /tmp/x.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void \u{1b}[31mmain\u{1b}[0m() {\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m     ^^^^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 1, column 6 of /tmp/x.dart: Boom.\n  ╷\n1 │ void main() {\n  │      ^^^^\n  ╵",
    },
    SpanCase {
        name: "file_url",
        text: "void main() {\n  foo(1,\n      2);\n}\n",
        url: Some("file:///tmp/x.dart"),
        start: 5,
        end: 9,
        message: "Boom.",
        plain: "line 1, column 6 of /tmp/x.dart: Boom.\n  ╷\n1 │ void main() {\n  │      ^^^^\n  ╵",
        color: "line 1, column 6 of /tmp/x.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void \u{1b}[31mmain\u{1b}[0m() {\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m     ^^^^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 1, column 6 of /tmp/x.dart: Boom.\n  ╷\n1 │ void main() {\n  │      ^^^^\n  ╵",
    },
    SpanCase {
        name: "package_url",
        text: "void main() {\n  foo(1,\n      2);\n}\n",
        url: Some("package:foo/a.dart"),
        start: 5,
        end: 9,
        message: "Boom.",
        plain: "line 1, column 6 of package:foo/a.dart: Boom.\n  ╷\n1 │ void main() {\n  │      ^^^^\n  ╵",
        color: "line 1, column 6 of package:foo/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void \u{1b}[31mmain\u{1b}[0m() {\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m     ^^^^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 1, column 6 of package:foo/a.dart: Boom.\n  ╷\n1 │ void main() {\n  │      ^^^^\n  ╵",
    },
    SpanCase {
        name: "crlf_multi",
        text: "a\r\nbb(\r\n  c,\r\n  d)\r\n",
        url: Some("lib/a.dart"),
        start: 5,
        end: 18,
        message: "Boom.",
        plain: "line 2, column 3 of lib/a.dart: Boom.\n  ╷\n2 │   bb(\n  │ ┌───^\n3 │ │   c,\n4 │ └   d)\n  ╵",
        color: "line 2, column 3 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m   bb\u{1b}[31m(\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m┌\u{1b}[0m\u{1b}[31m───^\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m \u{1b}[31m│\u{1b}[0m \u{1b}[31m  c,\u{1b}[0m\n\u{1b}[34m4 │\u{1b}[0m \u{1b}[31m└\u{1b}[0m \u{1b}[31m  d)\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 2, column 3 of lib/a.dart: Boom.\n  ╷\n2 │   bb(\n  │ ┌───^\n3 │ │   c,\n4 │ └   d)\n  ╵",
    },
    SpanCase {
        name: "crlf_multi_to_eol",
        text: "a\r\nbb(\r\n  c,\r\n  d)\r\n",
        url: Some("lib/a.dart"),
        start: 3,
        end: 6,
        message: "Boom.",
        plain: "line 2, column 1 of lib/a.dart: Boom.\n  ╷\n2 │ bb(\r\n  │ ^^^\n  ╵",
        color: "line 2, column 1 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m \u{1b}[31mbb(\u{1b}[0m\r\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m^^^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 2, column 1 of lib/a.dart: Boom.\n  ╷\n2 │ bb(\r\n  │ ^^^\n  ╵",
    },
    SpanCase {
        name: "crlf_single",
        text: "a\r\nbb(\r\n  c,\r\n  d)\r\n",
        url: Some("lib/a.dart"),
        start: 10,
        end: 11,
        message: "Boom.",
        plain: "line 3, column 3 of lib/a.dart: Boom.\n  ╷\n3 │   c,\r\n  │   ^\n  ╵",
        color: "line 3, column 3 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m   \u{1b}[31mc\u{1b}[0m,\r\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m  ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 3, column 3 of lib/a.dart: Boom.\n  ╷\n3 │   c,\r\n  │   ^\n  ╵",
    },
    SpanCase {
        name: "crlf_including_newline",
        text: "a\r\nbb(\r\n  c,\r\n  d)\r\n",
        url: Some("lib/a.dart"),
        start: 3,
        end: 8,
        message: "Boom.",
        plain: "line 2, column 1 of lib/a.dart: Boom.\n  ╷\n2 │ bb(\n  │ ^^^\n  ╵",
        color: "line 2, column 1 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m \u{1b}[31mbb(\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m^^^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 2, column 1 of lib/a.dart: Boom.\n  ╷\n2 │ bb(\n  │ ^^^\n  ╵",
    },
    SpanCase {
        name: "cr_multi",
        text: "a\rbb(\r  c,\r  d)\r",
        url: Some("lib/a.dart"),
        start: 4,
        end: 15,
        message: "Boom.",
        plain: "line 2, column 3 of lib/a.dart: Boom.\n  ╷\n2 │   bb(\r  c,\r  d)\r\n  │ ┌───^\n  ╵",
        color: "line 2, column 3 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m   bb\u{1b}[31m(\r  c,\r  d)\r\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m┌\u{1b}[0m\u{1b}[31m───^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 2, column 3 of lib/a.dart: Boom.\n  ╷\n2 │   bb(\r  c,\r  d)\r\n  │ ┌───^\n  ╵",
    },
    SpanCase {
        name: "cr_single",
        text: "a\rbb(\r  c,\r  d)\r",
        url: Some("lib/a.dart"),
        start: 8,
        end: 9,
        message: "Boom.",
        plain: "line 3, column 3 of lib/a.dart: Boom.\n  ╷\n3 │   c,\r\n  │   ^\n  ╵",
        color: "line 3, column 3 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m   \u{1b}[31mc\u{1b}[0m,\r\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m  ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 3, column 3 of lib/a.dart: Boom.\n  ╷\n3 │   c,\r\n  │   ^\n  ╵",
    },
    SpanCase {
        name: "tab_multi",
        text: "void f() {\n\tif (x) {\n\t\ty(\n\t\t\tz);\n\t}\n}\n",
        url: Some("lib/a.dart"),
        start: 23,
        end: 31,
        message: "Boom.",
        plain: "line 3, column 3 of lib/a.dart: Boom.\n  ╷\n3 │ ┌         y(\n4 │ │             z);\n  │ └──────────────^\n  ╵",
        color: "line 3, column 3 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m \u{1b}[31m┌\u{1b}[0m         \u{1b}[31my(\u{1b}[0m\n\u{1b}[34m4 │\u{1b}[0m \u{1b}[31m│\u{1b}[0m \u{1b}[31m            z)\u{1b}[0m;\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m└\u{1b}[0m\u{1b}[31m──────────────^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 3, column 3 of lib/a.dart: Boom.\n  ╷\n3 │ ┌         y(\n4 │ │             z);\n  │ └──────────────^\n  ╵",
    },
    SpanCase {
        name: "tab_single",
        text: "void f() {\n\tif (x) {\n\t\ty(\n\t\t\tz);\n\t}\n}\n",
        url: Some("lib/a.dart"),
        start: 23,
        end: 24,
        message: "Boom.",
        plain: "line 3, column 3 of lib/a.dart: Boom.\n  ╷\n3 │         y(\n  │         ^\n  ╵",
        color: "line 3, column 3 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m         \u{1b}[31my\u{1b}[0m(\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 3, column 3 of lib/a.dart: Boom.\n  ╷\n3 │         y(\n  │         ^\n  ╵",
    },
    SpanCase {
        name: "tab_in_span",
        text: "void f() {\n\tif (x) {\n\t\ty(\n\t\t\tz);\n\t}\n}\n",
        url: Some("lib/a.dart"),
        start: 21,
        end: 25,
        message: "Boom.",
        plain: "line 3, column 1 of lib/a.dart: Boom.\n  ╷\n3 │         y(\n  │ ^^^^^^^^^^\n  ╵",
        color: "line 3, column 1 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m \u{1b}[31m        y(\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m^^^^^^^^^^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 3, column 1 of lib/a.dart: Boom.\n  ╷\n3 │         y(\n  │ ^^^^^^^^^^\n  ╵",
    },
    SpanCase {
        name: "w9_10",
        text: "l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\nl11\nl12\nl13\nl14\nl15\nl16\nl17\nl18\nl19\nl20\nl21\nl22\nl23\nl24\nl25\nl26\nl27\nl28\nl29\nl30\nl31\nl32\nl33\nl34\nl35\nl36\nl37\nl38\nl39\nl40\nl41\nl42\nl43\nl44\nl45\nl46\nl47\nl48\nl49\nl50\nl51\nl52\nl53\nl54\nl55\nl56\nl57\nl58\nl59\nl60\nl61\nl62\nl63\nl64\nl65\nl66\nl67\nl68\nl69\nl70\nl71\nl72\nl73\nl74\nl75\nl76\nl77\nl78\nl79\nl80\nl81\nl82\nl83\nl84\nl85\nl86\nl87\nl88\nl89\nl90\nl91\nl92\nl93\nl94\nl95\nl96\nl97\nl98\nl99\nl100\nl101\nl102\nl103\nl104\nl105\nl106\nl107\nl108\nl109\nl110\nl111\nl112\nl113\nl114\nl115\nl116\nl117\nl118\nl119\n",
        url: Some("lib/a.dart"),
        start: 24,
        end: 30,
        message: "Boom.",
        plain: "line 9, column 1 of lib/a.dart: Boom.\n   ╷\n9  │ ┌ l9\n10 │ └ l10\n   ╵",
        color: "line 9, column 1 of lib/a.dart: Boom.\n\u{1b}[34m   ╷\u{1b}[0m\n\u{1b}[34m9  │\u{1b}[0m \u{1b}[31m┌\u{1b}[0m \u{1b}[31ml9\u{1b}[0m\n\u{1b}[34m10 │\u{1b}[0m \u{1b}[31m└\u{1b}[0m \u{1b}[31ml10\u{1b}[0m\n\u{1b}[34m   ╵\u{1b}[0m",
        exception: "Error on line 9, column 1 of lib/a.dart: Boom.\n   ╷\n9  │ ┌ l9\n10 │ └ l10\n   ╵",
    },
    SpanCase {
        name: "w99_100",
        text: "l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\nl11\nl12\nl13\nl14\nl15\nl16\nl17\nl18\nl19\nl20\nl21\nl22\nl23\nl24\nl25\nl26\nl27\nl28\nl29\nl30\nl31\nl32\nl33\nl34\nl35\nl36\nl37\nl38\nl39\nl40\nl41\nl42\nl43\nl44\nl45\nl46\nl47\nl48\nl49\nl50\nl51\nl52\nl53\nl54\nl55\nl56\nl57\nl58\nl59\nl60\nl61\nl62\nl63\nl64\nl65\nl66\nl67\nl68\nl69\nl70\nl71\nl72\nl73\nl74\nl75\nl76\nl77\nl78\nl79\nl80\nl81\nl82\nl83\nl84\nl85\nl86\nl87\nl88\nl89\nl90\nl91\nl92\nl93\nl94\nl95\nl96\nl97\nl98\nl99\nl100\nl101\nl102\nl103\nl104\nl105\nl106\nl107\nl108\nl109\nl110\nl111\nl112\nl113\nl114\nl115\nl116\nl117\nl118\nl119\n",
        url: Some("lib/a.dart"),
        start: 383,
        end: 391,
        message: "Boom.",
        plain: "line 99, column 1 of lib/a.dart: Boom.\n    ╷\n99  │ ┌ l99\n100 │ └ l100\n    ╵",
        color: "line 99, column 1 of lib/a.dart: Boom.\n\u{1b}[34m    ╷\u{1b}[0m\n\u{1b}[34m99  │\u{1b}[0m \u{1b}[31m┌\u{1b}[0m \u{1b}[31ml99\u{1b}[0m\n\u{1b}[34m100 │\u{1b}[0m \u{1b}[31m└\u{1b}[0m \u{1b}[31ml100\u{1b}[0m\n\u{1b}[34m    ╵\u{1b}[0m",
        exception: "Error on line 99, column 1 of lib/a.dart: Boom.\n    ╷\n99  │ ┌ l99\n100 │ └ l100\n    ╵",
    },
    SpanCase {
        name: "w100",
        text: "l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\nl11\nl12\nl13\nl14\nl15\nl16\nl17\nl18\nl19\nl20\nl21\nl22\nl23\nl24\nl25\nl26\nl27\nl28\nl29\nl30\nl31\nl32\nl33\nl34\nl35\nl36\nl37\nl38\nl39\nl40\nl41\nl42\nl43\nl44\nl45\nl46\nl47\nl48\nl49\nl50\nl51\nl52\nl53\nl54\nl55\nl56\nl57\nl58\nl59\nl60\nl61\nl62\nl63\nl64\nl65\nl66\nl67\nl68\nl69\nl70\nl71\nl72\nl73\nl74\nl75\nl76\nl77\nl78\nl79\nl80\nl81\nl82\nl83\nl84\nl85\nl86\nl87\nl88\nl89\nl90\nl91\nl92\nl93\nl94\nl95\nl96\nl97\nl98\nl99\nl100\nl101\nl102\nl103\nl104\nl105\nl106\nl107\nl108\nl109\nl110\nl111\nl112\nl113\nl114\nl115\nl116\nl117\nl118\nl119\n",
        url: Some("lib/a.dart"),
        start: 387,
        end: 391,
        message: "Boom.",
        plain: "line 100, column 1 of lib/a.dart: Boom.\n    ╷\n100 │ l100\n    │ ^^^^\n    ╵",
        color: "line 100, column 1 of lib/a.dart: Boom.\n\u{1b}[34m    ╷\u{1b}[0m\n\u{1b}[34m100 │\u{1b}[0m \u{1b}[31ml100\u{1b}[0m\n\u{1b}[34m    │\u{1b}[0m \u{1b}[31m^^^^\u{1b}[0m\n\u{1b}[34m    ╵\u{1b}[0m",
        exception: "Error on line 100, column 1 of lib/a.dart: Boom.\n    ╷\n100 │ l100\n    │ ^^^^\n    ╵",
    },
    SpanCase {
        name: "emoji_before",
        text: "var s = '😀😀'; x y;\n",
        url: Some("lib/a.dart"),
        start: 18,
        end: 19,
        message: "Boom.",
        plain: "line 1, column 19 of lib/a.dart: Boom.\n  ╷\n1 │ var s = '😀😀'; x y;\n  │                   ^\n  ╵",
        color: "line 1, column 19 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m var s = '😀😀'; x \u{1b}[31my\u{1b}[0m;\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m                  ^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 1, column 19 of lib/a.dart: Boom.\n  ╷\n1 │ var s = '😀😀'; x y;\n  │                   ^\n  ╵",
    },
    SpanCase {
        name: "emoji_in_span",
        text: "var s = '😀😀'; x y;\n",
        url: Some("lib/a.dart"),
        start: 8,
        end: 14,
        message: "Boom.",
        plain: "line 1, column 9 of lib/a.dart: Boom.\n  ╷\n1 │ var s = '😀😀'; x y;\n  │         ^^^^^^\n  ╵",
        color: "line 1, column 9 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m var s = \u{1b}[31m'😀😀'\u{1b}[0m; x y;\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m        ^^^^^^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 1, column 9 of lib/a.dart: Boom.\n  ╷\n1 │ var s = '😀😀'; x y;\n  │         ^^^^^^\n  ╵",
    },
    SpanCase {
        name: "blank_line_end",
        text: "a\n\n",
        url: Some("lib/a.dart"),
        start: 1,
        end: 3,
        message: "Boom.",
        plain: "line 1, column 2 of lib/a.dart: Boom.\n  ╷\n1 │   a\n  │ ┌──^\n2 │ └ \n  ╵",
        color: "line 1, column 2 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m   a\u{1b}[31m\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m┌\u{1b}[0m\u{1b}[31m──^\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m \u{1b}[31m└\u{1b}[0m \u{1b}[31m\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 1, column 2 of lib/a.dart: Boom.\n  ╷\n1 │   a\n  │ ┌──^\n2 │ └ \n  ╵",
    },
    SpanCase {
        name: "two_newlines_text",
        text: "a\n\n\nb\n",
        url: Some("lib/a.dart"),
        start: 1,
        end: 3,
        message: "Boom.",
        plain: "line 1, column 2 of lib/a.dart: Boom.\n  ╷\n1 │   a\n  │ ┌──^\n2 │ └ \n  ╵",
        color: "line 1, column 2 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m   a\u{1b}[31m\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m┌\u{1b}[0m\u{1b}[31m──^\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m \u{1b}[31m└\u{1b}[0m \u{1b}[31m\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 1, column 2 of lib/a.dart: Boom.\n  ╷\n1 │   a\n  │ ┌──^\n2 │ └ \n  ╵",
    },
    SpanCase {
        name: "empty_text",
        text: "",
        url: Some("lib/a.dart"),
        start: 0,
        end: 0,
        message: "Boom.",
        plain: "line 1, column 1 of lib/a.dart: Boom.\n  ╷\n1 │ \n  │ ^\n  ╵",
        color: "line 1, column 1 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m \u{1b}[31m\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 1, column 1 of lib/a.dart: Boom.\n  ╷\n1 │ \n  │ ^\n  ╵",
    },
    SpanCase {
        name: "yaml",
        text: "formatter:\n  page_width: [1\n",
        url: None,
        start: 28,
        end: 28,
        message: "While parsing a flow sequence, expected ',' or ']'.",
        plain: "line 3, column 1: While parsing a flow sequence, expected ',' or ']'.\n  ╷\n3 │ \n  │ ^\n  ╵",
        color: "line 3, column 1: While parsing a flow sequence, expected ',' or ']'.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m3 │\u{1b}[0m \u{1b}[31m\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 3, column 1: While parsing a flow sequence, expected ',' or ']'.\n  ╷\n3 │ \n  │ ^\n  ╵",
    },
    SpanCase {
        name: "multiline_message",
        text: "void main() {\n  foo(1,\n      2);\n}\n",
        url: Some("lib/a.dart"),
        start: 5,
        end: 9,
        message: "line one\nline two",
        plain: "line 1, column 6 of lib/a.dart: line one\nline two\n  ╷\n1 │ void main() {\n  │      ^^^^\n  ╵",
        color: "line 1, column 6 of lib/a.dart: line one\nline two\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m void \u{1b}[31mmain\u{1b}[0m() {\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m     ^^^^\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 1, column 6 of lib/a.dart: line one\nline two\n  ╷\n1 │ void main() {\n  │      ^^^^\n  ╵",
    },
    SpanCase {
        name: "no_trailing_newline_multi",
        text: "a(\nb",
        url: Some("lib/a.dart"),
        start: 1,
        end: 4,
        message: "Boom.",
        plain: "line 1, column 2 of lib/a.dart: Boom.\n  ╷\n1 │   a(\n  │ ┌──^\n2 │ └ b\n  ╵",
        color: "line 1, column 2 of lib/a.dart: Boom.\n\u{1b}[34m  ╷\u{1b}[0m\n\u{1b}[34m1 │\u{1b}[0m   a\u{1b}[31m(\u{1b}[0m\n\u{1b}[34m  │\u{1b}[0m \u{1b}[31m┌\u{1b}[0m\u{1b}[31m──^\u{1b}[0m\n\u{1b}[34m2 │\u{1b}[0m \u{1b}[31m└\u{1b}[0m \u{1b}[31mb\u{1b}[0m\n\u{1b}[34m  ╵\u{1b}[0m",
        exception: "Error on line 1, column 2 of lib/a.dart: Boom.\n  ╷\n1 │   a(\n  │ ┌──^\n2 │ └ b\n  ╵",
    },
];

/// (uri, prettyUri) with `p.current` = [PRETTY_CURRENT]; THROWS marks a
/// `FormatException` of `Uri.parse` in Dart.
const PRETTY_CURRENT: &str = "/private/tmp/claude-501/-Users-dominik-Projects-dartr/2a3f573b-301e-4a14-8b44-8e2e6b74db28/scratchpad/p10fmt/span";
const PRETTY_CASES: &[(&str, &str)] = &[
    ("", "."),
    ("lib/bad.dart", "lib/bad.dart"),
    ("./lib/../bad.dart", "bad.dart"),
    ("../x.dart", "../x.dart"),
    ("../../a/b/c.dart", "../../a/b/c.dart"),
    ("/tmp/x.dart", "/tmp/x.dart"),
    ("/private/tmp/x.dart", "/private/tmp/x.dart"),
    (
        "/private/tmp/claude-501/-Users-dominik-Projects-dartr/2a3f573b-301e-4a14-8b44-8e2e6b74db28/scratchpad/p10fmt/span/cases/a.dart",
        "cases/a.dart",
    ),
    (
        "/private/tmp/claude-501/-Users-dominik-Projects-dartr/2a3f573b-301e-4a14-8b44-8e2e6b74db28/scratchpad/p10fmt/span",
        ".",
    ),
    (
        "/private/tmp/claude-501/-Users-dominik-Projects-dartr/2a3f573b-301e-4a14-8b44-8e2e6b74db28/scratchpad/p10fmt/span/",
        ".",
    ),
    (
        "/private/tmp/claude-501/-Users-dominik-Projects-dartr/2a3f573b-301e-4a14-8b44-8e2e6b74db28/scratchpad/p10fmt/span/../span/cases/x.dart",
        "cases/x.dart",
    ),
    (
        "/private/tmp/claude-501/-Users-dominik-Projects-dartr/2a3f573b-301e-4a14-8b44-8e2e6b74db28/scratchpad/p10fmt/other/x.dart",
        "../other/x.dart",
    ),
    (
        "/private/tmp/claude-501/-Users-dominik-Projects-dartr/2a3f573b-301e-4a14-8b44-8e2e6b74db28/scratchpad/x.dart",
        "../../x.dart",
    ),
    (
        "file:///private/tmp/claude-501/-Users-dominik-Projects-dartr/2a3f573b-301e-4a14-8b44-8e2e6b74db28/scratchpad/p10fmt/span/cases/a.dart",
        "cases/a.dart",
    ),
    ("file:///tmp/x.dart", "/tmp/x.dart"),
    ("file:x.dart", "/x.dart"),
    ("FILE:x.dart", "/x.dart"),
    ("package:foo/bar.dart", "package:foo/bar.dart"),
    ("https://dart.dev/x", "https://dart.dev/x"),
    ("my file.dart", "my file.dart"),
    ("a%41b.dart", "aAb.dart"),
    ("a%zz.dart", "a%zz.dart"),
    ("a#b.dart", "a"),
    ("a?b.dart", "a"),
    ("a b:c.dart", "THROWS FormatException"),
    ("1a:b.dart", "THROWS FormatException"),
    ("c:x.dart", "c:x.dart"),
    ("//host/x.dart", "/x.dart"),
    ("/a//b/./c.dart", "/a/b/c.dart"),
    ("żółw.dart", "żółw.dart"),
    ("dir/", "dir"),
];

fn exception_of(case: &ErrorCase) -> FormatterException {
    FormatterException {
        errors: case
            .errors
            .iter()
            .map(|&(offset, length, message)| FormatterError {
                unique_name: String::new(),
                offset,
                length,
                message: message.to_string(),
                source: case.source.to_string(),
                path: case.path.to_string(),
            })
            .collect(),
    }
}

/// Prints each mismatch and fails when there is any.
fn check(failures: Vec<String>, total: usize) {
    for failure in &failures {
        eprintln!("{failure}");
    }
    assert!(
        failures.is_empty(),
        "{} of {total} comparisons differ from Dart",
        failures.len()
    );
}

fn diff(name: &str, what: &str, expected: &str, actual: &str) -> Option<String> {
    (expected != actual)
        .then(|| format!("== {name} ({what})\n--- dart\n{expected:?}\n--- rust\n{actual:?}"))
}

#[test]
fn formatter_exception_message_matches_dart() {
    let mut failures = Vec::new();
    for case in ERROR_CASES {
        let exception = exception_of(case);
        failures.extend(diff(
            case.name,
            "plain",
            case.plain,
            &exception.message(false),
        ));
        failures.extend(diff(
            case.name,
            "color",
            case.color,
            &exception.message(true),
        ));
    }
    check(failures, ERROR_CASES.len() * 2);
}

/// Parses the case sources with the dartr parser, so this also compares the
/// diagnostics (offsets, lengths, messages) with the analyzer.
#[test]
fn format_source_error_message_matches_dart() {
    let formatter = DartFormatter::new(Version::new(3, 13));
    let mut failures = Vec::new();
    for case in ERROR_CASES {
        let source =
            SourceCode::new(case.source, Some(case.path.to_string()), true, None, None).unwrap();
        match formatter.format_source(&source) {
            Err(FormatError::Formatter(exception)) => {
                failures.extend(diff(
                    case.name,
                    "e2e plain",
                    case.plain,
                    &exception.message(false),
                ));
                failures.extend(diff(
                    case.name,
                    "e2e color",
                    case.color,
                    &exception.message(true),
                ));
            }
            other => failures.push(format!(
                "== {} (e2e): expected a FormatterException, got {other:?}",
                case.name
            )),
        }
    }
    check(failures, ERROR_CASES.len() * 2);
}

#[test]
fn span_message_matches_dart() {
    let mut failures = Vec::new();
    for case in SPAN_CASES {
        let message = |color| {
            span_message(
                case.text,
                case.url,
                case.start,
                case.end,
                case.message,
                color,
            )
        };
        failures.extend(diff(case.name, "plain", case.plain, &message(false)));
        failures.extend(diff(case.name, "color", case.color, &message(true)));
        let exception =
            span_exception_text(case.text, case.url, case.start, case.end, case.message);
        failures.extend(diff(case.name, "exception", case.exception, &exception));
    }
    check(failures, SPAN_CASES.len() * 3);
}

#[test]
fn pretty_uri_matches_dart() {
    let mut failures = Vec::new();
    for &(uri, expected) in PRETTY_CASES {
        // Dart throws for these; the port shows the URI unchanged.
        let expected = if expected.starts_with("THROWS") {
            uri
        } else {
            expected
        };
        failures.extend(diff(
            uri,
            "prettyUri",
            expected,
            &pretty_uri_from(uri, PRETTY_CURRENT),
        ));
    }
    check(failures, PRETTY_CASES.len());
}
