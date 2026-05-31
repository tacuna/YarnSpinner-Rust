//! Public types for user-defined enum types, mirroring the C# `EnumType` and `EnumTypeBuilder`.

use yarnspinner_core::prelude::*;

/// A single case of a user-defined enum type.
///
/// Mirrors C# `ConstantTypeProperty`.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumCase {
    /// The name of this case (e.g. `"One"`).
    pub name: String,
    /// The raw value that this case resolves to at compile time.
    /// This is either a [`YarnValue::Number`] or a [`YarnValue::String`].
    pub raw_value: YarnValue,
    /// An optional human-readable description of this case.
    pub description: String,
}

/// A user-defined enum type, composed of a set of named cases each carrying a
/// constant raw value.
///
/// Mirrors C# `EnumType`.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumType {
    /// The name of this enum (e.g. `"MyEnum"`).
    pub name: String,
    /// An optional human-readable description of this enum.
    pub description: String,
    /// The raw value type for all cases.
    ///
    /// This will always be [`Type::Number`] or [`Type::String`].
    pub raw_type: Type,
    /// The cases of this enum, in declaration order.
    pub cases: Vec<EnumCase>,
}

/// A fluent builder for [`EnumType`].
///
/// Mirrors C# `EnumTypeBuilder`.
///
/// # Example
/// ```ignore
/// let my_enum = EnumTypeBuilder::new()
///     .with_name("Color")
///     .with_description("A color enum")
///     .with_raw_type(Type::String)
///     .with_case("Red", "red", "The color red")
///     .with_case("Green", "green", "The color green")
///     .build();
/// ```
#[derive(Debug, Default)]
pub struct EnumTypeBuilder {
    name: String,
    description: String,
    raw_type: Option<Type>,
    cases: Vec<EnumCase>,
}

impl EnumTypeBuilder {
    /// Creates a new, empty [`EnumTypeBuilder`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the name of the enum being built.
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    /// Sets the description of the enum being built.
    pub fn with_description(mut self, description: &str) -> Self {
        self.description = description.to_string();
        self
    }

    /// Sets the raw value type for all cases (`Type::Number` or `Type::String`).
    pub fn with_raw_type(mut self, raw_type: Type) -> Self {
        self.raw_type = Some(raw_type);
        self
    }

    /// Adds a case to the enum being built.
    ///
    /// - `name`: the case name (e.g. `"One"`)
    /// - `raw_value`: the raw value; any type that converts to [`YarnValue`]
    ///   (e.g. `"one"` for string enums or `1.0_f32` for number enums)
    /// - `description`: an optional human-readable description of the case
    pub fn with_case(mut self, name: &str, raw_value: impl Into<YarnValue>, description: &str) -> Self {
        self.cases.push(EnumCase {
            name: name.to_string(),
            raw_value: raw_value.into(),
            description: description.to_string(),
        });
        self
    }

    /// Consumes the builder and returns the constructed [`EnumType`].
    pub fn build(self) -> EnumType {
        EnumType {
            name: self.name,
            description: self.description,
            raw_type: self.raw_type.unwrap_or(Type::Number),
            cases: self.cases,
        }
    }

    /// Alias for [`build()`](Self::build), mirroring the C# `.EnumType` property.
    pub fn enum_type(self) -> EnumType {
        self.build()
    }
}
