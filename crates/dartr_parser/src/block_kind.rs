// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/block_kind.dart

use dartr_diagnostics::cfe::CfeMessage;
use dartr_diagnostics::cfe_codes as diag;

/// Dart `BlockKind`: the kind of a block, with the message (or template) for
/// a missing block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlockKind {
    CatchClause,
    ClassDeclaration,
    EnumDeclaration,
    ExtensionDeclaration,
    ExtensionTypeDeclaration,
    FinallyClause,
    FunctionBody,
    Invalid,
    MixinDeclaration,
    Statement,
    SwitchExpression,
    SwitchStatement,
    TryStatement,
}

impl BlockKind {
    /// Dart `BlockKind.name`.
    pub fn name(self) -> &'static str {
        match self {
            Self::CatchClause => "catch clause",
            Self::ClassDeclaration => "class declaration",
            Self::EnumDeclaration => "enum declaration",
            Self::ExtensionDeclaration => "extension declaration",
            Self::ExtensionTypeDeclaration => "extension type declaration",
            Self::FinallyClause => "finally clause",
            Self::FunctionBody => "function body",
            Self::Invalid => "invalid",
            Self::MixinDeclaration => "mixin declaration",
            Self::Statement => "statement",
            Self::SwitchExpression => "switch expression",
            Self::SwitchStatement => "switch statement",
            Self::TryStatement => "try statement",
        }
    }

    /// Dart `BlockKind.message`.
    pub fn message(self) -> Option<CfeMessage> {
        Some(match self {
            Self::CatchClause => diag::expected_catch_clause_body(),
            Self::ClassDeclaration => diag::expected_class_body(),
            Self::ExtensionDeclaration => diag::expected_extension_body(),
            Self::ExtensionTypeDeclaration => diag::expected_extension_type_body(),
            Self::FinallyClause => diag::expected_finally_clause_body(),
            Self::MixinDeclaration => diag::expected_mixin_body(),
            Self::SwitchExpression => diag::expected_switch_expression_body(),
            Self::SwitchStatement => diag::expected_switch_statement_body(),
            Self::TryStatement => diag::expected_try_statement_body(),
            _ => return None,
        })
    }

    /// Dart `BlockKind.template` (takes the lexeme of a token).
    pub fn template(self) -> Option<fn(&str) -> CfeMessage> {
        match self {
            Self::EnumDeclaration => Some(diag::expected_enum_body),
            Self::FunctionBody => Some(diag::expected_function_body),
            _ => None,
        }
    }
}
