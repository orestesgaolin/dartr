// Dart source: pkg/_fe_analyzer_shared/test/mini_types.dart

//! A "mini type system" that's similar to full Dart types, but light weight
//! enough to be suitable for unit testing of code in the
//! `_fe_analyzer_shared` package.
//!
//! # Representation
//!
//! Dart allocates a `Type` object per type. Here a [`Type`] is a `Copy`
//! handle (an index) into a `thread_local!` arena; the arena keeps the type
//! data ([`PrimaryType`], [`FunctionType`], [`RecordType`],
//! [`TypeParameterType`], [`UnknownType`]). Type names ([`TypeNameInfo`]) and
//! type parameters ([`TypeParameter`]) are handles into the same arena. Rust
//! runs each test on its own thread, so each test has its own arena. This
//! matches the Dart `setUp`/`tearDown` calls of [`TypeRegistry::init`] and
//! [`TypeRegistry::uninit`].
//!
//! Every constructor allocates a new handle, as every Dart constructor
//! allocates a new object. Dart `identical` (`same(...)` in tests) is
//! [`Type::identical`].
//!
//! # Equality
//!
//! `==` on [`Type`] is the Dart `operator ==` of mini types: structural
//! equality, where generic function types are equal under a consistent
//! renaming of their type formals, and type parameters are compared by
//! identity. [`Hash`] is consistent with it (it hashes
//! [`Type::hash_code`]). [`TypeParameter`] and [`TypeNameInfo`] compare by
//! identity (Dart does not override `==` for them).
//!
//! # Names
//!
//! Dart `String` names (type names, record fields, named parameters) are
//! [`Name`] (`&'static str`), interned by [`intern`] (the strings are leaked;
//! this is test code).

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::Mutex;

/// A name (Dart `String`) of a type, a type parameter, a record field or a
/// named parameter.
pub type Name = &'static str;

/// Interns `s` and returns the `'static` string (leaked once per distinct
/// string).
pub fn intern(s: &str) -> Name {
    static NAMES: Mutex<Option<HashSet<&'static str>>> = Mutex::new(None);
    let mut guard = NAMES.lock().unwrap();
    let set = guard.get_or_insert_with(HashSet::new);
    if let Some(existing) = set.get(s) {
        return existing;
    }
    let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());
    set.insert(leaked);
    leaked
}

/// Surrounds `s` with parentheses if `condition` is `true`, otherwise returns
/// `s` unchanged.
fn parenthesize_if(condition: bool, s: String) -> String {
    if condition { format!("({s})") } else { s }
}

// ============================================================== arena (private)

/// The special types of [`SpecialTypeName`] (the Dart `expectedRuntimeType`
/// of a special type name).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SpecialKind {
    Dynamic,
    Error,
    FutureOr,
    Never,
    Null,
    Void,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NameKind {
    Interface,
    Special(SpecialKind),
    TypeParameter,
}

#[derive(Clone)]
struct NameData {
    name: Name,
    kind: NameKind,
    /// `TypeParameter.explicitBound` (only for type parameters).
    explicit_bound: Option<Type>,
}

#[derive(Clone)]
enum TypeData {
    Primary(PrimaryType),
    Function(FunctionType),
    Record(RecordType),
    TypeParameter(TypeParameterType),
    Unknown(UnknownType),
}

struct Arena {
    names: Vec<NameData>,
    types: Vec<TypeData>,
    /// `TypeRegistry._typeNameInfoMap`.
    registry: Option<HashMap<String, TypeNameInfo>>,
}

// Fixed name infos (the Dart `static final` fields of `TypeRegistry`).
const DYNAMIC_NAME: TypeNameInfo = TypeNameInfo(0);
const ERROR_NAME: TypeNameInfo = TypeNameInfo(1);
const FUTURE_NAME: TypeNameInfo = TypeNameInfo(2);
const FUTURE_OR_NAME: TypeNameInfo = TypeNameInfo(3);
const ITERABLE_NAME: TypeNameInfo = TypeNameInfo(4);
const LIST_NAME: TypeNameInfo = TypeNameInfo(5);
const MAP_NAME: TypeNameInfo = TypeNameInfo(6);
const NEVER_NAME: TypeNameInfo = TypeNameInfo(7);
const NULL_NAME: TypeNameInfo = TypeNameInfo(8);
const STREAM_NAME: TypeNameInfo = TypeNameInfo(9);
const VOID_NAME: TypeNameInfo = TypeNameInfo(10);

// Fixed types (the Dart `instance` singletons).
const DYNAMIC_INSTANCE: Type = Type(0);
const INVALID_INSTANCE: Type = Type(1);
const NEVER_INSTANCE: Type = Type(2);
const NULL_INSTANCE: Type = Type(3);
const VOID_INSTANCE: Type = Type(4);

impl Arena {
    fn new() -> Arena {
        let special = |name: &str, kind| NameData {
            name: intern(name),
            kind: NameKind::Special(kind),
            explicit_bound: None,
        };
        let interface = |name: &str| NameData {
            name: intern(name),
            kind: NameKind::Interface,
            explicit_bound: None,
        };
        let names = vec![
            special("dynamic", SpecialKind::Dynamic),
            special("error", SpecialKind::Error),
            interface("Future"),
            special("FutureOr", SpecialKind::FutureOr),
            interface("Iterable"),
            interface("List"),
            interface("Map"),
            special("Never", SpecialKind::Never),
            special("Null", SpecialKind::Null),
            interface("Stream"),
            special("void", SpecialKind::Void),
        ];
        let simple = |name_info| {
            TypeData::Primary(PrimaryType {
                name_info,
                args: Vec::new(),
                is_question_type: false,
            })
        };
        let types = vec![
            simple(DYNAMIC_NAME),
            simple(ERROR_NAME),
            simple(NEVER_NAME),
            simple(NULL_NAME),
            simple(VOID_NAME),
        ];
        Arena {
            names,
            types,
            registry: None,
        }
    }
}

thread_local! {
    static ARENA: RefCell<Arena> = RefCell::new(Arena::new());
}

fn alloc_type(data: TypeData) -> Type {
    ARENA.with(|a| {
        let mut a = a.borrow_mut();
        a.types.push(data);
        Type((a.types.len() - 1) as u32)
    })
}

fn alloc_name(name: &str, kind: NameKind) -> u32 {
    let name = intern(name);
    ARENA.with(|a| {
        let mut a = a.borrow_mut();
        a.names.push(NameData {
            name,
            kind,
            explicit_bound: None,
        });
        (a.names.len() - 1) as u32
    })
}

fn name_data(id: u32) -> NameData {
    ARENA.with(|a| a.borrow().names[id as usize].clone())
}

// ================================================================== TypeNameInfo

/// Information about a single type name recognized by the [`Type`] parser
/// (Dart sealed class `TypeNameInfo` with the subclasses
/// `InterfaceTypeName`, `SpecialTypeName` and `TypeParameter`).
///
/// Compared by identity.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeNameInfo(u32);

impl TypeNameInfo {
    /// The name.
    pub fn name(self) -> Name {
        name_data(self.0).name
    }

    /// Dart `is InterfaceTypeName`: a type name that represents an ordinary
    /// interface type.
    pub fn is_interface_type_name(self) -> bool {
        name_data(self.0).kind == NameKind::Interface
    }

    /// Dart `is SpecialTypeName`: a type name that represents one of Dart's
    /// built-in "special" types, such as `dynamic`, `error` (to represent an
    /// invalid type), `FutureOr`, `Never`, `Null`, `void`.
    pub fn is_special_type_name(self) -> bool {
        matches!(name_data(self.0).kind, NameKind::Special(_))
    }

    /// Dart `is TypeParameter`: returns the type parameter if this name is
    /// one.
    pub fn as_type_parameter(self) -> Option<TypeParameter> {
        match name_data(self.0).kind {
            NameKind::TypeParameter => Some(TypeParameter(self.0)),
            _ => None,
        }
    }

    fn special_kind(self) -> Option<SpecialKind> {
        match name_data(self.0).kind {
            NameKind::Special(kind) => Some(kind),
            _ => None,
        }
    }
}

impl fmt::Debug for TypeNameInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TypeNameInfo({})", self.name())
    }
}

// ================================================================= TypeParameter

/// A type name that represents a type variable (Dart class `TypeParameter`,
/// which implements `SharedTypeParameter`).
///
/// Compared by identity: two type parameters with the same name are
/// different (for example the type formals of two parsed function types).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeParameter(u32);

impl TypeParameter {
    /// `TypeParameter._`: a new type parameter that is not registered in the
    /// [`TypeRegistry`].
    fn new_unregistered(name: &str) -> TypeParameter {
        TypeParameter(alloc_name(name, NameKind::TypeParameter))
    }

    /// This type parameter as a [`TypeNameInfo`].
    pub fn name_info(self) -> TypeNameInfo {
        TypeNameInfo(self.0)
    }

    /// The name.
    pub fn name(self) -> Name {
        name_data(self.0).name
    }

    /// The type variable's bound. If `None`, the bound is `Object?`.
    ///
    /// This is mutable because it needs to be possible to set it after
    /// construction, in order to create "F-bounded" type parameters (type
    /// parameters whose bound refers to the type parameter itself).
    pub fn explicit_bound(self) -> Option<Type> {
        name_data(self.0).explicit_bound
    }

    /// Sets [`explicit_bound`](Self::explicit_bound).
    pub fn set_explicit_bound(self, bound: Option<Type>) {
        ARENA.with(|a| a.borrow_mut().names[self.0 as usize].explicit_bound = bound);
    }

    /// `bound`: `explicitBound ?? Type('Object?')`.
    pub fn bound(self) -> Type {
        self.explicit_bound()
            .unwrap_or_else(|| Type::parse("Object?"))
    }

    /// `boundShared`.
    pub fn bound_shared(self) -> Option<Type> {
        Some(self.bound())
    }

    /// `displayName`.
    pub fn display_name(self) -> String {
        self.name().to_string()
    }

    /// `isLegacyCovariant`.
    // TODO(paulberry): Implement isLegacyCovariant.
    pub fn is_legacy_covariant(self) -> bool {
        true
    }

    /// `variance`.
    // TODO(paulberry): Implement variance.
    pub fn variance(self) -> dartr_flow::shared_type::Variance {
        dartr_flow::shared_type::Variance::Covariant
    }
}

impl fmt::Display for TypeParameter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl fmt::Debug for TypeParameter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", self.name(), self.0)
    }
}

// =================================================================== data views

/// Representation of a primary type suitable for unit testing of code in the
/// `_fe_analyzer_shared` package. A primary type is either an interface type
/// with zero or more type parameters (e.g. `double`, or `Map<int, String>`) or
/// one of the special types whose name is a single word (e.g. `dynamic`).
///
/// The special types `dynamic`, `error`, `FutureOr<T>`, `Never`, `Null` and
/// `void` are primary types too (Dart subclasses `DynamicType`, ...).
#[derive(Clone, Debug)]
pub struct PrimaryType {
    /// Information about the type name.
    pub name_info: TypeNameInfo,
    /// The type arguments, or empty if there are no type arguments.
    pub args: Vec<Type>,
    /// `isQuestionType`.
    pub is_question_type: bool,
}

impl PrimaryType {
    /// `PrimaryType(InterfaceTypeName nameInfo, {args, isQuestionType})`.
    pub fn new(name_info: TypeNameInfo, args: Vec<Type>) -> Type {
        assert!(
            name_info.is_interface_type_name(),
            "PrimaryType requires an InterfaceTypeName"
        );
        PrimaryType {
            name_info,
            args,
            is_question_type: false,
        }
        .into_type()
    }

