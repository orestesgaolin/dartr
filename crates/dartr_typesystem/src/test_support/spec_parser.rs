// Dart source: pkg/analyzer/lib/src/test_utilities/test_library_builder.dart
// (_SpecParser, _TokenStream, _Parsed* classes)

//! The parser of type and declaration specs (`List<int>?`,
//! `abstract class A<T> extends B<T> implements C`, ...). It produces
//! parsed values that [`super::builder`] materializes into elements and
//! types.

use dartr_element::{ParameterKind, TypeId, Variance};

/// `_ParsedType`.
#[derive(Clone, Debug)]
pub enum ParsedType {
    /// `_ParsedExplicitType` (`dynamic`, `InvalidType`, `Never`,
    /// `UnknownInferredType`, `void`).
    Explicit(TypeId),
    /// `_ParsedNamedType`.
    Named { name: String, args: Vec<ParsedType> },
    /// `_ParsedNullableType`.
    Nullable(Box<ParsedType>),
    /// `_ParsedPromotedType` (`T & B`).
    Promoted {
        base: Box<ParsedType>,
        promoted_bound: Box<ParsedType>,
    },
    /// `_ParsedFunctionType`.
    Function {
        type_parameters: Vec<ParsedTypeParameter>,
        formal_parameters: Vec<ParsedFormalParameter>,
        return_type: Box<ParsedType>,
    },
    /// `_ParsedRecordType`.
    Record {
        positional_fields: Vec<ParsedType>,
        named_fields: Vec<(String, ParsedType)>,
    },
}

/// `_ParsedTypeParameter`.
#[derive(Clone, Debug)]
pub struct ParsedTypeParameter {
    pub name: String,
    pub bound: Option<ParsedType>,
    pub variance: Option<Variance>,
}

/// `_ParsedFormalParameter`.
#[derive(Clone, Debug)]
pub struct ParsedFormalParameter {
    pub is_covariant: bool,
    pub kind: ParameterKind,
    pub name: Option<String>,
    pub ty: ParsedType,
}

/// `_ParsedClassHeader`.
#[derive(Clone, Debug)]
pub struct ParsedClassHeader {
    pub name: String,
    pub is_abstract: bool,
    pub is_sealed: bool,
    pub type_parameters: Vec<ParsedTypeParameter>,
    pub supertype: ParsedType,
    pub mixins: Vec<ParsedType>,
    pub interfaces: Vec<ParsedType>,
}

/// `_ParsedConstructorHeader`.
#[derive(Clone, Debug)]
pub struct ParsedConstructorHeader {
    pub name: String,
    pub is_const: bool,
    pub is_factory: bool,
    pub formal_parameters: Vec<ParsedFormalParameter>,
}

/// `_ParsedEnumHeader`.
#[derive(Clone, Debug)]
pub struct ParsedEnumHeader {
    pub name: String,
    pub mixins: Vec<ParsedType>,
    pub interfaces: Vec<ParsedType>,
}

/// `_ParsedExecutableHeader`.
#[derive(Clone, Debug)]
pub struct ParsedExecutableHeader {
    pub name: String,
    pub type_parameters: Vec<ParsedTypeParameter>,
    pub formal_parameters: Vec<ParsedFormalParameter>,
    pub return_type: ParsedType,
}

/// `_ParsedExtensionTypeHeader`.
#[derive(Clone, Debug)]
pub struct ParsedExtensionTypeHeader {
    pub name: String,
    pub type_parameters: Vec<ParsedTypeParameter>,
    pub representation_type: ParsedType,
    pub representation_name: String,
    pub interfaces: Vec<ParsedType>,
}

/// `_ParsedMixinHeader`.
#[derive(Clone, Debug)]
pub struct ParsedMixinHeader {
    pub name: String,
    pub type_parameters: Vec<ParsedTypeParameter>,
    pub constraints: Vec<ParsedType>,
    pub interfaces: Vec<ParsedType>,
}

/// `_ParsedTypeAliasHeader`.
#[derive(Clone, Debug)]
pub struct ParsedTypeAliasHeader {
    pub name: String,
    pub type_parameters: Vec<ParsedTypeParameter>,
    pub aliased_type: ParsedType,
}

/// `_TypeParameterContext`.
#[derive(Clone, Copy, Debug)]
enum TypeParameterContext {
    ClassDeclaration,
    ExtensionTypeDeclaration,
    GenericFunctionType,
    MethodDeclaration,
    MixinDeclaration,
    TopLevelFunctionDeclaration,
    TypeAliasDeclaration,
}

