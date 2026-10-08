// Dart source: pkg/_fe_analyzer_shared/lib/src/exhaustiveness/dart_template_buffer.dart

use std::any::Any;

/// An accumulator of a Dart code template, consisting of strings interspersed
/// with semantic nodes representing references to part of the user's program.
///
/// This is similar to a StringBuffer, except that:
/// - [write] requires a string argument (to prevent mistakes)
/// - additional methods are provided to allow writing semantic nodes that refer
///   to classes, enumerated values, etc.  These semantic nodes will be
///   converted to simple strings at a later time.
///
/// Clients who do not need the additional semantic information may obtain the
/// final string immediately using [SimpleDartBuffer].
///
/// The Dart type parameters `ConstantValue`, `EnumValue` and `Type` are
/// erased to [`Any`]: the exhaustiveness checker is not generic, so the
/// values reach the buffer type-erased and a client downcasts them to its
/// constant value, enum element and type representations.
pub trait DartTemplateBuffer {
    /// Adds a text string to the buffer.
    fn write(&mut self, text: &str);

    /// Adds a semantic node representing the given boolean value to the buffer.
    fn write_bool_value(&mut self, value: bool);

    /// Adds a semantic node representing the given core type to the buffer.
    ///
    /// Ideally, callers should use [write_general_type] instead, since it
    /// allows `Type` information to be associated with the semantic node.
    /// However, the exhaustiveness algorithm currently uses this method for the
    /// core types `Object`, `Never`, and `Null`, because its representation of
    /// those types doesn't track the necessary `Type` semantics.
    fn write_core_type(&mut self, name: &str);

    /// Adds a semantic node representing the given enumerated [value] to the
    /// buffer.
    ///
    /// [name] is a simple string representation of the enumerated value.
    fn write_enum_value(&mut self, value: &dyn Any, name: &str);

    /// Adds a semantic node representing the given constant [value] to the
    /// buffer.
    ///
    /// [name] is a simple string representation of constant value.
    ///
    /// This is used for any constants that are not enum values or booleans.
    fn write_general_constant_value(&mut self, value: &dyn Any, name: &str);

    /// Adds a semantic node representing the given [type] to the buffer.
    ///
    /// [name] is a simple string representation of the type.
    fn write_general_type(&mut self, type_: &dyn Any, name: &str);
}

/// An accumulator of a Dart code template that discards semantic information,
/// immediately producing a simple string.
#[derive(Default, Debug)]
pub struct SimpleDartBuffer {
    buffer: String,
}

impl SimpleDartBuffer {
    pub fn new() -> SimpleDartBuffer {
        SimpleDartBuffer::default()
    }
}

impl std::fmt::Display for SimpleDartBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.buffer)
    }
}

impl DartTemplateBuffer for SimpleDartBuffer {
    fn write(&mut self, text: &str) {
        self.buffer.push_str(text);
    }

    fn write_bool_value(&mut self, value: bool) {
        self.buffer.push_str(if value { "true" } else { "false" });
    }

    fn write_core_type(&mut self, name: &str) {
        self.buffer.push_str(name);
    }

    fn write_enum_value(&mut self, _value: &dyn Any, name: &str) {
        self.buffer.push_str(name);
    }

    fn write_general_constant_value(&mut self, _value: &dyn Any, name: &str) {
        self.buffer.push_str(name);
    }

    fn write_general_type(&mut self, _type: &dyn Any, name: &str) {
        self.buffer.push_str(name);
    }
}
