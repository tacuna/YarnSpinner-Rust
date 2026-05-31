#![allow(warnings)]
#![allow(clippy)]
//! Equivalent to <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner/YarnSpinner.cs>

use crate::prelude::*;
mod ext;
pub use self::ext::*;

include!("yarn.rs");
