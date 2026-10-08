// Dart source: pkg/_fe_analyzer_shared/test/mini_types.dart

//! `ParseError`, `_TypeParser`, and the `_PreType` classes.

use std::fmt;

use indexmap::IndexMap;

use super::name::Name;
use super::registry::{SpecialTypeName, TypeNameInfo, TypeParameter, TypeRegistry};
use super::types::{
    DynamicType, FunctionType, FutureOrType, InvalidType, NamedFunctionParameter, NamedType,
    NeverType, NullType, PrimaryType, RecordType, Type, TypeParameterType, UnknownType, VoidType,
};

/// Exception thrown if a type fails to parse properly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// `message`.
    pub message: String,
}

impl ParseError {
    fn new(message: String) -> ParseError {
        ParseError { message }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

impl Type {
    /// The Dart factory `Type(typeStr)`: parses [type_str].
    ///
    /// Panics with the [`ParseError`] message if the string does not parse
    /// (Dart throws `ParseError`), and if a name is not registered in the
    /// [`TypeRegistry`] (Dart throws `StateError`).
    #[allow(clippy::new_ret_no_self)]
    pub fn new(type_str: &str) -> Type {
        match Type::try_parse(type_str) {
            Ok(t) => t,
            Err(e) => panic!("ParseError: {e}"),
        }
    }

    /// Like [`Type::new`], but returns the [`ParseError`]. Still panics if a
    /// name is not registered in the [`TypeRegistry`].
    pub fn try_parse(type_str: &str) -> Result<Type, ParseError> {
        TypeParser::parse(type_str)
    }
}

/// The scope of type formals: Dart `Map<String, TypeParameter>`.
type TypeFormalScope = IndexMap<Name, TypeParameter>;

/// Representation of a [`Type`] that has been parsed but hasn't had meaning
/// assigned to its identifiers yet.
enum PreType {
    /// `_PreFunctionType`.
    Function(PreFunctionType),
    /// `_PrePrimaryType`.
    Primary(PrePrimaryType),
    /// `_PrePromotedType`.
    Promoted(PrePromotedType),
    /// `_PreRecordType`.
    Record(PreRecordType),
    /// `_PreTypeWithNullability`.
    WithNullability(PreTypeWithNullability),
    /// `_PreUnknownType`.
    Unknown,
}

impl PreType {
    /// Translates `self` into a [`Type`].
    ///
    /// The meaning of identifiers in `self` is determined by looking them up
    /// first in [type_formal_scope], and then, if they are not found, in the
    /// [`TypeRegistry`].
    fn materialize(&self, type_formal_scope: &TypeFormalScope) -> Result<Type, ParseError> {
        match self {
            PreType::Function(t) => t.materialize(type_formal_scope),
            PreType::Primary(t) => t.materialize(type_formal_scope),
            PreType::Promoted(t) => t.materialize(type_formal_scope),
            PreType::Record(t) => t.materialize(type_formal_scope),
            PreType::WithNullability(t) => Ok(t
                .inner
                .materialize(type_formal_scope)?
                .as_question_type(t.is_question_type)),
            PreType::Unknown => Ok(UnknownType::new(false).into_type()),
        }
    }
}

/// Representation of a [`FunctionType`] that has been parsed but hasn't had
/// meaning assigned to its identifiers yet.
struct PreFunctionType {
    return_type: Box<PreType>,
    type_formals: Vec<PreTypeFormal>,
    positional_parameter_types: Vec<PreType>,
    required_positional_parameter_count: usize,
    named_parameters: Vec<PreNamedFunctionParameter>,
}

impl PreFunctionType {
    fn materialize(&self, type_formal_scope: &TypeFormalScope) -> Result<Type, ParseError> {
        let materialized_type_formals: Vec<TypeParameter>;
        let mut scope = std::borrow::Cow::Borrowed(type_formal_scope);
        if !self.type_formals.is_empty() {
            let mut formals = Vec::new();
            let mut new_scope = type_formal_scope.clone();
            for type_formal in &self.type_formals {
                let materialized_type_formal =
                    TypeParameter::new_unregistered(type_formal.name.as_str());
                formals.push(materialized_type_formal);
                new_scope.insert(type_formal.name, materialized_type_formal);
            }
            for (i, type_formal) in self.type_formals.iter().enumerate() {
                if let Some(bound) = &type_formal.bound {
                    formals[i].set_explicit_bound(Some(bound.materialize(&new_scope)?));
                }
            }
            scope = std::borrow::Cow::Owned(new_scope);
            materialized_type_formals = formals;
        } else {
            materialized_type_formals = Vec::new();
        }
        let positional_parameters = self
            .positional_parameter_types
            .iter()
            .map(|p| p.materialize(&scope))
            .collect::<Result<Vec<_>, _>>()?;
        let named_parameters = self
            .named_parameters
            .iter()
            .map(|p| {
                Ok(NamedFunctionParameter::new(
                    p.is_required,
                    p.name,
                    p.type_.materialize(&scope)?,
                ))
            })
            .collect::<Result<Vec<_>, ParseError>>()?;
        Ok(
            FunctionType::new(self.return_type.materialize(&scope)?, positional_parameters)
                .with_type_parameters(materialized_type_formals)
                .with_required_positional_parameter_count(self.required_positional_parameter_count)
                .with_named_parameters(named_parameters)
                .into_type(),
        )
    }
}

/// Representation of a named function parameter in a [`PreFunctionType`].
struct PreNamedFunctionParameter {
    name: Name,
    type_: PreType,
    is_required: bool,
}

/// Representation of a named component of a [`PreRecordType`].
struct PreNamedType {
    name: Name,
    type_: PreType,
}

/// Representation of a [`PrimaryType`] or [`TypeParameterType`] that has
/// been parsed but hasn't had meaning assigned to its identifiers yet.
struct PrePrimaryType {
    type_name: Name,
    type_args: Vec<PreType>,
}

impl PrePrimaryType {
    fn materialize(&self, type_formal_scope: &TypeFormalScope) -> Result<Type, ParseError> {
        let name_info = match type_formal_scope.get(&self.type_name) {
            Some(type_parameter) => TypeNameInfo::TypeParameter(*type_parameter),
            None => TypeRegistry::lookup(self.type_name.as_str()),
        };
        let no_type_args = |message: &str| {
            if self.type_args.is_empty() {
                Ok(())
            } else {
                Err(ParseError::new(message.to_owned()))
            }
        };
        match name_info {
            TypeNameInfo::TypeParameter(type_parameter) => {
                no_type_args("Type parameter types do not accept type arguments")?;
                Ok(TypeParameterType::new(type_parameter).into_type())
            }
            TypeNameInfo::Interface(name) => {
                let args = self
                    .type_args
                    .iter()
                    .map(|a| a.materialize(type_formal_scope))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(PrimaryType::new(name, args).into_type())
            }
            TypeNameInfo::Special(special) => match special {
                SpecialTypeName::Dynamic => {
                    no_type_args("`dynamic` does not accept type arguments")?;
                    Ok(DynamicType::instance())
                }
                SpecialTypeName::Error => {
                    no_type_args("`error` does not accept type arguments")?;
                    Ok(InvalidType::instance())
                }
                SpecialTypeName::FutureOr => {
                    if self.type_args.len() != 1 {
                        return Err(ParseError::new(
                            "`FutureOr` requires exactly one type argument".to_owned(),
                        ));
                    }
                    Ok(FutureOrType::new(
                        self.type_args[0].materialize(type_formal_scope)?,
                        false,
                    ))
                }
                SpecialTypeName::Never => {
                    no_type_args("`Never` does not accept type arguments")?;
                    Ok(NeverType::instance())
                }
                SpecialTypeName::Null => {
                    no_type_args("`Null` does not accept type arguments")?;
                    Ok(NullType::instance())
                }
                SpecialTypeName::Void => {
                    no_type_args("`void` does not accept type arguments")?;
                    Ok(VoidType::instance())
                }
            },
        }
    }
}

/// Representation of a promoted [`TypeParameterType`] that has been parsed
/// but hasn't had meaning assigned to its identifiers yet.
struct PrePromotedType {
    inner: Box<PreType>,
    promotion: Box<PreType>,
}

impl PrePromotedType {
    fn materialize(&self, type_formal_scope: &TypeFormalScope) -> Result<Type, ParseError> {
        let type_ = self.inner.materialize(type_formal_scope)?;
        match type_.as_type_parameter_type() {
            Some(TypeParameterType {
                type_parameter,
                promotion: None,
                ..
            }) => Ok(TypeParameterType::new(type_parameter)
                .with_promotion(Some(self.promotion.materialize(type_formal_scope)?))
                .into_type()),
            _ => Err(ParseError::new(
                "The type to the left of & must be an unpromoted type parameter".to_owned(),
            )),
        }
    }
}

/// Representation of a [`RecordType`] that has been parsed but hasn't had
/// meaning assigned to its identifiers yet.
struct PreRecordType {
    positional_types: Vec<PreType>,
    named_types: Vec<PreNamedType>,
}

impl PreRecordType {
    fn materialize(&self, type_formal_scope: &TypeFormalScope) -> Result<Type, ParseError> {
        let positional_types = self
            .positional_types
            .iter()
            .map(|t| t.materialize(type_formal_scope))
            .collect::<Result<Vec<_>, _>>()?;
        let named_types = self
            .named_types
            .iter()
            .map(|n| {
                Ok(NamedType::new(
                    n.name,
                    n.type_.materialize(type_formal_scope)?,
                ))
            })
            .collect::<Result<Vec<_>, ParseError>>()?;
        Ok(RecordType::new(positional_types, named_types).into_type())
    }
}

/// Representation of a formal parameter of a function type that has been
/// parsed but hasn't had meaning assigned to its identifiers yet.
struct PreTypeFormal {
    name: Name,
    bound: Option<PreType>,
}

/// Representation of a [`Type`] with a nullability suffix that has been
/// parsed but hasn't had meaning assigned to its identifiers yet.
struct PreTypeWithNullability {
    inner: Box<PreType>,
    is_question_type: bool,
}

/// `_TypeParser`.
struct TypeParser {
    type_str: String,
    tokens: Vec<String>,
    i: usize,
}

/// `_identifierRegexp.matchAsPrefix(s) != null`: whether [s] starts with an
/// identifier (`[_a-zA-Z][_a-zA-Z0-9]*`).
fn starts_with_identifier(s: &str) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
}

type ParseResult<T> = Result<T, ParseError>;

impl TypeParser {
    fn current_token(&self) -> &str {
        self.tokens.get(self.i).map_or("<END>", |t| t.as_str())
    }

