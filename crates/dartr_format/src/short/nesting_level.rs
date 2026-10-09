// Dart source: dart_style lib/src/short/nesting_level.dart

use super::arena::{Arena, NestingId};

/// A single level of expression nesting.
///
/// When a line is split in the middle of an expression, this tracks the
/// context of where in the expression that split occurs. It ensures that the
/// [LineSplitter] obeys the expression nesting when deciding what column to
/// start lines at when split inside an expression.
///
/// Each instance of this represents a single level of expression nesting. If we
/// split at to chunks with different levels of nesting, the splitter ensures
/// they each get assigned to different columns.
///
/// In addition, each level has an indent. This is the number of spaces it is
/// indented relative to the outer expression. It's almost always
/// [Indent.expression], but cascades are special magic snowflakes and use
/// [Indent.cascade].
///
/// NestingLevels can be marked during processing in an algorithm but should be
/// left unmarked when the algorithm finishes to make marking work in subsequent
/// calls.
pub struct NestingLevel {
    /// The nesting level surrounding this one, or `None` if this is represents
    /// top level code in a block.
    pub parent: Option<NestingId>,

    /// The number of characters that this nesting level is indented relative to
    /// the containing level.
    ///
    /// Normally, this is [Indent.expression], but cascades use [Indent.cascade].
    pub indent: i32,

    /// The total number of characters of indentation from this level and all of
    /// its parents, after determining which nesting levels are actually used.
    ///
    /// This is only valid during line splitting.
    total_used_indent: Option<i32>,

    pub is_marked: bool,
}

impl NestingLevel {
    pub fn is_nested(&self) -> bool {
        self.parent.is_some()
    }

    /// Dart `totalUsedIndent`.
    #[inline]
    pub fn total_used_indent(&self) -> i32 {
        self.total_used_indent.unwrap()
    }
}

impl Arena {
    /// Dart `NestingLevel()`: a new top level nesting.
    pub fn new_nesting_level(&mut self) -> NestingId {
        let id = NestingId(self.nestings.len() as u32);
        self.nestings.push(NestingLevel {
            parent: None,
            indent: 0,
            total_used_indent: None,
            is_marked: false,
        });
        id
    }

    /// Dart `NestingLevel.nest`: creates a new deeper level of nesting
    /// indented [spaces] more characters that the outer level.
    pub fn nest(&mut self, parent: NestingId, spaces: i32) -> NestingId {
        let id = NestingId(self.nestings.len() as u32);
        self.nestings.push(NestingLevel {
            parent: Some(parent),
            indent: spaces,
            total_used_indent: None,
            is_marked: false,
        });
        id
    }

    /// Dart `Markable.mark`.
    #[inline]
    pub fn mark_nesting(&mut self, id: NestingId) -> bool {
        let level = &mut self.nestings[id.index()];
        if level.is_marked {
            return false;
        }
        level.is_marked = true;
        true
    }

    /// Dart `Markable.unmark`.
    #[inline]
    pub fn unmark_nesting(&mut self, id: NestingId) {
        self.nestings[id.index()].is_marked = false;
    }

    /// Clears the previously calculated total indent of this nesting level.
    pub fn clear_total_used_indent(&mut self, id: NestingId) {
        let mut current = Some(id);
        while let Some(id) = current {
            let level = &mut self.nestings[id.index()];
            level.total_used_indent = None;
            current = level.parent;
        }
    }

    /// Calculates the total amount of indentation from this nesting level and
    /// all of its parents assuming only marked levels are in use.
    pub fn refresh_total_used_indent(&mut self, id: NestingId) {
        let level = &self.nestings[id.index()];
        if level.total_used_indent.is_some() {
            return;
        }

        let mut total_indent = 0;

        if let Some(parent) = level.parent {
            self.refresh_total_used_indent(parent);
            total_indent += self.nestings[parent.index()].total_used_indent();
        }

        let level = &mut self.nestings[id.index()];
        if level.is_marked {
            total_indent += level.indent;
        }

        level.total_used_indent = Some(total_indent);
    }
}
