// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/token_stream_rewriter.dart

//! Inserting synthetic tokens into the token stream.
//!
//! Dart has two implementations: `TokenStreamRewriterImpl` changes the
//! tokens, `UndoableTokenStreamRewriter` also records each change so that
//! `undo` can restore the stream (used for look-ahead parsing). Here
//! [`TokenStreamRewriter`] borrows the token arena and, for the undoable
//! form, a change log ([`TokenStreamChange`]s); [`undo`] restores a log.
//!
//! The analyzer `AstBuilder` uses the parser's rewriter from inside
//! listener events; it creates `TokenStreamRewriter::new(tokens)` with the
//! token arena that the event receives (the undoable rewriter is only active
//! while events are dropped).

use dartr_syntax::{TokenId, TokenType, Tokens};

/// One recorded change of an `UndoableTokenStreamRewriter`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenStreamChange {
    /// Dart `NextTokenStreamChange`.
    Next {
        set_on: TokenId,
        set_on_next: TokenId,
        next_token: TokenId,
        next_token_previous: TokenId,
        next_token_before_synthetic: TokenId,
    },
    /// Dart `EndGroupTokenStreamChange`.
    EndGroup { set_on: TokenId, end_group: TokenId },
    /// Dart `OffsetTokenStreamChange`.
    Offset { set_on: TokenId, offset: u32 },
    /// Dart `PrecedingCommentsTokenStreamChange`.
    PrecedingComments { set_on: TokenId, comment: TokenId },
    /// Dart `PreviousTokenStreamChange`.
    Previous { set_on: TokenId, previous: TokenId },
}

/// Dart `UndoableTokenStreamRewriter.undo`: reverts the changes of [changes]
/// (newest first) and clears the log.
pub fn undo(tokens: &mut Tokens, changes: &mut Vec<TokenStreamChange>) {
    while let Some(change) = changes.pop() {
        match change {
            TokenStreamChange::Next {
                set_on,
                set_on_next,
                next_token,
                next_token_previous,
                next_token_before_synthetic,
            } => {
                let n = tokens.get_mut(next_token);
                n.before_synthetic = next_token_before_synthetic;
                n.previous = next_token_previous;
                tokens.get_mut(set_on).next = set_on_next;
            }
            TokenStreamChange::EndGroup { set_on, end_group } => {
                tokens.get_mut(set_on).end_group = end_group;
            }
            TokenStreamChange::Offset { set_on, offset } => {
                tokens.get_mut(set_on).offset = offset;
            }
            TokenStreamChange::PrecedingComments { set_on, comment } => {
                tokens.get_mut(set_on).preceding_comments = comment;
            }
            TokenStreamChange::Previous { set_on, previous } => {
                tokens.get_mut(set_on).previous = previous;
            }
        }
    }
}

/// Dart `TokenStreamRewriter` (`TokenStreamRewriterImpl`, or
/// `UndoableTokenStreamRewriter` when [`Self::changes`] is set).
pub struct TokenStreamRewriter<'a> {
    pub tokens: &'a mut Tokens,
    /// The change log of an undoable rewriter.
    pub changes: Option<&'a mut Vec<TokenStreamChange>>,
}

impl<'a> TokenStreamRewriter<'a> {
    /// Dart `TokenStreamRewriterImpl`.
    pub fn new(tokens: &'a mut Tokens) -> Self {
        TokenStreamRewriter {
            tokens,
            changes: None,
        }
    }

    /// Dart `UndoableTokenStreamRewriter`, recording into [changes].
    pub fn undoable(tokens: &'a mut Tokens, changes: &'a mut Vec<TokenStreamChange>) -> Self {
        TokenStreamRewriter {
            tokens,
            changes: Some(changes),
        }
    }

    fn next(&self, token: TokenId) -> TokenId {
        self.tokens.next(token)
    }

    fn char_offset(&self, token: TokenId) -> u32 {
        self.tokens.get(token).offset
    }

    fn byte_offset(&self, token: TokenId) -> u32 {
        self.tokens.byte_offset(token)
    }

    /// The check of Dart: throw if the token is eof, though allow an
    /// eof-token if the offset is negative (`syntheticPreviousToken`
    /// sometimes creates eof tokens that aren't the real eof-token).
    fn check_not_eof(&self, token: TokenId) {
        let t = self.tokens.get(token);
        if !(!t.is_eof() || t.signed_offset() < 0) {
            // Inserting here could cause a infinite loop. We prefer to crash.
            panic!("Internal Error: Rewriting at eof.");
        }
    }

    /// Drops any tokens after [before] and before [after].
    pub fn drop_range(&mut self, before: TokenId, after: TokenId) {
        self.set_next(before, after);
    }

