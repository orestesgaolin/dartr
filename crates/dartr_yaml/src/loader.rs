// Ported from package:yaml 3.1.4 lib/src/loader.dart and lib/yaml.dart.
// Copyright (c) 2012, 2014, the Dart project authors.
// Copyright (c) 2006, Kirill Simonov. MIT (see ../LICENSE).

use crate::event::{Event, EventType, TagDirective, VersionDirective};
use crate::parser::Parser;
use crate::scalar::{parse_bool, parse_null, parse_number, parse_plain_scalar};
use crate::{
    CollectionStyle, FileSpan, Scalar, ScalarStyle, YamlException, YamlList, YamlMap, YamlNode,
};
use indexmap::{IndexMap, IndexSet};

#[derive(Clone, Debug, PartialEq)]
pub struct YamlDocument {
    pub contents: YamlNode,
    pub span: FileSpan,
    pub version_directive: Option<VersionDirective>,
    pub tag_directives: Vec<TagDirective>,
    pub start_implicit: bool,
    pub end_implicit: bool,
}

pub struct Loader {
    parser: Parser,
    aliases: IndexMap<String, YamlNode>,
    active_anchors: IndexSet<String>,
    pub span: FileSpan,
    done: bool,
}
impl Loader {
    pub fn new(text: &str, recover: bool) -> Result<Self, YamlException> {
        let mut parser = Parser::new(text, recover);
        let event = parser.parse()?;
        Ok(Self {
            parser,
            aliases: IndexMap::new(),
            active_anchors: IndexSet::new(),
            span: event.span,
            done: false,
        })
    }
    pub fn take_errors(&mut self) -> Vec<YamlException> {
        self.parser.take_errors()
    }
    pub fn take_warnings(&mut self) -> Vec<crate::YamlWarning> {
        self.parser.take_warnings()
    }
    pub fn load(&mut self) -> Result<Option<YamlDocument>, YamlException> {
        if self.done {
            return Ok(None);
        }
        let event = self.parser.parse()?;
        if event.kind == EventType::StreamEnd {
            self.done = true;
            self.span = self.span.expand(event.span);
            return Ok(None);
        }
        let first = self.parser.parse()?;
        let contents = self.load_node(first)?;
        let last = self.parser.parse()?;
        debug_assert_eq!(last.kind, EventType::DocumentEnd);
        let document = YamlDocument {
            contents,
            span: event.span.expand(last.span),
            version_directive: event.version_directive,
            tag_directives: event.tag_directives,
            start_implicit: event.is_implicit,
            end_implicit: last.is_implicit,
        };
        self.span = self.span.expand(document.span);
        self.aliases.clear();
        Ok(Some(document))
    }
    fn register_anchor(&mut self, anchor: &Option<String>, node: &YamlNode) {
        if let Some(anchor) = anchor {
            self.aliases.insert(anchor.clone(), node.clone());
        }
    }
    fn load_node(&mut self, event: Event) -> Result<YamlNode, YamlException> {
        match event.kind {
            EventType::Alias => {
                let Some(alias) = self.aliases.get(&event.value) else {
                    return Err(YamlException {
                        runtime_error: None,
                        message: "Undefined alias.".into(),
                        span: event.span,
                    });
                };
                if self.active_anchors.contains(&event.value) {
                    return Err(YamlException {
                        runtime_error: None,
                        message: "Self-referential collections are not supported.".into(),
                        span: event.span,
                    });
                }
                Ok(alias.clone())
            }
            EventType::Scalar => {
                let value = match event.tag.as_deref() {
                    Some("!" | "tag:yaml.org,2002:str") => Scalar::String(event.value.clone()),
                    Some(tag) => {
                        // Dart codeUnitAt throws before reporting a YAML error for
                        // empty numeric tags, and one-code-unit float tags.
                        if matches!(tag, "tag:yaml.org,2002:int" | "tag:yaml.org,2002:float") {
                            let length = event.value.encode_utf16().count();
                            let range = if length == 0 {
                                Some(
                                    "RangeError (length): Invalid value: Valid value range is empty: 0",
                                )
                            } else if length == 1 && tag == "tag:yaml.org,2002:float" {
                                Some("RangeError (length): Invalid value: Only valid value is 0: 1")
                            } else {
                                None
                            };
                            if let Some(range) = range {
                                return Err(YamlException {
                                    runtime_error: Some(range.into()),
                                    message: range.into(),
                                    span: event.span.start.point_span(),
                                });
                            }
                        }
                        let parsed = match tag {
                            "tag:yaml.org,2002:null" => parse_null(&event.value),
                            "tag:yaml.org,2002:bool" => parse_bool(&event.value),
                            "tag:yaml.org,2002:int" => parse_number(&event.value, true, false),
                            "tag:yaml.org,2002:float" => parse_number(&event.value, false, true),
                            _ => {
                                return Err(YamlException {
                                    runtime_error: None,
                                    message: format!("Undefined tag: {tag}."),
                                    span: event.span,
                                });
                            }
                        };
                        parsed.ok_or_else(|| YamlException {
                            runtime_error: None,
                            message: format!(
                                "Invalid {} scalar.",
                                tag.trim_start_matches("tag:yaml.org,2002:")
                            ),
                            span: event.span,
                        })?
                    }
                    None => parse_plain_scalar(&event.value),
                };
                let node = YamlNode::scalar(value, event.span, event.scalar_style);
                self.register_anchor(&event.anchor, &node);
                Ok(node)
            }
            EventType::SequenceStart | EventType::MappingStart => self.load_collection(event),
            _ => unreachable!("parser emitted non-node event: {:?}", event.kind),
        }
    }
    fn load_collection(&mut self, first: Event) -> Result<YamlNode, YamlException> {
        let is_map = first.kind == EventType::MappingStart;
        let expected_tag = if is_map {
            "tag:yaml.org,2002:map"
        } else {
            "tag:yaml.org,2002:seq"
        };
        if first
            .tag
            .as_deref()
            .is_some_and(|tag| tag != "!" && tag != expected_tag)
        {
            return Err(YamlException {
                runtime_error: None,
                message: format!(
                    "Invalid tag for {}.",
                    if is_map { "mapping" } else { "sequence" }
                ),
                span: first.span,
            });
        }
        let mut node: YamlNode = if is_map {
            YamlMap {
                nodes: Vec::new(),
                span: first.span,
                style: first.collection_style,
            }
            .into()
        } else {
            YamlList {
                nodes: Vec::new(),
                span: first.span,
                style: first.collection_style,
            }
            .into()
        };
        // Register before composing children so self-reference is distinguished from undefined aliases.
        self.register_anchor(&first.anchor, &node);
        if let Some(anchor) = &first.anchor {
            self.active_anchors.insert(anchor.clone());
        }
        let result = (|| {
            let end_kind = if is_map {
                EventType::MappingEnd
            } else {
                EventType::SequenceEnd
            };
            loop {
                let event = self.parser.parse()?;
                if event.kind == end_kind {
                    node.span = first.span.expand(event.span);
                    break;
                }
                let child = self.load_node(event)?;
                match &mut node.kind {
                    crate::NodeKind::Map(entries) => {
                        let event = self.parser.parse()?;
                        let value = self.load_node(event)?;
                        if entries.iter().any(|(key, _)| key.value_equals(&child)) {
                            return Err(YamlException {
                                runtime_error: None,
                                message: "Duplicate mapping key.".into(),
                                span: child.span,
                            });
                        }
                        entries.push((child, value));
                    }
                    crate::NodeKind::List(nodes) => nodes.push(child),
                    _ => unreachable!(),
                }
            }
            Ok(node)
        })();
        if let Some(anchor) = &first.anchor {
            self.active_anchors.shift_remove(anchor);
        }
        if let Ok(node) = &result {
            self.register_anchor(&first.anchor, node);
        }
        result
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoadResult {
    pub node: Option<YamlNode>,
    /// Recovered errors in encounter order, followed by the fatal exception, if any.
    pub errors: Vec<YamlException>,
    pub warnings: Vec<crate::YamlWarning>,
}
pub fn load_yaml_node(text: &str) -> Result<YamlNode, YamlException> {
    Ok(load_yaml_document(text)?.contents)
}
pub fn load_yaml_document(text: &str) -> Result<YamlDocument, YamlException> {
    let mut loader = Loader::new(text, false)?;
    load_single_document(&mut loader)
}
fn load_single_document(loader: &mut Loader) -> Result<YamlDocument, YamlException> {
    let Some(document) = loader.load()? else {
        return Ok(YamlDocument {
            contents: YamlNode::scalar(Scalar::Null, loader.span, ScalarStyle::Any),
            span: loader.span,
            version_directive: None,
            tag_directives: Vec::new(),
            start_implicit: true,
            end_implicit: true,
        });
    };
    if let Some(next) = loader.load()? {
        return Err(YamlException {
            runtime_error: None,
            message: "Only expected one document.".into(),
            span: next.span,
        });
    }
    Ok(document)
}
pub fn load_yaml_node_with_options(text: &str, recover: bool) -> LoadResult {
    let mut loader = match Loader::new(text, recover) {
        Ok(loader) => loader,
        Err(error) => {
            return LoadResult {
                node: None,
                errors: vec![error],
                warnings: Vec::new(),
            };
        }
    };
    let result = load_single_document(&mut loader);
    let mut errors = loader.take_errors();
    let node = match result {
        Ok(document) => Some(document.contents),
        Err(error) => {
            errors.push(error);
            None
        }
    };
    let warnings = loader.take_warnings();
    LoadResult {
        node,
        errors,
        warnings,
    }
}
pub fn load_yaml_documents(text: &str) -> Result<Vec<YamlDocument>, YamlException> {
    let mut loader = Loader::new(text, false)?;
    let mut documents = Vec::new();
    while let Some(document) = loader.load()? {
        documents.push(document);
    }
    Ok(documents)
}
pub fn load_yaml_stream(text: &str) -> Result<YamlNode, YamlException> {
    let mut loader = Loader::new(text, false)?;
    let mut nodes = Vec::new();
    while let Some(document) = loader.load()? {
        nodes.push(document.contents);
    }
    Ok(YamlList {
        nodes,
        span: loader.span,
        style: CollectionStyle::Any,
    }
    .into())
}
