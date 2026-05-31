//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner/Types/IType.cs>
//! ## Implementation Notes
//! - `IBridgeableType` is not implemented because it is not actually used anywhere.

pub use function::*;
pub use r#type::*;
pub use type_util::*;

mod any;
mod boolean;
mod function;
mod number;
mod string;
mod r#type;
mod type_util;
