// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/async_modifier.dart

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AsyncModifier {
    Sync,
    SyncStar,
    Async,
    AsyncStar,
}

impl AsyncModifier {
    /// Dart `Enum.name`.
    pub fn name(self) -> &'static str {
        match self {
            AsyncModifier::Sync => "Sync",
            AsyncModifier::SyncStar => "SyncStar",
            AsyncModifier::Async => "Async",
            AsyncModifier::AsyncStar => "AsyncStar",
        }
    }
}
