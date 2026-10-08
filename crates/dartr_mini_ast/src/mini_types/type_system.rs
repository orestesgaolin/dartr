// Dart source: pkg/_fe_analyzer_shared/test/mini_types.dart

//! `TypeSystem`: subtyping, `factor` and `derivedFutureType` for mini types.

use std::rc::Rc;

use indexmap::IndexMap;

use super::name::Name;
use super::registry::TypeRegistry;
use super::types::{
    FunctionType, NeverType, NullType, PrimaryType, RecordType, Type, TypeData, TypeParameterType,
};

/// A super-interface template: maps the type arguments of a class to its
/// super-interfaces (Dart `List<Type> Function(List<Type>)`).
pub type SuperInterfaceTemplate = Rc<dyn Fn(&[Type]) -> Vec<Type>>;

/// `TypeSystem`.
pub struct TypeSystem {
    super_interface_templates: IndexMap<Name, SuperInterfaceTemplate>,
}

impl Default for TypeSystem {
    fn default() -> Self {
        TypeSystem::new()
    }
}

/// `TypeSystem._coreSuperInterfaceTemplates`.
fn core_super_interface_templates() -> IndexMap<Name, SuperInterfaceTemplate> {
    fn template(f: impl Fn(&[Type]) -> Vec<Type> + 'static) -> SuperInterfaceTemplate {
        Rc::new(f)
    }
    let mut map = IndexMap::new();
    let mut add = |name: &str, t: SuperInterfaceTemplate| {
        map.insert(Name::new(name), t);
    };
    add("bool", template(|_| vec![Type::new("Object")]));
    add(
        "double",
        template(|_| vec![Type::new("num"), Type::new("Object")]),
    );
    add("Future", template(|_| vec![Type::new("Object")]));
    add(
        "int",
        template(|_| vec![Type::new("num"), Type::new("Object")]),
    );
    add("Iterable", template(|_| vec![Type::new("Object")]));
    add(
        "List",
        template(|args| {
            vec![
                PrimaryType::new(TypeRegistry::iterable(), args.to_vec()).into_type(),
                Type::new("Object"),
            ]
        }),
    );
    add("Map", template(|_| vec![Type::new("Object")]));
    add("Object", template(|_| vec![]));
    add("num", template(|_| vec![Type::new("Object")]));
    add("StackTrace", template(|_| vec![Type::new("Object")]));
    add("String", template(|_| vec![Type::new("Object")]));
    map
}

/// `TypeSystem._objectQuestionType`. Rust: parsed on each use, because the
/// registry is per test.
fn object_question_type() -> Type {
    Type::new("Object?")
}

/// `TypeSystem._objectType`. Rust: parsed on each use.
fn object_type() -> Type {
    Type::new("Object")
}

/// `PrimaryType(TypeRegistry.future, args: [t])`.
fn future_of(t: Type) -> Type {
    PrimaryType::new(TypeRegistry::future(), vec![t]).into_type()
}

impl TypeSystem {
    /// `TypeSystem()`.
    pub fn new() -> TypeSystem {
        TypeSystem {
            super_interface_templates: core_super_interface_templates(),
        }
    }

    /// `addSuperInterfaces`.
    pub fn add_super_interfaces(
        &mut self,
        class_name: &str,
        template: impl Fn(&[Type]) -> Vec<Type> + 'static,
    ) {
        self.super_interface_templates
            .insert(Name::new(class_name), Rc::new(template));
    }

    /// If [t] derives a future type `F` (as defined in the "Function
    /// Expressions" section of the language spec), returns `F`. Otherwise
    /// returns `None`.
    pub fn derived_future_type(&self, t: Type) -> Option<Type> {
        // (Note: comments below are pulled from the definition of "derives a
        // future type" in the "Function Expressions" section of the language
        // spec.)

        // We say that a type T derives a future type F in the following
        // cases, using the first applicable case:
        // - If T is a type which is introduced by a class, mixin, or enum
        //   declaration, and if T or a direct or indirect superinterface of T
        //   is Future<U> for some U, then T derives the future type Future<U>.
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

        // - If T is the type FutureOr<U> for some U, then T derives the future
        //   type FutureOr<U>.
        if t.is_future_or_type() {
            return Some(t);
        }

        // - If T is S? for some S, and S derives the future type F, then T
        //   derives the future type F?.
        if t.is_question_type()
            && let Some(f) = self.derived_future_type(t.as_question_type(false))
        {
            return Some(f.as_question_type(true));
        }

        // - If T is a type variable with bound B, and B derives the future
        //   type F, then T derives the future type F.
        if let Some(tpt) = t.as_type_parameter_type()
            && let Some(f) = self.derived_future_type(tpt.bound())
        {
            return Some(f);
        }

        None
    }

