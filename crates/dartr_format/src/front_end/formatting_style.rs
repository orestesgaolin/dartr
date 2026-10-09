// Dart source: dart_style lib/src/front_end/formatting_style.dart

use dartr_ast::Ast;
use dartr_syntax::TokenId;

use crate::ast_extensions::has_comma_before;
use crate::dart_formatter::{DartFormatter, TrailingCommas};
use crate::dart_version_history::Version;

/// The formatting style that should be applied to code.
///
/// This is sort of the internal version of [DartFormatter]. The former is
/// public API so is limited in what it exposes. This contains getters for
/// internal use to determine what style rules to apply.
///
/// This also tracks how language version affects the style rules. From Dart
/// 3.7 and forward, most changes to the formatting style are language
/// versioned: code whose language version is older than a style change will
/// retain the older style.
#[derive(Clone, Debug)]
pub struct FormattingStyle {
    /// The language version of the style.
    ///
    /// Usually the same version as the formatter, but may be different if the
    /// file being formatted has an `@dart=` comment.
    language_version: Version,

    /// The number of characters allowed in a single line.
    ///
    /// Usually the same as the formatter's but may be different if the file
    /// being formatted has a `// dart format width = ` comment.
    pub page_width: i32,

    pub line_ending: String,

    /// The number of characters of indentation to prefix the output lines
    /// with.
    pub leading_indent: i32,

    /// The formatter's trailing comma handling.
    trailing_commas: TrailingCommas,
}

const VERSION_3_DOT_7: Version = Version::new(3, 7);
const VERSION_3_DOT_10: Version = Version::new(3, 10);
const VERSION_3_DOT_13: Version = Version::new(3, 13);

impl FormattingStyle {
    pub fn new(
        formatter: &DartFormatter,
        line_ending: &str,
        language_version: Option<Version>,
        page_width: Option<usize>,
    ) -> FormattingStyle {
        FormattingStyle {
            language_version: language_version.unwrap_or(formatter.language_version),
            page_width: page_width.unwrap_or(formatter.page_width) as i32,
            line_ending: line_ending.to_string(),
            leading_indent: formatter.indent as i32,
            trailing_commas: formatter.trailing_commas,
        }
    }

    /// Whether the code being formatted is at language version 3.7 and
    /// doesn't include the sweeping style changes in 3.8.
    pub fn is_3_dot_7(&self) -> bool {
        self.language_version == VERSION_3_DOT_7
    }

    /// Whether a trailing comma should be preserved after for-loop updaters.
    pub fn preserve_trailing_comma_after_for_updaters(&self) -> bool {
        self.trailing_commas == TrailingCommas::Preserve
    }

    /// Whether a trailing comma should be preserved after enum values.
    pub fn preserve_trailing_comma_after_enum_values(&self) -> bool {
        self.trailing_commas == TrailingCommas::Preserve
            && self.language_version >= VERSION_3_DOT_10
    }

    /// Whether the formatter should penalize splitting in the target of a
    /// call chain if the target is an argument list with only one argument or
    /// a collection literal with only one element.
    pub fn avoid_splitting_single_element_call_chain_targets(&self) -> bool {
        self.language_version >= VERSION_3_DOT_13
    }

    /// Whether mixin declarations and extension types with brace bodies
    /// should always get a blank line above and below them.
    ///
    /// They always should have, but they were overlooked. We already do this
    /// for classes, enums, and extensions.
    pub fn blank_line_around_mixin_and_extension_types(&self) -> bool {
        self.language_version >= VERSION_3_DOT_13
    }

    /// Whether parameter lists should be block formatted in things like
    /// typedefs.
    pub fn block_format_parameter_lists(&self) -> bool {
        self.language_version >= VERSION_3_DOT_13
    }

    /// Whether the LHS of an `as`, `is`, or `is!` expression can be block
    /// formatted.
    pub fn block_format_type_test(&self) -> bool {
        self.language_version >= VERSION_3_DOT_13
    }

    /// Whether an if-case pattern can be block-formatted when there is a
    /// guard clause as well.
    pub fn block_format_if_case_with_guard(&self) -> bool {
        self.language_version < VERSION_3_DOT_13
    }

    /// Whether the formatter should prefer overflow from "soft" characters
    /// versus others when no solution fits the page width and an overflowing
    /// solution must be chosen.
    ///
    /// Sometimes the formatter does it's best, but no solution fits in the
    /// page width. Usually, this is because the code has some long string
    /// literals or comments that the user should split manually. If the
    /// formatter treats all overflowing characters uniformly, then it will
    /// try to pick a solution that minimizes those overhanging strings and
    /// comments at the expense of choosing weird formatting for other code.
    ///
    /// What we want is for the formatter to leave those strings or comments
    /// hanging past the page width so it's clear to the user where they need
    /// to split things to get everything to fit.
    ///
    /// To do that, the formatter distinguishes "soft" characters from other
    /// kinds of code. "Soft" code is string literals, comments, or a few
    /// other things that often follow a string literal or comment: `,`, `;`,
    /// or `() {` (including `async` or other modifiers that can appear in a
    /// function header) for a trailing block-formatted lambda. When an
    /// overflowing line of code ends in soft characters, the overflow cost of
    /// all of those characters is collapsed to a single point of penalty
    /// instead of one per character.
    ///
    /// This feature has no effect on code that does fit in the page width.
    pub fn use_soft_overflow(&self) -> bool {
        self.language_version >= VERSION_3_DOT_13
    }

    /// Whether to force a blank line between imports and exports whose URIs
    /// are different categories: `dart:`, `package:`, or relative.
    pub fn separate_directive_sections(&self) -> bool {
        self.language_version >= VERSION_3_DOT_13
    }

    /// Whether to try to figure out a piece's state based on the page width
    /// before running the solver or during.
    ///
    /// Initially, this ran during solving but that leads to some subtle bugs
    /// in the solver. Performing it before solving is less effective for
    /// performance but avoids those bugs.
    ///
    /// We language version this even though the old logic was never correct
    /// to minimize unexpected churn.
    pub fn pin_state_by_page_width_before_solving(&self) -> bool {
        self.language_version >= VERSION_3_DOT_13
    }

    /// Whether an extension type's representation clause allows a trailing
    /// comma.
    ///
    /// When primary constructors were added in Dart 3.13, the grammar was
    /// adjusted to define extension types in terms of them which also means
    /// that a trailing comma is now permitted.
    pub fn allow_trailing_comma_in_representation_clause(&self) -> bool {
        self.language_version >= VERSION_3_DOT_13
    }

    /// Whether there is a trailing comma at the end of the list delimited by
    /// [right_bracket] which should be preserved by this style.
    pub fn preserve_trailing_comma_before(&self, ast: &Ast, right_bracket: TokenId) -> bool {
        self.trailing_commas == TrailingCommas::Preserve && has_comma_before(ast, right_bracket)
    }
}