    /// Allocates the type (`PrimaryType._`).
    pub fn into_type(self) -> Type {
        alloc_type(TypeData::Primary(self))
    }

    /// `isInterfaceType`.
    pub fn is_interface_type(&self) -> bool {
        self.name_info.is_interface_type_name()
    }

    /// The name of the type.
    pub fn name(&self) -> Name {
        self.name_info.name()
    }
}

/// A named parameter of a function type.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NamedFunctionParameter {
    /// `isRequired`.
    pub is_required: bool,
    /// `name`.
    pub name: Name,
    /// `type`.
    pub ty: Type,
}

impl NamedFunctionParameter {
    /// `NamedFunctionParameter({isRequired, name, type})`.
    pub fn new(is_required: bool, name: &str, ty: Type) -> Self {
        NamedFunctionParameter {
            is_required,
            name: intern(name),
            ty,
        }
    }

    fn substitute(&self, substitution: &HashMap<TypeParameter, Type>) -> Option<Self> {
        let new_type = self.ty.substitute(substitution)?;
        Some(NamedFunctionParameter {
            is_required: self.is_required,
            name: self.name,
            ty: new_type,
        })
    }
}

impl fmt::Display for NamedFunctionParameter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_required {
            write!(f, "required {} {}", self.ty, self.name)
        } else {
            write!(f, "{} {}", self.ty, self.name)
        }
    }
}

/// A named field of a record type.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NamedType {
    /// `name`.
    pub name: Name,
    /// `type`.
    pub ty: Type,
}

impl NamedType {
    /// `NamedType({name, type})`.
    pub fn new(name: &str, ty: Type) -> Self {
        NamedType {
            name: intern(name),
            ty,
        }
    }

    fn substitute(&self, substitution: &HashMap<TypeParameter, Type>) -> Option<Self> {
        let new_type = self.ty.substitute(substitution)?;
        Some(NamedType {
            name: self.name,
            ty: new_type,
        })
    }
}

/// Representation of a function type suitable for unit testing of code in the
/// `_fe_analyzer_shared` package.
#[derive(Clone, Debug)]
pub struct FunctionType {
    /// `returnType`.
    pub return_type: Type,
    /// `typeParametersShared`.
    pub type_parameters_shared: Vec<TypeParameter>,
    /// A list of the types of positional parameters.
    pub positional_parameters: Vec<Type>,
    /// `requiredPositionalParameterCount`.
    pub required_positional_parameter_count: usize,
    /// A list of the named parameters, sorted by name.
    pub named_parameters: Vec<NamedFunctionParameter>,
    /// `isQuestionType`.
    pub is_question_type: bool,
}

impl FunctionType {
    /// `FunctionType(returnType, positionalParameters)` with the Dart
    /// defaults: no type formals, all positional parameters required, no
    /// named parameters, no `?`. Change them with the `with_...` methods,
    /// then call [`into_type`](Self::into_type).
    pub fn new(return_type: Type, positional_parameters: Vec<Type>) -> Self {
        FunctionType {
            return_type,
            type_parameters_shared: Vec::new(),
            required_positional_parameter_count: positional_parameters.len(),
            positional_parameters,
            named_parameters: Vec::new(),
            is_question_type: false,
        }
    }

    /// Named parameter `typeParametersShared`.
    pub fn with_type_parameters(mut self, type_parameters: Vec<TypeParameter>) -> Self {
        self.type_parameters_shared = type_parameters;
        self
    }

    /// Named parameter `requiredPositionalParameterCount`.
    pub fn with_required_positional_parameter_count(mut self, count: usize) -> Self {
        self.required_positional_parameter_count = count;
        self
    }

    /// Named parameter `namedParameters` (must be sorted by name).
    pub fn with_named_parameters(mut self, named_parameters: Vec<NamedFunctionParameter>) -> Self {
        self.named_parameters = named_parameters;
        self
    }

    /// Named parameter `isQuestionType`.
    pub fn with_question_type(mut self, is_question_type: bool) -> Self {
        self.is_question_type = is_question_type;
        self
    }

    /// Allocates the type.
    pub fn into_type(self) -> Type {
        for i in 1..self.named_parameters.len() {
            assert!(
                self.named_parameters[i - 1].name < self.named_parameters[i].name,
                "namedParameters not properly sorted"
            );
        }
        alloc_type(TypeData::Function(self))
    }
}

/// Representation of a record type suitable for unit testing.
#[derive(Clone, Debug)]
pub struct RecordType {
    /// `positionalTypes`.
    pub positional_types: Vec<Type>,
    /// `namedTypes` (sorted by name).
    pub named_types: Vec<NamedType>,
    /// `isQuestionType`.
    pub is_question_type: bool,
}

impl RecordType {
    /// `RecordType({positionalTypes, namedTypes})`.
    pub fn new(positional_types: Vec<Type>, named_types: Vec<NamedType>) -> Type {
        RecordType {
            positional_types,
            named_types,
            is_question_type: false,
        }
        .into_type()
    }

    /// Allocates the type.
    pub fn into_type(self) -> Type {
        for i in 1..self.named_types.len() {
            assert!(
                self.named_types[i - 1].name < self.named_types[i].name,
                "namedTypes not properly sorted"
            );
        }
        alloc_type(TypeData::Record(self))
    }

    /// `sortedNamedTypes`.
    pub fn sorted_named_types(&self) -> &[NamedType] {
        &self.named_types
    }
}

/// Representation of a type parameter type suitable for unit testing of code
/// in the `_fe_analyzer_shared` package. A type parameter type might be
/// promoted, in which case it is often written using the syntax `a&b`, where
/// `a` is the type parameter and `b` is what it's promoted to. For example,
/// `T&int` represents the type parameter `T`, promoted to `int`.
#[derive(Clone, Debug)]
pub struct TypeParameterType {
    /// The type parameter this type is based on.
    pub type_parameter: TypeParameter,
    /// If non-null, the promoted type.
    pub promotion: Option<Type>,
    /// `isQuestionType`.
    pub is_question_type: bool,
}

impl TypeParameterType {
    /// `TypeParameterType(typeParameter)`.
    pub fn new(type_parameter: TypeParameter) -> Type {
        TypeParameterType {
            type_parameter,
            promotion: None,
            is_question_type: false,
        }
        .into_type()
    }

    /// Allocates the type.
    pub fn into_type(self) -> Type {
        alloc_type(TypeData::TypeParameter(self))
    }

    /// The type parameter's bound.
    pub fn bound(&self) -> Type {
        self.type_parameter.bound()
    }
}

/// Representation of the unknown type suitable for unit testing of code in
/// the `_fe_analyzer_shared` package.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnknownType {
    /// `isQuestionType`.
    pub is_question_type: bool,
}

impl UnknownType {
    /// `UnknownType()`.
    pub fn new() -> Type {
        UnknownType::default().into_type()
    }

    /// Allocates the type.
    pub fn into_type(self) -> Type {
        alloc_type(TypeData::Unknown(self))
    }
}

/// Representation of the type `FutureOr<T>` suitable for unit testing.
pub struct FutureOrType;

impl FutureOrType {
    /// `FutureOrType(typeArgument)`.
    pub fn new(type_argument: Type) -> Type {
        Self::with_question_type(type_argument, false)
    }

    /// `FutureOrType(typeArgument, isQuestionType: ...)`.
    pub fn with_question_type(type_argument: Type, is_question_type: bool) -> Type {
        PrimaryType {
            name_info: FUTURE_OR_NAME,
            args: vec![type_argument],
            is_question_type,
        }
        .into_type()
    }
}

/// Representation of the type `dynamic` suitable for unit testing.
pub struct DynamicType;
impl DynamicType {
    /// `DynamicType.instance`.
    pub fn instance() -> Type {
        DYNAMIC_INSTANCE
    }
}

/// Representation of an invalid type suitable for unit testing.
pub struct InvalidType;
impl InvalidType {
    /// `InvalidType.instance`.
    pub fn instance() -> Type {
        INVALID_INSTANCE
    }
}

/// Representation of the type `Never` suitable for unit testing.
pub struct NeverType;
impl NeverType {
    /// `NeverType.instance`.
    pub fn instance() -> Type {
        NEVER_INSTANCE
    }
}

/// Representation of the type `Null` suitable for unit testing.
pub struct NullType;
impl NullType {
    /// `NullType.instance`.
    pub fn instance() -> Type {
        NULL_INSTANCE
    }
}

/// Representation of the type `void` suitable for unit testing.
pub struct VoidType;
impl VoidType {
    /// `VoidType.instance`.
    pub fn instance() -> Type {
        VOID_INSTANCE
    }
}

// ========================================================================= Type

/// The Dart runtime type of a mini [`Type`] (for `is` tests).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TypeKind {
    /// `DynamicType`.
    Dynamic,
    /// `InvalidType`.
    Invalid,
    /// `NeverType`.
    Never,
    /// `NullType`.
    Null,
    /// `VoidType`.
    Void,
    /// `FutureOrType`.
    FutureOr,
    /// `PrimaryType` (an interface type).
    Primary,
    /// `FunctionType`.
    Function,
    /// `RecordType`.
    Record,
    /// `TypeParameterType`.
    TypeParameter,
    /// `UnknownType`.
    Unknown,
}

/// Representation of a type suitable for unit testing of code in the
/// `_fe_analyzer_shared` package.
///
/// A `Copy` handle into the thread-local arena; see the module docs for
/// equality.
#[derive(Clone, Copy)]
pub struct Type(u32);

/// Exception thrown if a type fails to parse properly.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParseError {
    /// `message`.
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

fn parse_error<T>(message: impl Into<String>) -> Result<T, ParseError> {
    Err(ParseError {
        message: message.into(),
    })
}

/// Shorthand for [`Type::parse`] (Dart `Type('...')`).
pub fn t(type_str: &str) -> Type {
    Type::parse(type_str)
}

/// Applies `f` to each element of `list`. If all calls return `None`
/// (meaning nothing changed), returns `None`. Otherwise returns a new list in
/// which each changed element is replaced with the result.
///
/// This is the shared logic of the Dart list extensions `substitute`,
/// `closureWithRespectToUnknown` and `recursivelyDemote`.
fn map_list<T: Copy>(list: &[T], f: impl Fn(&T) -> Option<T>) -> Option<Vec<T>> {
    let mut result: Option<Vec<T>> = None;
    for (i, old) in list.iter().enumerate() {
        let new = f(old);
        if new.is_some() && result.is_none() {
            result = Some(list[..i].to_vec());
        }
        if let Some(result) = &mut result {
            result.push(new.unwrap_or(*old));
        }
    }
    result
}

fn list_equals(a: &[Type], b: &[Type]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x == y)
}

impl Type {
    /// `Type(typeStr)`: parses `type_str`. Panics on a [`ParseError`] (and on
    /// an unknown type name, a Dart `StateError`).
    pub fn parse(type_str: &str) -> Type {
        match Self::try_parse(type_str) {
            Ok(ty) => ty,
            Err(e) => panic!("ParseError: {e}"),
        }
    }

    /// `Type(typeStr)`, returning the [`ParseError`] instead of throwing it.
    pub fn try_parse(type_str: &str) -> Result<Type, ParseError> {
        TypeParser::parse(type_str)
    }

    fn data(self) -> TypeData {
        ARENA.with(|a| a.borrow().types[self.0 as usize].clone())
    }

    /// Dart `identical(this, other)`.
    pub fn identical(self, other: Type) -> bool {
        self.0 == other.0
    }

