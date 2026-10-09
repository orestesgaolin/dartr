// Dart source: pkg/analyzer/lib/dart/ast/doc_comment.dart

//! The data of a documentation comment besides the comment references:
//! Markdown code blocks, doc directives (`{@template ...}`) and doc imports
//! (`@docImport`). The `DocCommentBuilder` of the AST builder fills them in
//! ([`crate::Comment::code_blocks`], [`crate::Comment::doc_directives`],
//! [`crate::Comment::doc_imports`]); they are not child entities.

use crate::arena::{Ast, Id};
use crate::generated::nodes::ImportDirective;

/// Dart `DocDirective`: a [`SimpleDocDirective`] or a [`BlockDocDirective`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocDirective {
    Simple(SimpleDocDirective),
    Block(BlockDocDirective),
}

impl DocDirective {
    /// Dart `DocDirective.type`.
    pub fn ty(&self) -> DocDirectiveType {
        match self {
            DocDirective::Simple(d) => d.tag.ty,
            DocDirective::Block(d) => d.opening_tag.ty,
        }
    }
}

/// Dart `SimpleDocDirective`: a doc directive with one tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimpleDocDirective {
    pub tag: DocDirectiveTag,
}

/// Dart `BlockDocDirective`: a doc directive with an opening tag and a
/// closing tag (`None` when the closing tag is missing).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockDocDirective {
    pub opening_tag: DocDirectiveTag,
    pub closing_tag: Option<DocDirectiveTag>,
}

/// Dart `CodeBlockType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CodeBlockType {
    /// Fenced with three or more backticks.
    Fenced,
    /// Indented by four or more spaces.
    Indented,
}

/// Dart `DocDirectiveArgument`: a positional or named argument of a doc
/// directive tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocDirectiveArgument {
    Positional(DocDirectivePositionalArgument),
    Named(DocDirectiveNamedArgument),
}

impl DocDirectiveArgument {
    pub fn offset(&self) -> u32 {
        match self {
            DocDirectiveArgument::Positional(a) => a.offset,
            DocDirectiveArgument::Named(a) => a.offset,
        }
    }

    pub fn end(&self) -> u32 {
        match self {
            DocDirectiveArgument::Positional(a) => a.end,
            DocDirectiveArgument::Named(a) => a.end,
        }
    }

    pub fn value(&self) -> &str {
        match self {
            DocDirectiveArgument::Positional(a) => &a.value,
            DocDirectiveArgument::Named(a) => &a.value,
        }
    }
}

/// Dart `DocDirectivePositionalArgument`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocDirectivePositionalArgument {
    pub offset: u32,
    pub end: u32,
    pub value: Box<str>,
}

/// Dart `DocDirectiveNamedArgument`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocDirectiveNamedArgument {
    pub offset: u32,
    pub end: u32,
    pub name: Box<str>,
    pub value: Box<str>,
}

/// Dart `DocDirectiveParameterFormat`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DocDirectiveParameterFormat {
    Any,
    Integer,
    Uri,
    YoutubeUrl,
}

impl DocDirectiveParameterFormat {
    /// Dart `youtubeUrlPrefix`.
    pub const YOUTUBE_URL_PREFIX: &'static str = "https://www.youtube.com/watch?v=";

    /// Dart `displayString`.
    pub fn display_string(self) -> &'static str {
        match self {
            DocDirectiveParameterFormat::Any => "any",
            DocDirectiveParameterFormat::Integer => "an integer",
            DocDirectiveParameterFormat::Uri => "a URI",
            DocDirectiveParameterFormat::YoutubeUrl => {
                "a YouTube URL, starting with 'https://www.youtube.com/watch?v='"
            }
        }
    }
}

/// Dart `DocDirectiveParameter`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocDirectiveParameter {
    pub name: &'static str,
    pub expected_format: DocDirectiveParameterFormat,
}

const fn param(
    name: &'static str,
    expected_format: DocDirectiveParameterFormat,
) -> DocDirectiveParameter {
    DocDirectiveParameter {
        name,
        expected_format,
    }
}

/// Dart `DocDirectiveTag`: an opening tag, a closing tag or the only tag of
/// a doc directive, with its arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocDirectiveTag {
    pub offset: u32,
    pub end: u32,
    pub name_offset: u32,
    pub name_end: u32,
    pub ty: DocDirectiveType,
    /// Dart `List<DocDirectiveArgument>`: positional arguments only.
    pub positional_arguments: Vec<DocDirectiveArgument>,
    pub named_arguments: Vec<DocDirectiveNamedArgument>,
}

/// Dart `DocDirectiveType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DocDirectiveType {
    Animation,
    CanonicalFor,
    Category,
    EndInjectHtml,
    EndTool,
    EndTemplate,
    InjectHtml,
    Macro,
    SubCategory,
    Template,
    Tool,
    Example,
    Youtube,
}