    /// Insert a synthetic open and close parenthesis and return the new
    /// synthetic open parenthesis. If [include_identifier] is true, then a
    /// synthetic identifier is included between the open and close
    /// parenthesis.
    pub fn insert_parens(&mut self, token: TokenId, include_identifier: bool) -> TokenId {
        self.check_not_eof(token);
        let next = self.next(token);
        let offset = self.char_offset(next);
        let byte = self.byte_offset(next);
        let left_paren = self
            .tokens
            .push_synthetic(TokenType::OPEN_PAREN, offset, byte);
        let mut next = left_paren;
        if include_identifier {
            let identifier =
                self.tokens
                    .push_synthetic_string(TokenType::IDENTIFIER, "", offset, byte, Some(0));
            next = self.set_next(next, identifier);
        }
        let close = self
            .tokens
            .push_synthetic(TokenType::CLOSE_PAREN, offset, byte);
        next = self.set_next(next, close);
        self.set_end_group(left_paren, next);
        let after = self.next(token);
        self.set_next(next, after);

        // A no-op rewriter could skip this step.
        self.set_next(token, left_paren);

        left_paren
    }

    /// Insert a synthetic `{` and `}` after [previous_token] and return them.
    pub fn insert_block(&mut self, previous_token: TokenId) -> (TokenId, TokenId) {
        let following_token = self.next(previous_token);
        let offset = self.char_offset(following_token);
        let byte = self.byte_offset(following_token);
        let left_bracket = self
            .tokens
            .push_synthetic(TokenType::OPEN_CURLY_BRACKET, offset, byte);
        let right_bracket = self
            .tokens
            .push_synthetic(TokenType::CLOSE_CURLY_BRACKET, offset, byte);
        self.insert_token(previous_token, left_bracket);
        self.insert_token(left_bracket, right_bracket);
        self.set_end_group(left_bracket, right_bracket);
        (left_bracket, right_bracket)
    }

    /// Insert [new_token] after [token] and return [new_token].
    pub fn insert_token(&mut self, token: TokenId, new_token: TokenId) -> TokenId {
        self.check_not_eof(token);
        let next = self.next(token);
        self.set_next(new_token, next);

        // A no-op rewriter could skip this step.
        self.set_next(token, new_token);

        new_token
    }

    /// Move [end_group] (a synthetic `)`, `]`, or `}` token) and associated
    /// error token after [token] in the token stream and return [end_group].
    pub fn move_synthetic(&mut self, token: TokenId, end_group: TokenId) -> TokenId {
        self.check_not_eof(token);
        debug_assert!(self.tokens.get(end_group).before_synthetic.is_some());
        if token == end_group {
            return end_group;
        }
        let mut error_token = None;
        let after_end_group = self.next(end_group);
        if let Some(error) = self.tokens.error(after_end_group)
            && error.begin().is_some()
        {
            // `endGroup.next is UnmatchedToken`.
            error_token = Some(after_end_group);
        }

        // Remove endGroup from its current location
        let before = self.tokens.get(end_group).before_synthetic;
        let after = self.next(error_token.unwrap_or(end_group));
        self.set_next(before, after);

        // Insert endGroup into its new location
        let next = self.next(token);
        self.set_next(token, end_group);
        self.set_next(error_token.unwrap_or(end_group), next);
        let offset = self.char_offset(next);
        self.set_offset(end_group, offset);
        if let Some(error_token) = error_token {
            self.set_offset(error_token, offset);
        }

        end_group
    }

    /// Replace the single token immediately following the [previous_token]
    /// with the chain of tokens starting at the [replacement_token]. Return
    /// the [replacement_token].
    pub fn replace_token_following(
        &mut self,
        previous_token: TokenId,
        replacement_token: TokenId,
    ) -> TokenId {
        let replaced_token = self.next(previous_token);
        self.set_next(previous_token, replacement_token);

        let comments = self.tokens.get(replaced_token).preceding_comments;
        self.set_preceding_comments(replacement_token, comments);

        let last = self.last_token_in_chain(replacement_token);
        let after = self.next(replaced_token);
        self.set_next(last, after);

        replacement_token
    }

    /// Given the [first_token] in a chain of tokens to be inserted, return
    /// the last token in the chain.
    ///
    /// As a side-effect, this method also ensures that the tokens in the
    /// chain have their `previous` pointers set correctly.
    fn last_token_in_chain(&mut self, first_token: TokenId) -> TokenId {
        let mut previous: Option<TokenId> = None;
        let mut current = first_token;
        loop {
            let next = self.tokens.get(current).next;
            if next.is_none() || self.tokens.ty(next) == TokenType::EOF {
                break;
            }
            if let Some(previous) = previous {
                self.set_previous(current, previous);
            }
            previous = Some(current);
            current = next;
        }
        if let Some(previous) = previous {
            self.set_previous(current, previous);
        }
        current
    }

    /// Insert a new simple synthetic token of [new_token_type] after
    /// [previous_token] instead of the token actually coming after it and
    /// return the new token. The old token will be linked from the new one
    /// though, so it's not totally gone.
    pub fn replace_next_token_with_synthetic_token(
        &mut self,
        previous_token: TokenId,
        new_token_type: TokenType,
    ) -> TokenId {
        debug_assert!(
            !new_token_type.is_keyword(),
            "use an unwritten variation of insertSyntheticKeyword instead"
        );

        // [token] <--> [a] <--> [b]
        let replaced = self.next(previous_token);
        let replacement = self.tokens.push_replacement(new_token_type, replaced);
        self.insert_token(previous_token, replacement);
        // [token] <--> [replacement] <--> [a] <--> [b]
        let b = self.next(self.next(replacement));
        self.set_next(replacement, b);
        // [token] <--> [replacement] <--> [b]

        replacement
    }

