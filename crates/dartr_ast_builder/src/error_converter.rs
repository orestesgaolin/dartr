// Dart source: pkg/analyzer/lib/src/fasta/error_converter.dart

//! `FastaErrorReporter`: converts the CFE messages that the parser reports
//! into analyzer diagnostics.

use rustc_hash::FxHashSet;

use dartr_diagnostics::cfe::{CfeArg, CfeMessage, PseudoSharedCode, SharedCode};
use dartr_diagnostics::{Diagnostic, DiagnosticCode, LocatableDiagnostic, LocatedDiagnostic, diag};

/// The diagnostics of one parse (Dart `RecordingDiagnosticListener` behind
/// a `DiagnosticReporter`): kept in report order; a diagnostic equal to one
/// reported before (same code, offset, length and message) is dropped,
/// because the Dart listener keeps the diagnostics in a set.
#[derive(Debug, Default)]
pub struct DiagnosticCollector {
    pub diagnostics: Vec<Diagnostic>,
    seen: FxHashSet<(&'static str, usize, usize, String)>,
}

impl DiagnosticCollector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Dart `DiagnosticListener.onDiagnostic`.
    pub fn on_diagnostic(&mut self, diagnostic: Diagnostic) {
        let key = (
            diagnostic.code.unique_name,
            diagnostic.offset,
            diagnostic.length,
            diagnostic.message.clone(),
        );
        if self.seen.insert(key) {
            self.diagnostics.push(diagnostic);
        }
    }
}

/// An error reporter that knows how to convert a Fasta error into an
/// analyzer error (Dart `FastaErrorReporter`).
#[derive(Debug, Default)]
pub struct FastaErrorReporter {
    /// Dart `diagnosticReporter` (never `null` here).
    pub diagnostic_reporter: DiagnosticCollector,
}

