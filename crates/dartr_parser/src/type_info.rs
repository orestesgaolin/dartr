// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/type_info.dart

//! [`TypeInfo`] and [`TypeParamOrArgInfo`]: information about a type
//! reference or about type parameters or arguments, computed by
//! [`compute_type`] and [`compute_type_param_or_arg`]. The implementations
//! (the Dart classes) are in [`crate::type_info_impl`].
//!
//! # `&Tokens` and `&mut Tokens`
//!
//! The Dart compute and skip methods split `>>`, `>=`, ... into new tokens
//! (not inserted into the stream; see [`crate::type_info_impl`]). The
//! functions that take `&Tokens` ([`compute_type`], [`TypeInfo::skip_type`],
//! ...) keep these tokens in a local table and return arena tokens in their
//! place. The `*_mut` functions ([`compute_type_mut`],
//! [`TypeInfo::skip_type_mut`], ...) add them to the arena, as Dart does,
//! and give results that are identical to Dart in all cases.

use dartr_syntax::token_constants::{IDENTIFIER_TOKEN, KEYWORD_TOKEN};
use dartr_syntax::{Keyword, TokenId, TokenType, Tokens};

use crate::listener::Listener;
use crate::parser_impl::Parser;
use crate::type_info_impl::{
    ComplexTypeInfo, ComplexTypeInfoRef, ComplexTypeParamOrArgInfo, NoType, NoTypeParamOrArg,
    Overlay, PREFIXED_TYPE, PrefixedType, SIMPLE_NULLABLE_TYPE,
    SIMPLE_NULLABLE_TYPE_WITH_1_ARGUMENT, SIMPLE_TYPE, SIMPLE_TYPE_ARGUMENT_1,
    SIMPLE_TYPE_ARGUMENT_1_GT_EQ, SIMPLE_TYPE_ARGUMENT_1_GT_GT, SIMPLE_TYPE_WITH_1_ARGUMENT,
    SIMPLE_TYPE_WITH_1_ARGUMENT_GT_EQ, SIMPLE_TYPE_WITH_1_ARGUMENT_GT_GT, SimpleType,
    SimpleTypeArgument1, SimpleTypeWith1Argument, TokenRead, TokenView, VoidType,
    looks_like_name_g,
};

/// [TypeInfo] provides information collected by [computeType]
/// about a particular type reference.
/// Dart (line 22): `abstract class TypeInfo`
///
/// Dart compares instances with the constants (`identical(typeInfo,
/// noType)`); here `==` with [`NO_TYPE`] etc. Two [`TypeInfo::Complex`]
/// values are `==` when they are the same Dart object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeInfo {
    /// Dart `NoType` ([`NO_TYPE`]).
    NoType,
    /// Dart `VoidType` ([`VOID_TYPE`]).
    VoidType,
    /// Dart `SimpleType` ([`SIMPLE_TYPE`]).
    SimpleType,
    /// Dart `SimpleNullableType` ([`SIMPLE_NULLABLE_TYPE`]).
    SimpleNullableType,
    /// Dart `PrefixedType` ([`PREFIXED_TYPE`]).
    PrefixedType,
    /// Dart `SimpleTypeWith1Argument(typeArg)`
    /// ([`SIMPLE_TYPE_WITH_1_ARGUMENT`], [`SIMPLE_TYPE_WITH_1_ARGUMENT_GT_EQ`],
    /// [`SIMPLE_TYPE_WITH_1_ARGUMENT_GT_GT`]).
    SimpleTypeWith1Argument(TypeParamOrArgInfo),
    /// Dart `SimpleNullableTypeWith1Argument`
    /// ([`SIMPLE_NULLABLE_TYPE_WITH_1_ARGUMENT`]).
    SimpleNullableTypeWith1Argument,
    /// Dart `ComplexTypeInfo`.
    Complex(ComplexTypeInfoRef),
}

/// [NoType] is a specialized [TypeInfo] returned by [computeType] when
/// there is no type information in the source.
/// Dart `noType`.
pub const NO_TYPE: TypeInfo = TypeInfo::NoType;

/// [VoidType] is a specialized [TypeInfo] returned by [computeType] when
/// `void` appears in the source.
/// Dart `voidType`.
pub const VOID_TYPE: TypeInfo = TypeInfo::VoidType;

impl TypeInfo {
    /// Return type info representing the receiver without the trailing `?`
    /// or the receiver if the receiver does not represent a nullable type.
    /// Dart `asNonNullable`.
    pub fn as_non_nullable(&self) -> TypeInfo {
        match self {
            TypeInfo::SimpleNullableType => SIMPLE_TYPE,
            TypeInfo::SimpleNullableTypeWith1Argument => SIMPLE_TYPE_WITH_1_ARGUMENT,
            TypeInfo::Complex(c) => ComplexTypeInfo::as_non_nullable(c),
            _ => self.clone(),
        }
    }

    /// Return `true` if the tokens comprising the type represented by the
    /// receiver could be interpreted as a valid standalone expression.
    /// For example, `A` or `A.b` could be interpreted as type references
    /// or expressions, while `A<T>` only looks like a type reference.
    /// Dart `couldBeExpression`.
    pub fn could_be_expression(&self) -> bool {
        match self {
            TypeInfo::NoType => false,
            TypeInfo::VoidType => false,
            TypeInfo::SimpleType | TypeInfo::SimpleNullableType => true,
            TypeInfo::PrefixedType => true,
            TypeInfo::SimpleTypeWith1Argument(_) | TypeInfo::SimpleNullableTypeWith1Argument => {
                false
            }
            TypeInfo::Complex(c) => c.0.could_be_expression(),
        }
    }

    /// Return true if the receiver has a trailing `?`.
    /// Dart `isNullable`.
    pub fn is_nullable(&self) -> bool {
        match self {
            TypeInfo::SimpleNullableType | TypeInfo::SimpleNullableTypeWith1Argument => true,
            TypeInfo::Complex(c) => c.0.is_nullable(),
            _ => false,
        }
    }