    /// The Dart runtime type of this type.
    pub fn kind(self) -> TypeKind {
        match self.data() {
            TypeData::Primary(p) => match p.name_info.special_kind() {
                None => TypeKind::Primary,
                Some(SpecialKind::Dynamic) => TypeKind::Dynamic,
                Some(SpecialKind::Error) => TypeKind::Invalid,
                Some(SpecialKind::FutureOr) => TypeKind::FutureOr,
                Some(SpecialKind::Never) => TypeKind::Never,
                Some(SpecialKind::Null) => TypeKind::Null,
                Some(SpecialKind::Void) => TypeKind::Void,
            },
            TypeData::Function(_) => TypeKind::Function,
            TypeData::Record(_) => TypeKind::Record,
            TypeData::TypeParameter(_) => TypeKind::TypeParameter,
            TypeData::Unknown(_) => TypeKind::Unknown,
        }
    }

    /// Dart `is PrimaryType` (also true for the special types).
    pub fn as_primary_type(self) -> Option<PrimaryType> {
        match self.data() {
            TypeData::Primary(p) => Some(p),
            _ => None,
        }
    }

    /// Dart `is FunctionType`.
    pub fn as_function_type(self) -> Option<FunctionType> {
        match self.data() {
            TypeData::Function(f) => Some(f),
            _ => None,
        }
    }

    /// Dart `is RecordType`.
    pub fn as_record_type(self) -> Option<RecordType> {
        match self.data() {
            TypeData::Record(r) => Some(r),
            _ => None,
        }
    }

    /// Dart `is TypeParameterType`.
    pub fn as_type_parameter_type(self) -> Option<TypeParameterType> {
        match self.data() {
            TypeData::TypeParameter(t) => Some(t),
            _ => None,
        }
    }

    /// Dart `is UnknownType`.
    pub fn as_unknown_type(self) -> Option<UnknownType> {
        match self.data() {
            TypeData::Unknown(u) => Some(u),
            _ => None,
        }
    }

    /// Dart `FutureOrType.typeArgument`, if this is a `FutureOrType`.
    pub fn future_or_type_argument(self) -> Option<Type> {
        match self.kind() {
            TypeKind::FutureOr => Some(self.as_primary_type().unwrap().args[0]),
            _ => None,
        }
    }

    /// `isQuestionType`.
    pub fn is_question_type(self) -> bool {
        match self.data() {
            TypeData::Primary(p) => p.is_question_type,
            TypeData::Function(f) => f.is_question_type,
            TypeData::Record(r) => r.is_question_type,
            TypeData::TypeParameter(t) => t.is_question_type,
            TypeData::Unknown(u) => u.is_question_type,
        }
    }

    /// `type`: the string representation (`toString()`).
    pub fn type_string(self) -> String {
        self.to_string()
    }

    /// `asQuestionType`.
    pub fn as_question_type(self, is_question_type: bool) -> Type {
        match self.kind() {
            TypeKind::Dynamic | TypeKind::Invalid | TypeKind::Null | TypeKind::Void => self,
            _ => match self.data() {
                TypeData::Primary(p) => PrimaryType {
                    is_question_type,
                    ..p
                }
                .into_type(),
                TypeData::Function(f) => FunctionType {
                    is_question_type,
                    ..f
                }
                .into_type(),
                TypeData::Record(r) => RecordType {
                    is_question_type,
                    ..r
                }
                .into_type(),
                TypeData::TypeParameter(t) => TypeParameterType {
                    is_question_type,
                    ..t
                }
                .into_type(),
                TypeData::Unknown(_) => UnknownType { is_question_type }.into_type(),
            },
        }
    }

    /// Finds the nearest type that doesn't involve the unknown type (`_`).
    ///
    /// If `covariant` is `true`, a supertype will be returned (replacing `_`
    /// with `Object?`); otherwise a subtype will be returned (replacing `_`
    /// with `Never`).
    pub fn closure_with_respect_to_unknown(self, covariant: bool) -> Option<Type> {
        match self.kind() {
            TypeKind::Dynamic
            | TypeKind::Invalid
            | TypeKind::Never
            | TypeKind::Null
            | TypeKind::Void => None,
            TypeKind::FutureOr => {
                let p = self.as_primary_type().unwrap();
                let new_arg = p.args[0].closure_with_respect_to_unknown(covariant)?;
                Some(FutureOrType::with_question_type(
                    new_arg,
                    p.is_question_type,
                ))
            }
            TypeKind::Primary => {
                let p = self.as_primary_type().unwrap();
                let new_args = map_list(&p.args, |t| t.closure_with_respect_to_unknown(covariant))?;
                Some(
                    PrimaryType {
                        args: new_args,
                        ..p
                    }
                    .into_type(),
                )
            }
            TypeKind::Function => {
                let f = self.as_function_type().unwrap();
                let new_return_type = f.return_type.closure_with_respect_to_unknown(covariant);
                let new_positional_parameters = map_list(&f.positional_parameters, |t| {
                    t.closure_with_respect_to_unknown(!covariant)
                });
                let new_named_parameters = map_list(&f.named_parameters, |p| {
                    let new_type = p.ty.closure_with_respect_to_unknown(!covariant)?;
                    Some(NamedFunctionParameter { ty: new_type, ..*p })
                });
                if new_return_type.is_none()
                    && new_positional_parameters.is_none()
                    && new_named_parameters.is_none()
                {
                    return None;
                }
                Some(
                    FunctionType {
                        return_type: new_return_type.unwrap_or(f.return_type),
                        positional_parameters: new_positional_parameters
                            .unwrap_or(f.positional_parameters),
                        named_parameters: new_named_parameters.unwrap_or(f.named_parameters),
                        ..f
                    }
                    .into_type(),
                )
            }
            TypeKind::Record => {
                let r = self.as_record_type().unwrap();
                let new_positional = map_list(&r.positional_types, |t| {
                    t.closure_with_respect_to_unknown(covariant)
                });
                let new_named = map_list(&r.named_types, |n| {
                    let new_type = n.ty.closure_with_respect_to_unknown(covariant)?;
                    Some(NamedType {
                        name: n.name,
                        ty: new_type,
                    })
                });
                if new_positional.is_none() && new_named.is_none() {
                    return None;
                }
                Some(
                    RecordType {
                        positional_types: new_positional.unwrap_or(r.positional_types),
                        named_types: new_named.unwrap_or(r.named_types),
                        is_question_type: r.is_question_type,
                    }
                    .into_type(),
                )
            }
            TypeKind::TypeParameter => {
                let t = self.as_type_parameter_type().unwrap();
                let new_promotion = t.promotion?.closure_with_respect_to_unknown(covariant)?;
                Some(
                    TypeParameterType {
                        promotion: Some(new_promotion),
                        ..t
                    }
                    .into_type(),
                )
            }
            TypeKind::Unknown => Some(if covariant {
                Type::parse("Object?")
            } else {
                NeverType::instance()
            }),
        }
    }

    /// Recursively visits `self`, gathering up all the identifiers that
    /// appear in it, and adds them to the set `identifiers`.
    ///
    /// This method is intended to aid in choosing safe names for
    /// substitutions. For example, it can be used to determine that in a
    /// type like `T Function<U>(U)`, it's not safe to rename the type
    /// variable `U` to `T`, since that would conflict with an existing use of
    /// `T`.
    ///
    /// To lower the risk of confusion, it is generous in which identifiers it
    /// reports. For example, in the type `void Function<T>({T X})`, it
    /// reports `X` as a used identifier. This is because even though it
    /// would technically be safe to rename the type variable `T` to `X`, to
    /// do so would be result in a confusing type.
    pub fn gather_used_identifiers(self, identifiers: &mut HashSet<String>) {
        match self.data() {
            TypeData::Primary(p) => {
                identifiers.insert(p.name().to_string());
                for arg in p.args {
                    arg.gather_used_identifiers(identifiers);
                }
            }
            TypeData::Function(f) => {
                f.return_type.gather_used_identifiers(identifiers);
                for positional_parameter in f.positional_parameters {
                    positional_parameter.gather_used_identifiers(identifiers);
                }
                for type_formal in f.type_parameters_shared {
                    identifiers.insert(type_formal.name().to_string());
                    if let Some(bound) = type_formal.explicit_bound() {
                        bound.gather_used_identifiers(identifiers);
                    }
                }
                for named_parameter in f.named_parameters {
                    // As explained in the documentation for
                    // `Type.gatherUsedIdentifiers`, to reduce the risk of
                    // confusion, this method is generous in which identifiers
                    // it reports. So report `namedParameter.name` even though
                    // it's not strictly necessary.
                    identifiers.insert(named_parameter.name.to_string());
                    named_parameter.ty.gather_used_identifiers(identifiers);
                }
            }
            TypeData::Record(r) => {
                for ty in r.positional_types {
                    ty.gather_used_identifiers(identifiers);
                }
                for named_type in r.named_types {
                    // As explained in the documentation for
                    // `Type.gatherUsedIdentifiers`, to reduce the risk of
                    // confusion, this method is generous in which identifiers
                    // it reports. So report `namedType.name` even though it's
                    // not strictly necessary.
                    identifiers.insert(named_type.name.to_string());
                    named_type.ty.gather_used_identifiers(identifiers);
                }
            }
            TypeData::TypeParameter(t) => {
                identifiers.insert(t.type_parameter.name().to_string());
                if let Some(promotion) = t.promotion {
                    promotion.gather_used_identifiers(identifiers);
                }
            }
            TypeData::Unknown(_) => {}
        }
    }

    /// `getDisplayString`.
    pub fn get_display_string(self) -> String {
        self.to_string()
    }

    /// `isStructurallyEqualTo`: `'$this' == '$other'`.
    pub fn is_structurally_equal_to(self, other: Type) -> bool {
        self.to_string() == other.to_string()
    }

    /// Finds the nearest type that doesn't involve any type parameter
    /// promotion. If `covariant` is `true`, a supertype will be returned
    /// (replacing promoted type parameters with their unpromoted
    /// counterparts); otherwise a subtype will be returned (replacing
    /// promoted type parameters with `Never`).
    ///
    /// Returns `None` if this type is already free from type promotion.
    pub fn recursively_demote(self, covariant: bool) -> Option<Type> {
        match self.kind() {
            TypeKind::Dynamic
            | TypeKind::Invalid
            | TypeKind::Never
            | TypeKind::Null
            | TypeKind::Void => None,
            TypeKind::FutureOr => {
                let p = self.as_primary_type().unwrap();
                let new_arg = p.args[0].recursively_demote(covariant)?;
                Some(FutureOrType::with_question_type(
                    new_arg,
                    p.is_question_type,
                ))
            }
            TypeKind::Primary => {
                let p = self.as_primary_type().unwrap();
                let new_args = map_list(&p.args, |t| t.recursively_demote(covariant))?;
                Some(
                    PrimaryType {
                        args: new_args,
                        ..p
                    }
                    .into_type(),
                )
            }
            TypeKind::Function => {
                let f = self.as_function_type().unwrap();
                let new_return_type = f.return_type.recursively_demote(covariant);
                let new_positional_parameters = map_list(&f.positional_parameters, |t| {
                    t.recursively_demote(!covariant)
                });
                let new_named_parameters = map_list(&f.named_parameters, |p| {
                    let new_type = p.ty.recursively_demote(!covariant)?;
                    Some(NamedFunctionParameter { ty: new_type, ..*p })
                });
                if new_return_type.is_none()
                    && new_positional_parameters.is_none()
                    && new_named_parameters.is_none()
                {
                    return None;
                }
                Some(
                    FunctionType {
                        return_type: new_return_type.unwrap_or(f.return_type),
                        positional_parameters: new_positional_parameters
                            .unwrap_or(f.positional_parameters),
                        named_parameters: new_named_parameters.unwrap_or(f.named_parameters),
                        ..f
                    }
                    .into_type(),
                )
            }
            TypeKind::Record => {
                let r = self.as_record_type().unwrap();
                let new_positional =
                    map_list(&r.positional_types, |t| t.recursively_demote(covariant));
                let new_named = map_list(&r.named_types, |n| {
                    let new_type = n.ty.recursively_demote(covariant)?;
                    Some(NamedType {
                        name: n.name,
                        ty: new_type,
                    })
                });
                if new_positional.is_none() && new_named.is_none() {
                    return None;
                }
                Some(
                    RecordType {
                        positional_types: new_positional.unwrap_or(r.positional_types),
                        named_types: new_named.unwrap_or(r.named_types),
                        is_question_type: r.is_question_type,
                    }
                    .into_type(),
                )
            }
            TypeKind::TypeParameter => {
                let t = self.as_type_parameter_type().unwrap();
                if !covariant {
                    Some(NeverType::instance().as_question_type(t.is_question_type))
                } else if t.promotion.is_none() {
                    None
                } else {
                    Some(
                        TypeParameterType {
                            type_parameter: t.type_parameter,
                            promotion: None,
                            is_question_type: t.is_question_type,
                        }
                        .into_type(),
                    )
                }
            }
            TypeKind::Unknown => None,
        }
    }

