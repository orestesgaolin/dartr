// Dart source: pkg/_fe_analyzer_shared/test/mini_types.dart

//! `Type` and its subclasses, `NamedFunctionParameter`, `NamedType`,
//! `FreshTypeParameterGenerator`, and the list extensions.

use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use dartr_flow::shared_type::{SharedNamedFunctionParameter, SharedNamedType, SharedTypeKind};
use indexmap::IndexSet;

use super::name::Name;
use super::parenthesize_if;
use super::registry::{InterfaceTypeName, SpecialTypeName, TypeNameInfo, TypeParameter};
use super::state::with_state;

/// A type substitution: Dart `Map<TypeParameter, Type>`.
pub type Substitution = HashMap<TypeParameter, Type>;

/// Representation of a type suitable for unit testing of code in the
/// `_fe_analyzer_shared` package.
///
/// Rust: an id into the thread-local interner. `==` is the Dart
/// `operator ==` (see the module documentation), and the derived `Hash` is
/// consistent with it.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Type(u32);

/// The structure of a [`Type`].
#[derive(Clone, Debug)]
pub enum TypeData {
    /// `PrimaryType` and its subclasses (`DynamicType`, `InvalidType`,
    /// `NeverType`, `NullType`, `VoidType`, `FutureOrType`).
    Primary(PrimaryType),
    /// `FunctionType`.
    Function(FunctionType),
    /// `RecordType`.
    Record(RecordType),
    /// `TypeParameterType`.
    TypeParameter(TypeParameterType),
    /// `UnknownType`.
    Unknown(UnknownType),
}

// ------------------------------------------------------------------ interning

impl TypeData {
    /// Interns `self`: returns the id of an interned type that is Dart-`==`
    /// to `self`, or interns `self` as a new type.
    fn intern(self) -> Type {
        let hash = self.bucket_hash();
        let candidates: Vec<u32> =
            with_state(|s| s.buckets.get(&hash).cloned().unwrap_or_default());
        for candidate in candidates {
            let candidate = Type(candidate);
            // Note: `dart_equals` can intern other types (for generic
            // function types), so no state borrow is held here.
            if self.dart_equals(&candidate.data()) {
                return candidate;
            }
        }
        with_state(|s| {
            let id = s.types.len() as u32;
            s.types.push(Rc::new(self));
            s.type_hashes.push(hash);
            s.buckets.entry(hash).or_default().push(id);
            Type(id)
        })
    }

    /// A hash that is equal for Dart-`==` types.
    ///
    /// Type parameter identities and type formal bounds are left out, so that
    /// generic function types that are equal up to renaming of their type
    /// formals get the same hash.
    fn bucket_hash(&self) -> u64 {
        let mut h = DefaultHasher::new();
        match self {
            TypeData::Primary(p) => {
                0u8.hash(&mut h);
                p.name_info.hash(&mut h);
                for arg in &p.args {
                    arg.bucket_hash().hash(&mut h);
                }
                p.is_question_type.hash(&mut h);
            }
            TypeData::Function(f) => {
                1u8.hash(&mut h);
                f.type_parameters_shared.len().hash(&mut h);
                f.return_type.bucket_hash().hash(&mut h);
                for p in &f.positional_parameters {
                    p.bucket_hash().hash(&mut h);
                }
                f.required_positional_parameter_count.hash(&mut h);
                for p in &f.named_parameters {
                    p.name.hash(&mut h);
                    p.is_required.hash(&mut h);
                    p.type_.bucket_hash().hash(&mut h);
                }
                f.is_question_type.hash(&mut h);
            }
            TypeData::Record(r) => {
                2u8.hash(&mut h);
                for p in &r.positional_types {
                    p.bucket_hash().hash(&mut h);
                }
                for n in &r.named_types {
                    n.name.hash(&mut h);
                    n.type_.bucket_hash().hash(&mut h);
                }
                r.is_question_type.hash(&mut h);
            }
            TypeData::TypeParameter(t) => {
                3u8.hash(&mut h);
                t.promotion.map(Type::bucket_hash).hash(&mut h);
                t.is_question_type.hash(&mut h);
            }
            TypeData::Unknown(u) => {
                4u8.hash(&mut h);
                u.is_question_type.hash(&mut h);
            }
        }
        h.finish()
    }