impl TypeParameterContext {
    fn allows_variance(self) -> bool {
        !matches!(
            self,
            TypeParameterContext::MethodDeclaration
                | TypeParameterContext::TopLevelFunctionDeclaration
        )
    }
}

/// `_ExecutableHeaderContext`.
#[derive(Clone, Copy, Debug)]
enum ExecutableHeaderContext {
    Method,
    TopLevelFunction,
}

/// `_TokenStream`.
struct TokenStream {
    tokens: Vec<String>,
    index: usize,
}

impl TokenStream {
    /// The tokenizer regular expression
    /// `[$a-zA-Z_][$\w]*|<|>|\+|-|\*|/|%|~|&|\||\^|,|\?|\(|\)|\{|\}|\[|\]|=|;`
    /// (other characters, such as spaces, are skipped).
    fn from_str(input: &str) -> TokenStream {
        let chars: Vec<char> = input.chars().collect();
        let mut tokens = Vec::new();
        let mut i = 0;
        let is_start = |c: char| c == '$' || c == '_' || c.is_ascii_alphabetic();
        let is_part = |c: char| c == '$' || c == '_' || c.is_ascii_alphanumeric();
        while i < chars.len() {
            let c = chars[i];
            if is_start(c) {
                let start = i;
                while i < chars.len() && is_part(chars[i]) {
                    i += 1;
                }
                tokens.push(chars[start..i].iter().collect());
                continue;
            }
            if "<>+-*/%~&|^,?(){}[]=;".contains(c) {
                tokens.push(c.to_string());
            }
            i += 1;
        }
        TokenStream { tokens, index: 0 }
    }

    fn is_at_end(&self) -> bool {
        self.index >= self.tokens.len()
    }

    fn consume(&mut self) -> String {
        assert!(!self.is_at_end(), "Unexpected end of token stream.");
        self.index += 1;
        self.tokens[self.index - 1].clone()
    }

    fn expect(&mut self, expected: &str) {
        assert!(
            !self.is_at_end(),
            "Expected \"{expected}\" but found end of stream."
        );
        let token = self.consume();
        assert_eq!(token, expected, "Expected \"{expected}\" but found \"{token}\".");
    }

    fn match_(&mut self, expected: &str) -> bool {
        if self.peek_is(expected) {
            self.index += 1;
            return true;
        }
        false
    }

    fn peek(&self) -> &str {
        assert!(!self.is_at_end(), "Unexpected end of token stream.");
        &self.tokens[self.index]
    }

    fn peek_is(&self, token: &str) -> bool {
        !self.is_at_end() && self.peek() == token
    }

    fn peek_is_any_of(&self, tokens: &[&str]) -> bool {
        !self.is_at_end() && tokens.contains(&self.peek())
    }
}

/// `_SpecParser`.
pub struct SpecParser {
    stream: TokenStream,
}

impl SpecParser {
    fn new(input: &str) -> SpecParser {
        SpecParser {
            stream: TokenStream::from_str(input),
        }
    }

    fn expect_end(&self, message: &str) {
        assert!(self.stream.is_at_end(), "{message}");
    }

    fn parse_class_header_(&mut self) -> ParsedClassHeader {
        let mut is_abstract = false;
        let mut is_sealed = false;
        while !self.stream.is_at_end() && !self.stream.peek_is("class") {
            match self.stream.consume().as_str() {
                "abstract" => is_abstract = true,
                "sealed" => is_sealed = true,
                modifier => panic!("Unsupported class modifier: {modifier}"),
            }
        }
        self.stream.expect("class");
        let name = self.stream.consume();
        let type_parameters =
            self.parse_optional_type_parameters(TypeParameterContext::ClassDeclaration);
        let supertype = if self.stream.match_("extends") {
            self.parse_type_()
        } else {
            ParsedType::Named {
                name: "Object".into(),
                args: Vec::new(),
            }
        };
        let mut mixins = Vec::new();
        if self.stream.match_("with") {
            mixins = self.parse_types(&["implements"]);
        }
        let mut interfaces = Vec::new();
        if self.stream.match_("implements") {
            interfaces = self.parse_types(&[]);
        }
        self.expect_end("Unexpected trailing tokens in class header.");
        ParsedClassHeader {
            name,
            is_abstract,
            is_sealed,
            type_parameters,
            supertype,
            mixins,
            interfaces,
        }
    }

    fn parse_constructor_header_(&mut self) -> ParsedConstructorHeader {
        let is_const = self.stream.match_("const");
        let is_factory = self.stream.match_("factory");
        let name = self.stream.consume();
        self.stream.expect("(");
        let formal_parameters = self.parse_formal_parameters();
        self.stream.expect(")");
        self.expect_end("Unexpected trailing tokens in constructor header.");
        ParsedConstructorHeader {
            name,
            is_const,
            is_factory,
            formal_parameters,
        }
    }