    /// Returns true if the type represents a function type, i.e. something like
    /// void Function foo(int x);
    /// Dart `isFunctionType`.
    pub fn is_function_type(&self) -> bool {
        match self {
            TypeInfo::Complex(c) => c.0.is_function_type(),
            _ => false,
        }
    }

    /// Returns true if the type has type arguments.
    /// Dart `hasTypeArguments`.
    pub fn has_type_arguments(&self) -> bool {
        match self {
            TypeInfo::SimpleTypeWith1Argument(_) | TypeInfo::SimpleNullableTypeWith1Argument => {
                true
            }
            TypeInfo::Complex(c) => c.0.has_type_arguments(),
            _ => false,
        }
    }

    /// Dart (line 43): `bool get recovered => false;` (a field in
    /// `ComplexTypeInfo`).
    pub fn recovered(&self) -> bool {
        match self {
            TypeInfo::Complex(c) => c.0.recovered.get(),
            _ => false,
        }
    }

    /// Dart `typeInfo is ComplexTypeInfo && typeInfo.isRecordType`.
    pub fn is_record_type(&self) -> bool {
        match self {
            TypeInfo::Complex(c) => c.0.is_record_type,
            _ => false,
        }
    }

    /// Dart `typeInfo is ComplexTypeInfo`.
    pub fn is_complex(&self) -> bool {
        matches!(self, TypeInfo::Complex(_))
    }

    /// Dart `typeInfo as ComplexTypeInfo` (`None` if it is not one).
    pub fn as_complex(&self) -> Option<&ComplexTypeInfo> {
        match self {
            TypeInfo::Complex(c) => Some(&c.0),
            _ => None,
        }
    }

    /// Call this function when the token after [token] must be a type (not void).
    /// This function will call the appropriate event methods on the [Parser]'s
    /// listener to handle the type, inserting a synthetic type reference if
    /// necessary. This may modify the token stream when parsing `>>` or `>>>`
    /// or `>>>=` in valid code or during recovery.
    /// Dart `ensureTypeNotVoid`.
    pub fn ensure_type_not_void<L: Listener>(
        &self,
        token: TokenId,
        parser: &mut Parser<L>,
    ) -> TokenId {
        match self {
            TypeInfo::NoType => NoType::ensure_type_not_void(token, parser),
            TypeInfo::VoidType => VoidType::ensure_type_not_void(token, parser),
            _ => self.parse_type(token, parser),
        }
    }

    /// Call this function when the token after [token] must be a type or void.
    /// This function will call the appropriate event methods on the [Parser]'s
    /// listener to handle the type, inserting a synthetic type reference if
    /// necessary. This may modify the token stream when parsing `>>` or `>>>`
    /// or `>>>=` in valid code or during recovery.
    /// Dart `ensureTypeOrVoid`.
    pub fn ensure_type_or_void<L: Listener>(
        &self,
        token: TokenId,
        parser: &mut Parser<L>,
    ) -> TokenId {
        match self {
            TypeInfo::NoType => NoType::ensure_type_or_void(token, parser),
            TypeInfo::VoidType => VoidType::ensure_type_or_void(token, parser),
            _ => self.parse_type(token, parser),
        }
    }

    /// Call this function to parse an optional type (not void) after [token].
    /// This function will call the appropriate event methods on the [Parser]'s
    /// listener to handle the type. This may modify the token stream
    /// when parsing `>>` or `>>>` or `>>>=`  in valid code or during recovery.
    /// Dart `parseTypeNotVoid`.
    pub fn parse_type_not_void<L: Listener>(
        &self,
        token: TokenId,
        parser: &mut Parser<L>,
    ) -> TokenId {
        match self {
            TypeInfo::NoType => NoType::parse_type_not_void(token, parser),
            TypeInfo::VoidType => VoidType::parse_type_not_void(token, parser),
            _ => self.parse_type(token, parser),
        }
    }

    /// Call this function to parse an optional type or void after [token].
    /// This function will call the appropriate event methods on the [Parser]'s
    /// listener to handle the type. This may modify the token stream
    /// when parsing `>>` or `>>>` or `>>>=` in valid code or during recovery.
    /// Dart `parseType`.
    pub fn parse_type<L: Listener>(&self, token: TokenId, parser: &mut Parser<L>) -> TokenId {
        match self {
            TypeInfo::NoType => NoType::parse_type(token, parser),
            TypeInfo::VoidType => VoidType::parse_type(token, parser),
            TypeInfo::SimpleType => SimpleType::parse_type(false, token, parser),
            TypeInfo::SimpleNullableType => SimpleType::parse_type(true, token, parser),
            TypeInfo::PrefixedType => PrefixedType::parse_type(token, parser),
            TypeInfo::SimpleTypeWith1Argument(type_arg) => {
                SimpleTypeWith1Argument::parse_type(*type_arg, false, token, parser)
            }
            TypeInfo::SimpleNullableTypeWith1Argument => {
                SimpleTypeWith1Argument::parse_type(SIMPLE_TYPE_ARGUMENT_1, true, token, parser)
            }
            TypeInfo::Complex(c) => c.0.parse_type(token, parser),
        }
    }

    /// Call this function with the [token] before the type to obtain
    /// the last token in the type. If there is no type, then this method
    /// will return [token]. This does not modify the token stream.
    /// Dart `skipType`.
    ///
    /// A token that Dart makes by splitting `>>` etc. is returned as the
    /// split token (see the module documentation and [`Self::skip_type_mut`]).
    pub fn skip_type(&self, tokens: &Tokens, token: TokenId) -> TokenId {
        let mut overlay = Overlay::new(tokens);
        let result = self.skip_type_g(&mut overlay, token);
        overlay.real(result)
    }

    /// Dart `skipType`, exact: tokens made by splitting `>>` etc. are added
    /// to [tokens] (not linked into the stream).
    pub fn skip_type_mut(&self, tokens: &mut Tokens, token: TokenId) -> TokenId {
        self.skip_type_g(tokens, token)
    }

