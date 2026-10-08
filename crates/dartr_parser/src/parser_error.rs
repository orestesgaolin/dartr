// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/parser_error.dart

use dartr_diagnostics::cfe::CfeMessage;
use dartr_syntax::{TokenId, Tokens};

#[derive(Clone, Debug, PartialEq)]
pub struct ParserError {
    pub begin_offset: u32,
    pub end_offset: u32,
    pub message: CfeMessage,
}

impl ParserError {
    pub fn new(begin_offset: u32, end_offset: u32, message: CfeMessage) -> Self {
        ParserError {
            begin_offset,
            end_offset,
            message,
        }
    }

    /// Dart `ParserError.fromTokens`.
    pub fn from_tokens(tokens: &Tokens, begin: TokenId, end: TokenId, message: CfeMessage) -> Self {
        Self::new(tokens.get(begin).offset, tokens.get(end).end(), message)
    }
}

impl std::fmt::Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "@{}: {}\n{}",
            self.begin_offset,
            self.message.problem_message,
            self.message.correction_message.as_deref().unwrap_or("null")
        )
    }
}
