//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner/Types/AnyType.cs>

use crate::types::TypeProperties;

/// Represents any type. this type is used in circumstances when a type
/// is known to have a value, but the specific type is not known or
/// required to be known.
pub(crate) fn any_type_properties() -> TypeProperties {
    TypeProperties::from_name("Any").with_description("Any type.")
}