    /// The Dart `operator ==` of the mini types. The constituent types are
    /// interned, so for them `==` is id equality.
    fn dart_equals(&self, other: &TypeData) -> bool {
        match (self, other) {
            (TypeData::Primary(a), TypeData::Primary(b)) => {
                a.name_info == b.name_info
                    && a.args == b.args
                    && a.is_question_type == b.is_question_type
            }
            (TypeData::Function(a), TypeData::Function(b)) => a.dart_equals(b),
            (TypeData::Record(a), TypeData::Record(b)) => {
                a.positional_types == b.positional_types
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

    fn is_question_type(&self) -> bool {
        match self {
            TypeData::Primary(p) => p.is_question_type,
            TypeData::Function(f) => f.is_question_type,
            TypeData::Record(r) => r.is_question_type,
            TypeData::TypeParameter(t) => t.is_question_type,
            TypeData::Unknown(u) => u.is_question_type,
        }
    }

    /// Recursively visits `self`, gathering up all the identifiers that
    /// appear in it, and adds them to the set [identifiers]. See
    /// [`Type::gather_used_identifiers`].
    fn gather_used_identifiers(&self, identifiers: &mut IndexSet<String>) {
        match self {
            TypeData::Primary(p) => {
                identifiers.insert(p.name().as_str().to_owned());
                for arg in &p.args {
                    arg.gather_used_identifiers(identifiers);
                }
            }
            TypeData::Function(f) => {
                f.return_type.gather_used_identifiers(identifiers);
                for positional_parameter in &f.positional_parameters {
                    positional_parameter.gather_used_identifiers(identifiers);
                }
                for type_formal in &f.type_parameters_shared {
                    identifiers.insert(type_formal.name().as_str().to_owned());
                    if let Some(bound) = type_formal.explicit_bound() {
                        bound.gather_used_identifiers(identifiers);
                    }
                }
                for named_parameter in &f.named_parameters {
                    // As explained in the documentation for
                    // `Type.gatherUsedIdentifiers`, to reduce the risk of
                    // confusion, this method is generous in which identifiers
                    // it reports. So report `namedParameter.name` even though
                    // it's not strictly necessary.
                    identifiers.insert(named_parameter.name.as_str().to_owned());
                    named_parameter.type_.gather_used_identifiers(identifiers);
                }
            }
            TypeData::Record(r) => {
                for type_ in &r.positional_types {
                    type_.gather_used_identifiers(identifiers);
                }
                for named_type in &r.named_types {
                    // As explained in the documentation for
                    // `Type.gatherUsedIdentifiers`, to reduce the risk of
                    // confusion, this method is generous in which identifiers
                    // it reports. So report `namedType.name` even though it's
                    // not strictly necessary.
                    identifiers.insert(named_type.name.as_str().to_owned());
                    named_type.type_.gather_used_identifiers(identifiers);
                }
            }
            TypeData::TypeParameter(t) => {
                identifiers.insert(t.type_parameter.name().as_str().to_owned());
                if let Some(promotion) = t.promotion {
                    promotion.gather_used_identifiers(identifiers);
                }
            }
            TypeData::Unknown(_) => {}
        }
    }

    /// Returns a string representation of the portion of this string that
    /// precedes the nullability suffix.
    ///
    /// If [parenthesize_if_complex] is `true`, then the result will be
    /// surrounded by parenthesis if it takes any of the following forms:
    /// - A function type (e.g. `void Function()`)
    /// - A promoted type variable type (e.g. `T&int`)
    fn to_string_without_suffix(&self, parenthesize_if_complex: bool) -> String {
        match self {
            TypeData::Primary(p) => {
                if p.args.is_empty() {
                    p.name().as_str().to_owned()
                } else {
                    format!("{}<{}>", p.name(), join(&p.args))
                }
            }
            TypeData::Function(f) => {
                let mut formals = String::new();
                if !f.type_parameters_shared.is_empty() {
                    let formal_strings: Vec<String> = f
                        .type_parameters_shared
                        .iter()
                        .map(|type_formal| match type_formal.explicit_bound() {
                            Some(bound) => format!("{} extends {bound}", type_formal.name()),
                            None => type_formal.name().as_str().to_owned(),
                        })
                        .collect();
                    formals = format!("<{}>", formal_strings.join(", "));
                }
                let required = f.required_positional_parameter_count;
                let mut parameters: Vec<String> = f.positional_parameters[..required]
                    .iter()
                    .map(|t| t.to_string())
                    .collect();
                if required < f.positional_parameters.len() {
                    parameters.push(format!("[{}]", join(&f.positional_parameters[required..])));
                }
                if !f.named_parameters.is_empty() {
                    parameters.push(format!("{{{}}}", join(&f.named_parameters)));
                }
                parenthesize_if(
                    parenthesize_if_complex,
                    format!(
                        "{} Function{formals}({})",
                        f.return_type,
                        parameters.join(", ")
                    ),
                )
            }
            TypeData::Record(r) => {
                let positional_str = join(&r.positional_types);
                let named_str = r
                    .named_types
                    .iter()
                    .map(|e| format!("{} {}", e.type_, e.name))
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
            TypeData::TypeParameter(t) => match t.promotion {
                Some(promotion) => parenthesize_if(
                    parenthesize_if_complex,
                    format!(
                        "{}&{}",
                        t.type_parameter.name(),
                        promotion.to_string_with(true)
                    ),
                ),
                None => t.type_parameter.name().as_str().to_owned(),
            },
            TypeData::Unknown(_) => "_".to_owned(),
        }
    }
}

/// Dart `list.join(', ')`.
fn join<T: fmt::Display>(items: &[T]) -> String {
    items
        .iter()
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The shared implementation of the Dart list extensions
/// (`closureWithRespectToUnknown`, `recursivelyDemote`, `substitute` on
/// `List<Type>`, `List<NamedFunctionParameter>`, `List<NamedType>`).
///
/// Calls [f] on each element of the list; if all those calls returned `None`
/// (meaning nothing was changed), returns `None`. Otherwise returns a new list
/// in which each changed element is replaced with the result.
fn map_list<T: Clone>(list: &[T], mut f: impl FnMut(&T) -> Option<T>) -> Option<Vec<T>> {
    let mut result: Option<Vec<T>> = None;
    for (i, old_element) in list.iter().enumerate() {
        let new_element = f(old_element);
        if new_element.is_some() && result.is_none() {
            result = Some(list[..i].to_vec());
        }
        if let Some(result) = &mut result {
            result.push(new_element.unwrap_or_else(|| old_element.clone()));
        }
    }
    result
}

// ----------------------------------------------------------------------- Type

impl Type {
    /// The structure of this type.
    pub fn data(self) -> Rc<TypeData> {
        with_state(|s| s.types[self.0 as usize].clone())
    }

    fn bucket_hash(self) -> u64 {
        with_state(|s| s.type_hashes[self.0 as usize])
    }

    /// The interned id (for debugging).
    pub fn id(self) -> u32 {
        self.0
    }

    /// `isQuestionType`: whether this type ends in a `?` suffix.
    pub fn is_question_type(self) -> bool {
        self.data().is_question_type()
    }

    /// `asQuestionType`: returns a modified version of this type, with the
    /// nullability suffix changed to [is_question_type].
    ///
    /// For types that don't accept a nullability suffix (`dynamic`,
    /// InvalidType, `Null`, and `void`), the type is returned unchanged.
    pub fn as_question_type(self, is_question_type: bool) -> Type {
        match &*self.data() {
            TypeData::Primary(p) => match p.name_info {
                TypeNameInfo::Special(
                    SpecialTypeName::Dynamic
                    | SpecialTypeName::Error
                    | SpecialTypeName::Null
                    | SpecialTypeName::Void,
                ) => self,
                _ => PrimaryType {
                    is_question_type,
                    ..p.clone()
                }
                .into_type(),
            },
            TypeData::Function(f) => FunctionType {
                is_question_type,
                ..f.clone()
            }
            .into_type(),
            TypeData::Record(r) => RecordType {
                is_question_type,
                ..r.clone()
            }
            .into_type(),
            TypeData::TypeParameter(t) => TypeParameterType {
                is_question_type,
                ..t.clone()
            }
            .into_type(),
            TypeData::Unknown(_) => UnknownType { is_question_type }.into_type(),
        }
    }

    /// Finds the nearest type that doesn't involve the unknown type (`_`).
    ///
    /// If [covariant] is `true`, a supertype will be returned (replacing `_`
    /// with `Object?`); otherwise a subtype will be returned (replacing `_`
    /// with `Never`).
    pub fn closure_with_respect_to_unknown(self, covariant: bool) -> Option<Type> {
        match &*self.data() {
            // Note: the special simple types have no arguments, so they
            // return `None`; for `FutureOrType` this is the closure of the
            // type argument.
            TypeData::Primary(p) => {
                let new_args = map_list(&p.args, |t| t.closure_with_respect_to_unknown(covariant))?;
                Some(
                    PrimaryType {
                        args: new_args,
                        ..p.clone()
                    }
                    .into_type(),
                )
            }
            TypeData::Function(f) => {
                let new_return_type = f.return_type.closure_with_respect_to_unknown(covariant);
                let new_positional_parameters = map_list(&f.positional_parameters, |t| {
                    t.closure_with_respect_to_unknown(!covariant)
                });
                let new_named_parameters = map_list(&f.named_parameters, |p| {
                    p.type_
                        .closure_with_respect_to_unknown(!covariant)
                        .map(|type_| NamedFunctionParameter { type_, ..*p })
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
                            .unwrap_or_else(|| f.positional_parameters.clone()),
                        named_parameters: new_named_parameters
                            .unwrap_or_else(|| f.named_parameters.clone()),
                        ..f.clone()
                    }
                    .into_type(),
                )
            }
            TypeData::Record(r) => {
                let new_positional = map_list(&r.positional_types, |t| {
                    t.closure_with_respect_to_unknown(covariant)
                });
                let new_named = map_list(&r.named_types, |n| {
                    n.type_
                        .closure_with_respect_to_unknown(covariant)
                        .map(|type_| NamedType::new(n.name, type_))
                });
                if new_positional.is_none() && new_named.is_none() {
                    return None;
                }
                Some(
                    RecordType {
                        positional_types: new_positional
                            .unwrap_or_else(|| r.positional_types.clone()),
                        named_types: new_named.unwrap_or_else(|| r.named_types.clone()),
                        is_question_type: r.is_question_type,
                    }
                    .into_type(),
                )
            }
            TypeData::TypeParameter(t) => {
                let new_promotion = t.promotion?.closure_with_respect_to_unknown(covariant)?;
                Some(
                    TypeParameterType {
                        promotion: Some(new_promotion),
                        ..t.clone()
                    }
                    .into_type(),
                )
            }
            TypeData::Unknown(_) => Some(if covariant {
                Type::new("Object?")
            } else {
                NeverType::instance()
            }),
        }
    }

    /// Recursively visits `self`, gathering up all the identifiers that
    /// appear in it, and adds them to the set [identifiers].
    ///
    /// This method is intended to aid in choosing safe names for
    /// substitutions. For example, it can be used to determine that in a type
    /// like `T Function<U>(U)`, it's not safe to rename the type variable `U`
    /// to `T`, since that would conflict with an existing use of `T`.
    ///
    /// To lower the risk of confusion, it is generous in which identifiers it
    /// reports. For example, in the type `void Function<T>({T X})`, it
    /// reports `X` as a used identifier. This is because even though it would
    /// technically be safe to rename the type variable `T` to `X`, to do so
    /// would be result in a confusing type.
    pub fn gather_used_identifiers(self, identifiers: &mut IndexSet<String>) {
        self.data().gather_used_identifiers(identifiers);
    }

    /// `getDisplayString`.
    pub fn get_display_string(self) -> String {
        self.to_string()
    }

    /// `isStructurallyEqualTo`.
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
        match &*self.data() {
            TypeData::Primary(p) => {
                let new_args = map_list(&p.args, |t| t.recursively_demote(covariant))?;
                Some(
                    PrimaryType {
                        args: new_args,
                        ..p.clone()
                    }
                    .into_type(),
                )
            }
            TypeData::Function(f) => {
                let new_return_type = f.return_type.recursively_demote(covariant);
                let new_positional_parameters = map_list(&f.positional_parameters, |t| {
                    t.recursively_demote(!covariant)
                });
                let new_named_parameters = map_list(&f.named_parameters, |p| {
                    p.type_
                        .recursively_demote(!covariant)
                        .map(|type_| NamedFunctionParameter { type_, ..*p })
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
                            .unwrap_or_else(|| f.positional_parameters.clone()),
                        named_parameters: new_named_parameters
                            .unwrap_or_else(|| f.named_parameters.clone()),
                        ..f.clone()
                    }
                    .into_type(),
                )
            }
            TypeData::Record(r) => {
                let new_positional =
                    map_list(&r.positional_types, |t| t.recursively_demote(covariant));
                let new_named = map_list(&r.named_types, |n| {
                    n.type_
                        .recursively_demote(covariant)
                        .map(|type_| NamedType::new(n.name, type_))
                });
                if new_positional.is_none() && new_named.is_none() {
                    return None;
                }
                Some(
                    RecordType {
                        positional_types: new_positional
                            .unwrap_or_else(|| r.positional_types.clone()),
                        named_types: new_named.unwrap_or_else(|| r.named_types.clone()),
                        is_question_type: r.is_question_type,
                    }
                    .into_type(),
                )
            }
            TypeData::TypeParameter(t) => {
                if !covariant {
                    Some(NeverType::instance().as_question_type(t.is_question_type))
                } else if t.promotion.is_none() {
                    None
                } else {
                    Some(
                        TypeParameterType::new(t.type_parameter)
                            .with_is_question_type(t.is_question_type)
                            .into_type(),
                    )
                }
            }
            TypeData::Unknown(_) => None,
        }
    }

    /// If `self` contains any references to a [`TypeParameter`] matching one
    /// of the keys in [substitution], returns a clone of `self` with those
    /// references replaced by the corresponding value. Otherwise returns
    /// `None`.
    ///
    /// For example, if `t` is a reference to the [`TypeParameter`] object
    /// representing `T`, then
    /// `Type::new("Map<T, U>").substitute({t: Type::new("int")})` returns a
    /// [`Type`] object representing `Map<int, U>`.
    ///
    /// For a function type this is `substitute(substitution,
    /// dropTypeFormals: false)`; see [`FunctionType::substitute`].
    pub fn substitute(self, substitution: &Substitution) -> Option<Type> {
        match &*self.data() {
            TypeData::Primary(p) => {
                let new_args = map_list(&p.args, |t| t.substitute(substitution))?;
                Some(
                    PrimaryType {
                        args: new_args,
                        ..p.clone()
                    }
                    .into_type(),
                )
            }
            TypeData::Function(f) => f.substitute(substitution, false),
            TypeData::Record(r) => {
                let new_positional_types =
                    map_list(&r.positional_types, |t| t.substitute(substitution));
                let new_named_types = map_list(&r.named_types, |n| n.substitute(substitution));
                if new_positional_types.is_none() && new_named_types.is_none() {
                    return None;
                }
                Some(
                    RecordType {
                        positional_types: new_positional_types
                            .unwrap_or_else(|| r.positional_types.clone()),
                        named_types: new_named_types.unwrap_or_else(|| r.named_types.clone()),
                        is_question_type: r.is_question_type,
                    }
                    .into_type(),
                )
            }
            TypeData::TypeParameter(t) => substitution.get(&t.type_parameter).copied(),
            TypeData::Unknown(_) => None,
        }
    }

    /// Returns a string representation of this type (Dart
    /// `toString(parenthesizeIfComplex: ...)`).
    ///
    /// If [parenthesize_if_complex] is `true`, then the result will be
    /// surrounded by parenthesis if it takes any of the following forms:
    /// - A type with a trailing `?` or `*`
    /// - A function type (e.g. `void Function()`)
    /// - A promoted type variable type (e.g. `T&int`)
    pub fn to_string_with(self, parenthesize_if_complex: bool) -> String {
        let data = self.data();
        if data.is_question_type() {
            parenthesize_if(
                parenthesize_if_complex,
                format!("{}?", data.to_string_without_suffix(true)),
            )
        } else {
            data.to_string_without_suffix(parenthesize_if_complex)
        }
    }

    // ------------------------------------------------------------ `is` tests

    /// Which `Shared*` interface this type implements.
    pub fn shared_type_kind(self) -> SharedTypeKind {
        match &*self.data() {
            TypeData::Primary(p) => match p.name_info {
                TypeNameInfo::Special(SpecialTypeName::Dynamic) => SharedTypeKind::Dynamic,
                TypeNameInfo::Special(SpecialTypeName::Error) => SharedTypeKind::Invalid,
                TypeNameInfo::Special(SpecialTypeName::Null) => SharedTypeKind::Null,
                TypeNameInfo::Special(SpecialTypeName::Void) => SharedTypeKind::Void,
                _ => SharedTypeKind::Other,
            },
            TypeData::Function(_) => SharedTypeKind::Function,
            TypeData::Record(_) => SharedTypeKind::Record,
            TypeData::TypeParameter(_) => SharedTypeKind::Other,
            TypeData::Unknown(_) => SharedTypeKind::Unknown,
        }
    }

    fn special_type_name(self) -> Option<SpecialTypeName> {
        match &*self.data() {
            TypeData::Primary(PrimaryType {
                name_info: TypeNameInfo::Special(s),
                ..
            }) => Some(*s),
            _ => None,
        }
    }

    /// `is DynamicType`.
    pub fn is_dynamic_type(self) -> bool {
        self.special_type_name() == Some(SpecialTypeName::Dynamic)
    }

    /// `is InvalidType`.
    pub fn is_invalid_type(self) -> bool {
        self.special_type_name() == Some(SpecialTypeName::Error)
    }

    /// `is NeverType` (`Never` or `Never?`).
    pub fn is_never_type(self) -> bool {
        self.special_type_name() == Some(SpecialTypeName::Never)
    }

    /// `is NullType`.
    pub fn is_null_type(self) -> bool {
        self.special_type_name() == Some(SpecialTypeName::Null)
    }

    /// `is VoidType`.
    pub fn is_void_type(self) -> bool {
        self.special_type_name() == Some(SpecialTypeName::Void)
    }

    /// `is FutureOrType`.
    pub fn is_future_or_type(self) -> bool {
        self.special_type_name() == Some(SpecialTypeName::FutureOr)
    }

    /// `is PrimaryType` (includes the special types and `FutureOr`).
    pub fn is_primary_type(self) -> bool {
        matches!(&*self.data(), TypeData::Primary(_))
    }

    /// `is FunctionType`.
    pub fn is_function_type(self) -> bool {
        matches!(&*self.data(), TypeData::Function(_))
    }

    /// `is RecordType`.
    pub fn is_record_type(self) -> bool {
        matches!(&*self.data(), TypeData::Record(_))
    }

    /// `is TypeParameterType`.
    pub fn is_type_parameter_type(self) -> bool {
        matches!(&*self.data(), TypeData::TypeParameter(_))
    }

    /// `is UnknownType`.
    pub fn is_unknown_type(self) -> bool {
        matches!(&*self.data(), TypeData::Unknown(_))
    }

    /// `as PrimaryType` (a copy of the data), or `None`.
    pub fn as_primary_type(self) -> Option<PrimaryType> {
        match &*self.data() {
            TypeData::Primary(p) => Some(p.clone()),
            _ => None,
        }
    }

    /// `as FunctionType` (a copy of the data), or `None`.
    pub fn as_function_type(self) -> Option<FunctionType> {
        match &*self.data() {
            TypeData::Function(f) => Some(f.clone()),
            _ => None,
        }
    }

    /// `as RecordType` (a copy of the data), or `None`.
    pub fn as_record_type(self) -> Option<RecordType> {
        match &*self.data() {
            TypeData::Record(r) => Some(r.clone()),
            _ => None,
        }
    }

    /// `as TypeParameterType` (a copy of the data), or `None`.
    pub fn as_type_parameter_type(self) -> Option<TypeParameterType> {
        match &*self.data() {
            TypeData::TypeParameter(t) => Some(t.clone()),
            _ => None,
        }
    }

    /// `(this as FutureOrType).typeArgument`, or `None` if this is not a
    /// `FutureOr` type.
    pub fn future_or_type_argument(self) -> Option<Type> {
        match &*self.data() {
            TypeData::Primary(p)
                if p.name_info == TypeNameInfo::Special(SpecialTypeName::FutureOr) =>
            {
                Some(p.args[0])
            }
            _ => None,
        }
    }

    // ------------------------------------------------- `Shared*` getters

    fn expect_function(self) -> Rc<TypeData> {
        let data = self.data();
        assert!(
            matches!(&*data, TypeData::Function(_)),
            "{self} is not a function type"
        );
        data
    }

    fn expect_record(self) -> Rc<TypeData> {
        let data = self.data();
        assert!(
            matches!(&*data, TypeData::Record(_)),
            "{self} is not a record type"
        );
        data
    }

    /// `SharedFunctionType.positionalParameterTypesShared`. Panics if this is
    /// not a function type.
    pub fn positional_parameter_types_shared(self) -> Vec<Type> {
        match &*self.expect_function() {
            TypeData::Function(f) => f.positional_parameters.clone(),
            _ => unreachable!(),
        }
    }

    /// `SharedFunctionType.requiredPositionalParameterCount`. Panics if this
    /// is not a function type.
    pub fn required_positional_parameter_count(self) -> usize {
        match &*self.expect_function() {
            TypeData::Function(f) => f.required_positional_parameter_count,
            _ => unreachable!(),
        }
    }

    /// `SharedFunctionType.returnTypeShared`. Panics if this is not a
    /// function type.
    pub fn return_type_shared(self) -> Type {
        match &*self.expect_function() {
            TypeData::Function(f) => f.return_type,
            _ => unreachable!(),
        }
    }

    /// `SharedFunctionType.sortedNamedParametersShared`. Panics if this is
    /// not a function type.
    pub fn sorted_named_parameters_shared(self) -> Vec<SharedNamedFunctionParameter<Name, Type>> {
        match &*self.expect_function() {
            TypeData::Function(f) => f
                .named_parameters
                .iter()
                .map(|p| SharedNamedFunctionParameter {
                    is_required: p.is_required,
                    name_shared: p.name,
                    type_shared: p.type_,
                })
                .collect(),
            _ => unreachable!(),
        }
    }

    /// `SharedFunctionType.typeParametersShared`. Panics if this is not a
    /// function type.
    pub fn type_parameters_shared(self) -> Vec<TypeParameter> {
        match &*self.expect_function() {
            TypeData::Function(f) => f.type_parameters_shared.clone(),
            _ => unreachable!(),
        }
    }

    /// `SharedRecordType.positionalTypesShared`. Panics if this is not a
    /// record type.
    pub fn positional_types_shared(self) -> Vec<Type> {
        match &*self.expect_record() {
            TypeData::Record(r) => r.positional_types.clone(),
            _ => unreachable!(),
        }
    }

    /// `SharedRecordType.sortedNamedTypesShared`. Panics if this is not a
    /// record type.
    pub fn sorted_named_types_shared(self) -> Vec<SharedNamedType<Name, Type>> {
        match &*self.expect_record() {
            TypeData::Record(r) => r
                .named_types
                .iter()
                .map(|n| SharedNamedType {
                    name_shared: n.name,
                    type_shared: n.type_,
                })
                .collect(),
            _ => unreachable!(),
        }
    }
}

impl fmt::Display for Type {
    /// Dart `toString()`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string_with(false))
    }
}

impl fmt::Debug for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Type({}: {})", self.0, self)
    }
}

// ------------------------------------------------------------- PrimaryType

/// Representation of a primary type suitable for unit testing of code in the
/// `_fe_analyzer_shared` package. A primary type is either an interface type
/// with zero or more type parameters (e.g. `double`, or `Map<int, String>`)
/// or one of the special types whose name is a single word (e.g. `dynamic`).
#[derive(Clone, Debug)]
pub struct PrimaryType {
    /// Information about the type name. Never a
    /// [`TypeNameInfo::TypeParameter`].
    pub name_info: TypeNameInfo,