    /// Dart `skipType` for any [`TokenView`].
    pub(crate) fn skip_type_g<V: TokenView + ?Sized>(
        &self,
        tokens: &mut V,
        token: TokenId,
    ) -> TokenId {
        match self {
            TypeInfo::NoType => NoType::skip_type(token),
            TypeInfo::VoidType => VoidType::skip_type(tokens, token),
            TypeInfo::SimpleType => SimpleType::skip_type(false, tokens, token),
            TypeInfo::SimpleNullableType => SimpleType::skip_type(true, tokens, token),
            TypeInfo::PrefixedType => PrefixedType::skip_type(tokens, token),
            TypeInfo::SimpleTypeWith1Argument(type_arg) => {
                SimpleTypeWith1Argument::skip_type(*type_arg, false, tokens, token)
            }
            TypeInfo::SimpleNullableTypeWith1Argument => {
                SimpleTypeWith1Argument::skip_type(SIMPLE_TYPE_ARGUMENT_1, true, tokens, token)
            }
            TypeInfo::Complex(c) => c.0.skip_type(token),
        }
    }
}

/// [TypeParamOrArgInfo] provides information collected by
/// [computeTypeParamOrArg] about a particular group of type arguments
/// or type parameters.
/// Dart (line 80): `abstract class TypeParamOrArgInfo`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeParamOrArgInfo {
    /// Dart `NoTypeParamOrArg` ([`NO_TYPE_PARAM_OR_ARG`]).
    NoTypeParamOrArg,
    /// Dart `SimpleTypeArgument1` ([`SIMPLE_TYPE_ARGUMENT_1`]).
    SimpleTypeArgument1,
    /// Dart `SimpleTypeArgument1GtEq` ([`SIMPLE_TYPE_ARGUMENT_1_GT_EQ`]).
    SimpleTypeArgument1GtEq,
    /// Dart `SimpleTypeArgument1GtGt` ([`SIMPLE_TYPE_ARGUMENT_1_GT_GT`]).
    SimpleTypeArgument1GtGt,
    /// Dart `ComplexTypeParamOrArgInfo`.
    Complex(ComplexTypeParamOrArgInfo),
}

/// [NoTypeParamOrArg] is a specialized [TypeParamOrArgInfo] returned by
/// [computeTypeParamOrArg] when no type parameters or arguments are found.
/// Dart `noTypeParamOrArg`.
pub const NO_TYPE_PARAM_OR_ARG: TypeParamOrArgInfo = TypeParamOrArgInfo::NoTypeParamOrArg;

impl TypeParamOrArgInfo {
    /// The closer of a simple type argument (`SimpleTypeArgument1*`).
    fn simple_closer(&self) -> Option<TokenType> {
        match self {
            TypeParamOrArgInfo::SimpleTypeArgument1 => Some(TokenType::GT),
            TypeParamOrArgInfo::SimpleTypeArgument1GtEq => Some(TokenType::GT_EQ),
            TypeParamOrArgInfo::SimpleTypeArgument1GtGt => Some(TokenType::GT_GT),
            _ => None,
        }
    }

    /// Return `true` if the receiver represents a single type argument
    /// Dart (line 84): `bool get isSimpleTypeArgument => false;`
    pub fn is_simple_type_argument(&self) -> bool {
        self.simple_closer().is_some()
    }

    /// Return the number of type arguments
    /// Dart `typeArgumentCount`.
    pub fn type_argument_count(&self) -> i32 {
        match self {
            TypeParamOrArgInfo::NoTypeParamOrArg => 0,
            TypeParamOrArgInfo::SimpleTypeArgument1
            | TypeParamOrArgInfo::SimpleTypeArgument1GtEq
            | TypeParamOrArgInfo::SimpleTypeArgument1GtGt => 1,
            TypeParamOrArgInfo::Complex(c) => c.type_argument_count,
        }
    }

    /// Dart (line 89): `bool get recovered => false;` (a field in
    /// `ComplexTypeParamOrArgInfo`).
    pub fn recovered(&self) -> bool {
        match self {
            TypeParamOrArgInfo::Complex(c) => c.recovered,
            _ => false,
        }
    }

    /// Return the simple type associated with this simple type argument
    /// or throw an exception if this is not a simple type argument.
    /// Dart (line 93): `TypeInfo get typeInfo`
    pub fn type_info(&self) -> TypeInfo {
        match self {
            TypeParamOrArgInfo::SimpleTypeArgument1 => SIMPLE_TYPE_WITH_1_ARGUMENT,
            TypeParamOrArgInfo::SimpleTypeArgument1GtEq => SIMPLE_TYPE_WITH_1_ARGUMENT_GT_EQ,
            TypeParamOrArgInfo::SimpleTypeArgument1GtGt => SIMPLE_TYPE_WITH_1_ARGUMENT_GT_GT,
            _ => panic!("Internal error: {self:?} is not a SimpleTypeArgument."),
        }
    }

    /// Call this function to parse optional type arguments after [token].
    /// This function will call the appropriate event methods on the [Parser]'s
    /// listener to handle the arguments. This may modify the token stream
    /// when parsing `>>` or `>>>` or `>>>=` in valid code or during recovery.
    /// Dart `parseArguments`.
    pub fn parse_arguments<L: Listener>(&self, token: TokenId, parser: &mut Parser<L>) -> TokenId {
        match self {
            TypeParamOrArgInfo::NoTypeParamOrArg => {
                NoTypeParamOrArg::parse_arguments(token, parser)
            }
            TypeParamOrArgInfo::Complex(c) => c.parse_arguments(token, parser),
            _ => SimpleTypeArgument1::parse_arguments(self.simple_closer().unwrap(), token, parser),
        }
    }

