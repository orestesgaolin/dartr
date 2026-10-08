//! Checks properties of the token stream that the parser port relies on and
//! that the oracle JSON does not show: links, `end_group`, byte ranges.
//! Runs on the token fixtures and, when present, on the SDK corpora.

use std::path::{Path, PathBuf};

use dartr_syntax::{TokenId, TokenType, Tokens, scan_for_analyzer, scan_string, strip_bom};

fn dart_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            dart_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "dart") {
            out.push(path);
        }
    }
}

fn inputs() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    dart_files(&root.join("crates/dartr/tests/fixtures/tokens"), &mut files);
    assert!(files.len() >= 40, "fixtures not found");
    // The corpora are not checked in (tools/fetch_sdk.sh); use them when
    // they are there.
    dart_files(
        &root.join("third_party/dart-sdk/tests/language"),
        &mut files,
    );
    dart_files(&root.join("third_party/dart-sdk/sdk/lib"), &mut files);
    // More directories, separated by `:` (for example generated inputs).
    if let Ok(extra) = std::env::var("DARTR_EXTRA_CORPUS") {
        for dir in extra.split(':').filter(|d| !d.is_empty()) {
            dart_files(Path::new(dir), &mut files);
        }
    }
    files
}

fn utf16_len(s: &str) -> u32 {
    s.chars().map(|c| c.len_utf16() as u32).sum()
}

fn closer_for(begin: TokenType) -> Option<TokenType> {
    match begin {
        TokenType::OPEN_PAREN => Some(TokenType::CLOSE_PAREN),
        TokenType::OPEN_SQUARE_BRACKET => Some(TokenType::CLOSE_SQUARE_BRACKET),
        TokenType::OPEN_CURLY_BRACKET | TokenType::STRING_INTERPOLATION_EXPRESSION => {
            Some(TokenType::CLOSE_CURLY_BRACKET)
        }
        _ => None,
    }
}

fn check_stream(path: &Path, source: &str, tokens: &Tokens, first: TokenId) {
    // UTF-16 offset of each byte offset (at character boundaries).
    let mut utf16_at = vec![0u32; source.len() + 1];
    let mut n = 0;
    for (i, c) in source.char_indices() {
        utf16_at[i] = n;
        n += c.len_utf16() as u32;
    }
    utf16_at[source.len()] = n;
    let ctx = |id: TokenId| {
        format!(
            "{}: token {:?} at {}",
            path.display(),
            tokens.ty(id),
            tokens.offset(id)
        )
    };
    // Position of each token in the stream, to check `end_group` order.
    let mut position = vec![usize::MAX; tokens.len()];
    let mut previous = TokenId::NONE;
    for (count, id) in tokens.iter_from(first).enumerate() {
        assert!(
            count <= tokens.len(),
            "{}: the stream does not end",
            path.display()
        );
        if previous.is_some() {
            assert_eq!(tokens.previous(id), previous, "{}", ctx(id));
        }
        position[id.index()] = count;
        previous = id;
    }
    let eof = previous;
    assert_eq!(tokens.ty(eof), TokenType::EOF);
    assert_eq!(tokens.next(eof), eof, "EOF.next is EOF");
    assert_eq!(
        tokens.offset(eof),
        utf16_len(source),
        "{}: EOF offset",
        path.display()
    );

    let mut last_end = 0;
    for id in tokens.iter_from(first) {
        let t = tokens.get(id);
        assert!(!t.is_error(), "{}", ctx(id));
        // The lexeme of a token that is not synthetic is its source text, and
        // the UTF-16 offset matches the byte offset.
        if !t.is_synthetic() {
            let range = tokens.byte_range(id);
            assert_eq!(&source[range.clone()], tokens.lexeme(id), "{}", ctx(id));
            if t.ty == TokenType::STRING {
                // Dart uses `tokenStart` as the offset and `start` for the
                // lexeme; after a `${` that curly bracket recovery discards,
                // `tokenStart` is still at the `${`.
                assert!(utf16_at[range.start] >= t.offset, "{}", ctx(id));
            } else {
                assert_eq!(utf16_at[range.start], t.offset, "{}", ctx(id));
            }
            assert_eq!(utf16_len(tokens.lexeme(id)), t.length, "{}", ctx(id));
            assert!(
                utf16_at[range.start] >= last_end,
                "{}: tokens overlap",
                ctx(id)
            );
            last_end = utf16_at[range.end];
        }
        for c in tokens.comments(id) {
            let range = tokens.byte_range(c);
            assert_eq!(&source[range.clone()], tokens.lexeme(c), "{}", ctx(c));
            assert_eq!(utf16_at[range.start], tokens.offset(c), "{}", ctx(c));
            assert!(
                tokens.offset(c) <= t.offset,
                "{}: comment after its token",
                ctx(c)
            );
        }
        // Every `(`, `[`, `{` and `${` has an end group of the matching type
        // later in the stream (a synthetic one if the source has none).
        if let Some(closer) = closer_for(t.ty) {
            let end = t.end_group;
            assert!(end.is_some(), "{}: no end group", ctx(id));
            assert_eq!(tokens.ty(end), closer, "{}", ctx(id));
            assert!(
                position[end.index()] > position[id.index()],
                "{}: end group before begin",
                ctx(id)
            );
        }
        if t.ty == TokenType::LT && t.end_group.is_some() {
            let end = tokens.ty(t.end_group);
            assert!(
                matches!(end, TokenType::GT | TokenType::GT_GT | TokenType::GT_GT_GT),
                "{}",
                ctx(id)
            );
        }
    }
}