    /// If `self` contains any references to a [`TypeParameter`] matching one
    /// of the keys in `substitution`, returns a clone of `self` with those
    /// references replaced by the corresponding value. Otherwise returns
    /// `None`.
    ///
    /// For example, if `t` is a reference to the [`TypeParameter`] object
    /// representing `T`, then `Type('Map<T, U>').substitute({t:
    /// Type('int')})` returns a [`Type`] object representing `Map<int, U>`.
    pub fn substitute(self, substitution: &HashMap<TypeParameter, Type>) -> Option<Type> {
        match self.kind() {
            TypeKind::Dynamic
            | TypeKind::Invalid
            | TypeKind::Never
            | TypeKind::Null
            | TypeKind::Void => None,
            TypeKind::FutureOr => {
                let p = self.as_primary_type().unwrap();
                let new_arg = p.args[0].substitute(substitution)?;
                Some(FutureOrType::with_question_type(
                    new_arg,
                    p.is_question_type,
                ))
            }
            TypeKind::Primary => {
                let p = self.as_primary_type().unwrap();
                let new_args = map_list(&p.args, |t| t.substitute(substitution))?;
                Some(
                    PrimaryType {
                        args: new_args,
                        ..p
                    }
                    .into_type(),
                )
            }
            TypeKind::Function => self.substitute_function_type(substitution, false),
            TypeKind::Record => {
                let r = self.as_record_type().unwrap();
                let new_positional_types =
                    map_list(&r.positional_types, |t| t.substitute(substitution));
                let new_named_types = map_list(&r.named_types, |n| n.substitute(substitution));
                if new_positional_types.is_none() && new_named_types.is_none() {
                    return None;
                }
                Some(
                    RecordType {
                        positional_types: new_positional_types.unwrap_or(r.positional_types),
                        named_types: new_named_types.unwrap_or(r.named_types),
                        is_question_type: r.is_question_type,
                    }
                    .into_type(),
                )
            }
            TypeKind::TypeParameter => {
                let t = self.as_type_parameter_type().unwrap();
                substitution.get(&t.type_parameter).copied()
            }
            TypeKind::Unknown => None,
        }
    }

    /// `FunctionType.substitute(substitution, {dropTypeFormals})`. Panics if
    /// `self` is not a function type.
    pub fn substitute_function_type(
        self,
        substitution: &HashMap<TypeParameter, Type>,
        drop_type_formals: bool,
    ) -> Option<Type> {
        let f = self
            .as_function_type()
            .expect("substitute_function_type: not a FunctionType");
        let mut substitution = substitution.clone();
        let mut new_type_formals: Option<Vec<TypeParameter>> = None;
        if !f.type_parameters_shared.is_empty() {
            if drop_type_formals {
                new_type_formals = Some(Vec::new());
            } else {
                // Check if any of the type formal bounds will be changed by the
                // substitution.
                if f.type_parameters_shared.iter().any(|type_formal| {
                    type_formal
                        .explicit_bound()
                        .and_then(|bound| bound.substitute(&substitution))
                        .is_some()
                }) {
                    // Yes, at least one of the type formal bounds will be changed by the
                    // substitution. So that type formal will have to be replaced by a
                    // fresh one. Since type formal bounds can refer to other type
                    // formals, other type formals might need to be replaced by fresh ones
                    // too. To make things easier, go ahead and replace all the type
                    // formals. Also, extend the substitution so that any references to
                    // old type formals will be replaced by references to the new type
                    // formals.
                    let mut formals = Vec::new();
                    for type_formal in &f.type_parameters_shared {
                        let new_type_formal = TypeParameter::new_unregistered(type_formal.name());
                        formals.push(new_type_formal);
                        substitution.insert(*type_formal, TypeParameterType::new(new_type_formal));
                    }
                    // Now that the substitution has been created, fix up all the bounds.
                    for i in 0..f.type_parameters_shared.len() {
                        if let Some(bound) = f.type_parameters_shared[i].explicit_bound() {
                            formals[i].set_explicit_bound(Some(
                                bound.substitute(&substitution).unwrap_or(bound),
                            ));
                        }
                    }
                    new_type_formals = Some(formals);
                }
            }
        }

        let new_return_type = f.return_type.substitute(&substitution);
        let new_positional_parameters =
            map_list(&f.positional_parameters, |t| t.substitute(&substitution));
        let new_named_parameters = map_list(&f.named_parameters, |p| p.substitute(&substitution));
        if new_return_type.is_none()
            && new_positional_parameters.is_none()
            && new_type_formals.is_none()
            && new_named_parameters.is_none()
        {
            None
        } else {
            Some(
                FunctionType {
                    return_type: new_return_type.unwrap_or(f.return_type),
                    positional_parameters: new_positional_parameters
                        .unwrap_or(f.positional_parameters),
                    type_parameters_shared: new_type_formals.unwrap_or(f.type_parameters_shared),
                    required_positional_parameter_count: f.required_positional_parameter_count,
                    named_parameters: new_named_parameters.unwrap_or(f.named_parameters),
                    is_question_type: f.is_question_type,
                }
                .into_type(),
            )
        }
    }

    /// Returns a string representation of this type (Dart `toString`).
    ///
    /// If `parenthesize_if_complex` is `true`, then the result will be
    /// surrounded by parenthesis if it takes any of the following forms:
    /// - A type with a trailing `?` or `*`
    /// - A function type (e.g. `void Function()`)
    /// - A promoted type variable type (e.g. `T&int`)
    pub fn to_string_with(self, parenthesize_if_complex: bool) -> String {
        if self.is_question_type() {
            parenthesize_if(
                parenthesize_if_complex,
                format!("{}?", self.to_string_without_suffix(true)),
            )
        } else {
            self.to_string_without_suffix(parenthesize_if_complex)
        }
    }