    /// Call this function to parse optional type parameters
    /// (also known as type variables) after [token].
    /// This function will call the appropriate event methods on the [Parser]'s
    /// listener to handle the parameters. This may modify the token stream
    /// when parsing `>>` or `>>>` or `>>>=` in valid code or during recovery.
    /// Dart `parseVariables`.
    pub fn parse_variables<L: Listener>(&self, token: TokenId, parser: &mut Parser<L>) -> TokenId {
        match self {
            TypeParamOrArgInfo::NoTypeParamOrArg => {
                NoTypeParamOrArg::parse_variables(token, parser)
            }
            TypeParamOrArgInfo::Complex(c) => c.parse_variables(token, parser),
            _ => SimpleTypeArgument1::parse_variables(self.simple_closer().unwrap(), token, parser),
        }
    }

    /// Call this function with the [token] before the type var to obtain
    /// the last token in the type var. If there is no type var, then this method
    /// will return [token]. This does not modify the token stream.
    /// Dart `skip`.
    ///
    /// A token that Dart makes by splitting `>>` etc. is returned as the
    /// split token (see the module documentation and [`Self::skip_mut`]).
    pub fn skip(&self, tokens: &Tokens, token: TokenId) -> TokenId {
        let mut overlay = Overlay::new(tokens);
        let result = self.skip_g(&mut overlay, token);
        overlay.real(result)
    }

    /// Dart `skip`, exact: tokens made by splitting `>>` etc. are added to
    /// [tokens] (not linked into the stream).
    pub fn skip_mut(&self, tokens: &mut Tokens, token: TokenId) -> TokenId {
        self.skip_g(tokens, token)
    }

    /// Dart `skip` for any [`TokenView`].
    pub(crate) fn skip_g<V: TokenView + ?Sized>(&self, tokens: &mut V, token: TokenId) -> TokenId {
        match self {
            TypeParamOrArgInfo::NoTypeParamOrArg => NoTypeParamOrArg::skip(token),
            TypeParamOrArgInfo::Complex(c) => c.skip(token),
            _ => SimpleTypeArgument1::skip(self.simple_closer().unwrap(), tokens, token),
        }
    }
}

/// Dart (line 128): `bool isGeneralizedFunctionType(Token token)`
pub fn is_generalized_function_type(tokens: &Tokens, token: TokenId) -> bool {
    is_generalized_function_type_g(tokens, token)
}

/// [`is_generalized_function_type`] for any [`TokenRead`].
pub(crate) fn is_generalized_function_type_g<R: TokenRead + ?Sized>(
    tokens: &R,
    token: TokenId,
) -> bool {
    tokens.is_a(token, Keyword::FUNCTION)
        && (tokens.is_a(tokens.next(token), TokenType::LT)
            || tokens.is_a(tokens.next(token), TokenType::OPEN_PAREN))
}

/// Dart (line 133): `bool isPossibleRecordType(Token token)`
pub fn is_possible_record_type(tokens: &Tokens, token: TokenId) -> bool {
    is_possible_record_type_g(tokens, token)
}

/// [`is_possible_record_type`] for any [`TokenRead`].
pub(crate) fn is_possible_record_type_g<R: TokenRead + ?Sized>(tokens: &R, token: TokenId) -> bool {
    tokens.is_a(token, TokenType::OPEN_PAREN)
        && tokens
            .end_group(token)
            .is_some_and(|end_group| !tokens.is_synthetic(end_group))
}

/// Dart (line 139): `bool isValidNonRecordTypeReference(Token token)`
pub fn is_valid_non_record_type_reference(tokens: &Tokens, token: TokenId) -> bool {
    is_valid_non_record_type_reference_g(tokens, token)
}

/// [`is_valid_non_record_type_reference`] for any [`TokenRead`].
pub(crate) fn is_valid_non_record_type_reference_g<R: TokenRead + ?Sized>(
    tokens: &R,
    token: TokenId,
) -> bool {
    let ty = tokens.ty(token);
    let kind = ty.kind();
    if IDENTIFIER_TOKEN == kind {
        return true;
    }
    if KEYWORD_TOKEN == kind {
        return ty.is_pseudo()
            || (ty.is_built_in() && tokens.is_a(tokens.next(token), TokenType::PERIOD))
            || ty == Keyword::DYNAMIC
            || ty == Keyword::FUNCTION
            || ty == Keyword::VOID;
    }
    false
}

/// Dart `isOkNextValueInFormalParameter` (identifier_context.dart, the same
/// code as [`crate::identifier_context::is_ok_next_value_in_formal_parameter`])
/// for any [`TokenRead`].
fn is_ok_next_value_in_formal_parameter_g<R: TokenRead + ?Sized>(
    tokens: &R,
    token: TokenId,
) -> bool {
    tokens.is_a(token, TokenType::EQ)
        || tokens.is_a(token, TokenType::COLON)
        || tokens.is_a(token, TokenType::COMMA)
        || tokens.is_a(token, TokenType::CLOSE_PAREN)
        || tokens.is_a(token, TokenType::CLOSE_SQUARE_BRACKET)
        || tokens.is_a(token, TokenType::CLOSE_CURLY_BRACKET)
}

/// Called by the parser to obtain information about a possible type reference
/// that follows [token]. This does not modify the token stream.
///
/// If [inDeclaration] is `true`, then this will more aggressively recover
/// given unbalanced `<` `>` and invalid parameters or arguments.
///
/// Dart `computeType(token, required, [inDeclaration = false,
/// acceptKeywordForSimpleType = false])`.
///
/// Tokens that Dart makes by splitting `>>` etc. are replaced in the result
/// by the split tokens (see the module documentation and
/// [`compute_type_mut`]).
pub fn compute_type(
    tokens: &Tokens,
    token: TokenId,
    required: bool,
    in_declaration: bool,
    accept_keyword_for_simple_type: bool,
) -> TypeInfo {
    let mut overlay = Overlay::new(tokens);
    let result = compute_type_g(
        &mut overlay,
        token,
        required,
        in_declaration,
        accept_keyword_for_simple_type,
    );
    overlay.real_type_info(result)
}

/// Dart `computeType`, exact: tokens made by splitting `>>` etc. are added
/// to [tokens] (not linked into the stream).
pub fn compute_type_mut(
    tokens: &mut Tokens,
    token: TokenId,
    required: bool,
    in_declaration: bool,
    accept_keyword_for_simple_type: bool,
) -> TypeInfo {
    compute_type_g(
        tokens,
        token,
        required,
        in_declaration,
        accept_keyword_for_simple_type,
    )
}

