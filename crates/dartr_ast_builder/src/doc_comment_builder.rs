// Dart source: pkg/analyzer/lib/src/fasta/doc_comment_builder.dart

//! Builds the `Comment` node of a documentation comment: comment
//! references, Markdown code blocks, doc directives and doc imports (Dart
//! `DocCommentBuilder`).

use dartr_ast::{Ast, Comment, Id, NodeList};
use dartr_parser::experimental_features::ExperimentalFeatures;
use dartr_syntax::TokenId;

use crate::error_converter::FastaErrorReporter;
use crate::parse::LibraryLanguageVersion;

/// A class which temporarily stores data for a documentation [`Comment`],
/// which is ultimately built with [`DocCommentBuilder::build`].
pub struct DocCommentBuilder<'a> {
    ast: &'a mut Ast,
    _diagnostic_reporter: &'a mut FastaErrorReporter,
    _uri: &'a str,
    _feature_set: ExperimentalFeatures,
    _language_version: LibraryLanguageVersion,
    start_token: TokenId,
}

impl<'a> DocCommentBuilder<'a> {
    pub fn new(
        ast: &'a mut Ast,
        diagnostic_reporter: &'a mut FastaErrorReporter,
        uri: &'a str,
        feature_set: ExperimentalFeatures,
        language_version: LibraryLanguageVersion,
        start_token: TokenId,
    ) -> Self {
        DocCommentBuilder {
            ast,
            _diagnostic_reporter: diagnostic_reporter,
            _uri: uri,
            _feature_set: feature_set,
            _language_version: language_version,
            start_token,
        }
    }

    /// Dart `build`.
    pub fn build(self) -> Id<Comment> {
        let tokens = &self.ast.tokens;
        let mut list = vec![self.start_token];
        if tokens.lexeme(self.start_token).starts_with("///") {
            let mut token = tokens.next(self.start_token);
            while token.is_some() {
                if tokens.lexeme(token).starts_with("///") {
                    list.push(token);
                }
                token = tokens.next(token);
            }
        }
        let tokens = self.ast.new_token_list(list);
        self.ast.add(Comment {
            references: NodeList::EMPTY,
            tokens,
            code_blocks: Vec::new(),
            doc_imports: Vec::new(),
            doc_directives: Vec::new(),
            has_nodoc: false,
        })
    }
}