    /// Returns a string representation of the portion of this string that
    /// precedes the nullability suffix.
    fn to_string_without_suffix(self, parenthesize_if_complex: bool) -> String {
        let join = |types: &[Type]| {
            types
                .iter()
                .map(|t| t.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        match self.data() {
            TypeData::Primary(p) => {
                if p.args.is_empty() {
                    p.name().to_string()
                } else {
                    format!("{}<{}>", p.name(), join(&p.args))
                }
            }
            TypeData::Function(f) => {
                let mut formals = String::new();
                if !f.type_parameters_shared.is_empty() {
                    let mut formal_strings = Vec::new();
                    for type_formal in &f.type_parameters_shared {
                        if let Some(bound) = type_formal.explicit_bound() {
                            formal_strings.push(format!(
                                "{} extends {}",
                                type_formal.name(),
                                bound
                            ));
                        } else {
                            formal_strings.push(type_formal.name().to_string());
                        }
                    }
                    formals = format!("<{}>", formal_strings.join(", "));
                }
                let mut parameters: Vec<String> = f.positional_parameters
                    [..f.required_positional_parameter_count]
                    .iter()
                    .map(|t| t.to_string())
                    .collect();
                if f.required_positional_parameter_count < f.positional_parameters.len() {
                    let optional_positional_parameters =
                        &f.positional_parameters[f.required_positional_parameter_count..];
                    parameters.push(format!("[{}]", join(optional_positional_parameters)));
                }
                if !f.named_parameters.is_empty() {
                    parameters.push(format!(
                        "{{{}}}",
                        f.named_parameters
                            .iter()
                            .map(|p| p.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                parenthesize_if(
                    parenthesize_if_complex,
                    format!(
                        "{} Function{}({})",
                        f.return_type,
                        formals,
                        parameters.join(", ")
                    ),
                )
            }
            TypeData::Record(r) => {
                let positional_str = join(&r.positional_types);
                let named_str = r
                    .named_types
                    .iter()
                    .map(|e| format!("{} {}", e.ty, e.name))
                    .collect::<Vec<_>>()
                    .join(", ");
                if !named_str.is_empty() {
                    if !r.positional_types.is_empty() {
                        format!("({positional_str}, {{{named_str}}})")
                    } else {
                        format!("({{{named_str}}})")
                    }
                } else if r.positional_types.len() == 1 {
                    format!("({positional_str},)")
                } else {
                    format!("({positional_str})")
                }
            }
            TypeData::TypeParameter(t) => {
                if let Some(promotion) = t.promotion {
                    parenthesize_if(
                        parenthesize_if_complex,
                        format!(
                            "{}&{}",
                            t.type_parameter.name(),
                            promotion.to_string_with(true)
                        ),
                    )
                } else {
                    t.type_parameter.name().to_string()
                }
            }
            TypeData::Unknown(_) => "_".to_string(),
        }
    }

    /// Dart `hashCode`, consistent with `==`.
    ///
    /// The values differ from Dart's, but the contract is the same: equal
    /// types have equal hash codes. A type parameter type hashes the name of
    /// its type parameter (Dart hashes the name and the bound); generic
    /// function types are hashed after renaming their type formals to a
    /// consistent set of names, as in Dart.
    pub fn hash_code(self) -> u64 {
        let mut h = DefaultHasher::new();
        match self.data() {
            TypeData::Primary(p) => {
                0u8.hash(&mut h);
                p.name_info.hash(&mut h);
                for arg in &p.args {
                    arg.hash_code().hash(&mut h);
                }
                p.is_question_type.hash(&mut h);
            }
            TypeData::Function(f) => {
                if !f.type_parameters_shared.is_empty() {
                    // Generic function types need to have the same hash if they are the same
                    // after renaming of type formals. To ensure this, we rename the type
                    // formals to a consistent sent of names and then hash the result.
                    //
                    // Note that it's essential *not* to call
                    // `FreshTypeParameterGenerator.excludeNamesUsedIn` here, to ensure that
                    // a consistent set of type parameter names is generated regardless of the
                    // the names used in the function type.
                    let mut fresh_type_parameter_generator = FreshTypeParameterGenerator::new();
                    let substitution: HashMap<TypeParameter, Type> = f
                        .type_parameters_shared
                        .iter()
                        .map(|type_formal| {
                            (
                                *type_formal,
                                TypeParameterType::new(fresh_type_parameter_generator.generate()),
                            )
                        })
                        .collect();
                    return self
                        .substitute_function_type(&substitution, true)
                        .unwrap()
                        .hash_code();
                }
                1u8.hash(&mut h);
                f.return_type.hash_code().hash(&mut h);
                for p in &f.positional_parameters {
                    p.hash_code().hash(&mut h);
                }
                f.required_positional_parameter_count.hash(&mut h);
                for p in &f.named_parameters {
                    p.name.hash(&mut h);
                    p.ty.hash_code().hash(&mut h);
                    p.is_required.hash(&mut h);
                }
                f.is_question_type.hash(&mut h);
            }
            TypeData::Record(r) => {
                2u8.hash(&mut h);
                for p in &r.positional_types {
                    p.hash_code().hash(&mut h);
                }
                for n in &r.named_types {
                    n.name.hash(&mut h);
                    n.ty.hash_code().hash(&mut h);
                }
                r.is_question_type.hash(&mut h);
            }
            TypeData::TypeParameter(t) => {
                3u8.hash(&mut h);
                t.type_parameter.name().hash(&mut h);
                t.promotion.map(|p| p.hash_code()).hash(&mut h);
                t.is_question_type.hash(&mut h);
            }
            TypeData::Unknown(u) => {
                4u8.hash(&mut h);
                u.is_question_type.hash(&mut h);
            }
        }
        h.finish()
    }

    /// Dart `operator ==`.
    fn dart_equals(self, other: Type) -> bool {
        if self.identical(other) {
            return true;
        }
        match (self.data(), other.data()) {
            (TypeData::Primary(a), TypeData::Primary(b)) => {
                a.name_info == b.name_info
                    && list_equals(&a.args, &b.args)
                    && a.is_question_type == b.is_question_type
            }
            (TypeData::Function(a), TypeData::Function(b)) => {
                if a.type_parameters_shared.len() != b.type_parameters_shared.len() {
                    return false;
                }
                if !a.type_parameters_shared.is_empty() {
                    // Check if types are equal under a consistent renaming of type formals
                    let mut fresh_type_parameter_generator = FreshTypeParameterGenerator::new();
                    fresh_type_parameter_generator.exclude_names_used_in(self);
                    fresh_type_parameter_generator.exclude_names_used_in(other);
                    let mut this_substitution = HashMap::new();
                    let mut other_substitution = HashMap::new();
                    let mut this_type_formal_bounds = Vec::new();
                    let mut other_type_formal_bounds = Vec::new();
                    for i in 0..a.type_parameters_shared.len() {
                        let fresh_type_parameter_type =
                            TypeParameterType::new(fresh_type_parameter_generator.generate());
                        this_substitution
                            .insert(a.type_parameters_shared[i], fresh_type_parameter_type);
                        other_substitution
                            .insert(b.type_parameters_shared[i], fresh_type_parameter_type);
                        this_type_formal_bounds.push(a.type_parameters_shared[i].bound());
                        other_type_formal_bounds.push(b.type_parameters_shared[i].bound());
                    }
                    let this_bounds = map_list(&this_type_formal_bounds, |t| {
                        t.substitute(&this_substitution)
                    })
                    .unwrap_or(this_type_formal_bounds);
                    let other_bounds = map_list(&other_type_formal_bounds, |t| {
                        t.substitute(&other_substitution)
                    })
                    .unwrap_or(other_type_formal_bounds);
                    list_equals(&this_bounds, &other_bounds)
                        && self
                            .substitute_function_type(&this_substitution, true)
                            .unwrap()
                            == other
                                .substitute_function_type(&other_substitution, true)
                                .unwrap()
                } else {
                    a.return_type == b.return_type
                        && list_equals(&a.positional_parameters, &b.positional_parameters)
                        && a.required_positional_parameter_count
                            == b.required_positional_parameter_count
                        && a.named_parameters == b.named_parameters
                        && a.is_question_type == b.is_question_type
                }
            }
            (TypeData::Record(a), TypeData::Record(b)) => {
                list_equals(&a.positional_types, &b.positional_types)
                    && a.named_types == b.named_types
                    && a.is_question_type == b.is_question_type
            }
            (TypeData::TypeParameter(a), TypeData::TypeParameter(b)) => {
                a.type_parameter == b.type_parameter
                    && a.promotion == b.promotion
                    && a.is_question_type == b.is_question_type
            }
            (TypeData::Unknown(a), TypeData::Unknown(b)) => {
                a.is_question_type == b.is_question_type
            }
            _ => false,
        }
    }
}

impl PartialEq for Type {
    fn eq(&self, other: &Type) -> bool {
        self.dart_equals(*other)
    }
}

impl Eq for Type {}

impl Hash for Type {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash_code());
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string_with(false))
    }
}

impl fmt::Debug for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Type({self})")
    }
}

// ================================================ FreshTypeParameterGenerator

/// Factory for creating fresh type parameters.
///
/// Generated type parameters will have names of the form `Tn`, where `n` is
/// a small non-negative integer.
#[derive(Default)]
pub struct FreshTypeParameterGenerator {
    names_to_exclude: HashSet<String>,
    counter: usize,
}

impl FreshTypeParameterGenerator {
    /// `FreshTypeParameterGenerator()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Ensures that when [`generate`](Self::generate) is called, the type
    /// parameter it returns will have a name that's distinct from all
    /// identifiers in `ty`.
    pub fn exclude_names_used_in(&mut self, ty: Type) {
        ty.gather_used_identifiers(&mut self.names_to_exclude);
    }

    /// Generates a fresh type parameter.
    pub fn generate(&mut self) -> TypeParameter {
        loop {
            let name = format!("T{}", self.counter);
            self.counter += 1;
            if self.names_to_exclude.insert(name.clone()) {
                return TypeParameter::new_unregistered(&name);
            }
        }
    }
}

// ================================================================ TypeRegistry

/// Container for static methods that can be used to customize the "mini
/// types" representation used in `_fe_analyzer_shared` unit tests.
///
/// Thanks to Dart's scoping rules, it's possible for a single identifier to
/// represent an interface type in some contexts, a special type like `Null`
/// in other contexts, and a type parameter name in other contexts. But
/// allowing a single name to have multiple meanings isn't useful in
/// `_fe_analyzer_shared` unit tests, and opens up greater risk of confusion.
/// Therefore, the "mini types" representation does not permit it; every test
/// must register each type name it intends to use, specifying its meaning,
/// before using that name in a call to [`Type::parse`]. This registration can
/// happen either within the test itself or in a set-up helper.
pub struct TypeRegistry;

impl TypeRegistry {
    /// The [`TypeNameInfo`] object representing the special type `dynamic`.
    pub fn dynamic_() -> TypeNameInfo {
        DYNAMIC_NAME
    }

    /// The [`TypeNameInfo`] object representing the special type `error`.
    pub fn error_() -> TypeNameInfo {
        ERROR_NAME
    }

    /// The [`TypeNameInfo`] object representing the interface type `Future`.
    pub fn future() -> TypeNameInfo {
        FUTURE_NAME
    }

    /// The [`TypeNameInfo`] object representing the special type `FutureOr`.
    pub fn future_or() -> TypeNameInfo {
        FUTURE_OR_NAME
    }

    /// The [`TypeNameInfo`] object representing the interface type
    /// `Iterable`.
    pub fn iterable() -> TypeNameInfo {
        ITERABLE_NAME
    }

    /// The [`TypeNameInfo`] object representing the interface type `List`.
    pub fn list() -> TypeNameInfo {
        LIST_NAME
    }

    /// The [`TypeNameInfo`] object representing the interface type `Map`.
    pub fn map() -> TypeNameInfo {
        MAP_NAME
    }

    /// The [`TypeNameInfo`] object representing the special type `Never`.
    pub fn never() -> TypeNameInfo {
        NEVER_NAME
    }

    /// The [`TypeNameInfo`] object representing the special type `Null`.
    pub fn null_() -> TypeNameInfo {
        NULL_NAME
    }

    /// The [`TypeNameInfo`] object representing the interface type `Stream`.
    pub fn stream() -> TypeNameInfo {
        STREAM_NAME
    }

    /// The [`TypeNameInfo`] object representing the special type `void`.
    pub fn void_() -> TypeNameInfo {
        VOID_NAME
    }

    /// Registers `name` as the name of an ordinary interface type.
    pub fn add_interface_type_name(name: &str) -> TypeNameInfo {
        let interface_type_name = TypeNameInfo(alloc_name(name, NameKind::Interface));
        Self::add(interface_type_name);
        interface_type_name
    }

    /// Registers `name` as the name of a type parameter.
    pub fn add_type_parameter(name: &str) -> TypeParameter {
        let type_parameter = TypeParameter::new_unregistered(name);
        Self::add(type_parameter.name_info());
        type_parameter
    }

    /// Initializes the "mini type" infrastructure.
    ///
    /// Must be called before any test code that makes use of mini types (Dart
    /// calls it from a test `setUp` callback).
    pub fn init() {
        ARENA.with(|a| {
            let mut a = a.borrow_mut();
            if a.registry.is_some() {
                panic!(
                    "init() already called. Did you forget to call uninit() from \
                     `tearDown`?"
                );
            }
            a.registry = Some(HashMap::new());
        });
        // Set up some common built-in type names.
        Self::add_interface_type_name("bool");
        Self::add_interface_type_name("double");
        Self::add(DYNAMIC_NAME);
        Self::add(ERROR_NAME);
        Self::add(FUTURE_NAME);
        Self::add(FUTURE_OR_NAME);
        Self::add_interface_type_name("int");
        Self::add(ITERABLE_NAME);
        Self::add(LIST_NAME);
        Self::add(MAP_NAME);
        Self::add(NEVER_NAME);
        Self::add(NULL_NAME);
        Self::add_interface_type_name("num");
        Self::add_interface_type_name("Object");
        Self::add(STREAM_NAME);
        Self::add_interface_type_name("String");
        Self::add_interface_type_name("StackTrace");
        Self::add(VOID_NAME);
    }

    /// Retrieves the [`TypeNameInfo`] corresponding to `name`.
    pub fn lookup(name: &str) -> TypeNameInfo {
        ARENA.with(|a| {
            let a = a.borrow();
            let map = a.registry.as_ref().unwrap_or_else(|| {
                panic!(
                    "TypeRegistry not initialized (call `TypeRegistry.init` from a test \
                     `setUp` callback)"
                )
            });
            *map.get(name).unwrap_or_else(|| {
                panic!("Unknown type name {name} (use `TypeRegistry.add...` first)")
            })
        })
    }

    /// Un-does the operation of [`init`](Self::init), rendering the "mini
    /// type" infrastructure unusable.
    pub fn uninit() {
        // Note: don't complain if `_typeNameInfoMap` is `null`, because we don't
        // want to produce confusing failure messages if a test runs into trouble
        // while trying to initialize itself.
        ARENA.with(|a| a.borrow_mut().registry = None);
    }

