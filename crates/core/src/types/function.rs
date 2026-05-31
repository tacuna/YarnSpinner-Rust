//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner/Types/FunctionType.cs>
use crate::prelude::*;
use crate::types::{Type, TypeFormat, TypeProperties};
use core::fmt::Display;

pub(crate) fn function_type_properties(function_type: &FunctionType) -> TypeProperties {
    TypeProperties::from_name("Function").with_description(function_type.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
#[cfg_attr(feature = "bevy", derive(Reflect))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "bevy", reflect(Debug, PartialEq, Default, Hash))]
#[cfg_attr(all(feature = "bevy", feature = "serde"), reflect(Serialize, Deserialize))]
/// A type that represents functions.
///
/// Functions have parameters and a return type, and can be called from
/// script. Instances of this type are created when the host
/// application registers new functions (such as through using the [`Library::add_function`] methods or similar.)
pub struct FunctionType {
    #[cfg_attr(feature = "bevy", reflect(ignore))]
    /// The list of the parameter types that this function is called with.
    ///
    /// The length of this list also determines the number of parameters this function accepts
    /// (also known as the function's *arity*).
    pub parameters: Vec<Option<Type>>,

    #[cfg_attr(feature = "bevy", reflect(ignore))]
    ///The type of value that this function returns.
    // Needs to be on the heap because of type recursion
    pub return_type: Box<Option<Type>>,

    #[cfg_attr(feature = "bevy", reflect(ignore))]
    /// If `Some`, this function accepts additional variadic arguments of this type after all fixed
    /// [`parameters`](Self::parameters). A `None` value means the function is not variadic.
    ///
    /// Stored behind a [`Box`] to avoid a recursive-type error (since [`Type`] may contain
    /// a [`FunctionType`]).
    pub variadic_parameter_type: Option<Box<Type>>,
}

impl From<FunctionType> for Type {
    fn from(function_type: FunctionType) -> Self {
        Type::Function(function_type)
    }
}

impl FunctionType {
    /// Sets the return type of this function signature
    pub fn set_return_type(&mut self, return_type: impl Into<Option<Type>>) -> &mut Self {
        *self.return_type = return_type.into();
        self
    }

    /// Adds a parameter type to this function signature
    pub fn add_parameter(&mut self, parameter: impl Into<Option<Type>>) -> &mut Self {
        self.parameters.push(parameter.into());
        self
    }
}

impl Display for FunctionType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut params: Vec<String> = self.parameters.iter().map(TypeFormat::format).collect();
        if let Some(var_type) = self.variadic_parameter_type.as_deref() {
            params.push(format!("{}...", var_type.format()));
        }
        let parameters = params.join(", ");
        let return_type = self.return_type.as_ref().format();
        write!(f, "Fn({parameters}) -> {return_type}")
    }
}