/// Dart (line 158): `TypeInfo computeType(Token token, bool required, [bool
/// inDeclaration = false, bool acceptKeywordForSimpleType = false])`, for
/// any [`TokenView`].
pub(crate) fn compute_type_g<V: TokenView + ?Sized>(
    tokens: &mut V,
    token: TokenId,
    required: bool,
    in_declaration: bool,
    accept_keyword_for_simple_type: bool,
) -> TypeInfo {
    let mut next = tokens.next(token);
    if !is_valid_non_record_type_reference_g(tokens, next)
        && !is_possible_record_type_g(tokens, next)
    {
        // As next is not a valid type reference, this is all recovery.
        if tokens.ty(next).is_built_in() {
            let type_param_or_arg =
                compute_type_param_or_arg_g(tokens, next, in_declaration, false);
            if type_param_or_arg != NO_TYPE_PARAM_OR_ARG {
                // Recovery: built-in `<` ... `>`
                let after = type_param_or_arg.skip_g(tokens, next);
                if required || looks_like_name_g(tokens, tokens.next(after)) {
                    let result = ComplexTypeInfo::new(tokens, token, type_param_or_arg)
                        .compute_builtin_or_var_as_type(tokens, required);
                    result.recovered.set(true);
                    return result.into_type_info();
                }
            } else if required || is_generalized_function_type_g(tokens, tokens.next(next)) {
                let value = tokens.string_value(next);
                if value != Some("get")
                    && value != Some("set")
                    && value != Some("factory")
                    && value != Some("operator")
                    && !(value == Some("typedef") && tokens.is_identifier(tokens.next(next)))
                {
                    let result = ComplexTypeInfo::new(tokens, token, type_param_or_arg)
                        .compute_builtin_or_var_as_type(tokens, required);
                    result.recovered.set(true);
                    return result.into_type_info();
                }
            }
        } else if required {
            // Recovery
            if tokens.is_a(next, TokenType::PERIOD) {
                // Looks like prefixed type missing the prefix
                let type_param_or_arg =
                    compute_type_param_or_arg_g(tokens, next, in_declaration, false);
                let result = ComplexTypeInfo::new(tokens, token, type_param_or_arg)
                    .compute_prefixed_type(tokens, required);
                if let TypeInfo::Complex(c) = &result {
                    c.0.recovered.set(true);
                }
                return result;
            } else if tokens.is_a(next, Keyword::VAR)
                && [TokenType::LT, TokenType::COMMA, TokenType::GT]
                    .contains(&tokens.ty(tokens.next(next)))
            {
                let type_param_or_arg =
                    compute_type_param_or_arg_g(tokens, next, in_declaration, false);
                let result = ComplexTypeInfo::new(tokens, token, type_param_or_arg)
                    .compute_builtin_or_var_as_type(tokens, required);
                result.recovered.set(true);
                return result.into_type_info();
            }
        }
        return NO_TYPE;
    }

    if tokens.is_a(next, Keyword::VOID) {
        next = tokens.next(next);
        if is_generalized_function_type_g(tokens, next) {
            // `void` `Function` ...
            return ComplexTypeInfo::new(tokens, token, NO_TYPE_PARAM_OR_ARG)
                .compute_void_gft(tokens, required);
        }
        // `void`
        return VOID_TYPE;
    }

    if is_generalized_function_type_g(tokens, next) {
        // `Function` ...
        return ComplexTypeInfo::new(tokens, token, NO_TYPE_PARAM_OR_ARG)
            .compute_no_type_gft(tokens, token, required);
    }

    if is_possible_record_type_g(tokens, next) {
        // ([...])
        let after = tokens.next(tokens.end_group(next).unwrap());
        if is_generalized_function_type_g(tokens, after) {
            // ([...]) `Function`
            return ComplexTypeInfo::new(tokens, token, NO_TYPE_PARAM_OR_ARG)
                .compute_record_type_gft(tokens, required);
        }
        if tokens.is_a(after, TokenType::QUESTION)
            && is_generalized_function_type_g(tokens, tokens.next(after))
        {
            // ([...]) `?` `Function`
            return ComplexTypeInfo::new(tokens, token, NO_TYPE_PARAM_OR_ARG)
                .compute_record_type_question_gft(tokens, required);
        }
        return ComplexTypeInfo::new(tokens, token, NO_TYPE_PARAM_OR_ARG)
            .compute_record_type(tokens, required);
    }

    // We've seen an identifier.

    let mut type_param_or_arg = compute_type_param_or_arg_g(tokens, next, in_declaration, false);
    if type_param_or_arg != NO_TYPE_PARAM_OR_ARG {
        if type_param_or_arg.is_simple_type_argument() {
            // We've seen identifier `<` identifier `>`
            let skipped = type_param_or_arg.skip_g(tokens, next);
            next = tokens.next(skipped);
            if tokens.is_a(next, TokenType::QUESTION) {
                next = tokens.next(next);
                if !is_generalized_function_type_g(tokens, next) {
                    if (required || looks_like_name_g(tokens, next))
                        && type_param_or_arg == SIMPLE_TYPE_ARGUMENT_1
                    {
                        // identifier `<` identifier `>` `?` identifier
                        return SIMPLE_NULLABLE_TYPE_WITH_1_ARGUMENT;
                    }
                    // identifier `<` identifier `>` `?` non-identifier
                    return NO_TYPE;
                }
            } else if !is_generalized_function_type_g(tokens, next) {
                if required || looks_like_name_g(tokens, next) {
                    // identifier `<` identifier `>` identifier
                    return type_param_or_arg.type_info();
                }
                // identifier `<` identifier `>` non-identifier
                return NO_TYPE;
            }
        }
        // TODO(danrubel): Consider adding a const for
        // identifier `<` identifier `,` identifier `>`
        // if that proves to be a common case.

        // identifier `<` ... `>`
        return ComplexTypeInfo::new(tokens, token, type_param_or_arg)
            .compute_simple_with_type_arguments(tokens, required);
    }

    debug_assert!(type_param_or_arg == NO_TYPE_PARAM_OR_ARG);
    next = tokens.next(next);

    if tokens.is_a(next, TokenType::PERIOD) {
        next = tokens.next(next);
        if is_valid_non_record_type_reference_g(tokens, next) {
            // We've seen identifier `.` identifier
            type_param_or_arg = compute_type_param_or_arg_g(tokens, next, in_declaration, false);
            next = tokens.next(next);
            if type_param_or_arg == NO_TYPE_PARAM_OR_ARG {
                if tokens.is_a(next, TokenType::QUESTION) {
                    next = tokens.next(next);
                    if !is_generalized_function_type_g(tokens, next) {
                        if required || looks_like_name_g(tokens, next) {
                            // identifier `.` identifier `?` identifier
                            // TODO(danrubel): consider adding PrefixedNullableType
                            // Fall through to build complex type
                        } else {
                            // identifier `.` identifier `?` non-identifier
                            return NO_TYPE;
                        }
                    }
                } else {
                    if !is_generalized_function_type_g(tokens, next) {
                        if required || looks_like_name_g(tokens, next) {
                            // identifier `.` identifier identifier
                            return PREFIXED_TYPE;
                        } else {
                            // identifier `.` identifier non-identifier
                            return NO_TYPE;
                        }
                    }
                }
            }
            // identifier `.` identifier
            return ComplexTypeInfo::new(tokens, token, type_param_or_arg)
                .compute_prefixed_type(tokens, required);
        }
        // identifier `.` non-identifier
        if required {
            let after_period = tokens.next(tokens.next(token));
            type_param_or_arg =
                compute_type_param_or_arg_g(tokens, after_period, in_declaration, false);
            return ComplexTypeInfo::new(tokens, token, type_param_or_arg)
                .compute_prefixed_type(tokens, required);
        }
        return NO_TYPE;
    }

    debug_assert!(type_param_or_arg == NO_TYPE_PARAM_OR_ARG);
    if is_generalized_function_type_g(tokens, next) {
        // identifier `Function`
        return ComplexTypeInfo::new(tokens, token, NO_TYPE_PARAM_OR_ARG)
            .compute_identifier_gft(tokens, required);
    }

    if tokens.is_a(next, TokenType::QUESTION) {
        next = tokens.next(next);
        if is_generalized_function_type_g(tokens, next) {
            // identifier `?` Function `(`
            return ComplexTypeInfo::new(tokens, token, NO_TYPE_PARAM_OR_ARG)
                .compute_identifier_question_gft(tokens, required);
        } else if required || looks_like_name_g(tokens, next) {
            // identifier `?`
            return SIMPLE_NULLABLE_TYPE;
        }
    } else if required
        || looks_like_name_g(tokens, next)
        || (accept_keyword_for_simple_type
            && tokens.is_keyword_or_identifier(next)
            && is_ok_next_value_in_formal_parameter_g(tokens, tokens.next(next)))
    {
        // identifier identifier
        return SIMPLE_TYPE;
    }
    NO_TYPE
}

