// Dart source: pkg/_fe_analyzer_shared/test/exhaustiveness/env.dart

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use dartr_type_analyzer::exhaustiveness::{
    EnumOperations, ExhaustivenessCache, Key, ObjectPropertyLookup, SealedClassOperations,
    StaticType, StaticTypeArena, TypeOperations,
};
use indexmap::{IndexMap, IndexSet};

/// Dart `_Class`. Classes have object identity, modelled by their index in
/// [EnvData::classes].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Class(usize);

impl Class {
    pub const OBJECT: Class = Class(0);
    pub const NEVER: Class = Class(1);
    pub const BOOL: Class = Class(2);
}

struct ClassData {
    name: String,
    is_sealed: bool,
    /// Dart `cls is _EnumClass`.
    is_enum: bool,
}

/// Dart `_Type`: `_InterfaceType`, `_NullableType` and `_RecordType`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Type {
    Interface(Class),
    Nullable(Box<Type>),
    Record(Rc<RecordType>),
}

impl Type {
    pub fn object() -> Type {
        Type::Interface(Class::OBJECT)
    }

    pub fn nullable_object() -> Type {
        Type::Nullable(Box::new(Type::object()))
    }

    pub fn never() -> Type {
        Type::Interface(Class::NEVER)
    }

    pub fn bool_() -> Type {
        Type::Interface(Class::BOOL)
    }

    pub fn null() -> Type {
        Type::Nullable(Box::new(Type::never()))
    }
}

/// Dart `_RecordType`.
#[derive(Debug)]
pub struct RecordType {
    positional: Vec<Type>,
    named: IndexMap<Key, Type>,
}

impl PartialEq for RecordType {
    fn eq(&self, other: &RecordType) -> bool {
        if self.positional.len() != other.positional.len() {
            return false;
        }
        if self.named.len() != other.named.len() {
            return false;
        }
        for i in 0..self.positional.len() {
            if self.positional[i] != other.positional[i] {
                return false;
            }
        }
        for (key, value) in &self.named {
            if other.named.get(key) != Some(value) {
                return false;
            }
        }
        true
    }
}

impl Eq for RecordType {}

impl Hash for RecordType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.positional.hash(state);
        self.named.len().hash(state);
    }
}

#[derive(Default)]
struct EnvData {
    classes: Vec<ClassData>,
    class_names: IndexSet<String>,
    fields: HashMap<Class, IndexMap<Key, Type>>,
    supertypes: HashMap<Class, IndexSet<Type>>,
    subtypes: HashMap<Class, IndexSet<Type>>,
}

impl EnvData {
    fn add_class(&mut self, name: &str, is_sealed: bool) -> Class {
        assert!(!self.class_names.contains(name), "Duplicate class '{name}'");
        self.class_names.insert(name.to_string());
        self.classes.push(ClassData {
            name: name.to_string(),
            is_sealed,
            is_enum: false,
        });
        Class(self.classes.len() - 1)
    }

    fn add_supertype(&mut self, type_: &Type, supertype: Type) {
        let (Type::Interface(cls), Type::Interface(super_cls)) = (type_, &supertype) else {
            unreachable!("Interface types expected");
        };
        self.supertypes
            .entry(*cls)
            .or_default()
            .insert(supertype.clone());
        self.subtypes
            .entry(*super_cls)
            .or_default()
            .insert(type_.clone());
    }