    fn next(&mut self) {
        self.i += 1;
    }

    fn parse_failure<T>(&self, message: &str) -> ParseResult<T> {
        Err(ParseError::new(format!(
            "Error parsing type `{}` at token {}: {message}",
            self.type_str,
            self.current_token()
        )))
    }

    fn parse_named_function_parameters(&mut self) -> ParseResult<Vec<PreNamedFunctionParameter>> {
        assert_eq!(self.current_token(), "{");
        self.next();
        let mut named_parameters = Vec::new();
        loop {
            let is_required = self.current_token() == "required";
            if is_required {
                self.next();
            }
            let type_ = self.parse_type()?;
            let name = self.current_token().to_owned();
            if !starts_with_identifier(&name) {
                return self.parse_failure("Expected an identifier");
            }
            named_parameters.push(PreNamedFunctionParameter {
                name: Name::new(&name),
                type_,
                is_required,
            });
            self.next();
            if self.current_token() == "," {
                self.next();
                continue;
            }
            if self.current_token() == "}" {
                break;
            }
            return self.parse_failure("Expected `}` or `,`");
        }
        self.next();
        named_parameters.sort_by_key(|a| a.name);
        Ok(named_parameters)
    }

    fn parse_optional_function_parameters(
        &mut self,
        positional_parameter_types: &mut Vec<PreType>,
    ) -> ParseResult<()> {
        assert_eq!(self.current_token(), "[");
        self.next();
        loop {
            let type_ = self.parse_type()?;
            positional_parameter_types.push(type_);
            if self.current_token() == "," {
                self.next();
                continue;
            }
            if self.current_token() == "]" {
                break;
            }
            return self.parse_failure("Expected `]` or `,`");
        }
        self.next();
        Ok(())
    }

