// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/loop_state.dart

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoopState {
    OutsideLoop,
    /// `break` statement allowed.
    InsideSwitch,
    /// `break` and `continue` statements allowed.
    InsideLoop,
}