#[test]
fn token_stream_properties() {
    let files = inputs();
    for path in &files {
        let text = std::fs::read_to_string(path).unwrap();
        let source = strip_bom(&text);
        let result = scan_for_analyzer(source);
        check_stream(path, source, result.tokens(), result.first);

        // The front end entry point: the error tokens are the start of the
        // stream.
        let scan = scan_string(source, None, true, None);
        let mut id = scan.first;
        let mut errors = 0;
        while scan.tokens.get(id).is_error() {
            errors += 1;
            id = scan.tokens.next(id);
        }
        assert_eq!(scan.has_errors, errors > 0, "{}", path.display());
    }
}

/// The parser port splits `>>` into `>` `>` and inserts synthetic tokens;
/// check that the arena supports that without moving other tokens.
#[test]
fn insert_tokens_into_the_stream() {
    let result = scan_for_analyzer("List<List<int>> x;");
    let mut tokens = result.scan.tokens.clone();
    let gt_gt = tokens
        .iter_from(result.first)
        .find(|&t| tokens.ty(t) == TokenType::GT_GT)
        .unwrap();
    let (offset, byte) = (tokens.offset(gt_gt), tokens.byte_range(gt_gt).start as u32);
    let before = tokens.previous(gt_gt);
    let after = tokens.next(gt_gt);
    let first_gt = tokens.push(dartr_syntax::Token::fixed(
        TokenType::GT,
        offset,
        byte,
        false,
    ));
    let second_gt = tokens.push(dartr_syntax::Token::fixed(
        TokenType::GT,
        offset + 1,
        byte + 1,
        false,
    ));
    let semicolon = tokens.push(dartr_syntax::Token::fixed(
        TokenType::SEMICOLON,
        offset + 2,
        byte + 2,
        true,
    ));
    tokens.set_next(before, first_gt);
    tokens.set_next(first_gt, second_gt);
    tokens.set_next(second_gt, semicolon);
    tokens.set_next(semicolon, after);

    let stream: Vec<(String, u32, bool)> = tokens
        .iter_from(result.first)
        .map(|t| {
            (
                tokens.lexeme(t).to_string(),
                tokens.offset(t),
                tokens.get(t).is_synthetic(),
            )
        })
        .collect();
    let expected: Vec<(String, u32, bool)> = [
        ("List", 0, false),
        ("<", 4, false),
        ("List", 5, false),
        ("<", 9, false),
        ("int", 10, false),
        (">", 13, false),
        (">", 14, false),
        (";", 15, true),
        ("x", 16, false),
        (";", 17, false),
        ("", 18, true),
    ]
    .iter()
    .map(|(l, o, s)| (l.to_string(), *o, *s))
    .collect();
    assert_eq!(stream, expected);
    assert_eq!(tokens.get(semicolon).before_synthetic, second_gt);
    assert_eq!(tokens.previous(after), semicolon);
}