    fn parse_record_type_named_fields(&mut self) -> ParseResult<Vec<PreNamedType>> {
        assert_eq!(self.current_token(), "{");
        self.next();
        let mut named_types = Vec::new();
        while self.current_token() != "}" {
            let type_ = self.parse_type()?;
            let name = self.current_token().to_owned();
            if !starts_with_identifier(&name) {
                return self.parse_failure("Expected an identifier");
            }
            named_types.push(PreNamedType {
                name: Name::new(&name),
                type_,
            });
            self.next();
            if self.current_token() == "," {
                self.next();
                continue;
            }
            if self.current_token() == "}" {
                break;
            }
            return self.parse_failure("Expected `}` or `,`");
        }
        if named_types.is_empty() {
            return self.parse_failure("Must have at least one named type between {}");
        }
        self.next();
        named_types.sort_by_key(|a| a.name);
        Ok(named_types)
    }

    fn parse_record_type_rest(
        &mut self,
        mut positional_types: Vec<PreType>,
    ) -> ParseResult<PreType> {
        let mut named_types: Option<Vec<PreNamedType>> = None;
        while self.current_token() != ")" {
            if self.current_token() == "{" {
                named_types = Some(self.parse_record_type_named_fields()?);
                if self.current_token() != ")" {
                    return self.parse_failure("Expected `)`");
                }
                break;
            }
            positional_types.push(self.parse_type()?);
            if self.current_token() == "," {
                self.next();
                continue;
            }
            if self.current_token() == ")" {
                break;
            }
            return self.parse_failure("Expected `)` or `,`");
        }
        self.next();
        Ok(PreType::Record(PreRecordType {
            positional_types,
            named_types: named_types.unwrap_or_default(),
        }))
    }

