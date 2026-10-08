//! Port of `package:glob` (`lib/glob.dart`, `lib/src/parser.dart`,
//! `lib/src/ast.dart`) for matching, with a posix path context and
//! case-sensitive matching (the analyzer's settings on macOS and Linux).
//!
//! A glob compiles to the same regular expression as in Dart; look-ahead
//! assertions need `fancy-regex`.

use crate::paths;

/// A compiled glob.
#[derive(Debug)]
pub struct Glob {
    pattern: String,
    ast: Node,
    regex: fancy_regex::Regex,
    can_match_absolute: bool,
    can_match_relative: bool,
}

/// An error in a glob pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlobError {
    pub message: String,
    pub position: usize,
}

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Sequence(Vec<Node>),
    Star,
    DoubleStar,
    AnyChar,
    Range {
        ranges: Vec<(u32, u32)>,
        negated: bool,
    },
    Options(Vec<Node>),
    Literal(String),
}

impl Glob {
    /// Parses [pattern].
    pub fn new(pattern: &str) -> Result<Glob, GlobError> {
        let ast = Parser {
            chars: pattern.chars().collect(),
            position: 0,
        }
        .parse()?;
        let regex_source = format!("^{}$", to_regex(&ast));
        let regex = fancy_regex::Regex::new(&regex_source).map_err(|e| GlobError {
            message: e.to_string(),
            position: 0,
        })?;
        let can_match_absolute = can_match_absolute(&ast);
        let can_match_relative = can_match_relative(&ast);
        Ok(Glob {
            pattern: pattern.to_string(),
            ast,
            regex,
            can_match_absolute,
            can_match_relative,
        })
    }

    /// The source pattern.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Returns `true` if [path] matches this glob (`Glob.matches`).
    pub fn matches(&self, path: &str) -> bool {
        let _ = &self.ast;
        if self.can_match_absolute {
            // The context of the analyzer has an absolute current directory.
            let absolute = paths::absolute_normalized(path);
            if self.regex.is_match(&absolute).unwrap_or(false) {
                return true;
            }
        }
        if self.can_match_relative {
            let relative = if paths::is_absolute(path) {
                let cwd = std::env::current_dir()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| "/".to_string());
                paths::relative(&paths::normalize(path), &paths::normalize(&cwd))
            } else {
                paths::normalize(path)
            };
            if self.regex.is_match(&relative).unwrap_or(false) {
                return true;
            }
        }
        false
    }
}

fn can_match_absolute(node: &Node) -> bool {
    match node {
        Node::Sequence(nodes) => nodes.first().is_some_and(can_match_absolute),
        Node::Options(options) => options.iter().any(can_match_absolute),
        Node::Literal(text) => paths::is_absolute(text),
        _ => false,
    }
}

fn can_match_relative(node: &Node) -> bool {
    match node {
        Node::Sequence(nodes) => nodes.first().is_none_or(can_match_relative),
        Node::Options(options) => options.iter().any(can_match_relative),
        Node::Literal(text) => !paths::is_absolute(text),
        _ => true,
    }
}

fn regex_quote(text: &str) -> String {
    let mut result = String::new();
    for c in text.chars() {
        if "-[]{}()*+?.\\^$|".contains(c) {
            result.push('\\');
        }
        result.push(c);
    }
    result
}

fn to_regex(node: &Node) -> String {
    match node {
        Node::Sequence(nodes) => {
            let mut buffer = String::new();
            let mut i = 0;
            while i < nodes.len() {
                let node = &nodes[i];
                if matches!(node, Node::DoubleStar) && is_dir_boundary(nodes, i) {
                    buffer.push_str(&double_star_regex(true));
                    i += 1;
                    if let Node::Literal(text) = &nodes[i] {
                        let remaining = &text[1..];
                        if !remaining.is_empty() {
                            buffer.push_str(&regex_quote(remaining));
                        }
                    }
                } else {
                    buffer.push_str(&to_regex(node));
                }
                i += 1;
            }
            buffer
        }
        Node::Star => "[^/]*".to_string(),
        Node::DoubleStar => double_star_regex(false),
        Node::AnyChar => "[^/]".to_string(),
        Node::Range { ranges, negated } => {
            let slash = '/' as u32;
            let contains_separator = ranges
                .iter()
                .any(|(min, max)| *min <= slash && slash <= *max);
            let mut buffer = String::new();
            if !negated && contains_separator {
                buffer.push_str("(?!/)");
            }
            buffer.push('[');
            if *negated {
                buffer.push('^');
                if !contains_separator {
                    buffer.push('/');
                }
            }
            for (min, max) in ranges {
                buffer.push_str(&regex_quote(
                    &char::from_u32(*min).unwrap_or('?').to_string(),
                ));
                if min != max {
                    buffer.push('-');
                    buffer.push_str(&regex_quote(
                        &char::from_u32(*max).unwrap_or('?').to_string(),
                    ));
                }
            }
            buffer.push(']');
            buffer
        }
        Node::Options(options) => {
            format!(
                "(?:{})",
                options.iter().map(to_regex).collect::<Vec<_>>().join("|")
            )
        }
        Node::Literal(text) => regex_quote(text),
    }
}

fn double_star_regex(followed_by_slash: bool) -> String {
    let mut buffer = String::from(r"(?!^(?:\.\./|");
    buffer.push('/');
    buffer.push_str(if followed_by_slash {
        r"))(?:[\s\S]*/)?"
    } else {
        r"))[\s\S]*"
    });
    buffer
}

fn is_dir_boundary(nodes: &[Node], i: usize) -> bool {
    if i + 1 >= nodes.len() {
        return false;
    }
    if let Node::Literal(next) = &nodes[i + 1]
        && next.starts_with('/')
    {
        if i == 0 {
            return true;
        }
        if let Node::Literal(previous) = &nodes[i - 1]
            && previous.ends_with('/')
        {
            return true;
        }
    }
    false
}