    /// The type arguments, or empty if there are no type arguments.
    pub args: Vec<Type>,

    /// `isQuestionType`.
    pub is_question_type: bool,
}

impl PrimaryType {
    /// `PrimaryType(nameInfo, args: args)`: an interface type.
    pub fn new(name_info: InterfaceTypeName, args: Vec<Type>) -> PrimaryType {
        PrimaryType {
            name_info: TypeNameInfo::Interface(name_info),
            args,
            is_question_type: false,
        }
    }

    /// Sets `isQuestionType` (Dart named parameter, default `false`).
    pub fn with_is_question_type(mut self, is_question_type: bool) -> PrimaryType {
        self.is_question_type = is_question_type;
        self
    }

    /// Interns this type.
    ///
    /// Panics where the Dart constructors assert that the runtime type
    /// matches the name: a type parameter name, a special type with the wrong
    /// number of type arguments, or a nullable `dynamic`, `error`, `Null` or
    /// `void`.
    pub fn into_type(self) -> Type {
        match self.name_info {
            TypeNameInfo::Interface(_) => {}
            TypeNameInfo::TypeParameter(_) => panic!(
                "{} should use TypeParameterType, but constructed PrimaryType instead",
                self.name()
            ),
            TypeNameInfo::Special(SpecialTypeName::FutureOr) => assert_eq!(self.args.len(), 1),
            TypeNameInfo::Special(SpecialTypeName::Never) => assert!(self.args.is_empty()),
            TypeNameInfo::Special(_) => {
                assert!(self.args.is_empty() && !self.is_question_type)
            }
        }
        TypeData::Primary(self).intern()
    }