    fn parse_suffix(&mut self, type_: PreType) -> ParseResult<Result<PreType, PreType>> {
        // Returns `Ok(Ok(new))` for a suffix, `Ok(Err(type_))` (the type
        // unchanged) where Dart returns `null`.
        if self.current_token() == "?" {
            self.next();
            Ok(Ok(PreType::WithNullability(PreTypeWithNullability {
                inner: Box::new(type_),
                is_question_type: true,
            })))
        } else if self.current_token() == "&" {
            self.next();
            let promotion = self.parse_unsuffixed_type()?;
            Ok(Ok(PreType::Promoted(PrePromotedType {
                inner: Box::new(type_),
                promotion: Box::new(promotion),
            })))
        } else if self.current_token() == "Function" {
            self.next();
            let type_formals = if self.current_token() == "<" {
                self.parse_type_formals()?
            } else {
                Vec::new()
            };
            if self.current_token() != "(" {
                return self.parse_failure("Expected `(`");
            }
            self.next();
            let mut positional_parameter_types = Vec::new();
            let mut named_function_parameters: Option<Vec<PreNamedFunctionParameter>> = None;
            let mut required_positional_parameter_count: Option<usize> = None;
            if self.current_token() != ")" {
                loop {
                    if self.current_token() == "{" {
                        named_function_parameters = Some(self.parse_named_function_parameters()?);
                        if self.current_token() != ")" {
                            return self.parse_failure("Expected `)`");
                        }
                        break;
                    } else if self.current_token() == "[" {
                        required_positional_parameter_count =
                            Some(positional_parameter_types.len());
                        self.parse_optional_function_parameters(&mut positional_parameter_types)?;
                        if self.current_token() != ")" {
                            return self.parse_failure("Expected `)`");
                        }
                        break;
                    }
                    positional_parameter_types.push(self.parse_type()?);
                    if self.current_token() == ")" {
                        break;
                    }
                    if self.current_token() != "," {
                        return self.parse_failure("Expected `,` or `)`");
                    }
                    self.next();
                }
            }
            self.next();
            let required_positional_parameter_count =
                required_positional_parameter_count.unwrap_or(positional_parameter_types.len());
            Ok(Ok(PreType::Function(PreFunctionType {
                return_type: Box::new(type_),
                positional_parameter_types,
                required_positional_parameter_count,
                named_parameters: named_function_parameters.unwrap_or_default(),
                type_formals,
            })))
        } else {
            Ok(Err(type_))
        }
    }

    fn parse_type(&mut self) -> ParseResult<PreType> {
        // We currently accept the following grammar for types:
        //   type := unsuffixedType nullability suffix*
        //   unsuffixedType := identifier typeArgs?
        //                   | `_`
        //                   | `(` type `)`
        //                   | `(` recordTypeFields `,` recordTypeNamedFields `)`
        //                   | `(` recordTypeFields `,`? `)`
        //                   | `(` recordTypeNamedFields? `)`
        //   recordTypeFields := type (`,` type)*
        //   recordTypeNamedFields := `{` recordTypeNamedField
        //                            (`,` recordTypeNamedField)* `,`? `}`
        //   recordTypeNamedField := type identifier
        //   typeArgs := `<` type (`,` type)* `>`
        //   nullability := `?`?
        //   suffix := `Function` typeParameters? `(` type (`,` type)* `)`
        //           | `Function` typeParameters? `(` (type `,`)*
        //             namedFunctionParameters `)`
        //           | `Function` typeParameters? `(` (type `,`)*
        //             optionalFunctionParameters `)`
        //           | `?`
        //           | `&` unsuffixedType
        //   namedFunctionParameters := `{` namedFunctionParameter
        //                              (`,` namedFunctionParameter)* `}`
        //   namedFunctionParameter := `required`? type identifier
        //   optionalFunctionParameters := `[` type (`,` type)* `]`
        //   typeParameters := `<` typeParameter (`,` typeParameter)* `>`
        //   typeParameter := identifier
        // TODO(paulberry): support more syntax if needed
        let mut result = self.parse_unsuffixed_type()?;
        loop {
            match self.parse_suffix(result)? {
                Ok(new_result) => result = new_result,
                Err(unchanged) => return Ok(unchanged),
            }
        }
    }