    fn get_supertypes(&self, cls: Class) -> Vec<Type> {
        self.supertypes
            .get(&cls)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn get_subtypes(&self, cls: Class) -> Vec<Type> {
        self.subtypes
            .get(&cls)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn get_fields(&self, cls: Class) -> IndexMap<Key, Type> {
        self.fields.get(&cls).cloned().unwrap_or_default()
    }

    fn type_to_string(&self, type_: &Type) -> String {
        match type_ {
            Type::Interface(cls) => self.classes[cls.0].name.clone(),
            Type::Nullable(inner) => {
                if **inner == Type::never() {
                    "Null".to_string()
                } else {
                    format!("{}?", self.type_to_string(inner))
                }
            }
            Type::Record(record) => {
                let mut sb = String::new();
                sb.push('(');
                let mut comma = "";
                for type_ in &record.positional {
                    sb.push_str(comma);
                    sb.push_str(&self.type_to_string(type_));
                    comma = ", ";
                }
                if !record.named.is_empty() {
                    sb.push_str(comma);
                    sb.push('{');
                    comma = "";
                    for (key, value) in &record.named {
                        sb.push_str(comma);
                        sb.push_str(&key.name());
                        sb.push_str(": ");
                        sb.push_str(&self.type_to_string(value));
                        comma = ", ";
                    }
                    sb.push('}');
                }
                sb.push(')');
                sb
            }
        }
    }
}

type Env = Rc<RefCell<EnvData>>;

pub type TestCache =
    ExhaustivenessCache<TestTypeOperations, TestEnumOperations, TestSealedClassOperations>;

/// Dart `TestEnvironment`.
pub struct TestEnvironment {
    env: Env,
    cache: TestCache,
}

impl Default for TestEnvironment {
    fn default() -> Self {
        TestEnvironment::new()
    }
}

impl TestEnvironment {
    pub fn new() -> TestEnvironment {
        let env: Env = Rc::new(RefCell::new(EnvData::default()));
        {
            let mut data = env.borrow_mut();
            let object = data.add_class("Object", false);
            assert_eq!(object, Class::OBJECT);
            let never = data.add_class("Never", false);
            assert_eq!(never, Class::NEVER);
            let bool_ = data.add_class("bool", false);
            assert_eq!(bool_, Class::BOOL);
        }
        let cache = ExhaustivenessCache::new(
            TestTypeOperations { env: env.clone() },
            TestEnumOperations { env: env.clone() },
            TestSealedClassOperations { env: env.clone() },
        );
        TestEnvironment { env, cache }
    }

    /// The arena of the static types.
    pub fn types(&self) -> &dyn StaticTypeArena {
        &self.cache
    }

    pub fn cache(&self) -> &TestCache {
        &self.cache
    }

    fn type_from_static_type(&self, type_: StaticType) -> Type {
        if let Some(type_) = self.cache.type_for_testing(type_) {
            type_
        } else if let Some(underlying) = self.cache.as_nullable_static_type(type_) {
            Type::Nullable(Box::new(self.type_from_static_type(underlying)))
        } else if type_ == StaticType::NULL_TYPE {
            Type::null()
        } else if type_ == StaticType::NON_NULLABLE_OBJECT {
            Type::object()
        } else if type_ == StaticType::NULLABLE_OBJECT {
            Type::nullable_object()
        } else if type_ == StaticType::NEVER_TYPE {
            Type::never()
        } else {
            panic!("Unexpected StaticType {}.", self.cache.name(type_));
        }
    }

    /// Dart `createClass(name, isSealed: ..., inherits: ..., fields: ...)`.
    pub fn create_class(
        &self,
        name: &str,
        is_sealed: bool,
        inherits: &[StaticType],
        fields: &[(&str, StaticType)],
    ) -> StaticType {
        let cls = self.env.borrow_mut().add_class(name, is_sealed);
        let type_ = Type::Interface(cls);
        self.env.borrow_mut().add_supertype(&type_, Type::object());
        for inherit in inherits {
            let supertype = self.type_from_static_type(*inherit);
            if let Type::Interface(_) = supertype {
                self.env.borrow_mut().add_supertype(&type_, supertype);
            } else {
                panic!("Unexpected supertype {supertype:?}.");
            }
        }

        if !fields.is_empty() {
            for (name, field_type) in fields {
                let field_type = self.type_from_static_type(*field_type);
                let mut data = self.env.borrow_mut();
                let field_map = data.fields.entry(cls).or_default();
                assert!(
                    !field_map.contains_key(&Key::name_key(*name)),
                    "Duplicate field '{name}'."
                );
                field_map.insert(Key::name_key(*name), field_type);
            }
        }

        let mut sealed = 0;
        {
            let data = self.env.borrow();
            for supertype in data.get_supertypes(cls) {
                if let Type::Interface(super_cls) = supertype
                    && data.classes[super_cls.0].is_sealed
                {
                    sealed += 1;
                }
            }
        }

        // We don't allow a sealed type's subtypes to be shared with some other
        // sibling supertype, as in D here:
        //
        //   (A) (B)
        //   / \ / \
        //  C   D   E
        //
        // We could remove this restriction but doing so will require
        // expandTypes() to be more complex. In the example here, if we subtract
        // E from A, the result should be C|D. That requires knowing that B
        // should be expanded, which expandTypes() doesn't currently handle.
        if sealed > 1 {
            panic!("Can only have one sealed supertype.");
        }

        self.cache.get_static_type(&type_)
    }

    /// Dart `createRecordType(named)`.
    pub fn create_record_type(&self, named: &[(&str, StaticType)]) -> StaticType {
        let mut named_types: IndexMap<Key, Type> = IndexMap::new();
        for (name, type_) in named {
            named_types.insert(
                Key::record_name_key(*name),
                self.type_from_static_type(*type_),
            );
        }
        let type_ = Type::Record(Rc::new(RecordType {
            positional: vec![],
            named: named_types,
        }));
        self.cache.get_static_type(&type_)
    }
}

impl ObjectPropertyLookup for TestEnvironment {
    fn get_object_field_type(&self, key: &Key) -> Option<StaticType> {
        self.cache.get_object_field_type(key)
    }

    fn static_types(&self) -> &dyn StaticTypeArena {
        &self.cache
    }
}

/// Dart `_TypeOperations`.
pub struct TestTypeOperations {
    env: Env,
}

impl TypeOperations for TestTypeOperations {
    type Type = Type;

    fn bool_type(&self) -> Type {
        Type::bool_()
    }

    fn get_field_types(&self, type_: &Type) -> IndexMap<Key, Type> {
        match type_ {
            Type::Interface(cls) => {
                let mut fields: IndexMap<Key, Type> = IndexMap::new();
                let (supertypes, own_fields) = {
                    let data = self.env.borrow();
                    (data.get_supertypes(*cls), data.get_fields(*cls))
                };
                for supertype in &supertypes {
                    fields.extend(self.get_field_types(supertype));
                }
                fields.extend(own_fields);
                fields
            }
            Type::Record(record) => {
                let mut fields: IndexMap<Key, Type> = IndexMap::new();
                fields.extend(self.get_field_types(&Type::object()));
                for (i, positional) in record.positional.iter().enumerate() {
                    fields.insert(Key::RecordIndex(i), positional.clone());
                }
                for (key, value) in &record.named {
                    fields.insert(Key::record_name_key(key.name()), value.clone());
                }
                fields
            }
            Type::Nullable(_) => self.get_field_types(&Type::object()),
        }
    }

    fn get_non_nullable(&self, type_: &Type) -> Type {
        if let Type::Nullable(inner) = type_ {
            return (**inner).clone();
        }
        type_.clone()
    }

    fn is_bool_type(&self, type_: &Type) -> bool {
        *type_ == Type::bool_()
    }

    fn is_never_type(&self, type_: &Type) -> bool {
        *type_ == Type::never()
    }

    fn is_non_nullable_object(&self, type_: &Type) -> bool {
        *type_ == Type::object()
    }

    fn is_dynamic(&self, type_: &Type) -> bool {
        *type_ == Type::nullable_object()
    }

    fn is_null_type(&self, type_: &Type) -> bool {
        *type_ == Type::null()
    }

    fn is_nullable(&self, type_: &Type) -> bool {
        matches!(type_, Type::Nullable(_))
    }

    fn is_potentially_nullable(&self, type_: &Type) -> bool {
        // TODO(johnniwinther): Support type variables.
        matches!(type_, Type::Nullable(_))
    }

    fn is_nullable_object(&self, type_: &Type) -> bool {
        *type_ == Type::nullable_object()
    }

    fn is_subtype_of(&self, s: &Type, t: &Type) -> bool {
        if s == t {
            return true;
        }
        if *t == Type::nullable_object() {
            return true;
        }
        if *s == Type::never() {
            return true;
        }
        if *s == Type::null() && matches!(t, Type::Nullable(_)) {
            return true;
        }
        if let Type::Nullable(s_inner) = s {
            if let Type::Nullable(t_inner) = t {
                return self.is_subtype_of(s_inner, t_inner);
            }
            false
        } else if let Type::Nullable(t_inner) = t {
            self.is_subtype_of(s, t_inner)
        } else {
            if let (Type::Interface(s_cls), Type::Interface(t_cls)) = (s, t) {
                if *t_cls == Class::OBJECT {
                    return true;
                }
                let supertypes = self.env.borrow().get_supertypes(*s_cls);
                for supertype in &supertypes {
                    if self.is_subtype_of(supertype, t) {
                        return true;
                    }
                }
            }
            false
        }
    }

    fn non_nullable_object_type(&self) -> Type {
        Type::object()
    }

    fn nullable_object_type(&self) -> Type {
        Type::nullable_object()
    }

    fn type_to_string(&self, type_: &Type) -> String {
        self.env.borrow().type_to_string(type_)
    }

    fn is_record_type(&self, type_: &Type) -> bool {
        matches!(type_, Type::Record(_))
    }

    fn overapproximate(&self, type_: &Type) -> Type {
        // TODO(johnniwinther): Support generic types in testing.
        type_.clone()
    }

    fn is_generic(&self, _type: &Type) -> bool {
        // TODO(johnniwinther): Support generic types in testing.
        false
    }

    fn instantiate_future(&self, _type: &Type) -> Type {
        unimplemented!("_TypeOperations.getFutureOrFutureType")
    }

    fn get_future_or_type_argument(&self, _type: &Type) -> Option<Type> {
        // TODO(johnniwinther): Support future or types in testing.
        None
    }

    fn get_list_element_type(&self, _type: &Type) -> Option<Type> {
        // TODO(johnniwinther): Support list types in testing.
        None
    }

    fn get_list_type(&self, _type: &Type) -> Option<Type> {
        // TODO(johnniwinther): Support list types in testing.
        None
    }

    fn get_map_value_type(&self, _type: &Type) -> Option<Type> {
        // TODO(johnniwinther): Support map types in testing.
        None
    }

    fn has_simple_name(&self, type_: &Type) -> bool {
        matches!(type_, Type::Interface(_))
    }

    fn get_type_variable_bound(&self, _type: &Type) -> Option<Type> {
        // TODO(johnniwinther): Support type variable bounds in testing.
        None
    }

    fn get_extension_type_erasure(&self, type_: &Type) -> Type {
        // TODO(johnniwinther): Support extension types in testing.
        type_.clone()
    }

    fn is_enum(&self, type_: &Type) -> bool {
        if let Type::Interface(cls) = type_ {
            return self.env.borrow().classes[cls.0].is_enum;
        }
        false
    }

    fn library_uri(&self, _type: &Type) -> Option<String> {
        // TODO(FMorschel): Support library URIs in testing.
        None
    }
}

/// Dart `_EnumOperations`. The Dart `_EnumElement` and `_EnumElementValue`
/// are `Object`; testing of enum elements is not supported.
pub struct TestEnumOperations {
    env: Env,
}

impl EnumOperations for TestEnumOperations {
    type Type = Type;
    type EnumClass = Class;
    type EnumElement = ();
    type EnumElementValue = ();

    fn get_enum_class(&self, type_: &Type) -> Option<Class> {
        if let Type::Interface(cls) = type_
            && self.env.borrow().classes[cls.0].is_enum
        {
            return Some(*cls);
        }
        None
    }

    fn get_enum_element_name(&self, _enum_element: &()) -> String {
        // TODO(johnniwinther): Support testing of enums.
        unimplemented!("_EnumOperations.getEnumElementName")
    }

    fn get_enum_element_type(&self, _enum_element: &()) -> Type {
        // TODO(johnniwinther): Support testing of enums.
        unimplemented!("_EnumOperations.getEnumElementType")
    }

    fn get_enum_element_value(&self, _enum_element: &()) -> Option<()> {
        // TODO(johnniwinther): Support testing of enums.
        unimplemented!("_EnumOperations.getEnumElementValue")
    }

    fn get_enum_elements(&self, _enum_class: &Class) -> Vec<()> {
        // TODO(johnniwinther): Support testing of enums.
        unimplemented!("_EnumOperations.getEnumElements")
    }
}

/// Dart `_SealedClassOperations`.
pub struct TestSealedClassOperations {
    env: Env,
}

impl SealedClassOperations for TestSealedClassOperations {
    type Type = Type;
    type Class = Class;

    fn get_direct_subclasses(&self, sealed_class: &Class) -> Vec<Class> {
        let mut classes = vec![];
        for subtype in self.env.borrow().get_subtypes(*sealed_class) {
            if let Type::Interface(cls) = subtype {
                classes.push(cls);
            }
        }
        classes
    }

    fn get_sealed_class(&self, type_: &Type) -> Option<Class> {
        if let Type::Interface(cls) = type_
            && self.env.borrow().classes[cls.0].is_sealed
        {
            return Some(*cls);
        }
        None
    }

    fn get_subclass_as_instance_of(
        &self,
        sub_class: &Class,
        _sealed_class_type: &Type,
    ) -> Option<Type> {
        Some(Type::Interface(*sub_class))
    }
}

impl fmt::Display for TestEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TestEnvironment({} classes)",
            self.env.borrow().classes.len()
        )
    }
}
