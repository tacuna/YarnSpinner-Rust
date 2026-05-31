//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner/Types/BooleanType.cs>

use crate::prelude::*;
use crate::types::TypeProperties;
use core::ops::*;

/// A type that bridges to [`bool`]
pub(crate) fn boolean_type_properties() -> TypeProperties {
    TypeProperties::from_name("Bool").with_methods(yarn_library! {
        Operator::EqualTo => <RustType as PartialEq>::eq,
        Operator::NotEqualTo => <RustType as PartialEq>::ne,
        Operator::And => <RustType as BitAnd>::bitand,
        Operator::Or => <RustType as BitOr>::bitor,
        Operator::Xor => <RustType as BitXor>::bitxor,
        Operator::Not => RustType::not,
    })
}

type RustType = bool;
