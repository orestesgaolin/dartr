// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_quotes.dart

//! Dart `ConvertQuotes`, `ConvertToSingleQuotes` and
//! `ConvertToDoubleQuotes`.

use dartr_ast::*;

use super::super::change_builder::ChangeBuilder;
use super::super::fix_kind::FixKind;
use super::super::generated::fix_kinds as k;
use super::super::producer::*;

/// Which conversion.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum QuotesKind {
    /// Dart `ConvertQuotes` (to the other quote).
    Swap,
    ToSingle,
    ToDouble,
}

/// Dart `_ConvertQuotes` and its subclasses.
pub struct ConvertQuotes {
    pub kind: QuotesKind,
    from_single: bool,
}

impl ConvertQuotes {
    pub fn new(kind: QuotesKind) -> Self {
        ConvertQuotes {
            kind,
            from_single: kind == QuotesKind::ToDouble,
        }
    }
}

/// The quote facts of a string token lexeme: raw, multiline, single quoted.
#[derive(Clone, Copy)]
pub struct StringFacts {
    pub is_raw: bool,
    pub is_multiline: bool,
    pub is_single_quoted: bool,
}

pub fn string_facts(lexeme: &str) -> StringFacts {
    let is_raw = lexeme.starts_with('r');
    let rest = if is_raw { &lexeme[1..] } else { lexeme };
    let is_multiline = rest.starts_with("'''") || rest.starts_with("\"\"\"");
    StringFacts {
        is_raw,
        is_multiline,
        is_single_quoted: rest.starts_with('\''),
    }
}

/// The facts of a `StringInterpolation` (from its first string).
fn interpolation_facts(ast: &Ast, node: Id<StringInterpolation>) -> Option<StringFacts> {
    let first = *ast.list_raw(ast[node].elements).first()?;
    let first = ast.cast::<InterpolationString>(first)?;
    Some(string_facts(ast.tokens.lexeme(ast[first].contents)))
}

struct QuotePair {
    new_quote: u16,
    new_quote_string: &'static str,
    new_quote_multiline: &'static str,
    opposite_quote: u16,
}

const TO_DOUBLE: QuotePair = QuotePair {
    new_quote: 0x22,
    new_quote_string: "\"",
    new_quote_multiline: "\"\"\"",
    opposite_quote: 0x27,
};

const TO_SINGLE: QuotePair = QuotePair {
    new_quote: 0x27,
    new_quote_string: "'",
    new_quote_multiline: "'''",
    opposite_quote: 0x22,
};

const BACKSLASH: u16 = 0x5C;
const DOLLAR: u16 = 0x24;