    fn parse_enum_header_(&mut self) -> ParsedEnumHeader {
        self.stream.expect("enum");
        let name = self.stream.consume();
        let mut mixins = Vec::new();
        if self.stream.match_("with") {
            mixins = self.parse_types(&["implements"]);
        }
        let mut interfaces = Vec::new();
        if self.stream.match_("implements") {
            interfaces = self.parse_types(&[]);
        }
        self.stream.match_(";");
        self.expect_end("Unexpected trailing tokens in enum header.");
        ParsedEnumHeader {
            name,
            mixins,
            interfaces,
        }
    }

    fn parse_executable_header(&mut self, context: ExecutableHeaderContext) -> ParsedExecutableHeader {
        let return_type = self.parse_type_();
        let name = self.parse_executable_name(context);
        let type_parameters = self.parse_optional_type_parameters(match context {
            ExecutableHeaderContext::Method => TypeParameterContext::MethodDeclaration,
            ExecutableHeaderContext::TopLevelFunction => {
                TypeParameterContext::TopLevelFunctionDeclaration
            }
        });
        self.stream.expect("(");
        let formal_parameters = self.parse_formal_parameters();
        self.stream.expect(")");
        self.stream.match_(";");
        self.expect_end("Unexpected trailing tokens in executable header.");
        ParsedExecutableHeader {
            name,
            type_parameters,
            formal_parameters,
            return_type,
        }
    }

    fn parse_executable_name(&mut self, context: ExecutableHeaderContext) -> String {
        if matches!(context, ExecutableHeaderContext::Method) && self.stream.match_("operator") {
            return self.parse_operator_name();
        }
        self.stream.consume()
    }

    fn parse_extension_type_header_(&mut self) -> ParsedExtensionTypeHeader {
        self.stream.expect("extension");
        self.stream.expect("type");
        let name = self.stream.consume();
        let type_parameters =
            self.parse_optional_type_parameters(TypeParameterContext::ExtensionTypeDeclaration);
        self.stream.expect("(");
        let representation_type = self.parse_type_();
        let representation_name = self.stream.consume();
        self.stream.expect(")");
        let mut interfaces = Vec::new();
        if self.stream.match_("implements") {
            interfaces = self.parse_types(&[]);
        }
        self.expect_end("Unexpected trailing tokens in extension type header.");
        ParsedExtensionTypeHeader {
            name,
            type_parameters,
            representation_type,
            representation_name,
            interfaces,
        }
    }

    fn parse_formal_parameter(&mut self, mut kind: ParameterKind) -> ParsedFormalParameter {
        if kind == ParameterKind::Named && self.stream.match_("required") {
            kind = ParameterKind::NamedRequired;
        }
        let is_covariant = self.stream.match_("covariant");
        let ty = self.parse_type_();
        let mut name = None;
        if !self.stream.is_at_end() && !self.stream.peek_is_any_of(&[",", ")", "]", "}"]) {
            name = Some(self.stream.consume());
        }
        ParsedFormalParameter {
            is_covariant,
            kind,
            name,
            ty,
        }
    }

    fn parse_formal_parameters(&mut self) -> Vec<ParsedFormalParameter> {
        let mut formal_parameters = Vec::new();
        if self.stream.is_at_end() || self.stream.peek_is(")") {
            return formal_parameters;
        }
        while !self.stream.is_at_end() && !self.stream.peek_is_any_of(&[")", "[", "{"]) {
            formal_parameters.push(self.parse_formal_parameter(ParameterKind::Required));
            self.stream.match_(",");
        }
        if self.stream.match_("[") {
            while !self.stream.is_at_end() && !self.stream.peek_is("]") {
                formal_parameters.push(self.parse_formal_parameter(ParameterKind::Positional));
                self.stream.match_(",");
            }
            self.stream.expect("]");
        }
        if self.stream.match_("{") {
            while !self.stream.is_at_end() && !self.stream.peek_is("}") {
                formal_parameters.push(self.parse_formal_parameter(ParameterKind::Named));
                self.stream.match_(",");
            }
            self.stream.expect("}");
        }
        formal_parameters
    }