/// Computes the [TypeInfo] for a variable pattern.
///
/// This is similar to [computeType], but has special logic to account for an
/// ambiguity that arises in patterns due to the fact that `as` can either be
/// an identifier or the operator in a castPattern.
///
/// Dart `computeVariablePatternType(token, [required = false])`.
///
/// Tokens that Dart makes by splitting `>>` etc. are replaced in the result
/// by the split tokens (see the module documentation and
/// [`compute_variable_pattern_type_mut`]).
pub fn compute_variable_pattern_type(tokens: &Tokens, token: TokenId, required: bool) -> TypeInfo {
    let mut overlay = Overlay::new(tokens);
    let result = compute_variable_pattern_type_g(&mut overlay, token, required);
    overlay.real_type_info(result)
}

/// Dart `computeVariablePatternType`, exact: tokens made by splitting `>>`
/// etc. are added to [tokens] (not linked into the stream).
pub fn compute_variable_pattern_type_mut(
    tokens: &mut Tokens,
    token: TokenId,
    required: bool,
) -> TypeInfo {
    compute_variable_pattern_type_g(tokens, token, required)
}

/// Dart (line 392): `TypeInfo computeVariablePatternType(Token token, [bool
/// required = false])`, for any [`TokenView`].
pub(crate) fn compute_variable_pattern_type_g<V: TokenView + ?Sized>(
    tokens: &mut V,
    token: TokenId,
    required: bool,
) -> TypeInfo {
    let type_info = compute_type_g(tokens, token, required, false, false);
    let after_type = type_info.skip_type_g(tokens, token);
    if after_type != token {
        let next = tokens.next(after_type);
        if tokens.is_identifier(next) {
            if tokens.is_a(next, Keyword::AS) || tokens.is_a(next, Keyword::WHEN) {
                // We've seen `TYPE as` or `TYPE when`.  `as` is a built-in identifier
                // and `when` is a pseudo-keyword, so this *could* be a variable
                // pattern.  Or it could be that TYPE should have been parsed as a
                // pattern.  We've decided to resolve the ambiguity by assuming that
                // TYPE was the pattern, and interpret the `when` or `as` as introducing
                // a guard or a cast pattern, respectively (see discussion at
                // https://github.com/dart-lang/sdk/issues/52199).
                return NO_TYPE;
            }
        }
    }
    type_info
}

/// Called by the parser to obtain information about a possible group of type
/// parameters or type arguments that follow [token].
/// This does not modify the token stream.
///
/// If [inDeclaration] is `true`, then this will more aggressively recover
/// given unbalanced `<` `>` and invalid parameters or arguments.
///
/// Dart `computeTypeParamOrArg(token, [inDeclaration = false,
/// allowsVariance = false])`.
///
/// Tokens that Dart makes by splitting `>>` etc. are replaced in the result
/// by the split tokens (see the module documentation and
/// [`compute_type_param_or_arg_mut`]).
pub fn compute_type_param_or_arg(
    tokens: &Tokens,
    token: TokenId,
    in_declaration: bool,
    allows_variance: bool,
) -> TypeParamOrArgInfo {
    let mut overlay = Overlay::new(tokens);
    let result = compute_type_param_or_arg_g(&mut overlay, token, in_declaration, allows_variance);
    overlay.real_type_param_or_arg(result)
}

