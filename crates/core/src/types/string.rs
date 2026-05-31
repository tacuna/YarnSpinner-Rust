//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner/Types/StringType.cs>

use crate::prelude::*;
use crate::types::TypeProperties;

/// A type that bridges to [`String`]
pub(crate) fn string_type_properties() -> TypeProperties {
    TypeProperties::from_name("String").with_methods(yarn_library! {
        Operator::EqualTo => <RustType as PartialEq>::eq,
        Operator::NotEqualTo => <RustType as PartialEq>::ne,
        Operator::Add => |a: RustType, b: RustType| a + &b,
    })
}

type RustType = String;