    /// Registers `info` as information about a type name.
    fn add(info: TypeNameInfo) {
        let name = info.name();
        ARENA.with(|a| {
            let mut a = a.borrow_mut();
            let info_map = a.registry.as_mut().unwrap_or_else(|| {
                panic!(
                    "TypeRegistry not initialized (call `TypeRegistry.init` from a test \
                     `setUp` callback)"
                )
            });
            if info_map.contains_key(name) {
                panic!("Type name {name} already registered");
            }
            info_map.insert(name.to_string(), info);
        });
    }
}

/// Calls [`TypeRegistry::init`] and returns a guard that calls
/// [`TypeRegistry::uninit`] when dropped (Dart `setUp` / `tearDown`).
#[must_use]
pub fn type_registry_scope() -> TypeRegistryScope {
    TypeRegistry::init();
    TypeRegistryScope(())
}

/// Guard returned by [`type_registry_scope`].
pub struct TypeRegistryScope(());

impl Drop for TypeRegistryScope {
    fn drop(&mut self) {
        TypeRegistry::uninit();
    }
}

// ================================================================== TypeSystem

/// A super-interface template: maps the type arguments of a class to its
/// super-interfaces (Dart `List<Type> Function(List<Type>)`).
pub type SuperInterfaceTemplate = Rc<dyn Fn(&[Type]) -> Vec<Type>>;

/// The subtype relation and related queries of mini types.
pub struct TypeSystem {
    super_interface_templates: HashMap<String, SuperInterfaceTemplate>,
}

impl Default for TypeSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeSystem {
    /// `TypeSystem()`, with `_coreSuperInterfaceTemplates`.
    pub fn new() -> Self {
        let mut templates: HashMap<String, SuperInterfaceTemplate> = HashMap::new();
        let mut add = |name: &str, template: SuperInterfaceTemplate| {
            templates.insert(name.to_string(), template);
        };
        add("bool", Rc::new(|_| vec![t("Object")]));
        add("double", Rc::new(|_| vec![t("num"), t("Object")]));
        add("Future", Rc::new(|_| vec![t("Object")]));
        add("int", Rc::new(|_| vec![t("num"), t("Object")]));
        add("Iterable", Rc::new(|_| vec![t("Object")]));
        add(
            "List",
            Rc::new(|args| vec![PrimaryType::new(ITERABLE_NAME, args.to_vec()), t("Object")]),
        );
        add("Map", Rc::new(|_| vec![t("Object")]));
        add("Object", Rc::new(|_| vec![]));
        add("num", Rc::new(|_| vec![t("Object")]));
        add("StackTrace", Rc::new(|_| vec![t("Object")]));
        add("String", Rc::new(|_| vec![t("Object")]));
        TypeSystem {
            super_interface_templates: templates,
        }
    }

    /// `addSuperInterfaces`.
    pub fn add_super_interfaces(
        &mut self,
        class_name: &str,
        template: impl Fn(&[Type]) -> Vec<Type> + 'static,
    ) {
        self.super_interface_templates
            .insert(class_name.to_string(), Rc::new(template));
    }

    /// If `t` derives a future type `F` (as defined in the "Function
    /// Expressions" section of the language spec), returns `F`. Otherwise
    /// returns `None`.
    pub fn derived_future_type(&self, t: Type) -> Option<Type> {
        // (Note: comments below are pulled from the definition of "derives a future
        // type" in the "Function Expressions" section of the language spec.)

        // We say that a type T derives a future type F in the following cases,
        // using the first applicable case:
        // - If T is a type which is introduced by a class, mixin, or enum
        //   declaration, and if T or a direct or indirect superinterface of T is
        //   Future<U> for some U, then T derives the future type Future<U>.
        if let Some(p) = t.as_primary_type()
            && p.is_interface_type()
            && !p.is_question_type
        {
            let mut candidates = vec![t];
            candidates.extend(self.get_super_interfaces(&p));
            for f in candidates {
                if let Some(fp) = f.as_primary_type()
                    && fp.name() == "Future"
                {
                    return Some(f);
                }
            }
        }

        // - If T is the type FutureOr<U> for some U, then T derives the future type
        //   FutureOr<U>.
        if t.kind() == TypeKind::FutureOr {
            return Some(t);
        }

        // - If T is S? for some S, and S derives the future type F, then T derives
        //   the future type F?.
        if t.is_question_type()
            && let Some(f) = self.derived_future_type(t.as_question_type(false))
        {
            return Some(f.as_question_type(true));
        }

        // - If T is a type variable with bound B, and B derives the future type F,
        //   then T derives the future type F.
        if let Some(tp) = t.as_type_parameter_type()
            && let Some(f) = self.derived_future_type(tp.bound())
        {
            return Some(f);
        }

        None
    }

    /// `factor(T, S)`.
    pub fn factor(&self, t: Type, s: Type) -> Type {
        // If T <: S then Never
        if self.is_subtype(t, s) {
            return NeverType::instance();
        }

        // Else if T is R? and Null <: S then factor(R, S)
        if t.is_question_type() && self.is_subtype(NullType::instance(), s) {
            return self.factor(t.as_question_type(false), s);
        }

        // Else if T is R? then factor(R, S)?
        if t.is_question_type() {
            return self
                .factor(t.as_question_type(false), s)
                .as_question_type(true);
        }

        // Else if T is FutureOr<R> and Future<R> <: S then factor(R, S)
        if let Some(r) = t.future_or_type_argument()
            && self.is_subtype(PrimaryType::new(FUTURE_NAME, vec![r]), s)
        {
            return self.factor(r, s);
        }

        // Else if T is FutureOr<R> and R <: S then factor(Future<R>, S)
        if let Some(r) = t.future_or_type_argument()
            && self.is_subtype(r, s)
        {
            return self.factor(PrimaryType::new(FUTURE_NAME, vec![r]), s);
        }

        // Else T
        t
    }

