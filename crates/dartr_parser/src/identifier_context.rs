// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/identifier_context.dart

use dartr_diagnostics::cfe::CfeMessage;
use dartr_diagnostics::cfe_codes as diag;
use dartr_syntax::{Keyword, TokenId, TokenType, Tokens};

use crate::identifier_context_impl as imp;
use crate::listener::Listener;
use crate::parser_impl::Parser;

/// Information about the parser state that is passed to the listener at the
/// time an identifier is encountered. It is also used by the parser for error
/// recovery when a recovery template is defined.
///
/// This can be used by the listener to determine the context in which the
/// identifier appears; that in turn can help the listener decide how to resolve
/// the identifier (if the listener is doing resolution).
///
/// Dart `IdentifierContext`: the context in which an identifier occurs.
/// One variant per Dart constant (`IdentifierContext.typeReference` is
/// `IdentifierContext::TypeReference`). Dart `operatorName` is the same
/// constant as `methodDeclarationContinuation`, see
/// [`IdentifierContext::OPERATOR_NAME`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IdentifierContext {
    /// Identifier is being declared as the name of an import prefix (i.e. `Foo`
    /// in `import "..." as Foo;`)
    ImportPrefixDeclaration,
    /// Identifier is the start of a dotted name in a conditional import or
    /// export.
    DottedName,
    /// Identifier is part of a dotted name in a conditional import or export, but
    /// it's not the first identifier of the dotted name.
    DottedNameContinuation,
    /// Identifier is one of the shown/hidden names in an import/export
    /// combinator.
    Combinator,
    /// Identifier is the start of a name in an annotation that precedes a
    /// declaration (i.e. it appears directly after an `@`).
    MetadataReference,
    /// Identifier is part of a name in an annotation that precedes a declaration,
    /// but it's not the first identifier in the name.
    MetadataContinuation,
    /// Identifier is part of a name in an annotation that precedes a declaration,
    /// but it appears after type parameters (e.g. `foo` in `@X<Y>.foo()`).
    MetadataContinuationAfterTypeArguments,
    /// Identifier is the name being declared by a typedef declaration.
    TypedefDeclaration,
    /// Identifier is a field initializer in a formal parameter list (i.e. it
    /// appears directly after `this.`).
    FieldInitializer,
    /// Identifier is a formal parameter being declared as part of a function,
    /// method, or typedef declaration.
    FormalParameterDeclaration,
    /// Identifier is a record field being declared as part of a record type
    /// declaration.
    RecordFieldDeclaration,
    /// Identifier is a formal parameter being declared as part of a catch block
    /// in a try/catch/finally statement.
    CatchParameter,
    /// Identifier is the start of a library name (e.g. `foo` in the directive
    /// 'library foo;`).
    LibraryName,
    /// Identifier is part of a library name, but it's not the first identifier in
    /// the name.
    LibraryNameContinuation,
    /// Identifier is the start of a library name referenced by a `part of`
    /// directive (e.g. `foo` in the directive `part of foo;`).
    PartName,
    /// Identifier is part of a library name referenced by a `part of` directive,
    /// but it's not the first identifier in the name.
    PartNameContinuation,
    /// Identifier is the type name being declared by an enum declaration.
    EnumDeclaration,
    /// Identifier is an enumerated value name being declared by an enum
    /// declaration.
    EnumValueDeclaration,
    /// Identifier is the name being declared by a class declaration, a mixin
    /// declaration, or a named mixin application, for example,
    /// `Foo` in `class Foo = X with Y;`.
    ClassOrMixinOrExtensionDeclaration,
    /// Identifier is the name of a type variable being declared (e.g. `Foo` in
    /// `class C<Foo extends num> {}`).
    TypeVariableDeclaration,
    /// Identifier is the start of a reference to a type that starts with prefix.
    PrefixedTypeReference,
    /// Identifier is the start of a reference to a type declared elsewhere.
    TypeReference,
    /// Identifier is part of a reference to a type declared elsewhere, but it's
    /// not the first identifier of the reference.
    TypeReferenceContinuation,
    /// Identifier is a name being declared by a top level variable declaration.
    TopLevelVariableDeclaration,
    /// Identifier is a name being declared by a field declaration.
    FieldDeclaration,
    /// Identifier is the name being declared by a top level function declaration.
    TopLevelFunctionDeclaration,
    /// Identifier is the start of the name being declared by a method
    /// declaration.
    MethodDeclaration,
    /// Identifier is part of the name being declared by a method declaration,
    /// but it's not the first identifier of the name.
    ///
    /// In valid Dart, this can only happen if the identifier is the name of a
    /// named constructor which is being declared, e.g. `foo` in
    /// `class C { C.foo(); }`.
    MethodDeclarationContinuation,
    /// Identifier is the start of the name being declared by a local function
    /// declaration.
    LocalFunctionDeclaration,
    /// Identifier is part of the name being declared by a local function
    /// declaration, but it's not the first identifier of the name.
    ///
    /// TODO(paulberry,ahe): Does this ever occur in valid Dart, or does it only
    /// occur as part of error recovery?
    LocalFunctionDeclarationContinuation,
    /// Identifier is the start of a reference to a constructor declared
    /// elsewhere.
    ConstructorReference,
    /// Identifier is part of a reference to a constructor declared elsewhere, but
    /// it's not the first identifier of the reference.
    ConstructorReferenceContinuation,
    /// Identifier is part of a reference to a constructor declared elsewhere, but
    /// it appears after type parameters (e.g. `foo` in `X<Y>.foo`).
    ///
    /// Added by the port: missing in the skeleton, used by
    /// `parseConstructorReference`.
    ConstructorReferenceContinuationAfterTypeArguments,
    /// Identifier is the name of a primary constructor declaration.
    PrimaryConstructorDeclaration,
    /// Identifier is the declaration of a label (i.e. it is followed by `:` and
    /// then a statement).
    LabelDeclaration,
    /// Identifier is the start of a reference occurring in a literal symbol (e.g.
    /// `foo` in `#foo`).
    LiteralSymbol,
    /// Identifier is part of a reference occurring in a literal symbol, but it's
    /// not the first identifier of the reference (e.g. `foo` in `#prefix.foo`).
    LiteralSymbolContinuation,
    /// Identifier appears in an expression, and it does not immediately follow a
    /// `.`.
    Expression,
    /// Identifier appears in an expression, and it immediately follows a `.`.
    ExpressionContinuation,
    /// Identifier is a reference to a named argument of a function or method
    /// invocation (e.g. `foo` in `f(foo: 0);`.
    NamedArgumentReference,
    /// Identifier is a reference to a named record field
    /// (e.g. `foo` in `(42, foo: 42);`.
    NamedRecordFieldReference,
    /// Identifier is a name being declared by a local variable declaration.
    LocalVariableDeclaration,
    /// Identifier is a reference to a label (e.g. `foo` in `break foo;`).
    /// Labels have their own scope.
    LabelReference,
}