impl DocDirectiveType {
    /// Dart `DocDirectiveType.values`.
    pub const VALUES: &'static [DocDirectiveType] = &[
        DocDirectiveType::Animation,
        DocDirectiveType::CanonicalFor,
        DocDirectiveType::Category,
        DocDirectiveType::EndInjectHtml,
        DocDirectiveType::EndTool,
        DocDirectiveType::EndTemplate,
        DocDirectiveType::InjectHtml,
        DocDirectiveType::Macro,
        DocDirectiveType::SubCategory,
        DocDirectiveType::Template,
        DocDirectiveType::Tool,
        DocDirectiveType::Example,
        DocDirectiveType::Youtube,
    ];

    /// Dart `name`: the name in the tag (`{@<name> ...}`).
    pub fn name(self) -> &'static str {
        match self {
            DocDirectiveType::Animation => "animation",
            DocDirectiveType::CanonicalFor => "canonicalFor",
            DocDirectiveType::Category => "category",
            DocDirectiveType::EndInjectHtml => "end-inject-html",
            DocDirectiveType::EndTool => "end-tool",
            DocDirectiveType::EndTemplate => "endtemplate",
            DocDirectiveType::InjectHtml => "inject-html",
            DocDirectiveType::Macro => "macro",
            DocDirectiveType::SubCategory => "subCategory",
            DocDirectiveType::Template => "template",
            DocDirectiveType::Tool => "tool",
            DocDirectiveType::Example => "example",
            DocDirectiveType::Youtube => "youtube",
        }
    }

    /// Dart `isBlock`: whether this is the opening tag of a block directive.
    pub fn is_block(self) -> bool {
        matches!(
            self,
            DocDirectiveType::InjectHtml | DocDirectiveType::Template | DocDirectiveType::Tool
        )
    }

    /// Dart `opposingName`: the name of the closing tag of a block
    /// directive, or of the opening tag of a closing tag.
    pub fn opposing_name(self) -> Option<&'static str> {
        match self {
            DocDirectiveType::EndInjectHtml => Some("inject-html"),
            DocDirectiveType::EndTool => Some("tool"),
            DocDirectiveType::EndTemplate => Some("template"),
            DocDirectiveType::InjectHtml => Some("end-inject-html"),
            DocDirectiveType::Template => Some("endtemplate"),
            DocDirectiveType::Tool => Some("end-tool"),
            _ => None,
        }
    }

    /// Dart `positionalParameters`.
    pub fn positional_parameters(self) -> &'static [DocDirectiveParameter] {
        use DocDirectiveParameterFormat::*;
        const ANIMATION: &[DocDirectiveParameter] = &[
            param("width", Integer),
            param("height", Integer),
            param("url", Uri),
        ];
        const ELEMENT: &[DocDirectiveParameter] = &[param("element", Any)];
        const NAME: &[DocDirectiveParameter] = &[param("name", Any)];
        const FILE: &[DocDirectiveParameter] = &[param("file", Any)];
        const YOUTUBE: &[DocDirectiveParameter] = &[
            param("width", Integer),
            param("height", Integer),
            param("url", YoutubeUrl),
        ];
        match self {
            DocDirectiveType::Animation => ANIMATION,
            DocDirectiveType::CanonicalFor => ELEMENT,
            DocDirectiveType::Macro | DocDirectiveType::Template | DocDirectiveType::Tool => NAME,
            DocDirectiveType::Example => FILE,
            DocDirectiveType::Youtube => YOUTUBE,
            _ => &[],
        }
    }

    /// Dart `namedParameters`.
    pub fn named_parameters(self) -> &'static [DocDirectiveParameter] {
        const ID: &[DocDirectiveParameter] = &[param("id", DocDirectiveParameterFormat::Any)];
        match self {
            DocDirectiveType::Animation => ID,
            _ => &[],
        }
    }

    /// Dart `restParametersAllowed`.
    pub fn rest_parameters_allowed(self) -> bool {
        matches!(
            self,
            DocDirectiveType::Category
                | DocDirectiveType::SubCategory
                | DocDirectiveType::Tool
                | DocDirectiveType::Example
        )
    }
}

/// Dart `DocImport`: an `@docImport` in a doc comment. The import directive
/// is parsed from a separate token stream (`import <text after
/// @docImport>`), with offsets in the compilation unit, so it is in its own
/// [`Ast`].
#[derive(Clone, Debug)]
pub struct DocImport {
    /// The offset of the start of the `@docImport` line content.
    pub offset: u32,
    /// The AST of the synthetic `import ...` unit.
    pub ast: Box<Ast>,
    /// Dart `import`.
    pub import: Id<ImportDirective>,
}

/// Dart `MdCodeBlock`: a fenced or indented Markdown code block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MdCodeBlock {
    /// Dart `infoString`: the text after the opening backticks.
    pub info_string: Option<Box<str>>,
    pub lines: Vec<MdCodeBlockLine>,
    pub ty: CodeBlockType,
}

/// Dart `MdCodeBlockLine`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MdCodeBlockLine {
    pub offset: u32,
    pub length: u32,
}