/// Dart `computeTypeParamOrArg`, exact: tokens made by splitting `>>` etc.
/// are added to [tokens] (not linked into the stream).
pub fn compute_type_param_or_arg_mut(
    tokens: &mut Tokens,
    token: TokenId,
    in_declaration: bool,
    allows_variance: bool,
) -> TypeParamOrArgInfo {
    compute_type_param_or_arg_g(tokens, token, in_declaration, allows_variance)
}

/// Dart (line 420): `TypeParamOrArgInfo computeTypeParamOrArg(Token token,
/// [bool inDeclaration = false, bool allowsVariance = false])`, for any
/// [`TokenView`].
#[inline]
pub(crate) fn compute_type_param_or_arg_g<V: TokenView + ?Sized>(
    tokens: &mut V,
    token: TokenId,
    in_declaration: bool,
    allows_variance: bool,
) -> TypeParamOrArgInfo {
    let begin_group = tokens.next(token);
    if !tokens.is_a(begin_group, TokenType::LT) {
        return NO_TYPE_PARAM_OR_ARG;
    }
    compute_type_param_or_arg_impl(tokens, token, begin_group, in_declaration, allows_variance)
}

/// Dart (line 437): `TypeParamOrArgInfo _computeTypeParamOrArgImpl(Token
/// token, Token beginGroup, bool inDeclaration, bool allowsVariance)`
fn compute_type_param_or_arg_impl<V: TokenView + ?Sized>(
    tokens: &mut V,
    token: TokenId,
    begin_group: TokenId,
    in_declaration: bool,
    allows_variance: bool,
) -> TypeParamOrArgInfo {
    // identifier `<` `void` `>` and `<` `dynamic` `>`
    // are handled by ComplexTypeInfo.
    let next = tokens.next(begin_group);
    if tokens.kind(next) == IDENTIFIER_TOKEN || tokens.ty(next).is_pseudo() {
        if tokens.is_a(tokens.next(next), TokenType::GT) {
            return SIMPLE_TYPE_ARGUMENT_1;
        } else if tokens.is_a(tokens.next(next), TokenType::GT_GT) {
            return SIMPLE_TYPE_ARGUMENT_1_GT_GT;
        } else if tokens.is_a(tokens.next(next), TokenType::GT_EQ) {
            return SIMPLE_TYPE_ARGUMENT_1_GT_EQ;
        }
    } else if tokens.is_a(next, TokenType::OPEN_PAREN) {
        let mut record_type = false;
        if is_possible_record_type_g(tokens, next) {
            let ty = compute_type_g(
                tokens,
                begin_group,
                /* required = */ false,
                false,
                false,
            );
            if let TypeInfo::Complex(c) = &ty {
                if (c.0.is_record_type || c.0.gft_return_type_has_record_type)
                    && !c.0.recovered.get()
                {
                    // Looks like a record type.
                    record_type = true;
                }
            }
        }
        if !record_type {
            return NO_TYPE_PARAM_OR_ARG;
        }
    }

    // TODO(danrubel): Consider adding additional const for common situations.
    ComplexTypeParamOrArgInfo::new(tokens, token, in_declaration, allows_variance).compute(tokens)
}

/// Called by the parser to obtain information about a possible group of type
/// type arguments that follow [token] and that are followed by '('.
/// Returns the type arguments if [token] matches '<' type (',' type)* '>' '(',
/// and otherwise returns [noTypeParamOrArg]. The final '(' is not part of the
/// grammar construct `typeArguments`, but it is required here such that type
/// arguments in generic method invocations can be recognized, and as few as
/// possible other constructs will pass (e.g., 'a < C, D > 3').
///
/// Dart `computeMethodTypeArguments`.
///
/// Tokens that Dart makes by splitting `>>` etc. are replaced in the result
/// by the split tokens (see the module documentation and
/// [`compute_method_type_arguments_mut`]).
pub fn compute_method_type_arguments(tokens: &Tokens, token: TokenId) -> TypeParamOrArgInfo {
    let mut overlay = Overlay::new(tokens);
    let result = compute_method_type_arguments_g(&mut overlay, token);
    overlay.real_type_param_or_arg(result)
}

/// Dart `computeMethodTypeArguments`, exact: tokens made by splitting `>>`
/// etc. are added to [tokens] (not linked into the stream).
pub fn compute_method_type_arguments_mut(
    tokens: &mut Tokens,
    token: TokenId,
) -> TypeParamOrArgInfo {
    compute_method_type_arguments_g(tokens, token)
}

/// Dart (line 485): `TypeParamOrArgInfo computeMethodTypeArguments(Token
/// token)`, for any [`TokenView`].
pub(crate) fn compute_method_type_arguments_g<V: TokenView + ?Sized>(
    tokens: &mut V,
    token: TokenId,
) -> TypeParamOrArgInfo {
    let type_arg = compute_type_param_or_arg_g(tokens, token, false, false);
    let skipped = type_arg.skip_g(tokens, token);
    let after = tokens.next(skipped);
    if may_follow_type_args(tokens.ty(after).index()) && !type_arg.recovered() {
        type_arg
    } else {
        NO_TYPE_PARAM_OR_ARG
    }
}

/// The set of identifiers that are illegal to use as the name of a variable in
/// a variable pattern, or as the name of an identifier in an identifier
/// pattern.
/// Dart (line 496): `illegalPatternIdentifiers`.
pub const ILLEGAL_PATTERN_IDENTIFIERS: &[&str] = &["when", "as"];