impl IdentifierContext {
    /// Dart `IdentifierContext.operatorName` (identical to
    /// `methodDeclarationContinuation` in Dart: same const constructor call).
    ///
    /// Identifier appears after the word `operator` in a method declaration.
    ///
    /// TODO(paulberry,ahe): Does this ever occur in valid Dart, or does it only
    /// occur as part of error recovery?  If it's only as part of error recovery,
    /// perhaps we should just re-use methodDeclaration.
    pub const OPERATOR_NAME: IdentifierContext = IdentifierContext::MethodDeclarationContinuation;

    /// Dart `toString()` (the `_name` field).
    /// Dart (line 314): `String toString() => _name;`
    pub fn name(self) -> &'static str {
        match self {
            Self::ImportPrefixDeclaration => "importPrefixDeclaration",
            Self::DottedName => "dottedName",
            Self::DottedNameContinuation => "dottedNameContinuation",
            Self::Combinator => "combinator",
            Self::MetadataReference => "metadataReference",
            Self::MetadataContinuation => "metadataContinuation",
            Self::MetadataContinuationAfterTypeArguments => {
                "metadataContinuationAfterTypeArguments"
            }
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
            // Dart `ClassOrMixinOrExtensionIdentifierContext()` passes
            // 'classOrMixinDeclaration' as the name.
            Self::ClassOrMixinOrExtensionDeclaration => "classOrMixinDeclaration",
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
            Self::ConstructorReferenceContinuationAfterTypeArguments => {
                "constructorReferenceContinuationAfterTypeArguments"
            }
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

    /// Indicates whether the identifier represents a name which is being
    /// declared.
    ///
    /// Dart (line 272): `final bool inDeclaration;` (default `false`).
    pub fn in_declaration(self) -> bool {
        match self {
            Self::ImportPrefixDeclaration
            | Self::TypedefDeclaration
            | Self::FieldInitializer
            | Self::FormalParameterDeclaration
            | Self::RecordFieldDeclaration
            | Self::EnumDeclaration
            | Self::EnumValueDeclaration
            | Self::ClassOrMixinOrExtensionDeclaration
            | Self::TypeVariableDeclaration
            | Self::TopLevelVariableDeclaration
            | Self::FieldDeclaration
            | Self::TopLevelFunctionDeclaration
            | Self::MethodDeclaration
            | Self::MethodDeclarationContinuation
            | Self::PrimaryConstructorDeclaration
            | Self::LocalFunctionDeclaration
            | Self::LocalFunctionDeclarationContinuation
            | Self::LabelDeclaration
            | Self::LocalVariableDeclaration => true,
            Self::DottedName
            | Self::DottedNameContinuation
            | Self::Combinator
            | Self::MetadataReference
            | Self::MetadataContinuation
            | Self::MetadataContinuationAfterTypeArguments
            | Self::CatchParameter
            | Self::LibraryName
            | Self::LibraryNameContinuation
            | Self::PartName
            | Self::PartNameContinuation
            | Self::PrefixedTypeReference
            | Self::TypeReference
            | Self::TypeReferenceContinuation
            | Self::ConstructorReference
            | Self::ConstructorReferenceContinuation
            | Self::ConstructorReferenceContinuationAfterTypeArguments
            | Self::LiteralSymbol
            | Self::LiteralSymbolContinuation
            | Self::Expression
            | Self::ExpressionContinuation
            | Self::NamedArgumentReference
            | Self::NamedRecordFieldReference
            | Self::LabelReference => false,
        }
    }

    /// Indicates whether the identifier is within a `library` or `part of`
    /// declaration.
    ///
    /// Dart (line 276): `final bool inLibraryOrPartOfDeclaration;` (default
    /// `false`).
    pub fn in_library_or_part_of_declaration(self) -> bool {
        matches!(
            self,
            Self::LibraryName
                | Self::LibraryNameContinuation
                | Self::PartName
                | Self::PartNameContinuation
        )
    }

    /// Indicates whether the identifier is within a symbol literal.
    ///
    /// Dart (line 279): `final bool inSymbol;` (default `false`).
    pub fn in_symbol(self) -> bool {
        matches!(self, Self::LiteralSymbol | Self::LiteralSymbolContinuation)
    }

    /// Indicates whether the identifier follows a `.`.
    ///
    /// Dart (line 282): `final bool isContinuation;` (default `false`).
    pub fn is_continuation(self) -> bool {
        match self {
            Self::DottedNameContinuation
            | Self::MetadataContinuation
            | Self::MetadataContinuationAfterTypeArguments
            | Self::FieldInitializer
            | Self::LibraryNameContinuation
            | Self::PartNameContinuation
            | Self::TypeReferenceContinuation
            | Self::MethodDeclarationContinuation
            | Self::PrimaryConstructorDeclaration
            | Self::LocalFunctionDeclarationContinuation
            | Self::ConstructorReferenceContinuation
            | Self::ConstructorReferenceContinuationAfterTypeArguments
            | Self::LiteralSymbolContinuation
            | Self::ExpressionContinuation => true,
            Self::ImportPrefixDeclaration
            | Self::DottedName
            | Self::Combinator
            | Self::MetadataReference
            | Self::TypedefDeclaration
            | Self::FormalParameterDeclaration
            | Self::RecordFieldDeclaration
            | Self::CatchParameter
            | Self::LibraryName
            | Self::PartName
            | Self::EnumDeclaration
            | Self::EnumValueDeclaration
            | Self::ClassOrMixinOrExtensionDeclaration
            | Self::TypeVariableDeclaration
            | Self::PrefixedTypeReference
            | Self::TypeReference
            | Self::TopLevelVariableDeclaration
            | Self::FieldDeclaration
            | Self::TopLevelFunctionDeclaration
            | Self::MethodDeclaration
            | Self::LocalFunctionDeclaration
            | Self::ConstructorReference
            | Self::LabelDeclaration
            | Self::LiteralSymbol
            | Self::Expression
            | Self::NamedArgumentReference
            | Self::NamedRecordFieldReference
            | Self::LocalVariableDeclaration
            | Self::LabelReference => false,
        }
    }

    /// Indicates whether the identifier should be looked up in the current scope.
    ///
    /// Dart (line 285): `final bool isScopeReference;` (default `false`).
    pub fn is_scope_reference(self) -> bool {
        matches!(
            self,
            Self::MetadataReference
                | Self::PrefixedTypeReference
                | Self::TypeReference
                | Self::ConstructorReference
                | Self::Expression
        )
    }

    /// Indicates whether built-in identifiers are allowed in this context.
    ///
    /// Dart (line 288): `final bool isBuiltInIdentifierAllowed;` (default
    /// `true`).
    pub fn is_built_in_identifier_allowed(self) -> bool {
        !matches!(
            self,
            Self::ImportPrefixDeclaration
                | Self::TypedefDeclaration
                | Self::EnumDeclaration
                | Self::ClassOrMixinOrExtensionDeclaration
                | Self::TypeVariableDeclaration
                | Self::TypeReference
                | Self::TypeReferenceContinuation
        )
    }

    /// Indicated whether the identifier is allowed in a context where constant
    /// expressions are required.
    ///
    /// Dart (line 292): `final bool allowedInConstantExpression;`, computed in
    /// the constructor (line 306):
    /// `allowedInConstantExpression ?? (inDeclaration || isContinuation || inSymbol)`.
    pub fn allowed_in_constant_expression(self) -> bool {
        match self {
            // Explicit `allowedInConstantExpression: true`.
            Self::NamedArgumentReference | Self::NamedRecordFieldReference => true,
            // Generally, declarations are legal in constant expressions.  A
            // continuation doesn't affect constant expressions: if what it's
            // continuing is a problem, it has already been reported.
            _ => self.in_declaration() || self.is_continuation() || self.in_symbol(),
        }
    }

    /// Dart `recoveryTemplate` (a `cfe_codes` function taking the lexeme).
    ///
    /// Dart (line 294): `final Template<...> recoveryTemplate;` (default
    /// `diag.expectedIdentifier`).
    pub fn recovery_template(self) -> fn(&str) -> CfeMessage {
        match self {
            Self::TypeReference | Self::PrefixedTypeReference => diag::expected_type,
            _ => diag::expected_identifier,
        }
    }

    /// Indicates whether the token `new` in this context should be treated as a
    /// valid identifier, under the rules of the "constructor tearoff" feature.
    /// Note that if the feature is disabled, such uses of `new` are still parsed
    /// as identifiers, however the parser will report an appropriate error; this
    /// should allow the best possible error recovery in the event that a user
    /// attempts to use the feature with a language version that doesn't permit
    /// it.
    ///
    /// Dart (line 323): `bool get allowsNewAsIdentifier => false;`, overridden
    /// by `ConstructorReferenceIdentifierContext`,
    /// `ExpressionIdentifierContext`, `MetadataReferenceIdentifierContext`,
    /// `MethodDeclarationIdentifierContext` (`=> isContinuation`) and
    /// `FieldInitializerIdentifierContext` (`=> true`).
    pub fn allows_new_as_identifier(self) -> bool {
        match self {
            Self::ConstructorReference
            | Self::ConstructorReferenceContinuation
            | Self::ConstructorReferenceContinuationAfterTypeArguments
            | Self::Expression
            | Self::ExpressionContinuation
            | Self::MetadataReference
            | Self::MetadataContinuation
            | Self::MetadataContinuationAfterTypeArguments
            | Self::MethodDeclaration
            | Self::MethodDeclarationContinuation
            | Self::PrimaryConstructorDeclaration => self.is_continuation(),
            Self::FieldInitializer => true,
            _ => false,
        }
    }

    /// Ensure that the next token is an identifier (or keyword which should be
    /// treated as an identifier) and return that identifier.
    /// Report errors as necessary via [parser].
    ///
    /// Dart (line 328): `Token ensureIdentifier(Token token, Parser parser);`
    /// (abstract; dispatched to the `*IdentifierContext` class of the
    /// constant).
    pub fn ensure_identifier<L: Listener>(self, token: TokenId, parser: &mut Parser<L>) -> TokenId {
        match self {
            Self::ImportPrefixDeclaration => {
                imp::import_prefix_ensure_identifier(self, token, parser)
            }
            Self::DottedName | Self::DottedNameContinuation => {
                imp::dotted_name_ensure_identifier(self, token, parser)
            }
            Self::Combinator => imp::combinator_ensure_identifier(self, token, parser),
            Self::MetadataReference
            | Self::MetadataContinuation
            | Self::MetadataContinuationAfterTypeArguments => {
                imp::metadata_reference_ensure_identifier(self, token, parser)
            }
            Self::TypedefDeclaration => {
                imp::typedef_declaration_ensure_identifier(self, token, parser)
            }
            Self::FieldInitializer => imp::field_initializer_ensure_identifier(self, token, parser),
            Self::FormalParameterDeclaration => {
                imp::formal_parameter_declaration_ensure_identifier(self, token, parser)
            }
            Self::RecordFieldDeclaration => {
                imp::record_field_declaration_ensure_identifier(self, token, parser)
            }
            Self::CatchParameter => imp::catch_parameter_ensure_identifier(self, token, parser),
            Self::LibraryName
            | Self::LibraryNameContinuation
            | Self::PartName
            | Self::PartNameContinuation => imp::library_ensure_identifier(self, token, parser),
            Self::EnumDeclaration => imp::enum_declaration_ensure_identifier(self, token, parser),
            Self::EnumValueDeclaration => {
                imp::enum_value_declaration_ensure_identifier(self, token, parser)
            }
            Self::ClassOrMixinOrExtensionDeclaration => {
                imp::class_or_mixin_or_extension_ensure_identifier(self, token, parser)
            }
            Self::TypeVariableDeclaration => {
                imp::type_variable_declaration_ensure_identifier(self, token, parser)
            }
            Self::PrefixedTypeReference | Self::TypeReference | Self::TypeReferenceContinuation => {
                imp::type_reference_ensure_identifier(self, token, parser)
            }
            Self::TopLevelVariableDeclaration | Self::TopLevelFunctionDeclaration => {
                imp::top_level_declaration_ensure_identifier(self, token, parser)
            }
            Self::FieldDeclaration => imp::field_declaration_ensure_identifier(self, token, parser),
            Self::MethodDeclaration
            | Self::MethodDeclarationContinuation
            | Self::PrimaryConstructorDeclaration => {
                imp::method_declaration_ensure_identifier(self, token, parser)
            }
            Self::LocalFunctionDeclaration | Self::LocalFunctionDeclarationContinuation => {
                imp::local_function_declaration_ensure_identifier(self, token, parser)
            }
            Self::ConstructorReference
            | Self::ConstructorReferenceContinuation
            | Self::ConstructorReferenceContinuationAfterTypeArguments => {
                imp::constructor_reference_ensure_identifier(self, token, parser)
            }
            Self::LabelDeclaration => imp::label_declaration_ensure_identifier(self, token, parser),
            Self::LiteralSymbol | Self::LiteralSymbolContinuation => {
                imp::literal_symbol_ensure_identifier(self, token, parser)
            }
            Self::Expression | Self::ExpressionContinuation => {
                imp::expression_ensure_identifier(self, token, parser)
            }
            Self::NamedArgumentReference => {
                imp::named_argument_reference_ensure_identifier(self, token, parser)
            }
            Self::NamedRecordFieldReference => {
                imp::named_record_field_reference_ensure_identifier(self, token, parser)
            }
            Self::LocalVariableDeclaration => {
                imp::local_variable_declaration_ensure_identifier(self, token, parser)
            }
            Self::LabelReference => imp::label_reference_ensure_identifier(self, token, parser),
        }
    }

    /// Ensure that the next token is an identifier (or keyword which should be
    /// treated as an identifier) and return that identifier.
    /// Report errors as necessary via [parser].
    /// If [isRecovered] implementers could allow 'token' to be used as an
    /// identifier, even if it isn't a valid identifier.
    ///
    /// Dart (line 335): `Token ensureIdentifierPotentiallyRecovered(Token
    /// token, Parser parser, bool isRecovered) => ensureIdentifier(token,
    /// parser);`, overridden by `FieldDeclarationIdentifierContext`,
    /// `MethodDeclarationIdentifierContext`,
    /// `TopLevelDeclarationIdentifierContext` and
    /// `TypedefDeclarationIdentifierContext`.
    pub fn ensure_identifier_potentially_recovered<L: Listener>(
        self,
        token: TokenId,
        parser: &mut Parser<L>,
        is_recovered: bool,
    ) -> TokenId {
        match self {
            Self::FieldDeclaration => {
                imp::field_declaration_ensure_identifier_potentially_recovered(
                    self,
                    token,
                    parser,
                    is_recovered,
                )
            }
            Self::MethodDeclaration
            | Self::MethodDeclarationContinuation
            | Self::PrimaryConstructorDeclaration => {
                imp::method_declaration_ensure_identifier_potentially_recovered(
                    self,
                    token,
                    parser,
                    is_recovered,
                )
            }
            Self::TopLevelVariableDeclaration | Self::TopLevelFunctionDeclaration => {
                imp::top_level_declaration_ensure_identifier_potentially_recovered(
                    self,
                    token,
                    parser,
                    is_recovered,
                )
            }
            Self::TypedefDeclaration => {
                imp::typedef_declaration_ensure_identifier_potentially_recovered(
                    self,
                    token,
                    parser,
                    is_recovered,
                )
            }
            _ => self.ensure_identifier(token, parser),
        }
    }
}

#[inline(always)]
fn is_a(tokens: &Tokens, token: TokenId, ty: TokenType) -> bool {
    tokens.ty(token) == ty
}

/// Return `true` if [next] should be treated like the start of an expression
/// for the purposes of recovery.
///
/// Dart (line 344): `bool looksLikeExpressionStart(Token next)`
pub fn looks_like_expression_start(tokens: &Tokens, next: TokenId) -> bool {
    tokens.get(next).is_identifier()
        || tokens.ty(next).is_keyword() && !looks_like_statement_start(tokens, next)
        || is_a(tokens, next, TokenType::DOUBLE)
        || is_a(tokens, next, TokenType::DOUBLE_WITH_SEPARATORS)
        || is_a(tokens, next, TokenType::HASH)
        || is_a(tokens, next, TokenType::HEXADECIMAL)
        || is_a(tokens, next, TokenType::HEXADECIMAL_WITH_SEPARATORS)
        || is_a(tokens, next, TokenType::IDENTIFIER)
        || is_a(tokens, next, TokenType::INT)
        || is_a(tokens, next, TokenType::INT_WITH_SEPARATORS)
        || is_a(tokens, next, TokenType::STRING)
        || is_a(tokens, next, TokenType::OPEN_CURLY_BRACKET)
        || is_a(tokens, next, TokenType::OPEN_PAREN)
        || is_a(tokens, next, TokenType::OPEN_SQUARE_BRACKET)
        || is_a(tokens, next, TokenType::INDEX)
        || is_a(tokens, next, TokenType::LT)
        || is_a(tokens, next, TokenType::BANG)
        || is_a(tokens, next, TokenType::MINUS)
        || is_a(tokens, next, TokenType::TILDE)
        || is_a(tokens, next, TokenType::PLUS_PLUS)
        || is_a(tokens, next, TokenType::MINUS_MINUS)
}

/// Returns `true` if [next] should be treated like the start of a pattern for
/// the purposes of recovery.
///
/// Note: since the syntax for patterns is very similar to that for expressions,
/// we mostly re-use [looksLikeExpressionStart].
///
/// Dart (line 372): `bool looksLikePatternStart(Token next)`
pub fn looks_like_pattern_start(tokens: &Tokens, next: TokenId) -> bool {
    tokens.get(next).is_identifier()
        || is_a(tokens, next, TokenType::DOUBLE)
        || is_a(tokens, next, TokenType::DOUBLE_WITH_SEPARATORS)
        || is_a(tokens, next, TokenType::HASH)
        || is_a(tokens, next, TokenType::HEXADECIMAL)
        || is_a(tokens, next, TokenType::HEXADECIMAL_WITH_SEPARATORS)
        || is_a(tokens, next, TokenType::IDENTIFIER)
        || is_a(tokens, next, TokenType::INT)
        || is_a(tokens, next, TokenType::INT_WITH_SEPARATORS)
        || is_a(tokens, next, TokenType::STRING)
        || is_a(tokens, next, Keyword::NULL)
        || is_a(tokens, next, Keyword::FALSE)
        || is_a(tokens, next, Keyword::TRUE)
        || is_a(tokens, next, TokenType::OPEN_CURLY_BRACKET)
        || is_a(tokens, next, TokenType::OPEN_PAREN)
        || is_a(tokens, next, TokenType::OPEN_SQUARE_BRACKET)
        || is_a(tokens, next, TokenType::INDEX)
        || is_a(tokens, next, TokenType::LT)
        || is_a(tokens, next, TokenType::LT_EQ)
        || is_a(tokens, next, TokenType::GT)
        || is_a(tokens, next, TokenType::GT_EQ)
        || is_a(tokens, next, TokenType::BANG_EQ)
        || is_a(tokens, next, TokenType::EQ_EQ)
        || is_a(tokens, next, Keyword::VAR)
        || is_a(tokens, next, Keyword::FINAL)
        || is_a(tokens, next, Keyword::CONST)
}

/// Return `true` if the given [token] should be treated like the start of
/// a new statement for the purposes of recovery.
///
/// Dart (line 402): `bool looksLikeStatementStart(Token token)`
pub fn looks_like_statement_start(tokens: &Tokens, token: TokenId) -> bool {
    is_a(tokens, token, TokenType::AT)
        || is_a(tokens, token, Keyword::ASSERT)
        || is_a(tokens, token, Keyword::BREAK)
        || is_a(tokens, token, Keyword::CONTINUE)
        || is_a(tokens, token, Keyword::DO)
        || is_a(tokens, token, Keyword::ELSE)
        || is_a(tokens, token, Keyword::FINAL)
        || is_a(tokens, token, Keyword::FOR)
        || is_a(tokens, token, Keyword::IF)
        || is_a(tokens, token, Keyword::RETURN)
        || is_a(tokens, token, Keyword::SWITCH)
        || is_a(tokens, token, Keyword::TRY)
        || is_a(tokens, token, Keyword::VAR)
        || is_a(tokens, token, Keyword::VOID)
        || is_a(tokens, token, Keyword::WHILE)
        || is_a(tokens, token, TokenType::EOF)
}

/// Dart (line 420): `bool isOkNextValueInFormalParameter(Token token)`
pub fn is_ok_next_value_in_formal_parameter(tokens: &Tokens, token: TokenId) -> bool {
    is_a(tokens, token, TokenType::EQ)
        || is_a(tokens, token, TokenType::COLON)
        || is_a(tokens, token, TokenType::COMMA)
        || is_a(tokens, token, TokenType::CLOSE_PAREN)
        || is_a(tokens, token, TokenType::CLOSE_SQUARE_BRACKET)
        || is_a(tokens, token, TokenType::CLOSE_CURLY_BRACKET)
}