struct Parser {
    chars: Vec<char>,
    position: usize,
}

impl Parser {
    fn error<T>(&self, message: &str) -> Result<T, GlobError> {
        Err(GlobError {
            message: message.to_string(),
            position: self.position,
        })
    }

    fn is_done(&self) -> bool {
        self.position >= self.chars.len()
    }

    fn peek_is(&self, c: char) -> bool {
        self.chars.get(self.position) == Some(&c)
    }

    fn scan(&mut self, c: char) -> bool {
        if self.peek_is(c) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn read_char(&mut self) -> Result<char, GlobError> {
        match self.chars.get(self.position) {
            Some(c) => {
                self.position += 1;
                Ok(*c)
            }
            None => self.error("expected more input."),
        }
    }

    fn parse(mut self) -> Result<Node, GlobError> {
        self.parse_sequence(false)
    }

    fn parse_sequence(&mut self, in_options: bool) -> Result<Node, GlobError> {
        let mut nodes = Vec::new();
        if self.is_done() {
            return self.error("expected a glob.");
        }
        while !self.is_done() {
            if in_options && (self.peek_is(',') || self.peek_is('}')) {
                break;
            }
            nodes.push(self.parse_node(in_options)?);
        }
        Ok(Node::Sequence(nodes))
    }

    fn parse_node(&mut self, in_options: bool) -> Result<Node, GlobError> {
        if self.scan('*') {
            return Ok(if self.scan('*') {
                Node::DoubleStar
            } else {
                Node::Star
            });
        }
        if self.scan('?') {
            return Ok(Node::AnyChar);
        }
        if self.scan('[') {
            return self.parse_range();
        }
        if self.scan('{') {
            return self.parse_options();
        }
        self.parse_literal(in_options)
    }

    fn parse_range(&mut self) -> Result<Node, GlobError> {
        if self.peek_is(']') {
            return self.error("unexpected \"]\".");
        }
        let negated = self.scan('!') || self.scan('^');
        let mut ranges = Vec::new();
        let read_range_char = |parser: &mut Parser| -> Result<u32, GlobError> {
            let c = parser.read_char()?;
            if negated || c != '/' {
                Ok(c as u32)
            } else {
                parser.position -= 1;
                parser.error("\"/\" may not be used in a range.")
            }
        };
        while !self.scan(']') {
            if self.is_done() {
                return self.error("expected \"]\".");
            }
            self.scan('\\');
            let start = read_range_char(self)?;
            if self.scan('-') {
                if self.peek_is(']') {
                    ranges.push((start, start));
                    ranges.push(('-' as u32, '-' as u32));
                    continue;
                }
                self.scan('\\');
                let end = read_range_char(self)?;
                if end < start {
                    return self.error("Range out of order.");
                }
                ranges.push((start, end));
            } else {
                ranges.push((start, start));
            }
        }
        // Dart keeps the ranges in a set.
        let mut unique: Vec<(u32, u32)> = Vec::new();
        for range in ranges {
            if !unique.contains(&range) {
                unique.push(range);
            }
        }
        Ok(Node::Range {
            ranges: unique,
            negated,
        })
    }

    fn parse_options(&mut self) -> Result<Node, GlobError> {
        if self.peek_is('}') {
            return self.error("unexpected \"}\".");
        }
        let mut options = Vec::new();
        loop {
            options.push(self.parse_sequence(true)?);
            if !self.scan(',') {
                break;
            }
        }
        if options.len() == 1 && !self.scan(',') {
            return self.error("expected \",\".");
        }
        if !self.scan('}') {
            return self.error("expected \"}\".");
        }
        Ok(Node::Options(options))
    }

    fn parse_literal(&mut self, in_options: bool) -> Result<Node, GlobError> {
        let stop = |c: char| {
            matches!(c, '*' | '{' | '[' | '?' | '\\' | '}' | ']' | '(' | ')')
                || (in_options && c == ',')
        };
        let mut buffer = String::new();
        let scan_run = |parser: &mut Parser, buffer: &mut String| {
            while let Some(&c) = parser.chars.get(parser.position) {
                if stop(c) {
                    break;
                }
                buffer.push(c);
                parser.position += 1;
            }
        };
        scan_run(self, &mut buffer);
        while self.scan('\\') {
            buffer.push(self.read_char()?);
            scan_run(self, &mut buffer);
        }
        for c in [']', '(', ')'] {
            if self.peek_is(c) {
                return self.error(&format!("unexpected \"{c}\""));
            }
        }
        if !in_options && self.peek_is('}') {
            return self.error("unexpected \"}\"");
        }
        Ok(Node::Literal(buffer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_like_package_glob() {
        let glob = Glob::new("**/*.g.dart").unwrap();
        assert!(glob.matches("lib/a.g.dart"));
        assert!(glob.matches("a.g.dart"));
        assert!(!glob.matches("lib/a.dart"));

        let glob = Glob::new("build/**").unwrap();
        assert!(glob.matches("build/a/b"));
        assert!(!glob.matches("build"));

        let glob = Glob::new("lib/{a,b}/*.dart").unwrap();
        assert!(glob.matches("lib/b/x.dart"));
        assert!(!glob.matches("lib/c/x.dart"));
        assert!(!glob.matches("lib/a/y/x.dart"));

        let glob = Glob::new("**.dart").unwrap();
        assert!(glob.matches("a/b.dart"));
        assert!(!glob.matches("../b.dart"));

        assert!(Glob::new("{a}").is_err());
        assert!(Glob::new("a]").is_err());
    }
}