    fn parse_mixin_header_(&mut self) -> ParsedMixinHeader {
        self.stream.expect("mixin");
        let name = self.stream.consume();
        let type_parameters =
            self.parse_optional_type_parameters(TypeParameterContext::MixinDeclaration);
        let mut constraints = vec![ParsedType::Named {
            name: "Object".into(),
            args: Vec::new(),
        }];
        if self.stream.match_("on") {
            constraints = self.parse_types(&["implements"]);
        }
        let mut interfaces = Vec::new();
        if self.stream.match_("implements") {
            interfaces = self.parse_types(&[]);
        }
        self.expect_end("Unexpected trailing tokens in mixin header.");
        ParsedMixinHeader {
            name,
            type_parameters,
            constraints,
            interfaces,
        }
    }

    fn parse_operator_name(&mut self) -> String {
        if self.stream.match_("[") {
            self.stream.expect("]");
            if self.stream.match_("=") {
                return "[]=".into();
            }
            return "[]".into();
        }
        if self
            .stream
            .peek_is_any_of(&["+", "-", "*", "/", "%", "~", "&", "|", "^"])
        {
            return self.stream.consume();
        }
        if self.stream.match_("=") {
            self.stream.expect("=");
            return "==".into();
        }
        if self.stream.match_("<") {
            if self.stream.match_("=") {
                return "<=".into();
            }
            if self.stream.match_("<") {
                return "<<".into();
            }
            return "<".into();
        }
        if self.stream.match_(">") {
            if self.stream.match_("=") {
                return ">=".into();
            }
            if self.stream.match_(">") {
                if self.stream.match_(">") {
                    return ">>>".into();
                }
                return ">>".into();
            }
            return ">".into();
        }
        panic!("Unsupported operator token in executable header.");
    }

    fn parse_optional_type_parameters(
        &mut self,
        context: TypeParameterContext,
    ) -> Vec<ParsedTypeParameter> {
        if self.stream.is_at_end() || !self.stream.match_("<") {
            return Vec::new();
        }
        self.parse_type_parameters_rest(context)
    }

    fn parse_parenthesized_or_record_type(&mut self) -> ParsedType {
        self.stream.expect("(");
        let mut positional_fields = Vec::new();
        let mut named_fields = Vec::new();
        let mut has_comma = false;
        while !self.stream.is_at_end() && !self.stream.peek_is_any_of(&[")", "{"]) {
            positional_fields.push(self.parse_type_());
            has_comma = self.stream.match_(",");
            if !has_comma {
                break;
            }
        }
        if self.stream.match_("{") {
            while !self.stream.is_at_end() && !self.stream.peek_is("}") {
                let ty = self.parse_type_();
                let name = self.stream.consume();
                named_fields.push((name, ty));
                self.stream.match_(",");
            }
            self.stream.expect("}");
        }
        self.stream.expect(")");
        // `(int)` is parenthesized, but `(int,)` is a record.
        if positional_fields.len() == 1 && !has_comma && named_fields.is_empty() {
            return positional_fields.pop().unwrap();
        }
        ParsedType::Record {
            positional_fields,
            named_fields,
        }
    }

    fn parse_primary_type(&mut self) -> ParsedType {
        if self.stream.peek_is("(") {
            return self.parse_parenthesized_or_record_type();
        }
        let name = self.stream.consume();
        match name.as_str() {
            "dynamic" => return ParsedType::Explicit(TypeId::DYNAMIC),
            "InvalidType" => return ParsedType::Explicit(TypeId::INVALID),
            "Never" => return ParsedType::Explicit(TypeId::NEVER),
            "UnknownInferredType" => return ParsedType::Explicit(TypeId::UNKNOWN),
            "void" => return ParsedType::Explicit(TypeId::VOID),
            _ => {}
        }
        let mut args = Vec::new();
        if self.stream.match_("<") {
            while !self.stream.is_at_end() && !self.stream.peek_is(">") {
                args.push(self.parse_type_());
                self.stream.match_(",");
            }
            self.stream.expect(">");
        }
        ParsedType::Named { name, args }
    }

    fn parse_type_(&mut self) -> ParsedType {
        let mut ty = self.parse_primary_type();
        loop {
            if self.stream.match_("?") {
                ty = ParsedType::Nullable(Box::new(ty));
                continue;
            }
            if self.stream.match_("Function") {
                let type_parameters =
                    self.parse_optional_type_parameters(TypeParameterContext::GenericFunctionType);
                self.stream.expect("(");
                let formal_parameters = self.parse_formal_parameters();
                self.stream.expect(")");
                ty = ParsedType::Function {
                    type_parameters,
                    formal_parameters,
                    return_type: Box::new(ty),
                };
                continue;
            }
            if self.stream.match_("&") {
                ty = ParsedType::Promoted {
                    base: Box::new(ty),
                    promoted_bound: Box::new(self.parse_type_()),
                };
                continue;
            }
            return ty;
        }
    }

