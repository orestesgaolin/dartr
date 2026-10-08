// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/identifier_context.dart

//! STUB: the API that the parser uses. The port replaces the `todo!()`s and
//! keeps these names and signatures.

#![allow(unused_variables)]

use dartr_diagnostics::cfe::CfeMessage;
use dartr_syntax::{TokenId, Tokens};

use crate::listener::Listener;
use crate::parser_impl::Parser;

/// Dart `IdentifierContext`: the context in which an identifier occurs.
/// One variant per Dart constant (`IdentifierContext.typeReference` is
/// `IdentifierContext::TypeReference`). Dart `operatorName` is the same
/// constant as `methodDeclarationContinuation`, see
/// [`IdentifierContext::OPERATOR_NAME`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IdentifierContext {
    ImportPrefixDeclaration,
    DottedName,
    DottedNameContinuation,
    Combinator,
    MetadataReference,
    MetadataContinuation,
    MetadataContinuationAfterTypeArguments,
    TypedefDeclaration,
    FieldInitializer,
    FormalParameterDeclaration,
    RecordFieldDeclaration,
    CatchParameter,
    LibraryName,
    LibraryNameContinuation,
    PartName,
    PartNameContinuation,
    EnumDeclaration,
    EnumValueDeclaration,
    ClassOrMixinOrExtensionDeclaration,
    TypeVariableDeclaration,
    PrefixedTypeReference,
    TypeReference,
    TypeReferenceContinuation,
    TopLevelVariableDeclaration,
    FieldDeclaration,
    TopLevelFunctionDeclaration,
    MethodDeclaration,
    MethodDeclarationContinuation,
    LocalFunctionDeclaration,
    LocalFunctionDeclarationContinuation,
    ConstructorReference,
    ConstructorReferenceContinuation,
    PrimaryConstructorDeclaration,
    LabelDeclaration,
    LiteralSymbol,
    LiteralSymbolContinuation,
    Expression,
    ExpressionContinuation,
    NamedArgumentReference,
    NamedRecordFieldReference,
    LocalVariableDeclaration,
    LabelReference,
}

impl IdentifierContext {
    /// Dart `IdentifierContext.operatorName` (identical to
    /// `methodDeclarationContinuation` in Dart: same const constructor call).
    pub const OPERATOR_NAME: IdentifierContext = IdentifierContext::MethodDeclarationContinuation;

    /// Dart `toString()` (the `_name` field).
    pub fn name(self) -> &'static str {
        match self {
            Self::ImportPrefixDeclaration => "importPrefixDeclaration",
            Self::DottedName => "dottedName",
            Self::DottedNameContinuation => "dottedNameContinuation",
            Self::Combinator => "combinator",
            Self::MetadataReference => "metadataReference",
            Self::MetadataContinuation => "metadataContinuation",
            Self::MetadataContinuationAfterTypeArguments => "metadataContinuationAfterTypeArguments",
            Self::TypedefDeclaration => "typedefDeclaration",
            Self::FieldInitializer => "fieldInitializer",
            Self::FormalParameterDeclaration => "formalParameterDeclaration",
            Self::RecordFieldDeclaration => "recordFieldDeclaration",
            Self::CatchParameter => "catchParameter",
            Self::LibraryName => "libraryName",
            Self::LibraryNameContinuation => "libraryNameContinuation",
            Self::PartName => "partName",
            Self::PartNameContinuation => "partNameContinuation",
            Self::EnumDeclaration => "enumDeclaration",
            Self::EnumValueDeclaration => "enumValueDeclaration",
            Self::ClassOrMixinOrExtensionDeclaration => "classOrMixinOrExtensionDeclaration",
            Self::TypeVariableDeclaration => "typeVariableDeclaration",
            Self::PrefixedTypeReference => "prefixedTypeReference",
            Self::TypeReference => "typeReference",
            Self::TypeReferenceContinuation => "typeReferenceContinuation",
            Self::TopLevelVariableDeclaration => "topLevelVariableDeclaration",
            Self::FieldDeclaration => "fieldDeclaration",
            Self::TopLevelFunctionDeclaration => "topLevelFunctionDeclaration",
            Self::MethodDeclaration => "methodDeclaration",
            Self::MethodDeclarationContinuation => "methodDeclarationContinuation",
            Self::LocalFunctionDeclaration => "localFunctionDeclaration",
            Self::LocalFunctionDeclarationContinuation => "localFunctionDeclarationContinuation",
            Self::ConstructorReference => "constructorReference",
            Self::ConstructorReferenceContinuation => "constructorReferenceContinuation",
            Self::PrimaryConstructorDeclaration => "primaryConstructorDeclaration",
            Self::LabelDeclaration => "labelDeclaration",
            Self::LiteralSymbol => "literalSymbol",
            Self::LiteralSymbolContinuation => "literalSymbolContinuation",
            Self::Expression => "expression",
            Self::ExpressionContinuation => "expressionContinuation",
            Self::NamedArgumentReference => "namedArgumentReference",
            Self::NamedRecordFieldReference => "namedRecordFieldReference",
            Self::LocalVariableDeclaration => "localVariableDeclaration",
            Self::LabelReference => "labelReference",
        }
    }

    /// Dart `inDeclaration`.
    pub fn in_declaration(self) -> bool {
        todo!()
    }
    /// Dart `inLibraryOrPartOfDeclaration`.
    pub fn in_library_or_part_of_declaration(self) -> bool {
        todo!()
    }
    /// Dart `inSymbol`.
    pub fn in_symbol(self) -> bool {
        todo!()
    }
    /// Dart `isContinuation`.
    pub fn is_continuation(self) -> bool {
        todo!()
    }
    /// Dart `isScopeReference`.
    pub fn is_scope_reference(self) -> bool {
        todo!()
    }
    /// Dart `isBuiltInIdentifierAllowed`.
    pub fn is_built_in_identifier_allowed(self) -> bool {
        todo!()
    }
    /// Dart `allowedInConstantExpression`.
    pub fn allowed_in_constant_expression(self) -> bool {
        todo!()
    }
    /// Dart `recoveryTemplate` (a `cfe_codes` function taking the lexeme).
    pub fn recovery_template(self) -> fn(&str) -> CfeMessage {
        todo!()
    }
    /// Dart `allowsNewAsIdentifier`.
    pub fn allows_new_as_identifier(self) -> bool {
        todo!()
    }
    /// Dart `ensureIdentifier(token, parser)`.
    pub fn ensure_identifier<L: Listener>(self, token: TokenId, parser: &mut Parser<L>) -> TokenId {
        todo!()
    }
    /// Dart `ensureIdentifierPotentiallyRecovered(token, parser, isRecovered)`.
    pub fn ensure_identifier_potentially_recovered<L: Listener>(
        self,
        token: TokenId,
        parser: &mut Parser<L>,
        is_recovered: bool,
    ) -> TokenId {
        todo!()
    }
}

/// Dart `looksLikeExpressionStart`.
pub fn looks_like_expression_start(tokens: &Tokens, next: TokenId) -> bool {
    todo!()
}

/// Dart `looksLikePatternStart`.
pub fn looks_like_pattern_start(tokens: &Tokens, next: TokenId) -> bool {
    todo!()
}

/// Dart `looksLikeStatementStart`.
pub fn looks_like_statement_start(tokens: &Tokens, token: TokenId) -> bool {
    todo!()
}

/// Dart `isOkNextValueInFormalParameter`.
pub fn is_ok_next_value_in_formal_parameter(tokens: &Tokens, token: TokenId) -> bool {
    todo!()
}
