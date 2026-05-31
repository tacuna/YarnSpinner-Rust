//! The core components of Yarn Spinner, used for both the compiler and the runtime. These mostly follow the same structure as in the original Yarn Spinner.
//!
//! You probably don't want to use this crate directly.
//! - If you're a game developer, you'll want to use a crate that is already designed for your game engine of choice,
//!   such as [`bevy_yarnspinner`](https://crates.io/crates/bevy_yarnspinner) for the [Bevy engine](https://bevyengine.org/).
//! - If you wish to write an adapter crate for an engine yourself, use the [`yarnspinner`](https://crates.io/crates/yarnspinner) crate.

#![warn(missing_docs, missing_debug_implementations)]
#![no_std]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

mod feature_gates;
mod generated;
mod internal_value;
mod library;
mod line_id;
mod operator;
mod position;
pub mod types;
mod yarn_fn;
mod yarn_value;

pub mod prelude {
    //! Types and functions used all throughout the runtime and compiler.
    #[cfg(any(feature = "bevy", feature = "serde"))]
    pub use crate::feature_gates::*;

    // Re-export alloc types for internal use only
    pub(crate) use crate::alloc::borrow::ToOwned;
    pub(crate) use crate::alloc::boxed::Box;
    pub(crate) use crate::alloc::string::{String, ToString};
    pub(crate) use crate::alloc::vec::Vec;
    pub(crate) use crate::alloc::{format, vec};

    pub use crate::generated::instruction::OpCode;
    pub use crate::generated::operand::Value as OperandValue;
    pub use crate::generated::{Header, Instruction, InvalidOpCodeError, Node, Operand, Program};
    pub use crate::internal_value::*;
    pub use crate::library::*;
    pub use crate::line_id::*;
    pub use crate::operator::*;
    pub use crate::position::*;
    pub use crate::types::Type;
    pub use crate::yarn_fn::*;
    pub use crate::yarn_value::*;
}