    /// `isSubtype(T0, T1)`.
    pub fn is_subtype(&self, t0: Type, t1: Type) -> bool {
        let object_question_type = || t("Object?");
        let object_type = || t("Object");
        let k0 = t0.kind();
        let k1 = t1.kind();
        let p0 = t0.as_primary_type();
        let p1 = t1.as_primary_type();
        let v0 = t0.as_type_parameter_type();
        let v1 = t1.as_type_parameter_type();

        // Reflexivity: if T0 and T1 are the same type then T0 <: T1
        //
        // - Note that this check is necessary as the base case for primitive types,
        //   and type variables but not for composite types.  We only check it for
        //   types with a single name and no type arguments (this covers both
        //   primitive types and type variables).
        if k0 == TypeKind::Invalid || k1 == TypeKind::Invalid {
            // `InvalidType` is treated as a top and a bottom type, which is
            // consistent with CFE and analyzer implementations.
            return true;
        }
        if let (Some(a), Some(b)) = (&p0, &p1)
            && !a.is_question_type
            && a.args.is_empty()
            && !b.is_question_type
            && b.args.is_empty()
            && a.name_info == b.name_info
        {
            return true;
        }
        if let (Some(a), Some(b)) = (&v0, &v1)
            && a.promotion.is_none()
            && !a.is_question_type
            && b.promotion.is_none()
            && !b.is_question_type
            && a.type_parameter == b.type_parameter
        {
            return true;
        }

        // Unknown types (note: this is not in the spec, but necessary because there
        // are circumstances where we do subtype tests between types and type
        // schemas): if T0 or T1 is the unknown type then T0 <: T1.
        if k0 == TypeKind::Unknown || k1 == TypeKind::Unknown {
            return true;
        }

        // Right Top: if T1 is a top type (i.e. dynamic, or void, or Object?) then
        // T0 <: T1
        if Self::is_top(t1) {
            return true;
        }

        // Left Top: if T0 is dynamic or void then T0 <: T1 if Object? <: T1
        if k0 == TypeKind::Dynamic || k0 == TypeKind::Void {
            return self.is_subtype(object_question_type(), t1);
        }

        // Left Bottom: if T0 is Never then T0 <: T1
        if k0 == TypeKind::Never && !t0.is_question_type() {
            return true;
        }

        // Right Object: if T1 is Object then:
        if let Some(b) = &p1
            && !b.is_question_type
            && b.args.is_empty()
            && b.name() == "Object"
        {
            // - if T0 is an unpromoted type variable with bound B then T0 <: T1 iff
            //   B <: Object
            if let Some(v) = &v0
                && v.promotion.is_none()
                && !v.is_question_type
            {
                return self.is_subtype(v.bound(), object_type());
            }

            // - if T0 is a promoted type variable X & S then T0 <: T1 iff S <: Object
            if let Some(v) = &v0
                && let (Some(s), false) = (v.promotion, v.is_question_type)
            {
                return self.is_subtype(s, object_type());
            }

            // - if T0 is FutureOr<S> for some S, then T0 <: T1 iff S <: Object.
            if k0 == TypeKind::FutureOr && !t0.is_question_type() {
                return self.is_subtype(t0.future_or_type_argument().unwrap(), object_type());
            }

            // - if T0 is Null, dynamic, void, or S? for any S, then the subtyping
            //   does not hold (per above, the result of the subtyping query is
            //   false).
            if k0 == TypeKind::Null
                || k0 == TypeKind::Dynamic
                || k0 == TypeKind::Void
                || t0.is_question_type()
            {
                return false;
            }

            // - Otherwise T0 <: T1 is true.
            return true;
        }

        // Left Null: if T0 is Null then:
        if k0 == TypeKind::Null {
            // - if T1 is a type variable (promoted or not) the query is false
            if let Some(v) = &v1
                && !v.is_question_type
            {
                return false;
            }

            // - If T1 is FutureOr<S> for some S, then the query is true iff
            //   Null <: S.
            if k1 == TypeKind::FutureOr && !t1.is_question_type() {
                return self
                    .is_subtype(NullType::instance(), t1.future_or_type_argument().unwrap());
            }

            // - If T1 is Null or S? for some S, then the query is true.
            if k1 == TypeKind::Null || t1.is_question_type() {
                return true;
            }

            // - Otherwise, the query is false
            return false;
        }

        // Left FutureOr: if T0 is FutureOr<S0> then:
        if k0 == TypeKind::FutureOr && !t0.is_question_type() {
            let s0 = t0.future_or_type_argument().unwrap();

            // - T0 <: T1 iff Future<S0> <: T1 and S0 <: T1
            return self.is_subtype(PrimaryType::new(FUTURE_NAME, vec![s0]), t1)
                && self.is_subtype(s0, t1);
        }

        // Left Nullable: if T0 is S0? then:
        if t0.is_question_type() {
            // - T0 <: T1 iff S0 <: T1 and Null <: T1
            return self.is_subtype(t0.as_question_type(false), t1)
                && self.is_subtype(NullType::instance(), t1);
        }

        // Type Variable Reflexivity 1: if T0 is a type variable X0 or a promoted
        // type variables X0 & S0 and T1 is X0 then:
        if let (Some(a), Some(b)) = (&v0, &v1)
            && !a.is_question_type
            && b.promotion.is_none()
            && !b.is_question_type
            && a.type_parameter == b.type_parameter
        {
            // - T0 <: T1
            return true;
        }

        // Type Variable Reflexivity 2: if T0 is a type variable X0 or a promoted
        // type variables X0 & S0 and T1 is X0 & S1 then:
        if let (Some(a), Some(b)) = (&v0, &v1)
            && let (false, Some(s1), false) = (a.is_question_type, b.promotion, b.is_question_type)
            && a.type_parameter == b.type_parameter
        {
            // - T0 <: T1 iff T0 <: S1.
            return self.is_subtype(t0, s1);
        }

        // Right Promoted Variable: if T1 is a promoted type variable X1 & S1 then:
        if let Some(b) = &v1
            && let (Some(s1), false) = (b.promotion, b.is_question_type)
        {
            // - T0 <: T1 iff T0 <: X1 and T0 <: S1
            return self.is_subtype(t0, TypeParameterType::new(b.type_parameter))
                && self.is_subtype(t0, s1);
        }

        // Right FutureOr: if T1 is FutureOr<S1> then:
        if k1 == TypeKind::FutureOr && !t1.is_question_type() {
            let s1 = t1.future_or_type_argument().unwrap();

            // - T0 <: T1 iff any of the following hold:
            //   - either T0 <: Future<S1>
            if self.is_subtype(t0, PrimaryType::new(FUTURE_NAME, vec![s1])) {
                return true;
            }
            //   - or T0 <: S1
            if self.is_subtype(t0, s1) {
                return true;
            }
            //   - or T0 is X0 and X0 has bound S0 and S0 <: T1
            if let Some(v) = &v0
                && v.promotion.is_none()
                && self.is_subtype(v.bound(), t1)
            {
                return true;
            }
            //   - or T0 is X0 & S0 and S0 <: T1
            if let Some(v) = &v0
                && let Some(s0) = v.promotion
                && self.is_subtype(s0, t1)
            {
                return true;
            }
            return false;
        }

        // Right Nullable: if T1 is S1? then:
        if t1.is_question_type() {
            let s1 = t1.as_question_type(false);

            // - T0 <: T1 iff any of the following hold:
            //   - either T0 <: S1
            if self.is_subtype(t0, s1) {
                return true;
            }
            //   - or T0 <: Null
            if self.is_subtype(t0, NullType::instance()) {
                return true;
            }
            //   - or T0 is X0 and X0 has bound S0 and S0 <: T1
            if let Some(v) = &v0
                && v.promotion.is_none()
                && self.is_subtype(v.bound(), t1)
            {
                return true;
            }
            //   - or T0 is X0 & S0 and S0 <: T1
            if let Some(v) = &v0
                && let Some(s0) = v.promotion
                && self.is_subtype(s0, t1)
            {
                return true;
            }
            return false;
        }

        // Left Promoted Variable: T0 is a promoted type variable X0 & S0
        if let Some(v) = &v0
            && let Some(s0) = v.promotion
        {
            // - and S0 <: T1
            if self.is_subtype(s0, t1) {
                return true;
            }
        }

        // Left Type Variable Bound: T0 is a type variable X0 with bound B0
        if let Some(v) = &v0
            && v.promotion.is_none()
        {
            // - and B0 <: T1
            if self.is_subtype(v.bound(), t1) {
                return true;
            }
        }

        // Function Type/Function: T0 is a function type and T1 is Function
        if k0 == TypeKind::Function
            && let Some(b) = &p1
            && b.args.is_empty()
            && b.name() == "Function"
        {
            return true;
        }

        // Record Type/Record: T0 is a record type and T1 is Record
        if k0 == TypeKind::Record
            && let Some(b) = &p1
            && b.args.is_empty()
            && b.name() == "Record"
        {
            return true;
        }

        let is_interface_compositionality_subtype = || {
            // Interface Compositionality: T0 is an interface type C0<S0, ..., Sk> and
            // T1 is C0<U0, ..., Uk>
            let (Some(a), Some(b)) = (&p0, &p1) else {
                return false;
            };
            if a.args.len() != b.args.len() || a.name() != b.name() {
                return false;
            }
            // - and each Si <: Ui
            for i in 0..a.args.len() {
                if !self.is_subtype(a.args[i], b.args[i]) {
                    return false;
                }
            }
            true
        };

        if is_interface_compositionality_subtype() {
            return true;
        }

        // Super-Interface: T0 is an interface type with super-interfaces S0,...Sn
        let is_super_interface_subtype = || {
            let Some(a) = &p0 else {
                return false;
            };
            let super_interfaces = self.get_super_interfaces(a);

            // - and Si <: T1 for some i
            for super_interface in super_interfaces {
                if self.is_subtype(super_interface, t1) {
                    return true;
                }
            }
            false
        };

        if is_super_interface_subtype() {
            return true;
        }

        let f0 = t0.as_function_type();
        let f1 = t1.as_function_type();

        let is_positional_function_subtype = || {
            // Positional Function Types: T0 is U0 Function<X0 extends B00, ...,
            // Xk extends B0k>(V0 x0, ..., Vn xn, [Vn+1 xn+1, ..., Vm xm])
            let Some(f0) = &f0 else { return false };
            if !f0.named_parameters.is_empty() {
                return false;
            }
            let n = f0.required_positional_parameter_count;
            let m = f0.positional_parameters.len();

            // - and T1 is U1 Function<Y0 extends B10, ..., Yk extends B1k>(S0 y0,
            //   ..., Sp yp, [Sp+1 yp+1, ..., Sq yq])
            let Some(f1) = &f1 else { return false };
            if !f1.named_parameters.is_empty() {
                return false;
            }
            let p = f1.required_positional_parameter_count;
            let q = f1.positional_parameters.len();

            // - and p >= n
            if p < n {
                return false;
            }

            // - and m >= q
            if m < q {
                return false;
            }

            // (Note: no substitution is needed in the code below; we don't support
            // type arguments on function types)

            // - and Si[Z0/Y0, ..., Zk/Yk] <: Vi[Z0/X0, ..., Zk/Xk] for i in 0...q
            for i in 0..q {
                if !self.is_subtype(f1.positional_parameters[i], f0.positional_parameters[i]) {
                    return false;
                }
            }

            // - and U0[Z0/X0, ..., Zk/Xk] <: U1[Z0/Y0, ..., Zk/Yk]
            if !self.is_subtype(f0.return_type, f1.return_type) {
                return false;
            }

            // - and B0i[Z0/X0, ..., Zk/Xk] === B1i[Z0/Y0, ..., Zk/Yk] for i in 0...k
            // - where the Zi are fresh type variables with bounds B0i[Z0/X0, ...,
            //   Zk/Xk]
            // (No check needed here since we don't support type arguments on function
            // types)
            true
        };

        if is_positional_function_subtype() {
            return true;
        }

        let is_named_function_subtype = || {
            // Named Function Types: T0 is U0 Function<X0 extends B00, ..., Xk extends
            // B0k>(V0 x0, ..., Vn xn, {r0n+1 Vn+1 xn+1, ..., r0m Vm xm}) where r0j is
            // empty or required for j in n+1...m
            let Some(f0) = &f0 else { return false };
            let n = f0.positional_parameters.len();
            if f0.required_positional_parameter_count != n {
                return false;
            }

            // - and T1 is U1 Function<Y0 extends B10, ..., Yk extends B1k>(S0 y0,
            //   ..., Sn yn, {r1n+1 Sn+1 yn+1, ..., r1q Sq yq}) where r1j is empty or
            //   required for j in n+1...q
            let Some(f1) = &f1 else { return false };
            if f1.positional_parameters.len() != n || f1.required_positional_parameter_count != n {
                return false;
            }

            // - and {yn+1, ... , yq} subsetof {xn+1, ... , xm}
            let mut t1_index_to_t0_index = Vec::new();
            let mut i = 0;
            let mut j = 0;
            while i < f0.named_parameters.len() || j < f1.named_parameters.len() {
                if i >= f0.named_parameters.len() {
                    break;
                }
                if j >= f1.named_parameters.len() {
                    return false;
                }
                match f0.named_parameters[i].name.cmp(f1.named_parameters[j].name) {
                    std::cmp::Ordering::Less => i += 1,
                    std::cmp::Ordering::Greater => return false,
                    std::cmp::Ordering::Equal => {
                        t1_index_to_t0_index.push(i);
                        i += 1;
                        j += 1;
                    }
                }
            }

            // (Note: no substitution is needed in the code below; we don't support
            // type arguments on function types)

            // - and Si[Z0/Y0, ..., Zk/Yk] <: Vi[Z0/X0, ..., Zk/Xk] for i in 0...n
            for i in 0..n {
                if !self.is_subtype(f1.positional_parameters[i], f0.positional_parameters[i]) {
                    return false;
                }
            }

            // - and Si[Z0/Y0, ..., Zk/Yk] <: Tj[Z0/X0, ..., Zk/Xk] for i in n+1...q,
            //   yj = xi
            for (j, &i) in t1_index_to_t0_index.iter().enumerate() {
                if !self.is_subtype(f1.named_parameters[j].ty, f0.named_parameters[i].ty) {
                    return false;
                }
            }

            // - and for each j such that r0j is required, then there exists an i in
            //   n+1...q such that xj = yi, and r1i is required
            for (j, &i) in t1_index_to_t0_index.iter().enumerate() {
                if f1.named_parameters[j].is_required && !f0.named_parameters[i].is_required {
                    return false;
                }
            }

            // - and U0[Z0/X0, ..., Zk/Xk] <: U1[Z0/Y0, ..., Zk/Yk]
            if !self.is_subtype(f0.return_type, f1.return_type) {
                return false;
            }

            // - and B0i[Z0/X0, ..., Zk/Xk] === B1i[Z0/Y0, ..., Zk/Yk] for i in 0...k
            // - where the Zi are fresh type variables with bounds B0i[Z0/X0, ...,
            //   Zk/Xk]
            // (No check needed here since we don't support type arguments on function
            // types)
            true
        };

        if is_named_function_subtype() {
            return true;
        }

        // Record Types: T0 is (V0, ..., Vn, {Vn+1 dn+1, ..., Vm dm})
        //
        // - and T1 is (S0, ..., Sn, {Sn+1 dn+1, ..., Sm dm})
        // - and Vi <: Si for i in 0...m
        let is_record_subtype = || {
            let (Some(r0), Some(r1)) = (t0.as_record_type(), t1.as_record_type()) else {
                return false;
            };
            if r0.positional_types.len() != r1.positional_types.len() {
                return false;
            }
            for i in 0..r0.positional_types.len() {
                if !self.is_subtype(r0.positional_types[i], r1.positional_types[i]) {
                    return false;
                }
            }
            if r0.named_types.len() != r1.named_types.len() {
                return false;
            }
            let t1_named_map: HashMap<Name, Type> =
                r1.named_types.iter().map(|n| (n.name, n.ty)).collect();
            for NamedType { name, ty: vi } in &r0.named_types {
                let Some(si) = t1_named_map.get(name) else {
                    return false;
                };
                if !self.is_subtype(*vi, *si) {
                    return false;
                }
            }
            true
        };

        if is_record_subtype() {
            return true;
        }

        false
    }

    fn get_super_interfaces(&self, t: &PrimaryType) -> Vec<Type> {
        let Some(super_interface_template) = self.super_interface_templates.get(t.name()) else {
            let ty = t.clone().into_type();
            panic!("Superinterfaces for {ty} not known");
        };
        super_interface_template(&t.args)
    }