    /// Insert a new simple synthetic token of [new_token_type] after
    /// [previous_token] instead of the [count] tokens actually coming after
    /// it and return the new token.
    pub fn replace_next_tokens_with_synthetic_token(
        &mut self,
        previous_token: TokenId,
        mut count: i32,
        new_token_type: TokenType,
    ) -> TokenId {
        debug_assert!(
            !new_token_type.is_keyword(),
            "use an unwritten variation of insertSyntheticKeyword instead"
        );

        // [token] <--> [a_1] <--> ... <--> [a_n] <--> [b]
        let replaced = self.next(previous_token);
        let replacement = self.tokens.push_replacement(new_token_type, replaced);
        self.insert_token(previous_token, replacement);
        // [token] <--> [replacement] <--> [a_1] <--> ... <--> [a_n] <--> [b]

        let mut end = self.next(replacement);
        while count > 0 {
            count -= 1;
            end = self.next(end);
        }
        self.set_next(replacement, end);
        // [token] <--> [replacement] <--> [b]

        replacement
    }

    /// Insert a synthetic identifier after [token] and return the new
    /// identifier.
    pub fn insert_synthetic_identifier(&mut self, token: TokenId, value: &str) -> TokenId {
        let next = self.next(token);
        let offset = self.char_offset(next);
        let byte = self.byte_offset(next);
        let identifier =
            self.tokens
                .push_synthetic_string(TokenType::IDENTIFIER, value, offset, byte, Some(0));
        self.insert_token(token, identifier)
    }

    /// Insert a new synthetic [keyword] after [token] and return the new
    /// token.
    pub fn insert_synthetic_keyword(&mut self, token: TokenId, keyword: TokenType) -> TokenId {
        let next = self.next(token);
        let offset = self.char_offset(next);
        let byte = self.byte_offset(next);
        let new_token = self.tokens.push_synthetic(keyword, offset, byte);
        self.insert_token(token, new_token)
    }

    /// Insert a new simple synthetic token of [new_token_type] after [token]
    /// and return the new token.
    pub fn insert_synthetic_token(&mut self, token: TokenId, new_token_type: TokenType) -> TokenId {
        debug_assert!(
            !new_token_type.is_keyword(),
            "use insertSyntheticKeyword instead"
        );
        let next = self.next(token);
        let offset = self.char_offset(next);
        let byte = self.byte_offset(next);
        let new_token = self.tokens.push_synthetic(new_token_type, offset, byte);
        self.insert_token(token, new_token)
    }

    /// Dart `_setNext`.
    fn set_next(&mut self, set_on: TokenId, next_token: TokenId) -> TokenId {
        match &mut self.changes {
            None => self.tokens.set_next(set_on, next_token),
            Some(changes) => {
                let n = self.tokens.get(next_token);
                changes.push(TokenStreamChange::Next {
                    set_on,
                    set_on_next: self.tokens.get(set_on).next,
                    next_token,
                    next_token_previous: n.previous,
                    next_token_before_synthetic: n.before_synthetic,
                });
                self.tokens.set_next(set_on, next_token);
            }
        }
        next_token
    }

    /// Dart `_setEndGroup`.
    fn set_end_group(&mut self, set_on: TokenId, end_group: TokenId) {
        if let Some(changes) = &mut self.changes {
            changes.push(TokenStreamChange::EndGroup {
                set_on,
                end_group: self.tokens.get(set_on).end_group,
            });
        }
        self.tokens.get_mut(set_on).end_group = end_group;
    }

    /// Dart `_setOffset`.
    fn set_offset(&mut self, set_on: TokenId, offset: u32) {
        if let Some(changes) = &mut self.changes {
            changes.push(TokenStreamChange::Offset {
                set_on,
                offset: self.tokens.get(set_on).offset,
            });
        }
        self.tokens.get_mut(set_on).offset = offset;
    }

    /// Dart `_setPrecedingComments`.
    fn set_preceding_comments(&mut self, set_on: TokenId, comment: TokenId) {
        if let Some(changes) = &mut self.changes {
            changes.push(TokenStreamChange::PrecedingComments {
                set_on,
                comment: self.tokens.get(set_on).preceding_comments,
            });
        }
        self.tokens.get_mut(set_on).preceding_comments = comment;
    }

    /// Dart `_setPrevious`.
    fn set_previous(&mut self, set_on: TokenId, previous: TokenId) {
        if let Some(changes) = &mut self.changes {
            changes.push(TokenStreamChange::Previous {
                set_on,
                previous: self.tokens.get(set_on).previous,
            });
        }
        self.tokens.get_mut(set_on).previous = previous;
    }
}