/// Indicates whether the given [tokenTypeIndex] is allowed to follow a list of
/// type arguments used as a selector after an expression.
///
/// Get the index from a token via `Token.typeIndex`.
///
/// This is used for disambiguating constructs like `f(a<b,c>(d))` and
/// `f(a<b,c>-d)`.  In the case of `f(a<b,c>(d))`, `true` will be returned,
/// indicating that the `<` and `>` should be interpreted as delimiting type
/// arguments (so one argument is being passed to `f` -- a call to the generic
/// function `a`).  In the case of `f(a<b,c>-d)`, `false` will be returned,
/// indicating that the `<` and `>` should be interpreted as operators (so two
/// arguments are being passed to `f`: `a < b` and `c > -d`).
///
/// Dart (line 521): `bool _mayFollowTypeArgs(int tokenTypeIndex)`
#[inline]
fn may_follow_type_args(token_type_index: u8) -> bool {
    // Table has size 256 to avoid bounds checks as this is called with
    // `Token.typeIndex` which is know to be in [0-255].
    #[rustfmt::skip]
    const TABLE: [bool; 256] = [
        // format hack.
        true, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, true, false, false, false, false, false,
        true, true, false, false, true, true, true, false,
        true, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, true, false, false, false,
        true, false, false, false, false, false, false, false,
        false, true, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
        // format hack.
    ];

    TABLE[token_type_index as usize]
}

/// Dart (line 565): `bool _mayFollowTypeArgs_helper_for_testing(int tokenTypeIndex)`
#[cfg(test)]
fn may_follow_type_args_helper_for_testing(token_type_index: u8) -> bool {
    token_type_index == TokenType::OPEN_PAREN.index()
        || token_type_index == TokenType::PERIOD.index()
        || token_type_index == TokenType::EQ_EQ.index()
        || token_type_index == TokenType::BANG_EQ.index()
        || token_type_index == TokenType::CLOSE_PAREN.index()
        || token_type_index == TokenType::CLOSE_SQUARE_BRACKET.index()
        || token_type_index == TokenType::CLOSE_CURLY_BRACKET.index()
        || token_type_index == TokenType::SEMICOLON.index()
        || token_type_index == TokenType::COLON.index()
        || token_type_index == TokenType::COMMA.index()
        || token_type_index == TokenType::EOF.index()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser_impl::synthetic_previous_token;

    /// Scans [source] and returns the arena and the token before the first
    /// token.
    fn scan(source: &str) -> (Tokens, TokenId) {
        let mut result = dartr_syntax::scan_string(source, None, false, None);
        let before = synthetic_previous_token(&mut result.tokens, result.first);
        (result.tokens, before)
    }

    /// The token after the type, and its lexeme, for both token views.
    fn after_type(source: &str) -> (String, String) {
        let (mut tokens, before) = scan(source);
        let info = compute_type(&tokens, before, true, false, false);
        let shared = tokens
            .lexeme(tokens.next(info.skip_type(&tokens, before)))
            .to_string();
        let info = compute_type_mut(&mut tokens, before, true, false, false);
        let end = info.skip_type_mut(&mut tokens, before);
        let exact = tokens.lexeme(tokens.next(end)).to_string();
        (shared, exact)
    }

    #[test]
    fn nested_type_arguments_end_after_split_closer() {
        for source in [
            "List<List<int>> x;",
            "Map<String, List<int>> x;",
            "A<B<C<D>>> x;",
            "List<List<int>>? x;",
        ] {
            let (shared, exact) = after_type(source);
            assert_eq!(exact, "x", "{source}");
            assert_eq!(shared, "x", "{source}");
        }
        // The type ends at the first part of a split token: Dart (and the
        // exact view) continue with the `=` part. The `&Tokens` view returns
        // the split token `>>>=`, so its `next` is `x` (the known difference
        // of the `&Tokens` functions).
        let (shared, exact) = after_type("A<B<C<D>>>= x;");
        assert_eq!(exact, "=");
        assert_eq!(shared, "x");
    }

    #[test]
    fn constants_keep_identity() {
        let (tokens, before) = scan("List<int> x;");
        assert_eq!(
            compute_type(&tokens, before, false, false, false),
            SIMPLE_TYPE_WITH_1_ARGUMENT
        );
        let (tokens, before) = scan("List<int>> x;");
        assert_eq!(
            compute_type(&tokens, before, true, false, false),
            SIMPLE_TYPE_WITH_1_ARGUMENT_GT_GT
        );
        let (tokens, before) = scan("a.B x;");
        assert_eq!(
            compute_type(&tokens, before, false, false, false),
            PREFIXED_TYPE
        );
        let (tokens, before) = scan("int? x;");
        assert_eq!(
            compute_type(&tokens, before, false, false, false),
            SIMPLE_NULLABLE_TYPE
        );
        let (tokens, before) = scan("void Function() x;");
        let info = compute_type(&tokens, before, false, false, false);
        assert!(info.is_complex() && info.is_function_type());
        assert_eq!(info, info.clone());
        assert_ne!(info, compute_type(&tokens, before, false, false, false));
        assert!(matches!(info.as_non_nullable(), TypeInfo::Complex(_)));
        assert!(!matches!(NO_TYPE, VOID_TYPE));
    }

    #[test]
    fn method_type_arguments_need_a_following_paren() {
        let (tokens, before) = scan("f<int, String>(x)");
        let first = tokens.next(before);
        let info = compute_method_type_arguments(&tokens, first);
        assert_eq!(info.type_argument_count(), 2);
        let (tokens, before) = scan("a<b, c> -d");
        let first = tokens.next(before);
        assert_eq!(
            compute_method_type_arguments(&tokens, first),
            NO_TYPE_PARAM_OR_ARG
        );
    }

    /// The Dart `DartDocTest` of `_mayFollowTypeArgs`: the table matches the
    /// token type indexes of `dartr_syntax`.
    #[test]
    fn may_follow_type_args_table_matches_token_types() {
        for i in 0..=255u8 {
            assert_eq!(
                may_follow_type_args(i),
                may_follow_type_args_helper_for_testing(i),
                "index {i}"
            );
        }
    }
}
