// Dart source: dart_style lib/src/back_end/solution_cache.dart

use std::rc::Rc;

use rustc_hash::FxHashMap;

use crate::front_end::formatting_style::FormattingStyle;
use crate::piece::{PieceId, Pieces, State};

use super::code::GroupCode;
use super::solver::Solver;

/// Maintains a cache of [Piece] subtrees that have been previously solved.
///
/// If a given [Piece] has newlines before and after it, then (in most
/// cases, assuming there are no other constraints) the way it is formatted
/// only depends on its leading indentation. In that case, we can format that
/// piece using a separate Solver and insert the results in any Solution that
/// has that piece at that leading indentation.
///
/// This cache stores those previously formatted subtree pieces so that
/// [CodeWriter] can reuse them across [Solution]s.
///
/// Note that this cache is shared across all Solvers and Solutions for an
/// entire format operation. Different Solvers and Solutions may end up
/// reaching the same child Piece and wanting to format it separately with
/// the same indentation. When that happens, sharing this cache allows us to
/// reuse that cached subtree Solution.
pub struct SolutionCache<'p> {
    pub style: FormattingStyle,

    cache: FxHashMap<Key, Rc<CachedSolution<'p>>>,
}

/// The parts of a cached subtree [Solution] that the surrounding solution
/// uses: its overflow, cost, and formatted code.
pub struct CachedSolution<'p> {
    pub overflow: i32,
    pub cost: i32,
    pub code: Rc<GroupCode<'p>>,
}

/// The key used to uniquely identify a previously formatted Piece.
///
/// Each subtree solution depends only on the Piece and the amount of leading
/// indentation in the context where it appears (which may vary based on how
/// surrounding pieces end up splitting).
///
/// In particular, note that if surrounding pieces split in *different* ways
/// that still end up producing the same overall leading indentation, we are
/// able to reuse a previously cached Solution for some Piece.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    piece: PieceId,
    indent: i32,
    subsequent_indent: i32,
}

impl<'p> SolutionCache<'p> {
    pub fn new(style: FormattingStyle) -> SolutionCache<'p> {
        SolutionCache {
            style,
            cache: FxHashMap::default(),
        }
    }

    /// Returns a previously cached solution for formatting [root] with
    /// leading [indent] (and [subsequent_indent] for lines after the first)
    /// or produces a new solution, caches it, and returns it.
    ///
    /// If [root] is already bound to a state in the surrounding piece tree's
    /// [Solution], then [state_if_bound] is that state. Otherwise, it is
    /// treated as unbound and the cache will find a state for [root] as well
    /// as its children.
    pub fn find(
        &mut self,
        pieces: &'p Pieces,
        root: PieceId,
        state_if_bound: Option<State>,
        page_width: i32,
        indent: i32,
        subsequent_indent: i32,
    ) -> Rc<CachedSolution<'p>> {
        let key = Key {
            piece: root,
            indent,
            subsequent_indent,
        };

        // See if we've already formatted this piece at this indentation. If
        // not, format it and store the result.
        if let Some(solution) = self.cache.get(&key) {
            return Rc::clone(solution);
        }

        let solution = Solver::new(page_width, indent, subsequent_indent).format(
            self,
            pieces,
            root,
            state_if_bound,
        );
        let cached = Rc::new(CachedSolution {
            overflow: solution.overflow(),
            cost: solution.cost(),
            code: Rc::clone(solution.code()),
        });
        // Dart `putIfAbsent`: if solving the subtree already stored this key,
        // the new value replaces it.
        self.cache.insert(key, Rc::clone(&cached));
        cached
    }
}