    /// `isInterfaceType`.
    pub fn is_interface_type(&self) -> bool {
        matches!(self.name_info, TypeNameInfo::Interface(_))
    }

    /// The name of the type.
    pub fn name(&self) -> Name {
        self.name_info.name()
    }
}

impl From<PrimaryType> for Type {
    fn from(t: PrimaryType) -> Type {
        t.into_type()
    }
}

/// Representation of the type `dynamic` suitable for unit testing of code in
/// the `_fe_analyzer_shared` package.
pub enum DynamicType {}

impl DynamicType {
    /// `DynamicType.instance`.
    pub fn instance() -> Type {
        PrimaryType {
            name_info: TypeNameInfo::Special(SpecialTypeName::Dynamic),
            args: Vec::new(),
            is_question_type: false,
        }
        .into_type()
    }
}

/// Representation of an invalid type suitable for unit testing of code in
/// the `_fe_analyzer_shared` package.
pub enum InvalidType {}

impl InvalidType {
    /// `InvalidType.instance`.
    pub fn instance() -> Type {
        PrimaryType {
            name_info: TypeNameInfo::Special(SpecialTypeName::Error),
            args: Vec::new(),
            is_question_type: false,
        }
        .into_type()
    }
}

/// Representation of the type `Never` suitable for unit testing of code in
/// the `_fe_analyzer_shared` package.
pub enum NeverType {}

impl NeverType {
    /// `NeverType.instance`.
    pub fn instance() -> Type {
        PrimaryType {
            name_info: TypeNameInfo::Special(SpecialTypeName::Never),
            args: Vec::new(),
            is_question_type: false,
        }
        .into_type()
    }
}

/// Representation of the type `Null` suitable for unit testing of code in
/// the `_fe_analyzer_shared` package.
pub enum NullType {}

impl NullType {
    /// `NullType.instance`.
    pub fn instance() -> Type {
        PrimaryType {
            name_info: TypeNameInfo::Special(SpecialTypeName::Null),
            args: Vec::new(),
            is_question_type: false,
        }
        .into_type()
    }
}

/// Representation of the type `void` suitable for unit testing of code in
/// the `_fe_analyzer_shared` package.
pub enum VoidType {}

impl VoidType {
    /// `VoidType.instance`.
    pub fn instance() -> Type {
        PrimaryType {
            name_info: TypeNameInfo::Special(SpecialTypeName::Void),
            args: Vec::new(),
            is_question_type: false,
        }
        .into_type()
    }
}

/// Representation of the type `FutureOr<T>` suitable for unit testing of
/// code in the `_fe_analyzer_shared` package.
pub enum FutureOrType {}

impl FutureOrType {
    /// `FutureOrType(typeArgument, isQuestionType: isQuestionType)`.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(type_argument: Type, is_question_type: bool) -> Type {
        PrimaryType {
            name_info: TypeNameInfo::Special(SpecialTypeName::FutureOr),
            args: vec![type_argument],
            is_question_type,
        }
        .into_type()
    }
}