impl ConvertQuotes {
    fn quotes(&self) -> &'static QuotePair {
        if self.from_single {
            &TO_DOUBLE
        } else {
            &TO_SINGLE
        }
    }

    fn insert_backslash_at(c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>, offset: u32) {
        builder.add_dart_file_edit(c.path, |b| b.add_simple_insertion(offset, "\\"));
    }

    fn can_keep_as_raw(&self, text: &str) -> bool {
        let q = self.quotes().new_quote_string;
        if text.ends_with(q) {
            return false;
        }
        !text.contains(&q.repeat(3))
    }

    fn escape_backslashes_and_dollars(
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        text: &[u16],
        offset: u32,
    ) {
        for (i, &ch) in text.iter().enumerate() {
            if ch == BACKSLASH || ch == DOLLAR {
                Self::insert_backslash_at(c, builder, offset + i as u32);
            }
        }
    }

    fn fix_backslashes_for_quotes(
        &self,
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        text: &[u16],
        offset: u32,
        is_multiline: bool,
        contains_string_end: bool,
    ) {
        let q = self.quotes();
        let mut is_escaping = false;
        let mut quote_count = 0;
        for (i, &ch) in text.iter().enumerate() {
            if ch == q.new_quote {
                if !is_escaping {
                    if is_multiline {
                        quote_count += 1;
                        if quote_count == 3 {
                            Self::insert_backslash_at(c, builder, offset + i as u32);
                            quote_count = 0;
                        } else if contains_string_end && i + 1 == text.len() {
                            Self::insert_backslash_at(c, builder, offset + i as u32);
                        }
                    } else {
                        Self::insert_backslash_at(c, builder, offset + i as u32);
                    }
                } else {
                    is_escaping = false;
                }
            } else {
                quote_count = 0;
                if ch == q.opposite_quote && is_escaping {
                    let o = offset + i as u32 - 1;
                    builder.add_dart_file_edit(c.path, |b| b.add_deletion(o, 1));
                    is_escaping = false;
                }
                is_escaping = ch == BACKSLASH && !is_escaping;
            }
        }
    }

    fn new_quote(&self, is_multiline: bool) -> &'static str {
        if is_multiline {
            self.quotes().new_quote_multiline
        } else {
            self.quotes().new_quote_string
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn replace_quotes(
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        offset: u32,
        end: u32,
        new_quote: &str,
        was_raw: bool,
        is_raw: bool,
    ) {
        let end_quote_length = new_quote.len() as u32;
        let mut start_quote_length = end_quote_length;
        let mut start_quote_offset = offset;
        if was_raw {
            if is_raw {
                start_quote_offset += 1;
            } else {
                start_quote_length += 1;
            }
        }
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(start_quote_offset, start_quote_length, new_quote);
            b.add_simple_replacement(end - end_quote_length, end_quote_length, new_quote);
        });
    }

    fn simple_string_literal(
        &self,
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        node: Id<SimpleStringLiteral>,
    ) {
        let ast = c.ast;
        let token = ast[node].literal;
        let facts = string_facts(ast.tokens.lexeme(token));
        if self.from_single != facts.is_single_quoted {
            return;
        }
        if ast.tokens.get(token).is_synthetic() {
            return;
        }
        let mut is_raw = facts.is_raw;
        let quote_length = if facts.is_multiline { 3 } else { 1 };
        let mut offset = c.token_offset(token) + quote_length;
        let token_length = ast.tokens.get(token).length;
        let mut text: Vec<u16> = c
            .utils
            .get_text(offset, token_length.saturating_sub(2 * quote_length))
            .encode_utf16()
            .collect();
        if is_raw {
            offset += 1;
            if !text.is_empty() {
                text.remove(0);
            }
            is_raw = self.can_keep_as_raw(&String::from_utf16_lossy(&text));
            if !is_raw {
                Self::escape_backslashes_and_dollars(c, builder, &text, offset);
            }
        }
        self.fix_backslashes_for_quotes(c, builder, &text, offset, facts.is_multiline, true);
        Self::replace_quotes(
            c,
            builder,
            ast.offset(node),
            ast.end(node),
            self.new_quote(facts.is_multiline),
            facts.is_raw,
            is_raw,
        );
    }

    fn string_interpolation(
        &self,
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        node: Id<StringInterpolation>,
    ) {
        let ast = c.ast;
        let Some(facts) = interpolation_facts(ast, node) else {
            return;
        };
        if self.from_single != facts.is_single_quoted {
            return;
        }
        let elements = ast.list_raw(ast[node].elements).to_vec();
        let (Some(&first), Some(&last)) = (elements.first(), elements.last()) else {
            return;
        };
        if ast.tokens.get(ast.end_token(last)).is_synthetic()
            || ast.tokens.get(ast.begin_token(first)).is_synthetic()
        {
            return;
        }
        let new_quote = self.new_quote(facts.is_multiline);
        for &element in &elements {
            if !ast.is::<InterpolationString>(element) {
                continue;
            }
            let mut offset = ast.offset(element);
            let mut length = ast.length(element);
            let mut contains_string_end = false;
            if element == first {
                offset += new_quote.len() as u32;
            } else if element == last {
                length -= new_quote.len() as u32;
                contains_string_end = true;
            }
            let text: Vec<u16> = c.utils.get_text(offset, length).encode_utf16().collect();
            self.fix_backslashes_for_quotes(
                c,
                builder,
                &text,
                offset,
                facts.is_multiline,
                contains_string_end,
            );
        }
        Self::replace_quotes(
            c,
            builder,
            ast.offset(node),
            ast.end(node),
            new_quote,
            false,
            false,
        );
    }
}

impl CorrectionProducer for ConvertQuotes {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(match self.kind {
            QuotesKind::Swap => &k::CONVERT_QUOTES,
            QuotesKind::ToSingle => &k::CONVERT_TO_SINGLE_QUOTED_STRING,
            QuotesKind::ToDouble => &k::CONVERT_TO_DOUBLE_QUOTED_STRING,
        })
    }

    fn assist_kind(&self) -> Option<&'static FixKind> {
        use crate::correction::generated::assist_kinds as a;
        match self.kind {
            QuotesKind::Swap => None,
            QuotesKind::ToSingle => Some(&a::CONVERT_TO_SINGLE_QUOTED_STRING),
            QuotesKind::ToDouble => Some(&a::CONVERT_TO_DOUBLE_QUOTED_STRING),
        }
    }

    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        Some(match self.kind {
            QuotesKind::Swap => &k::CONVERT_QUOTES_MULTI,
            QuotesKind::ToSingle => &k::CONVERT_TO_SINGLE_QUOTED_STRING_MULTI,
            QuotesKind::ToDouble => &k::CONVERT_TO_DOUBLE_QUOTED_STRING_MULTI,
        })
    }

    fn applicability(&self) -> Applicability {
        Applicability::Automatically
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let node = c.node;
        let interpolation_parent = || {
            ast.cast::<InterpolationString>(node)
                .and_then(|_| ast.parent(node))
                .and_then(|p| ast.cast::<StringInterpolation>(p))
        };
        if self.kind == QuotesKind::Swap {
            if let Some(s) = ast.cast::<SimpleStringLiteral>(node) {
                self.from_single = string_facts(ast.tokens.lexeme(ast[s].literal)).is_single_quoted;
            } else if let Some(i) = ast
                .cast::<StringInterpolation>(node)
                .or_else(interpolation_parent)
            {
                if let Some(f) = interpolation_facts(ast, i) {
                    self.from_single = f.is_single_quoted;
                }
            }
        }
        if let Some(s) = ast.cast::<SimpleStringLiteral>(node) {
            self.simple_string_literal(c, builder, s);
        } else if let Some(i) = ast
            .cast::<StringInterpolation>(node)
            .or_else(interpolation_parent)
        {
            self.string_interpolation(c, builder, i);
        }
    }
}
