// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/type_info.dart

//! STUB: the API that the parser uses. The port replaces the `todo!()`s and
//! may add variants and fields, but keeps these names and signatures.

#![allow(unused_variables)]

use dartr_syntax::{TokenId, Tokens};

use crate::listener::Listener;
use crate::parser_impl::Parser;

/// Dart `TypeInfo`: information about a type reference, computed by
/// [`compute_type`]. Dart compares instances with the constants
/// (`identical(typeInfo, noType)`); here `==` with [`NO_TYPE`] etc.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeInfo {
    NoType,
    VoidType,
}

/// Dart `noType`.
pub const NO_TYPE: TypeInfo = TypeInfo::NoType;
/// Dart `voidType`.
pub const VOID_TYPE: TypeInfo = TypeInfo::VoidType;

impl TypeInfo {
    /// Dart `asNonNullable`.
    pub fn as_non_nullable(&self) -> TypeInfo {
        todo!()
    }
    /// Dart `couldBeExpression`.
    pub fn could_be_expression(&self) -> bool {
        todo!()
    }
    /// Dart `isNullable`.
    pub fn is_nullable(&self) -> bool {
        todo!()
    }
    /// Dart `isFunctionType`.
    pub fn is_function_type(&self) -> bool {
        todo!()
    }
    /// Dart `hasTypeArguments`.
    pub fn has_type_arguments(&self) -> bool {
        todo!()
    }
    /// Dart `recovered`.
    pub fn recovered(&self) -> bool {
        todo!()
    }
    /// Dart `typeInfo is ComplexTypeInfo && typeInfo.isRecordType`.
    pub fn is_record_type(&self) -> bool {
        todo!()
    }
    /// Dart `typeInfo is ComplexTypeInfo`.
    pub fn is_complex(&self) -> bool {
        todo!()
    }
    /// Dart `ensureTypeNotVoid`.
    pub fn ensure_type_not_void<L: Listener>(&self, token: TokenId, parser: &mut Parser<L>) -> TokenId {
        todo!()
    }
    /// Dart `ensureTypeOrVoid`.
    pub fn ensure_type_or_void<L: Listener>(&self, token: TokenId, parser: &mut Parser<L>) -> TokenId {
        todo!()
    }
    /// Dart `parseTypeNotVoid`.
    pub fn parse_type_not_void<L: Listener>(&self, token: TokenId, parser: &mut Parser<L>) -> TokenId {
        todo!()
    }
    /// Dart `parseType`.
    pub fn parse_type<L: Listener>(&self, token: TokenId, parser: &mut Parser<L>) -> TokenId {
        todo!()
    }
    /// Dart `skipType`.
    pub fn skip_type(&self, tokens: &Tokens, token: TokenId) -> TokenId {
        todo!()
    }
}

/// Dart `TypeParamOrArgInfo`: information about type parameters or type
/// arguments, computed by [`compute_type_param_or_arg`]. Small and `Copy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeParamOrArgInfo {
    NoTypeParamOrArg,
}

/// Dart `noTypeParamOrArg`.
pub const NO_TYPE_PARAM_OR_ARG: TypeParamOrArgInfo = TypeParamOrArgInfo::NoTypeParamOrArg;

impl TypeParamOrArgInfo {
    /// Dart `isSimpleTypeArgument`.
    pub fn is_simple_type_argument(&self) -> bool {
        todo!()
    }
    /// Dart `typeArgumentCount`.
    pub fn type_argument_count(&self) -> i32 {
        todo!()
    }
    /// Dart `recovered`.
    pub fn recovered(&self) -> bool {
        todo!()
    }
    /// Dart `typeInfo` (only for simple type arguments).
    pub fn type_info(&self) -> TypeInfo {
        todo!()
    }
    /// Dart `parseArguments`.
    pub fn parse_arguments<L: Listener>(&self, token: TokenId, parser: &mut Parser<L>) -> TokenId {
        todo!()
    }
    /// Dart `parseVariables`.
    pub fn parse_variables<L: Listener>(&self, token: TokenId, parser: &mut Parser<L>) -> TokenId {
        todo!()
    }
    /// Dart `skip`.
    pub fn skip(&self, tokens: &Tokens, token: TokenId) -> TokenId {
        todo!()
    }
}

/// Dart `isGeneralizedFunctionType`.
pub fn is_generalized_function_type(tokens: &Tokens, token: TokenId) -> bool {
    todo!()
}

/// Dart `isPossibleRecordType`.
pub fn is_possible_record_type(tokens: &Tokens, token: TokenId) -> bool {
    todo!()
}

/// Dart `isValidNonRecordTypeReference`.
pub fn is_valid_non_record_type_reference(tokens: &Tokens, token: TokenId) -> bool {
    todo!()
}

/// Dart `computeType(token, required, [inDeclaration = false,
/// acceptKeywordForSimpleType = false])`.
pub fn compute_type(
    tokens: &Tokens,
    token: TokenId,
    required: bool,
    in_declaration: bool,
    accept_keyword_for_simple_type: bool,
) -> TypeInfo {
    todo!()
}

/// Dart `computeVariablePatternType(token, [required = false])`.
pub fn compute_variable_pattern_type(tokens: &Tokens, token: TokenId, required: bool) -> TypeInfo {
    todo!()
}

/// Dart `computeTypeParamOrArg(token, [inDeclaration = false,
/// allowsVariance = false])`.
pub fn compute_type_param_or_arg(
    tokens: &Tokens,
    token: TokenId,
    in_declaration: bool,
    allows_variance: bool,
) -> TypeParamOrArgInfo {
    todo!()
}

/// Dart `computeMethodTypeArguments`.
pub fn compute_method_type_arguments(tokens: &Tokens, token: TokenId) -> TypeParamOrArgInfo {
    todo!()
}

/// Dart `illegalPatternIdentifiers`.
pub const ILLEGAL_PATTERN_IDENTIFIERS: &[&str] = &["when", "as"];