    /// `factor`.
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
            && self.is_subtype(future_of(r), s)
        {
            return self.factor(r, s);
        }

        // Else if T is FutureOr<R> and R <: S then factor(Future<R>, S)
        if let Some(r) = t.future_or_type_argument()
            && self.is_subtype(r, s)
        {
            return self.factor(future_of(r), s);
        }

        // Else T
        t
    }

    /// `isSubtype`.
    pub fn is_subtype(&self, t0: Type, t1: Type) -> bool {
        let d0 = t0.data();
        let d1 = t1.data();
        let tpt0: Option<&TypeParameterType> = match &*d0 {
            TypeData::TypeParameter(t) => Some(t),
            _ => None,
        };
        let tpt1: Option<&TypeParameterType> = match &*d1 {
            TypeData::TypeParameter(t) => Some(t),
            _ => None,
        };
        let primary0: Option<&PrimaryType> = match &*d0 {
            TypeData::Primary(p) => Some(p),
            _ => None,
        };
        let primary1: Option<&PrimaryType> = match &*d1 {
            TypeData::Primary(p) => Some(p),
            _ => None,
        };

        // Reflexivity: if T0 and T1 are the same type then T0 <: T1
        //
        // - Note that this check is necessary as the base case for primitive
        //   types, and type variables but not for composite types.  We only
        //   check it for types with a single name and no type arguments (this
        //   covers both primitive types and type variables).
        if t0.is_invalid_type() || t1.is_invalid_type() {
            // `InvalidType` is treated as a top and a bottom type, which is
            // consistent with CFE and analyzer implementations.
            return true;
        }
        if let (Some(p0), Some(p1)) = (primary0, primary1)
            && !p0.is_question_type
            && p0.args.is_empty()
            && !p1.is_question_type
            && p1.args.is_empty()
            && p0.name_info == p1.name_info
        {
            return true;
        }
        if let (Some(x0), Some(x1)) = (tpt0, tpt1)
            && x0.promotion.is_none()
            && !x0.is_question_type
            && x1.promotion.is_none()
            && !x1.is_question_type
            && x0.type_parameter == x1.type_parameter
        {
            return true;
        }

        // Unknown types (note: this is not in the spec, but necessary because
        // there are circumstances where we do subtype tests between types and
        // type schemas): if T0 or T1 is the unknown type then T0 <: T1.
        if t0.is_unknown_type() || t1.is_unknown_type() {
            return true;
        }

        // Right Top: if T1 is a top type (i.e. dynamic, or void, or Object?)
        // then T0 <: T1
        if self.is_top(t1) {
            return true;
        }

        // Left Top: if T0 is dynamic or void then T0 <: T1 if Object? <: T1
        if t0.is_dynamic_type() || t0.is_void_type() {
            return self.is_subtype(object_question_type(), t1);
        }

        // Left Bottom: if T0 is Never then T0 <: T1
        if t0.is_never_type() && !t0.is_question_type() {
            return true;
        }

        // Right Object: if T1 is Object then:
        if let Some(p1) = primary1
            && !p1.is_question_type
            && p1.args.is_empty()
            && p1.name() == "Object"
        {
            // - if T0 is an unpromoted type variable with bound B then T0 <: T1
            //   iff B <: Object
            if let Some(x0) = tpt0
                && x0.promotion.is_none()
                && !x0.is_question_type
            {
                return self.is_subtype(x0.bound(), object_type());
            }

            // - if T0 is a promoted type variable X & S then T0 <: T1 iff
            //   S <: Object
            if let Some(x0) = tpt0
                && let Some(s) = x0.promotion
                && !x0.is_question_type
            {
                return self.is_subtype(s, object_type());
            }

            // - if T0 is FutureOr<S> for some S, then T0 <: T1 iff S <: Object.
            if let Some(s) = t0.future_or_type_argument()
                && !t0.is_question_type()
            {
                return self.is_subtype(s, object_type());
            }

            // - if T0 is Null, dynamic, void, or S? for any S, then the
            //   subtyping does not hold (per above, the result of the
            //   subtyping query is false).
            if t0.is_null_type()
                || t0.is_dynamic_type()
                || t0.is_void_type()
                || t0.is_question_type()
            {
                return false;
            }

            // - Otherwise T0 <: T1 is true.
            return true;
        }

        // Left Null: if T0 is Null then:
        if t0.is_null_type() {
            // - if T1 is a type variable (promoted or not) the query is false
            if let Some(x1) = tpt1
                && !x1.is_question_type
            {
                return false;
            }

            // - If T1 is FutureOr<S> for some S, then the query is true iff
            //   Null <: S.
            if let Some(s) = t1.future_or_type_argument()
                && !t1.is_question_type()
            {
                return self.is_subtype(NullType::instance(), s);
            }

            // - If T1 is Null or S? for some S, then the query is true.
            if t1.is_null_type() || t1.is_question_type() {
                return true;
            }

            // - Otherwise, the query is false
            return false;
        }

        // Left FutureOr: if T0 is FutureOr<S0> then:
        if let Some(s0) = t0.future_or_type_argument()
            && !t0.is_question_type()
        {
            // - T0 <: T1 iff Future<S0> <: T1 and S0 <: T1
            return self.is_subtype(future_of(s0), t1) && self.is_subtype(s0, t1);
        }

        // Left Nullable: if T0 is S0? then:
        if t0.is_question_type() {
            // - T0 <: T1 iff S0 <: T1 and Null <: T1
            return self.is_subtype(t0.as_question_type(false), t1)
                && self.is_subtype(NullType::instance(), t1);
        }

        // Type Variable Reflexivity 1: if T0 is a type variable X0 or a
        // promoted type variables X0 & S0 and T1 is X0 then:
        if let (Some(x0), Some(x1)) = (tpt0, tpt1)
            && !x0.is_question_type
            && x1.promotion.is_none()
            && !x1.is_question_type
            && x0.type_parameter == x1.type_parameter
        {
            // - T0 <: T1
            return true;
        }

        // Type Variable Reflexivity 2: if T0 is a type variable X0 or a
        // promoted type variables X0 & S0 and T1 is X0 & S1 then:
        if let (Some(x0), Some(x1)) = (tpt0, tpt1)
            && !x0.is_question_type
            && let Some(s1) = x1.promotion
            && !x1.is_question_type
            && x0.type_parameter == x1.type_parameter
        {
            // - T0 <: T1 iff T0 <: S1.
            return self.is_subtype(t0, s1);
        }

        // Right Promoted Variable: if T1 is a promoted type variable X1 & S1
        // then:
        if let Some(x1) = tpt1
            && let Some(s1) = x1.promotion
            && !x1.is_question_type
        {
            // - T0 <: T1 iff T0 <: X1 and T0 <: S1
            return self.is_subtype(t0, TypeParameterType::new(x1.type_parameter).into_type())
                && self.is_subtype(t0, s1);
        }

        // Right FutureOr: if T1 is FutureOr<S1> then:
        if let Some(s1) = t1.future_or_type_argument()
            && !t1.is_question_type()
        {
            // - T0 <: T1 iff any of the following hold:
            //   - either T0 <: Future<S1>
            if self.is_subtype(t0, future_of(s1)) {
                return true;
            }
            //   - or T0 <: S1
            if self.is_subtype(t0, s1) {
                return true;
            }
            //   - or T0 is X0 and X0 has bound S0 and S0 <: T1
            if let Some(x0) = tpt0
                && x0.promotion.is_none()
                && self.is_subtype(x0.bound(), t1)
            {
                return true;
            }
            //   - or T0 is X0 & S0 and S0 <: T1
            if let Some(x0) = tpt0
                && let Some(s0) = x0.promotion
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
            if let Some(x0) = tpt0
                && x0.promotion.is_none()
                && self.is_subtype(x0.bound(), t1)
            {
                return true;
            }
            //   - or T0 is X0 & S0 and S0 <: T1
            if let Some(x0) = tpt0
                && let Some(s0) = x0.promotion
                && self.is_subtype(s0, t1)
            {
                return true;
            }
            return false;
        }

        // Left Promoted Variable: T0 is a promoted type variable X0 & S0
        if let Some(x0) = tpt0
            && let Some(s0) = x0.promotion
        {
            // - and S0 <: T1
            if self.is_subtype(s0, t1) {
                return true;
            }
        }

        // Left Type Variable Bound: T0 is a type variable X0 with bound B0
        if let Some(x0) = tpt0
            && x0.promotion.is_none()
        {
            // - and B0 <: T1
            if self.is_subtype(x0.bound(), t1) {
                return true;
            }
        }

        // Function Type/Function: T0 is a function type and T1 is Function
        if t0.is_function_type()
            && let Some(p1) = primary1
            && p1.args.is_empty()
            && p1.name() == "Function"
        {
            return true;
        }

        // Record Type/Record: T0 is a record type and T1 is Record
        if t0.is_record_type()
            && let Some(p1) = primary1
            && p1.args.is_empty()
            && p1.name() == "Record"
        {
            return true;
        }

        let is_interface_compositionality_subtype = || {
            // Interface Compositionality: T0 is an interface type
            // C0<S0, ..., Sk> and T1 is C0<U0, ..., Uk>
            let (Some(p0), Some(p1)) = (primary0, primary1) else {
                return false;
            };
            if p0.args.len() != p1.args.len() || p0.name() != p1.name() {
                return false;
            }
            // - and each Si <: Ui
            for i in 0..p0.args.len() {
                if !self.is_subtype(p0.args[i], p1.args[i]) {
                    return false;
                }
            }
            true
        };

        if is_interface_compositionality_subtype() {
            return true;
        }

        // Super-Interface: T0 is an interface type with super-interfaces
        // S0,...Sn
        let is_super_interface_subtype = || {
            let Some(p0) = primary0 else {
                return false;
            };
            let super_interfaces = self.get_super_interfaces(p0);

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

        let function0: Option<&FunctionType> = match &*d0 {
            TypeData::Function(f) => Some(f),
            _ => None,
        };
        let function1: Option<&FunctionType> = match &*d1 {
            TypeData::Function(f) => Some(f),
            _ => None,
        };

        let is_positional_function_subtype = || {
            // Positional Function Types: T0 is U0 Function<X0 extends B00,
            // ..., Xk extends B0k>(V0 x0, ..., Vn xn, [Vn+1 xn+1, ..., Vm xm])
            let Some(f0) = function0 else {
                return false;
            };
            if !f0.named_parameters.is_empty() {
                return false;
            }
            let n = f0.required_positional_parameter_count;
            let m = f0.positional_parameters.len();

            // - and T1 is U1 Function<Y0 extends B10, ..., Yk extends
            //   B1k>(S0 y0, ..., Sp yp, [Sp+1 yp+1, ..., Sq yq])
            let Some(f1) = function1 else {
                return false;
            };
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

            // (Note: no substitution is needed in the code below; we don't
            // support type arguments on function types)

            // - and Si[Z0/Y0, ..., Zk/Yk] <: Vi[Z0/X0, ..., Zk/Xk] for i in
            //   0...q
            for i in 0..q {
                if !self.is_subtype(f1.positional_parameters[i], f0.positional_parameters[i]) {
                    return false;
                }
            }

            // - and U0[Z0/X0, ..., Zk/Xk] <: U1[Z0/Y0, ..., Zk/Yk]
            if !self.is_subtype(f0.return_type, f1.return_type) {
                return false;
            }

            // - and B0i[Z0/X0, ..., Zk/Xk] === B1i[Z0/Y0, ..., Zk/Yk] for i in
            //   0...k
            // - where the Zi are fresh type variables with bounds B0i[Z0/X0,
            //   ..., Zk/Xk]
            // (No check needed here since we don't support type arguments on
            // function types)
            true
        };

        if is_positional_function_subtype() {
            return true;
        }

        let is_named_function_subtype = || {
            // Named Function Types: T0 is U0 Function<X0 extends B00, ..., Xk
            // extends B0k>(V0 x0, ..., Vn xn, {r0n+1 Vn+1 xn+1, ..., r0m Vm
            // xm}) where r0j is empty or required for j in n+1...m
            let Some(f0) = function0 else {
                return false;
            };
            let n = f0.positional_parameters.len();
            if f0.required_positional_parameter_count != n {
                return false;
            }

            // - and T1 is U1 Function<Y0 extends B10, ..., Yk extends
            //   B1k>(S0 y0, ..., Sn yn, {r1n+1 Sn+1 yn+1, ..., r1q Sq yq})
            //   where r1j is empty or required for j in n+1...q
            let Some(f1) = function1 else {
                return false;
            };
            if f1.positional_parameters.len() != n || f1.required_positional_parameter_count != n {
                return false;
            }

            // - and {yn+1, ... , yq} subsetof {xn+1, ... , xm}
            let mut t1_index_to_t0_index = Vec::new();
            let (mut i, mut j) = (0, 0);
            while i < f0.named_parameters.len() || j < f1.named_parameters.len() {
                if i >= f0.named_parameters.len() {
                    break;
                }
                if j >= f1.named_parameters.len() {
                    return false;
                }
                match f0.named_parameters[i]
                    .name
                    .cmp(&f1.named_parameters[j].name)
                {
                    std::cmp::Ordering::Less => i += 1,
                    std::cmp::Ordering::Greater => return false,
                    std::cmp::Ordering::Equal => {
                        t1_index_to_t0_index.push(i);
                        i += 1;
                        j += 1;
                    }
                }
            }

            // (Note: no substitution is needed in the code below; we don't
            // support type arguments on function types)

            // - and Si[Z0/Y0, ..., Zk/Yk] <: Vi[Z0/X0, ..., Zk/Xk] for i in
            //   0...n
            for i in 0..n {
                if !self.is_subtype(f1.positional_parameters[i], f0.positional_parameters[i]) {
                    return false;
                }
            }

            // - and Si[Z0/Y0, ..., Zk/Yk] <: Tj[Z0/X0, ..., Zk/Xk] for i in
            //   n+1...q, yj = xi
            for (j, &i) in t1_index_to_t0_index.iter().enumerate() {
                if !self.is_subtype(f1.named_parameters[j].type_, f0.named_parameters[i].type_) {
                    return false;
                }
            }

            // - and for each j such that r0j is required, then there exists an
            //   i in n+1...q such that xj = yi, and r1i is required
            for (j, &i) in t1_index_to_t0_index.iter().enumerate() {
                if f1.named_parameters[j].is_required && !f0.named_parameters[i].is_required {
                    return false;
                }
            }

            // - and U0[Z0/X0, ..., Zk/Xk] <: U1[Z0/Y0, ..., Zk/Yk]
            if !self.is_subtype(f0.return_type, f1.return_type) {
                return false;
            }

            // - and B0i[Z0/X0, ..., Zk/Xk] === B1i[Z0/Y0, ..., Zk/Yk] for i in
            //   0...k
            // - where the Zi are fresh type variables with bounds B0i[Z0/X0,
            //   ..., Zk/Xk]
            // (No check needed here since we don't support type arguments on
            // function types)
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
            let (TypeData::Record(r0), TypeData::Record(r1)) = (&*d0, &*d1) else {
                return false;
            };
            let (r0, r1): (&RecordType, &RecordType) = (r0, r1);
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
            let t1_named_map: IndexMap<Name, Type> =
                r1.named_types.iter().map(|n| (n.name, n.type_)).collect();
            for named in &r0.named_types {
                let Some(&si) = t1_named_map.get(&named.name) else {
                    return false;
                };
                if !self.is_subtype(named.type_, si) {
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

    /// `_getSuperInterfaces`. Panics if the super-interfaces of the class
    /// are not known (Dart: `fail`).
    fn get_super_interfaces(&self, t: &PrimaryType) -> Vec<Type> {
        match self.super_interface_templates.get(&t.name()) {
            Some(super_interface_template) => super_interface_template(&t.args),
            None => panic!(
                "Superinterfaces for {} not known",
                PrimaryType::clone(t).into_type()
            ),
        }
    }

    /// `_isTop`.
    ///
    /// Note: like Dart, this returns `true` only for `dynamic`, the invalid
    /// type and `void` (`Object?` is a `PrimaryType`, so the second branch of
    /// the Dart code never matches).
    fn is_top(&self, t: Type) -> bool {
        if t.is_primary_type() {
            t.is_dynamic_type() || t.is_invalid_type() || t.is_void_type()
        } else if t.is_question_type() {
            matches!(t.as_primary_type(), Some(p) if p.args.is_empty() && p.name() == "Object")
        } else {
            false
        }
    }
}