// ------------------------------------------------------------ FunctionType

/// Representation of a function type suitable for unit testing of code in
/// the `_fe_analyzer_shared` package.
#[derive(Clone, Debug)]
pub struct FunctionType {
    /// `returnType`.
    pub return_type: Type,

    /// `typeParametersShared`: the type formals.
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
    /// `FunctionType(returnType, positionalParameters)`, with the defaults of
    /// the named parameters (no type formals, all positional parameters
    /// required, no named parameters, not nullable).
    pub fn new(return_type: Type, positional_parameters: Vec<Type>) -> FunctionType {
        FunctionType {
            return_type,
            type_parameters_shared: Vec::new(),
            required_positional_parameter_count: positional_parameters.len(),
            positional_parameters,
            named_parameters: Vec::new(),
            is_question_type: false,
        }
    }

    /// Sets `typeParametersShared`.
    pub fn with_type_parameters(mut self, type_parameters: Vec<TypeParameter>) -> FunctionType {
        self.type_parameters_shared = type_parameters;
        self
    }

    /// Sets `requiredPositionalParameterCount`.
    pub fn with_required_positional_parameter_count(mut self, count: usize) -> FunctionType {
        self.required_positional_parameter_count = count;
        self
    }

    /// Sets `namedParameters` (must be sorted by name).
    pub fn with_named_parameters(
        mut self,
        named_parameters: Vec<NamedFunctionParameter>,
    ) -> FunctionType {
        self.named_parameters = named_parameters;
        self
    }