    fn is_top(t: Type) -> bool {
        if t.as_primary_type().is_some() {
            matches!(
                t.kind(),
                TypeKind::Dynamic | TypeKind::Invalid | TypeKind::Void
            )
        } else if t.is_question_type() {
            // Dart: `t is PrimaryType && ...`, which is false here.
            false
        } else {
            false
        }
    }
}

// ===================================================================== parser

/// Representation of a [`Type`] that has been parsed but hasn't had meaning
/// assigned to its identifiers yet.
enum PreType {
    /// `_PreFunctionType`.
    Function {
        return_type: Box<PreType>,
        type_formals: Vec<PreTypeFormal>,
        positional_parameter_types: Vec<PreType>,
        required_positional_parameter_count: usize,
        named_parameters: Vec<PreNamedFunctionParameter>,
    },
    /// `_PrePrimaryType`.
    Primary {
        type_name: String,
        type_args: Vec<PreType>,
    },
    /// `_PrePromotedType`.
    Promoted {
        inner: Box<PreType>,
        promotion: Box<PreType>,
    },
    /// `_PreRecordType`.
    Record {
        positional_types: Vec<PreType>,
        named_types: Vec<PreNamedType>,
    },
    /// `_PreTypeWithNullability`.
    WithNullability {
        inner: Box<PreType>,
        is_question_type: bool,
    },
    /// `_PreUnknownType`.
    Unknown,
}

/// Representation of a named function parameter in a `_PreFunctionType`.
struct PreNamedFunctionParameter {
    name: String,
    ty: PreType,
    is_required: bool,
}

/// Representation of a named component of a `_PreRecordType`.
struct PreNamedType {
    name: String,
    ty: PreType,
}

/// Representation of a formal parameter of a function type that has been
/// parsed but hasn't had meaning assigned to its identifiers yet.
struct PreTypeFormal {
    name: String,
    bound: Option<PreType>,
}

impl PreType {
    /// Translates `self` into a [`Type`].
    ///
    /// The meaning of identifiers in `self` is determined by looking them up
    /// first in `type_formal_scope`, and then, if they are not found, in the
    /// [`TypeRegistry`].
    fn materialize(
        &self,
        type_formal_scope: &HashMap<String, TypeParameter>,
    ) -> Result<Type, ParseError> {
        match self {
            PreType::Function {
                return_type,
                type_formals,
                positional_parameter_types,
                required_positional_parameter_count,
                named_parameters,
            } => {
                let mut materialized_type_formals = Vec::new();
                let mut scope = type_formal_scope.clone();
                if !type_formals.is_empty() {
                    for type_formal in type_formals {
                        let materialized_type_formal =
                            TypeParameter::new_unregistered(&type_formal.name);
                        materialized_type_formals.push(materialized_type_formal);
                        scope.insert(type_formal.name.clone(), materialized_type_formal);
                    }
                    for (i, type_formal) in type_formals.iter().enumerate() {
                        if let Some(bound) = &type_formal.bound {
                            materialized_type_formals[i]
                                .set_explicit_bound(Some(bound.materialize(&scope)?));
                        }
                    }
                }
                let mut positional = Vec::new();
                for positional_parameter_type in positional_parameter_types {
                    positional.push(positional_parameter_type.materialize(&scope)?);
                }
                let mut named = Vec::new();
                for named_parameter in named_parameters {
                    named.push(NamedFunctionParameter::new(
                        named_parameter.is_required,
                        &named_parameter.name,
                        named_parameter.ty.materialize(&scope)?,
                    ));
                }
                Ok(FunctionType {
                    return_type: return_type.materialize(&scope)?,
                    type_parameters_shared: materialized_type_formals,
                    positional_parameters: positional,
                    required_positional_parameter_count: *required_positional_parameter_count,
                    named_parameters: named,
                    is_question_type: false,
                }
                .into_type())
            }
            PreType::Primary {
                type_name,
                type_args,
            } => {
                let name_info = match type_formal_scope.get(type_name) {
                    Some(type_parameter) => type_parameter.name_info(),
                    None => TypeRegistry::lookup(type_name),
                };
                if let Some(type_parameter) = name_info.as_type_parameter() {
                    if !type_args.is_empty() {
                        return parse_error("Type parameter types do not accept type arguments");
                    }
                    return Ok(TypeParameterType::new(type_parameter));
                }
                if name_info.is_interface_type_name() {
                    let mut args = Vec::new();
                    for type_arg in type_args {
                        args.push(type_arg.materialize(type_formal_scope)?);
                    }
                    return Ok(PrimaryType::new(name_info, args));
                }
                let no_args = |what: &str| -> Result<(), ParseError> {
                    if type_args.is_empty() {
                        Ok(())
                    } else {
                        parse_error(format!("`{what}` does not accept type arguments"))
                    }
                };
                match type_name.as_str() {
                    "dynamic" => {
                        no_args("dynamic")?;
                        Ok(DynamicType::instance())
                    }
                    "error" => {
                        no_args("error")?;
                        Ok(InvalidType::instance())
                    }
                    "FutureOr" => {
                        if type_args.len() != 1 {
                            return parse_error("`FutureOr` requires exactly one type argument");
                        }
                        Ok(FutureOrType::new(
                            type_args[0].materialize(type_formal_scope)?,
                        ))
                    }
                    "Never" => {
                        no_args("Never")?;
                        Ok(NeverType::instance())
                    }
                    "Null" => {
                        no_args("Null")?;
                        Ok(NullType::instance())
                    }
                    "void" => {
                        no_args("void")?;
                        Ok(VoidType::instance())
                    }
                    _ => panic!("UnimplementedError: Unknown special type name: {type_name}"),
                }
            }
            PreType::Promoted { inner, promotion } => {
                let ty = inner.materialize(type_formal_scope)?;
                match ty.as_type_parameter_type() {
                    Some(TypeParameterType {
                        type_parameter,
                        promotion: None,
                        ..
                    }) => Ok(TypeParameterType {
                        type_parameter,
                        promotion: Some(promotion.materialize(type_formal_scope)?),
                        is_question_type: false,
                    }
                    .into_type()),
                    _ => parse_error(
                        "The type to the left of & must be an unpromoted type parameter",
                    ),
                }
            }
            PreType::Record {
                positional_types,
                named_types,
            } => {
                let mut positional = Vec::new();
                for positional_type in positional_types {
                    positional.push(positional_type.materialize(type_formal_scope)?);
                }
                let mut named = Vec::new();
                for named_type in named_types {
                    named.push(NamedType::new(
                        &named_type.name,
                        named_type.ty.materialize(type_formal_scope)?,
                    ));
                }
                Ok(RecordType::new(positional, named))
            }
            PreType::WithNullability {
                inner,
                is_question_type,
            } => Ok(inner
                .materialize(type_formal_scope)?
                .as_question_type(*is_question_type)),
            PreType::Unknown => Ok(UnknownType::new()),
        }
    }
}

fn is_identifier_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic()
}

fn is_identifier_part(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
}

/// Dart `_identifierRegexp.matchAsPrefix(token) != null`.
fn is_identifier(token: &str) -> bool {
    token.chars().next().is_some_and(is_identifier_start)
}

struct TypeParser {
    type_str: String,
    tokens: Vec<String>,
    i: usize,
}

type PResult<T> = Result<T, ParseError>;

impl TypeParser {
    fn current_token(&self) -> &str {
        &self.tokens[self.i]
    }

    fn next(&mut self) {
        self.i += 1;
    }

    fn parse_failure<T>(&self, message: &str) -> PResult<T> {
        parse_error(format!(
            "Error parsing type `{}` at token {}: {}",
            self.type_str,
            self.current_token(),
            message
        ))
    }

    fn parse_named_function_parameters(&mut self) -> PResult<Vec<PreNamedFunctionParameter>> {
        debug_assert_eq!(self.current_token(), "{");
        self.next();
        let mut named_parameters = Vec::new();
        loop {
            let is_required = self.current_token() == "required";
            if is_required {
                self.next();
            }
            let ty = self.parse_type()?;
            let name = self.current_token().to_string();
            if !is_identifier(&name) {
                return self.parse_failure("Expected an identifier");
            }
            named_parameters.push(PreNamedFunctionParameter {
                name,
                ty,
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
        named_parameters.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(named_parameters)
    }

    fn parse_optional_function_parameters(
        &mut self,
        positional_parameter_types: &mut Vec<PreType>,
    ) -> PResult<()> {
        debug_assert_eq!(self.current_token(), "[");
        self.next();
        loop {
            let ty = self.parse_type()?;
            positional_parameter_types.push(ty);
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

    fn parse_record_type_named_fields(&mut self) -> PResult<Vec<PreNamedType>> {
        debug_assert_eq!(self.current_token(), "{");
        self.next();
        let mut named_types = Vec::new();
        while self.current_token() != "}" {
            let ty = self.parse_type()?;
            let name = self.current_token().to_string();
            if !is_identifier(&name) {
                return self.parse_failure("Expected an identifier");
            }
            named_types.push(PreNamedType { name, ty });
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
        named_types.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(named_types)
    }

    fn parse_record_type_rest(&mut self, mut positional_types: Vec<PreType>) -> PResult<PreType> {
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
        Ok(PreType::Record {
            positional_types,
            named_types: named_types.unwrap_or_default(),
        })
    }

    fn parse_suffix(&mut self, ty: PreType) -> PResult<Result<PreType, PreType>> {
        if self.current_token() == "?" {
            self.next();
            Ok(Ok(PreType::WithNullability {
                inner: Box::new(ty),
                is_question_type: true,
            }))
        } else if self.current_token() == "&" {
            self.next();
            let promotion = self.parse_unsuffixed_type()?;
            Ok(Ok(PreType::Promoted {
                inner: Box::new(ty),
                promotion: Box::new(promotion),
            }))
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
            let mut named_function_parameters = None;
            let mut required_positional_parameter_count = None;
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
            Ok(Ok(PreType::Function {
                return_type: Box::new(ty),
                positional_parameter_types,
                required_positional_parameter_count,
                named_parameters: named_function_parameters.unwrap_or_default(),
                type_formals,
            }))
        } else {
            // Dart returns `null`; the type is handed back unchanged.
            Ok(Err(ty))
        }
    }

    fn parse_type(&mut self) -> PResult<PreType> {
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
                Err(unchanged) => {
                    result = unchanged;
                    break;
                }
            }
        }
        Ok(result)
    }

    fn parse_type_formals(&mut self) -> PResult<Vec<PreTypeFormal>> {
        debug_assert_eq!(self.current_token(), "<");
        self.next();
        let mut type_formals = Vec::new();
        loop {
            let name = self.current_token().to_string();
            if !is_identifier(&name) {
                return self.parse_failure("Expected an identifier");
            }
            self.next();
            let mut bound = None;
            if self.current_token() == "extends" {
                self.next();
                bound = Some(self.parse_type()?);
            }
            type_formals.push(PreTypeFormal { name, bound });
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

    fn parse_unsuffixed_type(&mut self) -> PResult<PreType> {
        if self.current_token() == "_" {
            self.next();
            return Ok(PreType::Unknown);
        }
        if self.current_token() == "(" {
            self.next();
            if self.current_token() == ")" || self.current_token() == "{" {
                return self.parse_record_type_rest(Vec::new());
            }
            let ty = self.parse_type()?;
            if self.current_token() == "," {
                self.next();
                return self.parse_record_type_rest(vec![ty]);
            }
            if self.current_token() != ")" {
                return self.parse_failure("Expected `)` or `,`");
            }
            self.next();
            return Ok(ty);
        }
        let type_name = self.current_token().to_string();
        if !is_identifier(&type_name) {
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
        Ok(PreType::Primary {
            type_name,
            type_args,
        })
    }

    fn parse(type_str: &str) -> PResult<Type> {
        let mut parser = TypeParser {
            type_str: type_str.to_string(),
            tokens: Self::tokenize_type_str(type_str)?,
            i: 0,
        };
        let result = parser.parse_type()?;
        if parser.current_token() != "<END>" {
            return parse_error(format!(
                "Extra tokens after parsing type `{}`: [{}]",
                type_str,
                parser.tokens[parser.i..parser.tokens.len() - 1].join(", ")
            ));
        }
        result.materialize(&HashMap::new())
    }

    /// Splits `type_str` into tokens (Dart `_typeTokenizationRegexp`:
    /// identifiers and the characters `()<>,?*&{}[]`), ending with `<END>`.
    fn tokenize_type_str(type_str: &str) -> PResult<Vec<String>> {
        const PUNCTUATION: &str = "()<>,?*&{}[]";
        let chars: Vec<char> = type_str.chars().collect();
        let mut result = Vec::new();
        let mut extra_chars = String::new();
        let check_extra = |extra_chars: &mut String| -> PResult<()> {
            let trimmed = extra_chars.trim();
            if !trimmed.is_empty() {
                return parse_error(format!(
                    "Unrecognized character(s) in type `{type_str}`: {trimmed}"
                ));
            }
            extra_chars.clear();
            Ok(())
        };
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if is_identifier_start(c) {
                check_extra(&mut extra_chars)?;
                let start = i;
                while i < chars.len() && is_identifier_part(chars[i]) {
                    i += 1;
                }
                result.push(chars[start..i].iter().collect());
            } else if PUNCTUATION.contains(c) {
                check_extra(&mut extra_chars)?;
                result.push(c.to_string());
                i += 1;
            } else {
                extra_chars.push(c);
                i += 1;
            }
        }
        check_extra(&mut extra_chars)?;
        result.push("<END>".to_string());
        Ok(result)
    }
}