    fn parse_type_formals(&mut self) -> ParseResult<Vec<PreTypeFormal>> {
        assert_eq!(self.current_token(), "<");
        self.next();
        let mut type_formals = Vec::new();
        loop {
            let name = self.current_token().to_owned();
            if !starts_with_identifier(&name) {
                return self.parse_failure("Expected an identifier");
            }
            self.next();
            let mut bound = None;
            if self.current_token() == "extends" {
                self.next();
                bound = Some(self.parse_type()?);
            }
            type_formals.push(PreTypeFormal {
                name: Name::new(&name),
                bound,
            });
            if self.current_token() == "," {
                self.next();
                continue;
            }
            if self.current_token() == ">" {
                break;
            }
            return self.parse_failure("Expected `>` or `,`");
        }
        self.next();
        Ok(type_formals)
    }

    fn parse_unsuffixed_type(&mut self) -> ParseResult<PreType> {
        if self.current_token() == "_" {
            self.next();
            return Ok(PreType::Unknown);
        }
        if self.current_token() == "(" {
            self.next();
            if self.current_token() == ")" || self.current_token() == "{" {
                return self.parse_record_type_rest(Vec::new());
            }
            let type_ = self.parse_type()?;
            if self.current_token() == "," {
                self.next();
                return self.parse_record_type_rest(vec![type_]);
            }
            if self.current_token() != ")" {
                return self.parse_failure("Expected `)` or `,`");
            }
            self.next();
            return Ok(type_);
        }
        let type_name = self.current_token().to_owned();
        if !starts_with_identifier(&type_name) {
            return self.parse_failure("Expected an identifier, `_`, or `(`");
        }
        self.next();
        let mut type_args = Vec::new();
        if self.current_token() == "<" {
            self.next();
            loop {
                type_args.push(self.parse_type()?);
                if self.current_token() == ">" {
                    break;
                }
                if self.current_token() != "," {
                    return self.parse_failure("Expected `,` or `>`");
                }
                self.next();
            }
            self.next();
        }
        Ok(PreType::Primary(PrePrimaryType {
            type_name: Name::new(&type_name),
            type_args,
        }))
    }

    fn parse(type_str: &str) -> ParseResult<Type> {
        let mut parser = TypeParser {
            type_str: type_str.to_owned(),
            tokens: Self::tokenize_type_str(type_str)?,
            i: 0,
        };
        let result = parser.parse_type()?;
        if parser.current_token() != "<END>" {
            let extra = &parser.tokens[parser.i..parser.tokens.len() - 1];
            return Err(ParseError::new(format!(
                "Extra tokens after parsing type `{type_str}`: [{}]",
                extra.join(", ")
            )));
        }
        result.materialize(&TypeFormalScope::new())
    }

    /// Splits [type_str] into tokens: identifiers (`[_a-zA-Z][_a-zA-Z0-9]*`)
    /// and the punctuation `( ) < > , ? * & { } [ ]`, followed by `<END>`.
    /// Characters between tokens must be whitespace.
    fn tokenize_type_str(type_str: &str) -> ParseResult<Vec<String>> {
        let unrecognized = |extra_chars: &str| {
            Err(ParseError::new(format!(
                "Unrecognized character(s) in type `{type_str}`: {extra_chars}"
            )))
        };
        let mut result = Vec::new();
        let mut last_match_end = 0;
        let mut chars = type_str.char_indices().peekable();
        while let Some((start, c)) = chars.next() {
            let end = if c == '_' || c.is_ascii_alphabetic() {
                let mut end = start + c.len_utf8();
                while let Some(&(i, c)) = chars.peek() {
                    if c == '_' || c.is_ascii_alphanumeric() {
                        end = i + c.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }
                end
            } else if "()<>,?*&{}[]".contains(c) {
                start + c.len_utf8()
            } else {
                continue;
            };
            let extra_chars = type_str[last_match_end..start].trim();
            if !extra_chars.is_empty() {
                return unrecognized(extra_chars);
            }
            result.push(type_str[start..end].to_owned());
            last_match_end = end;
        }
        let extra_chars = type_str[last_match_end..].trim();
        if !extra_chars.is_empty() {
            return unrecognized(extra_chars);
        }
        result.push("<END>".to_owned());
        Ok(result)
    }
}