    /// Sets `isQuestionType`.
    pub fn with_is_question_type(mut self, is_question_type: bool) -> FunctionType {
        self.is_question_type = is_question_type;
        self
    }

    /// Interns this type. Panics if the named parameters are not properly
    /// sorted (a Dart assert).
    pub fn into_type(self) -> Type {
        for i in 1..self.named_parameters.len() {
            assert!(
                self.named_parameters[i - 1].name < self.named_parameters[i].name,
                "namedParameters not properly sorted"
            );
        }
        assert!(self.required_positional_parameter_count <= self.positional_parameters.len());
        TypeData::Function(self).intern()
    }

    /// `positionalParameterTypes`.
    pub fn positional_parameter_types(&self) -> &[Type] {
        &self.positional_parameters
    }

    /// The Dart `operator ==`.
    fn dart_equals(&self, other: &FunctionType) -> bool {
        if self.type_parameters_shared.len() != other.type_parameters_shared.len() {
            return false;
        }
        if !self.type_parameters_shared.is_empty() {
            // Check if types are equal under a consistent renaming of type
            // formals
            let mut fresh_type_parameter_generator = FreshTypeParameterGenerator::new();
            fresh_type_parameter_generator
                .exclude_names_used_in_data(&TypeData::Function(self.clone()));
            fresh_type_parameter_generator
                .exclude_names_used_in_data(&TypeData::Function(other.clone()));
            let mut this_substitution = Substitution::new();
            let mut other_substitution = Substitution::new();
            let mut this_type_formal_bounds = Vec::new();
            let mut other_type_formal_bounds = Vec::new();
            for i in 0..self.type_parameters_shared.len() {
                let fresh_type_parameter_type =
                    TypeParameterType::new(fresh_type_parameter_generator.generate()).into_type();
                this_substitution.insert(self.type_parameters_shared[i], fresh_type_parameter_type);
                other_substitution
                    .insert(other.type_parameters_shared[i], fresh_type_parameter_type);
                this_type_formal_bounds.push(self.type_parameters_shared[i].bound());
                other_type_formal_bounds.push(other.type_parameters_shared[i].bound());
            }
            let this_bounds = map_list(&this_type_formal_bounds, |t| {
                t.substitute(&this_substitution)
            })
            .unwrap_or(this_type_formal_bounds);
            let other_bounds = map_list(&other_type_formal_bounds, |t| {
                t.substitute(&other_substitution)
            })
            .unwrap_or(other_type_formal_bounds);
            this_bounds == other_bounds
                && self.substitute(&this_substitution, true)
                    == other.substitute(&other_substitution, true)
        } else {
            self.return_type == other.return_type
                && self.positional_parameters == other.positional_parameters
                && self.required_positional_parameter_count
                    == other.required_positional_parameter_count
                && self.named_parameters == other.named_parameters
                && self.is_question_type == other.is_question_type
        }
    }

