// Dart source: pkg/analyzer/lib/src/dart/constant/from_environment_evaluator.dart

//! [`FromEnvironmentEvaluator`]: `bool.fromEnvironment`,
//! `int.fromEnvironment`, `String.fromEnvironment`, `bool.hasEnvironment`.

use crate::dart_num;
use crate::declared_variables::DeclaredVariables;
use crate::type_system::ConstTypeSystem;
use crate::value::{BoolState, DartObjectImpl, InstanceState, IntState, StringState};

/// Dart `FromEnvironmentEvaluator`.
pub struct FromEnvironmentEvaluator<'a> {
    type_system: &'a dyn ConstTypeSystem,
    declared_variables: &'a DeclaredVariables,
}

impl<'a> FromEnvironmentEvaluator<'a> {
    pub fn new(
        type_system: &'a dyn ConstTypeSystem,
        declared_variables: &'a DeclaredVariables,
    ) -> FromEnvironmentEvaluator<'a> {
        FromEnvironmentEvaluator {
            type_system,
            declared_variables,
        }
    }

    fn lookup(&self, name: Option<&str>) -> Option<&'a str> {
        name.and_then(|n| self.declared_variables.get(n))
    }

    /// Dart `getBool`: the value of the variable with the given [name]
    /// interpreted as a 'boolean' value.
    ///
    /// If the variable is not defined, or the value cannot be parsed as a
    /// boolean, return [default_value]. If [default_value] is `None`, return a
    /// `false` object.
    pub fn get_bool(
        &self,
        name: Option<&str>,
        default_value: Option<DartObjectImpl>,
    ) -> DartObjectImpl {
        let ts = self.type_system;
        let bool_type = ts.ctx().tp.bool_type();
        let str = self.lookup(name);
        if str == Some("true") {
            return DartObjectImpl::new(ts, bool_type, InstanceState::Bool(BoolState::TRUE_STATE));
        }
        if str == Some("false") {
            return DartObjectImpl::new(ts, bool_type, InstanceState::Bool(BoolState::FALSE_STATE));
        }
        if let Some(default_value) = default_value {
            return default_value;
        }
        DartObjectImpl::new(ts, bool_type, InstanceState::Bool(BoolState::FALSE_STATE))
    }

    /// Dart `getInt`: the value of the variable with the given [name]
    /// interpreted as an integer value (Dart `int.parse`).
    ///
    /// If the variable is not defined, or the value cannot be parsed as an
    /// integer, return [default_value]. If [default_value] is `None`, return
    /// a `0` object.
    pub fn get_int(
        &self,
        name: Option<&str>,
        default_value: Option<DartObjectImpl>,
    ) -> DartObjectImpl {
        let ts = self.type_system;
        let int_type = ts.ctx().tp.int_type();
        if let Some(str) = self.lookup(name)
            && let Some(value) = dart_num::int_parse(str)
        {
            return DartObjectImpl::new(
                ts,
                int_type,
                InstanceState::Int(IntState::new(Some(value))),
            );
        }
        if let Some(default_value) = default_value {
            return default_value;
        }
        DartObjectImpl::new(ts, int_type, InstanceState::Int(IntState::new(Some(0))))
    }

    /// Dart `getString`: the value of the variable with the given [name]
    /// interpreted as a string value.
    ///
    /// If the variable is not defined, return [default_value]. If
    /// [default_value] is `None`, return an empty string object.
    pub fn get_string(
        &self,
        name: Option<&str>,
        default_value: Option<DartObjectImpl>,
    ) -> DartObjectImpl {
        let ts = self.type_system;
        let string_type = ts.ctx().tp.string_type();
        if let Some(str) = self.lookup(name) {
            return DartObjectImpl::new(
                ts,
                string_type,
                InstanceState::String(StringState::new(str)),
            );
        }
        if let Some(default_value) = default_value {
            return default_value;
        }
        DartObjectImpl::new(ts, string_type, InstanceState::String(StringState::new("")))
    }

    /// Dart `hasEnvironment`.
    pub fn has_environment(&self, name: Option<&str>) -> DartObjectImpl {
        let ts = self.type_system;
        let value = self.lookup(name).is_some();
        DartObjectImpl::new(
            ts,
            ts.ctx().tp.bool_type(),
            InstanceState::Bool(BoolState::new(Some(value))),
        )
    }
}
