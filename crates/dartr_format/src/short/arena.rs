// Dart source: none (Rust arena for the object graph of lib/src/short).
// Dart source: dart_style lib/src/short/marking_scheme.dart (Markable)

//! The short style builds a graph of mutable objects that refer to each
//! other: chunks refer to rules, nesting levels and spans, rules refer to the
//! chunks they split and constrain other rules, nesting levels refer to their
//! parents. Dart compares these objects by identity and mutates them during
//! line splitting (rule indexes, marks, cached indentation).
//!
//! The port keeps all of these objects in one [Arena] per format run and
//! refers to them with small ids. Identity comparisons become id
//! comparisons. Dart `Markable` (`marking_scheme.dart`) is the `is_marked`
//! field of [Span] and [NestingLevel].

use super::chunk::{Chunk, Span};
use super::nesting_level::NestingLevel;
use super::rule::Rule;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
        pub struct $name(pub u32);

        impl $name {
            #[inline(always)]
            pub fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

id_type!(RuleId);
id_type!(ChunkId);
id_type!(SpanId);
id_type!(NestingId);

/// All rules, chunks, spans and nesting levels of one format run.
pub struct Arena {
    pub rules: Vec<Rule>,
    pub chunks: Vec<Chunk>,
    pub spans: Vec<Span>,
    pub nestings: Vec<NestingLevel>,
}

impl Arena {
    /// Dart `Rule.dummy`: the rule used for dummy chunks.
    pub const DUMMY_RULE: RuleId = RuleId(0);

    pub fn new() -> Arena {
        let mut arena = Arena {
            rules: Vec::with_capacity(256),
            chunks: Vec::with_capacity(1024),
            spans: Vec::with_capacity(256),
            nestings: Vec::with_capacity(256),
        };
        arena.add_rule(Rule::hard());
        arena
    }

    #[inline]
    pub fn add_rule(&mut self, rule: Rule) -> RuleId {
        let id = RuleId(self.rules.len() as u32);
        self.rules.push(rule);
        id
    }

    #[inline]
    pub fn add_chunk(&mut self, chunk: Chunk) -> ChunkId {
        let id = ChunkId(self.chunks.len() as u32);
        self.chunks.push(chunk);
        id
    }

    #[inline]
    pub fn add_span(&mut self, cost: i32) -> SpanId {
        let id = SpanId(self.spans.len() as u32);
        self.spans.push(Span {
            cost,
            is_marked: false,
        });
        id
    }

    #[inline]
    pub fn rule(&self, id: RuleId) -> &Rule {
        &self.rules[id.index()]
    }

    #[inline]
    pub fn rule_mut(&mut self, id: RuleId) -> &mut Rule {
        &mut self.rules[id.index()]
    }

    #[inline]
    pub fn chunk(&self, id: ChunkId) -> &Chunk {
        &self.chunks[id.index()]
    }

    #[inline]
    pub fn chunk_mut(&mut self, id: ChunkId) -> &mut Chunk {
        &mut self.chunks[id.index()]
    }

    #[inline]
    pub fn nesting(&self, id: NestingId) -> &NestingLevel {
        &self.nestings[id.index()]
    }
}

impl Default for Arena {
    fn default() -> Self {
        Arena::new()
    }
}