    /// `substitute(substitution, dropTypeFormals: dropTypeFormals)`.
    ///
    /// If [drop_type_formals] is `true`, the result has no type formals
    /// (references to the type formals are left as they are, unless the
    /// substitution maps them).
    pub fn substitute(&self, substitution: &Substitution, drop_type_formals: bool) -> Option<Type> {
        let mut substitution = Cow::Borrowed(substitution);
        let mut new_type_formals: Option<Vec<TypeParameter>> = None;
        if !self.type_parameters_shared.is_empty() {
            if drop_type_formals {
                new_type_formals = Some(Vec::new());
            } else {
                // Check if any of the type formal bounds will be changed by
                // the substitution.
                if self.type_parameters_shared.iter().any(|type_formal| {
                    type_formal
                        .explicit_bound()
                        .and_then(|b| b.substitute(&substitution))
                        .is_some()
                }) {
                    // Yes, at least one of the type formal bounds will be
                    // changed by the substitution. So that type formal will
                    // have to be replaced by a fresh one. Since type formal
                    // bounds can refer to other type formals, other type
                    // formals might need to be replaced by fresh ones too. To
                    // make things easier, go ahead and replace all the type
                    // formals. Also, extend the substitution so that any
                    // references to old type formals will be replaced by
                    // references to the new type formals.
                    let mut extended = substitution.into_owned();
                    let mut formals = Vec::new();
                    for type_formal in &self.type_parameters_shared {
                        let new_type_formal =
                            TypeParameter::new_unregistered(type_formal.name().as_str());
                        formals.push(new_type_formal);
                        extended.insert(
                            *type_formal,
                            TypeParameterType::new(new_type_formal).into_type(),
                        );
                    }
                    // Now that the substitution has been created, fix up all
                    // the bounds.
                    for (i, type_formal) in self.type_parameters_shared.iter().enumerate() {
                        if let Some(bound) = type_formal.explicit_bound() {
                            formals[i].set_explicit_bound(Some(
                                bound.substitute(&extended).unwrap_or(bound),
                            ));
                        }
                    }
                    substitution = Cow::Owned(extended);
                    new_type_formals = Some(formals);
                }
            }
        }

        let new_return_type = self.return_type.substitute(&substitution);
        let new_positional_parameters =
            map_list(&self.positional_parameters, |t| t.substitute(&substitution));
        let new_named_parameters =
            map_list(&self.named_parameters, |p| p.substitute(&substitution));
        if new_return_type.is_none()
            && new_positional_parameters.is_none()
            && new_type_formals.is_none()
            && new_named_parameters.is_none()
        {
            None
        } else {
            Some(
                FunctionType {
                    return_type: new_return_type.unwrap_or(self.return_type),
                    type_parameters_shared: new_type_formals
                        .unwrap_or_else(|| self.type_parameters_shared.clone()),
                    positional_parameters: new_positional_parameters
                        .unwrap_or_else(|| self.positional_parameters.clone()),
                    required_positional_parameter_count: self.required_positional_parameter_count,
                    named_parameters: new_named_parameters
                        .unwrap_or_else(|| self.named_parameters.clone()),
                    is_question_type: self.is_question_type,
                }
                .into_type(),
            )
        }
    }
}

impl From<FunctionType> for Type {
    fn from(t: FunctionType) -> Type {
        t.into_type()
    }
}

/// A named parameter of a function type.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NamedFunctionParameter {
    /// `name`.
    pub name: Name,

    /// `type`.
    pub type_: Type,

    /// `isRequired`.
    pub is_required: bool,
}