    fn parse_type_alias_header_(&mut self) -> ParsedTypeAliasHeader {
        self.stream.expect("typedef");
        let name = self.stream.consume();
        let type_parameters =
            self.parse_optional_type_parameters(TypeParameterContext::TypeAliasDeclaration);
        self.stream.expect("=");
        let aliased_type = self.parse_type_();
        self.stream.match_(";");
        self.expect_end("Unexpected trailing tokens in type alias header.");
        ParsedTypeAliasHeader {
            name,
            type_parameters,
            aliased_type,
        }
    }

    fn parse_type_parameters_rest(&mut self, context: TypeParameterContext) -> Vec<ParsedTypeParameter> {
        assert!(
            !self.stream.peek_is(">"),
            "Type parameter clause cannot be empty."
        );
        let mut type_parameters = Vec::new();
        while !self.stream.is_at_end() && !self.stream.peek_is(">") {
            let mut variance = None;
            if let Some(keyword_variance) = variance_for_keyword(self.stream.peek()) {
                self.stream.consume();
                assert!(
                    context.allows_variance(),
                    "Variance modifiers are not allowed in {context:?}."
                );
                variance = Some(keyword_variance);
            }
            let name = self.stream.consume();
            let mut bound = None;
            if self.stream.match_("extends") {
                bound = Some(self.parse_type_());
            }
            type_parameters.push(ParsedTypeParameter {
                name,
                bound,
                variance,
            });
            self.stream.match_(",");
        }
        self.stream.expect(">");
        type_parameters
    }

    fn parse_types(&mut self, stop_tokens: &[&str]) -> Vec<ParsedType> {
        assert!(
            !(self.stream.is_at_end() || self.stream.peek_is_any_of(stop_tokens)),
            "Expected a type."
        );
        let mut types = Vec::new();
        while !self.stream.is_at_end() && !self.stream.peek_is_any_of(stop_tokens) {
            types.push(self.parse_type_());
            if !self.stream.match_(",") {
                break;
            }
            assert!(
                !(self.stream.is_at_end() || self.stream.peek_is_any_of(stop_tokens)),
                "Expected a type after \",\"."
            );
        }
        types
    }

    pub fn parse_class_header(input: &str) -> ParsedClassHeader {
        SpecParser::new(input).parse_class_header_()
    }

    pub fn parse_constructor_header(input: &str) -> ParsedConstructorHeader {
        SpecParser::new(input).parse_constructor_header_()
    }

    pub fn parse_enum_header(input: &str) -> ParsedEnumHeader {
        SpecParser::new(input).parse_enum_header_()
    }

    pub fn parse_extension_type_header(input: &str) -> ParsedExtensionTypeHeader {
        SpecParser::new(input).parse_extension_type_header_()
    }

    pub fn parse_method_header(input: &str) -> ParsedExecutableHeader {
        SpecParser::new(input).parse_executable_header(ExecutableHeaderContext::Method)
    }

    pub fn parse_mixin_header(input: &str) -> ParsedMixinHeader {
        SpecParser::new(input).parse_mixin_header_()
    }

    pub fn parse_top_level_function_header(input: &str) -> ParsedExecutableHeader {
        SpecParser::new(input).parse_executable_header(ExecutableHeaderContext::TopLevelFunction)
    }

    pub fn parse_type(input: &str) -> ParsedType {
        let mut parser = SpecParser::new(input);
        let ty = parser.parse_type_();
        parser.expect_end("Unexpected trailing tokens in type.");
        ty
    }

    pub fn parse_type_alias_header(input: &str) -> ParsedTypeAliasHeader {
        SpecParser::new(input).parse_type_alias_header_()
    }

    pub fn parse_type_parameters(input: &str) -> Vec<ParsedTypeParameter> {
        let input = input.trim();
        let input = if input.starts_with('<') {
            input.to_string()
        } else {
            format!("<{input}>")
        };
        let mut parser = SpecParser::new(&input);
        parser.stream.expect("<");
        let type_parameters = parser.parse_type_parameters_rest(TypeParameterContext::ClassDeclaration);
        parser.expect_end("Unexpected trailing tokens in type parameters.");
        type_parameters
    }
}

/// `_varianceForKeyword(keyword)`.
fn variance_for_keyword(keyword: &str) -> Option<Variance> {
    match keyword {
        "out" => Some(Variance::Covariant),
        "in" => Some(Variance::Contravariant),
        "inout" => Some(Variance::Invariant),
        _ => None,
    }
}