impl FastaErrorReporter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Dart `DiagnosticReporter.report(LocatedDiagnostic)`.
    pub fn report(&mut self, located: LocatedDiagnostic) {
        self.diagnostic_reporter
            .on_diagnostic(located.into_diagnostic());
    }

    /// Dart `DiagnosticReporter.reportError(Diagnostic)`.
    pub fn report_error(&mut self, diagnostic: Diagnostic) {
        self.diagnostic_reporter.on_diagnostic(diagnostic);
    }

    /// Dart `reportByCode`.
    pub fn report_by_code(
        &mut self,
        pseudo_shared_code: Option<PseudoSharedCode>,
        offset: usize,
        length: usize,
        message: &CfeMessage,
    ) {
        use PseudoSharedCode as P;
        let lexeme = || -> String {
            match argument(message, "lexeme") {
                Some(CfeArg::Token(s)) | Some(CfeArg::String(s)) => s.clone(),
                a => panic!("no lexeme argument in {}: {a:?}", message.code.name),
            }
        };
        let simple = |code: LocatableDiagnostic| code.at_offset(offset, length);
        let located = match pseudo_shared_code {
            Some(P::AsyncForInWrongContext) => simple(diag::async_for_in_wrong_context()),
            Some(P::AsyncKeywordUsedAsIdentifier) => {
                simple(diag::async_keyword_used_as_identifier())
            }
            Some(P::AwaitInWrongContext) => simple(diag::await_in_wrong_context()),
            Some(P::BuiltInIdentifierAsType) => {
                simple(diag::built_in_identifier_as_type(&lexeme()))
            }
            // Reported by [ErrorVerifier]
            Some(P::ConstConstructorWithBody) => return,
            // Reported by [ErrorVerifier]
            Some(P::ConstNotInitialized) => return,
            Some(P::DefaultValueInFunctionType) => simple(diag::default_value_in_function_type()),
            Some(P::ExpectedClassMember) => simple(diag::expected_class_member()),
            Some(P::ExpectedExecutable) => simple(diag::expected_executable()),
            Some(P::ExpectedStringLiteral) => simple(diag::expected_string_literal()),
            Some(P::ExpectedToken) => {
                let expected = match argument(message, "expected") {
                    Some(CfeArg::String(s)) | Some(CfeArg::Token(s)) => s.clone(),
                    a => panic!("no expected argument: {a:?}"),
                };
                simple(diag::expected_token(&expected))
            }
            Some(P::ExpectedTypeName) => simple(diag::expected_type_name()),
            // Reported by [ErrorVerifier._checkForExtensionDeclaresInstanceField]
            Some(P::ExtensionDeclaresInstanceField) => return,
            // Reported by [ErrorVerifier]
            Some(P::FinalNotInitialized) => return,
            Some(P::GetterWithParameters) => simple(diag::getter_with_parameters()),
            Some(P::IllegalCharacter) => {
                let code_point =
                    match argument(message, "unicode").or_else(|| argument(message, "character")) {
                        Some(CfeArg::Int(i)) => *i,
                        a => panic!("no code point argument: {a:?}"),
                    };
                simple(diag::illegal_character(code_point))
            }
            Some(P::InvalidInlineFunctionType) => simple(diag::invalid_inline_function_type()),
            Some(P::InvalidLiteralInConfiguration) => {
                simple(diag::invalid_literal_in_configuration())
            }
            Some(P::InvalidCodePoint) => simple(diag::invalid_code_point("\\u{...}")),
            Some(P::InvalidModifierOnSetter) => {
                self.report_by_code_with_arguments(
                    offset,
                    length,
                    &diag::INVALID_MODIFIER_ON_SETTER,
                    message,
                );
                return;
            }
            Some(P::MissingDigit) => simple(diag::missing_digit()),
            Some(P::MissingEnumBody) => simple(diag::missing_enum_body()),
            Some(P::MissingFunctionBody) => simple(diag::missing_function_body()),
            Some(P::MissingFunctionParameters) => simple(diag::missing_function_parameters()),
            Some(P::MissingHexDigit) => simple(diag::missing_hex_digit()),
            Some(P::MissingIdentifier) => simple(diag::missing_identifier()),
            Some(P::MissingMethodParameters) => simple(diag::missing_method_parameters()),
            Some(P::MissingStarAfterSync) => simple(diag::missing_star_after_sync()),
            Some(P::MissingTypedefParameters) => simple(diag::missing_typedef_parameters()),
            Some(P::MultipleImplementsClauses) => simple(diag::multiple_implements_clauses()),
            Some(P::NamedFunctionExpression) => simple(diag::named_function_expression()),
            Some(P::NamedParameterOutsideGroup) => simple(diag::named_parameter_outside_group()),
            Some(P::NonPartOfDirectiveInPart) => simple(diag::non_part_of_directive_in_part()),
            Some(P::NonSyncFactory) => simple(diag::non_sync_factory()),
            Some(P::PositionalAfterNamedArgument) => {
                simple(diag::positional_after_named_argument())
            }
            Some(P::ReturnInGenerator) => simple(diag::return_in_generator()),
            Some(P::UnexpectedDollarInString) => simple(diag::unexpected_dollar_in_string()),
            Some(P::UnexpectedToken) => simple(diag::unexpected_token(&lexeme())),
            Some(P::UnterminatedMultiLineComment) => {
                simple(diag::unterminated_multi_line_comment())
            }
            Some(P::UnterminatedStringLiteral) => simple(diag::unterminated_string_literal()),
            Some(P::WrongSeparatorForPositionalParameter) => {
                simple(diag::wrong_separator_for_positional_parameter())
            }
            // Reported by [YieldStatementResolver._resolve_notGenerator]
            Some(P::YieldInNonGenerator) => return,
            // Reported by [ErrorVerifier._checkForBuiltInIdentifierAsName].
            Some(P::BuiltInIdentifierInDeclaration) => return,
            Some(P::PrivateOptionalParameter) => simple(diag::private_optional_parameter()),
            Some(P::PrivateNamedNonFieldParameter) => {
                simple(diag::private_named_non_field_parameter())
            }
            // Not reported but followed by a MISSING_FUNCTION_BODY error.
            Some(P::NonSyncAbstractMethod) => return,
            // Not reported but followed by a
            // CompileTimeErrorCode.EXTENSION_DECLARES_INSTANCE_FIELD.
            Some(P::AbstractExtensionField) => return,
            // Reported by [ErrorVerifier._checkForExtensionTypeWithAbstractMember].
            Some(P::ExtensionTypeWithAbstractMember) => return,
            // Reported by
            // [ErrorVerifier._checkForExtensionTypeDeclaresInstanceField]
            Some(P::ExtensionTypeDeclaresInstanceField) => return,
            // Handled by `translateErrorToken`.
            Some(P::Encoding)
            | Some(P::UnexpectedSeparatorInNumber)
            | Some(P::UnsupportedOperator) => {
                debug_assert!(false, "Should be handled by translateErrorToken");
                return;
            }
            // Reported as EXPECTED_TWO_MAP_TYPE_ARGUMENTS in
            // [TypeArgumentsVerifier.checkMapLiteral].
            Some(P::SetOrMapLiteralTooManyTypeArguments) => return,
            // Reported as UNDEFINED_IDENTIFIER in
            // [SimpleIdentifierResolver._resolve1], followed by an
            // EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD error, or followed by an
            // EXPECTED_TOKEN error.
            Some(P::AssertAsExpression) => return,
            Some(P::FastaCliArgumentRequired)
            | Some(P::InternalProblemStackNotEmpty)
            | Some(P::InternalProblemUnhandled)
            | Some(P::InternalProblemUnsupported)
            | Some(P::Unspecified)
            | None => {
                // Dart: `assert(false, "Unreported message ...")`.
                return;
            }
        };
        self.report(located);
    }

    /// Report an error based on the given [message] whose range is
    /// described by the given [offset] and [length] (Dart `reportMessage`).
    pub fn report_message(&mut self, message: &CfeMessage, offset: usize, length: usize) {
        let code = message.code;
        if let Some(shared_code) = code.shared_code {
            // Reported by [ErrorVerifier].
            if matches!(
                shared_code,
                SharedCode::ExternalFactoryWithBody
                    | SharedCode::RedirectingConstructorWithBody
                    | SharedCode::ExternalMethodWithBody
                    | SharedCode::ExtensionDeclaresAbstractMember
            ) {
                return;
            }
            self.report_error(Diagnostic::with_arguments(
                shared_code.analyzer_code(),
                offset,
                length,
                &message.analyzer_arguments(),
                Vec::new(),
            ));
            return;
        }
        self.report_by_code(code.pseudo_shared_code, offset, length, message);
    }

    /// Dart `reportScannerError`.
    pub fn report_scanner_error(&mut self, diagnostic: Diagnostic) {
        self.report_error(diagnostic);
    }

    /// Dart `_reportByCode`: [code] with the arguments of [message].
    fn report_by_code_with_arguments(
        &mut self,
        offset: usize,
        length: usize,
        code: &'static DiagnosticCode,
        message: &CfeMessage,
    ) {
        self.report_error(Diagnostic::with_arguments(
            code,
            offset,
            length,
            &message.analyzer_arguments(),
            Vec::new(),
        ));
    }
}

/// Dart `message.arguments[name]`.
fn argument<'a>(message: &'a CfeMessage, name: &str) -> Option<&'a CfeArg> {
    message
        .code
        .parameters
        .iter()
        .position(|p| p.name == name)
        .and_then(|i| message.arguments.get(i))
}