impl NamedFunctionParameter {
    /// `NamedFunctionParameter(isRequired:, name:, type:)`.
    pub fn new(is_required: bool, name: impl Into<Name>, type_: Type) -> NamedFunctionParameter {
        NamedFunctionParameter {
            name: name.into(),
            type_,
            is_required,
        }
    }

    /// `nameShared`.
    pub fn name_shared(&self) -> Name {
        self.name
    }

    /// `typeShared`.
    pub fn type_shared(&self) -> Type {
        self.type_
    }

    /// `substitute`: see [`Type::substitute`].
    pub fn substitute(&self, substitution: &Substitution) -> Option<NamedFunctionParameter> {
        let new_type = self.type_.substitute(substitution)?;
        Some(NamedFunctionParameter {
            type_: new_type,
            ..*self
        })
    }
}

impl fmt::Display for NamedFunctionParameter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_required {
            write!(f, "required ")?;
        }
        write!(f, "{} {}", self.type_, self.name)
    }
}

// -------------------------------------------------------------- RecordType

/// A named field of a record type.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NamedType {
    /// `name`.
    pub name: Name,

    /// `type`.
    pub type_: Type,
}

impl NamedType {
    /// `NamedType(name:, type:)`.
    pub fn new(name: impl Into<Name>, type_: Type) -> NamedType {
        NamedType {
            name: name.into(),
            type_,
        }
    }

    /// `nameShared`.
    pub fn name_shared(&self) -> Name {
        self.name
    }

    /// `typeShared`.
    pub fn type_shared(&self) -> Type {
        self.type_
    }

    /// `substitute`: see [`Type::substitute`].
    pub fn substitute(&self, substitution: &Substitution) -> Option<NamedType> {
        let new_type = self.type_.substitute(substitution)?;
        Some(NamedType::new(self.name, new_type))
    }
}

/// Representation of a record type suitable for unit testing of code in the
/// `_fe_analyzer_shared` package.
#[derive(Clone, Debug)]
pub struct RecordType {
    /// `positionalTypes`.
    pub positional_types: Vec<Type>,

    /// `namedTypes`, sorted by name.
    pub named_types: Vec<NamedType>,

    /// `isQuestionType`.
    pub is_question_type: bool,
}

impl RecordType {
    /// `RecordType(positionalTypes:, namedTypes:)`.
    pub fn new(positional_types: Vec<Type>, named_types: Vec<NamedType>) -> RecordType {
        RecordType {
            positional_types,
            named_types,
            is_question_type: false,
        }
    }

    /// Sets `isQuestionType`.
    pub fn with_is_question_type(mut self, is_question_type: bool) -> RecordType {
        self.is_question_type = is_question_type;
        self
    }

    /// Interns this type. Panics if the named types are not properly sorted
    /// (a Dart assert).
    pub fn into_type(self) -> Type {
        for i in 1..self.named_types.len() {
            assert!(
                self.named_types[i - 1].name < self.named_types[i].name,
                "namedTypes not properly sorted"
            );
        }
        TypeData::Record(self).intern()
    }

    /// `sortedNamedTypes`.
    pub fn sorted_named_types(&self) -> &[NamedType] {
        &self.named_types
    }
}

impl From<RecordType> for Type {
    fn from(t: RecordType) -> Type {
        t.into_type()
    }
}

// ------------------------------------------------------- TypeParameterType

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
    pub fn new(type_parameter: TypeParameter) -> TypeParameterType {
        TypeParameterType {
            type_parameter,
            promotion: None,
            is_question_type: false,
        }
    }

    /// Sets `promotion`.
    pub fn with_promotion(mut self, promotion: Option<Type>) -> TypeParameterType {
        self.promotion = promotion;
        self
    }

    /// Sets `isQuestionType`.
    pub fn with_is_question_type(mut self, is_question_type: bool) -> TypeParameterType {
        self.is_question_type = is_question_type;
        self
    }

    /// Interns this type.
    pub fn into_type(self) -> Type {
        TypeData::TypeParameter(self).intern()
    }

    /// The type parameter's bound.
    pub fn bound(&self) -> Type {
        self.type_parameter.bound()
    }
}

impl From<TypeParameterType> for Type {
    fn from(t: TypeParameterType) -> Type {
        t.into_type()
    }
}

// ------------------------------------------------------------- UnknownType

/// Representation of the unknown type suitable for unit testing of code in
/// the `_fe_analyzer_shared` package.
#[derive(Clone, Debug)]
pub struct UnknownType {
    /// `isQuestionType`.
    pub is_question_type: bool,
}

impl UnknownType {
    /// `UnknownType(isQuestionType: isQuestionType)`.
    pub fn new(is_question_type: bool) -> UnknownType {
        UnknownType { is_question_type }
    }

    /// Interns this type.
    pub fn into_type(self) -> Type {
        TypeData::Unknown(self).intern()
    }
}

impl From<UnknownType> for Type {
    fn from(t: UnknownType) -> Type {
        t.into_type()
    }
}

// ------------------------------------------------ FreshTypeParameterGenerator

/// Factory for creating fresh type parameters.
///
/// Generated type parameters will have names of the form `Tn`, where `n` is a
/// small non-negative integer.
#[derive(Default)]
pub struct FreshTypeParameterGenerator {
    names_to_exclude: IndexSet<String>,
    counter: usize,
}

impl FreshTypeParameterGenerator {
    /// `FreshTypeParameterGenerator()`.
    pub fn new() -> FreshTypeParameterGenerator {
        FreshTypeParameterGenerator::default()
    }

    /// Ensures that when [generate](Self::generate) is called, the type
    /// parameter it returns will have a name that's distinct from all
    /// identifiers in [type_].
    pub fn exclude_names_used_in(&mut self, type_: Type) {
        type_.gather_used_identifiers(&mut self.names_to_exclude);
    }

    fn exclude_names_used_in_data(&mut self, data: &TypeData) {
        data.gather_used_identifiers(&mut self.names_to_exclude);
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
